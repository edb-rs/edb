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

//! Hook snapshot inspector for recording VM state at specific trigger points
//!
//! This inspector captures database snapshots only when execution calls
//! the magic trigger address 0x0000000000000000000000000000000000023333.
//! The call data contains an ABI-encoded number (usid) that identifies
//! the specific hook point.
//!
//! Unlike OpcodeSnapshotInspector which captures every instruction,
//! HookSnapshotInspector only captures at predetermined breakpoints,
//! making it more efficient for tracking specific execution states.

use alloy_dyn_abi::{DynSolType, DynSolValue};
use alloy_primitives::{Address, Bytes, U256};
use edb_common::{
    types::{CallResult, EdbSolValue, ExecutionFrameId, Trace},
    EdbContext, OpcodeTr,
};
use eyre::Result;
use foundry_compilers::{artifacts::Contract, Artifact};
use revm::{
    bytecode::OpCode,
    context::{ContextTr, CreateScheme, JournalTr},
    database::CacheDB,
    interpreter::{
        interpreter_types::{InputsTr, Jumps},
        CallInputs, CallOutcome, CreateInputs, CreateOutcome, Interpreter,
    },
    state::TransientStorage,
    Database, DatabaseCommit, DatabaseRef, Inspector,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
    sync::Arc,
};
use tracing::{debug, error};

use crate::{
    analysis::{dyn_sol_type, AnalysisResult, UserDefinedTypeRef, VariableRef, UVID},
    USID,
};

/// Magic number that indicates a snapshot to be taken
pub const MAGIC_SNAPSHOT_NUMBER: U256 = U256::from_be_bytes([
    0x20, 0x15, 0x05, 0x02, 0xff, 0xff, 0xff, 0xff, 0x20, 0x24, 0x01, 0x02, 0xff, 0xff, 0xff, 0xff,
    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
]);

/// Magic number that indicates a variable update to be recorded
pub const MAGIC_VARIABLE_UPDATE_NUMBER: U256 = U256::from_be_bytes([
    0x20, 0x25, 0x02, 0x08, 0xff, 0x20, 0x25, 0x09, 0x16, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
]);

/// Single hook execution snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookSnapshot<DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone,
    <CacheDB<DB> as Database>::Error: Clone,
    <DB as Database>::Error: Clone,
{
    /// Target address that triggered the hook
    pub target_address: Address,
    /// Bytecode address that the current snapshot is running
    pub bytecode_address: Address,
    /// Database state at the hook point
    pub database: Arc<CacheDB<DB>>,
    /// Transient storage
    #[serde(with = "edb_common::types::arc_transient_string_map")]
    pub transient_storage: Arc<TransientStorage>,
    /// Value of accessible local variables
    pub locals: HashMap<String, Option<Arc<EdbSolValue>>>,
    /// Value of state variables at this point (e.g., code address)
    pub state_variables: HashMap<String, Option<Arc<EdbSolValue>>>,
    /// User-defined snapshot ID from call data
    pub usid: USID,
}

/// Collection of hook snapshots organized by execution order
///
/// Unlike OpcodeSnapshots which use a HashMap, HookSnapshots maintains
/// insertion order to track when each snapshot was taken during execution.
#[derive(Debug, Clone)]
pub struct HookSnapshots<DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone,
    <CacheDB<DB> as Database>::Error: Clone,
    <DB as Database>::Error: Clone,
{
    /// Vector of (frame_id, optional_snapshot) pairs in execution order
    /// None indicates a frame where no hook was triggered
    snapshots: Vec<(ExecutionFrameId, Option<HookSnapshot<DB>>)>,
}

impl<DB> Default for HookSnapshots<DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone,
    <CacheDB<DB> as Database>::Error: Clone,
    <DB as Database>::Error: Clone,
{
    fn default() -> Self {
        Self { snapshots: Vec::new() }
    }
}

impl<DB> Deref for HookSnapshots<DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone,
    <CacheDB<DB> as Database>::Error: Clone,
    <DB as Database>::Error: Clone,
{
    type Target = Vec<(ExecutionFrameId, Option<HookSnapshot<DB>>)>;

    fn deref(&self) -> &Self::Target {
        &self.snapshots
    }
}

