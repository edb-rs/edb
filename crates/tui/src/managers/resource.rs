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

//! Resource management for TUI panels

use crate::rpc::RpcClient;
use alloy_primitives::{Address, Bytes, U256};
use edb_common::types::{CallResult, CallType, Trace, TraceEntry};
use eyre::Result;
use revm::context::CreateScheme;
use revm::interpreter::CallScheme;
use std::sync::Arc;

/// Resource manager for handling trace data and other shared resources
#[derive(Debug)]
pub struct ResourceManager {
    /// RPC client for server communication
    rpc_client: Arc<RpcClient>,
    /// Cached trace data (None = not loaded, Some = loaded)
    trace_data: Option<Trace>,
}

impl ResourceManager {
    /// Create a new resource manager
    pub fn new(rpc_client: Arc<RpcClient>) -> Self {
        Self { rpc_client, trace_data: None }
    }

    pub async fn get_trace(&self) -> Option<&Trace> {
        self.trace_data.as_ref()
    }

    pub async fn fetch_trace(&mut self) -> Result<()> {
        match self.rpc_client.get_trace().await {
            Ok(trace_value) => match serde_json::from_value::<Trace>(trace_value) {
                Ok(trace) => {
                    self.trace_data = Some(trace);
                    Ok(())
                }
                Err(e) => {
                    tracing::warn!("Failed to deserialize trace: {}", e);
                    Err(e.into())
                }
            },
            Err(e) => {
                tracing::warn!("Failed to fetch trace: {}", e);
                Err(e)
            }
        }
    }

    /// Clear cached trace data (for refresh)
    pub fn clear_trace_cache(&mut self) {
        self.trace_data = None;
    }
}
