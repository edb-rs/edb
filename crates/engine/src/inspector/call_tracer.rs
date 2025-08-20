//! Call tracer for collecting complete execution trace during transaction replay
//!
//! This inspector captures the complete call trace including call stack, creation events,
//! and execution flow. The trace can be replayed later to determine execution paths
//! without needing to re-examine transaction inputs/outputs.

use alloy_primitives::{Address, Bytes, U256};
use revm::{
    context::{ContextTr, CreateScheme},
    interpreter::{CallInputs, CallOutcome, CallScheme, CreateInputs, CreateOutcome, Interpreter},
    Inspector,
};
use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
};
use tracing::{debug, error};

/// Result of transaction replay with call trace
#[derive(Debug)]
pub struct TraceReplayResult {
    /// All addresses visited during execution
    pub visited_addresses: HashMap<Address, bool>,
    /// Complete execution trace with call/create details
    pub execution_trace: Trace,
}

/// Type of call/creation operation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallType {
    /// Regular call to existing contract
    Call(CallScheme),
    /// Contract creation via CREATE opcode
    Create(CreateScheme),
}

/// Result of a call/creation operation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallResult {
    /// Call succeeded
    Success {
        /// Output data from the call
        output: Bytes,
    },
    /// Call reverted
    Revert {
        /// Output data from the call
        output: Bytes,
    },
}

/// Trace representation
#[derive(Debug, Clone, Default)]
pub struct Trace {
    inner: Vec<TraceEntry>,
}

impl Deref for Trace {
    type Target = Vec<TraceEntry>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for Trace {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

// Convenient explicit iterator methods (optional but nice)
impl Trace {
    pub fn iter(&self) -> std::slice::Iter<'_, TraceEntry> {
        self.inner.iter()
    }
    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, TraceEntry> {
        self.inner.iter_mut()
    }
}

// IntoIterator for owned Trace (moves out its contents)
impl IntoIterator for Trace {
    type Item = TraceEntry;
    type IntoIter = std::vec::IntoIter<TraceEntry>;
    fn into_iter(self) -> Self::IntoIter {
        self.inner.into_iter()
    }
}

// IntoIterator for &Trace (shared iteration)
impl<'a> IntoIterator for &'a Trace {
    type Item = &'a TraceEntry;
    type IntoIter = std::slice::Iter<'a, TraceEntry>;
    fn into_iter(self) -> Self::IntoIter {
        self.inner.iter()
    }
}

// IntoIterator for &mut Trace (mutable iteration)
impl<'a> IntoIterator for &'a mut Trace {
    type Item = &'a mut TraceEntry;
    type IntoIter = std::slice::IterMut<'a, TraceEntry>;
    fn into_iter(self) -> Self::IntoIter {
        self.inner.iter_mut()
    }
}

/// Single trace entry representing a call or creation
#[derive(Debug, Clone)]
pub struct TraceEntry {
    /// Unique ID of this trace entry (its index in the trace vector)
    pub id: usize,
    /// ID of the parent trace entry (None for top-level calls)
    pub parent_id: Option<usize>,
    /// Depth in the call stack (0 = top level)
    pub depth: usize,
    /// Type of operation
    pub call_type: CallType,
    /// Address making the call
    pub caller: Address,
    /// Target address for calls, or computed address for creates
    pub target: Address,
    /// Address where the code actually lives (for delegate calls)
    pub code_address: Address,
    /// Input data / constructor args
    pub input: Bytes,
    /// Value transferred
    pub value: U256,
    /// Result of the call (populated on call_end)
    pub result: Option<CallResult>,
    /// Whether this created a new contract
    pub created_contract: bool,
    /// Create scheme for contract creation
    pub create_scheme: Option<CreateScheme>,
    /// The underlying running bytecode
    pub bytecode: Option<Bytes>,
}

/// Complete call tracer that captures execution flow
#[derive(Debug, Default)]
pub struct CallTracer {
    /// Sequential list of all calls/creates in execution order
    pub trace: Trace,
    /// Map of visited addresses to whether they were deployed in this transaction
    pub visited_addresses: HashMap<Address, bool>,
    /// Stack to track call indices for proper nesting
    call_stack: Vec<usize>,
}

impl CallTracer {
    /// Create a new call tracer
    pub fn new() -> Self {
        Self { trace: Trace::default(), visited_addresses: HashMap::new(), call_stack: Vec::new() }
    }

