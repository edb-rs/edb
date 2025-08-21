//! Navigation RPC methods
//!
//! This module implements RPC methods for navigating through snapshots
//! and controlling the debugger's current position.

use crate::core::EngineContext;
use crate::rpc::types::{RpcError, SnapshotInfo, SnapshotType};
use revm::database::CacheDB;
use revm::{Database, DatabaseCommit, DatabaseRef};
use std::sync::Arc;
use tracing::debug;

/// Get information about a snapshot at the given index
pub async fn get_current_snapshot<DB>(
    context: &Arc<EngineContext<DB>>,
    index: usize,
) -> Result<serde_json::Value, RpcError>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone + Send + Sync + 'static,
    <CacheDB<DB> as Database>::Error: Clone + Send + Sync,
    <DB as Database>::Error: Clone + Send + Sync,
{
    let total = context.snapshots.total_snapshot_count();

    if index >= total {
        return Err(RpcError {
            code: -33001, // SNAPSHOT_OUT_OF_BOUNDS
            message: format!("Snapshot index {} out of bounds (total: {})", index, total),
            data: None,
        });
    }

    // Get snapshot at current index
    if let Some((frame_id, snapshot)) = context.snapshots.get(index) {
        let snapshot_info = match snapshot {
            crate::Snapshot::Opcode(opcode_snapshot) => SnapshotInfo {
                index,
                total,
                snapshot_type: SnapshotType::Opcode,
                frame_id: crate::rpc::types::ExecutionFrameId {
                    trace_entry_id: frame_id.trace_entry_id(),
                    re_entry_count: frame_id.re_entry_count(),
                },
                address: opcode_snapshot.address,
                pc: Some(opcode_snapshot.pc),
                opcode: Some(opcode_snapshot.opcode.to_string()),
                usid: None,
            },
            crate::Snapshot::Hook(hook_snapshot) => SnapshotInfo {
                index,
                total,
                snapshot_type: SnapshotType::Hook,
                frame_id: crate::rpc::types::ExecutionFrameId {
                    trace_entry_id: frame_id.trace_entry_id(),
                    re_entry_count: frame_id.re_entry_count(),
                },
                address: hook_snapshot.address,
                pc: None,
                opcode: None,
                usid: Some(hook_snapshot.usid.into()),
            },
        };

        Ok(serde_json::to_value(snapshot_info).map_err(|e| RpcError {
            code: -32603, // INTERNAL_ERROR
            message: format!("Failed to serialize snapshot info: {}", e),
            data: None,
        })?)
    } else {
        Err(RpcError {
            code: -33001, // SNAPSHOT_OUT_OF_BOUNDS
            message: format!("No snapshot found at index {}", index),
            data: None,
        })
    }
}

/// Get the total number of snapshots
pub async fn get_snapshot_count<DB>(
    context: &Arc<EngineContext<DB>>,
) -> Result<serde_json::Value, RpcError>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone + Send + Sync + 'static,
    <CacheDB<DB> as Database>::Error: Clone + Send + Sync,
    <DB as Database>::Error: Clone + Send + Sync,
{
    let count = context.snapshots.total_snapshot_count();
    Ok(serde_json::json!(count))
}

/// Calculate the next snapshot index and return snapshot info
pub async fn step_next<DB>(
    context: &Arc<EngineContext<DB>>,
    current_index: usize,
) -> Result<serde_json::Value, RpcError>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone + Send + Sync + 'static,
    <CacheDB<DB> as Database>::Error: Clone + Send + Sync,
    <DB as Database>::Error: Clone + Send + Sync,
{
    let total = context.snapshots.total_snapshot_count();

    if current_index + 1 >= total {
        return Err(RpcError {
            code: -33001, // SNAPSHOT_OUT_OF_BOUNDS
            message: "Already at the last snapshot".to_string(),
            data: None,
        });
    }

    let new_index = current_index + 1;
    debug!("Stepping to next snapshot: {}", new_index);

    // Return snapshot info for the new index
    get_current_snapshot(context, new_index).await
}

/// Calculate the previous snapshot index and return snapshot info
pub async fn step_previous<DB>(
    context: &Arc<EngineContext<DB>>,
    current_index: usize,
) -> Result<serde_json::Value, RpcError>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone + Send + Sync + 'static,
    <CacheDB<DB> as Database>::Error: Clone + Send + Sync,
    <DB as Database>::Error: Clone + Send + Sync,
{
    if current_index == 0 {
        return Err(RpcError {
            code: -33001, // SNAPSHOT_OUT_OF_BOUNDS
            message: "Already at the first snapshot".to_string(),
            data: None,
        });
    }

    let new_index = current_index - 1;
    debug!("Stepping to previous snapshot: {}", new_index);

    // Return snapshot info for the new index
    get_current_snapshot(context, new_index).await
}

/// Validate and return snapshot info for a specific index
pub async fn set_current_snapshot<DB>(
    context: &Arc<EngineContext<DB>>,
    index: usize,
) -> Result<serde_json::Value, RpcError>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone + Send + Sync + 'static,
    <CacheDB<DB> as Database>::Error: Clone + Send + Sync,
    <DB as Database>::Error: Clone + Send + Sync,
{
    let total = context.snapshots.total_snapshot_count();

    if index >= total {
        return Err(RpcError {
            code: -33001, // SNAPSHOT_OUT_OF_BOUNDS
            message: format!("Snapshot index {} out of bounds (total: {})", index, total),
            data: None,
        });
    }

    debug!("Validating snapshot index: {}", index);

    // Return snapshot info for the requested index
    get_current_snapshot(context, index).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_navigation_methods() {
        // This test would need a mock EngineContext with snapshots
        // For now, just test that the module compiles
    }
}
