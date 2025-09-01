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

//! Code panel for displaying source code or opcodes
//!
//! This panel shows source code with syntax highlighting and current line indication.

use super::{EventResponse, PanelTr, PanelType};
use crate::data::DataManager;
use crate::ui::borders::BorderPresets;
use crate::ui::status::{FileStatus, StatusBar};
use crate::ui::syntax::{SyntaxHighlighter, SyntaxType};
use alloy_primitives::Address;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use edb_common::types::{Code, SnapshotInfo};
use eyre::Result;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    widgets::{Block, Borders, List, ListItem},
    Frame,
};
use std::collections::HashMap;
use tracing::{debug, info};

/// Code display mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeMode {
    /// Show source code
    Source,
    /// Show opcodes
    Opcodes,
}

/// File information with metadata
#[derive(Debug, Clone)]
pub struct FileInfo {
    /// File path
    pub path: String,
    /// Number of lines in the file
    pub line_count: usize,
    /// Whether this file contains current execution
    pub has_execution: bool,
}

/// Server-controlled display preferences
#[derive(Debug, Clone)]
pub struct CodeDisplayInfo {
    /// Current displayed bytecode address
    pub bytecode_address: Address,
    /// Whether source code is available from server
    pub has_source_code: bool,
    /// Server's preferred display mode
    pub mode: CodeMode,
    /// List of available source files for this address
    pub available_files: Vec<String>,
    /// Enhanced file information with metadata
    pub file_info: Vec<FileInfo>,
}

/// Code panel implementation (stub)
#[derive(Debug)]
pub struct CodePanel {
    // ========== Display ==========
    /// Current execution line (server-controlled, 1-based)
    current_execution_line: Option<usize>,
    /// User cursor line (user-controlled for breakpoints, 1-based)
    user_cursor_line: Option<usize>,
    /// Scroll offset
    scroll_offset: usize,
    /// Whether this panel is focused
    focused: bool,
    /// File selector state
    show_file_selector: bool,
    /// Selected file index in file selector
    file_selector_index: usize,
    /// File selector scroll offset for auto-scrolling
    file_selector_scroll_offset: usize,
    /// File selector viewport height
    file_selector_context_height: usize,
    /// Height percentage for file selector (0-100)
    file_selector_height_percent: u16,
    /// Content height for the current panel
    context_height: usize,
    /// Content width for the current panel
    content_width: usize,
    /// Horizontal scroll offset
    horizontal_offset: usize,
    /// Maximum line width (including line numbers and indicators)
    max_line_width: usize,
    /// Syntax highlighter for code
    syntax_highlighter: SyntaxHighlighter,
    /// VIM mode number prefix for repetition (e.g., "10" in "10j")
    vim_number_prefix: String,
    /// VIM command buffer for commands like ":n"
    vim_command_buffer: String,
    /// Whether we're in VIM command mode (after pressing :)
    vim_command_mode: bool,

    // ========== Data (Flag) ==========
    /// Current execution snapshot
    current_execution_snapshot: Option<usize>,
    /// Current display snapshot id
    current_display_snapshot: Option<usize>,
    /// Current selected path
    current_selected_path_id: Option<usize>,

    // ========== Data ==========
    /// Server-provided display information
    display_info: CodeDisplayInfo,
    /// Mock source code lines
    source_lines: Vec<String>,
    /// Mock opcode lines
    opcode_lines: Vec<String>,
    /// Available source code
    sources: HashMap<String, String>,
    /// Avaiable opcode
    opcodes: Vec<(u64, String)>,
    /// Currently selected source path index
    selected_path_index: usize,
}

impl CodePanel {
    /// Create a new code panel
    pub fn new() -> Self {
        let display_info = CodeDisplayInfo {
            bytecode_address: Address::default(),
            has_source_code: true, // This address has source code
            mode: CodeMode::Source,
            available_files: vec![],
            file_info: vec![],
        };

        Self {
            display_info,
            current_execution_snapshot: None,
            current_display_snapshot: None,
            current_selected_path_id: None,
            source_lines: vec![],
            opcode_lines: vec![],
            sources: HashMap::new(),
            opcodes: Vec::new(),
            selected_path_index: 0,
            current_execution_line: Some(1),
            user_cursor_line: Some(1),
            scroll_offset: 0,
            focused: false,
            show_file_selector: false,
            file_selector_index: 0,
            file_selector_scroll_offset: 0,
            file_selector_context_height: 0,
            file_selector_height_percent: 20,
            context_height: 0,
            content_width: 0,
            horizontal_offset: 0,
            max_line_width: 0,
            syntax_highlighter: SyntaxHighlighter::new(),
            vim_number_prefix: String::new(),
            vim_command_buffer: String::new(),
            vim_command_mode: false,
        }
    }

    /// Get current lines to display
    fn get_display_lines(&self) -> &Vec<String> {
        match self.display_info.mode {
            CodeMode::Source => &self.source_lines,
            CodeMode::Opcodes => &self.opcode_lines,
        }
    }

