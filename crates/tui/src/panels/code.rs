//! Code panel for displaying source code or opcodes
//!
//! This panel shows source code with syntax highlighting and current line indication.

use super::{BreakpointManager, EventResponse, Panel, PanelType};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use eyre::Result;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};
use tracing::debug;

/// Code display mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeMode {
    /// Show source code
    Source,
    /// Show opcodes
    Opcodes,
}

/// Type of cursor in code panel
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorType {
    /// Following execution position (server-controlled)
    Execution,
    /// User's navigation cursor (for setting breakpoints)
    User,
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
    pub preferred_display: CodeMode,
    /// Whether opcodes are available (always true)
    pub opcodes_available: bool,
    /// List of available source files for this address
    pub available_files: Vec<String>,
    /// Enhanced file information with metadata
    pub file_info: Vec<FileInfo>,
}

/// Code panel implementation (stub)
#[derive(Debug)]
pub struct CodePanel {
    /// Current code display mode (respects server preferences)
    mode: CodeMode,
    /// Server-provided display information
    display_info: CodeDisplayInfo,
    /// Mock source code lines
    source_lines: Vec<String>,
    /// Mock opcode lines
    opcode_lines: Vec<String>,
    /// Available source paths
    source_paths: Vec<String>,
    /// Currently selected source path index
    selected_path_index: usize,
    /// Current execution line (server-controlled, 1-based)
    current_execution_line: Option<usize>,
    /// User cursor line (user-controlled for breakpoints, 1-based)
    user_cursor_line: Option<usize>,
    /// Which cursor is active (for navigation)
    active_cursor: CursorType,
    /// Scroll offset
    scroll_offset: usize,
    /// Whether this panel is focused
    focused: bool,
    /// Shared breakpoint manager
    breakpoint_manager: Option<BreakpointManager>,
    /// File selector state
    show_file_selector: bool,
    /// Selected file index in file selector
    file_selector_index: usize,
    /// Height percentage for file selector (0-100)
    file_selector_height_percent: u16,
}

impl CodePanel {
    /// Create a new code panel
    pub fn new() -> Self {
        Self::new_with_breakpoints(None)
    }