impl<DB> DerefMut for HookSnapshots<DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone,
    <CacheDB<DB> as Database>::Error: Clone,
    <DB as Database>::Error: Clone,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.snapshots
    }
}

impl<DB> IntoIterator for HookSnapshots<DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone,
    <CacheDB<DB> as Database>::Error: Clone,
    <DB as Database>::Error: Clone,
{
    type Item = (ExecutionFrameId, Option<HookSnapshot<DB>>);
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.snapshots.into_iter()
    }
}

impl<DB> HookSnapshots<DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone,
    <CacheDB<DB> as Database>::Error: Clone,
    <DB as Database>::Error: Clone,
{
    /// Get snapshot for a specific frame ID
    pub fn get_snapshot(&self, frame_id: ExecutionFrameId) -> Option<&HookSnapshot<DB>> {
        self.snapshots
            .iter()
            .find(|(id, _)| *id == frame_id)
            .and_then(|(_, snapshot)| snapshot.as_ref())
    }

    /// Get all frames that have actual hook snapshots (non-None)
    pub fn get_frames_with_hooks(&self) -> Vec<ExecutionFrameId> {
        self.snapshots
            .iter()
            .filter_map(
                |(frame_id, snapshot)| {
                    if snapshot.is_some() {
                        Some(*frame_id)
                    } else {
                        None
                    }
                },
            )
            .collect()
    }

    /// Add a frame placeholder (will be None if no hook is triggered)
    fn add_frame_placeholder(&mut self, frame_id: ExecutionFrameId) {
        self.snapshots.push((frame_id, None));
    }

    /// Update the last frame with a hook snapshot
    fn update_last_frame_with_snapshot(
        &mut self,
        frame_id: ExecutionFrameId,
        snapshot: HookSnapshot<DB>,
    ) {
        if let Some((last_frame_id, slot)) = self.snapshots.last_mut() {
            if last_frame_id != &frame_id {
                error!("Mismatched frame IDs: expected {}, got {}", last_frame_id, frame_id);
            }
            if slot.is_none() {
                // If the last frame was empty, fill it with this snapshot
                *slot = Some(snapshot);
                return;
            }
        }

        self.snapshots.push((frame_id, Some(snapshot)));
    }
}

/// Inspector that records hook-triggered snapshots
#[derive(Debug)]
pub struct HookSnapshotInspector<'a, DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone,
    <CacheDB<DB> as Database>::Error: Clone,
    <DB as Database>::Error: Clone,
{
    /// The trace of the current tx
    trace: &'a Trace,

    /// Source code analysis results
    analysis: &'a HashMap<Address, AnalysisResult>,

    /// Collection of hook snapshots
    pub snapshots: HookSnapshots<DB>,

    /// Stack to track current execution frames
    frame_stack: Vec<ExecutionFrameId>,

    /// Current trace entry counter
    current_trace_id: usize,

    /// Creation hooks (original contract bytecode, hooked bytecode, constructor args)
    creation_hooks: Vec<(Bytes, Bytes, Bytes)>,

    /// The latest value of each UVID encountered (for variable tracking)
    uvid_values: HashMap<UVID, Arc<EdbSolValue>>,

    /// Last opcode
    last_opcode: Option<OpCode>,

    /// The current database
    database: Arc<CacheDB<DB>>,

    /// The current transient storage
    transient_storage: Arc<TransientStorage>,
}

