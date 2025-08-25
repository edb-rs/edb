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

//! Snapshot information RPC method implementation

use std::sync::Arc;

use edb_common::types::{HookSnapshotInfo, OpcodeSnapshotInfo, SnapshotInfo};
use revm::{database::CacheDB, Database, DatabaseCommit, DatabaseRef};
use serde_json::Value;
use tracing::debug;

use crate::{EngineContext, Snapshot};

use super::super::types::RpcError;

/// Get detailed information for a specific snapshot
///
/// This method returns structured information about a snapshot, including:
/// - For opcode snapshots: PC, opcode, memory, stack, calldata, and storage state
/// - For hook snapshots: Source location information (path, offset, length)
///
/// # Parameters
/// - `id`: The snapshot ID (0-indexed)
///
/// # Returns
/// - For opcode snapshots: Complete execution state at that opcode
/// - For hook snapshots: Source code location and debugging info
pub fn get_snapshot_info<DB>(
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
    let (frame_id, snapshot) = context.snapshots.get(snapshot_id).ok_or_else(|| RpcError {
        code: -32602,
        message: format!("Snapshot with id {} not found", snapshot_id),
        data: None,
    })?;

    let snapshot_info = match snapshot {
        Snapshot::Opcode(opcode_snapshot) => {
            // For opcode snapshots, return complete execution state
            SnapshotInfo::Opcode(OpcodeSnapshotInfo {
                address: opcode_snapshot.address,
                pc: opcode_snapshot.pc,
                opcode: opcode_snapshot.opcode,
                memory: opcode_snapshot.memory.as_ref().clone(),
                stack: opcode_snapshot.stack.clone(),
                calldata: opcode_snapshot.calldata.as_ref().clone(),
                transition_storage: opcode_snapshot.transition_storage.as_ref().clone(),
            })
        }
        Snapshot::Hook(hook_snapshot) => {
            // For hook snapshots, get source location from analysis results
            let address = hook_snapshot.address;
            let usid = hook_snapshot.usid;

            // Get the analysis result for this address
            let analysis_result =
                context.analysis_results.get(&address).ok_or_else(|| RpcError {
                    code: -32603,
                    message: format!("No analysis result found for address {}", address),
                    data: None,
                })?;

            // Get the step from USID
            let step_ref = analysis_result.usid_to_step.get(&usid).ok_or_else(|| RpcError {
                code: -32603,
                message: format!("No step found for USID {}", u64::from(usid)),
                data: None,
            })?;

            // Get step details
            let step = step_ref.read();
            let source_location = &step.src;

            // Find the source analysis using the source index
            let source_index = source_location.index.unwrap_or(0) as u32;
            let source_analysis =
                analysis_result.sources.get(&source_index).ok_or_else(|| RpcError {
                    code: -32603,
                    message: format!("No source analysis found for index {}", source_index),
                    data: None,
                })?;

            SnapshotInfo::Hook(HookSnapshotInfo {
                address,
                path: source_analysis.path.clone(),
                offset: source_location.start.unwrap_or(0),
                length: source_location.length.unwrap_or(0),
            })
        }
    };

    // Serialize the SnapshotInfo enum to JSON
    let json_value = serde_json::to_value(snapshot_info).map_err(|e| RpcError {
        code: -32603,
        message: format!("Failed to serialize snapshot info: {}", e),
        data: None,
    })?;

    debug!("Retrieved snapshot info for snapshot {}", snapshot_id);
    Ok(json_value)
}
