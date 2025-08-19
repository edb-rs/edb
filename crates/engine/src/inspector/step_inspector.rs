//! Execution step inspector for recording detailed VM state at each instruction
//!
//! This inspector captures instruction-level execution details including:
//! - Current instruction offset (PC)
//! - Contract address
//! - Current opcode  
//! - Memory state
//! - Stack state
//! - Call data
//!
//! The collected data is organized by execution frame ID, where a frame ID is
//! a tuple (x, y) where x is the trace entry ID and y is the re-entry count.

use alloy_primitives::{Address, Bytes, U256};
use revm::{
    bytecode::opcode::OpCode,
    context::{ContextTr, LocalContextTr},
    interpreter::{
        interpreter_types::{InputsTr, Jumps},
        CallInputs, CallOutcome, CreateInputs, CreateOutcome, Interpreter,
    },
    Inspector,
};
use serde::de;
use std::{
    borrow::Borrow,
    collections::{HashMap, HashSet},
    hash::Hash,
    ops::{Deref, DerefMut},
};

/// Execution frame identifier
/// (trace_entry_id, re_entry_count)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExecutionFrameId(pub usize, pub usize);

impl std::fmt::Display for ExecutionFrameId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.0, self.1)
    }
}

/// Single execution step record
#[derive(Debug, Clone)]
pub struct ExecutionStepRecord {
    /// Program counter (instruction offset)
    pub pc: usize,
    /// Contract address being executed
    pub contract_address: Address,
    /// Current opcode
    pub opcode: OpCode,
    /// Memory state (cloned for safety)
    pub memory: Vec<u8>,
    /// Stack state (cloned for safety)
    pub stack: Vec<U256>,
    /// Call data for this execution context
    pub calldata: Bytes,
}

/// Collection of execution step records
#[derive(Debug, Default, Clone)]
pub struct ExecutionStepRecords {
    inner: HashMap<ExecutionFrameId, Vec<ExecutionStepRecord>>,
}

