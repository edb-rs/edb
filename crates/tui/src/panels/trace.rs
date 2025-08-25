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
use crate::managers::execution::ExecutionManager;
use crate::managers::info::InfoManager;
use crate::managers::theme::ThemeManager;
use crate::managers::{ExecutionManagerCore, InfoManagerCore, ThemeManagerCore};
use crate::ui::borders::BorderPresets;
use crate::ui::status::StatusBar;
use crate::ui::syntax::{SyntaxHighlighter, SyntaxType};
use crate::ColorScheme;
use alloy_dyn_abi::{DynSolValue, EventExt, FunctionExt, JsonAbiExt};
use alloy_json_abi::{Function, JsonAbi};
use alloy_primitives::{hex, Address, Bytes, LogData, Selector, U256};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use edb_common::types::{CallResult, CallType, Trace, TraceEntry};
use edb_common::SolValueFormatter;
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
    /// Syntax highlighter for Solidity values
    syntax_highlighter: SyntaxHighlighter,
    /// Set of collapsed trace entry IDs (when collapsed, we hide children)
    collapsed_entries: HashSet<usize>,

    // ========== Managers ==========
    /// Shared execution state manager
    exec_mgr: ExecutionManager,
    /// Shared information manager
    info_mgr: InfoManager,
    /// Shared theme manager for styling
    theme_mgr: ThemeManager,
}