impl<'a, DB> HookSnapshotInspector<'a, DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone,
    <CacheDB<DB> as Database>::Error: Clone,
    <DB as Database>::Error: Clone,
{
    /// Create a new hook snapshot inspector
    pub fn new(
        ctx: &EdbContext<DB>,
        trace: &'a Trace,
        analysis: &'a HashMap<Address, AnalysisResult>,
    ) -> Self {
        Self {
            trace,
            analysis,
            snapshots: HookSnapshots::default(),
            frame_stack: Vec::new(),
            current_trace_id: 0,
            creation_hooks: Vec::new(),
            uvid_values: HashMap::new(),
            last_opcode: None,
            database: Arc::new(ctx.db().clone()),
            transient_storage: Arc::new(TransientStorage::default()),
        }
    }

    /// Add creation hooks
    pub fn with_creation_hooks(
        &mut self,
        hooks: Vec<(&Contract, &Contract, &Bytes)>,
    ) -> Result<()> {
        for (original, hooked, args) in hooks {
            self.creation_hooks.push((
                original
                    .get_bytecode_bytes()
                    .ok_or(eyre::eyre!("Failed to get bytecode for contract"))?
                    .as_ref()
                    .clone(),
                hooked
                    .get_bytecode_bytes()
                    .ok_or(eyre::eyre!("Failed to get bytecode for contract"))?
                    .as_ref()
                    .clone(),
                args.clone(),
            ));
        }

        Ok(())
    }

    /// Consume the inspector and return the collected snapshots
    pub fn into_snapshots(self) -> HookSnapshots<DB> {
        self.snapshots
    }

    /// Get the current execution frame ID
    fn current_frame_id(&self) -> Option<ExecutionFrameId> {
        self.frame_stack.last().copied()
    }

    /// Start tracking a new execution frame
    fn push_frame(&mut self, trace_id: usize) {
        let frame_id = ExecutionFrameId::new(trace_id, 0);
        self.frame_stack.push(frame_id);

        // Add placeholder for this frame
        self.snapshots.add_frame_placeholder(frame_id);
    }

    /// Stop tracking current execution frame and increment re-entry count
    fn pop_frame(&mut self) -> Option<ExecutionFrameId> {
        if let Some(frame_id) = self.frame_stack.pop() {
            // Increment re-entry count for parent frame if it exists
            if let Some(parent_frame_id) = self.frame_stack.last_mut() {
                parent_frame_id.increment_re_entry();
            }

            // Add placeholder for the new frame
            if let Some(current_frame_id) = self.current_frame_id() {
                self.snapshots.add_frame_placeholder(current_frame_id);
            }

            Some(frame_id)
        } else {
            None
        }
    }

    /// Update database when storage is changed
    fn update_database(&mut self, ctx: &EdbContext<DB>, force_update: bool) {
        if force_update
            || self
                .last_opcode
                .map(|c| c.modifies_evm_state() && !c.is_message_call())
                .unwrap_or(force_update)
        {
            // Clone current database state
            let mut inner = ctx.journal().to_inner();
            let changes = inner.finalize();
            let mut snap = ctx.db().clone();
            snap.commit(changes);

            self.database = Arc::new(snap);
        }

        if force_update
            || self
                .last_opcode
                .map(|c| c.modifies_transient_storage() && !c.is_message_call())
                .unwrap_or(force_update)
        {
            let transient_storage = ctx.journal().transient_storage.clone();
            self.transient_storage = Arc::new(transient_storage);
        }
    }

    /// Check if this is a hook trigger call and record snapshot if so
    fn check_and_record_hook(
        &mut self,
        data: &[u8],
        interp: &Interpreter,
        _ctx: &mut EdbContext<DB>,
    ) {
        let address = self
            .current_frame_id()
            .and_then(|frame_id| self.trace.get(frame_id.trace_entry_id()))
            .map(|entry| entry.code_address)
            .unwrap_or(interp.input.target_address());

        let usid_opt = if data.len() >= 32 {
            U256::from_be_slice(&data[..32]).try_into().ok()
        } else {
            error!("KECCAK256 input data too short for snapshot, skipping");
            return;
        };

        let Some(usid) = usid_opt else {
            error!("Hook call data does not contain valid USID, skipping snapshot");
            return;
        };

        // Check variables that are valid at this point
        let Some(step) = self.analysis.get(&address).and_then(|a| a.usid_to_step.get(&usid)) else {
            error!(
                address=?address,
                usid=?usid,
                "No analysis step found for address and USID, skipping hook snapshot",
            );
            return;
        };

        // Collect values of accessible variables
        let mut locals = HashMap::new();
        for variable in step.accessible_variables() {
            if variable.declaration().state_variable {
                continue;
            }
            let uvid = variable.id();
            let name = variable.declaration().name.clone();
            locals.insert(name, self.uvid_values.get(&uvid).cloned());
        }

        // Update the last frame with this snapshot
        if let Some(current_frame_id) = self.current_frame_id() {
            if let Some(entry) = self.trace.get(current_frame_id.trace_entry_id()) {
                // Create hook snapshot
                let hook_snapshot = HookSnapshot {
                    target_address: entry.target,
                    bytecode_address: entry.code_address,
                    database: self.database.clone(),
                    transient_storage: self.transient_storage.clone(),
                    locals,
                    usid,
                    state_variables: HashMap::new(), // State variables can be filled in later
                };

                self.snapshots.update_last_frame_with_snapshot(current_frame_id, hook_snapshot);
            } else {
                error!("No trace entry found for frame {}", current_frame_id);
            }
        } else {
            error!("No current frame to update with hook snapshot");
        }
    }

    fn check_and_record_variable_update(
        &mut self,
        data: &[u8],
        interp: &Interpreter,
        _ctx: &mut EdbContext<DB>,
    ) {
        let address = self
            .current_frame_id()
            .and_then(|frame_id| self.trace.get(frame_id.trace_entry_id()))
            .map(|entry| entry.code_address)
            .unwrap_or(interp.input.target_address());

        // The data is decoded as (uint256 magic, uint256 uvid, abi.encode(value))
        // So the data will be organized as
        //      -32 .. 0 : [uint256 magic] (parsed before this function)
        //        0 .. 32: [uint256 uvid]
        //       32 .. 64: [offset] (should be 0x60 considering the first two uint256)
        //       64 .. 96: [length of encoded value]
        //       96 .. _ : [encoded value]
        if data.len() < 96 {
            error!(
                address=?address,
                "KECCAK256 input data too short for variable update value, skipping"
            );
            return;
        }

        let Some(uvid) = U256::from_be_slice(&data[..32]).try_into().ok() else {
            error!("Hook call data does not contain valid UVID, skipping snapshot");
            return;
        };

        let offset = U256::from_be_slice(&data[32..64]);
        if offset != U256::from(0x60) {
            error!(
                address=?address,
                uvid=?uvid,
                offset=?offset,
                "Unexpected offset for variable update value, skipping"
            );
            return;
        }

        let length = U256::from_be_slice(&data[64..96]);
        let length_usize = match usize::try_from(length) {
            Ok(l) => l,
            Err(_) => {
                error!(
                    address=?address,
                    uvid=?uvid,
                    length=?length,
                    "Variable update value length too large, skipping"
                );
                return;
            }
        };

        let decoded_data = &data[96..96 + length_usize];

        let Some(analysis) = self.analysis.get(&address) else {
            error!(
                address=?address,
                uvid=?uvid,
                "No analysis found for address, skipping variable update recording",
            );
            return;
        };
        let Some(variable) = analysis.uvid_to_variable.get(&uvid) else {
            error!(
                address=?address,
                uvid=?uvid,
                "No variable found for address and UVID, skipping variable update recording",
            );
            return;
        };

        let value =
            match decode_variable_value(&analysis.user_defined_types, variable, decoded_data) {
                Ok(v) => v,
                Err(e) => {
                    error!(
                        address=?address,
                        uvid=?uvid,
                        variable=?variable.declaration().type_descriptions.type_string,
                        type_name = ?variable.declaration().type_name,
                        data=?hex::encode(decoded_data),
                        error=?e,
                    );
                    return;
                }
            };

        debug!(
            uvid=?uvid,
            address=?address,
            variable=?variable.declaration().name,
            value=?value,
            "Found variable update",
        );

        self.uvid_values.insert(uvid, Arc::new(value.into()));
    }

    /// Check and apply creation hooks if the bytecode matches
    fn check_and_apply_creation_hooks(
        &mut self,
        inputs: &mut CreateInputs,
        ctx: &mut EdbContext<DB>,
    ) {
        // Get the nonce from the caller account
        let Ok(account) = ctx.journaled_state.load_account(inputs.caller) else {
            error!("Failed to load account for caller {:?}", inputs.caller);
            return;
        };

        // Calculate what address would be created using the built-in method
        let nonce = account.info.nonce;
        let predicted_address = inputs.created_address(nonce);

        for (original_bytecode, hooked_bytecode, constructor_args) in &self.creation_hooks {
            // Check if constructor arguments are at the tail of input bytes
            if inputs.init_code.len() >= constructor_args.len() {
                let input_args_start = inputs.init_code.len() - constructor_args.len();
                let input_args = &inputs.init_code[input_args_start..];

                // Check if constructor args match
                if input_args == constructor_args.as_ref() {
                    // Get the creation bytecode (without constructor args)
                    let input_bytecode = &inputs.init_code[..input_args_start];

                    // Check if bytecode is very similar to original
                    // For now, we do exact match, but could be made fuzzy
                    if input_bytecode == original_bytecode.as_ref() {
                        // Match found! Replace with hooked bytecode + constructor args
                        let mut new_init_code = Vec::from(hooked_bytecode.as_ref());
                        new_init_code.extend_from_slice(constructor_args.as_ref());
                        inputs.init_code = Bytes::from(new_init_code);

                        // Update creation schema
                        inputs.scheme = CreateScheme::Custom { address: predicted_address };

                        // Log the replacement
                        debug!(
                            "Replaced creation bytecode with hooked version for {:?} -> {:?}",
                            inputs.caller, predicted_address
                        );

                        break; // Found a match, no need to check other hooks
                    }
                }
            }
        }
    }

    /// Clear all recorded data
    pub fn clear(&mut self) {
        self.snapshots.snapshots.clear();
        self.frame_stack.clear();
        self.current_trace_id = 0;
    }
}

