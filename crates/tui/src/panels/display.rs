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

//! Display panel for variables, stack, memory, and other debugging information
//!
//! This panel can switch between different display modes based on context.

use super::{EventResponse, PanelTr, PanelType};
use crate::data::DataManager;
use crate::ui::borders::BorderPresets;
use crate::ui::status::StatusBar;
use alloy_primitives::{Address, Bytes, U256};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use edb_common::types::{OpcodeSnapshotInfoDetail, SnapshotInfoDetail};
use eyre::Result;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, Paragraph},
    Frame,
};
use revm::state::TransientStorage;
use std::collections::HashMap;
use tracing::debug;

/// Display modes for the panel
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayMode {
    /// Show variables in scope (for hooked snapshots)
    Variables,
    /// Show breakpoints (for hooked snapshots)
    Breakpoints,
    /// Show current stack (for opcode snapshots)
    Stack,
    /// Show memory contents (for opcode snapshots)
    Memory,
    /// Show call data (for opcode snapshots)
    CallData,
    /// Show storage state (for opcode snapshots)
    Storage,
    /// Show transient storage state (for opcode snapshots)
    TransientStorage,
}

impl DisplayMode {
    /// Get display name for the mode
    pub fn name(&self) -> &'static str {
        match self {
            DisplayMode::Variables => "Variables",
            DisplayMode::Breakpoints => "Breakpoints",
            DisplayMode::Stack => "Stack",
            DisplayMode::Memory => "Memory",
            DisplayMode::CallData => "Call Data",
            DisplayMode::Storage => "Storage",
            DisplayMode::TransientStorage => "Transient Storage",
        }
    }
}

/// Represents a stack item with potential diff
#[derive(Debug, Clone)]
struct StackItem {
    index: usize,
    value: U256,
    diff_status: DiffStatus,
}

/// Represents a memory chunk with potential diff
#[derive(Debug, Clone)]
struct MemoryChunk {
    offset: usize,
    data: Vec<u8>,
    changed_bytes: Vec<usize>, // Indices of changed bytes within chunk
}

/// Status of an item compared to previous snapshot
#[derive(Debug, Clone, Copy, PartialEq)]
enum DiffStatus {
    Unchanged,
    New,
    Modified,
}

/// Display panel implementation
#[derive(Debug)]
pub struct DisplayPanel {
    // ========== Display ==========
    /// Current display mode
    mode: DisplayMode,
    /// Available modes based on snapshot type
    available_modes: Vec<DisplayMode>,
    /// Selected item index
    selected_index: usize,
    /// Scroll offset for auto-scrolling
    scroll_offset: usize,
    /// Content height for viewport calculations
    context_height: usize,
    /// Content width for viewport calculations
    content_width: usize,
    /// Horizontal scroll offset
    horizontal_offset: usize,
    /// Maximum line width for horizontal scrolling
    max_line_width: usize,
    /// Whether this panel is focused
    focused: bool,

    // ========== Data (Flag) ==========
    /// Current execution snapshot
    current_execution_snapshot: Option<usize>,
    /// Whether current snapshot is opcode or hooked
    is_opcode_snapshot: bool,

    // ========== Data (Cached) ==========
    /// Stack items with diff status
    stack_items: Vec<StackItem>,
    /// Memory chunks with diff status
    memory_chunks: Vec<MemoryChunk>,
    /// Raw calldata
    calldata: Bytes,
    /// Storage changes (slot -> (old_value, new_value))
    storage_changes: HashMap<U256, (U256, U256)>,
    /// Transient storage data
    transient_storage: HashMap<U256, U256>,
    /// Mock variables for hooked snapshots
    variables: Vec<String>,
    /// Mock breakpoints for hooked snapshots
    breakpoints: Vec<String>,
}

impl DisplayPanel {
    /// Create a new display panel
    pub fn new() -> Self {
        Self {
            mode: DisplayMode::Stack,
            available_modes: vec![DisplayMode::Stack],
            selected_index: 0,
            scroll_offset: 0,
            context_height: 0,
            content_width: 0,
            horizontal_offset: 0,
            max_line_width: 0,
            focused: false,
            current_execution_snapshot: None,
            is_opcode_snapshot: true,
            stack_items: Vec::new(),
            memory_chunks: Vec::new(),
            calldata: Bytes::new(),
            storage_changes: HashMap::new(),
            transient_storage: HashMap::new(),
            variables: vec![
                "totalSupply: uint256 = 1000000".to_string(),
                "balances[msg.sender]: uint256 = 500000".to_string(),
                "msg.sender: address = 0x1234...abcd".to_string(),
            ],
            breakpoints: vec![
                "Line 42: Transfer.sol - require(balance >= amount)".to_string(),
                "Line 58: Token.sol - emit Transfer(from, to, amount)".to_string(),
            ],
        }
    }

