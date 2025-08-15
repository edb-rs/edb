//! Core proxy server implementation

use crate::{
    cache::CacheManager,
    health::HealthService,
    providers::{ProviderManager, DEFAULT_MAINNET_RPCS},
    registry::EDBRegistry,
    rpc::RpcHandler,
};
use axum::{
    extract::State,
    http::{Method, StatusCode},
    response::Json,
    routing::post,
    Router,
};
use eyre::Result;
use serde_json::Value;
use std::{net::SocketAddr, path::PathBuf, sync::Arc};
use tokio::{net::TcpListener, sync::broadcast};
use tower_http::cors::{Any, CorsLayer};
use tracing::{debug, info, warn};

/// Builder for configuring ProxyServer with fluent API and sensible defaults
#[derive(Debug, Clone)]
pub struct ProxyServerBuilder {
    rpc_urls: Option<Vec<String>>,
    max_cache_items: u32,
    cache_dir: Option<PathBuf>,
    grace_period: u64,
    heartbeat_interval: u64,
    max_failures: u32,
    health_check_interval: u64,
    cache_save_interval: u64,
}

impl Default for ProxyServerBuilder {
    fn default() -> Self {
        Self {
            // General Configuration
            rpc_urls: None, // Will use DEFAULT_MAINNET_RPCS

            // Cache Configuration
            max_cache_items: 102400,
            cache_dir: None,        // Will use ~/.edb/cache/rpc/<chain_id>
            cache_save_interval: 5, // 5 minutes

            // Provider Health Check Configuration
            max_failures: 3,
            health_check_interval: 60,

            // EDB Register Configuration
            grace_period: 0, // No auto-shutdown by default
            heartbeat_interval: 10,
        }
    }
}

impl ProxyServerBuilder {
    /// Create a new builder with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Set custom RPC URLs (comma-separated string or Vec)
    #[allow(dead_code)]
    pub fn rpc_urls<T: Into<Vec<String>>>(mut self, urls: T) -> Self {
        self.rpc_urls = Some(urls.into());
        self
    }

    /// Set custom RPC URLs from comma-separated string
    pub fn rpc_urls_str(mut self, urls: &str) -> Self {
        self.rpc_urls =
            Some(urls.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect());
        self
    }

    /// Set maximum number of cached items
    pub fn max_cache_items(mut self, max_items: u32) -> Self {
        self.max_cache_items = max_items;
        self
    }

    /// Set cache directory path
    pub fn cache_dir<P: Into<PathBuf>>(mut self, dir: P) -> Self {
        self.cache_dir = Some(dir.into());
        self
    }

    /// Set grace period in seconds before shutdown when no EDB instances (0 = no auto-shutdown)
    pub fn grace_period(mut self, seconds: u64) -> Self {
        self.grace_period = seconds;
        self
    }

    /// Set heartbeat check interval in seconds
    pub fn heartbeat_interval(mut self, seconds: u64) -> Self {
        self.heartbeat_interval = seconds;
        self
    }

    /// Set maximum consecutive failures before marking provider unhealthy
    pub fn max_failures(mut self, failures: u32) -> Self {
        self.max_failures = failures;
        self
    }

    /// Set provider health check interval in seconds
    pub fn health_check_interval(mut self, seconds: u64) -> Self {
        self.health_check_interval = seconds;
        self
    }

    /// Set cache save interval in minutes (0 = save only on shutdown)
    pub fn cache_save_interval(mut self, minutes: u64) -> Self {
        self.cache_save_interval = minutes;
        self
    }

    /// Build the ProxyServer with the configured settings
    pub async fn build(self) -> Result<ProxyServer> {
        // Resolve RPC URLs
        let rpc_urls = self
            .rpc_urls
            .unwrap_or_else(|| DEFAULT_MAINNET_RPCS.iter().map(|s| s.to_string()).collect());

        // Resolve cache path
        let cache_path = CacheManager::get_cache_path(&rpc_urls, self.cache_dir).await?;

        // Now call the simplified ProxyServer::new with deterministic values
        ProxyServer::new(
            rpc_urls,
            self.max_cache_items,
            cache_path,
            self.grace_period,
            self.heartbeat_interval,
            self.max_failures,
            self.health_check_interval,
            self.cache_save_interval,
        )
        .await
    }
}

