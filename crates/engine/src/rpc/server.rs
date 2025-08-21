//! RPC server implementation with standard multi-threaded pattern
//!
//! This module implements the main JSON-RPC server that handles debugging requests
//! using the standard Axum multi-threaded approach with Send+Sync EngineContext.

use super::methods::MethodHandler;
use super::types::{RpcError, RpcRequest, RpcResponse};
use super::utils::get_default_rpc_port;
use crate::EngineContext;
use axum::{
    extract::{Json as JsonExtract, State},
    response::Json as JsonResponse,
    routing::{get, post},
    Router,
};
use eyre::Result;
use revm::database::CacheDB;
use revm::{Database, DatabaseCommit, DatabaseRef};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::oneshot;
use tracing::{info, warn};

/// Handle to the running RPC server
#[derive(Debug)]
pub struct RpcServerHandle {
    /// Address the server is listening on
    pub addr: SocketAddr,
    /// Shutdown signal
    shutdown_tx: oneshot::Sender<()>,
}

impl RpcServerHandle {
    /// Get the server address
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Get the port number
    pub fn port(&self) -> u16 {
        self.addr.port()
    }

    /// Gracefully shutdown the RPC server
    pub fn shutdown(self) -> Result<()> {
        if self.shutdown_tx.send(()).is_err() {
            warn!("RPC server already shut down");
        }
        Ok(())
    }
}

/// Thread-safe RPC state for Axum
#[derive(Clone)]
struct RpcState<DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone + Send + Sync + 'static,
    <CacheDB<DB> as Database>::Error: Clone + Send + Sync,
    <DB as Database>::Error: Clone + Send + Sync,
{
    /// The debug RPC server instance
    server: Arc<DebugRpcServer<DB>>,
}

/// Debug RPC server that provides read-only access to EngineContext
pub struct DebugRpcServer<DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone + Send + Sync + 'static,
    <CacheDB<DB> as Database>::Error: Clone + Send + Sync,
    <DB as Database>::Error: Clone + Send + Sync,
{
    /// Immutable debugging context (read-only access)
    context: Arc<EngineContext<DB>>,
    /// Method handler for RPC dispatch
    method_handler: Arc<MethodHandler<DB>>,
}

impl<DB> DebugRpcServer<DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone + Send + Sync + 'static,
    <CacheDB<DB> as Database>::Error: Clone + Send + Sync,
    <DB as Database>::Error: Clone + Send + Sync,
{
    /// Create a new debug RPC server
    pub fn new(context: EngineContext<DB>) -> Self {
        let context = Arc::new(context);
        let method_handler = Arc::new(MethodHandler::new(context.clone()));

        Self { context, method_handler }
    }

    /// Start the RPC server
    pub async fn start(self) -> Result<RpcServerHandle> {
        let port = get_default_rpc_port()?;
        self.start_on_port(port).await
    }

    /// Start the RPC server on a specific port using standard multi-threaded pattern
    ///
    /// This method creates the Axum server with Send+Sync state, leveraging
    /// the now thread-safe EngineContext.
    pub async fn start_on_port(self, port: u16) -> Result<RpcServerHandle> {
        // Create the Axum app with the server as state
        let app = Router::new()
            .route("/", post(handle_rpc_request))
            .route("/health", get(health_check))
            .with_state(RpcState { server: Arc::new(self) });

        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        let listener = tokio::net::TcpListener::bind(addr).await?;
        let actual_addr = listener.local_addr()?;

        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        // Spawn the Axum server
        tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    shutdown_rx.await.ok();
                })
                .await
                .expect("RPC server failed");
        });

        info!("Debug RPC server started on {}", actual_addr);

        Ok(RpcServerHandle { addr: actual_addr, shutdown_tx })
    }

    /// Handle an RPC request (called from worker thread)
    async fn handle_request(&self, request: RpcRequest) -> RpcResponse {
        let id = request.id.clone();

        // Dispatch to method handler
        match self.method_handler.handle_method(&request.method, request.params).await {
            Ok(result) => {
                RpcResponse { jsonrpc: "2.0".to_string(), result: Some(result), error: None, id }
            }
            Err(error) => {
                RpcResponse { jsonrpc: "2.0".to_string(), result: None, error: Some(error), id }
            }
        }
    }

    /// Get total snapshot count (stateless)
    pub fn snapshot_count(&self) -> usize {
        self.context.snapshots.len()
    }

    /// Validate snapshot index (stateless helper)
    pub fn validate_snapshot_index(&self, index: usize) -> Result<()> {
        if index >= self.snapshot_count() {
            return Err(eyre::eyre!(
                "Snapshot index {} out of bounds (max: {})",
                index,
                self.snapshot_count() - 1
            ));
        }
        Ok(())
    }

    /// Get read-only access to the engine context
    pub fn context(&self) -> &Arc<EngineContext<DB>> {
        &self.context
    }
}

/// Handle RPC requests directly with the thread-safe server
async fn handle_rpc_request<DB>(
    State(state): State<RpcState<DB>>,
    JsonExtract(request): JsonExtract<RpcRequest>,
) -> JsonResponse<RpcResponse>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone + Send + Sync + 'static,
    <CacheDB<DB> as Database>::Error: Clone + Send + Sync,
    <DB as Database>::Error: Clone + Send + Sync,
{
    // Validate JSON-RPC version
    if request.jsonrpc != "2.0" {
        return JsonResponse(RpcResponse {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(RpcError {
                code: -32600,
                message: "Invalid Request - JSON-RPC version must be 2.0".to_string(),
                data: None,
            }),
            id: request.id.clone(),
        });
    }

    // Handle request directly (no channel proxy needed)
    let response = state.server.handle_request(request).await;
    JsonResponse(response)
}

/// Health check endpoint
async fn health_check() -> JsonResponse<serde_json::Value> {
    JsonResponse(serde_json::json!({
        "status": "healthy",
        "service": "edb-debug-rpc-server",
        "version": env!("CARGO_PKG_VERSION"),
        "architecture": "multi-threaded"
    }))
}

/// Create and start a debug RPC server with auto-port detection
pub async fn start_debug_server<DB>(context: EngineContext<DB>) -> Result<RpcServerHandle>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone + Send + Sync + 'static,
    <CacheDB<DB> as Database>::Error: Clone + Send + Sync,
    <DB as Database>::Error: Clone + Send + Sync,
{
    let server = DebugRpcServer::new(context);
    server.start().await
}

#[cfg(test)]
mod tests {
    use super::*;

    // Tests would need a concrete DB type to compile
    // #[test]
    // fn test_rpc_state_is_send_sync() {
    //     fn assert_send_sync<T: Send + Sync>() {}
    //     // Would need a concrete DB type like:
    //     // assert_send_sync::<RpcState<EmptyDB>>();
    // }
}
