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
use crate::managers::execution::ExecutionManager;
use crate::managers::resolve::Resolver;
use crate::managers::theme::ThemeManager;
use crate::managers::{ExecutionManagerCore, ResolverCore, ThemeManagerCore};
use crate::ui::borders::BorderPresets;
use crate::ui::status::{FileStatus, StatusBar};
use crate::ui::syntax::{SyntaxHighlighter, SyntaxType};
use crate::ColorScheme;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use edb_common::types::Code;
use eyre::Result;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, List, ListItem},
    Frame,
};
use std::collections::HashMap;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};
use tracing::debug;

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
    /// Syntax highlighter for code
    syntax_highlighter: SyntaxHighlighter,

    // ========== Data ==========
    /// Current snapshot id (flag)
    current_snapshot_id: Option<usize>,
    /// Current selected path (flag)
    current_selected_path_id: Option<usize>,
    /// Server-provided display information
    display_info: CodeDisplayInfo,
    /// Mock source code lines
    source_lines: Vec<String>,
    /// Mock opcode lines
    opcode_lines: Vec<String>,
    /// Available source code
    sources: HashMap<String, String>,
    /// Available source paths
    source_paths: Vec<String>,
    /// Currently selected source path index
    selected_path_index: usize,

    // ========== Managers ==========
    /// Shared execution state manager
    exec_mgr: ExecutionManager,
    /// Shared label/abi resolver
    resolver: Resolver,
    /// Shared theme manager for styling
    theme_mgr: ThemeManager,
}

impl CodePanel {
    /// Create a new code panel
    pub fn new(exec_mgr: ExecutionManager, resolver: Resolver, theme_mgr: ThemeManager) -> Self {
        // Mock server response - for addresses WITH source code
        // In reality, if has_source_code is true, we show source
        // If has_source_code is false, we show opcodes
        // Create mock file information with metadata
        let file_info = vec![
            FileInfo {
                path: "contracts/SimpleToken.sol".to_string(),
                line_count: 45,
                has_execution: true, // This file contains current execution
            },
            FileInfo {
                path: "contracts/interfaces/IERC20.sol".to_string(),
                line_count: 28,
                has_execution: false,
            },
            FileInfo {
                path: "contracts/libraries/SafeMath.sol".to_string(),
                line_count: 156,
                has_execution: false,
            },
            FileInfo {
                path: "contracts/utils/Context.sol".to_string(),
                line_count: 12,
                has_execution: false,
            },
        ];

        let display_info = CodeDisplayInfo {
            has_source_code: true, // This address has source code
            mode: CodeMode::Source,
            available_files: file_info.iter().map(|f| f.path.clone()).collect(),
            file_info,
        };

        // If we have source code, populate source_lines
        // Otherwise, populate opcode_lines (but never both!)
        let source_lines = if display_info.has_source_code {
            vec![
                "// SPDX-License-Identifier: MIT".to_string(),
                "pragma solidity ^0.8.0;".to_string(),
                "".to_string(),
                "contract SimpleToken {".to_string(),
                "    uint256 public totalSupply;".to_string(),
                "    mapping(address => uint256) public balances;".to_string(),
                "    ".to_string(),
                "    function transfer(address to, uint256 amount) public {".to_string(),
                "        require(balances[msg.sender] >= amount);  // ← Current".to_string(),
                "        balances[msg.sender] -= amount;".to_string(),
                "        balances[to] += amount;".to_string(),
                "    }".to_string(),
                "}".to_string(),
            ]
        } else {
            vec![]
        };

        let opcode_lines = if !display_info.has_source_code {
            vec![
                "000: PUSH1 0x80".to_string(),
                "002: PUSH1 0x40".to_string(),
                "004: MSTORE".to_string(),
                "005: CALLVALUE".to_string(),
                "006: DUP1".to_string(),
                "007: ISZERO".to_string(),
                "008: PUSH2 0x0010".to_string(),
                "011: JUMPI    ← Current".to_string(),
                "012: PUSH1 0x00".to_string(),
                "014: DUP1".to_string(),
                "015: REVERT".to_string(),
            ]
        } else {
            vec![]
        };

        Self {
            display_info,
            current_snapshot_id: None,
            current_selected_path_id: None,
            source_lines: vec![],
            opcode_lines: vec![],
            source_paths: vec![],
            sources: HashMap::new(),
            selected_path_index: 0,
            current_execution_line: Some(10), // TODO
            user_cursor_line: Some(1),
            scroll_offset: 0,
            focused: false,
            show_file_selector: false,
            file_selector_index: 0,
            file_selector_scroll_offset: 0,
            file_selector_context_height: 0,
            file_selector_height_percent: 20,
            context_height: 0,
            exec_mgr,
            resolver,
            theme_mgr,
            syntax_highlighter: SyntaxHighlighter::new(),
        }
    }

