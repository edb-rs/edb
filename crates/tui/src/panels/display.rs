//! Display panel for variables, stack, memory, and other debugging information
//!
//! This panel can switch between different display modes based on context.

use super::{BreakpointManager, EventResponse, Panel, PanelType};
use crate::managers::ExecutionManager;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use eyre::Result;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, List, ListItem, Paragraph},
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
}

/// Display panel implementation (stub)
#[derive(Debug)]
pub struct DisplayPanel {
    /// Current display mode
    mode: DisplayMode,
    /// Selected item index
    selected_index: usize,
    /// Mock data for different modes
    variables: Vec<String>,
    stack: Vec<String>,
    memory: Vec<String>,
    /// Shared breakpoint manager
    breakpoint_manager: Option<BreakpointManager>,
    /// Shared execution state manager
    execution_manager: Option<ExecutionManager>,
    /// Whether this panel is focused
    focused: bool,
}

impl DisplayPanel {
    /// Create a new display panel
    pub fn new() -> Self {
        Self::new_with_managers(None, None)
    }

    /// Create a new display panel with shared breakpoint manager (legacy method)
    pub fn new_with_breakpoints(breakpoint_manager: Option<BreakpointManager>) -> Self {
        Self::new_with_managers(breakpoint_manager, None)
    }

    /// Create a new display panel with shared managers
    pub fn new_with_managers(
        breakpoint_manager: Option<BreakpointManager>,
        execution_manager: Option<ExecutionManager>,
    ) -> Self {
        Self {
            mode: DisplayMode::Variables,
            selected_index: 0,
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
            breakpoint_manager,
            execution_manager,
            focused: false,
        }
    }

    /// Switch to next display mode
    fn next_mode(&mut self) {
        self.mode = self.mode.next();
        self.selected_index = 0; // Reset selection when changing modes
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
                if let Some(mgr) = &self.breakpoint_manager {
                    let breakpoints = mgr.get_all_breakpoints();
                    if breakpoints.is_empty() {
                        vec!["No breakpoints set".to_string()]
                    } else {
                        breakpoints
                            .iter()
                            .map(|line| format!("● Line {} (SimpleToken.sol)", line))
                            .collect()
                    }
                } else {
                    vec!["Breakpoint manager not available".to_string()]
                }
            }
        }
    }

    /// Move selection up
    fn move_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    /// Move selection down
    fn move_down(&mut self) {
        let data = self.get_current_data();
        if self.selected_index < data.len().saturating_sub(1) {
            self.selected_index += 1;
        }
    }
}

impl Panel for DisplayPanel {
    fn panel_type(&self) -> PanelType {
        PanelType::Display
    }

    fn title(&self) -> String {
        let data = self.get_current_data();
        format!("{} ({} items)", self.mode.name(), data.len())
    }

    fn render(&mut self, frame: &mut Frame, area: Rect) {
        let border_color = if self.focused { Color::Cyan } else { Color::Gray };
        let data = self.get_current_data();

        if data.is_empty() {
            let paragraph =
                Paragraph::new(format!("No {} data available", self.mode.name().to_lowercase()))
                    .block(
                        Block::default()
                            .title(self.title())
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(border_color)),
                    );
            frame.render_widget(paragraph, area);
            return;
        }

        // Create list items with selection highlighting
        let items: Vec<ListItem> = data
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let style = if i == self.selected_index && self.focused {
                    Style::default().bg(Color::Blue).fg(Color::White)
                } else if i == self.selected_index {
                    Style::default().bg(Color::DarkGray)
                } else {
                    Style::default()
                };
                ListItem::new(item.as_str()).style(style)
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .title(self.title())
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(border_color)),
            )
            .highlight_style(Style::default().bg(Color::Blue));

        frame.render_widget(list, area);

        // Add help text at the bottom if focused
        if self.focused && area.height > 10 {
            let help_area = Rect {
                x: area.x + 1,
                y: area.y + area.height - 3,
                width: area.width - 2,
                height: 1,
            };
            let help_text = "Tab: Switch mode • ↑/↓: Navigate • Enter: Expand/Collapse • Ctrl+T: Return terminal";
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
            KeyCode::Tab => {
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
}

impl Default for DisplayPanel {
    fn default() -> Self {
        Self::new()
    }
}
