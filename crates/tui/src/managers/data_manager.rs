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

//! Unified data manager for TUI
//!
//! The `DataManager` is the central hub for all state management in the TUI.
//! It contains all managers and coordinates data flow between them.
//!
//! # Usage
//!
//! ```ignore
//! let mut data_manager = DataManager::new(rpc_client).await?;
//!
//! // In render functions - direct read access
//! let trace = data_manager.execution.get_trace();
//! let theme = &data_manager.theme;
//!
//! // In update loop - coordinate data flow
//! data_manager.update_pending_requests().await?;  // Push to cores
//! data_manager.process_core_updates()?;           // Pull from cores
//! ```
//!
//! # Design Rationale
//!
//! Having a single DataManager instance passed as a mutable reference ensures:
//! - No duplicate manager instances across panels
//! - Consistent state across the entire application
//! - Clear data flow and update patterns
//! - Easy access to all managers from any component

use eyre::Result;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::{
    execution::{ExecutionManager, ExecutionManagerCore},
    resolve::{Resolver, ResolverCore},
    Theme,
};
use crate::rpc::RpcClient;

/// Central data manager containing all state managers
///
/// This structure is passed as a mutable reference to all app functions,
/// providing centralized access to execution state, resolver, and theme.
pub struct DataManager {
    /// Execution state manager for trace and snapshot data
    pub execution: ExecutionManager,
    /// Resolver for contract ABIs and labels
    pub resolver: Resolver,
    /// Theme configuration (no Arc/RwLock needed)
    pub theme: Theme,
}

impl DataManager {
    /// Create a new DataManager with all managers initialized
    pub async fn new(rpc_client: Arc<RpcClient>) -> Result<Self> {
        // Initialize cores with tokio::sync::RwLock
        let exec_core = Arc::new(RwLock::new(ExecutionManagerCore::new(rpc_client.clone()).await?));
        let resolver_core = Arc::new(RwLock::new(ResolverCore::new(rpc_client.clone()).await?));

        Ok(Self {
            execution: ExecutionManager::new(exec_core).await,
            resolver: Resolver::new(resolver_core).await,
            theme: Theme::default(),
        })
    }

    /// Push all pending requests from managers to their cores
    ///
    /// This should be called in app.update() to queue requests for processing
    pub async fn update_pending_requests(&mut self) -> Result<()> {
        self.execution.push_pending_to_core().await?;
        self.resolver.push_pending_to_core().await?;
        Ok(())
    }

    /// Pull processed data from cores back to managers
    ///
    /// This updates the cached state in managers with data processed by cores
    pub fn process_core_updates(&mut self) -> Result<()> {
        self.execution.pull_from_core()?;
        self.resolver.pull_from_core()?;
        Ok(())
    }

    /// Get clone of execution core for background processing
    pub fn get_execution_core(&self) -> Arc<RwLock<ExecutionManagerCore>> {
        self.execution.get_core()
    }

    /// Get clone of resolver core for background processing
    pub fn get_resolver_core(&self) -> Arc<RwLock<ResolverCore>> {
        self.resolver.get_core()
    }
}