    /// Update snapshot data from data manager
    fn update_snapshot_data(&mut self, dm: &mut DataManager) -> Option<()> {
        let current_snapshot = dm.execution.get_current_snapshot();

        // Check if snapshot changed
        if self.current_execution_snapshot == Some(current_snapshot) {
            return Some(());
        }

        // Get snapshot info
        let snapshot_info = dm.execution.get_snapshot_info(current_snapshot)?.clone();

        // Determine snapshot type and update available modes
        match snapshot_info.detail() {
            SnapshotInfoDetail::Opcode(opcode_detail) => {
                self.is_opcode_snapshot = true;
                self.available_modes = vec![
                    DisplayMode::Stack,
                    DisplayMode::Memory,
                    DisplayMode::CallData,
                    DisplayMode::Storage,
                    DisplayMode::TransientStorage,
                ];

                // Update opcode-specific data
                self.update_opcode_data(opcode_detail, current_snapshot, dm)?;

                // Switch to Stack mode if not already in an opcode mode
                if !self.available_modes.contains(&self.mode) {
                    self.mode = DisplayMode::Stack;
                    self.selected_index = 0;
                    self.scroll_offset = 0;
                }
            }
            SnapshotInfoDetail::Hook(_hook_detail) => {
                self.is_opcode_snapshot = false;
                self.available_modes = vec![DisplayMode::Variables, DisplayMode::Breakpoints];

                // Switch to Variables mode if not already in a hook mode
                if !self.available_modes.contains(&self.mode) {
                    self.mode = DisplayMode::Variables;
                    self.selected_index = 0;
                    self.scroll_offset = 0;
                }
            }
        }

        self.current_execution_snapshot = Some(current_snapshot);

        Some(())
    }

    /// Update data for opcode snapshots
    fn update_opcode_data(
        &mut self,
        opcode_detail: &OpcodeSnapshotInfoDetail,
        current_id: usize,
        dm: &mut DataManager,
    ) -> Option<()> {
        let current_addr = dm.execution.get_current_address()?;

        // Get previous snapshot for diff calculation
        let prev_snapshot_detail = if current_id > 0 {
            dm.execution.get_snapshot_info(current_id - 1).and_then(|info| match info.detail() {
                SnapshotInfoDetail::Opcode(detail) => Some(detail.clone()),
                _ => None,
            })
        } else {
            None
        };

        // Update stack with diff
        self.update_stack_diff(
            &opcode_detail.stack,
            prev_snapshot_detail.as_ref().map(|d| &d.stack),
        );

        // Update memory with diff
        self.update_memory_diff(
            &opcode_detail.memory,
            prev_snapshot_detail.as_ref().map(|d| &d.memory),
        );

        // Update calldata
        self.calldata = opcode_detail.calldata.clone();

        // Update transient storage (only for current address)
        self.update_transient_storage(&opcode_detail.transition_storage, current_addr);

        // Get storage changes from execution manager
        if let Some(storage_diff) = dm.execution.get_storage_diff(current_id) {
            self.storage_changes = storage_diff.clone();
        } else {
            // If not available yet, keep existing or clear
            self.storage_changes.clear();
        }

        Some(())
    }

    /// Update stack items with diff status
    fn update_stack_diff(&mut self, current_stack: &[U256], prev_stack: Option<&Vec<U256>>) {
        self.stack_items.clear();

        if let Some(prev) = prev_stack {
            // Compare stacks to find diffs
            for (idx, value) in current_stack.iter().enumerate() {
                let diff_status = if idx >= prev.len() {
                    DiffStatus::New
                } else if prev[idx] != *value {
                    DiffStatus::Modified
                } else {
                    DiffStatus::Unchanged
                };

                self.stack_items.push(StackItem { index: idx, value: *value, diff_status });
            }
        } else {
            // First snapshot - all items are new
            for (idx, value) in current_stack.iter().enumerate() {
                self.stack_items.push(StackItem {
                    index: idx,
                    value: *value,
                    diff_status: DiffStatus::New,
                });
            }
        }
    }