    /// Get all visited addresses
    pub fn visited_addresses(&self) -> &HashMap<Address, bool> {
        &self.visited_addresses
    }

    /// Get the complete execution trace
    pub fn execution_trace(&self) -> &Trace {
        &self.trace
    }

    /// Convert the call tracer into a replay result
    pub fn into_replay_result(self) -> TraceReplayResult {
        TraceReplayResult { visited_addresses: self.visited_addresses, execution_trace: self.trace }
    }

    /// Add an address to the visited set
    fn mark_address_visited(&mut self, address: Address, deployed: bool) {
        self.visited_addresses
            .entry(address)
            .and_modify(|existing| *existing |= deployed)
            .or_insert(deployed);
    }

    /// Convert revm call inputs to our call type
    fn call_type_from_inputs(inputs: &CallInputs) -> CallType {
        CallType::Call(inputs.scheme)
    }

    /// Convert create scheme to call type
    fn call_type_from_create_scheme(input: &CreateInputs) -> CallType {
        CallType::Create(input.scheme)
    }

    /// Convert call outcome to result
    fn result_from_outcome(outcome: &CallOutcome) -> CallResult {
        if outcome.result.is_ok() {
            CallResult::Success { output: outcome.result.output.clone() }
        } else {
            CallResult::Revert { output: outcome.result.output.clone() }
        }
    }

    /// Convert create outcome to result
    fn result_from_create_outcome(outcome: &CreateOutcome) -> CallResult {
        if outcome.result.is_ok() {
            CallResult::Success { output: outcome.result.output.clone() }
        } else {
            CallResult::Revert { output: outcome.result.output.clone() }
        }
    }
}

impl<CTX: ContextTr> Inspector<CTX> for CallTracer {
    fn step(&mut self, interp: &mut Interpreter, _context: &mut CTX) {
        let Some(entry) = self.trace.last_mut() else {
            debug!("Trace is empty, cannot step");
            return;
        };

        if entry.bytecode.is_some() {
            // We already update the bytecode for the current entry
            return;
        }

        entry.bytecode = Some(interp.bytecode.bytes());
    }

    fn call(&mut self, context: &mut CTX, inputs: &mut CallInputs) -> Option<CallOutcome> {
        let call_type = Self::call_type_from_inputs(inputs);
        let target = inputs.target_address;
        let code_address = inputs.bytecode_address;
        let caller = inputs.caller;

        // Mark addresses as visited
        self.mark_address_visited(target, false);
        self.mark_address_visited(code_address, false);
        self.mark_address_visited(caller, false);

        // Determine the parent ID from the current call stack
        let parent_id = self.call_stack.last().copied();
        let trace_id = self.trace.len();

        // Create trace entry
        let trace_entry = TraceEntry {
            id: trace_id,
            parent_id,
            depth: self.call_stack.len(),
            call_type,
            caller,
            target,
            code_address,
            input: inputs.input.bytes(context).clone(),
            value: inputs.transfer_value().unwrap_or(U256::ZERO),
            result: None, // Will be filled in call_end
            created_contract: false,
            create_scheme: None,
            bytecode: None, // Will be set in step
        };

        // Add to trace and update stack
        self.trace.push(trace_entry);
        self.call_stack.push(trace_id);

        None // Continue with normal execution
    }