    /// Calculate the maximum line width including line numbers and indicators
    fn calculate_max_line_width(&mut self) {
        let lines = self.get_display_lines();
        if lines.is_empty() {
            self.max_line_width = 0;
            return;
        }

        let max_line_num = lines.len();
        let line_num_width = max_line_num.to_string().len().max(3);
        // Line number + space + breakpoint + cursor + " │ " separator
        let prefix_width = line_num_width + 1 + 1 + 1 + 3;

        self.max_line_width = lines.iter().map(|line| prefix_width + line.len()).max().unwrap_or(0);
    }

    /// Apply syntax highlighting to a line and return styled text
    fn highlight_line<'a>(
        &self,
        line: &'a str,
        line_num: usize,
        max_line_num: usize,
        dm: &mut DataManager,
    ) -> ratatui::text::Line<'a> {
        use ratatui::text::{Line, Span};

        // Determine syntax type based on display mode
        let syntax_type = match self.display_info.mode {
            CodeMode::Source => SyntaxType::Solidity,
            CodeMode::Opcodes => SyntaxType::Opcodes,
        };

        // Get line number width (for consistent alignment)
        let line_num_width = max_line_num.to_string().len().max(3);

        // Create line number with background
        let line_num_text = format!("{:>width$} ", line_num, width = line_num_width);
        let line_num_span = Span::styled(
            line_num_text,
            Style::default().fg(dm.theme.line_number).bg(dm.theme.line_number_bg),
        );

        // Tokenize the line for syntax highlighting
        let tokens = self.syntax_highlighter.tokenize(line, syntax_type);

        let mut spans = vec![line_num_span];
        let mut last_end = 0;

        // Apply syntax highlighting to tokens
        for token in tokens {
            // Add any unhighlighted text before this token
            if token.start > last_end {
                let unhighlighted = &line[last_end..token.start];
                if !unhighlighted.is_empty() {
                    spans.push(Span::raw(unhighlighted));
                }
            }

            // Add the highlighted token
            let token_text = &line[token.start..token.end];
            let token_style = self.syntax_highlighter.get_token_style(token.token_type, &dm.theme);
            spans.push(Span::styled(token_text, token_style));

            last_end = token.end;
        }

        // Add any remaining unhighlighted text
        if last_end < line.len() {
            let remaining = &line[last_end..];
            if !remaining.is_empty() {
                spans.push(Span::raw(remaining));
            }
        }

        Line::from(spans)
    }

    /// Toggle file selector visibility
    fn toggle_file_selector(&mut self) {
        if self.display_info.mode != CodeMode::Source || !self.display_info.has_source_code {
            // File selector only available in source mode with source code
            return;
        }

        self.show_file_selector = !self.show_file_selector;
        if self.show_file_selector {
            // Reset selection to current file when opening
            self.file_selector_index = self.selected_path_index;
        }
        debug!("File selector toggled: {}", self.show_file_selector);
    }

    /// Move file selector up with auto-scrolling
    fn file_selector_up(&mut self) {
        if self.file_selector_index > 0 {
            self.file_selector_index -= 1;
            // Auto-scroll up if selection moves above visible area
            if self.file_selector_index < self.file_selector_scroll_offset {
                self.file_selector_scroll_offset = self.file_selector_index;
            }
        }
    }

    /// Move file selector down with auto-scrolling
    fn file_selector_down(&mut self) {
        let max_index = self.display_info.file_info.len().saturating_sub(1);
        if self.file_selector_index < max_index {
            self.file_selector_index += 1;
            let viewport_height = self.file_selector_context_height;
            // Auto-scroll down if selection moves below visible area
            if self.file_selector_index >= self.file_selector_scroll_offset + viewport_height {
                self.file_selector_scroll_offset =
                    (self.file_selector_index + 1).saturating_sub(viewport_height);
            }
        }
    }

    /// Select file from file selector
    fn select_file_from_selector(&mut self) {
        if self.file_selector_index < self.display_info.file_info.len() {
            self.selected_path_index = self.file_selector_index;
            self.show_file_selector = false;

            // Update source_paths index to match
            let selected_file = &self.display_info.file_info[self.file_selector_index];
            debug!("Selected file: {}", selected_file.path);
        }
    }

    /// Get sorted file list for display (execution files first, then alphabetical)
    fn get_sorted_files(&self) -> Vec<(usize, &FileInfo)> {
        let mut files: Vec<(usize, &FileInfo)> =
            self.display_info.file_info.iter().enumerate().collect();

        files.sort_by(|(_, a), (_, b)| {
            // First, sort by execution (files with execution come first)
            match (a.has_execution, b.has_execution) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => {
                    // Then sort alphabetically by filename
                    let a_name = a.path.split('/').last().unwrap_or(&a.path);
                    let b_name = b.path.split('/').last().unwrap_or(&b.path);
                    a_name.cmp(b_name)
                }
            }
        });

        files
    }

    /// Render the file selector panel
    fn render_file_selector(&mut self, frame: &mut Frame<'_>, area: Rect, dm: &mut DataManager) {
        // Calculate file selector context height for viewport calculations
        self.file_selector_context_height = area.height.saturating_sub(2) as usize; // Account for borders

        let sorted_files = self.get_sorted_files();

        let items: Vec<ListItem<'_>> = sorted_files
            .iter()
            .enumerate()
            .skip(self.file_selector_scroll_offset) // Skip items before viewport
            .take(self.file_selector_context_height) // Take only visible items
            .map(|(display_idx, (_, file_info))| {
                let filename = file_info.path.split('/').last().unwrap_or(&file_info.path);

                // Determine file status for enhanced icon display
                let file_status = if file_info.has_execution {
                    FileStatus::HasExecution
                } else {
                    FileStatus::SourceAvailable
                };

                let content =
                    format!("{} ({} lines)", file_status.display(filename), file_info.line_count);

                let style = if display_idx == self.file_selector_index {
                    Style::default().bg(dm.theme.selection_bg).fg(dm.theme.selection_fg)
                } else if file_info.has_execution {
                    // Highlight files with current execution
                    Style::default().fg(dm.theme.warning_color)
                } else {
                    Style::default()
                };

                ListItem::new(content).style(style)
            })
            .collect();

        let file_list = List::new(items)
            .block(
                Block::default()
                    .title(format!(
                        "📁 Files ({}/{})",
                        self.file_selector_index + 1,
                        sorted_files.len()
                    ))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(dm.theme.success_color)),
            )
            .highlight_style(Style::default().bg(dm.theme.selection_bg));

        frame.render_widget(file_list, area);
    }

    /// Apply horizontal offset to a line for horizontal scrolling
    fn apply_horizontal_offset<'a>(
        &self,
        line: ratatui::text::Line<'a>,
    ) -> ratatui::text::Line<'a> {
        use ratatui::text::{Line, Span};

        if self.horizontal_offset == 0 {
            return line;
        }

        // Calculate the visual width of each span
        let mut accumulated_width = 0;
        let mut new_spans = Vec::new();
        let mut started_content = false;

        for span in line.spans {
            let span_width = span.content.chars().count();

            if accumulated_width + span_width <= self.horizontal_offset {
                // This span is completely before the viewport
                accumulated_width += span_width;
            } else if accumulated_width >= self.horizontal_offset {
                // This span is completely within the viewport
                new_spans.push(span);
                started_content = true;
            } else {
                // This span is partially visible - need to trim the beginning
                let skip_chars = self.horizontal_offset - accumulated_width;
                let visible_content: String = span.content.chars().skip(skip_chars).collect();
                if !visible_content.is_empty() {
                    new_spans.push(Span::styled(visible_content, span.style));
                    started_content = true;
                }
                accumulated_width += span_width;
            }
        }

        // If we've scrolled past all content, show empty line
        if !started_content {
            new_spans.push(Span::raw(""));
        }

        Line::from(new_spans)
    }

    /// Render the main code content with syntax highlighting
    fn render_code_content(&mut self, frame: &mut Frame<'_>, area: Rect, dm: &mut DataManager) {
        use ratatui::text::{Line, Span};
        use ratatui::widgets::{List, ListItem, Paragraph};

        // Calculate viewport height and width
        self.context_height = if self.focused && area.height > 10 {
            area.height.saturating_sub(4) // Account for borders and status lines
        } else {
            area.height.saturating_sub(2) // Just borders
        } as usize;
        self.content_width = area.width.saturating_sub(2) as usize; // Account for borders

        let lines = self.get_display_lines();

        if lines.is_empty() {
            let paragraph = Paragraph::new("No code available").block(BorderPresets::code(
                self.focused,
                self.title(dm),
                dm.theme.focused_border,
                dm.theme.unfocused_border,
            ));
            frame.render_widget(paragraph, area);
            return;
        }

        // Get the display lines with viewport scrolling
        let display_lines: Vec<_> =
            lines.iter().enumerate().skip(self.scroll_offset).take(self.context_height).collect();

        let max_line_num = lines.len();

        // Create list items with syntax highlighting, line numbers, and indicators
        let list_items: Vec<ListItem<'_>> = display_lines
            .iter()
            .map(|(line_idx, line)| {
                let line_num = line_idx + 1;
                let is_execution =
                    self.current_execution_line.map_or(false, |exec| exec == line_num);
                let is_user_cursor = self.user_cursor_line.map_or(false, |user| user == line_num);
                let has_breakpoint = false; // TODO

                // Start with syntax-highlighted line
                let highlighted_line = self.highlight_line(line, line_num, max_line_num, dm);

                // Add cursor and breakpoint indicators
                let cursor_indicator = if is_execution && is_user_cursor {
                    "◉" // Both cursors on same line
                } else if is_execution {
                    "►" // Execution cursor
                } else if is_user_cursor {
                    "◯" // User cursor
                } else {
                    " " // No cursor
                };

                let breakpoint_indicator = if has_breakpoint {
                    Span::styled("●", Style::default().fg(dm.theme.error_color))
                } else {
                    Span::raw(" ")
                };

                // Insert breakpoint and cursor indicators after line number
                let mut new_spans = vec![highlighted_line.spans[0].clone()]; // Line number
                new_spans.push(breakpoint_indicator);
                new_spans.push(Span::raw(format!("{} │ ", cursor_indicator)));

                // Add the syntax highlighted content (skip the line number span)
                if highlighted_line.spans.len() > 1 {
                    new_spans.extend_from_slice(&highlighted_line.spans[1..]);
                }

                let mut content_line = Line::from(new_spans);

                // Apply horizontal scrolling offset
                content_line = self.apply_horizontal_offset(content_line);

                // Apply background highlighting for execution/cursor lines
                let item_style = if is_execution {
                    Style::default().bg(dm.theme.current_line_bg)
                } else if is_user_cursor {
                    Style::default().bg(dm.theme.highlight_bg)
                } else {
                    Style::default()
                };

                ListItem::new(content_line).style(item_style)
            })
            .collect();

        // Create list with highlighted content
        let code_list = List::new(list_items).block(BorderPresets::code(
            self.focused,
            self.title(dm),
            dm.theme.focused_border,
            dm.theme.unfocused_border,
        ));

        frame.render_widget(code_list, area);

        // Add cursor status and help text at the bottom if focused
        if self.focused && area.height > 10 {
            // Status line
            let status_area = Rect {
                x: area.x + 1,
                y: area.y + area.height - 3,
                width: area.width - 2,
                height: 1,
            };

            // Build comprehensive status using StatusBar
            let exec_line =
                self.current_execution_line.map_or("None".to_string(), |l| l.to_string());
            let user_line = self.user_cursor_line.map_or("None".to_string(), |l| l.to_string());

            let mut status_bar = StatusBar::new()
                .current_panel("Code".to_string())
                .message(format!("► Exec: {} / {}", exec_line, lines.len()))
                .message(format!("◯ User: {} / {}", user_line, lines.len()));

            if self.display_info.has_source_code {
                status_bar = status_bar.message(format!(
                    "Files: {}/{}",
                    self.selected_path_index + 1,
                    self.display_info.file_info.len()
                ));
            }

            let status_text = status_bar.build();

            // Add horizontal scroll indicator if content is scrollable
            let final_status_text = if self.max_line_width > self.content_width {
                let scrollable_width = self.max_line_width.saturating_sub(self.content_width);
                let scroll_percentage = if scrollable_width > 0 {
                    (self.horizontal_offset as f32 / scrollable_width as f32).min(1.0)
                } else {
                    0.0
                };

                // Create scroll indicator with nice UTF-8 characters
                // Using box drawing characters for a clean look
                let indicator_width = 15; // Width of the scroll indicator
                let thumb_position = (scroll_percentage * (indicator_width - 3) as f32) as usize;

                let mut indicator = String::from(" [");
                for i in 0..indicator_width {
                    if i >= thumb_position && i < thumb_position + 3 {
                        indicator.push('█'); // Solid block for thumb
                    } else {
                        indicator.push('─'); // Horizontal line for track
                    }
                }
                indicator.push(']');

                format!("{}{}", status_text, indicator)
            } else {
                status_text
            };

            let status_paragraph =
                Paragraph::new(final_status_text).style(Style::default().fg(dm.theme.accent_color));
            frame.render_widget(status_paragraph, status_area);

            // Help line - updated to include file selector
            let help_area = Rect {
                x: area.x + 1,
                y: area.y + area.height - 2,
                width: area.width - 2,
                height: 1,
            };
            let help_text = if self.vim_command_mode {
                // Show VIM command mode prompt
                format!(":{}", self.vim_command_buffer)
            } else if self.show_file_selector {
                "↑/↓: Navigate • Enter: Select • F: Close".to_string()
            } else {
                let mut help = String::from("↑/↓: Navigate");
                if self.max_line_width > self.content_width {
                    help.push_str(" • ←/→: Scroll");
                }
                help.push_str(" • s/r/n/p: Step/Rev/Next/Prev • c/C: Next/Prev call");
                if self.display_info.mode == CodeMode::Source {
                    help.push_str(" • F: Files");
                }
                help.push_str(" • B: Breakpoint");
                help
            };

            let help_style = if self.vim_command_mode {
                // Use a different style for VIM command mode to make it more prominent
                Style::default().fg(dm.theme.help_text_color).bg(dm.theme.highlight_bg)
            } else {
                Style::default().fg(dm.theme.help_text_color)
            };

            let help_paragraph = Paragraph::new(help_text).style(help_style);
            frame.render_widget(help_paragraph, help_area);
        }
    }

    /// Get repetition count from vim_number_prefix, defaulting to 1
    fn get_vim_repetition(&self) -> usize {
        if self.vim_number_prefix.is_empty() {
            1
        } else {
            self.vim_number_prefix.parse().unwrap_or(1).max(1)
        }
    }

    /// Clear vim state after executing a command
    fn clear_vim_state(&mut self) {
        self.vim_number_prefix.clear();
    }

    /// Move cursor up by specified number of lines (VIM j command)
    fn move_up(&mut self, count: usize) {
        if let Some(current_line) = self.user_cursor_line {
            let new_line = current_line.saturating_sub(count).max(1);
            self.move_to(new_line);
        }
    }

    /// Move cursor down by specified number of lines (VIM k command)
    fn move_down(&mut self, count: usize) {
        let max_lines = self.get_display_lines().len();
        if let Some(current_line) = self.user_cursor_line {
            let new_line = (current_line + count).min(max_lines);
            self.move_to(new_line);
        }
    }

    /// Move to first line (VIM gg command)
    fn goto_top(&mut self) {
        self.move_to(1);
    }

    /// Move to last line (VIM G command)
    fn goto_bottom(&mut self) {
        let max_lines = self.get_display_lines().len();
        self.move_to(max_lines);
    }

    /// Find next blank line (VIM } command)
    fn vim_next_blank_line(&mut self, count: usize) {
        let lines = self.get_display_lines();
        if let Some(current_line) = self.user_cursor_line {
            let mut target_line = lines.len(); // Default as Max
            let mut blank_lines_found = 0;

            for line_num in (current_line + 1)..=lines.len() {
                if line_num <= lines.len() && lines[line_num - 1].trim().is_empty() {
                    blank_lines_found += 1;
                    if blank_lines_found == count {
                        target_line = line_num;
                        break;
                    }
                }
            }

            self.move_to(target_line);
        }
    }

    /// Find previous blank line (VIM { command)
    fn vim_prev_blank_line(&mut self, count: usize) {
        let lines = self.get_display_lines();
        if let Some(current_line) = self.user_cursor_line {
            let mut target_line = 1; // Default as 1
            let mut blank_lines_found = 0;

            for line_num in (1..current_line).rev() {
                if lines[line_num - 1].trim().is_empty() {
                    blank_lines_found += 1;
                    if blank_lines_found == count {
                        target_line = line_num;
                        break;
                    }
                }
            }

            self.move_to(target_line);
        }
    }

    /// Execute VIM command from command buffer
    fn execute_vim_command(&mut self) {
        let command = self.vim_command_buffer.trim();
        if let Ok(line_number) = command.parse::<usize>() {
            self.move_to(line_number);
        }
        // Clear command mode
        self.vim_command_buffer.clear();
        self.vim_command_mode = false;
    }

    fn move_to(&mut self, line: usize) {
        let max_line = self.get_display_lines().len();
        let line = line.max(1).min(max_line);

        let viewport_height = self.context_height;
        let half_viewport = viewport_height / 2;

        if line <= half_viewport || max_line <= viewport_height {
            self.scroll_offset = 0;
            self.user_cursor_line = Some(line);
        } else if line > max_line - viewport_height {
            self.scroll_offset = max_line.saturating_sub(viewport_height);
            self.user_cursor_line = Some(line);
        } else {
            self.scroll_offset = line.saturating_sub(half_viewport);
            self.user_cursor_line = Some(line);
        }
    }

    fn update_display_info(&mut self, dm: &mut DataManager) -> Option<()> {
        let id = dm.execution.get_display_snapshot();
        if self.current_display_snapshot == Some(id) {
            return Some(()); // No change
        }

        let exec_source_offset = dm.execution.get_snapshot_info(id)?.offset();
        let exec_opcode_pc = dm.execution.get_snapshot_info(id)?.pc();
        let execution_path = dm.execution.get_snapshot_info(id)?.path().cloned();

        let code = dm.execution.get_code(id)?;
        let bytecode_address = code.bytecode_address();

        // We reset the selected path id here, and only update it as Some when we
        // are about to show new code.
        self.current_selected_path_id = None;
        match code {
            Code::Source(info) => {
                if self.display_info.bytecode_address != bytecode_address {
                    info!("Code address changed to {:?}", bytecode_address);

                    self.display_info.has_source_code = true;
                    self.display_info.mode = CodeMode::Source;
                    self.display_info.available_files = info
                        .sources
                        .keys()
                        .map(|p| p.as_os_str().to_string_lossy().to_string())
                        .collect();
                    self.display_info.file_info = info
                        .sources
                        .iter()
                        .map(|(p, s)| {
                            let path = p.as_os_str().to_string_lossy().to_string();
                            let line_count = s.lines().count();
                            let has_execution = execution_path.as_ref() == Some(p);
                            FileInfo { path, line_count, has_execution }
                        })
                        .collect();

                    self.display_info.available_files.sort();
                    self.display_info.file_info.sort_by(|a, b| a.path.cmp(&b.path));

                    self.display_info.bytecode_address = bytecode_address;

                    // update selected path index
                    self.selected_path_index = self
                        .display_info
                        .file_info
                        .iter()
                        .position(|p| p.has_execution)
                        .unwrap_or(0);
                    self.current_selected_path_id = Some(self.selected_path_index);

                    self.sources = info
                        .sources
                        .iter()
                        .map(|(p, s)| {
                            let path = p.as_os_str().to_string_lossy().to_string();
                            (path, s.clone())
                        })
                        .collect();
                    self.opcodes.clear();

                    self.source_lines = self
                        .sources
                        .get(&self.display_info.available_files[self.selected_path_index])
                        .map_or(vec![], |source| source.lines().map(|l| l.to_string()).collect());
                    self.opcode_lines.clear();

                    // Calculate max line width for horizontal scrolling
                    self.calculate_max_line_width();
                    self.horizontal_offset = 0; // Reset horizontal scroll when content changes
                }

                // move user cursor
                let execution_path = &self.display_info.available_files[self.selected_path_index];
                let execution_line = self
                    .sources
                    .get(execution_path)
                    .zip(exec_source_offset)
                    .and_then(|(s, offset)| Some(s[..offset].lines().count()))
                    .unwrap_or_default(); // 1-based
                self.move_to(execution_line);
            }
            Code::Opcode(info) => {
                if self.display_info.bytecode_address != bytecode_address {
                    info!("Code address changed to {:?}", bytecode_address);
                    self.display_info.has_source_code = false;
                    self.display_info.mode = CodeMode::Opcodes;
                    self.display_info.available_files.clear();
                    self.display_info.file_info.clear();

                    self.display_info.bytecode_address = bytecode_address;

                    self.selected_path_index = 0;

                    self.opcodes =
                        info.codes.iter().map(|(pc, insn)| (*pc, insn.clone())).collect();
                    self.opcodes.sort_by_key(|(pc, _)| *pc);
                    self.sources.clear();

                    self.opcode_lines = self
                        .opcodes
                        .iter()
                        .map(|(pc, insn)| format!("{:05}: {}", pc, insn))
                        .collect();
                    self.source_lines.clear();

                    // Calculate max line width for horizontal scrolling
                    self.calculate_max_line_width();
                    self.horizontal_offset = 0; // Reset horizontal scroll when content changes
                }

                // move user cursor
                let execution_line = self
                    .opcodes
                    .iter()
                    .position(|(pc, _)| Some(*pc as usize) == exec_opcode_pc)
                    .map(|idx| idx + 1) // 1-based
                    .unwrap_or_default();
                self.move_to(execution_line);
            }
        }

        self.current_display_snapshot = Some(id);
        Some(())
    }

    fn fresh_source_code(&mut self, _dm: &DataManager) -> Option<()> {
        if !self.display_info.has_source_code {
            return Some(());
        }

        if self.current_selected_path_id == Some(self.selected_path_index) {
            return Some(());
        }

        if self.display_info.available_files.is_empty() {
            self.source_lines.clear();
            return Some(());
        }

        let selected_file = &self.display_info.available_files[self.selected_path_index];

        if let Some(source) = self.sources.get(selected_file) {
            self.source_lines = source.lines().map(|l| l.to_string()).collect();
            // Recalculate max line width when switching files
            self.calculate_max_line_width();
        }

        if self.current_selected_path_id.is_some() {
            // This means we are still under the same address, but change to another file
            // to display.
            self.scroll_offset = 0;
            self.user_cursor_line = Some(1);
            self.horizontal_offset = 0; // Reset horizontal scroll when switching files
        }

        self.current_selected_path_id = Some(self.selected_path_index);
        Some(())
    }

    fn update_execution_info(&mut self, dm: &mut DataManager) -> Option<()> {
        if self.current_execution_snapshot != Some(dm.execution.get_current_snapshot()) {
            // We have update the execution snapshot
            let execution_snapshot_id = dm.execution.get_current_snapshot();
            let display_snapshot_id = dm.execution.get_display_snapshot();

            let execution_entry_id =
                dm.execution.get_snapshot_info(execution_snapshot_id)?.frame_id().trace_entry_id();
            let display_entry_id =
                dm.execution.get_snapshot_info(display_snapshot_id)?.frame_id().trace_entry_id();

            let execution_address = dm.execution.get_trace().get(execution_entry_id)?.code_address;
            let display_address = dm.execution.get_trace().get(display_entry_id)?.code_address;

            self.current_execution_snapshot = Some(execution_snapshot_id);

            if execution_address != display_address {
                // We do not need to show the execution line
                self.current_execution_line = None;
                return Some(());
            }

            match dm.execution.get_snapshot_info(execution_snapshot_id)? {
                SnapshotInfo::Hook(hook_info) => {
                    let execution_path = hook_info.path.as_os_str().to_string_lossy();
                    let display_path = &self.display_info.available_files[self.selected_path_index];
                    if &*execution_path != display_path {
                        // Execution moved to a different file, we simply do not show it
                        self.current_execution_line = None;
                    } else {
                        let offset = hook_info.offset;
                        let execution_line = self
                            .sources
                            .get(display_path)
                            .and_then(|s| Some(s[..offset].lines().count()))
                            .unwrap_or_default(); // 1-based
                        self.current_execution_line = Some(execution_line);
                    }
                }
                SnapshotInfo::Opcode(opcode_info) => {
                    let execution_line = self
                        .opcodes
                        .iter()
                        .position(|(pc, _)| *pc as usize == opcode_info.pc)
                        .map(|idx| idx + 1) // 1-based
                        .unwrap_or_default();
                    self.current_execution_line = Some(execution_line);
                }
            }

            // Simply sync the execution line with the user cursor
            self.current_execution_line = self.user_cursor_line;
        }

        Some(())
    }
}

