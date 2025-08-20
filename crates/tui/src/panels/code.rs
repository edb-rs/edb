//! Code panel for displaying source code or opcodes
//!
//! This panel shows source code with syntax highlighting and current line indication.

use super::{EventResponse, Panel, PanelType};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use eyre::Result;
use ratatui::{
    layout::Rect,
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

/// Code panel implementation (stub)
#[derive(Debug)]
pub struct CodePanel {
    /// Current code display mode
    mode: CodeMode,
    /// Mock source code lines
    source_lines: Vec<String>,
    /// Mock opcode lines
    opcode_lines: Vec<String>,
    /// Available source paths
    source_paths: Vec<String>,
    /// Currently selected source path index
    selected_path_index: usize,
    /// Current highlighted line (1-based)
    current_line: Option<usize>,
    /// Scroll offset
    scroll_offset: usize,
    /// Whether this panel is focused
    focused: bool,
}

impl CodePanel {
    /// Create a new code panel
    pub fn new() -> Self {
        Self {
            mode: CodeMode::Source,
            source_lines: vec![
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
            ],
            opcode_lines: vec![
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
            ],
            source_paths: vec![
                "SimpleToken.sol".to_string(),
                "IERC20.sol".to_string(),
                "SafeMath.sol".to_string(),
            ],
            selected_path_index: 0,
            current_line: Some(9),
            scroll_offset: 0,
            focused: false,
        }
    }

    /// Toggle between source and opcode display
    fn toggle_mode(&mut self) {
        self.mode = match self.mode {
            CodeMode::Source => CodeMode::Opcodes,
            CodeMode::Opcodes => CodeMode::Source,
        };
        debug!("Switched code panel mode to: {:?}", self.mode);
    }

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
        let path_str = if self.source_paths.is_empty() {
            "No source".to_string()
        } else {
            self.source_paths[self.selected_path_index].clone()
        };
        format!("{} - {}", mode_str, path_str)
    }

    fn render(&mut self, frame: &mut Frame, area: Rect) {
        let border_color = if self.focused { Color::Cyan } else { Color::Gray };
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
        let display_lines: Vec<_> = lines
            .iter()
            .enumerate()
            .skip(self.scroll_offset)
            .take((area.height as usize).saturating_sub(4)) // Account for borders and title
            .collect();

        let items: Vec<ListItem> = display_lines
            .iter()
            .map(|(line_idx, line)| {
                let line_num = line_idx + 1;
                let is_current = self.current_line.map_or(false, |current| current == line_num);

                let line_content = if self.mode == CodeMode::Source {
                    format!("{:3} | {}", line_num, line)
                } else {
                    line.to_string()
                };

                let style = if is_current {
                    Style::default().bg(Color::Yellow).fg(Color::Black)
                } else {
                    Style::default()
                };

                ListItem::new(line_content).style(style)
            })
            .collect();

        let list = List::new(items).block(
            Block::default()
                .title(self.title())
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color)),
        );

        frame.render_widget(list, area);

        // Add help text at the bottom if focused
        if self.focused && area.height > 12 {
            let help_area = Rect {
                x: area.x + 1,
                y: area.y + area.height - 3,
                width: area.width - 2,
                height: 1,
            };
            let help_text =
                "O: Toggle source/opcodes • Tab: Switch paths • ↑/↓: Scroll • B: Breakpoint";
            let help_paragraph =
                Paragraph::new(help_text).style(Style::default().fg(Color::Yellow));
            frame.render_widget(help_paragraph, help_area);
        }
    }

    fn handle_key_event(&mut self, event: KeyEvent) -> Result<EventResponse> {
        if !self.focused || event.kind != KeyEventKind::Press {
            return Ok(EventResponse::NotHandled);
        }

        match event.code {
            KeyCode::Char('o') | KeyCode::Char('O') => {
                self.toggle_mode();
                Ok(EventResponse::Handled)
            }
            KeyCode::Tab => {
                self.next_source_path();
                Ok(EventResponse::Handled)
            }
            KeyCode::BackTab => {
                self.prev_source_path();
                Ok(EventResponse::Handled)
            }
            KeyCode::Up => {
                self.scroll_up();
                Ok(EventResponse::Handled)
            }
            KeyCode::Down => {
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
            KeyCode::Char('b') | KeyCode::Char('B') => {
                debug!("Breakpoint toggle requested (not implemented)");
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