impl TracePanel {
    /// Create a new trace panel
    pub fn new(exec_mgr: ExecutionManager, info_mgr: InfoManager, theme_mgr: ThemeManager) -> Self {
        Self {
            selected_index: 0,
            focused: false,
            scroll_offset: 0,
            context_height: 0,
            syntax_highlighter: SyntaxHighlighter::new(),
            collapsed_entries: HashSet::new(),
            exec_mgr,
            info_mgr,
            theme_mgr,
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
        if let Some(trace) = &self.exec_mgr.trace_data {
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
        if let Some(trace) = &self.exec_mgr.trace_data {
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
        if let Some(trace) = &self.exec_mgr.trace_data {
            let display_lines = self.generate_display_lines(trace);
            if let Some(line_type) = display_lines.get(self.selected_index) {
                let entry_id = match line_type {
                    TraceLineType::Call(id) => *id,
                    TraceLineType::Event(id, _) => *id,
                    TraceLineType::Return(id) => *id,
                };

                // Find the entry and only handle children collapse (details always shown)
                if trace.iter().find(|e| e.id == entry_id).is_some() {
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

    /// Adjust selection to stay on the main call line after expansion/collapse
    fn adjust_selection_after_expansion(&mut self, entry_id: usize) {
        if let Some(trace) = &self.exec_mgr.trace_data {
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
                if let Some(trace) = &self.exec_mgr.trace_data {
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

    /// Check if this entry is the last child of its parent
    fn is_last_child(&self, entry: &TraceEntry) -> bool {
        if let Some(parent_id) = entry.parent_id {
            if let Some(trace) = &self.exec_mgr.trace_data {
                // Find all visible siblings (same parent)
                let visible_siblings: Vec<_> = trace
                    .iter()
                    .filter(|e| e.parent_id == Some(parent_id) && self.is_entry_visible(e))
                    .collect();

                // Check if this is the last visible sibling
                if let Some(last) = visible_siblings.last() {
                    return last.id == entry.id;
                }
            }
        }
        false
    }

    /// Check if there are more visible children after this entry at the same level
    fn has_more_children_after(&self, entry: &TraceEntry) -> bool {
        if let Some(parent_id) = entry.parent_id {
            if let Some(trace) = &self.exec_mgr.trace_data {
                // Find if there are any visible siblings after this entry
                let mut found_current = false;
                for e in trace.iter() {
                    if e.parent_id == Some(parent_id) && self.is_entry_visible(e) {
                        if found_current {
                            // Found a visible sibling after the current entry
                            return true;
                        }
                        if e.id == entry.id {
                            found_current = true;
                        }
                    }
                }
            }
        }
        false
    }

    /// Check if an ancestor at a given depth is the last child
    fn is_ancestor_last_child(&self, entry: &TraceEntry, ancestor_depth: usize) -> bool {
        if ancestor_depth >= entry.depth {
            return false;
        }

        // Walk up the parent chain to find the ancestor at the target depth
        let mut current = Some(entry);
        let mut current_depth = entry.depth;

        while let Some(e) = current {
            if current_depth == ancestor_depth {
                return self.is_last_child(e);
            }

            if let Some(parent_id) = e.parent_id {
                if let Some(trace) = &self.exec_mgr.trace_data {
                    current = trace.iter().find(|p| p.id == parent_id);
                    current_depth = current_depth.saturating_sub(1);
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        false
    }

    /// Build tree indentation string based on depth and ancestry
    fn build_tree_indent(&self, entry: &TraceEntry, _is_call_line: bool) -> String {
        if entry.depth == 0 {
            return String::new();
        }

        let mut indent_parts = Vec::new();

        // For each ancestor level, determine if we need a vertical line
        for ancestor_depth in 0..entry.depth.saturating_sub(1) {
            // Check if the ancestor at this depth is the last child
            // If it is, we use spaces; otherwise, we use a vertical line
            if self.is_ancestor_last_child(entry, ancestor_depth + 1) {
                indent_parts.push("  "); // No vertical line for completed branches
            } else {
                indent_parts.push("│ "); // Vertical line for continuing branches
            }
        }

        indent_parts.join("")
    }

    /// Build clean tree indentation for the new design
    fn build_tree_indent_clean(&self, entry: &TraceEntry) -> String {
        if entry.depth == 0 {
            return String::new();
        }

        let mut indent_parts = Vec::new();

        // For each ancestor level, determine if we need a vertical line
        for ancestor_depth in 0..entry.depth.saturating_sub(1) {
            if self.is_ancestor_last_child(entry, ancestor_depth + 1) {
                indent_parts.push("  "); // Spaces for completed branches
            } else {
                indent_parts.push("│ "); // Vertical line for continuing branches
            }
        }

        indent_parts.join("")
    }

    /// Format a compact trace entry (main call line without events/returns)
    fn format_trace_entry_compact(&self, entry: &TraceEntry, depth: usize) -> Line<'static> {
        // Check if this entry has children
        let has_children = if let Some(trace) = &self.exec_mgr.trace_data {
            trace.iter().any(|e| e.parent_id == Some(entry.id))
        } else {
            false
        };

        // Build the line prefix as a single string
        let line_prefix = if depth == 0 {
            // Root level: collapse indicator at start, then tree structure if has siblings
            let collapse_char = if has_children {
                if self.collapsed_entries.contains(&entry.id) {
                    "▶ "
                } else {
                    "▼ "
                }
            } else {
                "  " // spaces for alignment when no children
            };

            // For root level, check if this is the last root entry
            if self.is_last_child(entry) {
                format!("{}", collapse_char)
            } else {
                format!("{}", collapse_char)
            }
        } else {
            // Child level: tree structure with proper spacing
            let tree_indent = self.build_tree_indent_clean(entry);
            let connector = if self.is_last_child(entry) { "└─" } else { "├─" };

            // Add collapse indicator if has children, otherwise just a space
            if has_children {
                let collapse_char =
                    if self.collapsed_entries.contains(&entry.id) { "▶ " } else { "▼ " };
                format!("  {}{}{}", tree_indent, connector, collapse_char)
            } else {
                format!("  {}{}  ", tree_indent, connector)
            }
        };

        let (call_type_str, call_color) = match &entry.call_type {
            CallType::Call(CallScheme::Call) => ("CALL", self.theme_mgr.color_scheme.call_color),
            CallType::Call(CallScheme::CallCode) => {
                ("CALLCODE", self.theme_mgr.color_scheme.call_color)
            }
            CallType::Call(CallScheme::DelegateCall) => {
                ("DELEGATECALL", self.theme_mgr.color_scheme.call_color)
            }
            CallType::Call(CallScheme::StaticCall) => {
                ("STATICCALL", self.theme_mgr.color_scheme.call_color)
            }
            CallType::Create(CreateScheme::Create) => {
                ("CREATE", self.theme_mgr.color_scheme.create_color)
            }
            CallType::Create(CreateScheme::Create2 { .. }) => {
                ("CREATE2", self.theme_mgr.color_scheme.create_color)
            }
            CallType::Create(CreateScheme::Custom { .. }) => {
                ("CUSTOM_CREATE", self.theme_mgr.color_scheme.create_color)
            }
        };

        // Build spans with the new format
        let mut spans = vec![
            Span::styled(
                line_prefix,
                Style::default().fg(self.theme_mgr.color_scheme.comment_color),
            ),
            Span::styled(call_type_str, Style::default().fg(call_color)),
            Span::raw(" "),
            Span::styled(
                self.format_address_readable(entry.code_address),
                Style::default().fg(self.theme_mgr.color_scheme.accent_color),
            ),
        ];

        // Add function call details
        if matches!(entry.call_type, CallType::Create(_)) {
            if let Some(constructor_call) = self.format_constructor_call(entry) {
                spans.push(Span::raw(" "));
                spans.push(Span::styled(
                    constructor_call,
                    Style::default().fg(self.theme_mgr.color_scheme.keyword_color),
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
                            Style::default().fg(self.theme_mgr.color_scheme.syntax_string_color),
                        ));
                    } else {
                        spans.push(Span::styled(
                            format!("0x{}", selector),
                            Style::default().fg(self.theme_mgr.color_scheme.syntax_string_color),
                        ));
                    }
                }
            }
        }

        // Add self-destruct indicator if present
        if let Some((beneficiary, value)) = &entry.self_destruct {
            spans.push(Span::styled(
                " [SELFDESTRUCT]",
                Style::default().fg(self.theme_mgr.color_scheme.error_color),
            ));
            spans.push(Span::styled(
                format!(
                    " → {} ({} ETH)",
                    self.format_address_readable(*beneficiary),
                    self.format_ether(*value)
                ),
                Style::default().fg(self.theme_mgr.color_scheme.warning_color),
            ));
        }

        // Result indicator with more specific symbols based on InstructionResult
        let (result_char, result_color) = match &entry.result {
            Some(CallResult::Success { result, .. }) => match result {
                InstructionResult::Return => ("✓", self.theme_mgr.color_scheme.success_color),
                InstructionResult::Stop => ("•", self.theme_mgr.color_scheme.success_color),
                InstructionResult::SelfDestruct => ("†", self.theme_mgr.color_scheme.warning_color),
                _ => ("✓", self.theme_mgr.color_scheme.success_color),
            },
            Some(CallResult::Revert { result, .. }) => match result {
                InstructionResult::Revert => ("↩", self.theme_mgr.color_scheme.error_color),
                _ => ("✗", self.theme_mgr.color_scheme.error_color),
            },
            Some(CallResult::Error { result, .. }) => match result {
                InstructionResult::OutOfGas => ("G", self.theme_mgr.color_scheme.warning_color),
                InstructionResult::StackOverflow => {
                    ("S", self.theme_mgr.color_scheme.warning_color)
                }
                InstructionResult::OpcodeNotFound => ("X", self.theme_mgr.color_scheme.error_color),
                InstructionResult::OutOfFunds => ("F", self.theme_mgr.color_scheme.warning_color),
                _ => ("E", self.theme_mgr.color_scheme.warning_color),
            },
            None => ("?", self.theme_mgr.color_scheme.comment_color),
        };

        spans.push(Span::raw(" "));
        spans.push(Span::styled(result_char, Style::default().fg(result_color)));

        Line::from(spans)
    }

    /// Format an event line
    fn format_event_line(&self, entry: &TraceEntry, event_idx: usize) -> Line<'static> {
        if let Some(event) = entry.events.get(event_idx) {
            // Detail line: vertical connector + spaces + dot
            let full_indent = if entry.depth == 0 {
                // Root level details - check if this root entry has more siblings
                if self.is_last_child(entry) {
                    "      ".to_string() // Last root entry, no vertical line
                } else {
                    "  │   ".to_string() // More root entries follow, show vertical line
                }
            } else {
                // Child level details - always start with 2 spaces, then tree indent, then connector
                let tree_indent = self.build_tree_indent_clean(entry);
                if self.is_last_child(entry) {
                    format!("  {}      ", tree_indent) // Parent is last child, no vertical line
                } else {
                    format!("  {}│     ", tree_indent) // Parent has more siblings, continue vertical line
                }
            };
            let event_text = self.format_event(event, entry.abi.as_ref());

            Line::from(vec![
                Span::styled(
                    full_indent,
                    Style::default().fg(self.theme_mgr.color_scheme.comment_color),
                ),
                Span::styled("· ", Style::default().fg(self.theme_mgr.color_scheme.comment_color)),
                Span::styled(
                    "[EVENT] ",
                    Style::default().fg(self.theme_mgr.color_scheme.comment_color),
                ),
                Span::styled(
                    event_text,
                    Style::default().fg(self.theme_mgr.color_scheme.accent_color),
                ),
            ])
        } else {
            Line::from("Invalid event")
        }
    }

    /// Format a return value line  
    fn format_return_line(&self, entry: &TraceEntry) -> Line<'static> {
        // Detail line: vertical connector + spaces + dot
        let full_indent = if entry.depth == 0 {
            // Root level details - check if this root entry has more siblings
            if self.is_last_child(entry) {
                "      ".to_string() // Last root entry, no vertical line
            } else {
                "  │   ".to_string() // More root entries follow, show vertical line
            }
        } else {
            // Child level details - always start with 2 spaces, then tree indent, then connector
            let tree_indent = self.build_tree_indent_clean(entry);
            if self.is_last_child(entry) {
                format!("  {}      ", tree_indent) // Parent is last child, no vertical line
            } else {
                format!("  {}│     ", tree_indent) // Parent has more siblings, continue vertical line
            }
        };

        match &entry.result {
            Some(CallResult::Success { output, .. }) => {
                let return_text = if output.is_empty() {
                    "()".to_string()
                } else {
                    // Try to decode return value using ABI
                    self.decode_return_value(entry, output)
                };

                Line::from(vec![
                    Span::styled(
                        full_indent,
                        Style::default().fg(self.theme_mgr.color_scheme.comment_color),
                    ),
                    Span::styled(
                        "· ",
                        Style::default().fg(self.theme_mgr.color_scheme.comment_color),
                    ),
                    Span::styled(
                        "[RETURN] ",
                        Style::default().fg(self.theme_mgr.color_scheme.comment_color),
                    ),
                    Span::styled(
                        return_text,
                        Style::default().fg(self.theme_mgr.color_scheme.syntax_string_color),
                    ),
                ])
            }
            Some(CallResult::Revert { output, .. }) => {
                let revert_text = self.decode_revert_reason(output);
                Line::from(vec![
                    Span::styled(
                        full_indent,
                        Style::default().fg(self.theme_mgr.color_scheme.comment_color),
                    ),
                    Span::styled(
                        "· ",
                        Style::default().fg(self.theme_mgr.color_scheme.comment_color),
                    ),
                    Span::styled(
                        "[REVERT] ",
                        Style::default().fg(self.theme_mgr.color_scheme.error_color),
                    ),
                    Span::styled(
                        revert_text,
                        Style::default().fg(self.theme_mgr.color_scheme.syntax_string_color),
                    ),
                ])
            }
            Some(CallResult::Error { output, result }) => {
                let error_text = self.format_instruction_result(*result, output);
                Line::from(vec![
                    Span::styled(
                        full_indent,
                        Style::default().fg(self.theme_mgr.color_scheme.comment_color),
                    ),
                    Span::styled(
                        "· ",
                        Style::default().fg(self.theme_mgr.color_scheme.comment_color),
                    ),
                    Span::styled(
                        "[ERROR] ",
                        Style::default().fg(self.theme_mgr.color_scheme.error_color),
                    ),
                    Span::styled(
                        error_text,
                        Style::default().fg(self.theme_mgr.color_scheme.error_color),
                    ),
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
                                    "{} {}: {}",
                                    value.format_type(),
                                    name,
                                    value.format_value(false)
                                ));
                            } else {
                                return_parts.push(value.format_value(true));
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
                if let DynSolValue::String(reason) = decoded {
                    return format!("\"{}\"", reason);
                }
            }
        }

        // Check if it's a Panic(uint256) revert (0x4e487b71)
        if output.len() >= 4 && output.starts_with(&[0x4e, 0x48, 0x7b, 0x71]) {
            if let Ok(decoded) = alloy_dyn_abi::DynSolType::Uint(256).abi_decode(&output[4..]) {
                if let DynSolValue::Uint(panic_code, _) = decoded {
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
                    decoded.iter().map(|param| param.format_value(true)).collect();

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
        self.highlight_solidity_code(call_string)
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

    /// Format event data using ABI if available
    fn format_event(&self, event: &LogData, abi: Option<&JsonAbi>) -> String {
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
                                "{} {}: {}",
                                value.format_type(),
                                param.name,
                                value.format_value(false)
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
                                    format!(
                                        "{} {}: {}",
                                        value.format_type(),
                                        input.name,
                                        value.format_value(false)
                                    )
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

    /// Apply syntax highlighting to Solidity code using the existing syntax highlighter (for owned strings)
    fn highlight_solidity_code(&self, code: String) -> Vec<Span<'static>> {
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
            let token_style = self
                .syntax_highlighter
                .get_token_style(token.token_type, &self.theme_mgr.color_scheme);
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
        if let Some(trace) = &self.exec_mgr.trace_data {
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
        match self.exec_mgr.trace_data {
            // No data: show spinner
            None => {
                let paragraph = Paragraph::new(Line::from(vec![
                    Span::raw("Fetching execution trace "),
                    Span::styled(
                        "⠋",
                        Style::default().fg(self.theme_mgr.color_scheme.accent_color),
                    ),
                ]))
                .block(BorderPresets::trace(
                    self.focused,
                    self.title(),
                    self.theme_mgr.color_scheme.focused_border,
                    self.theme_mgr.color_scheme.unfocused_border,
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
                        self.theme_mgr.color_scheme.focused_border,
                        self.theme_mgr.color_scheme.unfocused_border,
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
                                .bg(self.theme_mgr.color_scheme.selection_bg)
                                .fg(self.theme_mgr.color_scheme.selection_fg)
                        } else if global_index == self.selected_index {
                            Style::default().bg(self.theme_mgr.color_scheme.highlight_bg)
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
                        self.theme_mgr.color_scheme.focused_border,
                        self.theme_mgr.color_scheme.unfocused_border,
                    ))
                    .highlight_style(Style::default().bg(self.theme_mgr.color_scheme.selection_bg));

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
                            self.exec_mgr.trace_data.as_ref().map(|d| d.len()).unwrap_or(0)
                        )); // TODO

                    let status_text = status_bar.build();
                    let status_paragraph = Paragraph::new(status_text)
                        .style(Style::default().fg(self.theme_mgr.color_scheme.accent_color));
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
                        .style(Style::default().fg(self.theme_mgr.color_scheme.help_text_color));
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

    async fn fetch_data(&mut self) -> Result<()> {
        self.exec_mgr.fetch_data().await?;
        self.info_mgr.fetch_data().await?;
        self.theme_mgr.fetch_data().await?;

        Ok(())
    }
}
