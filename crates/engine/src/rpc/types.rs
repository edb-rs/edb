//! RPC data types and serialization structures
//!
//! This module defines all the data structures used for JSON-RPC communication
//! between the TUI and the RPC server.

use crate::USID;
use alloy_primitives::{Address, Bytes, U256};
use foundry_compilers::artifacts::ast::SourceLocation;
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

/// Information about current snapshot state
#[derive(Debug, Serialize, Deserialize)]
pub struct SnapshotInfo {
    pub index: usize,
    pub total: usize,
    pub snapshot_type: SnapshotType,
    pub frame_id: ExecutionFrameId,
    pub address: Address,
    // For opcode snapshots
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pc: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opcode: Option<String>,
    // For hook snapshots
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usid: Option<u64>,
}

/// Type of snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SnapshotType {
    Opcode,
    Hook,
}

/// Execution frame identifier  
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ExecutionFrameId {
    pub trace_entry_id: usize,
    pub re_entry_count: usize,
}

impl ExecutionFrameId {
    pub fn trace_entry_id(&self) -> usize {
        self.trace_entry_id
    }

    pub fn re_entry_count(&self) -> usize {
        self.re_entry_count
    }
}

/// Trace entry information
#[derive(Debug, Serialize, Deserialize)]
pub struct TraceEntry {
    pub id: usize,
    pub parent_id: Option<usize>,
    pub depth: usize,
    pub call_type: CallType,
    pub caller: Address,
    pub target: Address,
    pub code_address: Address,
    pub input: String,
    pub value: String,
    pub result: Option<CallResult>,
    pub created_contract: bool,
    pub bytecode: Option<String>,
}

/// Type of call operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CallType {
    Call(String),   // CallScheme as string
    Create(String), // CreateScheme as string
}

/// Result of a call operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CallResult {
    Success { output: String },
    Revert { output: String },
}

/// Source code information
#[derive(Debug, Serialize, Deserialize)]
pub struct SourceCodeInfo {
    pub path: String,
    pub content: String,
    pub current_line: Option<usize>,
    pub artifact_type: ArtifactType,
}

/// Type of artifact
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ArtifactType {
    Original,
    Instrumented,
}

/// Opcode information
#[derive(Debug, Serialize, Deserialize)]
pub struct OpcodeInfo {
    pub pc: usize,
    pub opcode: String,
    pub description: String,
}

/// Variable information
#[derive(Debug, Serialize, Deserialize)]
pub struct Variable {
    pub name: String,
    pub var_type: String,
    pub value: String,
    pub scope: String,
}

/// Stack entry
#[derive(Debug, Serialize, Deserialize)]
pub struct StackEntry {
    pub index: usize,
    pub value: String,
}

/// Memory information
#[derive(Debug, Serialize, Deserialize)]
pub struct MemoryInfo {
    pub offset: usize,
    pub size: usize,
    pub data: String,
}

/// Call data information
#[derive(Debug, Serialize, Deserialize)]
pub struct CallDataInfo {
    pub size: usize,
    pub data: String,
}

/// State information
#[derive(Debug, Serialize, Deserialize)]
pub struct StateInfo {
    pub address: Address,
    pub balance: String,
    pub nonce: u64,
    pub code_hash: String,
    pub storage: HashMap<String, String>,
}

/// Transition state information
#[derive(Debug, Serialize, Deserialize)]
pub struct TransitionStateInfo {
    pub transient_storage: HashMap<String, String>,
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

/// Command execution result
#[derive(Debug, Serialize, Deserialize)]
pub struct CommandResult {
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
}

/// Expression evaluation result  
#[derive(Debug, Serialize, Deserialize)]
pub struct EvaluationResult {
    pub success: bool,
    pub value: Option<String>,
    pub var_type: Option<String>,
    pub error: Option<String>,
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
    pub const SOURCE_NOT_FOUND: i32 = -33003;
    pub const BREAKPOINT_NOT_FOUND: i32 = -33004;
    pub const EVALUATION_ERROR: i32 = -33005;
}
