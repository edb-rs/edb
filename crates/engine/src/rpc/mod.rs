//! JSON-RPC server for debugging control
//!
//! This module provides a JSON-RPC interface for front-ends to control
//! and inspect debugging sessions.

use crate::analysis::AnalysisResult;
use alloy_primitives::Address;
use axum::{
    response::Json,
    routing::{get, post},
    Router,
};
use eyre::Result;
use revm::database::CacheDB;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, net::SocketAddr};
use std::sync::Arc;

/// Handle to the running RPC server
#[derive(Debug)]
pub struct RpcServerHandle {
    /// Address the server is listening on
    pub addr: SocketAddr,
    /// Shutdown signal
    shutdown_tx: tokio::sync::oneshot::Sender<()>,
}

impl RpcServerHandle {
    /// Gracefully shutdown the RPC server
    pub fn shutdown(self) -> Result<()> {
        if let Err(_) = self.shutdown_tx.send(()) {
            tracing::warn!("RPC server already shut down");
        }
        Ok(())
    }
}

/// RPC request structure
#[derive(Debug, Deserialize)]
struct RpcRequest {
    jsonrpc: String,
    method: String,
    params: serde_json::Value,
    id: u64,
}

/// RPC response structure
#[derive(Debug, Serialize)]
struct RpcResponse {
    jsonrpc: String,
    result: Option<serde_json::Value>,
    error: Option<RpcError>,
    id: u64,
}

/// RPC error structure
#[derive(Debug, Serialize)]
struct RpcError {
    code: i32,
    message: String,
}

/// State snapshots - a vector of CacheDB instances at different execution points
pub type StateSnapshots<DB> = Vec<CacheDB<DB>>;

/// Server state containing analysis results and state snapshots
#[derive(Clone)]
struct ServerState<DB> {
    analysis_result: Arc<HashMap<Address, AnalysisResult>>,
    snapshots: Arc<StateSnapshots<DB>>,
}

/// Start the JSON-RPC server with analysis results and state snapshots
pub async fn start_server<DB>(
    port: u16,
    analysis_results: HashMap<Address, AnalysisResult>,
    snapshots: StateSnapshots<DB>,
) -> Result<RpcServerHandle>
where
    DB: Clone + Send + Sync + 'static,
{
    let state =
        ServerState { analysis_result: Arc::new(analysis_results), snapshots: Arc::new(snapshots) };

    let app = Router::new()
        .route("/", post(handle_rpc_request))
        .route("/health", get(health_check))
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let actual_addr = listener.local_addr()?;

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();

    // Spawn the server
    tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                shutdown_rx.await.ok();
            })
            .await
            .expect("RPC server failed");
    });

    tracing::info!("JSON-RPC server started on {}", actual_addr);

    Ok(RpcServerHandle { addr: actual_addr, shutdown_tx })
}

/// Handle RPC requests
async fn handle_rpc_request(Json(request): Json<RpcRequest>) -> Json<RpcResponse> {
    tracing::debug!("Received RPC request: {:?}", request);

    // Stub implementation - just echo back for now
    let response = RpcResponse {
        jsonrpc: "2.0".to_string(),
        result: Some(serde_json::json!({
            "method": request.method,
            "status": "ok",
            "message": "RPC server is running (stub implementation)"
        })),
        error: None,
        id: request.id,
    };

    Json(response)
}

/// Health check endpoint
async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "edb-engine-rpc"
    }))
}
