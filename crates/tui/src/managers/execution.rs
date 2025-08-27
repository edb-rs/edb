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

use alloy_primitives::Address;
use eyre::Result;
use std::{collections::HashSet, mem, ops::Deref, sync::Arc};
use tokio::sync::RwLock;
use tracing::debug;

use edb_common::types::{Code, SnapshotInfo, Trace};

use crate::{managers::FetchCache, RpcClient};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum PendingRequest {
    SnapshotInfo(usize),
    Code(usize),
}

#[derive(Debug, Clone)]
pub struct ExecutionState {
    snapshot_count: usize,
    snapshot_info: FetchCache<usize, SnapshotInfo>,
    code: FetchCache<Address, Code>,
    trace_data: Trace,
}

impl ExecutionState {
    async fn with_rpc_client(rpc_client: Arc<RpcClient>) -> Result<Self> {
        let snapshot_count = rpc_client.get_snapshot_count().await?;
        let trace_data = rpc_client.get_trace().await?;
        Ok(Self {
            snapshot_count,
            snapshot_info: FetchCache::new(),
            code: FetchCache::new(),
            trace_data,
        })
    }
}

impl ExecutionState {
    fn update(&mut self, other: &ExecutionState) {
        if self.snapshot_info.need_update(&other.snapshot_info) {
            self.snapshot_info.update(&other.snapshot_info);
        }

        if self.code.need_update(&other.code) {
            self.code.update(&other.code);
        }
    }
}

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
    // User-controlled data
    current_snapshot: usize,
    display_snapshot: usize,

    /// State
    state: ExecutionState,

    /// Pending request
    pending_requests: HashSet<PendingRequest>,
    core: Arc<RwLock<ExecutionManagerCore>>,
}

// Data management
impl ExecutionManager {
    /// Create new execution manager with shared core
    pub async fn new(core: Arc<RwLock<ExecutionManagerCore>>) -> Self {
        let mut mgr = Self {
            state: core.clone().read().await.state.clone(),
            pending_requests: HashSet::new(),
            core,
            current_snapshot: 0,
            display_snapshot: 0,
        };

        let _ = mgr.goto_snapshot(0);
        let _ = mgr.display_snapshot(0);
        mgr
    }

    /// Push pending requests from manager to core
    pub async fn push_pending_to_core(&mut self) -> Result<()> {
        if !self.pending_requests.is_empty() {
            if let Ok(mut core) = self.core.try_write() {
                for request in self.pending_requests.drain() {
                    core.add_pending_request(request);
                }
            }
        }
        Ok(())
    }

    /// Pull processed data from core to manager
    pub fn pull_from_core(&mut self) -> Result<()> {
        if let Ok(core) = self.core.try_read() {
            self.state.update(&core.state);
        }
        Ok(())
    }

    /// Get clone of core for background processing
    pub fn get_core(&self) -> Arc<RwLock<ExecutionManagerCore>> {
        self.core.clone()
    }

    pub fn get_snapshot_info(&mut self, id: usize) -> Option<&SnapshotInfo> {
        let _ = self.pull_from_core();

        if !self.state.snapshot_info.contains_key(&id) {
            debug!("Snapshot info not found in cache, fetching...");
            self.new_fetching_request(PendingRequest::SnapshotInfo(id));
            return None;
        }

        match self.state.snapshot_info.get(&id) {
            Some(info) => info.as_ref(),
            _ => None,
        }
    }

    pub fn get_snapshot_count(&self) -> usize {
        self.state.snapshot_count
    }

    pub fn get_current_snapshot(&self) -> usize {
        self.current_snapshot
    }

    pub fn get_display_snapshot(&self) -> usize {
        self.display_snapshot
    }

    pub fn get_trace(&self) -> &Trace {
        &self.state.trace_data
    }

    pub fn get_code(&mut self, id: usize) -> Option<&Code> {
        let _ = self.pull_from_core();

        let Some(entry_id) =
            self.get_snapshot_info(id).map(|info| info.frame_id().trace_entry_id())
        else {
            debug!("Code not found in cache, fetching...");
            self.new_fetching_request(PendingRequest::Code(id));
            return None;
        };

        let Some(bytecode_address) = self.get_trace().get(entry_id).map(|e| e.code_address) else {
            debug!("Code not found in cache, fetching...");
            self.new_fetching_request(PendingRequest::Code(id));
            return None;
        };

        if !self.state.code.contains_key(&bytecode_address) {
            debug!("Code not found in cache, fetching...");
            self.new_fetching_request(PendingRequest::Code(id));
            return None;
        }

        match self.state.code.get(&bytecode_address) {
            Some(code) => code.as_ref(),
            _ => None,
        }
    }

