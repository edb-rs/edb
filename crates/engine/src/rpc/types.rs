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

//! RPC data types and serialization structures
//!
//! This module defines all the data structures used for JSON-RPC communication
//! between the TUI and the RPC server.

use alloy_primitives::Address;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// RPC request structure
#[derive(Debug, Deserialize)]
pub struct RpcRequest {
    pub jsonrpc: String,
    pub method: String,
    pub params: Option<serde_json::Value>,
    pub id: RpcId,
}

/// RPC response structure  
#[derive(Debug, Serialize)]
pub struct RpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
    pub id: RpcId,
}

/// RPC error structure
#[derive(Debug, Serialize)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// RPC ID can be string or number
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RpcId {
    Number(u64),
    String(String),
}

/// Breakpoint information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Breakpoint {
    pub id: BreakpointId,
    pub location: BreakpointLocation,
    pub enabled: bool,
    pub condition: Option<String>,
}

/// Breakpoint identifier
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct BreakpointId(pub String);

/// Breakpoint location
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreakpointLocation {
    pub address: Address,
    pub location_type: LocationType,
    pub line: Option<usize>,
    pub pc: Option<usize>,
    pub path: Option<String>,
}

/// Location type for breakpoints
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocationType {
    SourceLine,
    ProgramCounter,
}

/// Standard RPC error codes
pub mod error_codes {
    pub const PARSE_ERROR: i32 = -32700;
    pub const INVALID_REQUEST: i32 = -32600;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
    pub const INTERNAL_ERROR: i32 = -32603;

    // Custom error codes
    pub const SNAPSHOT_OUT_OF_BOUNDS: i32 = -33001;
    pub const INVALID_ADDRESS: i32 = -33002;
    pub const TRACE_ENTRY_NOT_FOUND: i32 = -33003;
    pub const CODE_NOT_FOUND: i32 = -33004;
    pub const USID_NOT_FOUND: i32 = -33005;
    pub const EVAL_FAILED: i32 = -33006;
}
