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

use std::{f64::consts::E, sync::Arc};

use alloy_primitives::Address;
use edb_common::types::{parse_callable_abi_info, CallableAbiInfo, ContractTy, EdbSolValue};
use revm::{database::CacheDB, Database, DatabaseCommit, DatabaseRef};
use serde_json::Value;
use tracing::debug;

use crate::{error_codes, eval, EngineContext, RpcError};

pub fn eval_on_snapshot<DB>(
    context: &Arc<EngineContext<DB>>,
    params: Option<Value>,
) -> Result<serde_json::Value, RpcError>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone + Send + Sync + 'static,
    <CacheDB<DB> as Database>::Error: Clone + Send + Sync,
    <DB as Database>::Error: Clone + Send + Sync,
{
    // Parse the snapshot ID as the first argument
    let snapshot_id = params
        .as_ref()
        .and_then(|p| p.as_array())
        .and_then(|arr| arr.first())
        .and_then(|v| v.as_u64())
        .ok_or_else(|| RpcError {
            code: error_codes::INVALID_PARAMS,
            message: "Invalid params: expected [snapshot_id, expr]".to_string(),
            data: None,
        })? as usize;

    // Parse the expression as the second argument
    let expr = params
        .as_ref()
        .and_then(|p| p.as_array())
        .and_then(|arr| arr.get(1))
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError {
            code: error_codes::INVALID_PARAMS,
            message: "Invalid params: expected [snapshot_id, expr]".to_string(),
            data: None,
        })?;

    let value: Result<EdbSolValue, String> =
        eval::eval_on_snapshot(context.clone(), expr, snapshot_id)
            .map(|v| v.into())
            .map_err(|e| e.to_string());

    let json_value = serde_json::to_value(value).map_err(|e| RpcError {
        code: error_codes::INTERNAL_ERROR,
        message: format!("Failed to serialize ABI: {}", e),
        data: None,
    })?;

    debug!("Evaluated expression '{}' on snapshot {}: {:?}", expr, snapshot_id, json_value);
    Ok(json_value)
}