impl<'a, DB> Inspector<EdbContext<DB>> for HookSnapshotInspector<'a, DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone,
    <CacheDB<DB> as Database>::Error: Clone,
    <DB as Database>::Error: Clone,
{
    fn step(&mut self, interp: &mut Interpreter, ctx: &mut EdbContext<DB>) {
        // Get current opcode safely
        let opcode = unsafe { OpCode::new_unchecked(interp.bytecode.opcode()) };
        self.last_opcode = Some(opcode);

        if opcode != OpCode::KECCAK256 {
            // KECCAK256 is the only hooked opcode.
            return;
        }

        let Some(data) = interp.stack.pop().ok().and_then(|offset_u256| {
            let data = interp.stack.pop().ok().and_then(|len_u256| {
                let offset = usize::try_from(offset_u256).ok()?;
                let len = usize::try_from(len_u256).ok()?;
                let data = interp.memory.slice_len(offset, len);

                let _ = interp.stack.push(len_u256);
                Some(data)
            });

            let _ = interp.stack.push(offset_u256);
            data
        }) else {
            error!("Failed to read KECCAK256 input data from stack");
            return;
        };

        if data.len() < 32 {
            // Not enough data for at least two U256 values
            return;
        }

        let magic_number = U256::from_be_slice(&data[..32]);

        if magic_number == MAGIC_SNAPSHOT_NUMBER {
            self.check_and_record_hook(&data[32..], interp, ctx);
        } else if magic_number == MAGIC_VARIABLE_UPDATE_NUMBER {
            self.check_and_record_variable_update(&data[32..], interp, ctx);
        }
    }

    fn step_end(&mut self, _interp: &mut Interpreter, context: &mut EdbContext<DB>) {
        self.update_database(context, false);
    }

    fn call(
        &mut self,
        context: &mut EdbContext<DB>,
        _inputs: &mut CallInputs,
    ) -> Option<CallOutcome> {
        // Update database
        self.update_database(context, true);

        // Start tracking new execution frame for regular calls only
        self.push_frame(self.current_trace_id);
        self.current_trace_id += 1;

        None
    }

    fn call_end(
        &mut self,
        context: &mut EdbContext<DB>,
        inputs: &CallInputs,
        outcome: &mut CallOutcome,
    ) {
        // Update database
        self.update_database(context, true);

        let Some(frame_id) = self.pop_frame() else { return };

        let Some(entry) = self.trace.get(frame_id.trace_entry_id()) else { return };

        if entry.result != Some(outcome.into()) {
            // Mismatched call outcome
            error!(
                target_address = inputs.target_address.to_string(),
                bytecode_address = inputs.bytecode_address.to_string(),
                "Call outcome mismatch at frame {}: expected {:?}, got {:?} ({:?})",
                frame_id,
                entry.result,
                Into::<CallResult>::into(&outcome),
                outcome,
            );
        }
    }

    fn create(
        &mut self,
        context: &mut EdbContext<DB>,
        inputs: &mut CreateInputs,
    ) -> Option<CreateOutcome> {
        // Update database
        self.update_database(context, true);

        // Check and apply creation hooks if applicable
        self.check_and_apply_creation_hooks(inputs, context);

        // Start tracking new execution frame for contract creation
        self.push_frame(self.current_trace_id);
        self.current_trace_id += 1;

        None
    }

    fn create_end(
        &mut self,
        context: &mut EdbContext<DB>,
        _inputs: &CreateInputs,
        outcome: &mut CreateOutcome,
    ) {
        // Update database
        self.update_database(context, true);

        // Stop tracking current execution frame
        let Some(frame_id) = self.pop_frame() else { return };

        let Some(entry) = self.trace.get(frame_id.trace_entry_id()) else { return };

        // For creation, we only check the return status, not the actually bytecode, since we
        // will instrument the code
        if entry.result.as_ref().map(|r| r.result()) != Some(outcome.result.result) {
            // Mismatch create outcome
            error!(
                "Create outcome mismatch at frame {}: expected {:?}, got {:?}",
                frame_id, entry.result, outcome
            );
        }
    }
}