    /// Update memory chunks with diff status
    fn update_memory_diff(&mut self, current_memory: &[u8], prev_memory: Option<&Vec<u8>>) {
        self.memory_chunks.clear();

        // Process memory in 32-byte chunks
        const CHUNK_SIZE: usize = 32;

        for offset in (0..current_memory.len()).step_by(CHUNK_SIZE) {
            let end = (offset + CHUNK_SIZE).min(current_memory.len());
            let chunk_data = current_memory[offset..end].to_vec();

            // Find changed bytes if we have previous memory
            let changed_bytes = if let Some(prev) = prev_memory {
                let mut changes = Vec::new();
                for i in 0..chunk_data.len() {
                    let mem_idx = offset + i;
                    if mem_idx >= prev.len() || prev[mem_idx] != chunk_data[i] {
                        changes.push(i);
                    }
                }
                changes
            } else {
                // All bytes are new
                (0..chunk_data.len()).collect()
            };

            if !chunk_data.is_empty() {
                self.memory_chunks.push(MemoryChunk { offset, data: chunk_data, changed_bytes });
            }
        }
    }

    /// Update transient storage
    fn update_transient_storage(&mut self, tstorage: &TransientStorage, current_addr: Address) {
        self.transient_storage.clear();

        // Only copy transient storage data for the current address
        for ((addr, slot), value) in tstorage.iter() {
            if *addr == current_addr {
                self.transient_storage.insert(*slot, *value);
            }
        }
    }

    /// Calculate the maximum line width for horizontal scrolling
    fn calculate_max_line_width(&mut self) {
        self.max_line_width = match self.mode {
            DisplayMode::Stack => {
                // Find longest stack item (index + hex + decoded + diff indicator)
                self.stack_items
                    .iter()
                    .map(|item| {
                        let index_str = format!("[{:2}]", item.index);
                        let value_str = self.format_u256_with_decode(&item.value);
                        let diff_indicator = match item.diff_status {
                            DiffStatus::New => " [NEW]",
                            DiffStatus::Modified => " [CHG]",
                            _ => "",
                        };
                        format!("{} {} {}", index_str, value_str, diff_indicator).len()
                    })
                    .max()
                    .unwrap_or(0)
            }
            DisplayMode::Memory => {
                // Memory lines: "0x1234: " + hex bytes + "  " + ascii
                // 32 bytes = 64 hex chars + 31 spaces + 2 spaces + 32 ascii = ~129 chars
                // Plus offset (8 chars) = ~137 chars
                145
            }
            DisplayMode::CallData => {
                // Calldata: hex only format
                // 32 bytes = 64 hex chars + 31 spaces + offset (8 chars) = ~103 chars
                110
            }
            DisplayMode::Storage => {
                // Storage format: "Slot: 0x..." + "  From: 0x..." + "  To:   0x..."
                // Each change takes 3 lines, each line ~75 chars
                75
            }
            DisplayMode::TransientStorage => {
                // Format: "Slot: 0x... = 0x..."
                // Slot (66) + value (66) + formatting = ~140
                140
            }
            DisplayMode::Variables => self.variables.iter().map(|s| s.len()).max().unwrap_or(0),
            DisplayMode::Breakpoints => self.breakpoints.iter().map(|s| s.len()).max().unwrap_or(0),
        };
    }

    /// Switch to next display mode
    fn next_mode(&mut self) {
        if let Some(current_idx) = self.available_modes.iter().position(|m| *m == self.mode) {
            let next_idx = (current_idx + 1) % self.available_modes.len();
            self.mode = self.available_modes[next_idx];
            self.selected_index = 0;
            self.scroll_offset = 0;
            self.horizontal_offset = 0;
            debug!("Switched to display mode: {:?}", self.mode);
        }
    }

    /// Switch to previous display mode
    fn prev_mode(&mut self) {
        if let Some(current_idx) = self.available_modes.iter().position(|m| *m == self.mode) {
            let prev_idx =
                if current_idx == 0 { self.available_modes.len() - 1 } else { current_idx - 1 };
            self.mode = self.available_modes[prev_idx];
            self.selected_index = 0;
            self.scroll_offset = 0;
            self.horizontal_offset = 0;
            debug!("Switched to display mode: {:?}", self.mode);
        }
    }

