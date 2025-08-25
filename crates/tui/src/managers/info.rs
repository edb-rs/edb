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

//! Information management for TUI panels
//!
//! This module implements a two-tier architecture for info management:
//!
//! - `InfoManager`: Per-thread instance for immediate data access during rendering
//! - `InfoManagerCore`: Shared core that handles RPC communication and data fetching
//!
//! This design ensures rendering threads never block on network I/O while
//! maintaining consistency across the application.

use crate::rpc::RpcClient;
use edb_common::types::Trace;
use eyre::Result;
use std::{
    ops::{Deref, DerefMut},
    sync::{Arc, RwLock},
};

/// Per-thread info manager providing cached data for rendering
///
/// # Design Philosophy
///
/// `InfoManager` follows the same pattern as ThemeManager:
/// - All data reads are immediate and non-blocking
/// - Complex RPC operations are delegated to InfoManagerCore
/// - Data synchronization happens explicitly via `fetch_data()`
///
/// # Usage Pattern
///
/// ```ignore
/// // When data updates are needed
/// info_manager.fetch_data().await?; // Sync with core
///
/// // During rendering - immediate access to cached data
/// // (future: add cached fields here for immediate reads)
/// ```
#[derive(Debug, Clone)]
pub struct InfoManager {
    // TODO: Add cached fields here for immediate read access
    // pub system_info: SystemInfo,
    // pub network_status: NetworkStatus,
    core: Arc<RwLock<InfoManagerCore>>,
}

impl Deref for InfoManager {
    type Target = Arc<RwLock<InfoManagerCore>>;

    fn deref(&self) -> &Self::Target {
        &self.core
    }
}

impl DerefMut for InfoManager {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.core
    }
}

impl InfoManager {
    /// Create a new info manager with a shared core
    pub fn new(core: Arc<RwLock<InfoManagerCore>>) -> Self {
        Self { core }
    }

    /// Synchronize local cache with the shared core
    ///
    /// This is the only async operation in InfoManager, designed to:
    /// - Trigger data fetching in InfoManagerCore
    /// - Update local caches when data is available
    /// - Be called periodically or when fresh data is needed
    pub async fn fetch_data(&mut self) -> Result<()> {
        let mut core = self.core.write().unwrap();
        core.fetch_data().await?;
        
        // TODO: Update local cached fields here
        // self.system_info = core.get_system_info();
        // self.network_status = core.get_network_status();

        Ok(())
    }
}

/// Centralized info state manager handling RPC communication and data fetching
///
/// # Design Philosophy
///
/// `InfoManagerCore` is responsible for:
/// - All RPC communication with the debug server
/// - Complex data fetching and processing
/// - Caching fetched data for distribution to InfoManager instances
/// - Thread-safe state updates via `Arc<RwLock<>>`
///
/// All network I/O and complex operations happen here, keeping
/// InfoManager instances lightweight for rendering.
///
/// # Architecture Benefits
///
/// - **Non-blocking UI**: Rendering threads never wait on RPC calls
/// - **Centralized I/O**: All network operations in one place
/// - **Consistent State**: Single source of truth for fetched data
/// - **Resource Efficiency**: Shared RPC client and cached data
#[derive(Debug, Clone)]
pub struct InfoManagerCore {
    /// RPC client for server communication
    rpc_client: Arc<RpcClient>,
    // TODO: Add cached data fields here
    // system_info: Option<SystemInfo>,
    // network_status: Option<NetworkStatus>,
}

impl InfoManagerCore {
    /// Create a new info manager core with RPC client
    pub fn new(rpc_client: Arc<RpcClient>) -> Self {
        Self { rpc_client }
    }

    /// Fetch latest data from the debug server
    ///
    /// This method handles all RPC communication and updates
    /// internal caches that InfoManager instances can read
    pub async fn fetch_data(&mut self) -> Result<()> {
        // TODO: Implement actual RPC calls here
        // self.system_info = Some(self.rpc_client.get_system_info().await?);
        // self.network_status = Some(self.rpc_client.get_network_status().await?);
        Ok(())
    }
    
    // TODO: Add getter methods for cached data
    // pub fn get_system_info(&self) -> SystemInfo { ... }
    // pub fn get_network_status(&self) -> NetworkStatus { ... }
}
