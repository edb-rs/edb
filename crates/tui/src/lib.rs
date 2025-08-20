// Copyright (C) 2024 Zhuo Zhang and Wuqi Zhang
// SPDX-License-Identifier: AGPL-3.0
//! Terminal User Interface for EDB
//!
//! This crate provides a terminal-based interface for interacting with the EDB engine.

use eyre::Result;
use jsonrpsee::http_client::{HttpClient, HttpClientBuilder};
use std::time::Duration;

/// Configuration for the TUI
#[derive(Debug, Clone)]
pub struct TuiConfig {
    /// RPC endpoint of the engine
    pub rpc_url: String,
    /// Refresh interval for the UI
    pub refresh_interval: Duration,
}

impl Default for TuiConfig {
    fn default() -> Self {
        Self {
            rpc_url: "http://localhost:8545".to_string(),
            refresh_interval: Duration::from_millis(100),
        }
    }
}

/// Main TUI application
pub struct Tui {
    config: TuiConfig,
    client: HttpClient,
}

impl Tui {
    /// Create a new TUI instance
    pub async fn new(config: TuiConfig) -> Result<Self> {
        let client = HttpClientBuilder::default().build(&config.rpc_url)?;

        Ok(Self { config, client })
    }

    /// Run the TUI application
    pub async fn run(self) -> Result<()> {
        // Skeleton implementation - to be filled later
        tracing::info!("Starting TUI with config: {:?}", self.config);
        Ok(())
    }
}

/// Public API for the TUI module
pub mod api {
    use super::*;

    /// Start the TUI with the given configuration
    pub async fn start_tui(config: TuiConfig) -> Result<()> {
        let tui = Tui::new(config).await?;
        tui.run().await
    }
}
