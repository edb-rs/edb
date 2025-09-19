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

use std::{collections::HashMap, path::PathBuf, sync::Arc};

use alloy_primitives::{Address, Bytes, U256};
use derive_more::From;
use revm::state::TransientStorage;
use serde::{Deserialize, Serialize};

use crate::types::{EdbSolValue, ExecutionFrameId};

/// Complete snapshot information capturing EVM state at a specific execution point for debugging navigation
#[derive(Debug, Clone, Serialize, Deserialize, From)]
pub struct SnapshotInfo {
    /// Unique snapshot identifier for debugging navigation
    pub id: usize,
    /// Execution frame this snapshot belongs to
    pub frame_id: ExecutionFrameId,
    /// Identifier of the next snapshot in execution order for forward navigation
    pub next_id: usize,
    /// Identifier of the previous snapshot in execution order for backward navigation
    pub prev_id: usize,
    /// Detailed snapshot information varying by debugging mode (opcode vs source)
    pub detail: SnapshotInfoDetail,
    /// Target contract address for this execution step
    pub target_address: Address,
    /// Address where the actual bytecode is stored (may differ from target in proxy patterns)
    pub bytecode_address: Address,
}

/// Snapshot detail information varying by debugging mode for different levels of analysis
#[derive(Debug, Clone, Serialize, Deserialize, From)]
pub enum SnapshotInfoDetail {
    /// Low-level opcode debugging with EVM state (program counter, stack, memory)
    Opcode(#[from] OpcodeSnapshotInfoDetail),
    /// High-level source debugging with variable state and source location mapping
    Hook(#[from] HookSnapshotInfoDetail),
}

impl SnapshotInfo {
    /// Get the execution frame identifier this snapshot belongs to
    pub fn frame_id(&self) -> ExecutionFrameId {
        self.frame_id
    }

    /// Get the unique snapshot identifier for navigation purposes
    pub fn id(&self) -> usize {
        self.id
    }

    /// Get the identifier of the next snapshot for forward debugging navigation
    pub fn next_id(&self) -> usize {
        self.next_id
    }

    /// Get the identifier of the previous snapshot for backward debugging navigation
    pub fn prev_id(&self) -> usize {
        self.prev_id
    }

    /// Get the detailed snapshot information based on debugging mode
    pub fn detail(&self) -> &SnapshotInfoDetail {
        &self.detail
    }

    /// Get the source file path for source-level debugging, None for opcode debugging
    pub fn path(&self) -> Option<&PathBuf> {
        match self.detail() {
            SnapshotInfoDetail::Opcode(_) => None,
            SnapshotInfoDetail::Hook(info) => Some(&info.path),
        }
    }

    /// Get the source file offset for source-level debugging, None for opcode debugging
    pub fn offset(&self) -> Option<usize> {
        match self.detail() {
            SnapshotInfoDetail::Opcode(_) => None,
            SnapshotInfoDetail::Hook(info) => Some(info.offset),
        }
    }

    /// Get the program counter for opcode-level debugging, None for source debugging
    pub fn pc(&self) -> Option<usize> {
        match self.detail() {
            SnapshotInfoDetail::Opcode(info) => Some(info.pc),
            SnapshotInfoDetail::Hook(_) => None,
        }
    }

    /// Get local variables for source-level debugging, None for opcode debugging
    pub fn locals(&self) -> Option<&HashMap<String, Option<Arc<EdbSolValue>>>> {
        match self.detail() {
            SnapshotInfoDetail::Opcode(_) => None,
            SnapshotInfoDetail::Hook(info) => Some(&info.locals),
        }
    }
}

/// Source-level debugging snapshot with variable states and source location mapping
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookSnapshotInfoDetail {
    /// Unique snapshot identifier for debugging navigation
    pub id: usize,
    /// Execution frame identifier this snapshot belongs to
    pub frame_id: ExecutionFrameId,
    /// Source file path where execution is currently positioned
    pub path: PathBuf,
    /// Character offset within the source file for precise location tracking
    pub offset: usize,
    /// Length of the current source code segment being executed
    pub length: usize,
    /// Local variables and their current values in this execution context
    pub locals: HashMap<String, Option<Arc<EdbSolValue>>>,
    /// Contract state variables and their values at the current bytecode address
    pub state_variables: HashMap<String, Option<Arc<EdbSolValue>>>,
}

/// Low-level opcode debugging snapshot with complete EVM state for instruction-level analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpcodeSnapshotInfoDetail {
    /// Unique snapshot identifier for debugging navigation
    pub id: usize,
    /// Execution frame identifier this snapshot belongs to
    pub frame_id: ExecutionFrameId,
    /// Program counter indicating current bytecode instruction offset
    pub pc: usize,
    /// Current opcode byte value being executed
    pub opcode: u8,
    /// EVM memory state at this execution point (shared via Arc when unchanged for efficiency)
    pub memory: Vec<u8>,
    /// EVM stack state with all values (always cloned as most opcodes modify it)
    pub stack: Vec<U256>,
    /// Call data for this execution context (shared via Arc within same call frame)
    pub calldata: Bytes,
    /// Transient storage state for EIP-1153 temporary storage operations
    pub transient_storage: TransientStorage,
}
