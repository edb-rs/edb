//! JSON-RPC server for debugging control
//!
//! This module provides a JSON-RPC interface for front-ends to control
//! and inspect debugging sessions.

use axum::{
    response::Json,
    routing::{get, post},
    Router,
};
use eyre::Result;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

/// Handle to the running RPC server
#[derive(Debug)]
pub struct RpcServerHandle {
    /// Address the server is listening on
    pub addr: SocketAddr,
    /// Shutdown signal
    shutdown_tx: tokio::sync::oneshot::Sender<()>,
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

/// Start the JSON-RPC server
pub async fn start_server(port: u16) -> Result<RpcServerHandle> {
    let app =
        Router::new().route("/", post(handle_rpc_request)).route("/health", get(health_check));

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