    /// Get current lines to display
    fn get_display_lines(&self) -> &Vec<String> {
        match self.display_info.mode {
            CodeMode::Source => &self.source_lines,
            CodeMode::Opcodes => &self.opcode_lines,
        }
    }

    /// Apply syntax highlighting to a line and return styled text
    fn highlight_line<'a>(
        &self,
        line: &'a str,
        line_num: usize,
        max_line_num: usize,
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
            Style::default()
                .fg(self.theme_mgr.color_scheme.line_number)
                .bg(self.theme_mgr.color_scheme.line_number_bg),
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
            let token_style = self
                .syntax_highlighter
                .get_token_style(token.token_type, &self.theme_mgr.color_scheme);
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
            let filename = selected_file.path.split('/').last().unwrap_or(&selected_file.path);

            // Find matching index in source_paths
            if let Some(index) = self.source_paths.iter().position(|p| p == filename) {
                self.selected_path_index = index;
            }

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
    fn render_file_selector(&mut self, frame: &mut Frame, area: Rect) {
        // Calculate file selector context height for viewport calculations
        self.file_selector_context_height = area.height.saturating_sub(2) as usize; // Account for borders

        let sorted_files = self.get_sorted_files();

        let items: Vec<ListItem> = sorted_files
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
                    Style::default()
                        .bg(self.theme_mgr.color_scheme.selection_bg)
                        .fg(self.theme_mgr.color_scheme.selection_fg)
                } else if file_info.has_execution {
                    // Highlight files with current execution
                    Style::default().fg(self.theme_mgr.color_scheme.warning_color)
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
                    .border_style(Style::default().fg(self.theme_mgr.color_scheme.success_color)),
            )
            .highlight_style(Style::default().bg(self.theme_mgr.color_scheme.selection_bg));