/// Pretty printing utilities for debugging
impl<DB> HookSnapshots<DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone,
    <CacheDB<DB> as Database>::Error: Clone,
    <DB as Database>::Error: Clone,
{
    /// Print comprehensive summary of hook snapshots
    pub fn print_summary(&self) {
        println!(
            "\n\x1b[36m╔══════════════════════════════════════════════════════════════════╗\x1b[0m"
        );
        println!(
            "\x1b[36m║              HOOK SNAPSHOT INSPECTOR SUMMARY                     ║\x1b[0m"
        );
        println!(
            "\x1b[36m╚══════════════════════════════════════════════════════════════════╝\x1b[0m\n"
        );

        // Overall statistics
        let total_frames = self.len();
        let hook_frames = self.get_frames_with_hooks().len();

        println!("\x1b[33m📊 Overall Statistics:\x1b[0m");
        println!("  Total frames tracked: \x1b[32m{total_frames}\x1b[0m");
        println!("  Frames with hooks: \x1b[32m{hook_frames}\x1b[0m");
        println!(
            "  Hook trigger rate: \x1b[32m{:.1}%\x1b[0m",
            if total_frames > 0 { hook_frames as f64 / total_frames as f64 * 100.0 } else { 0.0 }
        );

        if self.is_empty() {
            println!("\n\x1b[90m  No execution frames were tracked.\x1b[0m");
            return;
        }

        println!("\n\x1b[33m🎯 Hook Trigger Details:\x1b[0m");
        println!(
            "\x1b[90m─────────────────────────────────────────────────────────────────\x1b[0m"
        );

        // Group snapshots by frame ID
        use std::collections::HashMap;
        let mut frame_groups: HashMap<ExecutionFrameId, Vec<&HookSnapshot<DB>>> = HashMap::new();
        let mut frame_order = Vec::new();

        for (frame_id, snapshot) in &self.snapshots {
            if !frame_groups.contains_key(frame_id) {
                frame_order.push(*frame_id);
            }

            match snapshot {
                Some(hook_snapshot) => {
                    frame_groups.entry(*frame_id).or_default().push(hook_snapshot);
                }
                None => {
                    // Ensure frame exists in map even if empty
                    frame_groups.entry(*frame_id).or_default();
                }
            }
        }

        // Print grouped results in original order
        for (display_idx, frame_id) in frame_order.iter().enumerate() {
            let hooks = frame_groups.get(frame_id).unwrap();

            if hooks.is_empty() {
                // Frame with no hooks
                println!(
                    "  \x1b[90m[{:3}] Frame {}\x1b[0m (trace.{}, re-entry {}) - No hooks",
                    display_idx,
                    frame_id,
                    frame_id.trace_entry_id(),
                    frame_id.re_entry_count()
                );
            } else {
                // Frame with hooks - collect all USIDs in execution order (no sorting)
                let usids: Vec<_> = hooks.iter().map(|h| h.usid).collect();
                let hook_count = hooks.len();
                #[allow(deprecated)]
                let addresses: std::collections::HashSet<_> =
                    hooks.iter().map(|h| h.bytecode_address).collect();

                println!(
                    "\n  \x1b[32m[{:3}] Frame {}\x1b[0m (trace.{}, re-entry {})",
                    display_idx,
                    frame_id,
                    frame_id.trace_entry_id(),
                    frame_id.re_entry_count()
                );
                println!(
                    "       └─ \x1b[33m{} Hook{} Triggered\x1b[0m",
                    hook_count,
                    if hook_count == 1 { "" } else { "s" }
                );

                // Show addresses (usually just one per frame)
                for address in &addresses {
                    println!("          ├─ Address: \x1b[36m{address:?}\x1b[0m");
                }

                // Show USIDs in execution order with smart formatting
                if usids.len() == 1 {
                    println!("          └─ USID: \x1b[36m{}\x1b[0m", usids[0]);
                } else if usids.len() <= 10 {
                    // Show all USIDs for small lists
                    let usid_list: Vec<String> = usids.iter().map(|u| u.to_string()).collect();
                    println!("          └─ USIDs: \x1b[36m[{}]\x1b[0m", usid_list.join(", "));
                } else {
                    // For large lists, show first few, count, and last few
                    let first_few: Vec<String> =
                        usids.iter().take(3).map(|u| u.to_string()).collect();
                    let last_few: Vec<String> =
                        usids.iter().rev().take(3).rev().map(|u| u.to_string()).collect();

                    if first_few.last() == last_few.first() {
                        // Handle overlap case (shouldn't happen with take(3) for >10 items, but defensive)
                        println!(
                            "          └─ USIDs: \x1b[36m[{} ... {} total]\x1b[0m",
                            first_few.join(", "),
                            usids.len()
                        );
                    } else {
                        println!(
                            "          └─ USIDs: \x1b[36m[{}, ... {}, {} total]\x1b[0m",
                            first_few.join(", "),
                            last_few.join(", "),
                            usids.len()
                        );
                    }
                }
            }
        }

        println!(
            "\n\x1b[90m─────────────────────────────────────────────────────────────────\x1b[0m"
        );
        println!("\x1b[33m💡 Magic Snapshot Number:\x1b[0m {MAGIC_SNAPSHOT_NUMBER:?}");
    }
}

