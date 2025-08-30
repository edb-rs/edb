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
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use eyre::Result;
use ratatui::{
    layout::Rect,
    style::Style,
    widgets::{List, ListItem, Paragraph},
    Frame,
};
use tracing::debug;

/// Display modes for the panel
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayMode {
    /// Show variables in scope
    Variables,
    /// Show current stack
    Stack,
    /// Show memory contents
    Memory,
    /// Show call data
    CallData,
    /// Show account state
    State,
    /// Show transition state
    TransitionState,
    /// Show breakpoints
    Breakpoints,
}

impl DisplayMode {
    /// Get display name for the mode
    pub fn name(&self) -> &'static str {
        match self {
            DisplayMode::Variables => "Variables",
            DisplayMode::Stack => "Stack",
            DisplayMode::Memory => "Memory",
            DisplayMode::CallData => "Call Data",
            DisplayMode::State => "State",
            DisplayMode::TransitionState => "Transition State",
            DisplayMode::Breakpoints => "Breakpoints",
        }
    }

    /// Get next mode in cycle
    pub fn next(&self) -> DisplayMode {
        match self {
            DisplayMode::Variables => DisplayMode::Stack,
            DisplayMode::Stack => DisplayMode::Memory,
            DisplayMode::Memory => DisplayMode::CallData,
            DisplayMode::CallData => DisplayMode::State,
            DisplayMode::State => DisplayMode::TransitionState,
            DisplayMode::TransitionState => DisplayMode::Breakpoints,
            DisplayMode::Breakpoints => DisplayMode::Variables,
        }
    }

    /// Get prev mode in cycle
    pub fn prev(&self) -> DisplayMode {
        match self {
            DisplayMode::Variables => DisplayMode::Breakpoints,
            DisplayMode::Stack => DisplayMode::Variables,
            DisplayMode::Memory => DisplayMode::Stack,
            DisplayMode::CallData => DisplayMode::Memory,
            DisplayMode::State => DisplayMode::CallData,
            DisplayMode::TransitionState => DisplayMode::State,
            DisplayMode::Breakpoints => DisplayMode::TransitionState,
        }
    }
}

/// Display panel implementation (stub)
#[derive(Debug)]
pub struct DisplayPanel {
    // ========== Display ==========
    /// Current display mode
    mode: DisplayMode,
    /// Selected item index
    selected_index: usize,
    /// Scroll offset for auto-scrolling
    scroll_offset: usize,
    /// Content height for viewport calculations
    context_height: usize,
    /// Whether this panel is focused
    focused: bool,

    // ========== Data ==========
    /// Mock data for different modes
    variables: Vec<String>,
    stack: Vec<String>,
    memory: Vec<String>,
}

impl DisplayPanel {
    /// Create a new display panel
    pub fn new() -> Self {
        Self {
            mode: DisplayMode::Variables,
            selected_index: 0,
            scroll_offset: 0,
            context_height: 0,
            variables: vec![
                "totalSupply: uint256 = 1000000".to_string(),
                "balances[msg.sender]: uint256 = 500000".to_string(),
                "balances[to]: uint256 = 0".to_string(),
                "msg.sender: address = 0x123...abc".to_string(),
                "to: address = 0x456...def".to_string(),
                "amount: uint256 = 1000".to_string(),
            ],
            stack: vec![
                "[0] 0x456...def (address 'to')".to_string(),
                "[1] 0x000003e8 (uint256 1000)".to_string(),
                "[2] 0x123...abc (address msg.sender)".to_string(),
                "[3] 0x0007a120 (uint256 500000)".to_string(),
                "[4] 0x00000001".to_string(),
            ],
            memory: vec![
                "0x00: 0x0000000000000000000000000000000000000000000000000000000000000080"
                    .to_string(),
                "0x20: 0x0000000000000000000000000000000000000000000000000000000000000000"
                    .to_string(),
                "0x40: 0x0000000000000000000000000000000000000000000000000000000000000000"
                    .to_string(),
                "0x60: 0xa9059cbb000000000000000000000000456...def000000000000000003e8".to_string(),
            ],
            focused: false,
        }
    }

    /// Switch to next display mode
    fn next_mode(&mut self) {
        self.mode = self.mode.next();
        self.selected_index = 0; // Reset selection when changing modes
        self.scroll_offset = 0; // Reset scroll when changing modes
        debug!("Switched to display mode: {:?}", self.mode);
    }

