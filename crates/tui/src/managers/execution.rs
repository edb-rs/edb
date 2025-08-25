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
//!
//! This module implements a two-tier architecture for execution state management:
//!
//! - `ExecutionManager`: Per-thread instance with cached execution state for immediate reads
//! - `ExecutionManagerCore`: Shared core that handles trace data fetching and complex operations
//!
//! This design ensures rendering threads can access execution state without blocking
//! on RPC calls or complex trace processing.

use eyre::Result;
use std::{
    ops::{Deref, DerefMut},
    sync::{Arc, RwLock},
};

use edb_common::types::Trace;

use crate::RpcClient;

/// Per-thread execution manager with cached state for rendering
///
/// # Design Philosophy
///
/// `ExecutionManager` provides immediate access to execution state:
/// - All fields are directly accessible for non-blocking reads
/// - State updates happen through explicit `fetch_data()` calls
/// - Complex trace processing is handled by ExecutionManagerCore
///
/// # Cached Fields
///
/// All public fields are cached locally for immediate access:
/// - `current_snapshot`: Current execution position
/// - `total_snapshots`: Total execution steps available
/// - `current_line`: Active source line being executed
/// - `current_file`: Active source file
/// - `is_paused`: Execution pause state
/// - `trace_data`: Full trace data for rendering
///
/// # Usage Pattern
///
/// ```ignore
/// // In rendering loop - all reads are immediate
/// let line = execution_manager.current_line;
/// let paused = execution_manager.is_paused;
///
/// // When state changes
/// execution_manager.fetch_data().await?; // Sync with core
/// ```
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
    /// Create new execution manager with shared core
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

    /// Synchronize local execution state with the shared core
    ///
    /// This is the only async operation, designed to:
    /// - Fetch latest trace data from ExecutionManagerCore
    /// - Update all cached fields for immediate access
    /// - Be called when execution state changes or on initialization
    pub async fn fetch_data(&mut self) -> eyre::Result<()> {
        let mut core = self.core.write().unwrap();
        core.fetch_data().await?;

        // Update local cached trace data
        if self.trace_data.is_none() {
            self.trace_data = core.get_trace().cloned();
        }

        // TODO: Update other cached execution state fields
        // self.current_snapshot = core.get_current_snapshot();
        // self.total_snapshots = core.get_total_snapshots();
        // self.current_line = core.get_current_line();
        // self.current_file = core.get_current_file();
        // self.is_paused = core.is_paused();

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

/// Centralized execution state manager handling trace data and debugging operations
///
/// # Design Philosophy
///
/// `ExecutionManagerCore` is the single source of truth for execution state:
/// - Handles all RPC communication for trace data
/// - Manages complex trace processing and analysis
/// - Provides thread-safe access to execution state
/// - Caches expensive operations for efficiency
///
/// All complex debugging operations and RPC calls happen here,
/// keeping ExecutionManager instances lightweight.
///
/// # Architecture Benefits
///
/// - **Non-blocking UI**: Rendering never waits on trace fetching
/// - **Centralized Logic**: All debugging logic in one place
/// - **Shared State**: Multiple panels share the same execution state
/// - **Efficient Caching**: Trace data fetched once, used everywhere
#[derive(Debug)]
pub struct ExecutionManagerCore {
    /// RPC client for server communication
    rpc_client: Arc<RpcClient>,
    /// Cached trace data (None = not loaded, Some = loaded)
    trace_data: Option<Trace>,
    // TODO: Add more execution state fields
    // current_snapshot: usize,
    // breakpoints: Vec<Breakpoint>,
    // call_stack: Vec<CallFrame>,
}

impl ExecutionManagerCore {
    /// Create a new execution manager core with RPC client
    pub fn new(rpc_client: Arc<RpcClient>) -> Self {
        Self { rpc_client, trace_data: None }
    }

    /// Fetch and process execution data from debug server
    ///
    /// This method:
    /// - Fetches trace data via RPC if not cached
    /// - Processes and analyzes the trace
    /// - Updates internal state for ExecutionManager instances
    pub async fn fetch_data(&mut self) -> Result<()> {
        self.fetch_trace().await?;
        // TODO: Process trace to extract execution state
        // self.update_execution_position();
        // self.update_call_stack();
        Ok(())
    }

    /// Get cached trace data
    pub fn get_trace(&self) -> Option<&Trace> {
        self.trace_data.as_ref()
    }

    /// Fetch trace data from debug server
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
