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

use std::sync::Arc;

use alloy_primitives::Address;
use revm::{database::CacheDB, Database, DatabaseCommit, DatabaseRef};
use serde_json::Value;
use tracing::debug;

use crate::{error_codes, EngineContext, RpcError};

pub fn get_contract_abi<DB>(
    context: &Arc<EngineContext<DB>>,
    params: Option<Value>,
) -> Result<serde_json::Value, RpcError>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone + Send + Sync + 'static,
    <CacheDB<DB> as Database>::Error: Clone + Send + Sync,
    <DB as Database>::Error: Clone + Send + Sync,
{
    // Parse the address as the first argument
    let address: Address = params
        .as_ref()
        .and_then(|p| p.as_array())
        .and_then(|arr| arr.first())
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .ok_or_else(|| RpcError {
            code: error_codes::INVALID_PARAMS,
            message: "Invalid params: expected [address, recompiled]".to_string(),
            data: None,
        })?;

    // Parse recompiled as the second argument
    let recompiled = params
        .as_ref()
        .and_then(|p| p.as_array())
        .and_then(|arr| arr.get(1))
        .and_then(|v| v.as_bool())
        .ok_or_else(|| RpcError {
            code: error_codes::INVALID_PARAMS,
            message: "Invalid params: expected [address, recompiled]".to_string(),
            data: None,
        })?;

    let abi = if recompiled {
        context
            .recompiled_artifacts
            .get(&address)
            .and_then(|artifact| artifact.contract())
            .and_then(|contract| contract.abi.as_ref())
            .cloned()
    } else {
        context
            .artifacts
            .get(&address)
            .and_then(|artifact| artifact.contract())
            .and_then(|contract| contract.abi.as_ref())
            .cloned()
    };

    let json_value = serde_json::to_value(abi).map_err(|e| RpcError {
        code: error_codes::INTERNAL_ERROR,
        message: format!("Failed to serialize ABI: {}", e),
        data: None,
    })?;

    debug!("Retrieved contract ABI for address {}", address);
    Ok(json_value)
}
