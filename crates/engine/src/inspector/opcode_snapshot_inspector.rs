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

//! Opcode snapshot inspector for recording detailed VM state at each instruction
//!
//! This inspector captures instruction-level execution details including:
//! - Current instruction offset (PC)
//! - Contract address
//! - Current opcode  
//! - Memory state (with Arc sharing for unchanged memory)
//! - Stack state (always cloned as most opcodes modify it)
//! - Call data (with Arc sharing across same execution context)
//!
//! Memory optimization: Uses Arc to share memory and calldata when unchanged,
//! reducing memory usage for large execution traces.

use alloy_primitives::{Address, Bytes, U256};
use edb_common::{
    types::{ExecutionFrameId, Trace},
    EdbContext, OpcodeTr,
};
use revm::{
    bytecode::opcode::OpCode,
    context::{ContextTr, LocalContextTr},
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
    borrow::Borrow,
    collections::{HashMap, HashSet},
    ops::{Deref, DerefMut},
    sync::Arc,
};
use tracing::error;

/// Single opcode execution snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpcodeSnapshot<DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone,
    <CacheDB<DB> as Database>::Error: Clone,
    <DB as Database>::Error: Clone,
{
    /// Program counter (instruction offset)
    pub pc: usize,
    /// Target address that triggered the hook
    pub target_address: Address,
    /// Bytecode address that the current snapshot is running
    pub bytecode_address: Address,
    /// Current opcode
    pub opcode: u8,
    /// Memory state (shared via Arc when unchanged)
    pub memory: Arc<Vec<u8>>,
    /// Stack state (always cloned as most opcodes modify it)
    pub stack: Vec<U256>,
    /// Call data for this execution context (shared via Arc within same context)
    pub calldata: Arc<Bytes>,
    /// Database state (shared via Arc within same context)
    pub database: Arc<CacheDB<DB>>,
    /// Transition storage
    pub transient_storage: Arc<TransientStorage>,
}

/// Collection of opcode snapshots
#[derive(Debug, Clone)]
pub struct OpcodeSnapshots<DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone,
    <CacheDB<DB> as Database>::Error: Clone,
    <DB as Database>::Error: Clone,
{
    inner: HashMap<ExecutionFrameId, Vec<OpcodeSnapshot<DB>>>,
}

impl<DB> Default for OpcodeSnapshots<DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone,
    <CacheDB<DB> as Database>::Error: Clone,
    <DB as Database>::Error: Clone,
{
    fn default() -> Self {
        Self { inner: HashMap::new() }
    }
}

