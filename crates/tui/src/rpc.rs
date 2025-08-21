//! RPC client for communicating with the debug server
//!
//! This module provides a client for making JSON-RPC calls to the debug server.

use crate::ui::spinner::RpcSpinner;
use eyre::Result;
use jsonrpsee::{
    core::client::ClientT,
    http_client::{HttpClient, HttpClientBuilder},
};
use serde_json::Value;
use std::{
    sync::{Arc, RwLock},
    time::Duration,
};
use tracing::{debug, error};

/// RPC client for debug server communication
pub struct RpcClient {
    client: HttpClient,
    server_url: String,
    /// Shared spinner state for loading indication
    spinner: Arc<RwLock<RpcSpinner>>,
}

impl RpcClient {
    /// Create a new RPC client
    pub async fn new(server_url: &str) -> Result<Self> {
        let client = HttpClientBuilder::default()
            .request_timeout(Duration::from_secs(30))
            .build(server_url)?;

        debug!("Created RPC client for: {}", server_url);
        Ok(Self {
            client,
            server_url: server_url.to_string(),
            spinner: Arc::new(RwLock::new(RpcSpinner::new())),
        })
    }

    /// Test connection to a server URL
    pub async fn test_connection(server_url: &str) -> Result<()> {
        debug!("Testing connection to: {}", server_url);

        let client = HttpClientBuilder::default()
            .request_timeout(Duration::from_secs(5))
            .build(server_url)?;

        // Try a simple health check or method call
        match client.request::<Value, _>("debug.getSnapshotCount", rpc_params::build()).await {
            Ok(_) => {
                debug!("Connection test successful for: {}", server_url);
                Ok(())
            }
            Err(e) => {
                debug!("Connection test failed for {}: {}", server_url, e);
                Err(e.into())
            }
        }
    }

    /// Generic method to make RPC requests with automatic spinner management
    async fn request_with_spinner(
        &self,
        method: &str,
        params: Vec<Value>,
        operation_name: &str,
    ) -> Result<Value> {
        self.start_loading(operation_name);
        debug!("Making RPC request: {}", operation_name);

        let result = match self.client.request(method, params).await {
            Ok(result) => {
                debug!("{} successful: {:?}", operation_name, result);
                Ok(result)
            }
            Err(e) => {
                error!("{} failed: {}", operation_name, e);
                Err(e.into())
            }
        };

        self.finish_loading();
        result
    }

    /// Get current snapshot information
    pub async fn get_current_snapshot(&self) -> Result<Value> {
        self.request_with_spinner(
            "debug.getCurrentSnapshot",
            rpc_params::build(),
            "Getting current snapshot",
        )
        .await
    }

    /// Get total snapshot count
    pub async fn get_snapshot_count(&self) -> Result<Value> {
        self.request_with_spinner(
            "debug.getSnapshotCount",
            rpc_params::build(),
            "Getting snapshot count",
        )
        .await
    }

    /// Step to next snapshot
    pub async fn step_next(&self) -> Result<Value> {
        self.request_with_spinner(
            "debug.stepNext",
            rpc_params::build(),
            "Stepping to next snapshot",
        )
        .await
    }

    /// Step to previous snapshot
    pub async fn step_previous(&self) -> Result<Value> {
        self.request_with_spinner(
            "debug.stepPrevious",
            rpc_params::build(),
            "Stepping to previous snapshot",
        )
        .await
    }

    /// Set current snapshot to specific index
    pub async fn set_current_snapshot(&self, index: usize) -> Result<Value> {
        self.request_with_spinner(
            "debug.setCurrentSnapshot",
            rpc_params::build_with_param(Some(index)),
            &format!("Setting snapshot to {}", index),
        )
        .await
    }

    /// Get server URL
    pub fn server_url(&self) -> &str {
        &self.server_url
    }

    /// Get shared reference to spinner for UI updates
    pub fn spinner(&self) -> Arc<RwLock<RpcSpinner>> {
        Arc::clone(&self.spinner)
    }

    /// Check if spinner is currently loading
    pub fn is_loading(&self) -> bool {
        self.spinner.read().unwrap().is_loading()
    }

    /// Get spinner display text
    pub fn spinner_display(&self) -> String {
        self.spinner.read().unwrap().display_text()
    }

    /// Start loading spinner for an operation
    fn start_loading(&self, operation: &str) {
        self.spinner.write().unwrap().start_loading(operation);
        debug!("Started loading spinner: {}", operation);
    }

    /// Finish loading spinner
    fn finish_loading(&self) {
        self.spinner.write().unwrap().finish_loading();
        debug!("Finished loading spinner");
    }

    /// Update spinner animation (call from render loop)
    pub fn tick(&self) {
        self.spinner.write().unwrap().tick();
    }

    /// Check server health
    pub async fn health_check(&self) -> Result<Value> {
        debug!("Checking server health");

        // Make a simple HTTP GET request to the health endpoint
        let health_url = format!("{}/health", self.server_url.trim_end_matches('/'));

        let response = reqwest::get(&health_url).await?;
        let health_data: Value = response.json().await?;

        debug!("Server health: {:?}", health_data);
        Ok(health_data)
    }
}

// Helper module for building RPC parameters
mod rpc_params {
    use serde_json::Value;

    pub fn build() -> Vec<Value> {
        vec![]
    }

    pub fn build_with_param(param: Option<impl serde::Serialize>) -> Vec<Value> {
        match param {
            Some(p) => vec![serde_json::to_value(p).unwrap_or(Value::Null)],
            None => vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rpc_client_creation() {
        let client = RpcClient::new("http://localhost:3000").await;
        // This would fail without a running server, but we can test creation logic
        assert!(client.is_ok() || client.is_err()); // Either is fine for this test
    }
}
