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

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use alloy_dyn_abi::{DynSolValue, FunctionExt, JsonAbiExt};
use alloy_json_abi::Function;
use alloy_primitives::{Address, Bytes, TxHash, U256};
use edb_common::{
    disable_nonce_check, relax_evm_context_constraints, relax_evm_tx_constraints,
    types::{parse_callable_abi_entries, Trace},
    DerivedContext, ForkInfo,
};
use eyre::{eyre, Result};
use indicatif::ProgressBar;
use once_cell::sync::OnceCell;
use revm::{
    context::{result::ExecutionResult, tx::TxEnvBuilder, BlockEnv, CfgEnv, TxEnv},
    database::CacheDB,
    Context, Database, DatabaseCommit, DatabaseRef, ExecuteEvm, MainBuilder, MainContext,
    MainnetEvm,
};
use serde::{Deserialize, Serialize};
use tracing::error;

use crate::{analysis::AnalysisResult, Artifact, SnapshotDetail, Snapshots};

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
    /// Relation between target addresses and their (delegated) code addresses
    #[serde(skip)]
    address_code_address_map: OnceCell<HashMap<Address, HashSet<Address>>>,
}

impl<DB> EngineContext<DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone + Send + Sync + 'static,
    <CacheDB<DB> as Database>::Error: Clone + Send + Sync,
    <DB as Database>::Error: Clone + Send + Sync,
{
    /// Build a new EngineContext
    pub fn build(
        fork_info: ForkInfo,
        cfg: CfgEnv,
        block: BlockEnv,
        tx: TxEnv,
        tx_hash: TxHash,
        snapshots: Snapshots<DB>,
        artifacts: HashMap<Address, Artifact>,
        recompiled_artifacts: HashMap<Address, Artifact>,
        analysis_results: HashMap<Address, AnalysisResult>,
        trace: Trace,
    ) -> Result<Self> {
        let mut context = Self {
            fork_info,
            cfg,
            block,
            tx,
            tx_hash,
            snapshots,
            artifacts,
            recompiled_artifacts,
            analysis_results,
            trace,
            address_code_address_map: OnceCell::new(),
        };

        // Finalize the context to populate derived fields
        context.finalize()?;
        Ok(context)
    }

    /// Finalize the EngineContext
    fn finalize(&mut self) -> Result<()> {
        self.finalize_trace()?;
        self.finalize_snapshots()?;

        Ok(())
    }

    /// Finalize the trace by adding the first_step_id to each entry
    fn finalize_trace(&mut self) -> Result<()> {
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

        Ok(())
    }

    /// Finalize snapshots
    fn finalize_snapshots(&mut self) -> Result<()> {
        let tx_hash = self.tx_hash;

        // Actually execute each transaction with revm
        let console_bar = Arc::new(ProgressBar::new(self.snapshots.len() as u64));
        let template = format!("{{spinner:.green}} 🔮 Finalizing steps for {} [{{bar:40.cyan/blue}}] {{pos:>3}}/{{len:3}} ⛽ {{msg}}", &tx_hash.to_string()[2..10]);
        console_bar.set_style(
            indicatif::ProgressStyle::with_template(&template)?
                .progress_chars("🟩🟦⬜")
                .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"),
        );

        let mut results: HashMap<usize, _> = HashMap::new();
        for (snapshot_id, (_, snapshot)) in self.snapshots.iter().enumerate() {
            if snapshot.is_opcode() {
                console_bar.set_message(format!("Analyzing step {} w/o source code", snapshot_id));
                console_bar.inc(1);
                continue;
            } else {
                console_bar.set_message(format!("Analyzing step {} with source code", snapshot_id));
                console_bar.inc(1);
            }

            let code_address = snapshot.bytecode_address();
            let Some(contract) =
                self.recompiled_artifacts.get(&code_address).and_then(|a| a.contract())
            else {
                return Err(eyre!("No contract found for address {}", code_address));
            };

            let mut states = HashMap::new();
            for state_variable in parse_callable_abi_entries(contract)
                .into_iter()
                .filter(|v| v.is_state_variable() && v.inputs.is_empty())
            {
                match self.call_in_derived_evm(
                    snapshot_id,
                    snapshot.target_address(),
                    &state_variable.abi,
                    &[],
                    None,
                ) {
                    Ok(value) => {
                        states.insert(state_variable.name.clone(), Some(Arc::new(value.into())));
                    }
                    Err(e) => {
                        error!(id=?snapshot_id, "Failed to call state variable: {} ({})", state_variable, e);
                        states.insert(state_variable.name.clone(), None);
                    }
                }
            }

            results.insert(snapshot_id, states);
        }

        for (snapshot_id, states) in results.into_iter() {
            if let Some((_, snapshot)) = self.snapshots.get_mut(snapshot_id) {
                if let SnapshotDetail::Hook(ref mut hook_detail) = snapshot.detail_mut() {
                    hook_detail.state_variables = states;
                }
            }
        }

        console_bar.finish_with_message(format!(
            "✨ Ready! All {} steps analyzed and finalized.",
            self.snapshots.len()
        ));

        Ok(())
    }
}

