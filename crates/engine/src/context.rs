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

use std::collections::HashMap;

use alloy_dyn_abi::abi;
use alloy_primitives::{Address, Selector, TxHash};
use edb_common::{types::Trace, ForkInfo};
use revm::{
    context::{BlockEnv, CfgEnv, TxEnv},
    database::CacheDB,
    Database, DatabaseCommit, DatabaseRef,
};
use serde::{Deserialize, Serialize};

use crate::{analysis::AnalysisResult, Artifact, Snapshots};

/// Complete debugging context containing all analysis results and state snapshots
///
/// This struct encapsulates all the data produced during the debugging workflow,
/// including the original transaction context, collected snapshots, analyzed source code,
/// and recompiled artifacts. It serves as the primary data structure passed to the
/// JSON-RPC server for time travel debugging.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineContext<DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone + Send + Sync + 'static,
    <CacheDB<DB> as Database>::Error: Clone + Send + Sync,
    <DB as Database>::Error: Clone + Send + Sync,
{
    /// Forked information
    pub fork_info: ForkInfo,
    /// Configuration environment for the EVM
    pub cfg: CfgEnv,
    /// Block environment for the target block
    pub block: BlockEnv,
    /// Transaction environment for the target transaction
    pub tx: TxEnv,
    /// Transaction hash for the target transaction
    pub tx_hash: TxHash,
    /// Merged snapshots from both opcode-level and hook-based collection
    pub snapshots: Snapshots<DB>,
    /// Original contract artifacts with source code and metadata
    pub artifacts: HashMap<Address, Artifact>,
    /// Recompiled artifacts with instrumented source code
    pub recompiled_artifacts: HashMap<Address, Artifact>,
    /// Analysis results identifying instrumentation points
    pub analysis_results: HashMap<Address, AnalysisResult>,
    /// Execution trace showing call hierarchy and frame structure
    pub trace: Trace,
}

impl<DB> EngineContext<DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone + Send + Sync + 'static,
    <CacheDB<DB> as Database>::Error: Clone + Send + Sync,
    <DB as Database>::Error: Clone + Send + Sync,
{
    /// Finalize the EngineContext
    pub fn finalize(&mut self) {
        self.finalize_trace();
    }

    /// Finalize the trace by adding the first_step_id to each entry
    fn finalize_trace(&mut self) {
        for entry in &mut self.trace {
            let trace_id = entry.id;

            // We first update the snapshot id
            for (snapshot_id, (frame_id, _)) in self.snapshots.iter().enumerate() {
                if frame_id.trace_entry_id() == trace_id {
                    entry.first_snapshot_id = Some(snapshot_id);
                    break;
                }
            }
        }
    }

    /// Get code address of a given snapshot id
    pub fn get_bytecode_address(&self, snapshot_id: usize) -> Option<Address> {
        let (frame_id, _) = self.snapshots.get(snapshot_id)?;
        self.trace.get(frame_id.trace_entry_id()).and_then(|entry| Some(entry.code_address))
    }

    /// Get target address of a given snapshot id
    pub fn get_target_address(&self, snapshot_id: usize) -> Option<Address> {
        let (frame_id, _) = self.snapshots.get(snapshot_id)?;
        self.trace.get(frame_id.trace_entry_id()).and_then(|entry| Some(entry.target))
    }

    /// Is the given trace id a parent of another trace id?
    pub fn is_parent_trace(&self, parent_id: usize, child_id: usize) -> bool {
        match self.trace.get(child_id) {
            Some(child_entry) => child_entry.parent_id == Some(parent_id),
            None => false,
        }
    }
}