    fn new_fetching_request(&mut self, request: PendingRequest) {
        if let Ok(mut core) = self.core.try_write() {
            core.add_pending_request(request);
        } else {
            self.pending_requests.insert(request);
        }
    }
}

// Execution-related functions
impl ExecutionManager {
    pub fn display_snapshot(&mut self, id: usize) -> Result<()> {
        if id >= self.state.snapshot_count {
            Err(eyre::eyre!(
                "Snapshot id {} out of bounds (total {})",
                id,
                self.state.snapshot_count
            ))
        } else {
            self.get_snapshot_info(id);
            self.get_code(id);

            self.display_snapshot = id;

            Ok(())
        }
    }

    pub fn goto_snapshot(&mut self, id: usize) -> Result<()> {
        if id >= self.state.snapshot_count {
            Err(eyre::eyre!(
                "Snapshot id {} out of bounds (total {})",
                id,
                self.state.snapshot_count
            ))
        } else {
            self.get_snapshot_info(id);
            self.get_code(id);

            // Use try_write instead of blocking write
            self.current_snapshot = id;

            Ok(())
        }
    }

    pub fn step(&mut self) -> Result<()> {
        let next_id = if self.current_snapshot + 1 < self.state.snapshot_count {
            self.current_snapshot + 1
        } else {
            self.current_snapshot
        };
        let _ = self.goto_snapshot(next_id);
        let _ = self.display_snapshot(next_id);

        Ok(())
    }

    pub fn reverse_step(&mut self) -> Result<()> {
        let prev_id = if self.current_snapshot > 0 { self.current_snapshot - 1 } else { 0 };
        let _ = self.goto_snapshot(prev_id);
        let _ = self.display_snapshot(prev_id);

        Ok(())
    }
}

impl Deref for ExecutionManager {
    type Target = ExecutionState;

    fn deref(&self) -> &Self::Target {
        &self.state
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

    /// State
    pub(super) state: ExecutionState,

    /// Pending requests to be processed
    pending_requests: HashSet<PendingRequest>,
}

impl ExecutionManagerCore {
    /// Create a new execution manager core with RPC client
    pub async fn new(rpc_client: Arc<RpcClient>) -> Result<Self> {
        Ok(Self {
            state: ExecutionState::with_rpc_client(rpc_client.clone()).await?,
            rpc_client,
            pending_requests: HashSet::new(),
        })
    }

    /// Add a pending request to be processed
    fn add_pending_request(&mut self, request: PendingRequest) {
        self.pending_requests.insert(request);
    }

    /// Process all pending requests
    pub async fn process_pending_requests(&mut self) -> Result<()> {
        let requests = mem::take(&mut self.pending_requests);
        for request in requests {
            if let Err(e) = self.fetch_data(request).await {
                debug!("Error processing request: {}", e);
                // Continue processing other requests even if one fails
            }
        }
        Ok(())
    }

    /// Fetch data per request
    async fn fetch_data(&mut self, request: PendingRequest) -> Result<()> {
        match request {
            PendingRequest::SnapshotInfo(id) => {
                if self.state.snapshot_info.contains_key(&id) {
                    return Ok(());
                }

                let info = self.rpc_client.get_snapshot_info(id).await?;
                self.state.snapshot_info.insert(id, Some(info));
            }
            PendingRequest::Code(id) => {
                if self.state.snapshot_info.get(&id).is_none() {
                    let info = self.rpc_client.get_snapshot_info(id).await?;
                    self.state.snapshot_info.insert(id, Some(info));
                }

                let entry_id = self
                    .state
                    .snapshot_info
                    .get(&id)
                    .and_then(|info| info.as_ref().map(|i| i.frame_id()))
                    .ok_or_else(|| eyre::eyre!("Snapshot info for id {} not found", id))?
                    .trace_entry_id();
                let bytecode_address = self
                    .state
                    .trace_data
                    .get(entry_id)
                    .map(|e| e.code_address)
                    .ok_or_else(|| eyre::eyre!("Trace entry with id {} not found", entry_id))?;

                if self.state.code.contains_key(&bytecode_address) {
                    return Ok(());
                }

                let code = self.rpc_client.get_code(id).await?;
                self.state.code.insert(bytecode_address, Some(code));
            }
        }

        Ok(())
    }
}