    /// Format a U256 value as hex with decoded string
    fn format_u256_with_decode(&self, value: &U256) -> String {
        let hex_str = format!("{:#066x}", value);

        // Try to decode as ASCII/UTF-8
        let bytes = value.to_be_bytes::<32>();
        let mut decoded = String::new();
        for byte in bytes.iter() {
            if byte.is_ascii_graphic() || *byte == b' ' {
                decoded.push(*byte as char);
            } else {
                decoded.push('.'); // Show all non-printable bytes (including 0) as dots
            }
        }

        // Always show the decoded string (no trimming of leading zeros)
        format!("{} {}", hex_str, decoded)
    }

    /// Move selection up with auto-scrolling
    fn move_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
            // Auto-scroll up if selection moves above visible area
            if self.selected_index < self.scroll_offset {
                self.scroll_offset = self.selected_index;
            }
        }
    }

    /// Move selection down with auto-scrolling
    fn move_down(&mut self) {
        let max_items = match self.mode {
            DisplayMode::Stack => self.stack_items.len(),
            DisplayMode::Memory => self.memory_chunks.len(),
            DisplayMode::CallData => {
                if self.calldata.is_empty() {
                    0
                } else {
                    (self.calldata.len() + 31) / 32
                }
            }
            DisplayMode::Storage => self.storage_changes.len() * 3, // 3 lines per change
            DisplayMode::TransientStorage => self.transient_storage.len(),
            DisplayMode::Variables => self.variables.len(),
            DisplayMode::Breakpoints => self.breakpoints.len(),
        };

        if self.selected_index < max_items.saturating_sub(1) {
            self.selected_index += 1;
            let viewport_height = self.context_height;
            // Auto-scroll down if selection moves below visible area
            if self.selected_index >= self.scroll_offset + viewport_height {
                self.scroll_offset = (self.selected_index + 1).saturating_sub(viewport_height);
            }
        }
    }

    /// Render stack display
    fn render_stack(&mut self, frame: &mut Frame<'_>, area: Rect, dm: &mut DataManager) {
        if self.stack_items.is_empty() {
            let paragraph = Paragraph::new("Stack is empty").block(BorderPresets::display(
                self.focused,
                self.title(dm),
                dm.theme.focused_border,
                dm.theme.unfocused_border,
            ));
            frame.render_widget(paragraph, area);
            return;
        }

        // Create list items for stack display (reversed - stack top first)
        let items: Vec<ListItem<'_>> = self
            .stack_items
            .iter()
            .rev() // Reverse to show stack top first
            .enumerate()
            .skip(self.scroll_offset)
            .take(self.context_height)
            .map(|(display_idx, item)| {
                let is_selected = display_idx == self.selected_index;

                // Format the stack item
                let index_str = format!("[{:2}]", item.index);
                let value_str = self.format_u256_with_decode(&item.value);

                // Add diff indicator
                let diff_indicator = match item.diff_status {
                    DiffStatus::New => " [NEW]",
                    DiffStatus::Modified => " [CHG]",
                    _ => "",
                };

                let content = format!("{} {} {}", index_str, value_str, diff_indicator);

                // Apply styling based on diff status and selection
                let mut style = if is_selected && self.focused {
                    Style::default().bg(dm.theme.selection_bg).fg(dm.theme.selection_fg)
                } else {
                    Style::default()
                };

                // Highlight based on diff status
                if item.diff_status == DiffStatus::New {
                    style = style.fg(Color::Green);
                } else if item.diff_status == DiffStatus::Modified {
                    style = style.fg(Color::Yellow);
                }

                ListItem::new(content).style(style)
            })
            .collect();

        let list = List::new(items).block(BorderPresets::display(
            self.focused,
            self.title(dm),
            dm.theme.focused_border,
            dm.theme.unfocused_border,
        ));

        frame.render_widget(list, area);
        self.render_status_and_help(frame, area, dm);
    }

    /// Render memory display
    fn render_memory(&mut self, frame: &mut Frame<'_>, area: Rect, dm: &mut DataManager) {
        if self.memory_chunks.is_empty() {
            let paragraph = Paragraph::new("Memory is empty").block(BorderPresets::display(
                self.focused,
                self.title(dm),
                dm.theme.focused_border,
                dm.theme.unfocused_border,
            ));
            frame.render_widget(paragraph, area);
            return;
        }

        // Create list items for memory display
        let items: Vec<ListItem<'_>> = self
            .memory_chunks
            .iter()
            .enumerate()
            .skip(self.scroll_offset)
            .take(self.context_height)
            .map(|(display_idx, chunk)| {
                let is_selected = display_idx == self.selected_index;

                // Format offset
                let offset_str = format!("0x{:04x}:", chunk.offset);

                // Format bytes with highlighting
                let byte_spans = format_bytes_with_decode(&chunk.data, &chunk.changed_bytes);

                let mut line = vec![Span::raw(offset_str), Span::raw(" ")];
                line.extend(byte_spans);

                let style = if is_selected && self.focused {
                    Style::default().bg(dm.theme.selection_bg).fg(dm.theme.selection_fg)
                } else {
                    Style::default()
                };

                ListItem::new(Line::from(line)).style(style)
            })
            .collect();

        let list = List::new(items).block(BorderPresets::display(
            self.focused,
            self.title(dm),
            dm.theme.focused_border,
            dm.theme.unfocused_border,
        ));

        frame.render_widget(list, area);
        self.render_status_and_help(frame, area, dm);
    }

    /// Render calldata display
    fn render_calldata(&mut self, frame: &mut Frame<'_>, area: Rect, dm: &mut DataManager) {
        if self.calldata.is_empty() {
            let paragraph = Paragraph::new("No calldata").block(BorderPresets::display(
                self.focused,
                self.title(dm),
                dm.theme.focused_border,
                dm.theme.unfocused_border,
            ));
            frame.render_widget(paragraph, area);
            return;
        }

        // Display calldata in 32-byte chunks
        let mut items = Vec::new();
        let calldata_bytes = &self.calldata[..];

        for (chunk_idx, chunk) in calldata_bytes.chunks(32).enumerate() {
            let offset = chunk_idx * 32;
            let offset_str = format!("0x{:04x}:", offset);

            // Format bytes (hex only for calldata, no ASCII decoding)
            let byte_spans = format_bytes_hex_only(chunk, &[]);

            let mut line = vec![Span::raw(offset_str), Span::raw(" ")];
            line.extend(byte_spans);

            items.push(ListItem::new(Line::from(line)));
        }

        // Apply scrolling and selection
        let visible_items: Vec<ListItem<'_>> = items
            .into_iter()
            .enumerate()
            .skip(self.scroll_offset)
            .take(self.context_height)
            .map(|(display_idx, item)| {
                let is_selected = display_idx == self.selected_index;
                let style = if is_selected && self.focused {
                    Style::default().bg(dm.theme.selection_bg).fg(dm.theme.selection_fg)
                } else {
                    Style::default()
                };
                item.style(style)
            })
            .collect();

        let list = List::new(visible_items).block(BorderPresets::display(
            self.focused,
            self.title(dm),
            dm.theme.focused_border,
            dm.theme.unfocused_border,
        ));

        frame.render_widget(list, area);
        self.render_status_and_help(frame, area, dm);
    }

    /// Render storage display
    fn render_storage(&mut self, frame: &mut Frame<'_>, area: Rect, dm: &mut DataManager) {
        if self.storage_changes.is_empty() {
            let paragraph = Paragraph::new("No storage changes").block(BorderPresets::display(
                self.focused,
                self.title(dm),
                dm.theme.focused_border,
                dm.theme.unfocused_border,
            ));
            frame.render_widget(paragraph, area);
            return;
        }

        // Convert storage changes to display items (3 lines per change)
        let mut display_items = Vec::new();
        let mut sorted_changes: Vec<_> = self.storage_changes.iter().collect();
        sorted_changes.sort_by_key(|(slot, _)| **slot);

        for (slot, (old_value, new_value)) in sorted_changes {
            // Line 1: Slot address
            display_items.push(format!("Slot: {:#066x}", slot));
            // Line 2: Old value
            display_items.push(format!("  From: {:#066x}", old_value));
            // Line 3: New value
            display_items.push(format!("  To:   {:#066x}", new_value));
        }

        // Create list items with proper scrolling and selection
        let items: Vec<ListItem<'_>> = display_items
            .iter()
            .enumerate()
            .skip(self.scroll_offset)
            .take(self.context_height)
            .map(|(display_idx, line)| {
                let is_selected = display_idx == self.selected_index;

                // Different styling for slot vs value lines
                let style = if display_idx % 3 == 0 {
                    // Slot line - use accent color
                    if is_selected && self.focused {
                        Style::default()
                            .bg(dm.theme.selection_bg)
                            .fg(dm.theme.selection_fg)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(dm.theme.accent_color).add_modifier(Modifier::BOLD)
                    }
                } else if is_selected && self.focused {
                    Style::default().bg(dm.theme.selection_bg).fg(dm.theme.selection_fg)
                } else if display_idx % 3 == 2 {
                    // "To" line - highlight new values
                    Style::default().fg(dm.theme.success_color)
                } else {
                    // "From" line - dim old values
                    Style::default().fg(dm.theme.comment_color)
                };

                ListItem::new(line.clone()).style(style)
            })
            .collect();

        let list = List::new(items).block(BorderPresets::display(
            self.focused,
            self.title(dm),
            dm.theme.focused_border,
            dm.theme.unfocused_border,
        ));

        frame.render_widget(list, area);
        self.render_status_and_help(frame, area, dm);
    }

    /// Render transient storage display
    fn render_transient_storage(
        &mut self,
        frame: &mut Frame<'_>,
        area: Rect,
        dm: &mut DataManager,
    ) {
        if self.transient_storage.is_empty() {
            let paragraph =
                Paragraph::new("No transient storage data").block(BorderPresets::display(
                    self.focused,
                    self.title(dm),
                    dm.theme.focused_border,
                    dm.theme.unfocused_border,
                ));
            frame.render_widget(paragraph, area);
            return;
        }

        // Convert to sorted list for consistent display
        let mut tstorage_items: Vec<_> = self.transient_storage.iter().collect();
        tstorage_items.sort_by_key(|(slot, _)| *slot);

        let items: Vec<ListItem<'_>> = tstorage_items
            .iter()
            .enumerate()
            .skip(self.scroll_offset)
            .take(self.context_height)
            .map(|(display_idx, (slot, value))| {
                let is_selected = display_idx == self.selected_index;

                // Only show slot since all entries are for current address
                let content = format!("Slot: {:#066x} = {:#066x}", slot, value);

                let style = if is_selected && self.focused {
                    Style::default().bg(dm.theme.selection_bg).fg(dm.theme.selection_fg)
                } else {
                    Style::default()
                };

                ListItem::new(content).style(style)
            })
            .collect();

        let list = List::new(items).block(BorderPresets::display(
            self.focused,
            self.title(dm),
            dm.theme.focused_border,
            dm.theme.unfocused_border,
        ));

        frame.render_widget(list, area);
        self.render_status_and_help(frame, area, dm);
    }

    /// Render variables display (for hooked snapshots)
    fn render_variables(&mut self, frame: &mut Frame<'_>, area: Rect, dm: &mut DataManager) {
        let items: Vec<ListItem<'_>> = self
            .variables
            .iter()
            .enumerate()
            .skip(self.scroll_offset)
            .take(self.context_height)
            .map(|(display_idx, var)| {
                let is_selected = display_idx == self.selected_index;
                let style = if is_selected && self.focused {
                    Style::default().bg(dm.theme.selection_bg).fg(dm.theme.selection_fg)
                } else {
                    Style::default()
                };
                ListItem::new(var.clone()).style(style)
            })
            .collect();

        let list = List::new(items).block(BorderPresets::display(
            self.focused,
            self.title(dm),
            dm.theme.focused_border,
            dm.theme.unfocused_border,
        ));

        frame.render_widget(list, area);
        self.render_status_and_help(frame, area, dm);
    }

    /// Render breakpoints display (for hooked snapshots)
    fn render_breakpoints(&mut self, frame: &mut Frame<'_>, area: Rect, dm: &mut DataManager) {
        let items: Vec<ListItem<'_>> = self
            .breakpoints
            .iter()
            .enumerate()
            .skip(self.scroll_offset)
            .take(self.context_height)
            .map(|(display_idx, bp)| {
                let is_selected = display_idx == self.selected_index;
                let style = if is_selected && self.focused {
                    Style::default().bg(dm.theme.selection_bg).fg(dm.theme.selection_fg)
                } else {
                    Style::default()
                };
                ListItem::new(bp.clone()).style(style)
            })
            .collect();

        let list = List::new(items).block(BorderPresets::display(
            self.focused,
            self.title(dm),
            dm.theme.focused_border,
            dm.theme.unfocused_border,
        ));

        frame.render_widget(list, area);
        self.render_status_and_help(frame, area, dm);
    }

    /// Render status and help text
    fn render_status_and_help(&self, frame: &mut Frame<'_>, area: Rect, dm: &DataManager) {
        if !self.focused || area.height <= 10 {
            return;
        }

        // Status line
        let status_area =
            Rect { x: area.x + 1, y: area.y + area.height - 3, width: area.width - 2, height: 1 };

        let item_count = match self.mode {
            DisplayMode::Stack => self.stack_items.len(),
            DisplayMode::Memory => self.memory_chunks.len(),
            DisplayMode::CallData => (self.calldata.len() + 31) / 32,
            DisplayMode::Storage => self.storage_changes.len() * 3,
            DisplayMode::TransientStorage => self.transient_storage.len(),
            DisplayMode::Variables => self.variables.len(),
            DisplayMode::Breakpoints => self.breakpoints.len(),
        };

        let status_bar = StatusBar::new()
            .current_panel("Display".to_string())
            .message(format!("Mode: {}", self.mode.name()))
            .message(format!("Items: {}", item_count));

        let status_text = status_bar.build();
        let status_paragraph =
            Paragraph::new(status_text).style(Style::default().fg(dm.theme.accent_color));
        frame.render_widget(status_paragraph, status_area);

        // Help line
        let help_area =
            Rect { x: area.x + 1, y: area.y + area.height - 2, width: area.width - 2, height: 1 };

        let help_text = "↑/↓: Navigate • s/S: Switch mode";
        let help_paragraph =
            Paragraph::new(help_text).style(Style::default().fg(dm.theme.help_text_color));
        frame.render_widget(help_paragraph, help_area);
    }
}

