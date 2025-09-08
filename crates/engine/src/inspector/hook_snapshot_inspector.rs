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

use alloy_primitives::{Address, Bytes, U256};
use edb_common::{
    types::{ExecutionFrameId, Trace},
    EdbContext,
};
use eyre::Result;
use foundry_compilers::{artifacts::Contract, Artifact};
use revm::{
    context::{ContextTr, CreateScheme, JournalTr, LocalContextTr},
    database::CacheDB,
    interpreter::{CallInput, CallInputs, CallOutcome, CreateInputs, CreateOutcome},
    Database, DatabaseCommit, DatabaseRef, Inspector,
};
use serde::{Deserialize, Serialize};
use std::{
    ops::{Deref, DerefMut},
    sync::Arc,
};
use tracing::{debug, error};

use crate::USID;

/// Magic trigger address that causes snapshots to be taken
/// 0x0000000000000000000000000000000000023333
pub const HOOK_TRIGGER_ADDRESS: Address = Address::new([
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x02, 0x33, 0x33,
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

    /// Collection of hook snapshots
    pub snapshots: HookSnapshots<DB>,

    /// Stack to track current execution frames
    frame_stack: Vec<ExecutionFrameId>,

    /// Current trace entry counter
    current_trace_id: usize,

    /// Creation hooks (original contract bytecode, hooked bytecode, constructor args)
    creation_hooks: Vec<(Bytes, Bytes, Bytes)>,
}

impl<'a, DB> HookSnapshotInspector<'a, DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone,
    <CacheDB<DB> as Database>::Error: Clone,
    <DB as Database>::Error: Clone,
{
    /// Create a new hook snapshot inspector
    pub fn new(trace: &'a Trace) -> Self {
        Self {
            trace,
            snapshots: HookSnapshots::default(),
            frame_stack: Vec::new(),
            current_trace_id: 0,
            creation_hooks: Vec::new(),
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

    /// Check if this is a hook trigger call and record snapshot if so
    fn check_and_record_hook(&mut self, inputs: &CallInputs, ctx: &mut EdbContext<DB>) {
        // Check if this is a call to the magic trigger address
        if inputs.target_address != HOOK_TRIGGER_ADDRESS {
            return;
        }

        // Extract usid from call data (assume it's a single U256)
        let input_data = match &inputs.input {
            CallInput::SharedBuffer(range) => ctx
                .local()
                .shared_memory_buffer_slice(range.clone())
                .map(|slice| slice.to_vec())
                .unwrap_or_else(|| Vec::new()),
            CallInput::Bytes(bytes) => bytes.to_vec().clone(),
        };
        let usid_opt = if input_data.len() >= 32 {
            U256::from_be_slice(&input_data[..32]).try_into().ok()
        } else {
            None
        };
        let usid = match usid_opt {
            Some(usid) => usid,
            None => {
                error!("Hook call data does not contain valid USID, skipping snapshot");
                return;
            }
        };

        // Clone current database state
        let mut inner = ctx.journal().to_inner();
        let changes = inner.finalize();
        let mut snap = ctx.db().clone();
        snap.commit(changes);

        // Update the last frame with this snapshot
        if let Some(current_frame_id) = self.current_frame_id() {
            if let Some(entry) = self.trace.get(current_frame_id.trace_entry_id()) {
                // Create hook snapshot
                let hook_snapshot = HookSnapshot {
                    target_address: entry.target,
                    bytecode_address: entry.code_address,
                    database: Arc::new(snap),
                    usid,
                };

                self.snapshots.update_last_frame_with_snapshot(current_frame_id, hook_snapshot);
            } else {
                error!("No trace entry found for frame {}", current_frame_id);
            }
        } else {
            error!("No current frame to update with hook snapshot");
        }
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
    fn call(
        &mut self,
        context: &mut EdbContext<DB>,
        inputs: &mut CallInputs,
    ) -> Option<CallOutcome> {
        // Check if this is a hook trigger - if so, record snapshot but don't create frame
        if inputs.target_address == HOOK_TRIGGER_ADDRESS {
            self.check_and_record_hook(inputs, context);
            return None; // Don't create a frame for hook calls
        }

        // Start tracking new execution frame for regular calls only
        self.push_frame(self.current_trace_id);
        self.current_trace_id += 1;
        None
    }

    fn call_end(
        &mut self,
        _context: &mut EdbContext<DB>,
        inputs: &CallInputs,
        outcome: &mut CallOutcome,
    ) {
        // Only pop frame for non-hook calls
        if inputs.target_address != HOOK_TRIGGER_ADDRESS {
            let Some(frame_id) = self.pop_frame() else { return };

            let Some(entry) = self.trace.get(frame_id.trace_entry_id()) else { return };

            println!("MDZZ we are here 1!");
            if entry.result != Some(outcome.into()) {
                // Mismatched call outcome
                error!(
                    "Call outcome mismatch at frame {}: expected {:?}, got {:?}",
                    frame_id, entry.result, outcome
                );
            }
        }
    }

    fn create(
        &mut self,
        context: &mut EdbContext<DB>,
        inputs: &mut CreateInputs,
    ) -> Option<CreateOutcome> {
        // Check and apply creation hooks if applicable
        self.check_and_apply_creation_hooks(inputs, context);

        // Start tracking new execution frame for contract creation
        self.push_frame(self.current_trace_id);
        self.current_trace_id += 1;
        None
    }

    fn create_end(
        &mut self,
        _context: &mut EdbContext<DB>,
        _inputs: &CreateInputs,
        outcome: &mut CreateOutcome,
    ) {
        // Stop tracking current execution frame
        let Some(frame_id) = self.pop_frame() else { return };

        let Some(entry) = self.trace.get(frame_id.trace_entry_id()) else { return };
        println!("MDZZ we are here 2!");

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
        println!("  Total frames tracked: \x1b[32m{}\x1b[0m", total_frames);
        println!("  Frames with hooks: \x1b[32m{}\x1b[0m", hook_frames);
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
                    println!("          ├─ Address: \x1b[36m{:?}\x1b[0m", address);
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
        println!("\x1b[33m💡 Magic Address:\x1b[0m {HOOK_TRIGGER_ADDRESS:?}");
    }
}
