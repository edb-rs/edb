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
use edb_common::types::Trace;
use eyre::Result;
use std::{
    ops::{Deref, DerefMut},
    sync::{Arc, RwLock},
};

#[derive(Debug, Clone)]
pub struct InfoManager {
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
    pub fn new(core: Arc<RwLock<InfoManagerCore>>) -> Self {
        Self { core }
    }

    pub async fn fetch_data(&mut self) -> Result<()> {
        let mut core = self.core.write().unwrap();
        core.fetch_data().await?;

        Ok(())
    }
}

/// Resource manager for handling trace data and other shared resources
#[derive(Debug, Clone)]
pub struct InfoManagerCore {
    /// RPC client for server communication
    rpc_client: Arc<RpcClient>,
}

impl InfoManagerCore {
    /// Create a new resource manager
    pub fn new(rpc_client: Arc<RpcClient>) -> Self {
        Self { rpc_client }
    }

    pub async fn fetch_data(&mut self) -> Result<()> {
        Ok(())
    }
}