impl PanelTr for DisplayPanel {
    fn panel_type(&self) -> PanelType {
        PanelType::Display
    }

    fn title(&self, _dm: &mut DataManager) -> String {
        let item_count = match self.mode {
            DisplayMode::Stack => self.stack_items.len(),
            DisplayMode::Memory => self.memory_chunks.len(),
            DisplayMode::CallData => {
                if self.calldata.is_empty() {
                    0
                } else {
                    (self.calldata.len() + 31) / 32
                }
            }
            DisplayMode::Storage => self.storage_changes.len() * 3,
            DisplayMode::TransientStorage => self.transient_storage.len(),
            DisplayMode::Variables => self.variables.len(),
            DisplayMode::Breakpoints => self.breakpoints.len(),
        };

        let snapshot_type = if self.is_opcode_snapshot { "Opcode" } else { "Hook" };
        format!("{} - {} ({} items)", snapshot_type, self.mode.name(), item_count)
    }

    fn render(&mut self, frame: &mut Frame<'_>, area: Rect, dm: &mut DataManager) {
        // Calculate context height and width for viewport calculations
        self.context_height = if self.focused && area.height > 10 {
            area.height.saturating_sub(4) // Account for borders and status lines
        } else {
            area.height.saturating_sub(2) // Just borders
        } as usize;
        self.content_width = area.width.saturating_sub(2) as usize; // Account for borders

        // Update snapshot data if needed
        let _ = self.update_snapshot_data(dm);

        // Calculate max line width for horizontal scrolling
        self.calculate_max_line_width();

        // Render based on current mode
        match self.mode {
            DisplayMode::Stack => self.render_stack(frame, area, dm),
            DisplayMode::Memory => self.render_memory(frame, area, dm),
            DisplayMode::CallData => self.render_calldata(frame, area, dm),
            DisplayMode::Storage => self.render_storage(frame, area, dm),
            DisplayMode::TransientStorage => self.render_transient_storage(frame, area, dm),
            DisplayMode::Variables => self.render_variables(frame, area, dm),
            DisplayMode::Breakpoints => self.render_breakpoints(frame, area, dm),
        }
    }

