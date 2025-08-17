# EDB RPC Proxy - Developer Guide

This document provides deep technical insights into the RPC Proxy architecture, design decisions, and implementation details for developers working on or extending the codebase.

_This document is created by Claude with ❤️._

## 📋 Table of Contents

- [Architecture Overview](#architecture-overview)
- [Process Model & Concurrency](#process-model--concurrency)
- [Module Breakdown](#module-breakdown)
- [Design Patterns](#design-patterns)
- [Intelligent Caching System](#intelligent-caching-system)
- [Weighted Provider Management](#weighted-provider-management)
- [Error Handling Strategy](#error-handling-strategy)
- [Metrics & Observability](#metrics--observability)
- [Testing Strategy](#testing-strategy)
- [Performance Considerations](#performance-considerations)
- [Extension Points](#extension-points)

## 🏗️ Architecture Overview

### Core Design Principles

1. **Separation of Concerns**: Each module has a single, well-defined responsibility
2. **Async-First**: Built on tokio for high-concurrency performance
3. **Fault Tolerance**: Designed to degrade gracefully under failures
4. **Zero-Copy Where Possible**: Minimize allocations in hot paths
5. **Configuration-Driven**: Behavior controlled through builder pattern
6. **Testability**: Each component can be tested in isolation
7. **Observability**: Comprehensive metrics and real-time monitoring

### Layered Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        HTTP Layer                               │ 
│  ├─ Axum Router (JSON-RPC + Management endpoints)               │
│  ├─ Request/Response serialization                              │
│  └─ CORS and middleware                                         │
├─────────────────────────────────────────────────────────────────┤
│                     Application Layer                           │
│  ├─ ProxyServer (orchestrates all components)                   │
│  ├─ ProxyServerBuilder (fluent configuration API)               │
│  ├─ TUI Application (real-time monitoring)                      │
│  └─ Background task coordination                                │
├─────────────────────────────────────────────────────────────────┤
│                       Domain Layer                              │
│  ├─ RpcHandler (request routing and caching logic)              │
│  ├─ CacheManager (LRU + persistence)                            │
│  ├─ ProviderManager (weighted selection + health monitoring)    │
│  ├─ EDBRegistry (instance lifecycle management)                 │
│  ├─ HealthService (service monitoring)                          │
│  └─ MetricsCollector (performance tracking)                     │
├─────────────────────────────────────────────────────────────────┤
│                    Infrastructure Layer                         │
│  ├─ HTTP Client (reqwest for upstream requests)                 │
│  ├─ File System (atomic writes for cache persistence)           │
│  ├─ Process Management (cross-platform process checking)        │
│  └─ Time/Scheduling (tokio intervals and timers)                │
└─────────────────────────────────────────────────────────────────┘
```

## ⚙️ Process Model & Concurrency

### Main Process Lifecycle

```rust
#[tokio::main]
async fn main() -> Result<()> {
    // 1. Sequential Initialization Phase
    init_logging();
    let args = parse_cli_args();
    
    match args.command {
        Commands::Server(config) => {
            let proxy = build_proxy_server(config).await?;
            
            // 2. Concurrent Execution Phase
            tokio::select! {
                // Main HTTP server (always runs)
                result = proxy.serve(addr) => result?,
                
                // Optional TUI monitoring
                _ = run_tui_if_enabled() => {},
                
                // Graceful shutdown signal
                _ = tokio::signal::ctrl_c() => {
                    info!("Received shutdown signal");
                }
            }
            
            // 3. Cleanup Phase
            proxy.cache_manager().save_to_disk().await?;
        }
        Commands::Monitor(url) => {
            run_monitoring_tui(url).await?;
        }
    }
    
    Ok(())
}
```

### Background Task Architecture

The proxy spawns multiple concurrent background tasks during initialization:

```rust
impl ProxyServer {
    async fn new(...) -> Result<Self> {
        // Core components initialization...
        
        // Task 1: EDB Heartbeat Monitor (conditional)
        if grace_period > 0 {
            let registry_clone = Arc::clone(&registry);
            tokio::spawn(async move {
                registry_clone.start_heartbeat_monitor(heartbeat_interval).await;
            });
        }
        
        // Task 2: Provider Health Monitor (always)
        let provider_manager_clone = provider_manager.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                Duration::from_secs(health_check_interval)
            );
            loop {
                interval.tick().await;
                provider_manager_clone.health_check_all().await;
            }
        });
        
        // Task 3: Cache Persistence (conditional)
        if cache_save_interval > 0 {
            let cache_manager_clone = cache_manager.clone();
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(
                    Duration::from_secs(cache_save_interval * 60)
                );
                loop {
                    interval.tick().await;
                    if let Err(e) = cache_manager_clone.save_to_disk().await {
                        warn!("Failed to save cache periodically: {}", e);
                    }
                }
            });
        }
        
        Ok(Self { /* ... */ })
    }
}
```

### Thread Safety Model

```rust
// Shared read-heavy data structures
Arc<RwLock<HashMap<String, CacheEntry>>>     // Cache entries
Arc<RwLock<Vec<ProviderInfo>>>               // Provider health status
Arc<RwLock<HashMap<u32, EDBInstance>>>       // EDB instance registry

// Lock-free atomic operations
AtomicUsize                                   // Round-robin counter
AtomicU64                                     // Metrics counters

// Immutable shared data
Arc<CacheManager>                             // Shared ownership
Arc<ProviderManager>                          // Shared ownership
Arc<MetricsCollector>                         // Shared ownership
```

### Lock Hierarchy and Deadlock Prevention

**Lock Ordering Rules**:
1. Cache locks acquired before provider locks
2. Short-lived locks only (no I/O while holding locks)
3. Clone data out of locks for expensive operations

## 📦 Module Breakdown

### 1. `main.rs` - CLI Interface & Application Entry

**Responsibilities**:
- Command-line argument parsing with `clap` using derive macros
- Logging initialization with `tracing`
- Application mode selection (server vs monitor)
- Signal handling and graceful shutdown

**Key Design Decisions**:
- Uses `tokio::select!` for concurrent signal handling
- Subcommand pattern for different modes (server/monitor)
- Comprehensive CLI validation before application start

### 2. `proxy.rs` - Server Orchestration & HTTP Layer

**Responsibilities**:
- Builder pattern implementation for configuration
- HTTP server setup with Axum
- Background task spawning and coordination
- Graceful shutdown coordination

**Design Patterns**:
```rust
// Builder pattern with fluent API
ProxyServerBuilder::new()
    .max_cache_items(500000)
    .grace_period(300)
    .health_check_interval(30)
    .build().await?

// Service composition
struct ProxyServer {
    rpc_handler: Arc<RpcHandler>,        // Core request handling
    registry: Arc<EDBRegistry>,          // Instance management
    health_service: Arc<HealthService>,  // Health endpoints
    metrics_collector: Arc<MetricsCollector>, // Performance tracking
    shutdown_tx: broadcast::Sender<()>,  // Shutdown coordination
}
```

**Axum Router Design**:
```rust
let app = Router::new()
    .route("/", post(handle_rpc))          // Single endpoint for all RPC
    .layer(CorsLayer::permissive())        // CORS for browser clients
    .with_state(AppState { proxy: self }); // Shared state injection
```

### 3. `rpc.rs` - Request Processing Engine

**Responsibilities**:
- RPC method analysis and caching decisions
- Request forwarding with weighted provider selection
- Error detection, classification, and deduplication
- Provider exclusion tracking per request
- Response validation for debug/trace methods

**Core Algorithm**:
```rust
async fn handle_request(&self, request: Value) -> Result<Value> {
    let method = extract_method(&request);
    
    // Step 1: Cacheability analysis
    if !CACHEABLE_METHODS.contains(&method) {
        return self.forward_request(&request).await;
    }
    
    // Step 2: Deterministic parameter check
    if self.has_non_deterministic_block_params(&request) {
        return self.forward_request(&request).await;
    }
    
    // Step 3: Cache lookup
    let cache_key = self.generate_cache_key(&request);
    if let Some(cached) = self.cache_manager.get(&cache_key).await {
        self.metrics_collector.record_cache_hit(method, response_time);
        return Ok(cached);
    }
    
    // Step 4: Provider forwarding with error handling
    let response = self.forward_request(&request).await?;
    
    // Step 5: Response validation and caching
    if self.is_valid_response(&response, &method) {
        self.cache_manager.set(cache_key, response.clone()).await;
    }
    
    Ok(response)
}
```

**Provider Exclusion Algorithm**:
```rust
async fn forward_request(&self, request: &Value) -> Result<Value> {
    let mut tried_providers = HashSet::new();
    let mut error_responses: HashMap<u64, (Value, usize)> = HashMap::new();
    
    for retry in 0..MAX_RETRIES {
        // Get a provider we haven't tried yet
        let provider_url = match self.provider_manager
            .get_weighted_provider_excluding(&tried_providers).await 
        {
            Some(url) => url,
            None => {
                // All providers tried, trigger health check and retry
                self.provider_manager.health_check_all().await;
                tried_providers.clear(); // Allow retry after health check
                continue;
            }
        };
        
        tried_providers.insert(provider_url.clone());
        
        // Try request with this provider...
    }
}
```

### 4. `cache.rs` - Intelligent Caching System

**Responsibilities**:
- In-memory LRU cache management with `accessed_at` timestamps
- Atomic disk persistence with merge logic
- Concurrent access coordination with RwLock
- Cache size management and batch eviction

**LRU Implementation**:
```rust
// Timestamp-based LRU (simpler than linked list)
struct CacheEntry {
    data: Value,
    accessed_at: u64,  // Unix timestamp for LRU ordering
}

// Eviction algorithm - batch eviction for performance
async fn evict_oldest(&self, cache: &mut HashMap<String, CacheEntry>) {
    let to_remove = (cache.len() / 10).max(1);  // Remove 10% at a time
    
    // Sort by timestamp, remove oldest entries
    let mut entries: Vec<_> = cache.iter()
        .map(|(key, entry)| (key.clone(), entry.accessed_at))
        .collect();
    entries.sort_by_key(|(_, timestamp)| *timestamp);
    
    for (key, _) in entries.into_iter().take(to_remove) {
        cache.remove(&key);
    }
}
```

**Atomic Persistence Pattern**:
```rust
async fn save_to_disk(&self) -> Result<()> {
    // 1. Load existing cache (if any)
    let existing_cache = self.load_existing_cache()?;
    
    // 2. Merge with in-memory cache (newest wins by timestamp)
    let merged_cache = self.merge_caches(existing_cache, current_cache);
    
    // 3. Apply size management rules
    let final_cache = self.apply_size_management(merged_cache).await;
    
    // 4. Atomic write (temp file + rename)
    let temp_file = self.cache_file_path.with_extension("tmp");
    fs::write(&temp_file, &serialized_cache)?;
    fs::rename(&temp_file, &self.cache_file_path)?;  // Atomic!
    
    Ok(())
}
```

### 5. `providers.rs` - Weighted Provider Management

**Responsibilities**:
- Provider health monitoring with configurable thresholds
- Weighted provider selection based on response time performance tiers
- Failure detection and automatic recovery
- Provider exclusion for unique-per-request selection

**Performance Tier System**:
```rust
fn get_performance_tier(response_time_ms: u64) -> u8 {
    match response_time_ms / 100 {
        0..=1 => 1,    // 0-199ms: Tier 1 (fastest)
        2..=3 => 2,    // 200-399ms: Tier 2 
        4..=5 => 3,    // 400-599ms: Tier 3
        _ => 4,        // 600ms+: Tier 4 (slowest)
    }
}

fn get_tier_weight(tier: u8) -> u32 {
    match tier {
        1 => 100,  // Fast providers get 100% weight
        2 => 60,   // Medium providers get 60% weight  
        3 => 30,   // Slow providers get 30% weight
        4 => 10,   // Very slow providers get 10% weight
        _ => 1,    // Fallback for unknown tiers
    }
}
```

**Weighted Selection with Exclusion**:
```rust
pub async fn get_weighted_provider_excluding(
    &self,
    tried_providers: &HashSet<String>,
) -> Option<String> {
    let providers = self.providers.read().await;
    let available_providers: Vec<_> = providers.iter()
        .filter(|p| p.is_healthy && !tried_providers.contains(&p.url))
        .collect();

    if available_providers.is_empty() {
        return None;
    }

    // Calculate weights for available providers only
    let mut weighted_providers = Vec::new();
    let mut total_weight = 0u32;

    for provider in &available_providers {
        let response_time = provider.response_time_ms.unwrap_or(300);
        let tier = get_performance_tier(response_time);
        let weight = get_tier_weight(tier);
        
        total_weight += weight;
        weighted_providers.push((provider, weight));
    }

    // Weighted random selection from available providers
    let random_weight = rng.gen_range(0..total_weight);
    // ... selection logic
}
```

### 6. `metrics.rs` - Performance Tracking & Analytics

**Responsibilities**:
- Real-time metrics collection for cache, providers, and requests
- Thread-safe counters using atomic operations
- Historical data tracking with configurable limits
- Method-level cache performance analysis

**Metrics Structure**:
```rust
pub struct MetricsCollector {
    // Cache metrics
    cache_hits: AtomicU64,
    cache_misses: AtomicU64,
    
    // Request tracking
    total_requests: AtomicU64,
    request_timestamps: Arc<RwLock<VecDeque<u64>>>,
    
    // Provider usage
    provider_usage: Arc<RwLock<HashMap<String, ProviderUsage>>>,
    
    // Method-specific statistics
    method_stats: Arc<RwLock<HashMap<String, MethodStats>>>,
    
    // Error tracking
    error_counts: Arc<RwLock<HashMap<ErrorType, u64>>>,
}
```

**Thread-Safe Metric Updates**:
```rust
pub fn record_cache_hit(&self, method: &str, response_time_ms: u64) {
    self.cache_hits.fetch_add(1, Ordering::Relaxed);
    
    // Update method-specific stats
    let mut method_stats = self.method_stats.write().await;
    let stats = method_stats.entry(method.to_string()).or_default();
    stats.hits += 1;
    stats.update_avg_response_time(response_time_ms);
}

pub fn record_request(&self, method: &str, provider_url: &str, 
                      response_time_ms: u64, success: bool) {
    // All forwarded requests count as cache misses
    self.cache_misses.fetch_add(1, Ordering::Relaxed);
    
    // Track provider usage
    let mut provider_usage = self.provider_usage.write().await;
    let usage = provider_usage.entry(provider_url.to_string()).or_default();
    usage.record_request(response_time_ms, success);
    
    // Update method stats
    let mut method_stats = self.method_stats.write().await;
    let stats = method_stats.entry(method.to_string()).or_default();
    stats.misses += 1;
    stats.update_avg_response_time(response_time_ms);
}
```

### 7. `tui/` - Real-Time Monitoring Interface

**Responsibilities**:
- Real-time dashboard for proxy monitoring
- Interactive navigation between different metric views
- Live charts and provider health visualization
- Remote proxy client for fetching metrics

**Architecture**:
```rust
// tui/app.rs - Application state management
pub struct App {
    pub current_tab: Tab,
    pub proxy_client: ProxyClient,
    pub cache_stats: Option<CacheStats>,
    pub provider_info: Vec<ProviderInfo>,
    pub active_instances: Vec<EDBInstance>,
    pub cache_metrics: Option<CacheMetrics>,
    pub should_refresh: bool,
}

// tui/remote.rs - Remote proxy communication
pub struct ProxyClient {
    base_url: String,
    client: reqwest::Client,
}

impl ProxyClient {
    pub async fn get_cache_stats(&self) -> Result<CacheStats> {
        let response = self.client.post(&format!("{}/", self.base_url))
            .json(&json!({
                "jsonrpc": "2.0",
                "method": "edb_cache_stats",
                "id": 1
            }))
            .send().await?;
        // ... parse response
    }
}
```

## 🎨 Design Patterns

### 1. Builder Pattern (Configuration)

**Problem**: `ProxyServer::new()` had 8+ parameters, making it unwieldy.

**Solution**: Fluent builder with sensible defaults:
```rust
pub struct ProxyServerBuilder {
    rpc_urls: Option<Vec<String>>,
    max_cache_items: u32,        // Default: 1024000
    cache_dir: Option<PathBuf>,  // Default: ~/.edb/cache/rpc/<chain_id>
    grace_period: u64,           // Default: 0 (no auto-shutdown)
    // ... other fields with defaults
}

impl ProxyServerBuilder {
    pub fn new() -> Self { Self::default() }
    
    pub fn max_cache_items(mut self, max_items: u32) -> Self {
        self.max_cache_items = max_items;
        self
    }
    
    pub async fn build(self) -> Result<ProxyServer> {
        // Resolve defaults and build
    }
}
```

### 2. Shared Ownership (Arc Pattern)

**Problem**: Multiple background tasks need access to the same components.

**Solution**: Reference-counted shared ownership:
```rust
let cache_manager = Arc::new(CacheManager::new(/* ... */));
let provider_manager = Arc::new(ProviderManager::new(/* ... */));

// Each task gets its own Arc clone
let cache_clone = Arc::clone(&cache_manager);
tokio::spawn(async move {
    // Task owns cache_clone, original still owned by main
});
```

### 3. Error Type Classification

**Problem**: Different error types require different handling strategies.

**Solution**: Pattern matching on error characteristics:
```rust
// Rate limit detection
if self.is_rate_limit_response(status, &response_text, json_response.as_ref()) {
    self.provider_manager.mark_provider_failed(&provider_url).await;
    continue; // Try next provider
}

// User error detection  
if self.is_user_error(&response_json) {
    self.provider_manager.mark_provider_success(&provider_url, response_time).await;
    return Ok(response_json); // Return immediately, don't retry
}
```

## 🧠 Intelligent Caching System

### Cache Key Generation

**Requirements**:
- Deterministic (same request = same key)
- Collision-resistant
- Fast to compute

**Implementation**:
```rust
fn generate_cache_key(&self, request: &Value) -> String {
    let method = request["method"].as_str().unwrap_or("");
    let params = &request["params"];
    
    let mut hasher = DefaultHasher::new();
    method.hash(&mut hasher);
    self.hash_json_value(&mut hasher, params);
    
    format!("{}:{:x}", method, hasher.finish())
}

// Consistent JSON hashing (order-independent for objects)
fn hash_json_value(&self, hasher: &mut DefaultHasher, value: &Value) {
    match value {
        Value::Object(obj) => {
            "object".hash(hasher);
            let sorted: BTreeMap<_, _> = obj.iter().collect(); // Sort keys
            for (key, val) in sorted {
                key.hash(hasher);
                self.hash_json_value(hasher, val);
            }
        }
        // ... handle other JSON types
    }
}
```

### Cacheability Rules

**Always Cacheable**:
- Chain constants (`eth_chainId`, `net_version`)
- Historical transaction data (`eth_getTransactionByHash`)
- Historical block data (with specific block numbers)

**Never Cacheable**:
- Dynamic queries (`eth_blockNumber`, `eth_gasPrice`)
- Latest state queries (`eth_call` with "latest" block)
- Account-specific current state

**Conditionally Cacheable**:
- State queries with specific block numbers
- Debug traces (after validation)
- Log queries with specific block ranges

### Cache Persistence Strategy

**Design Goals**:
- Atomic writes to prevent corruption
- Merge logic for multiple proxy instances
- Size management based on memory vs disk

**Implementation**:
```rust
async fn apply_size_management(
    &self,
    mut merged_cache: HashMap<String, CacheEntry>,
    original_disk_size: usize,
    current_memory_size: usize,
) -> HashMap<String, CacheEntry> {
    // Determine target size based on policy
    let target_size = if original_disk_size >= current_memory_size {
        // Case 1: Disk cache was larger - respect disk size, no growth
        original_disk_size
    } else {
        // Case 2: Memory cache is larger - allow growth up to max_items
        std::cmp::min(self.max_items as usize, merged_cache.len())
    };

    // Apply LRU eviction to fit target size if needed
    if merged_cache.len() > target_size {
        self.evict_to_size(&mut merged_cache, target_size);
    }
    
    merged_cache
}
```

## 🔄 Weighted Provider Management

### Performance-Based Selection

The provider selection algorithm prioritizes faster providers while ensuring diversity:

```rust
pub async fn get_weighted_provider_excluding(&self, tried_providers: &HashSet<String>) -> Option<String> {
    // 1. Filter to healthy, untried providers
    let available_providers: Vec<_> = providers.iter()
        .filter(|p| p.is_healthy && !tried_providers.contains(&p.url))
        .collect();
    
    // 2. Calculate weights based on performance tiers
    let mut weighted_providers = Vec::new();
    let mut total_weight = 0u32;

    for provider in &available_providers {
        let response_time = provider.response_time_ms.unwrap_or(300);
        let tier = get_performance_tier(response_time);
        let weight = get_tier_weight(tier);
        
        total_weight += weight;
        weighted_providers.push((provider, weight));
    }

    // 3. Weighted random selection
    let random_weight = rng.gen_range(0..total_weight);
    let mut current_weight = 0u32;
    
    for (provider, weight) in weighted_providers {
        current_weight += weight;
        if random_weight < current_weight {
            return Some(provider.url.clone());
        }
    }
    
    None
}
```

### Health Monitoring

**Provider Health State Machine**:
```
Healthy ──failure──> Consecutive Failures (1)
   ▲                           │
   │                          failure
   │                           ▼
   │                  Consecutive Failures (2)
   │                           │
   │                          failure
   │                           ▼
   └──success─────── Unhealthy (max_failures reached)
```

**Health Check Implementation**:
```rust
pub async fn health_check_all(&self) {
    let providers_snapshot = {
        let providers = self.providers.read().await;
        providers.clone()
    };

    for provider in providers_snapshot {
        // Check if provider needs health check (unhealthy or stale)
        let needs_check = !provider.is_healthy
            || provider.last_health_check
                .map_or(true, |t| t.elapsed() > Duration::from_secs(60));

        if needs_check {
            match Self::check_provider_health(&self.client, &provider.url).await {
                Ok(response_time) => {
                    self.mark_provider_success(&provider.url, response_time).await;
                }
                Err(e) => {
                    debug!("Health check failed for {}: {}", provider.url, e);
                    self.mark_provider_failed(&provider.url).await;
                }
            }
        }
    }
}
```

## 🚨 Error Handling Strategy

### Error Categorization

1. **Rate Limit Errors**: Retry with different provider
2. **User Errors**: Return immediately without retry (4xx codes)
3. **Provider Errors**: Mark provider as failed, try alternative
4. **Network Errors**: Retry with exponential backoff

### Error Deduplication with Provider Tracking

**Problem**: With weighted selection, the same provider might be selected multiple times, making error consensus unreliable.

**Solution**: Track unique providers per request to ensure genuine error consensus:

```rust
async fn forward_request(&self, request: &Value) -> Result<Value> {
    let mut tried_providers = HashSet::new();
    let mut error_responses: HashMap<u64, (Value, usize)> = HashMap::new();
    
    for retry in 0..MAX_RETRIES {
        // Ensure each provider is tried only once per request
        let provider_url = match self.provider_manager
            .get_weighted_provider_excluding(&tried_providers).await 
        {
            Some(url) => url,
            None => break, // All unique providers exhausted
        };
        
        tried_providers.insert(provider_url.clone());
        
        match self.try_provider(&provider_url, request).await {
            Ok(response) => return Ok(response),
            Err(error) => {
                // Track error for deduplication
                let error_hash = self.create_error_signature(&error);
                error_responses.entry(error_hash)
                    .and_modify(|(_, count)| *count += 1)
                    .or_insert((error.clone(), 1));
                
                // If multiple unique providers return same error, it's likely legitimate
                if let Some((_, count)) = error_responses.get(&error_hash) {
                    if *count >= MAX_MULTIPLE_SAME_ERROR {
                        return Ok(error);
                    }
                }
            }
        }
    }
    
    // Return most common error if all providers failed
    return_most_common_error(error_responses)
}
```

### Silent Failure Philosophy

**Core Principle**: The proxy should never crash due to secondary failures.

```rust
// Cache save failures are logged but don't propagate
pub async fn save_to_disk(&self) -> Result<()> {
    match self.save_to_disk_impl().await {
        Ok(()) => Ok(()),
        Err(e) => {
            warn!("Failed to save cache: {}. In-memory cache remains available.", e);
            Ok(()) // Silent failure - don't crash the service
        }
    }
}
```

## 📊 Metrics & Observability

### Comprehensive Metrics Collection

**Performance Metrics**:
- Cache hit/miss rates by method
- Provider response times and success rates
- Request rates (requests per minute)
- Error classification and rates

**Implementation**:
```rust
pub struct MetricsCollector {
    // Atomic counters for lock-free updates
    cache_hits: AtomicU64,
    cache_misses: AtomicU64,
    total_requests: AtomicU64,
    
    // Thread-safe collections for detailed metrics
    method_stats: Arc<RwLock<HashMap<String, MethodStats>>>,
    provider_usage: Arc<RwLock<HashMap<String, ProviderUsage>>>,
    error_counts: Arc<RwLock<HashMap<ErrorType, u64>>>,
    
    // Historical data with bounded collections
    request_timestamps: Arc<RwLock<VecDeque<u64>>>,
}

// Method-specific cache performance
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MethodStats {
    pub hits: u64,
    pub misses: u64,
    pub total_response_time_ms: u64,
    pub request_count: u64,
}

impl MethodStats {
    pub fn hit_rate(&self) -> f64 {
        if self.hits + self.misses == 0 {
            0.0
        } else {
            self.hits as f64 / (self.hits + self.misses) as f64
        }
    }
    
    pub fn avg_response_time(&self) -> f64 {
        if self.request_count == 0 {
            0.0
        } else {
            self.total_response_time_ms as f64 / self.request_count as f64
        }
    }
}
```

### Real-Time TUI Monitoring

**Features**:
- Live provider health dashboard
- Cache performance charts
- EDB instance registry
- Interactive navigation

**Implementation**:
```rust
// tui/widgets.rs - Custom UI components
pub fn render_provider_health(providers: &[ProviderInfo]) -> Table {
    let headers = Row::new(vec!["Provider", "Status", "Response Time", "Success Rate"]);
    let rows: Vec<Row> = providers.iter().map(|provider| {
        let status = if provider.is_healthy { "✓ Healthy" } else { "✗ Unhealthy" };
        let response_time = provider.response_time_ms
            .map(|rt| format!("{}ms", rt))
            .unwrap_or_else(|| "N/A".to_string());
        
        Row::new(vec![
            Cell::from(provider.url.clone()),
            Cell::from(status),
            Cell::from(response_time),
            Cell::from(format!("{:.1}%", provider.success_rate * 100.0)),
        ])
    }).collect();
    
    Table::new(rows).header(headers).widths(&[
        Constraint::Percentage(50),
        Constraint::Percentage(15),
        Constraint::Percentage(15),
        Constraint::Percentage(20),
    ])
}
```

## 🧪 Testing Strategy

### Test Categories

1. **Unit Tests**: Individual component testing with mocking
2. **Integration Tests**: Full HTTP server testing with wiremock
3. **Concurrency Tests**: Multi-threaded behavior verification
4. **Property Tests**: Cache invariants and LRU behavior

### Mock Server Testing

**Pattern**: Use `wiremock` for realistic HTTP simulation:
```rust
async fn create_test_rpc_handler() -> (RpcHandler, MockServer, TempDir) {
    let mock_server = MockServer::start().await;
    
    // Set up expected responses
    Mock::given(method("POST"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(200)
            .set_body_json(test_response))
        .expect(1) // Verify call count
        .mount(&mock_server)
        .await;
    
    // Create handler with mock URL
    let handler = RpcHandler::new(
        mock_server.uri(),
        cache_manager,
        metrics_collector
    ).unwrap();
    
    (handler, mock_server, temp_dir)
}
```

### Concurrency Testing

**Approach**: 
1. Test components in isolation where possible
2. Use deterministic test scenarios
3. Verify invariants rather than exact execution order

```rust
#[tokio::test]
async fn test_concurrent_cache_access() {
    let cache = Arc::new(CacheManager::new(100, temp_path));
    
    // Spawn multiple tasks that access cache concurrently
    let handles: Vec<_> = (0..10).map(|i| {
        let cache_clone = Arc::clone(&cache);
        tokio::spawn(async move {
            cache_clone.set(format!("key_{}", i), json!({"value": i})).await;
            cache_clone.get(&format!("key_{}", i)).await
        })
    }).collect();
    
    // Verify all tasks completed successfully
    for handle in handles {
        assert!(handle.await.unwrap().is_some());
    }
}
```

### Provider Selection Testing

**Key Test**: Verify each provider tried only once per request:
```rust
#[tokio::test]
async fn test_provider_tried_once_per_request() {
    // Create 3 mock servers that all return errors
    let mock1 = MockServer::start().await;
    let mock2 = MockServer::start().await; 
    let mock3 = MockServer::start().await;
    
    // Each provider should be called exactly once per request
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&error_response))
        .expect(1) // Should only be called once
        .mount(&mock1).await;
    // ... similar for mock2, mock3
    
    let result = handler.handle_request(request).await.unwrap();
    
    // Mock expectations will verify each provider called exactly once
}
```

## ⚡ Performance Considerations

### Hot Path Optimization

**Critical Paths** (optimized for speed):
1. Cache lookup and key generation
2. Provider selection for cache misses
3. JSON serialization/deserialization

**Optimizations Applied**:
```rust
// Use fast hash function for cache keys
use std::collections::hash_map::DefaultHasher; // Fast but not cryptographic

// Minimize allocations in cache lookup
pub async fn get(&self, key: &str) -> Option<Value> {
    let mut cache = self.cache.write().await;
    if let Some(entry) = cache.get_mut(key) {
        entry.update_access_time(); // Update LRU tracking
        Some(entry.data.clone()) // Only clone if found
    } else {
        None
    }
}

// Lock-free atomic for metrics
let cache_hits = self.cache_hits.fetch_add(1, Ordering::Relaxed);
```

### Memory Management

**Strategy**: Minimize allocations and copies in request handling:
```rust
// Good: Pass references where possible
async fn handle_request(&self, request: &Value) -> Result<Value>

// Good: Clone only when necessary for async boundaries
let provider_url = self.provider_manager.get_weighted_provider_excluding(&tried).await?;

// Good: Use Arc for shared ownership instead of cloning large data
let cache_manager = Arc::new(CacheManager::new(/* ... */));
```

### Network Optimization

**Connection Reuse**: Single HTTP client instance:
```rust
// Shared HTTP client with connection pooling
let upstream_client = reqwest::Client::builder()
    .timeout(Duration::from_secs(10))
    .build()?;
```

**Parallel Health Checks**: Don't block on slow providers:
```rust
// Health check all providers concurrently
let health_futures: Vec<_> = providers.iter()
    .map(|provider| self.check_provider_health(&provider.url))
    .collect();

let results = futures::future::join_all(health_futures).await;
```

## 🔧 Extension Points

### Plugin Architecture

**Extensibility Points**:
1. Custom cache backends
2. Provider discovery mechanisms  
3. Request/response middleware
4. Custom health check strategies

**Example Plugin Interface**:
```rust
#[async_trait]
pub trait RequestMiddleware {
    async fn process_request(&self, request: &mut Value) -> Result<()>;
    async fn process_response(&self, response: &mut Value) -> Result<()>;
}

pub struct ProxyServerBuilder {
    // ...
    middleware: Vec<Box<dyn RequestMiddleware>>,
}
```

### Custom Cache Backends

**Trait Definition**:
```rust
#[async_trait]
pub trait CacheBackend {
    async fn get(&self, key: &str) -> Option<Value>;
    async fn set(&self, key: String, value: Value) -> Result<()>;
    async fn remove(&self, key: &str) -> Result<()>;
    async fn clear(&self) -> Result<()>;
    async fn size(&self) -> usize;
}

// Implementations
pub struct MemoryCacheBackend { /* current implementation */ }
pub struct RedisCacheBackend { /* distributed caching */ }
pub struct SqliteCacheBackend { /* persistent local cache */ }
```

### Provider Discovery

**Service Discovery Interface**:
```rust
#[async_trait]
pub trait ProviderDiscovery {
    async fn discover_providers(&self) -> Vec<String>;
    async fn health_check_provider(&self, url: &str) -> bool;
}

pub struct StaticProviderDiscovery { /* current implementation */ }
pub struct ConsulProviderDiscovery { /* dynamic discovery */ }
pub struct KubernetesProviderDiscovery { /* k8s service discovery */ }
```

### Metrics Integration

**OpenTelemetry Integration**:
```rust
use opentelemetry::metrics::{Counter, Histogram};

struct ProxyMetrics {
    cache_hits: Counter<u64>,
    cache_misses: Counter<u64>,
    request_duration: Histogram<f64>,
    provider_errors: Counter<u64>,
}

impl ProxyMetrics {
    pub fn record_cache_hit(&self, method: &str) {
        self.cache_hits.add(1, &[KeyValue::new("method", method)]);
    }
}
```

---

This developer guide provides the technical foundation needed to understand, maintain, and extend the EDB RPC Proxy. The architecture prioritizes reliability, performance, and maintainability while remaining flexible for future enhancements.