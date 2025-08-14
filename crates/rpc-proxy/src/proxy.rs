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
use std::{net::SocketAddr, sync::Arc};
use tokio::{net::TcpListener, sync::broadcast};
use tower_http::cors::{Any, CorsLayer};
use tracing::{debug, info, warn};

/// Main proxy server that combines RPC handling, registry, and health services
///
/// The ProxyServer coordinates between:
/// - RPC request handling and caching
/// - EDB instance registry and lifecycle management
/// - Health check and monitoring endpoints
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
    /// Creates a new proxy server with multiple RPC providers
    ///
    /// # Arguments
    /// * `rpc_urls` - List of upstream RPC endpoint URLs
    /// * `max_cache_items` - Maximum number of items to cache
    /// * `cache_dir` - Optional directory for cache persistence
    /// * `grace_period` - Seconds to wait before shutdown when no EDB instances
    /// * `heartbeat_interval` - Seconds between heartbeat checks
    /// * `max_failures` - Maximum consecutive failures before marking provider unhealthy
    ///
    /// # Returns
    /// A new ProxyServer instance with background tasks started
    pub async fn new(
        rpc_urls: Option<Vec<String>>,
        max_cache_items: u32,
        cache_dir: Option<std::path::PathBuf>,
        grace_period: u64,
        heartbeat_interval: u64,
        max_failures: u32,
    ) -> Result<Self> {
        // Determine which RPC URLs to use
        let rpc_urls =
            rpc_urls.unwrap_or(DEFAULT_MAINNET_RPCS.iter().map(|s| s.to_string()).collect());
        info!("Starting EDB RPC Proxy with {} providers", rpc_urls.len());
        for url in &rpc_urls {
            info!("  - {}", url);
        }

        // Use first URL for cache path (they all cache the same chain data)
        let cache_path = CacheManager::get_cache_path(&rpc_urls, cache_dir).await?;
        let cache_manager = Arc::new(CacheManager::new(max_cache_items, cache_path)?);

        // Create provider manager with all URLs
        let provider_manager = Arc::new(ProviderManager::new(rpc_urls, max_failures).await?);

        // Create RPC handler with provider manager
        let rpc_handler = Arc::new(RpcHandler::new(provider_manager.clone(), cache_manager)?);
        let registry = Arc::new(EDBRegistry::new(grace_period));
        let health_service = Arc::new(HealthService::new());
        let (shutdown_tx, _) = broadcast::channel(1);

        // Start background tasks
        let registry_clone = Arc::clone(&registry);
        tokio::spawn(async move {
            registry_clone.start_heartbeat_monitor(heartbeat_interval).await;
        });

        // Start periodic health checks for providers
        let provider_manager_clone = provider_manager.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
            loop {
                interval.tick().await;
                provider_manager_clone.health_check_all().await;
            }
        });

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
            info!("Shutdown signal received, stopping server gracefully");
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
