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
use std::{
    collections::HashSet,
    ops::{Deref, DerefMut},
    sync::{Arc, RwLock},
};
use tracing::debug;

use edb_common::types::{Code, SnapshotInfo, Trace};

use crate::{managers::FetchCache, RpcClient};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum PendingRequest {
    Trace(),
    SnapshotInfo(usize),
    Code(usize, Address),
}

#[derive(Debug, Default, Clone)]
struct ExecutionState {
    current_snapshot: usize,
    display_snapshot: usize,
    snapshot_count: usize,
    snapshot_info: FetchCache<usize, SnapshotInfo>,
    code: FetchCache<Address, Code>,
    trace_data: Option<Trace>,
}

impl ExecutionState {
    fn update(&mut self, other: &ExecutionState) {
        if self.trace_data.is_none() && other.trace_data.is_some() {
            self.trace_data = other.trace_data.clone();
        }

        if self.snapshot_count != other.snapshot_count {
            self.snapshot_count = other.snapshot_count;
        }

        if self.current_snapshot != other.current_snapshot {
            self.current_snapshot = other.current_snapshot;
        }

        if self.display_snapshot != other.display_snapshot {
            self.display_snapshot = other.display_snapshot;
        }

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
    /// State
    state: ExecutionState,

    /// Pending request
    pending_request: HashSet<PendingRequest>,

    core: Arc<RwLock<ExecutionManagerCore>>,
}

impl ExecutionManager {
    /// Create new execution manager with shared core
    pub fn new(core: Arc<RwLock<ExecutionManagerCore>>) -> Self {
        let mut mgr =
            Self { state: ExecutionState::default(), pending_request: HashSet::new(), core };

        let _ = mgr.goto_snapshot(0);
        let _ = mgr.display_snapshot(0);
        mgr
    }

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

            let mut core = self.core.write().unwrap();
            self.state.display_snapshot = id;
            core.state.display_snapshot = id;

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

            let mut core = self.core.write().unwrap();
            self.state.current_snapshot = id;
            core.state.current_snapshot = id;

            Ok(())
        }
    }

    pub fn get_snapshot_info(&mut self, id: usize) -> Option<&SnapshotInfo> {
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
        self.state.current_snapshot
    }

    pub fn get_display_snapshot(&self) -> usize {
        self.state.display_snapshot
    }

    pub fn get_trace_ref(&self) -> Option<&Trace> {
        self.state.trace_data.as_ref()
    }

    pub fn get_trace(&mut self) -> Option<&Trace> {
        if self.state.trace_data.is_none() {
            debug!("Trace not found in cache, fetching...");
            self.new_fetching_request(PendingRequest::Trace());
            return None;
        }

        self.state.trace_data.as_ref()
    }

    pub fn get_code(&mut self, id: usize) -> Option<&Code> {
        let info = self.get_snapshot_info(id)?;
        let address = info.address();
        if !self.state.code.contains_key(&address) {
            debug!("Code not found in cache, fetching...");
            self.new_fetching_request(PendingRequest::Code(id, address));
            return None;
        }

        match self.state.code.get(&address) {
            Some(code) => code.as_ref(),
            _ => None,
        }
    }

    fn new_fetching_request(&mut self, request: PendingRequest) {
        self.pending_request.insert(request);
    }

    /// Synchronize local execution state with the shared core
    ///
    /// This is the only async operation, designed to:
    /// - Fetch latest trace data from ExecutionManagerCore
    /// - Update all cached fields for immediate access
    /// - Be called when execution state changes or on initialization
    pub async fn fetch_data(&mut self) -> eyre::Result<()> {
        let mut core = self.core.write().unwrap();

        for request in self.pending_request.drain() {
            core.fetch_data(request).await?;
        }

        // Update snapshot count
        if self.state.snapshot_count == 0 {
            core.fetch_snapshot_count().await?;
        }

        self.state.update(&core.state);

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

    /// State
    state: ExecutionState,
}

impl ExecutionManagerCore {
    /// Create a new execution manager core with RPC client
    pub fn new(rpc_client: Arc<RpcClient>) -> Self {
        Self { rpc_client, state: ExecutionState::default() }
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
            PendingRequest::Code(id, address) => {
                if self.state.code.contains_key(&address) {
                    return Ok(());
                }

                let code = self.rpc_client.get_code(id).await?;
                self.state.code.insert(address, Some(code));
            }
            PendingRequest::Trace() => {
                if self.state.trace_data.is_some() {
                    return Ok(());
                }

                let trace = self.rpc_client.get_trace().await?;
                self.state.trace_data = Some(trace);
            }
        }

        Ok(())
    }

    /// Fetch snapshot count
    async fn fetch_snapshot_count(&mut self) -> Result<()> {
        if self.state.snapshot_count > 0 {
            // We already have the snapshot count, no need to fetch
            return Ok(());
        }

        match self.rpc_client.get_snapshot_count().await {
            Ok(count) => {
                self.state.snapshot_count = count;
                Ok(())
            }
            Err(e) => {
                tracing::warn!("Failed to fetch snapshot count: {}", e);
                Err(e)
            }
        }
    }

    /// Fetch trace data from debug server
    async fn fetch_trace(&mut self) -> Result<()> {
        if self.state.trace_data.is_some() {
            // We already have trace data, no need to fetch
            return Ok(());
        }

        match self.rpc_client.get_trace().await {
            Ok(trace_value) => {
                self.state.trace_data = Some(trace_value);
                Ok(())
            }
            Err(e) => {
                tracing::warn!("Failed to fetch trace: {}", e);
                Err(e)
            }
        }
    }
}
