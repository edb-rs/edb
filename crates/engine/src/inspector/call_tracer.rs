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
use std::collections::HashMap;
use tracing::{debug, error};

/// Result of transaction replay with call trace
#[derive(Debug)]
pub struct TraceReplayResult {
    /// All addresses visited during execution
    pub visited_addresses: HashMap<Address, bool>,
    /// Complete execution trace with call/create details
    pub execution_trace: Vec<TraceEntry>,
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

/// Single trace entry representing a call or creation
#[derive(Debug, Clone)]
pub struct TraceEntry {
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
}

/// Complete call tracer that captures execution flow
#[derive(Debug, Default)]
pub struct CallTracer {
    /// Sequential list of all calls/creates in execution order
    pub trace: Vec<TraceEntry>,
    /// Map of visited addresses to whether they were deployed in this transaction
    pub visited_addresses: HashMap<Address, bool>,
    /// Stack to track call indices for proper nesting
    call_stack: Vec<usize>,
}

impl CallTracer {
    /// Create a new call tracer
    pub fn new() -> Self {
        Self { trace: Vec::new(), visited_addresses: HashMap::new(), call_stack: Vec::new() }
    }

    /// Get all visited addresses
    pub fn visited_addresses(&self) -> &HashMap<Address, bool> {
        &self.visited_addresses
    }

    /// Get the complete execution trace
    pub fn execution_trace(&self) -> &Vec<TraceEntry> {
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
    fn step(&mut self, _interp: &mut Interpreter, _context: &mut CTX) {
        // We trace calls/creates, not individual steps
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

        // Create trace entry
        let trace_entry = TraceEntry {
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
        };

        // Add to trace and update stack
        let trace_index = self.trace.len();
        self.trace.push(trace_entry);
        self.call_stack.push(trace_index);

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

        // Create trace entry
        let trace_entry = TraceEntry {
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
        };

        // Add to trace and update stack
        let trace_index = self.trace.len();
        self.trace.push(trace_entry);
        self.call_stack.push(trace_index);

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
