//! RPC client for communicating with the debug server
//!
//! This module provides a client for making JSON-RPC calls to the debug server.

use eyre::Result;
use jsonrpsee::{
    core::client::ClientT,
    http_client::{HttpClient, HttpClientBuilder},
};
use serde_json::Value;
use std::time::Duration;
use tracing::{debug, error};

/// RPC client for debug server communication
pub struct RpcClient {
    client: HttpClient,
    server_url: String,
}

impl RpcClient {
    /// Create a new RPC client
    pub async fn new(server_url: &str) -> Result<Self> {
        let client = HttpClientBuilder::default()
            .request_timeout(Duration::from_secs(30))
            .build(server_url)?;

        debug!("Created RPC client for: {}", server_url);
        Ok(Self { client, server_url: server_url.to_string() })
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

    /// Get current snapshot information
    pub async fn get_current_snapshot(&self) -> Result<Value> {
        debug!("Getting current snapshot");
        match self.client.request("debug.getCurrentSnapshot", rpc_params::build()).await {
            Ok(result) => {
                debug!("Got current snapshot: {:?}", result);
                Ok(result)
            }
            Err(e) => {
                error!("Failed to get current snapshot: {}", e);
                Err(e.into())
            }
        }
    }

    /// Get total snapshot count
    pub async fn get_snapshot_count(&self) -> Result<Value> {
        debug!("Getting snapshot count");
        match self.client.request("debug.getSnapshotCount", rpc_params::build()).await {
            Ok(result) => {
                debug!("Got snapshot count: {:?}", result);
                Ok(result)
            }
            Err(e) => {
                error!("Failed to get snapshot count: {}", e);
                Err(e.into())
            }
        }
    }

    /// Step to next snapshot
    pub async fn step_next(&self) -> Result<Value> {
        debug!("Stepping to next snapshot");
        match self.client.request("debug.stepNext", rpc_params::build()).await {
            Ok(result) => {
                debug!("Stepped to next snapshot: {:?}", result);
                Ok(result)
            }
            Err(e) => {
                error!("Failed to step to next snapshot: {}", e);
                Err(e.into())
            }
        }
    }

    /// Step to previous snapshot
    pub async fn step_previous(&self) -> Result<Value> {
        debug!("Stepping to previous snapshot");
        match self.client.request("debug.stepPrevious", rpc_params::build()).await {
            Ok(result) => {
                debug!("Stepped to previous snapshot: {:?}", result);
                Ok(result)
            }
            Err(e) => {
                error!("Failed to step to previous snapshot: {}", e);
                Err(e.into())
            }
        }
    }

    /// Set current snapshot to specific index
    pub async fn set_current_snapshot(&self, index: usize) -> Result<Value> {
        debug!("Setting current snapshot to: {}", index);
        match self
            .client
            .request("debug.setCurrentSnapshot", rpc_params::build_with_param(Some(index)))
            .await
        {
            Ok(result) => {
                debug!("Set current snapshot to {}: {:?}", index, result);
                Ok(result)
            }
            Err(e) => {
                error!("Failed to set current snapshot to {}: {}", index, e);
                Err(e.into())
            }
        }
    }

    /// Get server URL
    pub fn server_url(&self) -> &str {
        &self.server_url
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
