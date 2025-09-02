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

//! Navigation RPC methods
//!
//! This module implements RPC methods for navigating through snapshots.

use crate::rpc::types::{RpcError, SnapshotInfo};
use crate::{EngineContext, Snapshot};
use edb_common::types::ExecutionFrameId;
use edb_common::OpcodeTr;
use revm::bytecode::OpCode;
use revm::database::CacheDB;
use revm::{Database, DatabaseCommit, DatabaseRef};
use serde_json::Value;
use std::sync::Arc;
use tracing::debug;

pub fn get_next_call<DB>(
    context: &Arc<EngineContext<DB>>,
    params: Option<Value>,
) -> Result<Value, RpcError>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone + Send + Sync + 'static,
    <CacheDB<DB> as Database>::Error: Clone + Send + Sync,
    <DB as Database>::Error: Clone + Send + Sync,
{
    // Parse the snapshot ID from parameters
    let snapshot_id = params
        .as_ref()
        .and_then(|p| p.as_array())
        .and_then(|arr| arr.first())
        .and_then(|v| v.as_u64())
        .ok_or_else(|| RpcError {
            code: -32602,
            message: "Invalid params: expected [snapshot_id]".to_string(),
            data: None,
        })? as usize;

    // Get the snapshot at the specified index
    let _ = context.snapshots.get(snapshot_id).ok_or_else(|| RpcError {
        code: -32602,
        message: format!("Snapshot with id {} not found", snapshot_id),
        data: None,
    })?;

    let mut next_call = context.snapshots.len() - 1;
    for (s_id, (f_id, snapshot)) in context.snapshots.iter().enumerate().skip(snapshot_id + 1) {
        if snapshot_is_call(context, snapshot, f_id, s_id)? {
            next_call = s_id;
            break;
        }
    }

    // Serialize the SnapshotInfo enum to JSON
    let json_value = serde_json::to_value(next_call).map_err(|e| RpcError {
        code: -32603,
        message: format!("Failed to serialize snapshot info: {}", e),
        data: None,
    })?;

    debug!("Retrieved snapshot info for snapshot {}", snapshot_id);
    Ok(json_value)
}

// Helper function
fn snapshot_is_call<DB>(
    context: &Arc<EngineContext<DB>>,
    snapshot: &Snapshot<DB>,
    f_id: &ExecutionFrameId,
    s_id: usize,
) -> Result<bool, RpcError>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone + Send + Sync + 'static,
    <CacheDB<DB> as Database>::Error: Clone + Send + Sync,
    <DB as Database>::Error: Clone + Send + Sync,
{
    match snapshot {
        Snapshot::<DB>::Opcode(s) => {
            let op = unsafe { OpCode::new_unchecked(s.opcode) };
            Ok(op.is_call())
        }
        Snapshot::<DB>::Hook(s) => {
            let usid = s.usid;
            let address = context.get_bytecode_address(s_id).ok_or_else(|| RpcError {
                code: -32603,
                message: format!("Trace entry with id {} not found", f_id.trace_entry_id()),
                data: None,
            })?;

            let analysis_result =
                context.analysis_results.get(&address).ok_or_else(|| RpcError {
                    code: -32603,
                    message: format!("Analysis result for address {} not found", address),
                    data: None,
                })?;

            let step_info = analysis_result.usid_to_step.get(&usid).ok_or_else(|| RpcError {
                code: -32603,
                message: format!("Step info for USID {} not found", usid),
                data: None,
            })?;

            Ok(step_info.function_calls() > 0)
        }
    }
}
