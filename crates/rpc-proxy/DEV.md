# EDB RPC Proxy - Developer Guide

This document provides deep technical insights into the RPC Proxy architecture, design decisions, and implementation details for developers working on or extending the codebase.

## 📋 Table of Contents

- [Architecture Overview](#architecture-overview)
- [Process Model](#process-model)
- [Concurrency Design](#concurrency-design)
- [Module Breakdown](#module-breakdown)
- [Design Patterns](#design-patterns)
- [Error Handling Strategy](#error-handling-strategy)
- [Caching Algorithm](#caching-algorithm)
- [Provider Management](#provider-management)
- [Testing Strategy](#testing-strategy)
- [Performance Considerations](#performance-considerations)
- [Future Architecture](#future-architecture)

## 🏗️ Architecture Overview

### Core Design Principles

1. **Separation of Concerns**: Each module has a single, well-defined responsibility
2. **Async-First**: Built on tokio for high-concurrency performance
3. **Fault Tolerance**: Designed to degrade gracefully under failures
4. **Zero-Copy Where Possible**: Minimize allocations in hot paths
5. **Configuration-Driven**: Behavior controlled through builder pattern
6. **Testability**: Each component can be tested in isolation

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
│  └─ Background task coordination                                │
├─────────────────────────────────────────────────────────────────┤
│                       Domain Layer                              │
│  ├─ RpcHandler (request routing and caching logic)              │
│  ├─ CacheManager (LRU + persistence)                            │
│  ├─ ProviderManager (health monitoring + load balancing)        │
│  ├─ EDBRegistry (instance lifecycle management)                 │
│  └─ HealthService (service monitoring)                          │
├─────────────────────────────────────────────────────────────────┤
│                    Infrastructure Layer                         │
│  ├─ HTTP Client (reqwest for upstream requests)                 │
│  ├─ File System (atomic writes for cache persistence)           │
│  ├─ Process Management (cross-platform process checking)        │
│  └─ Time/Scheduling (tokio intervals and timers)                │
└─────────────────────────────────────────────────────────────────┘
```

## ⚙️ Process Model

### Main Process Lifecycle

```rust
#[tokio::main]
async fn main() -> Result<()> {
    // 1. Sequential Initialization Phase
    init_logging();
    let args = parse_cli_args();
    let proxy = build_proxy_server(args).await?;
    
    // 2. Concurrent Execution Phase
    tokio::select! {
        // Main HTTP server (always runs)
        result = proxy.serve(addr) => result?,
        
        // Graceful shutdown signal
        _ = tokio::signal::ctrl_c() => {
            info!("Received shutdown signal");
        }
    }
    
    // 3. Cleanup Phase
    cache_manager.save_to_disk().await?;
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

### Task Communication

Tasks communicate through:
1. **Shared State**: `Arc<RwLock<T>>` for thread-safe shared data
2. **Broadcast Channels**: For shutdown coordination
3. **Atomic Counters**: For lock-free round-robin selection
4. **No Direct Task Communication**: Each task is independent

## 🔄 Concurrency Design

### Thread Safety Model

```rust
// Shared read-heavy data structures
Arc<RwLock<HashMap<String, CacheEntry>>>     // Cache entries
Arc<RwLock<Vec<ProviderInfo>>>               // Provider health status
Arc<RwLock<HashMap<u32, EDBInstance>>>       // EDB instance registry

// Lock-free atomic operations
AtomicUsize                                   // Round-robin counter
AtomicBool                                    // Shutdown flags

// Immutable shared data
Arc<CacheManager>                             // Shared ownership
Arc<ProviderManager>                          // Shared ownership
```

### Lock Hierarchy and Deadlock Prevention

**Lock Ordering Rules**:
1. Cache locks acquired before provider locks
2. Short-lived locks only (no I/O while holding locks)
3. Clone data out of locks for expensive operations

**Example Safe Pattern**:
```rust
async fn handle_request(&self, request: Value) -> Result<Value> {
    // 1. Quick cache check (short lock)
    let cache_key = self.generate_cache_key(&request);
    if let Some(cached) = self.cache_manager.get(&cache_key).await {
        return Ok(cached);
    }
    
    // 2. Provider selection (short lock)
    let provider_url = self.provider_manager.get_next_provider().await?;
    
    // 3. Network I/O (no locks held)
    let response = self.forward_to_provider(&provider_url, &request).await?;
    
    // 4. Cache update (short lock)
    if response.get("error").is_none() {
        self.cache_manager.set(cache_key, response.clone()).await;
    }
    
    Ok(response)
}
```

### Reader-Writer Lock Usage

**High-Read Scenarios** (RwLock appropriate):
- Cache lookups (frequent reads, infrequent writes)
- Provider health status (frequent reads, periodic health updates)
- EDB instance registry (frequent reads, occasional registration)

**Write-Heavy Scenarios** (Mutex would be better, but we don't have any):
- All our use cases are read-heavy, justifying RwLock usage

## 📦 Module Breakdown

### 1. `main.rs` - CLI Interface

**Responsibilities**:
- Command-line argument parsing with `clap`
- Logging initialization
- Builder pattern orchestration
- Signal handling and graceful shutdown

**Key Design Decisions**:
- Uses `tokio::select!` for concurrent signal handling
- Explicit cache save on shutdown to prevent data loss
- Comprehensive CLI validation before server start

### 2. `proxy.rs` - Server Orchestration

**Responsibilities**:
- Builder pattern implementation for configuration
- HTTP server setup with Axum
- Background task spawning and coordination
- Graceful shutdown coordination

**Design Patterns**:
```rust
// Builder pattern with fluent API
ProxyServerBuilder::new()
    .max_cache_items(50000)
    .grace_period(300)
    .build().await?

// Service composition
struct ProxyServer {
    rpc_handler: Arc<RpcHandler>,        // Core request handling
    registry: Arc<EDBRegistry>,          // Instance management
    health_service: Arc<HealthService>,  // Health endpoints
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
- Request forwarding with provider failover
- Error detection and classification
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
        return Ok(cached);
    }
    
    // Step 4: Provider forwarding with error handling
    let response = self.forward_request_with_retry(&request).await?;
    
    // Step 5: Response validation and caching
    if self.is_valid_response(&response, &method) {
        self.cache_manager.set(cache_key, response.clone()).await;
    }
    
    Ok(response)
}
```

**Error Classification System**:
```rust
enum ErrorType {
    RateLimit,      // Retry with different provider
    UserError,      // Return immediately, don't retry
    ProviderError,  // Mark provider as failed, retry
    NetworkError,   // Retry with backoff
}
```

### 4. `cache.rs` - Intelligent Caching

**Responsibilities**:
- In-memory LRU cache management
- Atomic disk persistence with merge logic
- Concurrent access coordination
- Cache size management and eviction

**LRU Implementation**:
```rust
// Timestamp-based LRU (simpler than linked list)
struct CacheEntry {
    data: Value,
    created_at: u64,  // Unix timestamp for LRU ordering
}

// Eviction algorithm
async fn evict_oldest(&self, cache: &mut HashMap<String, CacheEntry>) {
    let to_remove = (cache.len() / 10).max(1);  // Remove 10% at a time
    
    // Sort by timestamp, remove oldest entries
    let mut entries: Vec<_> = cache.iter()
        .map(|(key, entry)| (key.clone(), entry.created_at))
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
    
    // 2. Merge with in-memory cache (newest wins)
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

### 5. `providers.rs` - Multi-Provider Management

**Responsibilities**:
- Provider health monitoring with configurable thresholds
- Round-robin load balancing
- Failure detection and automatic recovery
- Response time tracking

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

**Round-Robin Implementation**:
```rust
// Lock-free atomic counter for performance
pub async fn get_next_provider(&self) -> Option<String> {
    let providers = self.providers.read().await;
    let healthy: Vec<_> = providers.iter()
        .filter(|p| p.is_healthy)
        .collect();
    
    if healthy.is_empty() { return None; }
    
    // Atomic increment for thread-safe round-robin
    let index = self.round_robin_counter
        .fetch_add(1, Ordering::Relaxed) % healthy.len();
    
    Some(healthy[index].url.clone())
}
```

### 6. `registry.rs` - Lifecycle Management

**Responsibilities**:
- EDB instance registration and heartbeat tracking
- Process liveness verification (cross-platform)
- Grace period management for auto-shutdown
- Instance cleanup on process termination

**Cross-Platform Process Checking**:
```rust
fn is_process_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        // Use kill -0 (doesn't actually kill, just checks existence)
        Command::new("kill")
            .args(["-0", &pid.to_string()])
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }
    
    #[cfg(windows)]
    {
        // Use tasklist to check process existence
        Command::new("tasklist")
            .args(["/FI", &format!("PID eq {}", pid)])
            .output()
            .map(|output| {
                output.status.success() && 
                String::from_utf8_lossy(&output.stdout)
                    .contains(&pid.to_string())
            })
            .unwrap_or(false)
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
    max_cache_items: u32,        // Default: 102400
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

### 3. Type-Safe State Management

**Problem**: Need to pass multiple components to HTTP handlers.

**Solution**: Wrapped state with Axum's state extraction:
```rust
#[derive(Clone)]
struct AppState {
    proxy: ProxyServer,
}

async fn handle_rpc(
    State(state): State<AppState>,
    Json(request): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    // Type-safe access to all proxy components
    let response = state.proxy.rpc_handler.handle_request(request).await?;
    Ok(Json(response))
}
```

### 4. Error Type Classification

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

## 🚨 Error Handling Strategy

### Error Categorization

1. **Recoverable Errors**: Retry with exponential backoff
2. **Provider Errors**: Mark provider as failed, try alternative
3. **User Errors**: Return immediately without retry
4. **System Errors**: Log and degrade gracefully

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

### Provider Fallback Strategy

```rust
async fn forward_request(&self, request: &Value) -> Result<Value> {
    const MAX_RETRIES: usize = 5;
    let mut error_responses = HashMap::new();
    
    for retry in 0..MAX_RETRIES {
        match self.try_single_provider(request).await {
            Ok(response) => return Ok(response),
            Err(ProviderError::RateLimit) => continue,        // Try next provider
            Err(ProviderError::UserError(resp)) => return Ok(resp), // Return immediately
            Err(ProviderError::Network(e)) => {
                // Track error for potential return
                error_responses.insert(hash_error(&e), e);
                continue;
            }
        }
    }
    
    // Return most common error if all providers failed
    return_most_common_error(error_responses)
}
```

## 🧠 Caching Algorithm

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

### Cache Eviction Strategy

**LRU with Batch Eviction**:
```rust
// Instead of evicting one item at a time, evict 10% when limit reached
// This amortizes the cost of sorting by timestamp
async fn evict_oldest(&self, cache: &mut HashMap<String, CacheEntry>) {
    let to_remove = (cache.len() / 10).max(1);
    
    // Sort entries by creation time (oldest first)
    let mut entries: Vec<(String, u64)> = cache.iter()
        .map(|(key, entry)| (key.clone(), entry.created_at))
        .collect();
    entries.sort_by_key(|(_, created_at)| *created_at);
    
    // Remove oldest entries
    for (key, _) in entries.into_iter().take(to_remove) {
        cache.remove(&key);
    }
}
```

## 🧪 Testing Strategy

### Test Categories

1. **Unit Tests**: Individual component testing
2. **Integration Tests**: Multi-component interaction testing
3. **Mock Testing**: External dependency simulation
4. **Property Tests**: Invariant verification

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
        cache_manager
    ).unwrap();
    
    (handler, mock_server, temp_dir)
}
```

### Concurrency Testing

**Challenge**: Testing multi-threaded behavior is inherently difficult.

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

### Property-Based Testing Ideas

**Cache Invariants**:
- Cache size never exceeds `max_items`
- Cache hits return exact same data as original response
- Eviction maintains LRU ordering

**Provider Management Invariants**:
- At least one provider remains healthy if any are responsive
- Round-robin provides fair distribution over time
- Health recovery works after provider becomes responsive

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
    let cache = self.cache.read().await;
    cache.get(key).map(|entry| entry.data.clone()) // Only clone if found
}

// Lock-free atomic for round-robin
let index = self.counter.fetch_add(1, Ordering::Relaxed) % provider_count;
```

### Memory Management

**Strategy**: Minimize allocations and copies in request handling:
```rust
// Good: Pass references where possible
async fn handle_request(&self, request: &Value) -> Result<Value>

// Good: Clone only when necessary for async boundaries
let provider_url = self.provider_manager.get_next_provider().await?;

// Good: Use Arc for shared ownership instead of cloning large data
let cache_manager = Arc::new(CacheManager::new(/* ... */));
```

### Disk I/O Optimization

**Atomic Writes**: Prevent corruption during concurrent access:
```rust
// Write to temporary file first, then atomic rename
let temp_file = self.cache_file_path.with_extension("tmp");
fs::write(&temp_file, &content)?;
fs::rename(&temp_file, &self.cache_file_path)?; // Atomic on most filesystems
```

**Batch Operations**: Reduce I/O frequency:
```rust
// Save cache periodically rather than on every update
// Configurable interval: --cache-save-interval <minutes>
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

## 🔮 Future Architecture

### Scalability Considerations

**Current Limitations**:
1. Single-node deployment only
2. Cache limited by single machine memory
3. No horizontal scaling capability

**Potential Enhancements**:

**1. Distributed Caching**:
```rust
// Potential Redis integration for shared cache
pub trait CacheBackend {
    async fn get(&self, key: &str) -> Option<Value>;
    async fn set(&self, key: String, value: Value);
}

pub struct RedisCacheBackend { /* ... */ }
pub struct LocalCacheBackend { /* ... */ }
```

**2. Multi-Node Provider Pool**:
```rust
// Service discovery for dynamic provider management
pub trait ProviderDiscovery {
    async fn discover_providers(&self) -> Vec<String>;
    async fn health_check_provider(&self, url: &str) -> bool;
}
```

**3. Metrics and Observability**:
```rust
// OpenTelemetry integration for production monitoring
use opentelemetry::metrics::{Counter, Histogram};

struct ProxyMetrics {
    cache_hits: Counter<u64>,
    cache_misses: Counter<u64>,
    request_duration: Histogram<f64>,
    provider_errors: Counter<u64>,
}
```

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

### Performance Monitoring

**Future Metrics**:
- Cache hit/miss ratios by method
- Provider response times and availability
- Request latency percentiles
- Memory usage and GC pressure
- Error rates by category

**Alerting Integration**:
- Prometheus metrics export
- Health check endpoints for load balancers
- Structured logging for centralized monitoring

---

This developer guide provides the technical foundation needed to understand, maintain, and extend the EDB RPC Proxy. The architecture prioritizes reliability, performance, and maintainability while remaining flexible for future enhancements.