    /// Switch to previous display mode
    fn prev_mode(&mut self) {
        self.mode = self.mode.prev();
        self.selected_index = 0; // Reset selection when changing modes
        self.scroll_offset = 0; // Reset scroll when changing modes
        debug!("Switched to display mode: {:?}", self.mode);
    }

    /// Get current data for display
    fn get_current_data(&self) -> Vec<String> {
        match self.mode {
            DisplayMode::Variables => self.variables.clone(),
            DisplayMode::Stack => self.stack.clone(),
            DisplayMode::Memory => self.memory.clone(),
            DisplayMode::CallData => vec!["Call data display not implemented".to_string()],
            DisplayMode::State => vec!["State display not implemented".to_string()],
            DisplayMode::TransitionState => {
                vec!["Transition state display not implemented".to_string()]
            }
            DisplayMode::Breakpoints => {
                // TODO
                vec!["No breakpoints set".to_string()]
            }
        }
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
        let data = self.get_current_data();
        let max_lines = data.len();
        if self.selected_index < max_lines.saturating_sub(1) {
            self.selected_index += 1;
            let viewport_height = self.context_height;
            // Auto-scroll down if selection moves below visible area
            if self.selected_index >= self.scroll_offset + viewport_height {
                self.scroll_offset = (self.selected_index + 1).saturating_sub(viewport_height);
            }
        }
    }
}

impl PanelTr for DisplayPanel {
    fn panel_type(&self) -> PanelType {
        PanelType::Display
    }

    fn title(&self, _dm: &mut DataManager) -> String {
        let data = self.get_current_data();
        format!("{} ({} items)", self.mode.name(), data.len())
    }

    fn render(&mut self, frame: &mut Frame<'_>, area: Rect, dm: &mut DataManager) {
        // Calculate context height for viewport calculations
        self.context_height = if self.focused && area.height > 10 {
            area.height.saturating_sub(4) // Account for borders and status lines
        } else {
            area.height.saturating_sub(2) // Just borders
        } as usize;

        let data = self.get_current_data();

        if data.is_empty() {
            let paragraph =
                Paragraph::new(format!("No {} data available", self.mode.name().to_lowercase()))
                    .block(BorderPresets::display(
                        self.focused,
                        self.title(dm),
                        dm.theme.focused_border,
                        dm.theme.unfocused_border,
                    ));
            frame.render_widget(paragraph, area);
            return;
        }

        // Create list items with selection highlighting and viewport scrolling
        let items: Vec<ListItem<'_>> = data
            .iter()
            .enumerate()
            .skip(self.scroll_offset) // Skip items before viewport
            .take(self.context_height) // Take only visible items
            .map(|(i, item)| {
                let style = if i == self.selected_index && self.focused {
                    Style::default().bg(dm.theme.selection_bg).fg(dm.theme.selection_fg)
                } else if i == self.selected_index {
                    Style::default().bg(dm.theme.highlight_bg)
                } else {
                    Style::default()
                };
                ListItem::new(item.as_str()).style(style)
            })
            .collect();

        let list = List::new(items)
            .block(BorderPresets::display(
                self.focused,
                self.title(dm),
                dm.theme.focused_border,
                dm.theme.unfocused_border,
            ))
            .highlight_style(Style::default().bg(dm.theme.selection_bg));

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

            let status_bar = StatusBar::new()
                .current_panel("Display".to_string())
                .message(format!("Mode: {}", self.mode.name()))
                .message(format!("Items: {}", self.get_current_data().len()));

            let status_text = status_bar.build();
            let status_paragraph =
                Paragraph::new(status_text).style(Style::default().fg(dm.theme.accent_color));
            frame.render_widget(status_paragraph, status_area);

            let help_area = Rect {
                x: area.x + 1,
                y: area.y + area.height - 2,
                width: area.width - 2,
                height: 1,
            };
            let help_text = "Space: Switch mode • ↑/↓: Navigate • Enter: Expand/Collapse";
            let help_paragraph =
                Paragraph::new(help_text).style(Style::default().fg(dm.theme.help_text_color));
            frame.render_widget(help_paragraph, help_area);
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
            KeyCode::Char(' ') => {
                self.next_mode();
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
            KeyCode::Enter => {
                let data = self.get_current_data();
                if let Some(item) = data.get(self.selected_index) {
                    debug!("Selected {} item: {}", self.mode.name().to_lowercase(), item);
                    // TODO: Expand/collapse complex variables or perform mode-specific action
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
                let data = self.get_current_data();
                self.selected_index = data.len().saturating_sub(1);
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