    fn handle_key_event(
        &mut self,
        event: KeyEvent,
        _dm: &mut DataManager,
    ) -> Result<EventResponse> {
        if !self.focused || event.kind != KeyEventKind::Press {
            return Ok(EventResponse::NotHandled);
        }

        match event.code {
            KeyCode::Char('s') => {
                self.next_mode();
                Ok(EventResponse::Handled)
            }
            KeyCode::Char('S') => {
                self.prev_mode();
                Ok(EventResponse::Handled)
            }
            KeyCode::Up => {
                self.move_up();
                Ok(EventResponse::Handled)
            }
            KeyCode::Down => {
                self.move_down();
                Ok(EventResponse::Handled)
            }
            KeyCode::Left => {
                // Scroll left
                if self.horizontal_offset > 0 {
                    self.horizontal_offset = self.horizontal_offset.saturating_sub(5);
                    debug!("Scrolled left to offset {}", self.horizontal_offset);
                }
                Ok(EventResponse::Handled)
            }
            KeyCode::Right => {
                // Scroll right
                if self.max_line_width > self.content_width {
                    let max_scroll = self.max_line_width.saturating_sub(self.content_width);
                    if self.horizontal_offset < max_scroll {
                        self.horizontal_offset = (self.horizontal_offset + 5).min(max_scroll);
                        debug!("Scrolled right to offset {}", self.horizontal_offset);
                    }
                }
                Ok(EventResponse::Handled)
            }
            KeyCode::PageUp => {
                for _ in 0..5 {
                    self.move_up();
                }
                Ok(EventResponse::Handled)
            }
            KeyCode::PageDown => {
                for _ in 0..5 {
                    self.move_down();
                }
                Ok(EventResponse::Handled)
            }
            KeyCode::Home => {
                self.selected_index = 0;
                Ok(EventResponse::Handled)
            }
            KeyCode::End => {
                let max_items = match self.mode {
                    DisplayMode::Stack => self.stack_items.len(),
                    DisplayMode::Memory => self.memory_chunks.len(),
                    DisplayMode::CallData => {
                        if self.calldata.is_empty() {
                            0
                        } else {
                            (self.calldata.len() + 31) / 32
                        }
                    }
                    DisplayMode::Storage => self.storage_changes.len() * 3,
                    DisplayMode::TransientStorage => self.transient_storage.len(),
                    DisplayMode::Variables => self.variables.len(),
                    DisplayMode::Breakpoints => self.breakpoints.len(),
                };
                self.selected_index = max_items.saturating_sub(1);
                Ok(EventResponse::Handled)
            }
            _ => Ok(EventResponse::NotHandled),
        }
    }