    fn call_end(&mut self, _context: &mut CTX, inputs: &CallInputs, outcome: &mut CallOutcome) {
        // Pop from call stack and update result
        let Some(trace_index) = self.call_stack.pop() else {
            error!("Call stack underflow - no matching call entry found");
            return;
        };

        let Some(trace_entry) = self.trace.get_mut(trace_index) else {
            error!("Call stack entry not found");
            return;
        };

        trace_entry.result = Some(Self::result_from_outcome(outcome));

        let target = inputs.target_address;
        let code_address = inputs.bytecode_address;
        let caller = inputs.caller;
        if trace_entry.target != target
            || trace_entry.code_address != code_address
            || trace_entry.caller != caller
        {
            error!("Call stack entry mismatch");
        }
    }

    fn create(&mut self, _context: &mut CTX, inputs: &mut CreateInputs) -> Option<CreateOutcome> {
        let call_type = Self::call_type_from_create_scheme(inputs);
        let caller = inputs.caller;

        // Mark addresses
        self.mark_address_visited(caller, false);

        // Determine the parent ID from the current call stack
        let parent_id = self.call_stack.last().copied();
        let trace_id = self.trace.len();

        // Create trace entry
        let trace_entry = TraceEntry {
            id: trace_id,
            parent_id,
            depth: self.call_stack.len(),
            call_type,
            caller,
            target: Address::ZERO,       // Target is not known yet
            code_address: Address::ZERO, // Code address is not known yet
            input: inputs.init_code.clone(),
            value: inputs.value,
            result: None,            // Will be filled in create_end
            created_contract: false, // Will be updated in create_end
            create_scheme: Some(inputs.scheme),
            bytecode: None, // Will be set in step
        };

        // Add to trace and update stack
        self.trace.push(trace_entry);
        self.call_stack.push(trace_id);

        None // Continue with normal execution
    }

    fn create_end(
        &mut self,
        _context: &mut CTX,
        inputs: &CreateInputs,
        outcome: &mut CreateOutcome,
    ) {
        // Pop from call stack and update result
        let Some(trace_index) = self.call_stack.pop() else {
            error!("Call stack underflow - no matching create entry found");
            return;
        };

        let Some(trace_entry) = self.trace.get_mut(trace_index) else {
            error!("Trace entry not found");
            return;
        };

        // Check caller consistence first, and no need to return
        let caller = inputs.caller;
        if trace_entry.caller != caller {
            error!("Create stack entry mismatch");
        }

        trace_entry.result = Some(Self::result_from_create_outcome(outcome));

        if matches!(trace_entry.result, Some(CallResult::Revert { .. })) {
            debug!("Creation failed");
            return;
        }

        let Some(created_address) = outcome.address else {
            error!("Create outcome did not provide created address");
            return;
        };

        trace_entry.target = created_address;
        trace_entry.code_address = created_address;
        trace_entry.created_contract = true;

        let created_address_for_marking = trace_entry.target;
        // Mark address after trace_entry is no longer borrowed
        let _ = trace_entry;

        self.mark_address_visited(created_address_for_marking, true);
    }

    fn selfdestruct(&mut self, contract: Address, target: Address, _value: U256) {
        // Mark both addresses as visited
        self.mark_address_visited(contract, false);
        self.mark_address_visited(target, false);

        // Note: We could add a special trace entry for selfdestruct if needed
        // For now, we just track the addresses
    }
}

impl Trace {
    /// Print the trace tree structure showing parent-child relationships with fancy formatting
    pub fn print_trace_tree(&self) {
        println!();
        println!(
            "\x1b[36m╔══════════════════════════════════════════════════════════════════╗\x1b[0m"
        );
        println!(
            "\x1b[36m║                      EXECUTION TRACE TREE                        ║\x1b[0m"
        );
        println!(
            "\x1b[36m╚══════════════════════════════════════════════════════════════════╝\x1b[0m"
        );
        println!();

        // Find root entries (those without parents)
        let roots: Vec<&TraceEntry> =
            self.inner.iter().filter(|entry| entry.parent_id.is_none()).collect();

        if roots.is_empty() {
            println!("  \x1b[90mNo trace entries found\x1b[0m");
            return;
        }

        for (i, root) in roots.iter().enumerate() {
            let is_last = i == roots.len() - 1;
            self.print_trace_entry(root, 0, is_last, vec![]);
        }

        println!();
        println!(
            "\x1b[36m══════════════════════════════════════════════════════════════════\x1b[0m"
        );
        self.print_summary();
    }

