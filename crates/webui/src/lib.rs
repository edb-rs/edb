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

// Copyright (C) 2024 Zhuo Zhang and Wuqi Zhang
// SPDX-License-Identifier: AGPL-3.0
//! Web User Interface for EDB
//!
//! This crate provides a web-based interface for interacting with the EDB engine.

use axum::{routing::get, Router};
use eyre::Result;
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;

/// Configuration for the Web UI
#[derive(Debug, Clone)]
pub struct WebUiConfig {
    /// Port to serve the web UI on
    pub port: u16,
    /// RPC endpoint of the engine
    pub engine_rpc_url: String,
}

impl Default for WebUiConfig {
    fn default() -> Self {
        Self { port: 3000, engine_rpc_url: "http://localhost:8545".to_string() }
    }
}

/// Main Web UI server
pub struct WebUi {
    config: WebUiConfig,
}

impl WebUi {
    /// Create a new Web UI instance
    pub fn new(config: WebUiConfig) -> Self {
        Self { config }
    }

    /// Build the router for the web application
    fn build_router(&self) -> Router {
        Router::new()
            .route("/", get(|| async { "EDB Web UI - Coming Soon" }))
            .route("/health", get(|| async { "OK" }))
            .layer(CorsLayer::permissive())
    }

    /// Run the Web UI server
    pub async fn run(self) -> Result<()> {
        let app = self.build_router();
        let addr = SocketAddr::from(([127, 0, 0, 1], self.config.port));

        tracing::info!("Starting Web UI server on http://{}", addr);

        // Skeleton implementation - to be expanded later
        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app).await?;

        Ok(())
    }
}

/// Public API for the Web UI module
pub mod api {
    use super::*;

    /// Start the Web UI with the given configuration
    pub async fn start_webui(config: WebUiConfig) -> Result<()> {
        let webui = WebUi::new(config);
        webui.run().await
    }
}