/// Main proxy server that combines RPC handling, registry, and health services
///
/// The ProxyServer coordinates between:
/// - RPC request handling and caching
/// - EDB instance registry and lifecycle management
/// - Health check and monitoring endpoints
///
/// Use ProxyServerBuilder for easy configuration:
/// ```no_run
/// # use edb_rpc_proxy::proxy::ProxyServerBuilder;
/// # async fn example() -> eyre::Result<()> {
/// let proxy = ProxyServerBuilder::new()
///     .max_cache_items(50000)
///     .grace_period(300)
///     .build()
///     .await?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct ProxyServer {
    /// RPC request handler with caching capabilities
    pub rpc_handler: Arc<RpcHandler>,
    /// Registry for tracking connected EDB instances
    pub registry: Arc<EDBRegistry>,
    /// Health check service for monitoring
    pub health_service: Arc<HealthService>,
    /// Shutdown signal sender
    shutdown_tx: broadcast::Sender<()>,
}

#[derive(Clone)]
struct AppState {
    proxy: ProxyServer,
}

impl ProxyServer {
    /// Creates a new proxy server with deterministic configuration
    ///
    /// This method is now simplified to take concrete values rather than Options.
    /// Use ProxyServerBuilder for a more convenient fluent API.
    ///
    /// # Arguments
    /// * `rpc_urls` - List of upstream RPC endpoint URLs
    /// * `max_cache_items` - Maximum number of items to cache
    /// * `cache_path` - Resolved path for cache persistence
    /// * `grace_period` - Seconds to wait before shutdown when no EDB instances
    /// * `heartbeat_interval` - Seconds between heartbeat checks
    /// * `max_failures` - Maximum consecutive failures before marking provider unhealthy
    /// * `health_check_interval` - Seconds between provider health checks
    /// * `cache_save_interval` - Minutes between periodic cache saves
    ///
    /// # Returns
    /// A new ProxyServer instance with background tasks started
    async fn new(
        rpc_urls: Vec<String>,
        max_cache_items: u32,
        cache_path: PathBuf,
        grace_period: u64,
        heartbeat_interval: u64,
        max_failures: u32,
        health_check_interval: u64,
        cache_save_interval: u64,
    ) -> Result<Self> {
        info!("Starting EDB RPC Proxy with {} providers", rpc_urls.len());
        for url in &rpc_urls {
            info!("  - {}", url);
        }

        let cache_manager = Arc::new(CacheManager::new(max_cache_items, cache_path)?);

        // Create provider manager with all URLs
        let provider_manager = Arc::new(ProviderManager::new(rpc_urls, max_failures).await?);

        // Create RPC handler with provider manager
        let rpc_handler =
            Arc::new(RpcHandler::new(provider_manager.clone(), cache_manager.clone())?);
        let health_service = Arc::new(HealthService::new());
        let (shutdown_tx, _) = broadcast::channel(1);

        // Create registry with shutdown channel
        let registry = Arc::new(EDBRegistry::new(grace_period, shutdown_tx.clone()));

        // Start background tasks (if grace period is active)
        if grace_period > 0 {
            let registry_clone = Arc::clone(&registry);
            tokio::spawn(async move {
                registry_clone.start_heartbeat_monitor(heartbeat_interval).await;
            });
        }

        // Start periodic health checks for providers
        let provider_manager_clone = provider_manager.clone();
        tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(std::time::Duration::from_secs(health_check_interval));
            loop {
                interval.tick().await;
                provider_manager_clone.health_check_all().await;
            }
        });

        // Start periodic cache saving (if enabled)
        if cache_save_interval > 0 {
            let cache_manager_clone = cache_manager.clone();
            tokio::spawn(async move {
                let mut interval =
                    tokio::time::interval(std::time::Duration::from_secs(cache_save_interval * 60));
                loop {
                    interval.tick().await;
                    if let Err(e) = cache_manager_clone.save_to_disk().await {
                        warn!("Failed to save cache periodically: {}", e);
                    } else {
                        debug!("Cache saved to disk (periodic save)");
                    }
                }
            });
        }

        Ok(Self { rpc_handler, registry, health_service, shutdown_tx })
    }

    /// Returns a reference to the cache manager
    ///
    /// # Returns
    /// Reference to the underlying cache manager
    pub fn cache_manager(&self) -> &Arc<CacheManager> {
        self.rpc_handler.cache_manager()
    }

    /// Starts the proxy server listening on the specified address
    ///
    /// Creates an Axum web server with routes for:
    /// - Standard JSON-RPC requests (POST /)
    /// - EDB-specific management endpoints (edb_ping, edb_register, etc.)
    ///
    /// # Arguments
    /// * `addr` - Socket address to bind to
    ///
    /// # Returns
    /// Result indicating server startup success or failure
    pub async fn serve(self, addr: SocketAddr) -> Result<()> {
        let mut shutdown_rx = self.shutdown_tx.subscribe();
        let cache_manager_for_shutdown = self.cache_manager().clone();

        let app = Router::new()
            .route("/", post(handle_rpc))
            .layer(
                CorsLayer::new()
                    .allow_methods([Method::POST, Method::GET])
                    .allow_headers(Any)
                    .allow_origin(Any),
            )
            .with_state(AppState { proxy: self });

        let listener = TcpListener::bind(addr).await?;
        info!("EDB RPC Proxy listening on {}", addr);

        // Create the server with graceful shutdown
        let server = axum::serve(listener, app).with_graceful_shutdown(async move {
            let _ = shutdown_rx.recv().await;
            info!("Shutdown signal received, saving cache and stopping server gracefully");

            // Save cache before shutdown
            if let Err(e) = cache_manager_for_shutdown.save_to_disk().await {
                warn!("Failed to save cache during shutdown: {}", e);
            } else {
                info!("Cache saved successfully during shutdown");
            }
        });

        server.await?;

        Ok(())
    }
}