        frame.render_widget(file_list, area);
    }

    /// Render the main code content with syntax highlighting
    fn render_code_content(&mut self, frame: &mut Frame, area: Rect) {
        use ratatui::text::{Line, Span};
        use ratatui::widgets::{List, ListItem, Paragraph};

        // Calculate viewport height
        self.context_height = if self.focused && area.height > 10 {
            area.height.saturating_sub(4) // Account for borders and status lines
        } else {
            area.height.saturating_sub(2) // Just borders
        } as usize;

        let lines = self.get_display_lines();

        if lines.is_empty() {
            let paragraph = Paragraph::new("No code available").block(BorderPresets::code(
                self.focused,
                self.title(),
                self.theme_mgr.color_scheme.focused_border,
                self.theme_mgr.color_scheme.unfocused_border,
            ));
            frame.render_widget(paragraph, area);
            return;
        }

        // Get the display lines with viewport scrolling
        let display_lines: Vec<_> =
            lines.iter().enumerate().skip(self.scroll_offset).take(self.context_height).collect();

        let max_line_num = lines.len();

        // Create list items with syntax highlighting, line numbers, and indicators
        let list_items: Vec<ListItem> = display_lines
            .iter()
            .map(|(line_idx, line)| {
                let line_num = line_idx + 1;
                let is_execution =
                    self.current_execution_line.map_or(false, |exec| exec == line_num);
                let is_user_cursor = self.user_cursor_line.map_or(false, |user| user == line_num);
                let has_breakpoint = false; // TODO

                // Start with syntax-highlighted line
                let highlighted_line = self.highlight_line(line, line_num, max_line_num);

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
                    Span::styled("●", Style::default().fg(self.theme_mgr.color_scheme.error_color))
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

                let content_line = Line::from(new_spans);

                // Apply background highlighting for execution/cursor lines
                let item_style = if is_execution {
                    Style::default().bg(self.theme_mgr.color_scheme.current_line_bg)
                } else if is_user_cursor {
                    Style::default().bg(self.theme_mgr.color_scheme.highlight_bg)
                } else {
                    Style::default()
                };

                ListItem::new(content_line).style(item_style)
            })
            .collect();

        // Create list with highlighted content
        let code_list = List::new(list_items).block(BorderPresets::code(
            self.focused,
            self.title(),
            self.theme_mgr.color_scheme.focused_border,
            self.theme_mgr.color_scheme.unfocused_border,
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
            let status_paragraph = Paragraph::new(status_text)
                .style(Style::default().fg(self.theme_mgr.color_scheme.accent_color));
            frame.render_widget(status_paragraph, status_area);

            // Help line - updated to include file selector
            let help_area = Rect {
                x: area.x + 1,
                y: area.y + area.height - 2,
                width: area.width - 2,
                height: 1,
            };
            let help_text = if self.show_file_selector {
                "↑/↓: Navigate • Enter: Select • F: Close".to_string()
            } else if self.display_info.mode == CodeMode::Source {
                "↑/↓: Navigate • s/r/n/p: Step/Rev/Next/Prev • c/C: Next/Prev call • F: Files • B: Breakpoint"
                    .to_string()
            } else {
                "↑/↓: Navigate • s/r/n/p: Step/Rev/Next/Prev • c/C: Next/Prev call • B: Breakpoint"
                    .to_string()
            };
            let help_paragraph = Paragraph::new(help_text)
                .style(Style::default().fg(self.theme_mgr.color_scheme.help_text_color));
            frame.render_widget(help_paragraph, help_area);
        }
    }

    fn update_display_info(&mut self) -> Option<()> {
        let id = self.exec_mgr.get_display_snapshot();
        if self.current_snapshot_id == Some(id) {
            return Some(()); // No change
        }

        let execution_path = self.exec_mgr.get_snapshot_info(id)?.path().cloned();
        let code = self.exec_mgr.get_code(id)?;
        match code {
            Code::Source(info) => {
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

                self.source_paths = self.display_info.available_files.clone();
                self.source_lines.clear(); // We will update this later
                self.opcode_lines.clear();

                self.sources = info
                    .sources
                    .iter()
                    .map(|(p, s)| {
                        let path = p.as_os_str().to_string_lossy().to_string();
                        (path, s.clone())
                    })
                    .collect();

                self.selected_path_index =
                    self.display_info.file_info.iter().position(|p| p.has_execution).unwrap_or(0);
            }
            Code::Opcode(info) => {
                self.display_info.has_source_code = false;
                self.display_info.mode = CodeMode::Opcodes;
                self.display_info.available_files.clear();
                self.display_info.file_info.clear();

                self.source_lines.clear();
                self.source_paths.clear();
                self.selected_path_index = 0;

                let mut opcodes: Vec<_> =
                    info.codes.iter().map(|(pc, insn)| (*pc, insn.clone())).collect();
                opcodes.sort_by_key(|(pc, _)| *pc);

                // TODO: We need a better way to render opcodes
                self.opcode_lines =
                    opcodes.iter().map(|(pc, insn)| format!("{:03}: {}", pc, insn)).collect();
            }
        }

        // Reset scroll and cursor
        self.scroll_offset = 0;
        self.user_cursor_line = Some(1);

        self.current_snapshot_id = Some(id);
        self.current_selected_path_id = None; // We always reset the selected path
        Some(())
    }

    fn fresh_source_code(&mut self) {
        if self.current_selected_path_id == Some(self.selected_path_index) {
            return; // No change
        }

        if !self.display_info.has_source_code || self.source_paths.is_empty() {
            self.source_lines.clear();
            return;
        }

        let selected_file = &self.source_paths[self.selected_path_index];

        if let Some(source) = self.sources.get(selected_file) {
            self.source_lines = source.lines().map(|l| l.to_string()).collect();
        }

        // Reset scroll and cursor
        self.scroll_offset = 0;
        self.user_cursor_line = Some(1);

        self.current_selected_path_id = Some(self.selected_path_index);
    }
}

impl PanelTr for CodePanel {
    fn panel_type(&self) -> PanelType {
        PanelType::Code
    }