impl Deref for ExecutionStepRecords {
    type Target = HashMap<ExecutionFrameId, Vec<ExecutionStepRecord>>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for ExecutionStepRecords {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

/// Inspector that records detailed execution steps
#[derive(Debug, Default)]
pub struct ExecutionStepInspector {
    /// Map from execution frame ID to list of step records
    pub step_records: ExecutionStepRecords,

    /// Set of addresses to exclude from recording (verified source code)
    pub excluded_addresses: HashSet<Address>,

    /// Stack to track current execution frames
    frame_stack: Vec<ExecutionFrameId>,

    /// Current trace entry counter (to match with call tracer)
    current_trace_id: usize,
}

impl ExecutionStepInspector {
    /// Create a new execution step inspector
    pub fn new() -> Self {
        Self::default()
    }

    /// Create inspector with excluded addresses
    pub fn with_excluded_addresses(excluded_addresses: HashSet<Address>) -> Self {
        Self { excluded_addresses, ..Default::default() }
    }

    /// Consume the inspector and return the collected step records
    pub fn into_step_records(self) -> ExecutionStepRecords {
        self.step_records
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

    /// Start tracking a new execution frame
    fn push_frame(&mut self, trace_id: usize) {
        let frame_id = ExecutionFrameId(trace_id, 0);
        self.frame_stack.push(frame_id);

        // Initialize empty record list for this frame if not exists
        if !self.step_records.contains_key(&frame_id) {
            self.step_records.insert(frame_id, Vec::new());
        }
    }

    /// Stop tracking current execution frame and increment re-entry count
    fn pop_frame(&mut self) {
        let _ = self.frame_stack.pop(); // Discard current frame
        if let Some(frame_id) = self.frame_stack.last_mut() {
            frame_id.1 += 1; // Increment re-entry count
        }
    }

    /// Record an execution step for the current frame
    fn record_step(&mut self, interp: &Interpreter, ctx: &mut impl ContextTr) {
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

        // Get current opcode safely
        let opcode = unsafe { OpCode::new_unchecked(interp.bytecode.opcode()) };

        // Get calldata using the context for shared memory buffer access
        let calldata = match interp.input.input() {
            revm::interpreter::CallInput::SharedBuffer(range) => {
                // Use context to access the shared memory buffer
                ctx.local()
                    .shared_memory_buffer_slice(range.clone())
                    .map(|slice| Bytes::from(slice.to_vec()))
                    .unwrap_or_else(|| Bytes::new())
            }
            revm::interpreter::CallInput::Bytes(bytes) => bytes.clone(),
        };

        // Create step record
        let step_record = ExecutionStepRecord {
            pc: interp.bytecode.pc(),
            contract_address,
            opcode,
            memory: interp.memory.borrow().context_memory().to_vec(),
            stack: interp.stack.data().clone(),
            calldata,
        };

        // Add to records for this frame
        self.step_records.entry(frame_id).or_default().push(step_record);
    }

    /// Get all recorded steps for a specific frame
    pub fn get_frame_steps(&self, frame_id: ExecutionFrameId) -> Option<&Vec<ExecutionStepRecord>> {
        self.step_records.get(&frame_id)
    }

    /// Get all execution frame IDs that have recorded steps
    pub fn get_recorded_frames(&self) -> Vec<ExecutionFrameId> {
        self.step_records.keys().copied().collect()
    }

    /// Clear all recorded data
    pub fn clear(&mut self) {
        self.step_records.clear();
        self.frame_stack.clear();
        self.current_trace_id = 0;
    }
}

impl<CTX: ContextTr> Inspector<CTX> for ExecutionStepInspector {
    fn step(&mut self, interp: &mut Interpreter, context: &mut CTX) {
        self.record_step(interp, context);
    }

    fn call(&mut self, _context: &mut CTX, _inputs: &mut CallInputs) -> Option<CallOutcome> {
        // Start tracking new execution frame
        self.push_frame(self.current_trace_id);
        self.current_trace_id += 1;
        None
    }

    fn call_end(&mut self, _context: &mut CTX, _inputs: &CallInputs, _outcome: &mut CallOutcome) {
        // Stop tracking current execution frame
        self.pop_frame();
    }

    fn create(&mut self, _context: &mut CTX, _inputs: &mut CreateInputs) -> Option<CreateOutcome> {
        // Start tracking new execution frame for contract creation
        self.push_frame(self.current_trace_id);
        self.current_trace_id += 1;
        None
    }

    fn create_end(
        &mut self,
        _context: &mut CTX,
        _inputs: &CreateInputs,
        _outcome: &mut CreateOutcome,
    ) {
        // Stop tracking current execution frame
        self.pop_frame();
    }
}

/// Pretty printing utilities for debugging
impl ExecutionStepRecords {
    /// Print comprehensive summary with frame details
    pub fn print_summary(&self) {
        println!(
            "\n\x1b[36m╔══════════════════════════════════════════════════════════════════╗\x1b[0m"
        );
        println!(
            "\x1b[36m║              EXECUTION STEP INSPECTOR SUMMARY                    ║\x1b[0m"
        );
        println!(
            "\x1b[36m╚══════════════════════════════════════════════════════════════════╝\x1b[0m\n"
        );

        // Overall statistics
        let total_frames = self.len();
        let total_steps: usize = self.values().map(|v| v.len()).sum();

        println!("\x1b[33m📊 Overall Statistics:\x1b[0m");
        println!("  Total frames recorded: \x1b[32m{}\x1b[0m", total_frames);
        println!("  Total steps recorded:  \x1b[32m{}\x1b[0m", total_steps);

        if self.is_empty() {
            println!("\n\x1b[90m  No execution steps were recorded.\x1b[0m");
            return;
        }

        println!("\n\x1b[33m📋 Frame Details:\x1b[0m");
        println!(
            "\x1b[90m─────────────────────────────────────────────────────────────────\x1b[0m"
        );

        // Sort frames for consistent output
        let mut sorted_frames: Vec<_> = self.iter().collect();
        sorted_frames.sort_by_key(|(frame_id, _)| (frame_id.0, frame_id.1));

        for (frame_id, steps) in sorted_frames {
            // Frame header with color coding based on step count
            let color = if steps.is_empty() {
                "\x1b[90m" // Gray for empty
            } else if steps.len() < 10 {
                "\x1b[32m" // Green for small
            } else if steps.len() < 100 {
                "\x1b[33m" // Yellow for medium
            } else {
                "\x1b[31m" // Red for large
            };

            println!(
                "\n  {}Frame {}\x1b[0m (trace.{}, re-entry {})",
                color, frame_id, frame_id.0, frame_id.1
            );
            println!("  └─ Steps: \x1b[36m{}\x1b[0m", steps.len());

            if !steps.is_empty() {
                // Show first few and last few steps for context
                let preview_count = 3.min(steps.len());

                // First few steps
                println!("     \x1b[90mFirst {} steps:\x1b[0m", preview_count);
                for (i, step) in steps.iter().take(preview_count).enumerate() {
                    self.print_step_line(i, step, "     ");
                }

                // Last few steps if there are more
                if steps.len() > preview_count * 2 {
                    println!(
                        "     \x1b[90m... {} more steps ...\x1b[0m",
                        steps.len() - preview_count * 2
                    );
                    println!("     \x1b[90mLast {} steps:\x1b[0m", preview_count);
                    let start_idx = steps.len() - preview_count;
                    for (i, step) in steps.iter().skip(start_idx).enumerate() {
                        self.print_step_line(start_idx + i, step, "     ");
                    }
                } else if steps.len() > preview_count {
                    // Show remaining steps
                    for (i, step) in steps.iter().skip(preview_count).enumerate() {
                        self.print_step_line(preview_count + i, step, "     ");
                    }
                }

                // Summary stats for this frame
                let total_memory: usize = steps.iter().map(|s| s.memory.len()).sum();
                let avg_stack_depth: f64 =
                    steps.iter().map(|s| s.stack.len()).sum::<usize>() as f64 / steps.len() as f64;

                println!("     \x1b[90m├─ Avg stack depth: {:.1}\x1b[0m", avg_stack_depth);
                println!("     \x1b[90m└─ Total memory used: {} bytes\x1b[0m", total_memory);
            }
        }

        println!(
            "\n\x1b[90m─────────────────────────────────────────────────────────────────\x1b[0m"
        );
    }

    /// Helper to print a single step line
    fn print_step_line(&self, index: usize, step: &ExecutionStepRecord, indent: &str) {
        let opcode_str = format!("{}", step.opcode.as_str());
        let addr_short = format!("{:?}", step.contract_address);
        let addr_display = if addr_short.len() > 10 {
            format!("{}...{}", &addr_short[0..6], &addr_short[addr_short.len() - 4..])
        } else {
            addr_short
        };

        println!(
            "{}  [{:4}] PC={:5} \x1b[94m{:18}\x1b[0m @ \x1b[37m{}\x1b[0m | Stack:{:2} Mem:{:6}B",
            indent,
            index,
            step.pc,
            opcode_str,
            addr_display,
            step.stack.len(),
            step.memory.len()
        );
    }
}
