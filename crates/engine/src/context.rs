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

//! Engine context management and EVM instantiation for debugging.
//!
//! This module provides the core [`EngineContext`] struct that consolidates all debugging
//! data and state needed for time travel debugging and expression evaluation. The context
//! serves as the primary data structure passed to the JSON-RPC server and contains all
//! analysis results, snapshots, artifacts, and execution state.
//!
//! # Core Components
//!
//! ## EngineContext
//! The [`EngineContext`] is the central data structure that encapsulates:
//! - **Fork Information**: Network and block context for the debugging session
//! - **EVM Environment**: Configuration, block, and transaction environments
//! - **Snapshots**: Merged opcode-level and hook-based execution snapshots
//! - **Artifacts**: Original and recompiled contract artifacts with source code
//! - **Analysis Results**: Instrumentation points and debugging metadata
//! - **Execution Trace**: Call hierarchy and frame structure
//!
//! ## EVM Instantiation
//! The context provides methods to create derived EVMs for expression evaluation:
//! - **Snapshot-specific EVMs**: Create EVM instances at any execution point
//! - **Transaction replay**: Send mock transactions in derived state
//! - **Function calls**: Invoke contract functions for expression evaluation
//!
//! # Workflow Integration
//!
//! 1. **Context Building**: Constructed after analysis and snapshot collection
//! 2. **Finalization**: Processes traces and pre-evaluates state variables
//! 3. **Server Integration**: Passed to JSON-RPC server for debugging API
//! 4. **Expression Evaluation**: Used to create derived EVMs for real-time evaluation
//!
//! # Thread Safety
//!
//! The [`EngineContext`] is designed to be thread-safe and can be shared across
//! multiple debugging clients through Arc wrapping. All database operations
//! use read-only snapshots to ensure debugging session integrity.

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
    /// Build a new EngineContext with all debugging data.
    ///
    /// This constructor consolidates all the debugging data collected during the analysis
    /// and snapshot collection phases into a unified context for debugging operations.
    ///
    /// # Arguments
    ///
    /// * `fork_info` - Network and fork information for the debugging session
    /// * `cfg` - EVM configuration environment
    /// * `block` - Block environment for the target block
    /// * `tx` - Transaction environment for the target transaction
    /// * `tx_hash` - Hash of the target transaction
    /// * `snapshots` - Merged snapshots from opcode and hook collection
    /// * `artifacts` - Original contract artifacts with source code
    /// * `recompiled_artifacts` - Recompiled artifacts with instrumentation
    /// * `analysis_results` - Analysis results identifying instrumentation points
    /// * `trace` - Execution trace showing call hierarchy
    ///
    /// # Returns
    ///
    /// Returns a finalized [`EngineContext`] ready for debugging operations.
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

    /// Finalize the EngineContext by processing traces and snapshots.
    ///
    /// This method performs post-processing on the collected debugging data:
    /// 1. Links trace entries with their corresponding snapshot IDs
    /// 2. Pre-evaluates state variables for all hook-based snapshots
    /// 3. Populates derived mappings for efficient lookups
    fn finalize(&mut self) -> Result<()> {
        self.finalize_trace()?;
        self.finalize_snapshots()?;

        Ok(())
    }

    /// Finalize the trace by linking trace entries with their corresponding snapshot IDs.
    ///
    /// This method processes the execution trace to establish the relationship between
    /// trace entries and snapshots, enabling efficient navigation during debugging.
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

    /// Finalize snapshots by pre-evaluating state variables.
    ///
    /// This method processes all hook-based snapshots to pre-evaluate their state
    /// variables, making them immediately available for expression evaluation.
    /// This optimization reduces latency during debugging sessions.
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
                console_bar.set_message(format!("Analyzing step {snapshot_id} w/o source code"));
                console_bar.inc(1);
                continue;
            } else {
                console_bar.set_message(format!("Analyzing step {snapshot_id} with source code"));
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

// Context query methods for debugging operations
impl<DB> EngineContext<DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone + Send + Sync + 'static,
    <CacheDB<DB> as Database>::Error: Clone + Send + Sync,
    <DB as Database>::Error: Clone + Send + Sync,
{
    /// Get the bytecode address for a snapshot.
    ///
    /// Returns the address where the executing bytecode is stored, which may differ
    /// from the target address in cases of delegatecall or proxy contracts.
    pub fn get_bytecode_address(&self, snapshot_id: usize) -> Option<Address> {
        let (frame_id, _) = self.snapshots.get(snapshot_id)?;
        self.trace.get(frame_id.trace_entry_id()).map(|entry| entry.code_address)
    }

    /// Get the target address for a snapshot.
    ///
    /// Returns the address that was the target of the call, which is the address
    /// receiving the call in the current execution frame.
    pub fn get_target_address(&self, snapshot_id: usize) -> Option<Address> {
        let (frame_id, _) = self.snapshots.get(snapshot_id)?;
        self.trace.get(frame_id.trace_entry_id()).map(|entry| entry.target)
    }

    /// Check if one trace entry is the parent of another.
    ///
    /// This method determines the parent-child relationship between trace entries,
    /// useful for understanding call hierarchy during debugging.
    pub fn is_parent_trace(&self, parent_id: usize, child_id: usize) -> bool {
        match self.trace.get(child_id) {
            Some(child_entry) => child_entry.parent_id == Some(parent_id),
            None => false,
        }
    }

    /// Get the address to code address mapping.
    ///
    /// Returns a cached mapping from target addresses to all code addresses that
    /// have been executed for each target. This is useful for understanding
    /// proxy patterns and delegatecall relationships.
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

// EVM creation and expression evaluation methods
impl<DB> EngineContext<DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone + Send + Sync + 'static,
    <CacheDB<DB> as Database>::Error: Clone + Send + Sync,
    <DB as Database>::Error: Clone + Send + Sync,
{
    /// Create a derived EVM instance for a specific snapshot.
    ///
    /// This method creates a new EVM instance using the database state from the
    /// specified snapshot. The resulting EVM can be used for expression evaluation
    /// and function calls without affecting the original debugging state.
    ///
    /// # Arguments
    ///
    /// * `snapshot_id` - The snapshot ID to create the EVM for
    ///
    /// # Returns
    ///
    /// Returns a configured EVM instance or None if the snapshot doesn't exist.
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

    /// Send a mock transaction in a derived EVM.
    ///
    /// This method executes a transaction in the EVM state at the specified snapshot
    /// without affecting the original debugging state. Used for expression evaluation
    /// that requires transaction execution.
    ///
    /// # Arguments
    ///
    /// * `snapshot_id` - The snapshot ID to use as the base state
    /// * `to` - The target address for the transaction
    /// * `data` - The transaction data (call data)
    /// * `value` - The value to send with the transaction
    ///
    /// # Returns
    ///
    /// Returns the execution result or an error if the transaction fails.
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

    /// Invoke a contract function call in a derived EVM.
    ///
    /// This method calls a specific contract function in the EVM state at the
    /// specified snapshot. It handles ABI encoding/decoding automatically and
    /// returns the decoded result.
    ///
    /// # Arguments
    ///
    /// * `snapshot_id` - The snapshot ID to use as the base state
    /// * `to` - The contract address to call
    /// * `function` - The ABI function definition
    /// * `args` - The function arguments
    /// * `value` - Optional value to send with the call
    ///
    /// # Returns
    ///
    /// Returns the decoded function result or an error if the call fails.
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