    /// Create a new code panel with shared breakpoint manager
    pub fn new_with_breakpoints(breakpoint_manager: Option<BreakpointManager>) -> Self {
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
            preferred_display: CodeMode::Source,
            opcodes_available: false, // When we have source, we don't show opcodes
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
            mode: display_info.preferred_display, // Use server preference
            display_info,
            source_lines,
            opcode_lines,
            source_paths: vec![
                "SimpleToken.sol".to_string(),
                "IERC20.sol".to_string(),
                "SafeMath.sol".to_string(),
                "Context.sol".to_string(),
            ],
            selected_path_index: 0,
            current_execution_line: Some(9),
            user_cursor_line: Some(9), // Initially follows execution
            active_cursor: CursorType::Execution,
            scroll_offset: 0,
            focused: false,
            breakpoint_manager,
            show_file_selector: false,
            file_selector_index: 0,
            file_selector_height_percent: 20, // Default to 20% of panel height
        }
    }

    // REMOVED: toggle_mode function
    // Source and opcodes are mutually exclusive per address
    // If has_source_code = true, we show source
    // If has_source_code = false, we show opcodes
    // There is no toggling between them

    /// Switch to next source path
    fn next_source_path(&mut self) {
        if !self.source_paths.is_empty() {
            self.selected_path_index = (self.selected_path_index + 1) % self.source_paths.len();
            debug!("Switched to source path: {}", self.source_paths[self.selected_path_index]);
        }
    }

    /// Switch to previous source path
    fn prev_source_path(&mut self) {
        if !self.source_paths.is_empty() {
            self.selected_path_index = if self.selected_path_index == 0 {
                self.source_paths.len() - 1
            } else {
                self.selected_path_index - 1
            };
            debug!("Switched to source path: {}", self.source_paths[self.selected_path_index]);
        }
    }

    /// Scroll up
    fn scroll_up(&mut self) {
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
    }

    /// Scroll down
    fn scroll_down(&mut self) {
        let max_lines = match self.mode {
            CodeMode::Source => self.source_lines.len(),
            CodeMode::Opcodes => self.opcode_lines.len(),
        };
        if self.scroll_offset < max_lines.saturating_sub(1) {
            self.scroll_offset += 1;
        }
    }

    /// Get current lines to display
    fn get_display_lines(&self) -> &Vec<String> {
        match self.mode {
            CodeMode::Source => &self.source_lines,
            CodeMode::Opcodes => &self.opcode_lines,
        }
    }

    /// Toggle file selector visibility
    fn toggle_file_selector(&mut self) {
        self.show_file_selector = !self.show_file_selector;
        if self.show_file_selector {
            // Reset selection to current file when opening
            self.file_selector_index = self.selected_path_index;
        }
        debug!("File selector toggled: {}", self.show_file_selector);
    }

    /// Move file selector up
    fn file_selector_up(&mut self) {
        if self.file_selector_index > 0 {
            self.file_selector_index -= 1;
        }
    }

    /// Move file selector down
    fn file_selector_down(&mut self) {
        let max_index = self.display_info.file_info.len().saturating_sub(1);
        if self.file_selector_index < max_index {
            self.file_selector_index += 1;
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
    fn render_file_selector(&self, frame: &mut Frame, area: Rect, _border_color: Color) {
        let sorted_files = self.get_sorted_files();

        let items: Vec<ListItem> = sorted_files
            .iter()
            .enumerate()
            .map(|(display_idx, (_, file_info))| {
                let filename = file_info.path.split('/').last().unwrap_or(&file_info.path);
                let execution_indicator = if file_info.has_execution { "►" } else { " " };
                let content = format!(
                    "{} 📄 {} ({} lines)",
                    execution_indicator, filename, file_info.line_count
                );

                let style = if display_idx == self.file_selector_index {
                    Style::default().bg(Color::Blue).fg(Color::White)
                } else if file_info.has_execution {
                    Style::default().fg(Color::Yellow) // Highlight files with current execution
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
                    .border_style(Style::default().fg(Color::Green)),
            )
            .highlight_style(Style::default().bg(Color::Blue));

        frame.render_widget(file_list, area);
    }

    /// Render the main code content
    fn render_code_content(&self, frame: &mut Frame, area: Rect, border_color: Color) {
        let lines = self.get_display_lines();

        if lines.is_empty() {
            let paragraph = Paragraph::new("No code available").block(
                Block::default()
                    .title(self.title())
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(border_color)),
            );
            frame.render_widget(paragraph, area);
            return;
        }

        // Create list items with line numbers and highlighting
        let available_height = if self.focused && area.height > 12 {
            area.height.saturating_sub(4) // Account for borders and status lines
        } else {
            area.height.saturating_sub(2) // Just borders
        };

        let display_lines: Vec<_> = lines
            .iter()
            .enumerate()
            .skip(self.scroll_offset)
            .take(available_height as usize)
            .collect();

        let items: Vec<ListItem> = display_lines
            .iter()
            .map(|(line_idx, line)| {
                let line_num = line_idx + 1;
                let is_execution =
                    self.current_execution_line.map_or(false, |exec| exec == line_num);
                let is_user_cursor = self.user_cursor_line.map_or(false, |user| user == line_num);

                // Determine cursor indicator
                let cursor_indicator = if is_execution && is_user_cursor {
                    "◉" // Both cursors on same line
                } else if is_execution {
                    "►" // Execution cursor
                } else if is_user_cursor {
                    "◯" // User cursor
                } else {
                    " " // No cursor
                };

                // Check if this line has a breakpoint
                let has_breakpoint = self
                    .breakpoint_manager
                    .as_ref()
                    .map_or(false, |mgr| mgr.has_breakpoint(line_num));
                let breakpoint_indicator = if has_breakpoint { "●" } else { " " };

                let line_content = if self.mode == CodeMode::Source {
                    format!(
                        "{:3} {} {} | {}",
                        line_num, breakpoint_indicator, cursor_indicator, line
                    )
                } else {
                    format!("{} {} {}", breakpoint_indicator, cursor_indicator, line)
                };

                let style = if is_execution {
                    Style::default().bg(Color::Yellow).fg(Color::Black)
                } else if is_user_cursor {
                    Style::default().bg(Color::DarkGray).fg(Color::White)
                } else {
                    Style::default()
                };

                // Apply red color to breakpoint indicator if present
                let styled_content = if has_breakpoint {
                    // Create a styled line with red breakpoint indicator
                    if self.mode == CodeMode::Source {
                        format!("{:3} ", line_num) + 
                        &format!("\x1b[31m●\x1b[0m") + // Red bullet
                        &format!(" {} | {}", cursor_indicator, line)
                    } else {
                        format!("\x1b[31m●\x1b[0m {} {}", cursor_indicator, line)
                    }
                } else {
                    line_content
                };

                ListItem::new(styled_content).style(style)
            })
            .collect();

        let list = List::new(items).block(
            Block::default()
                .title(self.title())
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color)),
        );

        frame.render_widget(list, area);

        // Add cursor status and help text at the bottom if focused
        if self.focused && area.height > 12 {
            // Status line
            let status_area = Rect {
                x: area.x + 1,
                y: area.y + area.height - 4,
                width: area.width - 2,
                height: 1,
            };

            let exec_line =
                self.current_execution_line.map_or("None".to_string(), |l| l.to_string());
            let user_line = self.user_cursor_line.map_or("None".to_string(), |l| l.to_string());
            let active_cursor_indicator = match self.active_cursor {
                CursorType::Execution => "► Exec",
                CursorType::User => "◯ User",
            };

            let status_text = format!(
                "► Exec: {} │ ◯ User: {} │ Active: {} │ Files: {}/{}",
                exec_line,
                user_line,
                active_cursor_indicator,
                self.selected_path_index + 1,
                self.display_info.file_info.len()
            );
            let status_paragraph =
                Paragraph::new(status_text).style(Style::default().fg(Color::Cyan));
            frame.render_widget(status_paragraph, status_area);

            // Help line - updated to include file selector
            let help_area = Rect {
                x: area.x + 1,
                y: area.y + area.height - 3,
                width: area.width - 2,
                height: 1,
            };
            let help_text = if self.show_file_selector {
                "↑/↓: Navigate files • Enter: Select • Esc/F: Close • B: Breakpoint • Ctrl+T: Return terminal".to_string()
            } else {
                "Space: Switch cursor • ↑/↓: Scroll • j/k: Move cursor • F: Files • B: Breakpoint"
                    .to_string()
            };
            let help_paragraph =
                Paragraph::new(help_text).style(Style::default().fg(Color::Yellow));
            frame.render_widget(help_paragraph, help_area);
        }
    }
}

