//! RPC server implementation with async channel proxy pattern
//!
//! This module implements the main JSON-RPC server that handles debugging requests
//! using async channels to proxy requests to the non-Send EngineContext data.

use super::methods::MethodHandler;
use super::types::{RpcError, RpcRequest, RpcResponse};
use super::utils::get_default_rpc_port;
use crate::core::EngineContext;
use axum::{
    extract::{Json as JsonExtract, State},
    response::Json as JsonResponse,
    routing::{get, post},
    Router,
};
use eyre::Result;
use revm::database::CacheDB;
use revm::{Database, DatabaseCommit, DatabaseRef};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::{net::SocketAddr, sync::RwLock};
use tokio::sync::{mpsc, oneshot, Mutex};
use tracing::{error, info, warn};

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
struct RpcState {
    /// Channel to send work to the dedicated worker thread
    tx: Arc<Mutex<mpsc::Sender<Work>>>,
}

/// Work item sent to the single-thread worker
struct Work {
    /// The RPC request to handle
    req: RpcRequest,
    /// Channel to send back the response
    rsp: oneshot::Sender<RpcResponse>,
}

/// Debug RPC server that owns the non-Send EngineContext
pub struct DebugRpcServer<DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone + 'static,
    <CacheDB<DB> as Database>::Error: Clone,
    <DB as Database>::Error: Clone,
{
    /// Complete debugging context (NOT Send/Sync due to Rc<RefCell> in REVM)
    context: Arc<EngineContext<DB>>,
    /// Current snapshot index (for navigation)
    current_snapshot_index: Arc<AtomicUsize>,
    /// Breakpoint management
    breakpoints: Arc<RwLock<Vec<super::types::Breakpoint>>>,
    /// Method handler for RPC dispatch
    method_handler: Arc<MethodHandler<DB>>,
}

impl<DB> DebugRpcServer<DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone + 'static,
    <CacheDB<DB> as Database>::Error: Clone,
    <DB as Database>::Error: Clone,
{
    /// Create a new debug RPC server
    pub fn new(context: EngineContext<DB>) -> Self {
        let context = Arc::new(context);
        let current_snapshot_index = Arc::new(AtomicUsize::new(0));
        let breakpoints = Arc::new(RwLock::new(Vec::new()));
        let method_handler = Arc::new(MethodHandler::new(
            context.clone(),
            current_snapshot_index.clone(),
            breakpoints.clone(),
        ));

        Self { context, current_snapshot_index, breakpoints, method_handler }
    }

    /// Start the RPC server
    pub async fn start(self) -> Result<RpcServerHandle> {
        let port = get_default_rpc_port()?;
        self.start_on_port(port).await
    }

    /// Start the RPC server on a specific port using LocalSet for non-Send types
    ///
    /// This method creates the Axum server that uses Send+Sync state, while keeping
    /// the non-Send EngineContext in a LocalSet (single-threaded execution).
    pub async fn start_on_port(self, port: u16) -> Result<RpcServerHandle> {
        // 1) Create channel to communicate between Axum handlers and the EngineContext
        let (tx, mut rx) = mpsc::channel::<Work>(1024);

        // 2) Spawn the engine context handler on LocalSet
        // Note: This requires the caller to be running inside a LocalSet context
        tokio::task::spawn_local(async move {
            info!("Starting RPC engine context handler");

            while let Some(Work { req, rsp }) = rx.recv().await {
                let response = self.handle_request(req).await;

                // Send response back (ignore if receiver dropped)
                if rsp.send(response).is_err() {
                    warn!("Client dropped connection before response");
                }
            }

            info!("RPC engine context handler shutting down");
        });

        // 3) Create the Axum app with Send+Sync state
        let app = Router::new()
            .route("/", post(handle_rpc_request))
            .route("/health", get(health_check))
            .with_state(RpcState { tx: Arc::new(Mutex::new(tx)) });

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

    /// Get current snapshot count
    pub fn snapshot_count(&self) -> usize {
        self.context.snapshots.len()
    }

    /// Get current snapshot index
    pub fn current_snapshot_index(&self) -> usize {
        self.current_snapshot_index.load(Ordering::SeqCst)
    }

    /// Set current snapshot index
    pub fn set_current_snapshot_index(&self, index: usize) -> Result<()> {
        if index >= self.snapshot_count() {
            return Err(eyre::eyre!("Snapshot index {} out of bounds", index));
        }
        self.current_snapshot_index.store(index, Ordering::SeqCst);
        Ok(())
    }
}

/// Handle RPC requests by forwarding to the engine context handler
async fn handle_rpc_request(
    State(state): State<RpcState>,
    JsonExtract(request): JsonExtract<RpcRequest>,
) -> JsonResponse<RpcResponse> {
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

    // Send work to the single-thread worker
    let (rsp_tx, rsp_rx) = oneshot::channel();
    let request_id = request.id.clone();
    {
        let tx = state.tx.lock().await;
        if tx.send(Work { req: request, rsp: rsp_tx }).await.is_err() {
            error!("Worker thread is dead");
            return JsonResponse(RpcResponse {
                jsonrpc: "2.0".to_string(),
                result: None,
                error: Some(RpcError {
                    code: -32603,
                    message: "Internal error - worker thread unavailable".to_string(),
                    data: None,
                }),
                id: request_id,
            });
        }
    }

    // Wait for response from worker
    let response = match rsp_rx.await {
        Ok(resp) => resp,
        Err(_) => {
            error!("Worker dropped response channel");
            RpcResponse {
                jsonrpc: "2.0".to_string(),
                result: None,
                error: Some(RpcError {
                    code: -32603,
                    message: "Internal error - worker communication failed".to_string(),
                    data: None,
                }),
                id: super::types::RpcId::String("unknown".to_string()),
            }
        }
    };

    JsonResponse(response)
}

/// Health check endpoint
async fn health_check() -> JsonResponse<serde_json::Value> {
    JsonResponse(serde_json::json!({
        "status": "healthy",
        "service": "edb-debug-rpc-server",
        "version": env!("CARGO_PKG_VERSION"),
        "architecture": "async-channel-proxy"
    }))
}

/// Create and start a debug RPC server with auto-port detection
pub async fn start_debug_server<DB>(context: EngineContext<DB>) -> Result<RpcServerHandle>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone + 'static,
    <CacheDB<DB> as Database>::Error: Clone,
    <DB as Database>::Error: Clone,
{
    let server = DebugRpcServer::new(context);
    server.start().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rpc_state_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<RpcState>();
    }

    #[test]
    fn test_work_message_not_send() {
        // Work contains oneshot::Sender which is Send, so this should compile
        fn assert_send<T: Send>() {}
        assert_send::<Work>();
    }
}
