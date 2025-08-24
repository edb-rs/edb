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

//! Trace panel for displaying execution trace
//!
//! This panel shows the call trace and allows navigation through trace entries.

use super::{EventResponse, PanelTr, PanelType};
use crate::managers::{ExecutionManager, ResourceManager, ThemeManager};
use crate::ui::borders::BorderPresets;
use crate::ui::icons::Icons;
use crate::ui::status::StatusBar;
use crate::ui::syntax::{SyntaxHighlighter, SyntaxType, TokenStyle};
use crate::ColorScheme;
use alloy_dyn_abi::{DynSolValue, EventExt, FunctionExt, JsonAbiExt};
use alloy_json_abi::Function;
use alloy_primitives::{hex, Address, Bytes, LogData, Selector, U256};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use edb_common::types::{CallResult, CallType, Trace, TraceEntry};
use eyre::Result;
use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{List, ListItem, Paragraph},
    Frame,
};
use revm::{
    context::CreateScheme,
    interpreter::{CallScheme, InstructionResult},
};
use std::collections::HashSet;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};
use tracing::debug;

/// Represents different types of trace lines for multi-line display
#[derive(Debug, Clone)]
enum TraceLineType {
    /// Main call/create line  
    Call(usize), // trace entry id
    /// Event line
    Event(usize, usize), // trace entry id, event index
    /// Return value line
    Return(usize), // trace entry id
}

/// Trace panel implementation
#[derive(Debug)]
pub struct TracePanel {
    // ========== Display ==========
    /// Currently selected trace entry index
    selected_index: usize,
    /// Scroll offset
    scroll_offset: usize,
    /// Current content height
    context_height: usize,
    /// Whether this panel is focused
    focused: bool,

    // ========== Data ==========
    /// Trace
    trace_data: Option<Trace>,
    /// Color Scheme:
    color_scheme: ColorScheme,
    /// Syntax highlighter for Solidity values
    syntax_highlighter: SyntaxHighlighter,
    /// Set of collapsed trace entry IDs (when collapsed, we hide children)
    collapsed_entries: HashSet<usize>,

    // ========== Managers ==========
    /// Shared execution state manager
    execution_manager: Arc<RwLock<ExecutionManager>>,
    /// Shared resource manager
    resource_manager: Arc<RwLock<ResourceManager>>,
    /// Theme manager for styling
    theme_manager: Arc<RwLock<ThemeManager>>,
}

impl TracePanel {
    /// Create a new trace panel
    pub fn new(
        execution_manager: Arc<RwLock<ExecutionManager>>,
        resource_manager: Arc<RwLock<ResourceManager>>,
        theme_manager: Arc<RwLock<ThemeManager>>,
    ) -> Self {
        Self {
            selected_index: 0,
            focused: false,
            scroll_offset: 0,
            context_height: 0,
            trace_data: None,
            color_scheme: ColorScheme::default(),
            syntax_highlighter: SyntaxHighlighter::new(),
            collapsed_entries: HashSet::new(),
            execution_manager,
            resource_manager,
            theme_manager,
        }
    }