impl PanelTr for CodePanel {
    fn panel_type(&self) -> PanelType {
        PanelType::Code
    }

    fn title(&self, _dm: &mut DataManager) -> String {
        let mode_str = match self.display_info.mode {
            CodeMode::Source => "Source",
            CodeMode::Opcodes => "Opcodes",
        };

        // Show source availability status
        let availability =
            if self.display_info.has_source_code { "✓" } else { "✗ Opcodes Only" };

        let path_str = if self.display_info.available_files.is_empty() {
            "No source"
        } else {
            self.display_info.available_files[self.selected_path_index].as_str()
        };

        // Show file count if multiple files available
        let file_count = if self.display_info.available_files.len() > 1 {
            format!(
                " [{}/{}]",
                self.selected_path_index + 1,
                self.display_info.available_files.len()
            )
        } else {
            String::new()
        };

        format!("{} {} - {}{}", mode_str, availability, path_str, file_count)
    }

    fn render(&mut self, frame: &mut Frame<'_>, area: Rect, dm: &mut DataManager) {
        if self
            .update_display_info(dm)
            .and_then(|_| self.fresh_source_code(dm))
            .and_then(|_| self.update_execution_info(dm))
            .is_none()
        {
            // Do nothing;
        }

        // Split area if file selector is shown
        let (file_selector_area, code_area) = if self.show_file_selector {
            let file_height = (area.height * self.file_selector_height_percent / 100).max(3);
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(file_height), Constraint::Min(0)])
                .split(area);
            (Some(chunks[0]), chunks[1])
        } else {
            (None, area)
        };

        // Render file selector if shown
        if let Some(selector_area) = file_selector_area {
            self.render_file_selector(frame, selector_area, dm);
        }

        // Render main code content
        self.render_code_content(frame, code_area, dm);
    }

    fn handle_key_event(&mut self, event: KeyEvent, dm: &mut DataManager) -> Result<EventResponse> {
        if !self.focused || event.kind != KeyEventKind::Press {
            return Ok(EventResponse::NotHandled);
        }

        // Handle VIM command mode first
        if self.vim_command_mode {
            match event.code {
                KeyCode::Char(c) => {
                    self.vim_command_buffer.push(c);
                    Ok(EventResponse::Handled)
                }
                KeyCode::Backspace => {
                    self.vim_command_buffer.pop();
                    Ok(EventResponse::Handled)
                }
                KeyCode::Enter => {
                    self.execute_vim_command();
                    Ok(EventResponse::Handled)
                }
                KeyCode::Esc => {
                    self.vim_command_buffer.clear();
                    self.vim_command_mode = false;
                    Ok(EventResponse::Handled)
                }
                _ => Ok(EventResponse::Handled),
            }
        } else {
            match event.code {
                // Handle numeric input for VIM repetition
                KeyCode::Char(c) if c.is_ascii_digit() && !self.show_file_selector => {
                    self.vim_number_prefix.push(c);
                    Ok(EventResponse::Handled)
                }
                // 'F' key toggles file selector
                KeyCode::Char('f') | KeyCode::Char('F') => {
                    self.toggle_file_selector();
                    Ok(EventResponse::Handled)
                }
                // Handle file selector navigation when it's open
                _ if self.show_file_selector => {
                    match event.code {
                        KeyCode::Up => {
                            self.file_selector_up();
                            Ok(EventResponse::Handled)
                        }
                        KeyCode::Down => {
                            self.file_selector_down();
                            Ok(EventResponse::Handled)
                        }
                        KeyCode::Enter => {
                            self.select_file_from_selector();
                            Ok(EventResponse::Handled)
                        }
                        KeyCode::Char('f') | KeyCode::Char('F') => {
                            self.show_file_selector = false;
                            debug!("File selector closed with F");
                            Ok(EventResponse::Handled)
                        }
                        // Allow breakpoints to work in file selector mode
                        KeyCode::Char('b') | KeyCode::Char('B') => {
                            if let Some(line) = self.user_cursor_line {
                                // TODO
                                let added = false;
                                if added {
                                    debug!("Added breakpoint at line {}", line);
                                } else {
                                    debug!("Removed breakpoint at line {}", line);
                                }
                            }
                            {
                                debug!("No user cursor position for breakpoint");
                            }
                            Ok(EventResponse::Handled)
                        }
                        _ => Ok(EventResponse::NotHandled),
                    }
                }
                // VIM-like navigation: k (up)
                KeyCode::Up | KeyCode::Char('k') => {
                    let count = self.get_vim_repetition();
                    self.move_up(count);
                    self.clear_vim_state();
                    Ok(EventResponse::Handled)
                }
                // VIM-like navigation: j (down)
                KeyCode::Down | KeyCode::Char('j') => {
                    let count = self.get_vim_repetition();
                    self.move_down(count);
                    self.clear_vim_state();
                    Ok(EventResponse::Handled)
                }
                // VIM-like navigation: h (left scroll)
                KeyCode::Left | KeyCode::Char('h') => {
                    let count = self.get_vim_repetition();
                    if self.horizontal_offset > 0 {
                        self.horizontal_offset = self.horizontal_offset.saturating_sub(count * 5);
                    }
                    self.clear_vim_state();
                    Ok(EventResponse::Handled)
                }
                // VIM-like navigation: l (right scroll)
                KeyCode::Right | KeyCode::Char('l') => {
                    let count = self.get_vim_repetition();
                    if self.max_line_width > self.content_width {
                        let max_scroll = self.max_line_width.saturating_sub(self.content_width);
                        if self.horizontal_offset < max_scroll {
                            self.horizontal_offset =
                                (self.horizontal_offset + count * 5).min(max_scroll);
                        }
                    }
                    self.clear_vim_state();
                    Ok(EventResponse::Handled)
                }
                // VIM-like navigation: { (previous blank line)
                KeyCode::Char('{') => {
                    let count = self.get_vim_repetition();
                    self.vim_prev_blank_line(count);
                    self.clear_vim_state();
                    Ok(EventResponse::Handled)
                }
                // VIM-like navigation: } (next blank line)
                KeyCode::Char('}') => {
                    let count = self.get_vim_repetition();
                    self.vim_next_blank_line(count);
                    self.clear_vim_state();
                    Ok(EventResponse::Handled)
                }
                // VIM-like navigation: g (might be gg)
                KeyCode::Char('g') => {
                    // Handle 'gg' sequence
                    if self.vim_number_prefix == "g" {
                        self.goto_top();
                        self.vim_number_prefix.clear();
                    } else {
                        self.vim_number_prefix = String::from("g");
                    }
                    Ok(EventResponse::Handled)
                }
                // VIM-like navigation: G (go to bottom or specific line)
                KeyCode::Char('G') => {
                    if self.vim_number_prefix.is_empty() {
                        // G without number - go to bottom
                        self.goto_bottom();
                    } else {
                        // nG - go to line n
                        let line = self.get_vim_repetition();
                        self.move_to(line);
                    }
                    self.clear_vim_state();
                    Ok(EventResponse::Handled)
                }
                // VIM command mode
                KeyCode::Char(':') => {
                    self.vim_command_mode = true;
                    self.vim_command_buffer.clear();
                    Ok(EventResponse::Handled)
                }
                KeyCode::Char('b') | KeyCode::Char('B') => {
                    // Toggle breakpoint at user cursor position
                    if let Some(line) = self.user_cursor_line {
                        // TODO
                        let added = false;
                        if added {
                            debug!("Added breakpoint at line {}", line);
                        } else {
                            debug!("Removed breakpoint at line {}", line);
                        }
                    } else {
                        debug!("No user cursor position for breakpoint");
                    }
                    Ok(EventResponse::Handled)
                }
                KeyCode::Char('s') => {
                    // Step: Move to next snapshot/instruction
                    debug!("Step (next instruction) requested from code panel");
                    dm.execution.step(1)?;
                    Ok(EventResponse::Handled)
                }
                KeyCode::Char('r') => {
                    debug!("Reverse step (previous instruction) requested from code panel");
                    dm.execution.reverse_step(1)?;
                    Ok(EventResponse::Handled)
                }
                KeyCode::Char('n') => {
                    // Next: Step over function calls (skip internal calls)
                    // TODO: This will send a next command to the RPC server
                    debug!("Next (step over) requested from code panel");
                    // // For now, simulate stepping over to next significant point
                    // let current = dm.execution.current_snapshot;
                    // let total = dm.execution.snapshot_count;
                    // let next_pos = (current + 5).min(total.saturating_sub(1));
                    // // TODO
                    // // dm.execution_mut().update_state(next_pos, total, Some(next_pos + 9), None);
                    // debug!("Next (step over) to snapshot {}", next_pos);

                    Ok(EventResponse::Handled)
                }
                KeyCode::Char('p') => {
                    // Previous: Step back over function calls (reverse step over)
                    // TODO: This will send a previous command to the RPC server
                    debug!("Previous (reverse step over) requested from code panel");
                    // // For now, simulate stepping back to previous significant point
                    // let current = dm.execution.current_snapshot;
                    // let total = dm.execution.snapshot_count;
                    // let prev_pos = current.saturating_sub(5);
                    // // TODO
                    // // dm.execution_mut().update_state(prev_pos, total, Some(prev_pos + 9), None);
                    // debug!("Previous (reverse step over) to snapshot {}", prev_pos);

                    Ok(EventResponse::Handled)
                }
                KeyCode::Char('c') => {
                    // TODO: Step into next call (forward call navigation)
                    // This will send a command to the RPC server to step into the next function call
                    debug!("Next call navigation requested");
                    Ok(EventResponse::Handled)
                }
                KeyCode::Char('C') => {
                    // TODO: Step back from call (reverse call navigation)
                    // This will send a command to the RPC server to step back from the current call
                    debug!("Previous call navigation requested");
                    Ok(EventResponse::Handled)
                }
                _ => Ok(EventResponse::NotHandled),
            }
        }
    }

    fn on_focus(&mut self) {
        self.focused = true;
        debug!("Code panel gained focus");
    }

    fn on_blur(&mut self) {
        self.focused = false;
        debug!("Code panel lost focus");
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