/// Decode the variable value from the given ABI-encoded data according to the variable declaration.
///
/// This function takes raw ABI-encoded data and decodes it according to the variable's
/// type information from its declaration. It handles both primitive Solidity types
/// (uint, address, bool, etc.) and user-defined types (structs, enums).
///
/// # Arguments
/// * `user_defined_types` - Mapping of type IDs to user-defined type references for resolving custom types
/// * `variable` - The variable reference containing the declaration and type information
/// * `data` - The ABI-encoded variable value as raw bytes
///
/// # Returns
/// The decoded variable value as a [`DynSolValue`] that can be used in expression evaluation
///
/// # Errors
/// Returns an error if:
/// - The variable declaration lacks type information
/// - The type cannot be resolved from the declaration
/// - The data cannot be ABI-decoded according to the resolved type
///
/// # Example
/// ```rust,ignore
/// let decoded = decode_variable_value(&user_types, &variable_ref, &encoded_data)?;
/// match decoded {
///     DynSolValue::Uint(val, _) => println!("Uint value: {}", val),
///     DynSolValue::Address(addr) => println!("Address: {}", addr),
///     _ => println!("Other type: {:?}", decoded),
/// }
/// ```
pub fn decode_variable_value(
    user_defined_types: &HashMap<usize, UserDefinedTypeRef>,
    variable: &VariableRef,
    data: &[u8],
) -> Result<DynSolValue> {
    let type_name = variable
        .type_name()
        .as_ref()
        .ok_or(eyre::eyre!("Failed to get variable type: no type name in the declaration"))?;
    let Some(variable_type): Option<DynSolType> = dyn_sol_type(user_defined_types, type_name)
    else {
        return Err(eyre::eyre!("Failed to get variable type: no type string in the declaration"));
    };
    let value = variable_type
        .abi_decode(data)
        .map_err(|e| eyre::eyre!("Failed to decode variable value: {}", e))?;
    Ok(value)
}