// Get methods for context
impl<DB> EngineContext<DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone + Send + Sync + 'static,
    <CacheDB<DB> as Database>::Error: Clone + Send + Sync,
    <DB as Database>::Error: Clone + Send + Sync,
{
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

    /// Get or initialize the address to code address mapping
    pub fn address_code_address_map(&self) -> &HashMap<Address, HashSet<Address>> {
        self.address_code_address_map.get_or_init(|| {
            let mut map: HashMap<Address, HashSet<Address>> = HashMap::new();
            for entry in &self.trace {
                map.entry(entry.target).or_default().insert(entry.code_address);
            }
            map
        })
    }
}

// EVM creation and manipulation
impl<DB> EngineContext<DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone + Send + Sync + 'static,
    <CacheDB<DB> as Database>::Error: Clone + Send + Sync,
    <DB as Database>::Error: Clone + Send + Sync,
{
    /// Create an EVM for the given snapshot id
    pub fn create_evm_for_snapshot(
        &self,
        snapshot_id: usize,
    ) -> Option<MainnetEvm<DerivedContext<DB>>> {
        let (_, snapshot) = self.snapshots.get(snapshot_id)?;

        let db = CacheDB::new(CacheDB::new(snapshot.db()));
        let cfg = self.cfg.clone();
        let block = self.block.clone();

        let mut ctx = Context::mainnet().with_db(db).with_cfg(cfg).with_block(block);
        relax_evm_context_constraints(&mut ctx);
        disable_nonce_check(&mut ctx);

        Some(ctx.build_mainnet())
    }

    /// Send a mocked transaction in the EVM for the given snapshot id
    pub fn send_transaction_in_derived_evm(
        &self,
        snapshot_id: usize,
        to: Address,
        data: &[u8],
        value: U256,
    ) -> Result<ExecutionResult> {
        let mut evm = self
            .create_evm_for_snapshot(snapshot_id)
            .ok_or(eyre!("No EVM found at snapshot {}", snapshot_id))?;

        let mut tx_env = TxEnvBuilder::new()
            .caller(self.tx.caller)
            .call(to)
            .value(value)
            .data(Bytes::copy_from_slice(data))
            .build_fill();
        relax_evm_tx_constraints(&mut tx_env);

        evm.transact_one(tx_env).map_err(|e| eyre!(e.to_string()))
    }

    /// Invoke a call in the EVM for the given snapshot id
    pub fn call_in_derived_evm(
        &self,
        snapshot_id: usize,
        to: Address,
        function: &Function,
        args: &[DynSolValue],
        value: Option<U256>,
    ) -> Result<DynSolValue> {
        let data = function.abi_encode_input(args).map_err(|e| eyre!(e.to_string()))?;
        let value = value.unwrap_or_default();

        let result = self.send_transaction_in_derived_evm(snapshot_id, to, &data, value)?;

        match result {
            ExecutionResult::Success { output, .. } => {
                let decoded =
                    function.abi_decode_output(output.data()).map_err(|e| eyre!(e.to_string()))?;
                if decoded.len() == 1 {
                    Ok(decoded.into_iter().next().unwrap())
                } else {
                    Ok(DynSolValue::Tuple(decoded))
                }
            }
            ExecutionResult::Revert { output, .. } => {
                Err(eyre!("Call reverted with output: 0x{}", hex::encode(output)))
            }
            ExecutionResult::Halt { reason, .. } => {
                Err(eyre!("Call halted with reason: {:?}", reason))
            }
        }
    }
}