    /// Helper function to recursively print trace entries with fancy indentation
    fn print_trace_entry(
        &self,
        entry: &TraceEntry,
        indent_level: usize,
        is_last: bool,
        mut prefix: Vec<bool>,
    ) {
        // Build the tree structure with proper connectors
        let mut tree_str = String::new();
        for (i, &is_empty) in prefix.iter().enumerate() {
            if i < prefix.len() {
                tree_str.push_str(if is_empty { "    " } else { "\x1b[90m│\x1b[0m   " });
            }
        }

        // Add the branch connector
        let connector = if indent_level > 0 {
            if is_last {
                "\x1b[90m└──\x1b[0m "
            } else {
                "\x1b[90m├──\x1b[0m "
            }
        } else {
            ""
        };
        tree_str.push_str(connector);

        // Format the call type with colors based on operation type
        let (_call_color, type_color, call_type_str) = match &entry.call_type {
            CallType::Call(CallScheme::Call) => ("\x1b[94m", "\x1b[34m", "CALL"),
            CallType::Call(CallScheme::CallCode) => ("\x1b[94m", "\x1b[34m", "CALLCODE"),
            CallType::Call(CallScheme::DelegateCall) => ("\x1b[96m", "\x1b[36m", "DELEGATECALL"),
            CallType::Call(CallScheme::StaticCall) => ("\x1b[95m", "\x1b[35m", "STATICCALL"),
            CallType::Create(CreateScheme::Create) => ("\x1b[93m", "\x1b[33m", "CREATE"),
            CallType::Create(CreateScheme::Create2 { .. }) => ("\x1b[93m", "\x1b[33m", "CREATE2"),
            CallType::Create(CreateScheme::Custom { .. }) => {
                ("\x1b[93m", "\x1b[33m", "CREATE_CUSTOM")
            }
        };

        // Format the result indicator
        let (result_indicator, result_color) = match &entry.result {
            Some(CallResult::Success { output }) => {
                if output.is_empty() {
                    ("✓", "\x1b[32m")
                } else {
                    ("✓", "\x1b[32m")
                }
            }
            Some(CallResult::Revert { .. }) => ("✗", "\x1b[31m"),
            None => ("", ""),
        };

        // Format value transfer with better visibility
        let value_str = if entry.value > U256::ZERO {
            format!(" \x1b[93m[{} ETH]\x1b[0m", format_ether(entry.value))
        } else {
            String::new()
        };

        // Format addresses with better distinction
        let caller_str = if entry.caller == Address::ZERO {
            "\x1b[90m0x0\x1b[0m".to_string()
        } else {
            format!("\x1b[37m{}\x1b[0m", format_address_short(entry.caller))
        };

        let target_str = if entry.target == Address::ZERO {
            "\x1b[90m0x0\x1b[0m".to_string()
        } else if entry.created_contract {
            format!("\x1b[92m{}\x1b[0m", format_address_short(entry.target))
        } else {
            format!("\x1b[37m{}\x1b[0m", format_address_short(entry.target))
        };

        // Different arrow based on call type
        let arrow = if matches!(entry.call_type, CallType::Create(_)) {
            "\x1b[93m→\x1b[0m"
        } else {
            "\x1b[90m→\x1b[0m"
        };

        // Print the formatted entry with cleaner layout
        print!(
            "{}{}{:12}\x1b[0m {} {} {}",
            tree_str, type_color, call_type_str, caller_str, arrow, target_str
        );

        // Add result indicator at the end
        if !result_indicator.is_empty() {
            print!(" {}{} \x1b[0m", result_color, result_indicator);
        }

        // Add value if present
        if !value_str.is_empty() {
            print!("{}", value_str);
        }

        println!();

        // Print input data with better formatting if significant
        if entry.input.len() > 4 {
            let data_preview = format_data_preview(&entry.input);
            let padding = "    ".repeat(indent_level + 1);
            println!("{}\x1b[90m└ data: {}\x1b[0m", padding, data_preview);
        }

        // Get children and recursively print them
        let children = self.get_children(entry.id);

        // Update prefix for children
        if indent_level > 0 {
            prefix.push(is_last);
        }

        for (i, child) in children.iter().enumerate() {
            let child_is_last = i == children.len() - 1;
            self.print_trace_entry(child, indent_level + 1, child_is_last, prefix.clone());
        }
    }

