use std::sync::Arc;

use alloy_primitives::{
    map::foldhash::{HashMap, HashMapExt},
    U256,
};
use revm::{
    database::{empty_db, CacheDB},
    Database, DatabaseCommit, DatabaseRef,
};
use serde_json::Value;
use tracing::debug;

use crate::{error_codes, methods::snapshot, EngineContext, RpcError};

pub fn get_storage_diff<DB>(
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
            code: error_codes::INVALID_PARAMS,
            message: "Invalid params: expected [snapshot_id, slot]".to_string(),
            data: None,
        })? as usize;

    let (f_id, snapshot) = context.snapshots.get(snapshot_id).ok_or_else(|| RpcError {
        code: error_codes::SNAPSHOT_OUT_OF_BOUNDS,
        message: format!("Snapshot with id {} not found", snapshot_id),
        data: None,
    })?;

    let target_address = context
        .trace
        .get(f_id.trace_entry_id())
        .ok_or_else(|| RpcError {
            code: error_codes::INTERNAL_ERROR,
            message: format!("Execution frame id {} not found in trace", f_id),
            data: None,
        })?
        .target;

    let empty_storage = HashMap::new();
    let dst_db = snapshot.db();
    let dst_cached_storage = dst_db
        .cache
        .accounts
        .get(&target_address)
        .map(|acc| &acc.storage)
        .unwrap_or(&empty_storage);

    let src_db = context
        .snapshots
        .get(0)
        .ok_or_else(|| RpcError {
            code: error_codes::SNAPSHOT_OUT_OF_BOUNDS,
            message: "Initial snapshot (id 0) not found".to_string(),
            data: None,
        })?
        .1
        .db();

    let mut changes = HashMap::new();
    for (slot, dst_value) in dst_cached_storage.iter() {
        let src_value = src_db.storage_ref(target_address, *slot).map_err(|e| RpcError {
            code: error_codes::INTERNAL_ERROR,
            message: format!(
                "Failed to retrieve storage at {} for slot {}: {}",
                target_address, slot, e
            ),
            data: None,
        })?;
        if &src_value != dst_value {
            changes.insert(*slot, (src_value, *dst_value));
        }
    }

    // Serialize the SnapshotInfo enum to JSON
    let json_value = serde_json::to_value(changes).map_err(|e| RpcError {
        code: error_codes::INTERNAL_ERROR,
        message: format!("Failed to serialize snapshot info: {}", e),
        data: None,
    })?;

    debug!("Retrieved snapshot info for snapshot {}", snapshot_id);
    Ok(json_value)
}

pub fn get_storage<DB>(
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
            code: error_codes::INVALID_PARAMS,
            message: "Invalid params: expected [snapshot_id, slot]".to_string(),
            data: None,
        })? as usize;

    // Parse recompiled as the second argument
    let slot: U256 = params
        .as_ref()
        .and_then(|p| p.as_array())
        .and_then(|arr| arr.get(1))
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .ok_or_else(|| RpcError {
            code: error_codes::INVALID_PARAMS,
            message: "Invalid params: expected [snapshot_id, slot]".to_string(),
            data: None,
        })?;

    let (f_id, snapshot) = context.snapshots.get(snapshot_id).ok_or_else(|| RpcError {
        code: error_codes::SNAPSHOT_OUT_OF_BOUNDS,
        message: format!("Snapshot with id {} not found", snapshot_id),
        data: None,
    })?;

    let target_address = context
        .trace
        .get(f_id.trace_entry_id())
        .ok_or_else(|| RpcError {
            code: error_codes::INTERNAL_ERROR,
            message: format!("Execution frame id {} not found in trace", f_id),
            data: None,
        })?
        .target;

    let db = snapshot.db();
    let value = db.storage_ref(target_address, slot).map_err(|e| RpcError {
        code: error_codes::INTERNAL_ERROR,
        message: format!(
            "Failed to retrieve storage at {} for slot {}: {}",
            target_address, slot, e
        ),
        data: None,
    })?;

    // Serialize the SnapshotInfo enum to JSON
    let json_value = serde_json::to_value(value).map_err(|e| RpcError {
        code: error_codes::INTERNAL_ERROR,
        message: format!("Failed to serialize snapshot info: {}", e),
        data: None,
    })?;

    debug!("Retrieved snapshot info for snapshot {}", snapshot_id);
    Ok(json_value)
}
