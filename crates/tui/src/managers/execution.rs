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

//! Execution state management for TUI panels

use eyre::Result;
use std::{
    ops::{Deref, DerefMut},
    sync::{Arc, RwLock},
};

use edb_common::types::Trace;

use crate::RpcClient;

#[derive(Debug, Clone)]
pub struct ExecutionManager {
    /// Current snapshot index
    pub current_snapshot: usize,
    /// Total number of snapshots
    pub total_snapshots: usize,
    /// Current execution line (1-based line number)
    pub current_line: Option<usize>,
    /// Current file path
    pub current_file: Option<String>,
    /// Whether execution is paused/stopped
    pub is_paused: bool,

    /// Trace data
    pub trace_data: Option<Trace>,

    core: Arc<RwLock<ExecutionManagerCore>>,
}

impl ExecutionManager {
    /// Create new execution state
    pub fn new(core: Arc<RwLock<ExecutionManagerCore>>) -> Self {
        Self {
            current_snapshot: 0,
            total_snapshots: 0,
            current_line: None,
            current_file: None,
            is_paused: true,
            trace_data: None,
            core,
        }
    }

    /// Fetch data
    pub async fn fetch_data(&mut self) -> eyre::Result<()> {
        let mut core = self.core.write().unwrap();
        core.fetch_data().await?;

        if self.trace_data.is_none() {
            self.trace_data = core.get_trace().cloned();
        }

        Ok(())
    }
}

impl Deref for ExecutionManager {
    type Target = Arc<RwLock<ExecutionManagerCore>>;

    fn deref(&self) -> &Self::Target {
        &self.core
    }
}

impl DerefMut for ExecutionManager {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.core
    }
}

/// Shared execution manager for communication between panels
#[derive(Debug)]
pub struct ExecutionManagerCore {
    /// RPC client for server communication
    rpc_client: Arc<RpcClient>,
    /// Cached trace data (None = not loaded, Some = loaded)
    trace_data: Option<Trace>,
}

impl ExecutionManagerCore {
    /// Create a new execution manager
    pub fn new(rpc_client: Arc<RpcClient>) -> Self {
        Self { rpc_client, trace_data: None }
    }

    /// Fetch data
    pub async fn fetch_data(&mut self) -> Result<()> {
        self.fetch_trace().await?;

        Ok(())
    }

    /// Get trace data
    pub fn get_trace(&self) -> Option<&Trace> {
        self.trace_data.as_ref()
    }

    /// Fetch trace data
    async fn fetch_trace(&mut self) -> Result<()> {
        if self.trace_data.is_some() {
            // We already have trace data, no need to fetch
            return Ok(());
        }

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
}