    fn title(&self) -> String {
        let mode_str = match self.display_info.mode {
            CodeMode::Source => "Source",
            CodeMode::Opcodes => "Opcodes",
        };

        // Show source availability status
        let availability =
            if self.display_info.has_source_code { "✓" } else { "✗ Opcodes Only" };

        let path_str = if self.source_paths.is_empty() {
            "No source".to_string()
        } else {
            self.source_paths[self.selected_path_index].clone()
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

    fn render(&mut self, frame: &mut Frame, area: Rect) {
        if self.update_display_info().is_some() {
            self.fresh_source_code();
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
            self.render_file_selector(frame, selector_area);
        }

        // Render main code content
        self.render_code_content(frame, code_area);
    }

    fn handle_key_event(&mut self, event: KeyEvent) -> Result<EventResponse> {
        if !self.focused || event.kind != KeyEventKind::Press {
            return Ok(EventResponse::NotHandled);
        }

        match event.code {
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
            KeyCode::Up => {
                // Move user cursor up with automatic scrolling
                if let Some(line) = self.user_cursor_line {
                    if line > 1 {
                        self.user_cursor_line = Some(line - 1);
                        // Auto-scroll if cursor moves out of view
                        if line - 1 < self.scroll_offset + 1 {
                            self.scroll_offset = self.scroll_offset.saturating_sub(1);
                        }
                    }
                } else {
                    // If no user cursor, start at current view position
                    self.user_cursor_line = Some(self.scroll_offset + 1);
                }
                Ok(EventResponse::Handled)
            }
            KeyCode::Down => {
                // Move user cursor down with automatic scrolling
                let max_lines = self.get_display_lines().len();
                if let Some(line) = self.user_cursor_line {
                    if line < max_lines {
                        self.user_cursor_line = Some(line + 1);
                        // Auto-scroll if cursor moves out of view
                        // We need to ensure cursor stays visible in the viewport
                        let viewport_height = self.context_height;
                        if line + 1 > self.scroll_offset + viewport_height {
                            self.scroll_offset = (line + 1).saturating_sub(viewport_height);
                        }
                    }
                } else {
                    // If no user cursor, start at current view position
                    self.user_cursor_line = Some(self.scroll_offset + 1);
                }
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
                // TODO: This will send a step command to the RPC server
                debug!("Step (next instruction) requested from code panel");
                // For now, simulate moving to next snapshot
                // let current = self.exec_mgr.current_snapshot;
                // let total = self.exec_mgr.snapshot_count;
                // if current < total.saturating_sub(1) {
                //     // TODO
                //     // self.exec_mgr_mut().update_state(current + 1, total, Some(current + 10), None);
                //     debug!("Stepped to snapshot {}", current + 1);
                // }
                Ok(EventResponse::Handled)
            }
            KeyCode::Char('r') => {
                // Reverse step: Move to previous snapshot/instruction
                // TODO: This will send a reverse step command to the RPC server
                debug!("Reverse step (previous instruction) requested from code panel");
                // For now, simulate moving to previous snapshot
                // let current = self.exec_mgr.current_snapshot;
                // let total = self.exec_mgr.snapshot_count;
                // if current > 0 {
                //     // TODO
                //     // self.exec_mgr_mut().update_state(current - 1, total, Some(current + 8), None);
                //     debug!("Reverse stepped to snapshot {}", current - 1);
                // }
                Ok(EventResponse::Handled)
            }
            KeyCode::Char('n') => {
                // Next: Step over function calls (skip internal calls)
                // TODO: This will send a next command to the RPC server
                debug!("Next (step over) requested from code panel");
                // // For now, simulate stepping over to next significant point
                // let current = self.exec_mgr.current_snapshot;
                // let total = self.exec_mgr.snapshot_count;
                // let next_pos = (current + 5).min(total.saturating_sub(1));
                // // TODO
                // // self.exec_mgr_mut().update_state(next_pos, total, Some(next_pos + 9), None);
                // debug!("Next (step over) to snapshot {}", next_pos);

                Ok(EventResponse::Handled)
            }
            KeyCode::Char('p') => {
                // Previous: Step back over function calls (reverse step over)
                // TODO: This will send a previous command to the RPC server
                debug!("Previous (reverse step over) requested from code panel");
                // // For now, simulate stepping back to previous significant point
                // let current = self.exec_mgr.current_snapshot;
                // let total = self.exec_mgr.snapshot_count;
                // let prev_pos = current.saturating_sub(5);
                // // TODO
                // // self.exec_mgr_mut().update_state(prev_pos, total, Some(prev_pos + 9), None);
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

    async fn fetch_data(&mut self) -> Result<()> {
        self.theme_mgr.fetch_data().await?;
        self.exec_mgr.fetch_data().await?;
        self.resolver.fetch_data().await?;
        Ok(())
    }
}
