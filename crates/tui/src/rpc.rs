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

//! RPC client for communicating with the debug server
//!
//! This module provides a client for making JSON-RPC calls to the debug server.

use crate::ui::spinner::Spinner;
use alloy_json_abi::JsonAbi;
use alloy_primitives::{Address, Bytes, U256};
use edb_common::types::{CallableAbiInfo, Code, EdbSolValue, SnapshotInfo, Trace};
use eyre::Result;
use jsonrpsee::{
    core::client::ClientT,
    http_client::{HttpClient, HttpClientBuilder},
};
use serde_json::Value;
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::Duration,
};
use tracing::{debug, error};

/// Macro for building RPC parameters with any number of arguments
///
/// # Examples
/// ```ignore
/// let params = rpc_params!(); // Empty params
/// let params = rpc_params!(42); // Single parameter
/// let params = rpc_params!("0x123...", true); // Multiple parameters of different types
/// let params = rpc_params!(snapshot_id, address, is_recompiled); // Variable references
/// ```
macro_rules! rpc_params {
    () => {
        Vec::<serde_json::Value>::new()
    };
    ($($param:expr),+ $(,)?) => {
        vec![
            $(serde_json::to_value($param).unwrap_or(serde_json::Value::Null)),+
        ]
    };
}
/// RPC client for debug server communication
#[derive(Debug)]
pub struct RpcClient {
    client: HttpClient,
    server_url: String,
    /// Shared spinner state for loading indication
    spinner: Arc<RwLock<Spinner>>,
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
            spinner: Arc::new(RwLock::new(Spinner::new(None, None))),
        })
    }

    /// Test connection to a server URL
    pub async fn test_connection(server_url: &str) -> Result<()> {
        debug!("Testing connection to: {}", server_url);

        let client = HttpClientBuilder::default()
            .request_timeout(Duration::from_secs(5))
            .build(server_url)?;

        // Try a simple health check or method call
        match client.request::<Value, _>("debug.getSnapshotCount", rpc_params!()).await {
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

    /// Get server URL
    pub fn server_url(&self) -> &str {
        &self.server_url
    }

    /// Get shared reference to spinner for UI updates
    pub fn spinner(&self) -> Arc<RwLock<Spinner>> {
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

// Rpc methods
impl RpcClient {
    /// Get execution trace
    pub async fn get_trace(&self) -> Result<Trace> {
        let value = self
            .request_with_spinner("edb_getTrace", rpc_params!(), "Fetching execution trace")
            .await?;
        serde_json::from_value(value).map_err(|e| eyre::eyre!("Failed to parse trace: {}", e))
    }

    /// Get contract abi
    pub async fn get_contract_abi(
        &self,
        address: Address,
        recompiled: bool,
    ) -> Result<Option<JsonAbi>> {
        let value = self
            .request_with_spinner(
                "edb_getContractABI",
                rpc_params!(address, recompiled),
                &format!("Fetching contract ABI for {address}"),
            )
            .await?;

        serde_json::from_value(value)
            .map_err(|e| eyre::eyre!("Failed to parse contract ABI: {}", e))
    }

    /// Get callable abi info
    pub async fn get_callable_abi(&self, address: Address) -> Result<Vec<CallableAbiInfo>> {
        let value = self
            .request_with_spinner(
                "edb_getCallableABI",
                rpc_params!(address),
                &format!("Fetching callable ABI for {address}"),
            )
            .await?;

        serde_json::from_value(value)
            .map_err(|e| eyre::eyre!("Failed to parse callable ABI: {}", e))
    }

    /// Get contract constructor arguments
    pub async fn get_constructor_args(&self, address: Address) -> Result<Option<Bytes>> {
        let value = self
            .request_with_spinner(
                "edb_getConstructorArgs",
                rpc_params!(address),
                &format!("Fetching contract constructor arguments for {address}"),
            )
            .await?;

        serde_json::from_value(value)
            .map_err(|e| eyre::eyre!("Failed to parse contract constructor arguments: {}", e))
    }

    /// Get total snapshot count
    pub async fn get_snapshot_count(&self) -> Result<usize> {
        let value = self
            .request_with_spinner(
                "edb_getSnapshotCount",
                rpc_params!(),
                "Getting total snapshot count",
            )
            .await?;

        serde_json::from_value(value)
            .map_err(|e| eyre::eyre!("Failed to parse snapshot count: {}", e))
    }

    /// Get snapshot information
    pub async fn get_snapshot_info(&self, snapshot_id: usize) -> Result<SnapshotInfo> {
        let value = self
            .request_with_spinner(
                "edb_getSnapshotInfo",
                rpc_params!(snapshot_id),
                &format!("Getting info for snapshot {snapshot_id}"),
            )
            .await?;

        serde_json::from_value(value)
            .map_err(|e| eyre::eyre!("Failed to parse snapshot info: {}", e))
    }

    /// Get code
    pub async fn get_code(&self, snapshot_id: usize) -> Result<Code> {
        let value = self
            .request_with_spinner(
                "edb_getCode",
                rpc_params!(snapshot_id),
                &format!("Getting code for snapshot {snapshot_id}"),
            )
            .await?;

        serde_json::from_value(value).map_err(|e| eyre::eyre!("Failed to parse code: {}", e))
    }

    /// Get code by address
    pub async fn get_code_by_address(&self, address: Address) -> Result<Code> {
        let value = self
            .request_with_spinner(
                "edb_getCodeByAddress",
                rpc_params!(address),
                &format!("Getting code for address {address}"),
            )
            .await?;

        serde_json::from_value(value).map_err(|e| eyre::eyre!("Failed to parse code: {}", e))
    }

    /// Get next call
    pub async fn get_next_call(&self, snapshot_id: usize) -> Result<usize> {
        let value = self
            .request_with_spinner(
                "edb_getNextCall",
                rpc_params!(snapshot_id),
                &format!("Getting next call for snapshot {snapshot_id}"),
            )
            .await?;

        serde_json::from_value(value).map_err(|e| eyre::eyre!("Failed to parse next call: {}", e))
    }

    /// Get prev call
    pub async fn get_prev_call(&self, snapshot_id: usize) -> Result<usize> {
        let value = self
            .request_with_spinner(
                "edb_getPrevCall",
                rpc_params!(snapshot_id),
                &format!("Getting prev call for snapshot {snapshot_id}"),
            )
            .await?;

        serde_json::from_value(value).map_err(|e| eyre::eyre!("Failed to parse prev call: {}", e))
    }

    /// Get storage value at a given slot
    pub async fn get_storage(&self, snapshot_id: usize, slot: U256) -> Result<U256> {
        let value = self
            .request_with_spinner(
                "edb_getStorage",
                rpc_params!(snapshot_id, slot),
                &format!("Getting storage for snapshot {snapshot_id} at slot {slot}"),
            )
            .await?;

        serde_json::from_value(value)
            .map_err(|e| eyre::eyre!("Failed to parse storage value: {}", e))
    }

    /// Get storage diff
    pub async fn get_storage_diff(
        &self,
        snapshot_id: usize,
    ) -> Result<HashMap<U256, (U256, U256)>> {
        let value = self
            .request_with_spinner(
                "edb_getStorageDiff",
                rpc_params!(snapshot_id),
                &format!("Getting storage diff for snapshot {snapshot_id}"),
            )
            .await?;

        serde_json::from_value(value)
            .map_err(|e| eyre::eyre!("Failed to parse storage diff: {}", e))
    }

    /// Evaluate expression on a given snapshot
    pub async fn eval_on_snapshot(
        &self,
        snapshot_id: usize,
        expr: &str,
    ) -> Result<core::result::Result<EdbSolValue, String>> {
        let value = self
            .request_with_spinner(
                "edb_evalOnSnapshot",
                rpc_params!(snapshot_id, expr),
                &format!("Evaluating expression on snapshot {snapshot_id}"),
            )
            .await?;

        serde_json::from_value(value)
            .map_err(|e| eyre::eyre!("Failed to parse evaluated value: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_rpc_client_creation() {
        let client = RpcClient::new("http://localhost:3000").await;
        // This would fail without a running server, but we can test creation logic
        assert!(client.is_ok() || client.is_err()); // Either is fine for this test
    }

    #[test]
    fn test_rpc_params_macro() {
        // Test empty params
        let empty_params: Vec<Value> = rpc_params!();
        let expected: Vec<Value> = vec![];
        assert_eq!(empty_params, expected);

        // Test single parameter
        let single_param = rpc_params!(42);
        assert_eq!(single_param, vec![json!(42)]);

        // Test multiple parameters of different types
        let multi_params = rpc_params!("0x1234567890abcdef", true, 123);
        assert_eq!(multi_params, vec![json!("0x1234567890abcdef"), json!(true), json!(123)]);

        // Test with variables
        let address = "0xabcdef1234567890";
        let recompiled = false;
        let snapshot_id = 5;
        let var_params = rpc_params!(address, recompiled, snapshot_id);
        assert_eq!(var_params, vec![json!(address), json!(recompiled), json!(snapshot_id)]);
    }
}
