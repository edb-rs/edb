// EDB - Ethereum Debugger
// Copyright (C) 2024 Zhuo Zhang and Wuqi Zhang
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

//! WebSocket server command - manages remote debugging sessions

use alloy_primitives::TxHash;
use axum::{
    extract::{
        ws::{Message, WebSocket},
        State, WebSocketUpgrade,
    },
    response::Response,
    routing::get,
    Router,
};
use edb_common::fork_and_prepare;
use edb_engine::Engine;
use eyre::Result;
use futures::{SinkExt, StreamExt};
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tokio::sync::Mutex;
use tracing::{error, info, warn};

use crate::ws_protocol::{ClientRequest, ServerResponse};

/// Server state shared across WebSocket connections
#[derive(Clone)]
struct ServerState {
    /// Shared thread-safe Engine instance
    /// Engine now handles its own internal locking per transaction
    engine: Arc<Engine>,
    /// Track number of active connections per tx_hash
    active_connections: Arc<Mutex<HashMap<TxHash, usize>>>,
    /// RPC proxy URL for forking
    rpc_url: String,
    /// Quick mode flag
    quick: bool,
}

/// Start the WebSocket server
pub async fn start_server(ws_port: u16, cli: &crate::Cli, rpc_url: &str) -> Result<()> {
    info!("Starting EDB WebSocket server on port {}", ws_port);

    // Create the engine with configuration
    let engine_config = cli.to_engine_config();
    let engine = Engine::new(engine_config);

    // Create shared state
    let state = ServerState {
        engine: Arc::new(engine),
        active_connections: Arc::new(Mutex::new(HashMap::new())),
        rpc_url: rpc_url.to_string(),
        quick: cli.quick,
    };

    // Create the Axum router
    let app = Router::new().route("/", get(ws_handler)).with_state(state);

    // Bind and serve
    let addr = SocketAddr::from(([127, 0, 0, 1], ws_port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let actual_addr = listener.local_addr()?;

    info!("WebSocket server listening on {}", actual_addr);

    // Run the server with LocalSet to support !Send futures from fork_and_prepare
    // While Engine is now Send + Sync, fork_and_prepare still uses REVM's LocalContext
    // Trade-off: Server handles one request at a time per connection, but Engine can
    // still process different transactions concurrently via its internal locking
    let local = tokio::task::LocalSet::new();
    local.run_until(async move { axum::serve(listener, app).await.expect("Server failed") }).await;

    Ok(())
}

/// WebSocket upgrade handler
async fn ws_handler(ws: WebSocketUpgrade, State(state): State<ServerState>) -> Response {
    ws.on_upgrade(|socket| async move {
        // Spawn on LocalSet to handle !Send futures
        tokio::task::spawn_local(handle_socket(socket, state)).await.ok();
    })
}

/// Handle a WebSocket connection
async fn handle_socket(socket: WebSocket, state: ServerState) {
    let (mut sender, mut receiver) = socket.split();

    // Wait for client request
    while let Some(msg) = receiver.next().await {
        let msg = match msg {
            Ok(msg) => msg,
            Err(e) => {
                error!("WebSocket error: {}", e);
                break;
            }
        };

        // Only handle text messages
        let text = match msg {
            Message::Text(text) => text,
            Message::Close(_) => {
                info!("Client closed connection");
                break;
            }
            _ => continue,
        };

        // Parse client request
        let request: ClientRequest = match serde_json::from_str(&text) {
            Ok(req) => req,
            Err(e) => {
                error!("Failed to parse client request: {}", e);
                let response = ServerResponse::error(format!("Invalid request: {e}"));
                if let Ok(json) = serde_json::to_string(&response) {
                    let _ = sender.send(Message::Text(json.into())).await;
                }
                continue;
            }
        };

        // Handle the request
        // We don't need tokio::spawn here because:
        // 1. Engine already handles per-tx-hash locking internally
        // 2. fork_and_prepare contains !Send types (REVM's LocalContext)
        // 3. Each WebSocket connection is already on its own async task
        let response = match request {
            ClientRequest::Replay { tx_hash } => handle_replay_request(tx_hash, &state).await,
            ClientRequest::Test { test_name, block } => {
                handle_test_request(test_name, block, &state).await
            }
        };

        // Send response
        if let Ok(json) = serde_json::to_string(&response) {
            if let Err(e) = sender.send(Message::Text(json.into())).await {
                error!("Failed to send response: {}", e);
                break;
            }
        }

        // If successful, track this connection and wait for close
        if let ServerResponse::Success { tx_hash, .. } = &response {
            if let Ok(tx_hash) = tx_hash.parse::<TxHash>() {
                // Keep connection alive and track it
                track_connection(tx_hash, &state, &mut receiver).await;
            }
        }
    }
}

/// Handle replay request
async fn handle_replay_request(tx_hash_str: String, state: &ServerState) -> ServerResponse {
    // Parse transaction hash
    let tx_hash: TxHash = match tx_hash_str.parse() {
        Ok(hash) => hash,
        Err(e) => {
            return ServerResponse::error(format!("Invalid transaction hash: {e}"));
        }
    };

    info!("Received replay request for tx: {:?}", tx_hash);

    // Check if we already have a server for this tx
    // Engine handles this check internally now, but we still track connections
    if let Some(addr) = state.engine.get_rpc_server_addr(&tx_hash) {
        info!("Reusing existing RPC server on port {} for tx: {:?}", addr.port(), tx_hash);

        // Increment connection count
        let mut connections = state.active_connections.lock().await;
        *connections.entry(tx_hash).or_insert(0) += 1;

        return ServerResponse::success(addr.port(), tx_hash_str, true);
    }

    // Fork and prepare, then run engine.prepare
    info!("Forking and preparing for tx: {:?}", tx_hash);
    let fork_result = match fork_and_prepare(&state.rpc_url, tx_hash, state.quick).await {
        Ok(result) => result,
        Err(e) => {
            error!("Failed to fork and prepare: {}", e);
            return ServerResponse::error(format!("Fork failed: {e}"));
        }
    };

    // Run engine.prepare - engine handles internal locking
    // Multiple concurrent requests for the same tx will be deduplicated by Engine
    info!("Running engine.prepare for tx: {:?}", tx_hash);
    let addr = match state.engine.prepare(fork_result).await {
        Ok(addr) => addr,
        Err(e) => {
            error!("Failed to prepare engine: {}", e);
            return ServerResponse::error(format!("Preparation failed: {e}"));
        }
    };

    let port = addr.port();
    info!("RPC server started on port {} for tx: {:?}", port, tx_hash);

    // Initialize connection count to 1
    {
        let mut connections = state.active_connections.lock().await;
        connections.insert(tx_hash, 1);
    }

    ServerResponse::success(port, tx_hash_str, false)
}

/// Handle test request (not yet implemented)
async fn handle_test_request(
    _test_name: String,
    _block: Option<u64>,
    _state: &ServerState,
) -> ServerResponse {
    ServerResponse::error("Test debugging not yet implemented")
}

/// Track connection and handle disconnection
async fn track_connection(
    tx_hash: TxHash,
    state: &ServerState,
    receiver: &mut futures::stream::SplitStream<WebSocket>,
) {
    // Wait for the connection to close
    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(Message::Close(_)) | Err(_) => break,
            _ => continue,
        }
    }

    info!("Connection closed for tx: {:?}", tx_hash);

    // Decrement connection count
    let should_shutdown = {
        let mut connections = state.active_connections.lock().await;
        if let Some(count) = connections.get_mut(&tx_hash) {
            *count -= 1;
            if *count == 0 {
                connections.remove(&tx_hash);
                true
            } else {
                info!("Still {} active connections for tx: {:?}", count, tx_hash);
                false
            }
        } else {
            false
        }
    };

    // If no more connections, shutdown the RPC server
    if should_shutdown {
        info!("No more connections for tx: {:?}, shutting down RPC server", tx_hash);

        // Shutdown the RPC server using Engine's shutdown method
        if let Err(e) = state.engine.shutdown_rpc_server(&tx_hash) {
            warn!("Error shutting down RPC server: {}", e);
        } else {
            info!("RPC server shutdown successfully for tx: {:?}", tx_hash);
        }
    }
}