impl<DB> Deref for OpcodeSnapshots<DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone,
    <CacheDB<DB> as Database>::Error: Clone,
    <DB as Database>::Error: Clone,
{
    type Target = HashMap<ExecutionFrameId, Vec<OpcodeSnapshot<DB>>>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<DB> DerefMut for OpcodeSnapshots<DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone,
    <CacheDB<DB> as Database>::Error: Clone,
    <DB as Database>::Error: Clone,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

/// Frame state tracking for memory optimization
#[derive(Debug, Clone)]
struct FrameState {
    /// Last captured memory state
    last_memory: Arc<Vec<u8>>,
    /// Last captured calldata
    last_calldata: Arc<Bytes>,
}

/// Inspector that records detailed opcode execution snapshots
#[derive(Debug)]
pub struct OpcodeSnapshotInspector<'a, DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone,
    <CacheDB<DB> as Database>::Error: Clone,
    <DB as Database>::Error: Clone,
{
    /// The trace of the current tx
    trace: &'a Trace,

    /// Map from execution frame ID to list of snapshots
    pub snapshots: OpcodeSnapshots<DB>,

    /// Set of addresses to exclude from recording (verified source code)
    pub excluded_addresses: HashSet<Address>,

    /// Stack to track current execution frames
    frame_stack: Vec<ExecutionFrameId>,

    /// Current trace entry counter (to match with call tracer)
    current_trace_id: usize,

    /// Frame state for each active frame (for memory optimization)
    frame_states: HashMap<ExecutionFrameId, FrameState>,

    /// Database context
    database: Arc<CacheDB<DB>>,

    /// Transition storage
    transition_storage: Arc<TransientStorage>,

    /// Last opcode
    last_opcode: Option<OpCode>,
}

impl<'a, DB> OpcodeSnapshotInspector<'a, DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone,
    <CacheDB<DB> as Database>::Error: Clone,
    <DB as Database>::Error: Clone,
{
    /// Create a new opcode snapshot inspector
    pub fn new(ctx: &EdbContext<DB>, trace: &'a Trace) -> Self {
        Self {
            trace,
            snapshots: OpcodeSnapshots::<DB>::default(),
            excluded_addresses: HashSet::new(),
            frame_stack: Vec::new(),
            current_trace_id: 0,
            frame_states: HashMap::new(),
            database: Arc::new(ctx.db().clone()),
            transition_storage: Arc::new(TransientStorage::default()),
            last_opcode: None,
        }
    }

    /// Create inspector with excluded addresses
    pub fn with_excluded_addresses(&mut self, excluded_addresses: HashSet<Address>) {
        self.excluded_addresses = excluded_addresses;
    }

    /// Consume the inspector and return the collected snapshots
    pub fn into_snapshots(self) -> OpcodeSnapshots<DB> {
        self.snapshots
    }

    /// Add an address to exclude from recording
    pub fn exclude_address(&mut self, address: Address) {
        self.excluded_addresses.insert(address);
    }

    /// Get the current execution frame ID
    fn current_frame_id(&self) -> Option<ExecutionFrameId> {
        self.frame_stack.last().copied()
    }

    /// Check if we should record steps for the given address
    fn should_record(&self, address: Address) -> bool {
        !self.excluded_addresses.contains(&address)
    }

    /// Update storage
    fn update_storage(&mut self, _interp: &Interpreter, ctx: &mut EdbContext<DB>) {
        let Some(last_opcode) = self.last_opcode else { return };

        if last_opcode.modifies_evm_state() {
            let mut inner = ctx.journal().to_inner();
            let changes = inner.finalize();
            let mut snap = ctx.db().clone();
            snap.commit(changes);
            self.database = Arc::new(snap);
        }

        if last_opcode.modifies_transient_storage() {
            let transient_storage = ctx.journal().transient_storage.clone();
            self.transition_storage = Arc::new(transient_storage);
        }
    }

    /// Record a snapshot at the current step
    fn record_snapshot(&mut self, interp: &Interpreter, ctx: &mut EdbContext<DB>) {
        // Get current opcode safely
        let opcode = unsafe { OpCode::new_unchecked(interp.bytecode.opcode()) };

        // Update last opcode
        self.last_opcode = Some(opcode);

        // Get current frame
        let Some(frame_id) = self.current_frame_id() else {
            return;
        };

        // Check if we should record for this address
        let contract_address =
            interp.input.bytecode_address().cloned().unwrap_or(interp.input.target_address());
        if !self.should_record(contract_address) {
            return;
        }

        let address = interp.input.target_address();

        // Get or create frame state
        let frame_state = self.frame_states.get(&frame_id);

        // Get memory - reuse Arc if unchanged
        let memory = if let Some(state) = frame_state {
            let mem_ref = interp.memory.borrow();
            let current_memory = mem_ref.context_memory();
            if current_memory.len() == state.last_memory.len()
                && &*current_memory == state.last_memory.as_slice()
            {
                // Memory unchanged, reuse Arc
                state.last_memory.clone()
            } else {
                // Memory changed, create new Arc
                Arc::new(current_memory.to_vec())
            }
        } else {
            // First snapshot in frame
            Arc::new(interp.memory.borrow().context_memory().to_vec())
        };

        // Get calldata - reuse Arc if in same frame
        let calldata = if let Some(state) = frame_state {
            state.last_calldata.clone()
        } else {
            // First snapshot in frame, get calldata
            match interp.input.input() {
                revm::interpreter::CallInput::SharedBuffer(range) => Arc::new(
                    ctx.local()
                        .shared_memory_buffer_slice(range.clone())
                        .map(|slice| Bytes::from(slice.to_vec()))
                        .unwrap_or_else(Bytes::new),
                ),
                revm::interpreter::CallInput::Bytes(bytes) => Arc::new(bytes.clone()),
            }
        };

        // Create snapshot (stack is always cloned as it changes frequently)
        let entry = self.trace.get(frame_id.trace_entry_id());
        let snapshot = OpcodeSnapshot {
            pc: interp.bytecode.pc(),
            bytecode_address: entry.map(|t| t.code_address).unwrap_or(address),
            target_address: entry.map(|t| t.target).unwrap_or(address),
            opcode: opcode.get(),
            memory: memory.clone(),
            stack: interp.stack.data().clone(),
            calldata: calldata.clone(),
            database: self.database.clone(),
            transient_storage: self.transition_storage.clone(),
        };

        // Add to snapshots for this frame
        self.snapshots.entry(frame_id).or_default().push(snapshot);

        // Update frame state for next snapshot
        self.frame_states
            .insert(frame_id, FrameState { last_memory: memory, last_calldata: calldata });
    }

    /// Start tracking a new execution frame
    fn push_frame(&mut self, trace_id: usize) {
        let frame_id = ExecutionFrameId::new(trace_id, 0);
        self.frame_stack.push(frame_id);

        // Initialize empty snapshot list for this frame if not exists
        self.snapshots.entry(frame_id).or_default();
    }

    /// Stop tracking current execution frame and increment re-entry count
    fn pop_frame(&mut self) -> Option<ExecutionFrameId> {
        if let Some(frame_id) = self.frame_stack.pop() {
            // Clean up frame state
            self.frame_states.remove(&frame_id);

            // Increment re-entry count for parent frame if it exists
            if let Some(parent_frame_id) = self.frame_stack.last_mut() {
                parent_frame_id.increment_re_entry();
            }

            Some(frame_id)
        } else {
            None
        }
    }

    /// Get all recorded snapshots for a specific frame
    pub fn get_frame_snapshots(
        &self,
        frame_id: ExecutionFrameId,
    ) -> Option<&Vec<OpcodeSnapshot<DB>>> {
        self.snapshots.get(&frame_id)
    }

    /// Get all execution frame IDs that have recorded snapshots
    pub fn get_recorded_frames(&self) -> Vec<ExecutionFrameId> {
        self.snapshots.keys().copied().collect()
    }

    /// Clear all recorded data
    pub fn clear(&mut self) {
        self.snapshots.clear();
        self.frame_stack.clear();
        self.frame_states.clear();
        self.current_trace_id = 0;
    }
}

impl<'a, DB> Inspector<EdbContext<DB>> for OpcodeSnapshotInspector<'a, DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone,
    <CacheDB<DB> as Database>::Error: Clone,
    <DB as Database>::Error: Clone,
{
    fn step(&mut self, interp: &mut Interpreter, context: &mut EdbContext<DB>) {
        // Record snapshot BEFORE executing the opcode
        self.record_snapshot(interp, context);
    }

    fn step_end(&mut self, interp: &mut Interpreter, context: &mut EdbContext<DB>) {
        // Record snapshot AFTER executing the opcode
        self.update_storage(interp, context);
    }

    fn call(
        &mut self,
        _context: &mut EdbContext<DB>,
        _inputs: &mut CallInputs,
    ) -> Option<CallOutcome> {
        // Start tracking new execution frame
        self.push_frame(self.current_trace_id);
        self.current_trace_id += 1;
        None
    }

    fn call_end(
        &mut self,
        _context: &mut EdbContext<DB>,
        _inputs: &CallInputs,
        outcome: &mut CallOutcome,
    ) {
        // Stop tracking current execution frame
        let Some(frame_id) = self.pop_frame() else { return };

        let Some(entry) = self.trace.get(frame_id.trace_entry_id()) else { return };

        if entry.result != Some(outcome.into()) {
            // Mismatch in expected outcome, log error
            error!(
                "Call outcome mismatch in frame {:?}: expected {:?}, got {:?}",
                frame_id, entry.result, outcome
            );
        }
    }

    fn create(
        &mut self,
        _context: &mut EdbContext<DB>,
        _inputs: &mut CreateInputs,
    ) -> Option<CreateOutcome> {
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

        if entry.result != Some(outcome.into()) {
            // Mismatch in expected outcome, log error
            error!(
                "Create outcome mismatch in frame {:?}: expected {:?}, got {:?}",
                frame_id, entry.result, outcome
            );
        }
    }
}

/// Pretty printing utilities for debugging
impl<DB> OpcodeSnapshots<DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone,
    <CacheDB<DB> as Database>::Error: Clone,
    <DB as Database>::Error: Clone,
{
    /// Print comprehensive summary with frame details
    pub fn print_summary(&self) {
        println!(
            "\n\x1b[36m╔══════════════════════════════════════════════════════════════════╗\x1b[0m"
        );
        println!(
            "\x1b[36m║              OPCODE SNAPSHOT INSPECTOR SUMMARY                   ║\x1b[0m"
        );
        println!(
            "\x1b[36m╚══════════════════════════════════════════════════════════════════╝\x1b[0m\n"
        );

        // Overall statistics
        let total_frames = self.len();
        let total_snapshots: usize = self.values().map(|v| v.len()).sum();

        println!("\x1b[33m📊 Overall Statistics:\x1b[0m");
        println!("  Total frames recorded: \x1b[32m{total_frames}\x1b[0m");
        println!("  Total snapshots recorded:  \x1b[32m{total_snapshots}\x1b[0m");

        if self.is_empty() {
            println!("\n\x1b[90m  No opcode snapshots were recorded.\x1b[0m");
            return;
        }

        // Calculate memory sharing statistics
        let mut total_memory_instances = 0;
        let mut unique_memory_instances = HashSet::new();
        let mut total_calldata_instances = 0;
        let mut unique_calldata_instances = HashSet::new();

        for snapshots in self.values() {
            for snapshot in snapshots {
                total_memory_instances += 1;
                unique_memory_instances.insert(Arc::as_ptr(&snapshot.memory) as usize);
                total_calldata_instances += 1;
                unique_calldata_instances.insert(Arc::as_ptr(&snapshot.calldata) as usize);
            }
        }

        let memory_sharing_ratio = if total_memory_instances > 0 {
            (total_memory_instances - unique_memory_instances.len()) as f64
                / total_memory_instances as f64
                * 100.0
        } else {
            0.0
        };

        let calldata_sharing_ratio = if total_calldata_instances > 0 {
            (total_calldata_instances - unique_calldata_instances.len()) as f64
                / total_calldata_instances as f64
                * 100.0
        } else {
            0.0
        };

        println!("\n\x1b[33m💾 Memory Optimization:\x1b[0m");
        println!("  Memory - Unique instances: \x1b[32m{}\x1b[0m / Total refs: \x1b[32m{}\x1b[0m (Sharing: \x1b[32m{:.1}%\x1b[0m)", 
            unique_memory_instances.len(), total_memory_instances, memory_sharing_ratio);
        println!("  Calldata - Unique instances: \x1b[32m{}\x1b[0m / Total refs: \x1b[32m{}\x1b[0m (Sharing: \x1b[32m{:.1}%\x1b[0m)", 
            unique_calldata_instances.len(), total_calldata_instances, calldata_sharing_ratio);

        println!("\n\x1b[33m📋 Frame Details:\x1b[0m");
        println!(
            "\x1b[90m─────────────────────────────────────────────────────────────────\x1b[0m"
        );

        // Sort frames for consistent output
        let mut sorted_frames: Vec<_> = self.iter().collect();
        sorted_frames
            .sort_by_key(|(frame_id, _)| (frame_id.trace_entry_id(), frame_id.re_entry_count()));

        for (frame_id, snapshots) in sorted_frames {
            // Frame header with color coding based on snapshot count
            let color = if snapshots.is_empty() {
                "\x1b[90m" // Gray for empty
            } else if snapshots.len() < 10 {
                "\x1b[32m" // Green for small
            } else if snapshots.len() < 100 {
                "\x1b[33m" // Yellow for medium
            } else {
                "\x1b[31m" // Red for large
            };

            println!(
                "\n  {}Frame {}\x1b[0m (trace.{}, re-entry {})",
                color,
                frame_id,
                frame_id.trace_entry_id(),
                frame_id.re_entry_count()
            );
            println!("  └─ Snapshots: \x1b[36m{}\x1b[0m", snapshots.len());

            if !snapshots.is_empty() {
                // Show first few and last few snapshots for context
                let preview_count = 3.min(snapshots.len());

                // First few snapshots
                println!("     \x1b[90mFirst {preview_count} snapshots:\x1b[0m");
                for (i, snapshot) in snapshots.iter().take(preview_count).enumerate() {
                    self.print_snapshot_line(i, snapshot, "     ");
                }

                // Last few snapshots if there are more
                if snapshots.len() > preview_count * 2 {
                    println!(
                        "     \x1b[90m... {} more snapshots ...\x1b[0m",
                        snapshots.len() - preview_count * 2
                    );
                    println!("     \x1b[90mLast {preview_count} snapshots:\x1b[0m");
                    let start_idx = snapshots.len() - preview_count;
                    for (i, snapshot) in snapshots.iter().skip(start_idx).enumerate() {
                        self.print_snapshot_line(start_idx + i, snapshot, "     ");
                    }
                } else if snapshots.len() > preview_count {
                    // Show remaining snapshots
                    for (i, snapshot) in snapshots.iter().skip(preview_count).enumerate() {
                        self.print_snapshot_line(preview_count + i, snapshot, "     ");
                    }
                }

                // Summary stats for this frame
                let total_memory: usize = snapshots.iter().map(|s| s.memory.len()).sum();
                let avg_stack_depth: f64 = snapshots.iter().map(|s| s.stack.len()).sum::<usize>()
                    as f64
                    / snapshots.len() as f64;

                println!("     \x1b[90m├─ Avg stack depth: {avg_stack_depth:.1}\x1b[0m");
                println!("     \x1b[90m└─ Total memory used: {total_memory} bytes\x1b[0m");
            }
        }

        println!(
            "\n\x1b[90m─────────────────────────────────────────────────────────────────\x1b[0m"
        );
    }

    /// Helper to print a single snapshot line
    fn print_snapshot_line(&self, index: usize, snapshot: &OpcodeSnapshot<DB>, indent: &str) {
        let opcode = unsafe { OpCode::new_unchecked(snapshot.opcode) };
        let opcode_str = opcode.as_str().to_string();

        #[allow(deprecated)]
        let addr_short = format!("{:?}", snapshot.bytecode_address);
        let addr_display = if addr_short.len() > 10 {
            format!("{}...{}", &addr_short[0..6], &addr_short[addr_short.len() - 4..])
        } else {
            addr_short
        };

        println!(
            "{}  [{:4}] PC={:5} \x1b[94m{:18}\x1b[0m @ \x1b[37m{}\x1b[0m | Stack:{:2} Mem:{:6}B",
            indent,
            index,
            snapshot.pc,
            opcode_str,
            addr_display,
            snapshot.stack.len(),
            snapshot.memory.len()
        );
    }
}