    /// Move selection up
    fn move_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
            // Auto-scroll up if needed
            if self.selected_index < self.scroll_offset {
                self.scroll_offset = self.selected_index;
            }
        }
    }

    /// Move selection down
    fn move_down(&mut self) {
        if let Some(trace) = &self.trace_data {
            let display_lines = self.generate_display_lines(trace);
            let max_lines = display_lines.len();
            if self.selected_index < max_lines.saturating_sub(1) {
                self.selected_index += 1;
                // Auto-scroll down if needed
                let viewport_height = self.context_height;
                if self.selected_index >= self.scroll_offset + viewport_height {
                    self.scroll_offset = (self.selected_index + 1).saturating_sub(viewport_height);
                }
            }
        }
    }

    /// Get currently selected trace entry
    pub fn selected_entry(&self) -> Option<&TraceEntry> {
        if let Some(trace) = &self.trace_data {
            let display_lines = self.generate_display_lines(trace);
            if let Some(line_type) = display_lines.get(self.selected_index) {
                let entry_id = match line_type {
                    TraceLineType::Call(id) => *id,
                    TraceLineType::Event(id, _) => *id,
                    TraceLineType::Return(id) => *id,
                };
                trace.iter().find(|e| e.id == entry_id)
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Toggle expansion/collapse for current selected entry
    pub fn toggle_expansion(&mut self) {
        if let Some(trace) = &self.trace_data {
            let display_lines = self.generate_display_lines(trace);
            if let Some(line_type) = display_lines.get(self.selected_index) {
                let entry_id = match line_type {
                    TraceLineType::Call(id) => *id,
                    TraceLineType::Event(id, _) => *id,
                    TraceLineType::Return(id) => *id,
                };

                // Find the entry and only handle children collapse (details always shown)
                if let Some(_entry) = trace.iter().find(|e| e.id == entry_id) {
                    // Check if this entry has children that can be collapsed
                    let has_children = trace.iter().any(|e| e.parent_id == Some(entry_id));

                    if has_children {
                        // Handle parent/child collapse only
                        if self.collapsed_entries.contains(&entry_id) {
                            // Expand children: remove from collapsed set
                            self.collapsed_entries.remove(&entry_id);
                        } else {
                            // Collapse children: add to collapsed set
                            self.collapsed_entries.insert(entry_id);
                        }
                        // Adjust selection after parent collapse
                        self.adjust_selection_after_expansion(entry_id);
                    }
                }
            }
        }
    }

    /// Check if an entry is a descendant of the given parent_id
    fn is_descendant_of(&self, entry: &TraceEntry, parent_id: usize) -> bool {
        let mut current_parent = entry.parent_id;
        while let Some(pid) = current_parent {
            if pid == parent_id {
                return true;
            }
            // Find the next parent up the chain
            if let Some(trace) = &self.trace_data {
                if let Some(parent_entry) = trace.iter().find(|e| e.id == pid) {
                    current_parent = parent_entry.parent_id;
                } else {
                    break;
                }
            } else {
                break;
            }
        }
        false
    }

    /// Adjust selection to stay on the main call line after expansion/collapse
    fn adjust_selection_after_expansion(&mut self, entry_id: usize) {
        if let Some(trace) = &self.trace_data {
            let display_lines = self.generate_display_lines(trace);
            // Find the call line for this entry
            if let Some(call_line_index) = display_lines
                .iter()
                .position(|line| matches!(line, TraceLineType::Call(id) if *id == entry_id))
            {
                self.selected_index = call_line_index;

                // Ensure selection is within viewport
                let viewport_height = self.context_height;
                if self.selected_index < self.scroll_offset {
                    self.scroll_offset = self.selected_index;
                } else if self.selected_index >= self.scroll_offset + viewport_height {
                    self.scroll_offset = (self.selected_index + 1).saturating_sub(viewport_height);
                }
            }
        }
    }

    /// Format a trace entry into a display string with smart labeling and decoding
    fn format_trace_entry<'a>(&'a self, entry: &'a TraceEntry, depth: usize) -> Line<'a> {
        // Build structural indentation with UTF-8 tree characters
        let mut indent_chars = Vec::new();

        // Add tree structure for depth levels
        for i in 0..depth {
            if i == depth - 1 {
                // Last level - use branch character
                indent_chars.push(Icons::TREE_BRANCH);
            } else {
                // Intermediate levels - use vertical line and spacing
                indent_chars.push(Icons::TREE_VERTICAL);
                indent_chars.push(" ");
            }
        }

        let indent = indent_chars.join("");

        // Determine call type and color using color scheme
        let (call_type_str, call_color) = match &entry.call_type {
            CallType::Call(CallScheme::Call) => ("CALL", self.color_scheme.call_color),
            CallType::Call(CallScheme::CallCode) => ("CALLCODE", self.color_scheme.call_color),
            CallType::Call(CallScheme::DelegateCall) => {
                ("DELEGATECALL", self.color_scheme.call_color)
            }
            CallType::Call(CallScheme::StaticCall) => ("STATICCALL", self.color_scheme.call_color),
            CallType::Create(CreateScheme::Create) => ("CREATE", self.color_scheme.create_color),
            CallType::Create(CreateScheme::Create2 { .. }) => {
                ("CREATE2", self.color_scheme.create_color)
            }
            CallType::Create(CreateScheme::Custom { .. }) => {
                ("CREATE_CUSTOM", self.color_scheme.create_color)
            }
        };

        // Format target address with label if available
        let target_str = if let Some(label) = &entry.target_label {
            format!("{} ({})", label, self.format_address_short(entry.target))
        } else {
            // For addresses without labels, show a more readable shortened format
            self.format_address_readable(entry.target)
        };

        // Determine if this entry has children and add collapse/expand icon
        let has_children = self.entry_has_children(entry);
        let collapse_icon = if has_children {
            if self.collapsed_entries.contains(&entry.id) {
                Icons::COLLAPSED
            } else {
                Icons::EXPANDED
            }
        } else {
            " "
        };

        // Build the line with spans (removed caller since it's redundant in call trace)
        let mut spans = vec![
            Span::raw(indent),
            Span::styled(collapse_icon, Style::default().fg(self.color_scheme.accent_color)),
            Span::raw(" "),
            Span::styled(format!("{:<12}", call_type_str), Style::default().fg(call_color)),
            Span::raw(" "),
            Span::styled(
                target_str,
                Style::default().fg(self.color_scheme.syntax_identifier_color),
            ),
        ];

        // Add function call with syntax-highlighted parameters
        if matches!(entry.call_type, CallType::Create(_)) {
            // Handle contract creation
            if let Some(constructor_call) = self.format_constructor_call(entry) {
                spans.push(Span::raw(" "));
                spans.extend(self.highlight_solidity_code_owned(constructor_call));
            } else if !entry.input.is_empty() {
                spans.push(Span::raw(" "));
                spans.push(Span::styled(
                    format!("bytecode ({} bytes)", entry.input.len()),
                    Style::default().fg(self.color_scheme.syntax_string_color),
                ));
            }
        } else {
            // Handle regular function calls
            let selector = if entry.input.len() >= 4 {
                Some(Selector::from_slice(&entry.input[..4]))
            } else {
                None
            };
            if let Some(function_abi) = entry
                .abi
                .as_ref()
                .zip(selector)
                .and_then(|(abi, sel)| abi.function_by_selector(sel))
            {
                spans.push(Span::raw(" "));
                spans.extend(
                    self.format_function_call_with_highlighting(function_abi, &entry.input),
                );
            } else if !entry.input.is_empty() {
                spans.push(Span::raw(" "));
                // Try to show function selector and data size instead of raw hex
                if entry.input.len() >= 4 {
                    let selector = hex::encode(&entry.input[..4]);
                    let data_size = entry.input.len() - 4;
                    if data_size > 0 {
                        spans.push(Span::styled(
                            format!("0x{}({} bytes)", selector, data_size),
                            Style::default().fg(self.color_scheme.syntax_string_color),
                        ));
                    } else {
                        spans.push(Span::styled(
                            format!("0x{}()", selector),
                            Style::default().fg(self.color_scheme.syntax_string_color),
                        ));
                    }
                } else {
                    spans.push(Span::styled(
                        format!("0x{}", hex::encode(&entry.input)),
                        Style::default().fg(self.color_scheme.syntax_string_color),
                    ));
                }
            }
        }

        // Format value if present
        if entry.value > U256::ZERO {
            spans.push(Span::styled(
                format!(" value: {} ETH", self.format_ether(entry.value)),
                Style::default().fg(self.color_scheme.warning_color),
            ));
        }

        // Add self-destruct indicator if present
        if let Some((beneficiary, value)) = &entry.self_destruct {
            spans.push(Span::styled(
                " [SELFDESTRUCT]",
                Style::default().fg(self.color_scheme.error_color),
            ));
            spans.push(Span::styled(
                format!(
                    " → {} ({} ETH)",
                    self.format_address_readable(*beneficiary),
                    self.format_ether(*value)
                ),
                Style::default().fg(self.color_scheme.warning_color),
            ));
        }

        // Add events indicator if present
        if !entry.events.is_empty() {
            spans.push(Span::styled(
                format!(
                    " 📝 {} event{}",
                    entry.events.len(),
                    if entry.events.len() == 1 { "" } else { "s" }
                ),
                Style::default().fg(self.color_scheme.accent_color),
            ));
        }

        // Result indicator
        let (result_char, result_color) = match &entry.result {
            Some(CallResult::Success { .. }) => ("✓", self.color_scheme.success_color),
            Some(CallResult::Revert { .. }) => ("✗", self.color_scheme.error_color),
            Some(CallResult::Error { .. }) => ("⚠", self.color_scheme.error_color),
            None => (" ", self.color_scheme.comment_color),
        };

        // Add return data if available
        if let Some(return_spans) = self.format_return_data(entry) {
            spans.push(Span::raw(" "));
            spans.extend(return_spans);
        }

        spans.push(Span::raw(" "));
        spans.push(Span::styled(result_char, Style::default().fg(result_color)));

        Line::from(spans)
    }

    /// Check if a trace entry has children (callees)
    fn entry_has_children(&self, entry: &TraceEntry) -> bool {
        if let Some(trace) = &self.trace_data {
            // Look for any entry with this entry as parent
            trace.iter().any(|e| e.parent_id == Some(entry.id))
        } else {
            false
        }
    }

    /// Toggle collapse state for a trace entry
    fn toggle_collapse(&mut self, entry_id: usize) {
        if self.collapsed_entries.contains(&entry_id) {
            self.collapsed_entries.remove(&entry_id);
        } else {
            self.collapsed_entries.insert(entry_id);
        }
    }

    /// Check if an entry should be visible (not hidden by collapsed parent)
    fn is_entry_visible(&self, entry: &TraceEntry) -> bool {
        if let Some(parent_id) = entry.parent_id {
            // Check if any ancestor is collapsed
            let mut current_parent_id = Some(parent_id);
            while let Some(pid) = current_parent_id {
                if self.collapsed_entries.contains(&pid) {
                    return false;
                }
                // Find the parent entry to check its parent
                if let Some(trace) = &self.trace_data {
                    if let Some(parent_entry) = trace.iter().find(|e| e.id == pid) {
                        current_parent_id = parent_entry.parent_id;
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
        }
        true
    }

    /// Generate display lines for the trace, always showing details
    fn generate_display_lines(&self, trace: &Trace) -> Vec<TraceLineType> {
        let mut lines = Vec::new();

        for entry in trace.iter() {
            if !self.is_entry_visible(entry) {
                continue;
            }

            // Always show the main call line
            lines.push(TraceLineType::Call(entry.id));

            // Always show events and return values (no expansion toggle for details)
            // Add event lines
            for (i, _) in entry.events.iter().enumerate() {
                lines.push(TraceLineType::Event(entry.id, i));
            }

            // Add return value line if there's meaningful return data
            if entry.result.is_some() && self.should_show_return_line(entry) {
                lines.push(TraceLineType::Return(entry.id));
            }
        }

        lines
    }

    /// Check if we should show a separate return line for this entry
    fn should_show_return_line(&self, entry: &TraceEntry) -> bool {
        match &entry.result {
            Some(CallResult::Success { output, .. }) => !output.is_empty(),
            Some(CallResult::Revert { .. }) => true,
            Some(CallResult::Error { .. }) => true,
            None => false,
        }
    }

    /// Format a display line based on its type
    fn format_display_line(&self, line_type: &TraceLineType, trace: &Trace) -> Line<'static> {
        match line_type {
            TraceLineType::Call(entry_id) => {
                if let Some(entry) = trace.iter().find(|e| e.id == *entry_id) {
                    self.format_trace_entry_compact(entry, entry.depth)
                } else {
                    Line::from("Unknown entry")
                }
            }
            TraceLineType::Event(entry_id, event_idx) => {
                if let Some(entry) = trace.iter().find(|e| e.id == *entry_id) {
                    self.format_event_line(entry, *event_idx)
                } else {
                    Line::from("Unknown event")
                }
            }
            TraceLineType::Return(entry_id) => {
                if let Some(entry) = trace.iter().find(|e| e.id == *entry_id) {
                    self.format_return_line(entry)
                } else {
                    Line::from("Unknown return")
                }
            }
        }
    }

    /// Format a compact trace entry (main call line without events/returns)
    fn format_trace_entry_compact(&self, entry: &TraceEntry, depth: usize) -> Line<'static> {
        // Check if this entry has children
        let has_children = if let Some(trace) = &self.trace_data {
            trace.iter().any(|e| e.parent_id == Some(entry.id))
        } else {
            false
        };

        // Collapse indicator at the beginning of the line
        let collapse_char = if has_children {
            if self.collapsed_entries.contains(&entry.id) {
                "▶ " // children collapsed
            } else {
                "▼ " // children expanded
            }
        } else {
            "  " // no children - just spaces for alignment
        };

        // Build indentation for the tree structure
        let mut indent_chars = Vec::new();
        for i in 0..depth {
            if i == depth - 1 {
                indent_chars.push("├─");
            } else {
                indent_chars.push(" ");
            }
        }
        let indent = indent_chars.join("");

        let (call_type_str, call_color) = match &entry.call_type {
            CallType::Call(CallScheme::Call) => ("CALL", self.color_scheme.call_color),
            CallType::Call(CallScheme::CallCode) => ("CALLCODE", self.color_scheme.call_color),
            CallType::Call(CallScheme::DelegateCall) => {
                ("DELEGATECALL", self.color_scheme.call_color)
            }
            CallType::Call(CallScheme::StaticCall) => ("STATICCALL", self.color_scheme.call_color),
            CallType::Create(CreateScheme::Create) => ("CREATE", self.color_scheme.create_color),
            CallType::Create(CreateScheme::Create2 { .. }) => {
                ("CREATE2", self.color_scheme.create_color)
            }
            CallType::Create(CreateScheme::Custom { .. }) => {
                ("CUSTOM_CREATE", self.color_scheme.create_color)
            }
        };

        let mut spans = vec![
            Span::styled(collapse_char, Style::default().fg(self.color_scheme.comment_color)),
            Span::raw(indent),
            Span::styled(call_type_str, Style::default().fg(call_color)),
            Span::raw(" "),
            Span::styled(
                self.format_address_readable(entry.code_address),
                Style::default().fg(self.color_scheme.accent_color),
            ),
        ];

        // Add function call details
        if matches!(entry.call_type, CallType::Create(_)) {
            if let Some(constructor_call) = self.format_constructor_call(entry) {
                spans.push(Span::raw(" "));
                spans.push(Span::styled(
                    constructor_call,
                    Style::default().fg(self.color_scheme.keyword_color),
                ));
            }
        } else {
            let selector = if entry.input.len() >= 4 {
                Some(Selector::from_slice(&entry.input[..4]))
            } else {
                None
            };
            if let Some(function_abi) = entry
                .abi
                .as_ref()
                .zip(selector)
                .and_then(|(abi, sel)| abi.function_by_selector(sel))
            {
                spans.push(Span::raw(" "));
                spans.extend(
                    self.format_function_call_with_highlighting(function_abi, &entry.input),
                );
            } else if !entry.input.is_empty() {
                spans.push(Span::raw(" "));
                if entry.input.len() >= 4 {
                    let selector = hex::encode(&entry.input[..4]);
                    let data_size = entry.input.len() - 4;
                    if data_size > 0 {
                        spans.push(Span::styled(
                            format!("0x{}...({} bytes)", selector, data_size),
                            Style::default().fg(self.color_scheme.syntax_string_color),
                        ));
                    } else {
                        spans.push(Span::styled(
                            format!("0x{}", selector),
                            Style::default().fg(self.color_scheme.syntax_string_color),
                        ));
                    }
                }
            }
        }

        // Add self-destruct indicator if present
        if let Some((beneficiary, value)) = &entry.self_destruct {
            spans.push(Span::styled(
                " [SELFDESTRUCT]",
                Style::default().fg(self.color_scheme.error_color),
            ));
            spans.push(Span::styled(
                format!(
                    " → {} ({} ETH)",
                    self.format_address_readable(*beneficiary),
                    self.format_ether(*value)
                ),
                Style::default().fg(self.color_scheme.warning_color),
            ));
        }

        // Result indicator with more specific symbols based on InstructionResult
        let (result_char, result_color) = match &entry.result {
            Some(CallResult::Success { result, .. }) => match result {
                InstructionResult::Return => ("✓", self.color_scheme.success_color),
                InstructionResult::Stop => ("•", self.color_scheme.success_color),
                InstructionResult::SelfDestruct => ("†", self.color_scheme.warning_color),
                _ => ("✓", self.color_scheme.success_color),
            },
            Some(CallResult::Revert { result, .. }) => match result {
                InstructionResult::Revert => ("↩", self.color_scheme.error_color),
                _ => ("✗", self.color_scheme.error_color),
            },
            Some(CallResult::Error { result, .. }) => match result {
                InstructionResult::OutOfGas => ("G", self.color_scheme.warning_color),
                InstructionResult::StackOverflow => ("S", self.color_scheme.warning_color),
                InstructionResult::OpcodeNotFound => ("X", self.color_scheme.error_color),
                InstructionResult::OutOfFunds => ("F", self.color_scheme.warning_color),
                _ => ("E", self.color_scheme.warning_color),
            },
            None => ("?", self.color_scheme.comment_color),
        };

        spans.push(Span::raw(" "));
        spans.push(Span::styled(result_char, Style::default().fg(result_color)));

        Line::from(spans)
    }

    /// Format an event line
    fn format_event_line(&self, entry: &TraceEntry, event_idx: usize) -> Line<'static> {
        if let Some(event) = entry.events.get(event_idx) {
            // Tree structure: spaces for alignment + tree branch + [EVENT] label
            let base_indent = "  ".to_string(); // Align with collapse indicator
            let tree_indent = " ".repeat(entry.depth * 2);
            let tree_branch = "├─ ";
            let full_indent = format!("{}{}{}", base_indent, tree_indent, tree_branch);

            let event_text = self.format_event(event, entry.abi.as_ref());

            Line::from(vec![
                Span::styled(full_indent, Style::default().fg(self.color_scheme.comment_color)),
                Span::styled("[EVENT] ", Style::default().fg(self.color_scheme.comment_color)),
                Span::styled(event_text, Style::default().fg(self.color_scheme.accent_color)),
            ])
        } else {
            Line::from("Invalid event")
        }
    }

    /// Format a return value line  
    fn format_return_line(&self, entry: &TraceEntry) -> Line<'static> {
        // Tree structure: use └─ for return (always last)
        let base_indent = "  ".to_string(); // Align with collapse indicator
        let tree_indent = " ".repeat(entry.depth * 2);
        let tree_branch = "└─ ";
        let full_indent = format!("{}{}{}", base_indent, tree_indent, tree_branch);

        match &entry.result {
            Some(CallResult::Success { output, .. }) => {
                let return_text = if output.is_empty() {
                    "()".to_string()
                } else {
                    // Try to decode return value using ABI
                    self.decode_return_value(entry, output)
                };

                Line::from(vec![
                    Span::styled(full_indent, Style::default().fg(self.color_scheme.comment_color)),
                    Span::styled("[RETURN] ", Style::default().fg(self.color_scheme.comment_color)),
                    Span::styled(
                        return_text,
                        Style::default().fg(self.color_scheme.syntax_string_color),
                    ),
                ])
            }
            Some(CallResult::Revert { output, result }) => {
                let revert_text = self.decode_revert_reason(output);
                Line::from(vec![
                    Span::styled(full_indent, Style::default().fg(self.color_scheme.comment_color)),
                    Span::styled("[REVERT] ", Style::default().fg(self.color_scheme.error_color)),
                    Span::styled(
                        revert_text,
                        Style::default().fg(self.color_scheme.syntax_string_color),
                    ),
                ])
            }
            Some(CallResult::Error { output, result }) => {
                let error_text = self.format_instruction_result(*result, output);
                Line::from(vec![
                    Span::styled(full_indent, Style::default().fg(self.color_scheme.comment_color)),
                    Span::styled("[ERROR] ", Style::default().fg(self.color_scheme.error_color)),
                    Span::styled(error_text, Style::default().fg(self.color_scheme.error_color)),
                ])
            }
            None => Line::from("Unknown result"),
        }
    }

    /// Decode return value using function ABI
    fn decode_return_value(&self, entry: &TraceEntry, output: &Bytes) -> String {
        // Get the function selector from the input
        if entry.input.len() >= 4 {
            let selector = Selector::from_slice(&entry.input[..4]);

            // Try to find the function in the ABI and decode return value
            if let Some(function_abi) =
                entry.abi.as_ref().and_then(|abi| abi.function_by_selector(selector))
            {
                // Try to decode the return data
                match function_abi.abi_decode_output(output) {
                    Ok(decoded_values) => {
                        if decoded_values.is_empty() {
                            return "()".to_string();
                        }

                        // Format decoded return values with names if available
                        let mut return_parts = Vec::new();
                        for (i, value) in decoded_values.iter().enumerate() {
                            let param_name = function_abi
                                .outputs
                                .get(i)
                                .map(|param| param.name.as_str())
                                .filter(|name| !name.is_empty());

                            if let Some(name) = param_name {
                                return_parts.push(format!(
                                    "{}: {}",
                                    name,
                                    self.format_solidity_value(value)
                                ));
                            } else {
                                return_parts.push(self.format_solidity_value(value));
                            }
                        }

                        if return_parts.len() == 1 {
                            return_parts[0].clone()
                        } else {
                            format!("({})", return_parts.join(", "))
                        }
                    }
                    Err(_) => {
                        // Fallback to hex if decoding fails
                        if output.len() <= 32 {
                            format!("0x{}", hex::encode(output))
                        } else {
                            format!("0x{}...({} bytes)", hex::encode(&output[..8]), output.len())
                        }
                    }
                }
            } else {
                // No ABI available, show hex
                if output.len() <= 32 {
                    format!("0x{}", hex::encode(output))
                } else {
                    format!("0x{}...({} bytes)", hex::encode(&output[..8]), output.len())
                }
            }
        } else {
            // No selector, show hex
            if output.len() <= 32 {
                format!("0x{}", hex::encode(output))
            } else {
                format!("0x{}...({} bytes)", hex::encode(&output[..8]), output.len())
            }
        }
    }

    /// Decode revert reason from output data
    fn decode_revert_reason(&self, output: &Bytes) -> String {
        if output.is_empty() {
            return "(empty revert)".to_string();
        }

        // Check if it's a standard Error(string) revert (0x08c379a0)
        if output.len() >= 4 && output.starts_with(&[0x08, 0xc3, 0x79, 0xa0]) {
            // Try to decode the string from Error(string) signature
            if let Ok(decoded) = alloy_dyn_abi::DynSolType::String.abi_decode(&output[4..]) {
                if let alloy_dyn_abi::DynSolValue::String(reason) = decoded {
                    return format!("\"{}\"", reason);
                }
            }
        }

        // Check if it's a Panic(uint256) revert (0x4e487b71)
        if output.len() >= 4 && output.starts_with(&[0x4e, 0x48, 0x7b, 0x71]) {
            if let Ok(decoded) = alloy_dyn_abi::DynSolType::Uint(256).abi_decode(&output[4..]) {
                if let alloy_dyn_abi::DynSolValue::Uint(panic_code, _) = decoded {
                    let panic_reason = match panic_code.to_string().as_str() {
                        "1" => "assertion failed",
                        "17" => "arithmetic overflow/underflow",
                        "18" => "division by zero",
                        "33" => "enum conversion error",
                        "34" => "invalid storage byte array access",
                        "49" => "pop() on empty array",
                        "50" => "array index out of bounds",
                        "65" => "memory allocation overflow",
                        "81" => "zero initialization of invalid type",
                        _ => "unknown panic",
                    };
                    return format!("Panic({}: {})", panic_code, panic_reason);
                }
            }
        }

        // Fallback: show hex data
        if output.len() <= 32 {
            format!("0x{}", hex::encode(output))
        } else {
            format!("0x{}...({} bytes)", hex::encode(&output[..8]), output.len())
        }
    }

    /// Format InstructionResult with context
    fn format_instruction_result(&self, result: InstructionResult, output: &Bytes) -> String {
        match result {
            InstructionResult::Stop => "stop".to_string(),
            InstructionResult::Return => "return".to_string(),
            InstructionResult::SelfDestruct => "selfdestruct".to_string(),
            InstructionResult::Revert => {
                // This shouldn't happen in Error variant, but handle it
                self.decode_revert_reason(output)
            }
            InstructionResult::CallTooDeep => "call stack too deep".to_string(),
            InstructionResult::OutOfFunds => "insufficient funds".to_string(),
            InstructionResult::InvalidJump => "invalid jump destination".to_string(),
            InstructionResult::StackOverflow => "stack overflow".to_string(),
            InstructionResult::StackUnderflow => "stack underflow".to_string(),
            InstructionResult::OutOfGas => "out of gas".to_string(),
            InstructionResult::MemoryOOG => "out of gas (memory)".to_string(),
            InstructionResult::MemoryLimitOOG => "memory limit exceeded".to_string(),
            InstructionResult::PrecompileOOG => "precompile out of gas".to_string(),
            InstructionResult::InvalidOperandOOG => "invalid operand (OOG)".to_string(),
            InstructionResult::OpcodeNotFound => "opcode not found".to_string(),
            InstructionResult::CreateInitCodeSizeLimit => "init code size limit".to_string(),
            InstructionResult::CreateContractSizeLimit => "contract size limit".to_string(),
            InstructionResult::OverflowPayment => "payment overflow".to_string(),
            InstructionResult::StateChangeDuringStaticCall => {
                "state change in static call".to_string()
            }
            InstructionResult::CallNotAllowedInsideStatic => {
                "call not allowed in static".to_string()
            }
            InstructionResult::OutOfOffset => "out of offset".to_string(),
            InstructionResult::CreateCollision => "create collision".to_string(),
            InstructionResult::FatalExternalError => "fatal external error".to_string(),
            _ => format!("unknown error ({:?})", result),
        }
    }

    /// Format address to short form for labels (6...4 format)
    #[allow(dead_code)]
    fn format_address_short(&self, address: Address) -> String {
        if address == Address::ZERO {
            "0x0".to_string()
        } else {
            let addr_str = format!("{:?}", address);
            if addr_str.len() > 10 {
                format!("{}...{}", &addr_str[..6], &addr_str[addr_str.len() - 4..])
            } else {
                addr_str
            }
        }
    }

    /// Format address in a more readable way for unlabeled addresses (8...6 format)
    fn format_address_readable(&self, address: Address) -> String {
        if address == Address::ZERO {
            "0x0000000000000000".to_string()
        } else {
            let addr_str = format!("{:?}", address);
            // Show more characters for better identification: 8 chars + ... + 6 chars
            format!("{}...{}", &addr_str[..8], &addr_str[addr_str.len() - 6..])
        }
    }

    /// Format function call with ABI decoding (returns string for backwards compatibility)
    fn format_function_call(&self, function_abi: &Function, input_data: &Bytes) -> String {
        if input_data.len() < 4 {
            return format!("{}()", function_abi.name);
        }

        // Try to decode the input data
        match function_abi.abi_decode_input(&input_data[4..]) {
            Ok(decoded) => {
                let params: Vec<String> =
                    decoded.iter().map(|param| self.format_solidity_value(param)).collect();

                format!("{}({})", function_abi.name, params.join(", "))
            }
            Err(_) => {
                // Fallback to raw data display
                format!(
                    "{}(0x{}...)",
                    function_abi.name,
                    hex::encode(&input_data[4..input_data.len().min(8)])
                )
            }
        }
    }

    /// Format function call with syntax highlighting using the syntax highlighter
    fn format_function_call_with_highlighting(
        &self,
        function_abi: &Function,
        input_data: &Bytes,
    ) -> Vec<Span<'static>> {
        // Get the function call as a string (reuse existing logic)
        let call_string = self.format_function_call(function_abi, input_data);

        // Use the syntax highlighter to tokenize and highlight the string
        self.highlight_solidity_code_owned(call_string)
    }

    /// Format a Solidity value for display
    fn format_solidity_value(&self, value: &DynSolValue) -> String {
        match value {
            DynSolValue::Address(addr) => format!("0x{:x}", addr),
            DynSolValue::Uint(n, _) => n.to_string(),
            DynSolValue::Int(n, _) => n.to_string(),
            DynSolValue::Bool(b) => b.to_string(),
            DynSolValue::Bytes(b) => format!("0x{}", hex::encode(&b[..b.len().min(8)])),
            DynSolValue::FixedBytes(b, _) => format!("0x{}", hex::encode(b)),
            DynSolValue::String(s) => format!("\"{}\"", s),
            DynSolValue::Array(arr) => {
                if arr.len() <= 3 {
                    let items: Vec<String> =
                        arr.iter().map(|v| self.format_solidity_value(v)).collect();
                    format!("[{}]", items.join(", "))
                } else {
                    format!("[...{} items]", arr.len())
                }
            }
            DynSolValue::FixedArray(arr) => {
                if arr.len() <= 3 {
                    let items: Vec<String> =
                        arr.iter().map(|v| self.format_solidity_value(v)).collect();
                    format!("[{}]", items.join(", "))
                } else {
                    format!("[...{} items]", arr.len())
                }
            }
            DynSolValue::Tuple(tuple) => {
                if tuple.len() <= 2 {
                    let items: Vec<String> =
                        tuple.iter().map(|v| self.format_solidity_value(v)).collect();
                    format!("({})", items.join(", "))
                } else {
                    format!("(...{} fields)", tuple.len())
                }
            }
            DynSolValue::Function(_) => "<function>".to_string(),
        }
    }

    /// Format Wei value to ETH
    fn format_ether(&self, value: U256) -> String {
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

    /// Format return data for successful function calls with syntax highlighting
    fn format_return_data(&self, entry: &TraceEntry) -> Option<Vec<Span<'static>>> {
        // Only show return data for successful calls
        if let Some(CallResult::Success { output, .. }) = &entry.result {
            if !output.is_empty() {
                // If we have function ABI, try to decode the return data
                if entry.input.len() >= 4 {
                    let selector = Selector::from_slice(&entry.input[..4]);
                    if let Some(function_abi) =
                        entry.abi.as_ref().and_then(|abi| abi.function_by_selector(selector))
                    {
                        if let Ok(decoded) = function_abi.abi_decode_output(output) {
                            let mut spans = vec![Span::styled(
                                "→ ",
                                Style::default().fg(self.color_scheme.comment_color),
                            )];

                            // Format decoded return values with syntax highlighting
                            let return_values: Vec<String> =
                                decoded.iter().map(|v| self.format_solidity_value(v)).collect();
                            let return_str = return_values.join(", ");
                            spans.extend(self.highlight_solidity_code_owned(return_str));

                            return Some(spans);
                        }
                    }
                }

                // Fallback: show return data size and preview
                if output.len() <= 32 {
                    return Some(vec![
                        Span::styled("→ ", Style::default().fg(self.color_scheme.comment_color)),
                        Span::styled(
                            format!("0x{}", hex::encode(output)),
                            Style::default().fg(self.color_scheme.syntax_number_color),
                        ),
                    ]);
                } else {
                    return Some(vec![
                        Span::styled("→ ", Style::default().fg(self.color_scheme.comment_color)),
                        Span::styled(
                            format!("0x{}...({} bytes)", hex::encode(&output[..8]), output.len()),
                            Style::default().fg(self.color_scheme.syntax_number_color),
                        ),
                    ]);
                }
            }
        } else if let Some(CallResult::Revert { output, .. }) = &entry.result {
            // Show revert reason if available
            if !output.is_empty() {
                // Try to decode revert reason from output
                let reason = if output.len() >= 4 {
                    // Check if it's a standard Error(string) revert
                    if output.starts_with(&[0x08, 0xc3, 0x79, 0xa0]) {
                        // Decode the string from Error(string) signature
                        if let Ok(decoded) =
                            alloy_dyn_abi::DynSolType::String.abi_decode(&output[4..])
                        {
                            if let alloy_dyn_abi::DynSolValue::String(s) = decoded {
                                Some(s)
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                };

                return Some(vec![
                    Span::styled("→ ", Style::default().fg(self.color_scheme.comment_color)),
                    Span::styled("revert: ", Style::default().fg(self.color_scheme.error_color)),
                    Span::styled(
                        if let Some(reason) = reason {
                            format!("\"{}\"", reason)
                        } else {
                            format!("0x{}", hex::encode(output))
                        },
                        Style::default().fg(self.color_scheme.syntax_string_color),
                    ),
                ]);
            }
        } else if let Some(CallResult::Error { output, .. }) = &entry.result {
            // Show error details
            if !output.is_empty() {
                return Some(vec![
                    Span::styled("→ ", Style::default().fg(self.color_scheme.comment_color)),
                    Span::styled("error: ", Style::default().fg(self.color_scheme.error_color)),
                    Span::styled(
                        if output.len() <= 32 {
                            format!("0x{}", hex::encode(output))
                        } else {
                            format!("0x{}...({} bytes)", hex::encode(&output[..8]), output.len())
                        },
                        Style::default().fg(self.color_scheme.syntax_string_color),
                    ),
                ]);
            } else {
                return Some(vec![
                    Span::styled("→ ", Style::default().fg(self.color_scheme.comment_color)),
                    Span::styled(
                        "execution error",
                        Style::default().fg(self.color_scheme.error_color),
                    ),
                ]);
            }
        }

        None
    }

    /// Format event data using ABI if available
    fn format_event(&self, event: &LogData, abi: Option<&alloy_json_abi::JsonAbi>) -> String {
        if event.topics().is_empty() {
            return format!("Anonymous event ({} bytes data)", event.data.len());
        }

        let event_signature = event.topics()[0];

        // Try to find the event in the ABI
        if let Some(abi) = abi {
            if let Some(event_abi) = abi.events().find(|e| e.selector() == event_signature) {
                // Try to decode the event
                match event_abi.decode_log(event) {
                    Ok(decoded) => {
                        // Format decoded event with parameters
                        let mut params = Vec::new();
                        for (param, value) in event_abi.inputs.iter().zip(decoded.body.iter()) {
                            params.push(format!(
                                "{}: {}",
                                param.name,
                                self.format_solidity_value(value)
                            ));
                        }

                        if params.is_empty() {
                            format!("{}()", event_abi.name)
                        } else {
                            format!("{}({})", event_abi.name, params.join(", "))
                        }
                    }
                    Err(_) => {
                        // Fallback to event name with raw data
                        format!("{}(...) [decode failed]", event_abi.name)
                    }
                }
            } else {
                // Unknown event signature
                format!(
                    "Event 0x{}... ({} indexed, {} bytes data)",
                    hex::encode(&event_signature.as_slice()[..4]),
                    event.topics().len() - 1,
                    event.data.len()
                )
            }
        } else {
            // No ABI available
            format!(
                "Event 0x{}... ({} indexed, {} bytes data)",
                hex::encode(&event_signature.as_slice()[..4]),
                event.topics().len() - 1,
                event.data.len()
            )
        }
    }

    /// Try to decode constructor arguments for contract creation
    fn format_constructor_call(&self, entry: &TraceEntry) -> Option<String> {
        // Only for contract creation calls
        if !matches!(entry.call_type, CallType::Create(_)) {
            return None;
        }

        // Try to get constructor ABI
        if let Some(abi) = &entry.abi {
            if let Some(constructor) = abi.constructor() {
                // For contract creation, the input contains: bytecode + constructor arguments
                // We need to try to extract just the constructor arguments
                // This is tricky because we don't know the exact bytecode length

                // Simple heuristic: if input is much larger than typical bytecode and has trailing data,
                // assume the trailing data are constructor arguments
                if entry.input.len() > 1000 {
                    // Arbitrary threshold for "has constructor args"
                    // Try different offsets to find valid constructor arguments
                    // Start from the end and work backwards in 32-byte chunks
                    let input_len = entry.input.len();

                    // Calculate expected argument size from constructor ABI
                    let expected_size: usize = constructor
                        .inputs
                        .iter()
                        .map(|input| match input.ty.as_str() {
                            ty if ty.starts_with("uint") || ty.starts_with("int") => 32,
                            "address" => 32, // padded to 32 bytes
                            "bool" => 32,    // padded to 32 bytes
                            ty if ty.starts_with("bytes") && ty != "bytes" => 32, // fixed bytes
                            _ => 32,         // rough estimate for dynamic types
                        })
                        .sum();

                    if input_len >= expected_size {
                        let potential_args = &entry.input[input_len - expected_size..];

                        if let Ok(decoded) = constructor.abi_decode_input(potential_args) {
                            let params: Vec<String> = decoded
                                .iter()
                                .zip(constructor.inputs.iter())
                                .map(|(value, input)| {
                                    format!("{}: {}", input.name, self.format_solidity_value(value))
                                })
                                .collect();

                            return Some(format!("constructor({})", params.join(", ")));
                        }
                    }
                }

                // Fallback: just show that it's a constructor without decoding
                return Some("constructor(...)".to_string());
            }
        }

        None
    }

    /// Convert TokenStyle to ratatui Style using theme colors
    fn get_token_style(&self, token_style: TokenStyle) -> Style {
        let color = match token_style {
            TokenStyle::Keyword => self.color_scheme.syntax_keyword_color,
            TokenStyle::Type => self.color_scheme.syntax_type_color,
            TokenStyle::String => self.color_scheme.syntax_string_color,
            TokenStyle::Number => self.color_scheme.syntax_number_color,
            TokenStyle::Comment => self.color_scheme.syntax_comment_color,
            TokenStyle::Identifier => self.color_scheme.syntax_identifier_color,
            TokenStyle::Operator => self.color_scheme.syntax_operator_color,
            TokenStyle::Punctuation => self.color_scheme.syntax_punctuation_color,
            TokenStyle::Address => self.color_scheme.syntax_address_color,
            TokenStyle::Pragma => self.color_scheme.syntax_pragma_color,
            TokenStyle::Opcode
            | TokenStyle::OpcodeNumber
            | TokenStyle::OpcodeAddress
            | TokenStyle::OpcodeData => self.color_scheme.syntax_opcode_color,
            TokenStyle::Default => self.color_scheme.comment_color,
        };
        Style::default().fg(color)
    }

    /// Apply syntax highlighting to Solidity code using the existing syntax highlighter (for owned strings)
    fn highlight_solidity_code_owned(&self, code: String) -> Vec<Span<'static>> {
        let tokens = self.syntax_highlighter.tokenize(&code, SyntaxType::Solidity);

        let mut spans = Vec::new();
        let mut last_end = 0;

        // Apply syntax highlighting to tokens (same pattern as code panel)
        for token in tokens {
            // Add any unhighlighted text before this token
            if token.start > last_end {
                let unhighlighted = code[last_end..token.start].to_owned();
                if !unhighlighted.is_empty() {
                    spans.push(Span::raw(unhighlighted));
                }
            }

            // Add the highlighted token
            let token_text = code[token.start..token.end].to_owned();
            let token_style =
                self.get_token_style(self.syntax_highlighter.get_token_style(token.token_type));
            spans.push(Span::styled(token_text, token_style));

            last_end = token.end;
        }

        // Add any remaining unhighlighted text
        if last_end < code.len() {
            let remaining = code[last_end..].to_owned();
            if !remaining.is_empty() {
                spans.push(Span::raw(remaining));
            }
        }

        spans
    }
}

impl PanelTr for TracePanel {
    fn panel_type(&self) -> PanelType {
        PanelType::Trace
    }

    fn title(&self) -> String {
        if let Some(trace) = &self.trace_data {
            let display_lines = self.generate_display_lines(trace);
            let visible_entries = trace.iter().filter(|entry| self.is_entry_visible(entry)).count();
            format!("Trace ({} lines, {} entries)", display_lines.len(), visible_entries)
        } else {
            "Trace (Loading...)".to_string()
        }
    }

    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        // Update context height for viewport calculations

        self.context_height = if self.focused && area.height > 10 {
            area.height.saturating_sub(4) // Account for borders and status lines
        } else {
            area.height.saturating_sub(2) // Just borders
        } as usize;

        // Handle different display states
        match self.trace_data {
            // No data: show spinner
            None => {
                let paragraph = Paragraph::new(Line::from(vec![
                    Span::raw("Fetching execution trace "),
                    Span::styled("⠋", Style::default().fg(self.color_scheme.accent_color)),
                ]))
                .block(BorderPresets::trace(
                    self.focused,
                    self.title(),
                    self.color_scheme.focused_border,
                    self.color_scheme.unfocused_border,
                ));
                frame.render_widget(paragraph, area);
                return;
            }
            // Data available
            Some(ref trace) => {
                if trace.is_empty() {
                    let paragraph = Paragraph::new("Trace is empty").block(BorderPresets::trace(
                        self.focused,
                        self.title(),
                        self.color_scheme.focused_border,
                        self.color_scheme.unfocused_border,
                    ));
                    frame.render_widget(paragraph, area);
                    return;
                }

                // Generate display lines with expansion/collapse support
                let display_lines = self.generate_display_lines(trace);

                let items: Vec<ListItem<'_>> = display_lines
                    .iter()
                    .enumerate()
                    .skip(self.scroll_offset)
                    .take(self.context_height)
                    .map(|(viewport_index, line_type)| {
                        let global_index = viewport_index + self.scroll_offset;
                        let formatted_line = self.format_display_line(line_type, trace);

                        let style = if global_index == self.selected_index && self.focused {
                            Style::default()
                                .bg(self.color_scheme.selection_bg)
                                .fg(self.color_scheme.selection_fg)
                        } else if global_index == self.selected_index {
                            Style::default().bg(self.color_scheme.highlight_bg)
                        } else {
                            Style::default()
                        };

                        ListItem::new(formatted_line).style(style)
                    })
                    .collect();

                let list = List::new(items)
                    .block(BorderPresets::trace(
                        self.focused,
                        self.title(),
                        self.color_scheme.focused_border,
                        self.color_scheme.unfocused_border,
                    ))
                    .highlight_style(Style::default().bg(self.color_scheme.selection_bg));

                frame.render_widget(list, area);

                // Add status and help text at the bottom if focused
                if self.focused && area.height > 10 {
                    // Status line
                    let status_area = Rect {
                        x: area.x + 1,
                        y: area.y + area.height - 3,
                        width: area.width - 2,
                        height: 1,
                    };

                    let display_lines = self.generate_display_lines(trace);

                    let status_bar =
                        StatusBar::new().current_panel("Trace".to_string()).message(format!(
                            "Line: {}/{} | Trace: ???/{}",
                            self.selected_index + 1,
                            display_lines.len(),
                            self.trace_data.as_ref().map(|d| d.len()).unwrap_or(0)
                        )); // TODO

                    let status_text = status_bar.build();
                    let status_paragraph = Paragraph::new(status_text)
                        .style(Style::default().fg(self.color_scheme.accent_color));
                    frame.render_widget(status_paragraph, status_area);

                    let help_area = Rect {
                        x: area.x + 1,
                        y: area.y + area.height - 2,
                        width: area.width - 2,
                        height: 1,
                    };
                    let help_text =
                        "↑/↓: Navigate • Space: Toggle expand/collapse • Enter: Jump to snapshot";
                    let help_paragraph = Paragraph::new(help_text)
                        .style(Style::default().fg(self.color_scheme.help_text_color));
                    frame.render_widget(help_paragraph, help_area);
                }
            }
        }
    }

    fn handle_key_event(&mut self, event: KeyEvent) -> Result<EventResponse> {
        if !self.focused || event.kind != KeyEventKind::Press {
            return Ok(EventResponse::NotHandled);
        }

        match event.code {
            KeyCode::Up => {
                self.move_up();
                Ok(EventResponse::Handled)
            }
            KeyCode::Down => {
                self.move_down();
                Ok(EventResponse::Handled)
            }
            KeyCode::Char(' ') => {
                self.toggle_expansion();
                Ok(EventResponse::Handled)
            }
            KeyCode::Enter => {
                if let Some(entry) = self.selected_entry() {
                    debug!("Selected trace entry ID: {} at depth: {}", entry.id, entry.depth);

                    // Jump to the first snapshot of this trace entry if available
                    if let Some(snapshot_id) = entry.first_snapshot_id {
                        debug!("Jumping to snapshot: {}", snapshot_id);
                        // TODO: Use execution manager to set current snapshot
                        // This would require an async RPC call to set_current_snapshot
                    }
                }
                Ok(EventResponse::Handled)
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                // Refresh trace data
                debug!("Refreshing trace data");
                self.trace_data = None;

                Ok(EventResponse::Handled)
            }
            KeyCode::Char(' ') => {
                // Toggle collapse/expand on spacebar
                if let Some(entry) = self.selected_entry() {
                    if self.entry_has_children(entry) {
                        debug!("Toggling collapse for trace entry ID: {}", entry.id);
                        self.toggle_collapse(entry.id);
                    }
                }
                Ok(EventResponse::Handled)
            }
            _ => Ok(EventResponse::NotHandled),
        }
    }

    fn on_focus(&mut self) {
        self.focused = true;
        debug!("Trace panel gained focus");
    }

    fn on_blur(&mut self) {
        self.focused = false;
        debug!("Trace panel lost focus");
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    /// Get execution manager read-only reference
    fn exec_mgr(&self) -> RwLockReadGuard<'_, ExecutionManager> {
        self.execution_manager.read().expect("ExecutionManager lock poisoned")
    }

    /// Get execution manager reference
    fn exec_mgr_mut(&self) -> RwLockWriteGuard<'_, ExecutionManager> {
        self.execution_manager.write().expect("ExecutionManager lock poisoned")
    }

    /// Get resource manager read-only reference
    fn res_mgr(&self) -> RwLockReadGuard<'_, ResourceManager> {
        self.resource_manager.read().expect("ResourceManager lock poisoned")
    }

    /// Get resource manager reference
    fn res_mgr_mut(&self) -> RwLockWriteGuard<'_, ResourceManager> {
        self.resource_manager.write().expect("ResourceManager lock poisoned")
    }

    /// Get theme manager reference
    fn theme_mgr(&self) -> RwLockReadGuard<'_, ThemeManager> {
        self.theme_manager.read().expect("ThemeManager lock poisoned")
    }

    /// Get theme manager reference
    fn theme_mgr_mut(&self) -> RwLockWriteGuard<'_, ThemeManager> {
        self.theme_manager.write().expect("ThemeManager lock poisoned")
    }

    async fn fetch_data(&mut self) -> Result<()> {
        self.res_mgr_mut().fetch_trace().await?;
        let trace_data = self.res_mgr().get_trace().await.cloned();
        self.trace_data = trace_data;

        let color_scheme = self.theme_mgr().get_current_colors();
        self.color_scheme = color_scheme;
        Ok(())
    }
}
