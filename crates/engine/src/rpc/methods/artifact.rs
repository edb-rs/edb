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

//! Code retrieval RPC method implementation

use std::collections::HashMap;
use std::sync::Arc;

use alloy_primitives::Address;
use edb_common::types::{Code, OpcodeInfo, SourceInfo};
use revm::{database::CacheDB, Database, DatabaseCommit, DatabaseRef};
use serde_json::Value;
use tracing::debug;

use crate::{utils::disasm::disassemble, EngineContext, Snapshot};

use super::super::types::RpcError;

/// Get code for a specific snapshot
///
/// This method returns either disassembled opcodes (for opcode snapshots)
/// or source code (for hook snapshots).
///
/// # Parameters
/// - `id`: The snapshot ID (0-indexed)
///
/// # Returns
/// - For opcode snapshots: Disassembled bytecode with PC mappings
/// - For hook snapshots: Source code files from the artifact
pub fn get_code<DB>(
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

    let code = match snapshot {
        Snapshot::Opcode(opcode_snapshot) => {
            // For opcode snapshots, return disassembled bytecode
            // Get the bytecode from the database
            let entry_id = frame_id.trace_entry_id();
            let entry = context.trace.get(entry_id).ok_or_else(|| RpcError {
                code: -32603,
                message: format!("Trace entry with id {} not found", entry_id),
                data: None,
            })?;
            let bytecode = entry.bytecode.as_ref().ok_or_else(|| RpcError {
                code: -32603,
                message: format!("No bytecode found for trace entry {}", entry_id),
                data: None,
            })?;

            let disasm_result = disassemble(&bytecode);

            let mut codes = HashMap::new();
            for instruction in disasm_result.instructions {
                let pc = instruction.pc as u64;
                let opcode_str = if instruction.is_push() && !instruction.push_data.is_empty() {
                    // Format PUSH instructions with their data
                    let data_hex = hex::encode(&instruction.push_data);
                    format!("{} 0x{}", instruction.opcode, data_hex)
                } else {
                    instruction.opcode.to_string()
                };
                codes.insert(pc, opcode_str);
            }

            Code::Opcode(OpcodeInfo { address: opcode_snapshot.address, codes })
        }
        Snapshot::Hook(hook_snapshot) => {
            // For hook snapshots, return source code from artifacts
            let address = hook_snapshot.address;

            // Get the artifact for this address
            let artifact = context.artifacts.get(&address).ok_or_else(|| RpcError {
                code: -32603,
                message: format!("No artifact found for address {}", address),
                data: None,
            })?;

            // Extract sources from the SolcInput
            let mut sources = HashMap::new();
            for (path, source) in &artifact.input.sources {
                sources.insert(path.clone(), source.content.to_string());
            }

            Code::Source(SourceInfo { address, sources })
        }
    };

    // Serialize the Code enum to JSON
    let json_value = serde_json::to_value(code).map_err(|e| RpcError {
        code: -32603,
        message: format!("Failed to serialize code: {}", e),
        data: None,
    })?;

    debug!("Retrieved code for snapshot {}", snapshot_id);
    Ok(json_value)
}

pub fn get_constructor_args<DB>(
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
            code: -32602,
            message: "Invalid params: expected [address]".to_string(),
            data: None,
        })?;

    let args =
        context.artifacts.get(&address).map(|artifact| artifact.meta.constructor_arguments.clone());

    let json_value = serde_json::to_value(args).map_err(|e| RpcError {
        code: -32603,
        message: format!("Failed to serialize ABI: {}", e),
        data: None,
    })?;

    debug!("Retrieved contract ABI for address {}", address);
    Ok(json_value)
}