    /// Print summary statistics about the trace
    fn print_summary(&self) {
        let total = self.inner.len();
        let successful = self
            .inner
            .iter()
            .filter(|e| matches!(e.result, Some(CallResult::Success { .. })))
            .count();
        let reverted = self
            .inner
            .iter()
            .filter(|e| matches!(e.result, Some(CallResult::Revert { .. })))
            .count();
        let creates =
            self.inner.iter().filter(|e| matches!(e.call_type, CallType::Create(_))).count();
        let calls = self.inner.iter().filter(|e| matches!(e.call_type, CallType::Call(_))).count();
        let max_depth = self.inner.iter().map(|e| e.depth).max().unwrap_or(0);

        println!("\x1b[36mSummary:\x1b[0m");
        println!("  Total: {} | \x1b[32mSuccess: {}\x1b[0m | \x1b[31mReverts: {}\x1b[0m | \x1b[94mCalls: {}\x1b[0m | \x1b[93mCreates: {}\x1b[0m | Depth: {}",
                 total, successful, reverted, calls, creates, max_depth);
    }

    /// Get the parent trace entry for a given trace entry ID
    pub fn get_parent(&self, trace_id: usize) -> Option<&TraceEntry> {
        self.inner
            .get(trace_id)
            .and_then(|entry| entry.parent_id.and_then(|parent_id| self.inner.get(parent_id)))
    }

    /// Get all children trace entries for a given trace entry ID
    pub fn get_children(&self, trace_id: usize) -> Vec<&TraceEntry> {
        self.inner.iter().filter(|entry| entry.parent_id == Some(trace_id)).collect()
    }
}

// Helper functions for formatting

/// Format an address to a shortened display format
fn format_address_short(addr: Address) -> String {
    if addr == Address::ZERO {
        "0x0".to_string()
    } else {
        format!("{:?}", addr)
    }
}

/// Format data/input bytes to a preview format
fn format_data_preview(data: &Bytes) -> String {
    if data.is_empty() {
        "0x".to_string()
    } else if data.len() <= 4 {
        format!("0x{}", hex::encode(data))
    } else {
        // Show function selector and total length
        format!("0x{}… [{} bytes]", hex::encode(&data[..4]), data.len())
    }
}

/// Format Wei value to ETH
fn format_ether(value: U256) -> String {
    // Convert Wei to ETH (1 ETH = 10^18 Wei)
    let eth_value = value.to_string();
    if eth_value.len() <= 18 {
        // Less than 1 ETH - show significant digits only
        let padded = format!("{:0>18}", eth_value);
        let trimmed = padded.trim_end_matches('0');
        if trimmed.is_empty() {
            "0".to_string()
        } else {
            format!("0.{}", &trimmed[..trimmed.len().min(6)])
        }
    } else {
        // More than 1 ETH
        let (whole, decimal) = eth_value.split_at(eth_value.len() - 18);
        let decimal_trimmed = decimal[..4.min(decimal.len())].trim_end_matches('0');
        if decimal_trimmed.is_empty() {
            whole.to_string()
        } else {
            format!("{}.{}", whole, decimal_trimmed)
        }
    }
}