impl Panel for CodePanel {
    fn panel_type(&self) -> PanelType {
        PanelType::Code
    }

    fn title(&self) -> String {
        let mode_str = match self.mode {
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
        let border_color = if self.focused { Color::Cyan } else { Color::Gray };

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
            self.render_file_selector(frame, selector_area, border_color);
        }

        // Render main code content
        self.render_code_content(frame, code_area, border_color);
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
                    KeyCode::Esc => {
                        self.show_file_selector = false;
                        debug!("File selector closed with Esc");
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
                            if let Some(mgr) = &self.breakpoint_manager {
                                let added = mgr.toggle_breakpoint(line);
                                if added {
                                    debug!("Added breakpoint at line {}", line);
                                } else {
                                    debug!("Removed breakpoint at line {}", line);
                                }
                            }
                        } else {
                            debug!("No user cursor position for breakpoint");
                        }
                        Ok(EventResponse::Handled)
                    }
                    _ => Ok(EventResponse::NotHandled),
                }
            }
            // Normal code navigation when file selector is closed
            KeyCode::Tab => {
                self.next_source_path();
                Ok(EventResponse::Handled)
            }
            KeyCode::BackTab => {
                self.prev_source_path();
                Ok(EventResponse::Handled)
            }
            KeyCode::Char(' ') => {
                // Toggle between execution and user cursor
                self.active_cursor = match self.active_cursor {
                    CursorType::Execution => CursorType::User,
                    CursorType::User => CursorType::Execution,
                };
                debug!("Switched active cursor to: {:?}", self.active_cursor);
                Ok(EventResponse::Handled)
            }
            KeyCode::Up => {
                // Arrow keys now ONLY scroll, they don't move cursors
                // User cursor can be moved with 'j'/'k' keys if needed
                self.scroll_up();
                Ok(EventResponse::Handled)
            }
            KeyCode::Down => {
                // Arrow keys now ONLY scroll, they don't move cursors
                self.scroll_down();
                Ok(EventResponse::Handled)
            }
            KeyCode::PageUp => {
                for _ in 0..10 {
                    self.scroll_up();
                }
                Ok(EventResponse::Handled)
            }
            KeyCode::PageDown => {
                for _ in 0..10 {
                    self.scroll_down();
                }
                Ok(EventResponse::Handled)
            }
            KeyCode::Home => {
                self.scroll_offset = 0;
                Ok(EventResponse::Handled)
            }
            KeyCode::End => {
                let max_lines = self.get_display_lines().len();
                self.scroll_offset = max_lines.saturating_sub(1);
                Ok(EventResponse::Handled)
            }
            KeyCode::Char('j') | KeyCode::Char('J') => {
                // Move user cursor down (vim-style)
                if self.active_cursor == CursorType::User {
                    if let Some(line) = self.user_cursor_line {
                        let max_lines = self.get_display_lines().len();
                        self.user_cursor_line = Some((line + 1).min(max_lines));
                    }
                }
                Ok(EventResponse::Handled)
            }
            KeyCode::Char('k') | KeyCode::Char('K') => {
                // Move user cursor up (vim-style)
                if self.active_cursor == CursorType::User {
                    if let Some(line) = self.user_cursor_line {
                        self.user_cursor_line = Some(line.saturating_sub(1).max(1));
                    }
                }
                Ok(EventResponse::Handled)
            }
            KeyCode::Char('b') | KeyCode::Char('B') => {
                // Toggle breakpoint at user cursor position
                if let Some(line) = self.user_cursor_line {
                    if let Some(mgr) = &self.breakpoint_manager {
                        let added = mgr.toggle_breakpoint(line);
                        if added {
                            debug!("Added breakpoint at line {}", line);
                        } else {
                            debug!("Removed breakpoint at line {}", line);
                        }
                    }
                } else {
                    debug!("No user cursor position for breakpoint");
                }
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
}

impl Default for CodePanel {
    fn default() -> Self {
        Self::new()
    }
}
