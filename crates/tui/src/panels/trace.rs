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
use alloy_dyn_abi::{DynSolValue, FunctionExt, JsonAbiExt};
use alloy_json_abi::Function;
use alloy_primitives::{hex, Address, Bytes, U256};
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
use revm::{context::CreateScheme, interpreter::CallScheme};
use std::collections::HashSet;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};
use tracing::debug;

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
    /// Set of collapsed trace entry IDs
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
            if self.selected_index < self.scroll_offset {
                self.scroll_offset = self.selected_index;
            }
        }
    }

    /// Move selection down
    fn move_down(&mut self) {
        if let Some(trace) = &self.trace_data {
            let visible_entries: Vec<&TraceEntry> =
                trace.iter().filter(|entry| self.is_entry_visible(entry)).collect();
            let max_lines = visible_entries.len();

            if self.selected_index < max_lines.saturating_sub(1) {
                self.selected_index += 1;
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
            let visible_entries: Vec<&TraceEntry> =
                trace.iter().filter(|entry| self.is_entry_visible(entry)).collect();
            visible_entries.get(self.selected_index).copied()
        } else {
            None
        }
    }

    /// Update trace data from execution manager (simplified for now)
    fn update_trace_data(&mut self) {
        // For now, we'll implement a simple polling approach
        // In a real implementation, this would need proper async handling
        // or message passing between async tasks and the UI thread
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
        if let Some(function_abi) = &entry.function_abi {
            spans.push(Span::raw(" "));
            spans.extend(self.format_function_call_with_highlighting(function_abi, &entry.input));
        } else if !entry.input.is_empty() {
            spans.push(Span::raw(" "));
            // Try to show function selector and data size instead of raw hex
            if entry.input.len() >= 4 {
                let selector = hex::encode(&entry.input[..4]);
                let data_size = entry.input.len() - 4;
                if data_size > 0 {
                    spans.push(Span::styled(
                        format!("{}({} bytes)", selector, data_size),
                        Style::default().fg(self.color_scheme.syntax_string_color),
                    ));
                } else {
                    spans.push(Span::styled(
                        format!("{}()", selector),
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

        // Format value if present
        if entry.value > U256::ZERO {
            spans.push(Span::styled(
                format!(" value: {} ETH", self.format_ether(entry.value)),
                Style::default().fg(self.color_scheme.warning_color),
            ));
        }

        // Result indicator
        let (result_char, result_color) = match &entry.result {
            Some(CallResult::Success { .. }) => ("✓", self.color_scheme.success_color),
            Some(CallResult::Revert { .. }) => ("✗", self.color_scheme.error_color),
            Some(CallResult::Error { .. }) => ("☠", self.color_scheme.error_color), // TODO (change icon)
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
                if let Some(function_abi) = &entry.function_abi {
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
        } else if let Some(CallResult::Revert { output }) = &entry.result {
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

    /// Format a Solidity value with syntax highlighting using the syntax highlighter
    fn format_solidity_value_with_highlighting(&self, value: &DynSolValue) -> Vec<Span<'static>> {
        // Convert the value to string first
        let value_str = self.format_solidity_value(value);

        // Use the syntax highlighter to parse and highlight the value
        self.highlight_solidity_code_owned(value_str)
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

    /// Apply syntax highlighting to Solidity code using the existing syntax highlighter
    fn highlight_solidity_code<'a>(&'a self, code: &'a str) -> Vec<Span<'a>> {
        let tokens = self.syntax_highlighter.tokenize(code, SyntaxType::Solidity);

        let mut spans = Vec::new();
        let mut last_end = 0;

        // Apply syntax highlighting to tokens (same pattern as code panel)
        for token in tokens {
            // Add any unhighlighted text before this token
            if token.start > last_end {
                let unhighlighted = &code[last_end..token.start];
                if !unhighlighted.is_empty() {
                    spans.push(Span::raw(unhighlighted));
                }
            }

            // Add the highlighted token
            let token_text = &code[token.start..token.end];
            let token_style =
                self.get_token_style(self.syntax_highlighter.get_token_style(token.token_type));
            spans.push(Span::styled(token_text, token_style));

            last_end = token.end;
        }

        // Add any remaining unhighlighted text
        if last_end < code.len() {
            let remaining = &code[last_end..];
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
            let visible_count = trace.iter().filter(|entry| self.is_entry_visible(entry)).count();
            if visible_count == trace.len() {
                format!("Trace ({} entries)", trace.len())
            } else {
                format!("Trace ({}/{} entries)", visible_count, trace.len())
            }
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

                // Create list items with smart formatting, filtering out collapsed entries
                let visible_entries: Vec<&TraceEntry> =
                    trace.iter().filter(|entry| self.is_entry_visible(entry)).collect();

                let items: Vec<ListItem<'_>> = visible_entries
                    .iter()
                    .enumerate()
                    .skip(self.scroll_offset)
                    .take(self.context_height)
                    .map(|(i, entry)| {
                        let formatted_line = self.format_trace_entry(entry, entry.depth);

                        let style = if i == self.selected_index && self.focused {
                            Style::default()
                                .bg(self.color_scheme.selection_bg)
                                .fg(self.color_scheme.selection_fg)
                        } else if i == self.selected_index {
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

                    let visible_entries: Vec<&TraceEntry> =
                        trace.iter().filter(|entry| self.is_entry_visible(entry)).collect();

                    let status_bar =
                        StatusBar::new().current_panel("Trace".to_string()).message(format!(
                            "Entry: {}/{} | Depth: {} | Space: Toggle collapse",
                            self.selected_index + 1,
                            visible_entries.len(),
                            if let Some(entry) = visible_entries.get(self.selected_index) {
                                entry.depth
                            } else {
                                0
                            }
                        ));

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
                        "↑/↓: Navigate • Space: Collapse/Expand • Enter: Jump to snapshot";
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
