use std::path::PathBuf;

use alloy_primitives::{Address, Bytes, U256};
use revm::state::TransientStorage;
use serde::{Deserialize, Serialize};

use crate::types::ExecutionFrameId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SnapshotInfo {
    Opcode(OpcodeSnapshotInfo),
    Hook(HookSnapshotInfo),
}

impl SnapshotInfo {
    pub fn address(&self) -> Address {
        match self {
            SnapshotInfo::Opcode(info) => info.address,
            SnapshotInfo::Hook(info) => info.address,
        }
    }

    pub fn frame_id(&self) -> ExecutionFrameId {
        match self {
            SnapshotInfo::Opcode(info) => info.frame_id,
            SnapshotInfo::Hook(info) => info.frame_id,
        }
    }

    pub fn path(&self) -> Option<&PathBuf> {
        match self {
            SnapshotInfo::Opcode(_) => None,
            SnapshotInfo::Hook(info) => Some(&info.path),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookSnapshotInfo {
    /// Address of the contract
    pub address: Address,
    /// Execution Frame Id
    pub frame_id: ExecutionFrameId,
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
    /// Execution Frame Id
    pub frame_id: ExecutionFrameId,
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