    fn on_focus(&mut self) {
        self.focused = true;
        debug!("Display panel gained focus");
    }

    fn on_blur(&mut self) {
        self.focused = false;
        debug!("Display panel lost focus");
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// Helper functions
/// Format bytes as hex with ASCII decode (like xxd)
fn format_bytes_with_decode<'a>(bytes: &'a [u8], highlight_indices: &'a [usize]) -> Vec<Span<'a>> {
    let mut spans = Vec::new();

    // Hex part
    for (i, byte) in bytes.iter().enumerate() {
        let hex = format!("{:02x}", byte);
        if highlight_indices.contains(&i) {
            spans.push(Span::styled(
                hex,
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::raw(hex));
        }
        if i < bytes.len() - 1 {
            spans.push(Span::raw(" "));
        }
    }

    // ASCII part
    spans.push(Span::raw("  "));
    for (i, byte) in bytes.iter().enumerate() {
        let ch = if byte.is_ascii_graphic() || *byte == b' ' {
            *byte as char
        } else {
            '.' // Show all non-printable bytes (including 0) as dots
        };

        if highlight_indices.contains(&i) {
            spans.push(Span::styled(ch.to_string(), Style::default().fg(Color::Yellow)));
        } else {
            spans.push(Span::raw(ch.to_string()));
        }
    }

    spans
}

/// Format bytes as hex only (no ASCII decode) - for calldata
fn format_bytes_hex_only<'a>(bytes: &'a [u8], highlight_indices: &'a [usize]) -> Vec<Span<'a>> {
    let mut spans = Vec::new();

    // Hex part only
    for (i, byte) in bytes.iter().enumerate() {
        let hex = format!("{:02x}", byte);
        if highlight_indices.contains(&i) {
            spans.push(Span::styled(
                hex,
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::raw(hex));
        }
        if i < bytes.len() - 1 {
            spans.push(Span::raw(" "));
        }
    }

    spans
}
