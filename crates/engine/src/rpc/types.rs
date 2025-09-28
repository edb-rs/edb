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

//! JSON-RPC protocol types and data structures.
//!
//! This module defines all the data structures used for JSON-RPC communication
//! between debugging clients (frontends, IDEs, etc.) and the EDB RPC server.
//! All types implement the JSON-RPC 2.0 specification for consistent protocol handling.
//!
//! # Protocol Types
//!
//! - [`RpcRequest`] - Incoming JSON-RPC request with method and parameters
//! - [`RpcResponse`] - Outgoing JSON-RPC response with result or error
//! - [`RpcError`] - Structured error information following JSON-RPC error format
//! - [`RpcId`] - Request/response identifier (string or number)
//!
//! # Error Handling
//!
//! The module includes standard JSON-RPC error codes in the [`error_codes`] module
//! for consistent error reporting across all RPC methods.

use serde::{Deserialize, Serialize};

/// JSON-RPC 2.0 request structure.
///
/// Represents an incoming RPC request from a debugging client.
/// Contains the method to invoke and optional parameters.
#[derive(Debug, Deserialize)]
pub struct RpcRequest {
    /// JSON-RPC version (always "2.0")
    pub jsonrpc: String,
    /// Method name to invoke (e.g., "snapshot_get", "expr_eval")
    pub method: String,
    /// Optional method parameters as JSON value
    pub params: Option<serde_json::Value>,
    /// Request identifier for matching with response
    pub id: RpcId,
}

/// JSON-RPC 2.0 response structure.
///
/// Represents an outgoing RPC response to a debugging client.
/// Contains either a successful result or an error, never both.
#[derive(Debug, Serialize)]
pub struct RpcResponse {
    /// JSON-RPC version (always "2.0")
    pub jsonrpc: String,
    /// Successful method result (omitted if error occurred)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    /// Error information (omitted if method succeeded)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
    /// Request identifier matching the original request
    pub id: RpcId,
}

/// JSON-RPC 2.0 error structure.
///
/// Provides structured error information when RPC methods fail.
/// Follows the JSON-RPC error object specification.
#[derive(Debug, Serialize)]
pub struct RpcError {
    /// Numeric error code indicating the error type
    pub code: i32,
    /// Human-readable error message
    pub message: String,
    /// Optional additional error data (context, stack traces, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// JSON-RPC request/response identifier.
///
/// Can be either a string or number as per JSON-RPC 2.0 specification.
/// Used to match responses with their corresponding requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RpcId {
    /// Numeric identifier
    Number(u64),
    /// String identifier
    String(String),
}

/// JSON-RPC error codes for consistent error reporting.
///
/// Includes both standard JSON-RPC 2.0 error codes and EDB-specific error codes
/// for debugging-related failures.
pub mod error_codes {
    // Standard JSON-RPC 2.0 error codes

    /// Parse error - Invalid JSON was received by the server
    pub const PARSE_ERROR: i32 = -32700;
    /// Invalid request - The JSON sent is not a valid request object
    pub const INVALID_REQUEST: i32 = -32600;
    /// Method not found - The method does not exist or is not available
    pub const METHOD_NOT_FOUND: i32 = -32601;
    /// Invalid params - Invalid method parameter(s)
    pub const INVALID_PARAMS: i32 = -32602;
    /// Internal error - Internal JSON-RPC error
    pub const INTERNAL_ERROR: i32 = -32603;

    // EDB-specific error codes (starting from -33000)

    /// Snapshot index is out of bounds
    pub const SNAPSHOT_OUT_OF_BOUNDS: i32 = -33001;
    /// Invalid or malformed address
    pub const INVALID_ADDRESS: i32 = -33002;
    /// Trace entry not found for the given ID
    pub const TRACE_ENTRY_NOT_FOUND: i32 = -33003;
    /// Contract code not found at the given address
    pub const CODE_NOT_FOUND: i32 = -33004;
    /// User-defined snapshot ID not found
    pub const USID_NOT_FOUND: i32 = -33005;
    /// Expression evaluation failed
    pub const EVAL_FAILED: i32 = -33006;
}