async fn handle_rpc(
    State(state): State<AppState>,
    Json(request): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    // Handle special EDB health check methods
    debug!("Received RPC request: {}", request);
    let response = if let Some(method) = request.get("method").and_then(|m| m.as_str()) {
        match method {
            "edb_ping" => {
                let response = state.proxy.health_service.ping().await;
                Ok(Json(response))
            }
            "edb_info" => {
                let response = state.proxy.health_service.info().await;
                Ok(Json(response))
            }
            "edb_register" => {
                if let Some(params) = request.get("params").and_then(|p| p.as_array()) {
                    if let (Some(pid), Some(timestamp)) = (
                        params.get(0).and_then(|v| v.as_u64()),
                        params.get(1).and_then(|v| v.as_u64()),
                    ) {
                        let response =
                            state.proxy.registry.register_edb_instance(pid as u32, timestamp).await;
                        Ok(Json(response))
                    } else {
                        Err(StatusCode::BAD_REQUEST)
                    }
                } else {
                    Err(StatusCode::BAD_REQUEST)
                }
            }
            "edb_heartbeat" => {
                if let Some(params) = request.get("params").and_then(|p| p.as_array()) {
                    if let Some(pid) = params.get(0).and_then(|v| v.as_u64()) {
                        let response = state.proxy.registry.heartbeat(pid as u32).await;
                        Ok(Json(response))
                    } else {
                        Err(StatusCode::BAD_REQUEST)
                    }
                } else {
                    Err(StatusCode::BAD_REQUEST)
                }
            }
            "edb_cache_stats" => {
                let stats = state.proxy.cache_manager().detailed_stats().await;
                let response = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": request.get("id").unwrap_or(&serde_json::Value::from(1)),
                    "result": stats
                });
                Ok(Json(response))
            }
            "edb_active_instances" => {
                let active_pids = state.proxy.registry.get_active_instances().await;
                let response = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": request.get("id").unwrap_or(&serde_json::Value::from(1)),
                    "result": {
                        "active_instances": active_pids,
                        "count": active_pids.len()
                    }
                });
                Ok(Json(response))
            }
            "edb_providers" => {
                let providers_info =
                    state.proxy.rpc_handler.provider_manager().get_providers_info().await;
                let healthy_count =
                    state.proxy.rpc_handler.provider_manager().healthy_provider_count().await;
                let response = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": request.get("id").unwrap_or(&serde_json::Value::from(1)),
                    "result": {
                        "providers": providers_info,
                        "healthy_count": healthy_count,
                        "total_count": providers_info.len()
                    }
                });
                Ok(Json(response))
            }
            "edb_shutdown" => {
                info!("Shutdown request received");
                let response = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": request.get("id").unwrap_or(&serde_json::Value::from(1)),
                    "result": {
                        "status": "shutting_down",
                        "message": "Server shutdown initiated"
                    }
                });

                // Send shutdown signal (non-blocking)
                let _ = state.proxy.shutdown_tx.send(());

                Ok(Json(response))
            }
            _ => {
                // Forward to RPC handler
                match state.proxy.rpc_handler.handle_request(request).await {
                    Ok(response) => Ok(Json(response)),
                    Err(e) => {
                        warn!("RPC request failed: {}", e);
                        Err(StatusCode::INTERNAL_SERVER_ERROR)
                    }
                }
            }
        }
    } else {
        warn!("Invalid RPC request: {}", request);
        Err(StatusCode::BAD_REQUEST)
    };

    debug!("RPC response: {}", &format!("{:?}", response).chars().take(200).collect::<String>());
    response
}
