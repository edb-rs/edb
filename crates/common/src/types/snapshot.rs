use std::path::PathBuf;

use alloy_primitives::{Address, Bytes, U256};
use revm::state::TransientStorage;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SnapshotInfo {
    Opcode(OpcodeSnapshotInfo),
    Hook(HookSnapshotInfo),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookSnapshotInfo {
    /// Address of the contract
    pub address: Address,
    /// Current file
    pub path: PathBuf,
    /// Offset within the file
    pub offset: usize,
    /// Length of the step
    pub length: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpcodeSnapshotInfo {
    /// Address of the contract
    pub address: Address,
    /// Program counter (instruction offset)
    pub pc: usize,
    /// Current opcode
    pub opcode: u8,
    /// Memory state (shared via Arc when unchanged)
    pub memory: Vec<u8>,
    /// Stack state (always cloned as most opcodes modify it)
    pub stack: Vec<U256>,
    /// Call data for this execution context (shared via Arc within same context)
    pub calldata: Bytes,
    /// Transition storage
    pub transition_storage: TransientStorage,
}
