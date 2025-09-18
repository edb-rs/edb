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

#[derive(Debug, Clone, Serialize, Deserialize, From)]
pub struct SnapshotInfo {
    pub id: usize,
    pub frame_id: ExecutionFrameId,
    pub next_id: usize,
    pub prev_id: usize,
    pub detail: SnapshotInfoDetail,
    pub target_address: Address,
    pub bytecode_address: Address,
}

#[derive(Debug, Clone, Serialize, Deserialize, From)]
pub enum SnapshotInfoDetail {
    Opcode(#[from] OpcodeSnapshotInfoDetail),
    Hook(#[from] HookSnapshotInfoDetail),
}

impl SnapshotInfo {
    pub fn frame_id(&self) -> ExecutionFrameId {
        self.frame_id
    }

    pub fn id(&self) -> usize {
        self.id
    }

    pub fn next_id(&self) -> usize {
        self.next_id
    }

    pub fn prev_id(&self) -> usize {
        self.prev_id
    }

    pub fn detail(&self) -> &SnapshotInfoDetail {
        &self.detail
    }

    pub fn path(&self) -> Option<&PathBuf> {
        match self.detail() {
            SnapshotInfoDetail::Opcode(_) => None,
            SnapshotInfoDetail::Hook(info) => Some(&info.path),
        }
    }

    pub fn offset(&self) -> Option<usize> {
        match self.detail() {
            SnapshotInfoDetail::Opcode(_) => None,
            SnapshotInfoDetail::Hook(info) => Some(info.offset),
        }
    }

    pub fn pc(&self) -> Option<usize> {
        match self.detail() {
            SnapshotInfoDetail::Opcode(info) => Some(info.pc),
            SnapshotInfoDetail::Hook(_) => None,
        }
    }

    pub fn locals(&self) -> Option<&HashMap<String, Option<Arc<EdbSolValue>>>> {
        match self.detail() {
            SnapshotInfoDetail::Opcode(_) => None,
            SnapshotInfoDetail::Hook(info) => Some(&info.locals),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookSnapshotInfoDetail {
    /// Snapshot Id
    pub id: usize,
    /// Execution Frame Id
    pub frame_id: ExecutionFrameId,
    /// Current file
    pub path: PathBuf,
    /// Offset within the file
    pub offset: usize,
    /// Length of the step
    pub length: usize,
    /// Local variables
    pub locals: HashMap<String, Option<Arc<EdbSolValue>>>,
    /// State variables (at the current bytecode address)
    pub state_variables: HashMap<String, Option<Arc<EdbSolValue>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpcodeSnapshotInfoDetail {
    /// Snapshot Id
    pub id: usize,
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
    /// Transient storage
    pub transient_storage: TransientStorage,
}
