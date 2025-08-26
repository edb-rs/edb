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

use alloy_json_abi::{Function, JsonAbi};
use alloy_primitives::{hex, Address, Bytes, Log, LogData, U256};
use revm::{
    context::{ContextTr, CreateScheme},
    interpreter::{
        CallInputs, CallOutcome, CallScheme, CreateInputs, CreateOutcome, InstructionResult,
        Interpreter,
    },
    Inspector,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
};
use tracing::{debug, error};

/// Type of call/creation operation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CallType {
    /// Regular call to existing contract
    Call(CallScheme),
    /// Contract creation via CREATE opcode
    Create(CreateScheme),
}

/// Result of a call/creation operation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CallResult {
    /// Call succeeded
    Success {
        /// Output data from the call
        output: Bytes,
        /// Result
        result: InstructionResult,
    },
    /// Call reverted
    Revert {
        /// Output data from the call
        output: Bytes,
        /// Result
        result: InstructionResult,
    },
    /// Self-destruct
    Error {
        /// Output data from the call
        output: Bytes,
        /// Result
        result: InstructionResult,
    },
}

/// Trace representation
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
    /// Convert trace to serde_json::Value for RPC serialization
    pub fn to_json_value(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::to_value(self)
    }

    /// Create a new empty trace
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a trace entry to this trace
    pub fn push(&mut self, entry: TraceEntry) {
        self.inner.push(entry);
    }

    /// Get the number of trace entries
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Check if the trace is empty
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// Label of the target contract
    pub target_label: Option<String>,
    /// Self-destruct information
    pub self_destruct: Option<(Address, U256)>,
    /// Events
    pub events: Vec<LogData>,
    /// The first snapshot id that belongs to this entry
    pub first_snapshot_id: Option<usize>,
}

// Pretty print for Trace
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
            Some(CallResult::Success { output, .. }) => {
                if output.is_empty() {
                    ("✓", "\x1b[32m")
                } else {
                    ("✓", "\x1b[32m")
                }
            }
            Some(CallResult::Revert { .. }) => ("✗", "\x1b[31m"),
            Some(CallResult::Error { .. }) => {
                ("☠", "\x1b[31m") // TODO (change icon)
            }
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

        // Add self-destruct indicator if present
        if let Some((beneficiary, value)) = &entry.self_destruct {
            print!(
                " \x1b[91m SELFDESTRUCT → {} ({} ETH)\x1b[0m",
                format_address_short(*beneficiary),
                format_ether(*value)
            );
        }

        println!();

        // Print input data with better formatting if significant
        if entry.input.len() > 4 {
            let data_preview = format_data_preview(&entry.input);
            let padding = "    ".repeat(indent_level + 1);
            println!("{}\x1b[90m└ Calldata: {}\x1b[0m", padding, data_preview);
        }

        // Print events if any
        if !entry.events.is_empty() {
            let padding = "    ".repeat(indent_level + 1);
            for (i, event) in entry.events.iter().enumerate() {
                let event_str = format_event(event);
                if i == 0 {
                    println!("{}\x1b[96m└ Events:\x1b[0m", padding);
                }
                println!("{}    \x1b[96m• {}\x1b[0m", padding, event_str);
            }
        }

        // Print error details if result is Error
        if let Some(CallResult::Error { output, .. }) = &entry.result {
            let padding = "    ".repeat(indent_level + 1);
            let error_msg = if output.is_empty() {
                "Execution error (no output)".to_string()
            } else if output.len() >= 4 {
                // Try to decode as a revert message
                format!("Error: {}", format_data_preview(output))
            } else {
                format!("Error output: 0x{}", hex::encode(output))
            };
            println!("{}\x1b[91m└ ⚠️  {}\x1b[0m", padding, error_msg);
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
        let errors = self
            .inner
            .iter()
            .filter(|e| matches!(e.result, Some(CallResult::Error { .. })))
            .count();
        let self_destructs = self.inner.iter().filter(|e| e.self_destruct.is_some()).count();
        let with_events = self.inner.iter().filter(|e| !e.events.is_empty()).count();
        let total_events: usize = self.inner.iter().map(|e| e.events.len()).sum();
        let creates =
            self.inner.iter().filter(|e| matches!(e.call_type, CallType::Create(_))).count();
        let calls = self.inner.iter().filter(|e| matches!(e.call_type, CallType::Call(_))).count();
        let max_depth = self.inner.iter().map(|e| e.depth).max().unwrap_or(0);

        println!("\x1b[36mSummary:\x1b[0m");
        println!("  Total: {} | \x1b[32mSuccess: {}\x1b[0m | \x1b[31mReverts: {}\x1b[0m | \x1b[91mErrors: {}\x1b[0m | \x1b[94mCalls: {}\x1b[0m | \x1b[93mCreates: {}\x1b[0m | Depth: {}",
                 total, successful, reverted, errors, calls, creates, max_depth);

        if self_destructs > 0 {
            println!("  \x1b[91m💀 Self-destructs: {}\x1b[0m", self_destructs);
        }

        if total_events > 0 {
            println!("  \x1b[96m📝 Events: {} (in {} calls)\x1b[0m", total_events, with_events);
        }
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

/// Format event/log data for display
fn format_event(event: &LogData) -> String {
    if event.topics().is_empty() {
        // Anonymous event or no topics
        format!("Anonymous event with {} bytes data", event.data.len())
    } else {
        // First topic is usually the event signature hash
        let sig_hash = &event.topics()[0];
        let additional_topics = event.topics().len() - 1;
        let data_len = event.data.len();

        // Format the signature hash (first 8 chars)
        let sig_preview = format!("0x{}...", hex::encode(&sig_hash.as_slice()[..4]));

        if additional_topics > 0 && data_len > 0 {
            format!("{} ({} indexed, {} bytes data)", sig_preview, additional_topics, data_len)
        } else if additional_topics > 0 {
            format!("{} ({} indexed params)", sig_preview, additional_topics)
        } else if data_len > 0 {
            format!("{} ({} bytes data)", sig_preview, data_len)
        } else {
            sig_preview
        }
    }
}
