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

//! RPC method handlers
//!
//! This module contains all the RPC method implementations for the debug server.

mod code;
mod snapshot;
mod trace;

use super::types::RpcError;
use crate::EngineContext;
use revm::{database::CacheDB, Database, DatabaseCommit, DatabaseRef};
use std::sync::Arc;
use tracing::debug;

/// Method handler for dispatching RPC calls (stateless)
pub struct MethodHandler<DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone + Send + Sync + 'static,
    <CacheDB<DB> as Database>::Error: Clone + Send + Sync,
    <DB as Database>::Error: Clone + Send + Sync,
{
    /// Immutable debugging context
    context: Arc<EngineContext<DB>>,
}

impl<DB> MethodHandler<DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone + Send + Sync + 'static,
    <CacheDB<DB> as Database>::Error: Clone + Send + Sync,
    <DB as Database>::Error: Clone + Send + Sync,
{
    pub fn new(context: Arc<EngineContext<DB>>) -> Self {
        Self { context }
    }

    /// Handle an RPC method call with client-provided state
    pub async fn handle_method(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, RpcError> {
        debug!("Handling RPC method: {}", method);

        match method {
            "edb_getTrace" => trace::get_trace(&self.context),
            "edb_getCode" => code::get_code(&self.context, params),
            "edb_getSnapshotInfo" => snapshot::get_snapshot_info(&self.context, params),
            // Unimplemented methods
            _ => Err(RpcError {
                code: -32601,
                message: format!("Method '{}' not found", method),
                data: None,
            }),
        }
    }
}
