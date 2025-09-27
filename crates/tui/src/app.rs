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

//! Main application state and logic
//!
//! This module contains the core application state management and event handling.

use crate::data::DataManager;
use crate::layout::{LayoutConfig, LayoutManager, LayoutType};
use crate::panels::{
    CodePanel, DisplayPanel, EventResponse, HelpOverlay, Panel, PanelTr, PanelType, TerminalPanel,
    TracePanel,
};
use crate::rpc::RpcClient;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent};
use eyre::Result;
use ratatui::layout::Alignment;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    Frame,
};
use std::sync::Arc;
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
use tracing::{debug, warn};

/// Direction for panel boundary resize
#[derive(Debug, Clone, Copy)]
pub enum ResizeDirection {
    Left,
    Right,
    Up,
    Down,
}

/// RPC connection status
#[derive(Debug, Clone)]
pub struct ConnectionStatus {
    /// Whether we're connected to the RPC server
    pub connected: bool,
    /// Last successful connection time
    pub last_success: Option<Instant>,
    /// Last connection attempt time
    pub last_attempt: Option<Instant>,
    /// Response time for last successful request (milliseconds)
    pub response_time_ms: Option<u64>,
    /// Current error message if disconnected
    pub error_message: Option<String>,
    /// Number of consecutive failures
    pub failure_count: u32,
}

impl Default for ConnectionStatus {
    fn default() -> Self {
        Self::new()
    }
}

impl ConnectionStatus {
    /// Create new connection status in disconnected state
    pub fn new() -> Self {
        Self {
            connected: false,
            last_success: None,
            last_attempt: None,
            response_time_ms: None,
            error_message: None,
            failure_count: 0,
        }
    }

    /// Mark connection as successful
    pub fn mark_success(&mut self, response_time_ms: u64) {
        self.connected = true;
        self.last_success = Some(Instant::now());
        self.last_attempt = Some(Instant::now());
        self.response_time_ms = Some(response_time_ms);
        self.error_message = None;
        self.failure_count = 0;
    }

    /// Mark connection as failed
    pub fn mark_failure(&mut self, error: String) {
        self.connected = false;
        self.last_attempt = Some(Instant::now());
        self.error_message = Some(error);
        self.failure_count += 1;
    }

    /// Get status display string for UI
    pub fn status_display(&self) -> String {
        if self.connected {
            if let Some(response_time) = self.response_time_ms {
                format!("🟢 Connected ({response_time}ms)")
            } else {
                "🟢 Connected".to_string()
            }
        } else {
            match self.failure_count {
                0 => "🔸 Connecting...".to_string(),
                1..=3 => format!("🟡 Reconnecting... ({})", self.failure_count),
                _ => "🔴 Disconnected".to_string(),
            }
        }
    }

    /// Get detailed status for debugging
    pub fn detailed_status(&self) -> String {
        let base = self.status_display();
        if let Some(error) = &self.error_message {
            format!("{base} - {error}")
        } else {
            base
        }
    }
}

/// Main application state
pub struct App {
    /// RPC client for communicating with debug server
    pub(crate) rpc_client: Arc<RpcClient>,
    /// Layout manager for responsive design
    layout_manager: LayoutManager,
    /// Current focused panel
    current_panel: PanelType,
    /// All panels
    panels: HashMap<PanelType, Panel>,
    /// Whether the application should exit
    should_exit: bool,
    /// Main panel type for compact layout (Trace/Code/Display cycle)
    compact_main_panel: PanelType,
    /// Left panel type for full layout (Code/Trace toggle)
    full_left_panel: PanelType,
    /// RPC connection status and health monitoring
    connection_status: ConnectionStatus,
    /// Last health check time for periodic monitoring
    last_health_check: Option<Instant>,
    /// Panel resize ratios
    vertical_split: u16, // Left panel width % (default: 50)
    horizontal_split: u16, // Top panels height % (default: 60)
    /// Help overlay
    help_overlay: HelpOverlay,
    /// Whether to show help overlay
    show_help: bool,
    /// Error popup message
    error_popup: Option<String>,
}

impl App {
    /// Create a new application instance
    pub async fn new(rpc_client: Arc<RpcClient>, _config: LayoutConfig) -> Result<Self> {
        let layout_manager = LayoutManager::new();
        let current_panel = PanelType::Trace;

        // Initialize panels without managers (they will receive DataManager as parameter)
        let mut panels: HashMap<PanelType, Panel> = HashMap::new();
        panels.insert(PanelType::Trace, Panel::Trace(TracePanel::new()));
        panels.insert(PanelType::Code, Panel::Code(CodePanel::new()));
        panels.insert(PanelType::Display, Panel::Display(DisplayPanel::new()));
        panels.insert(PanelType::Terminal, Panel::Terminal(TerminalPanel::new()));

        Ok(Self {
            rpc_client,
            layout_manager,
            current_panel,
            panels,
            should_exit: false,
            compact_main_panel: PanelType::Trace, // Default to Trace in compact mode
            full_left_panel: PanelType::Trace,    // Default to Trace in full layout left side
            connection_status: ConnectionStatus::new(),
            last_health_check: None,
            vertical_split: 50,   // 50% left panel width
            horizontal_split: 50, // 50% top panels height
            help_overlay: HelpOverlay::new(),
            show_help: false,
            error_popup: None,
        })
    }

    /// Render the application
    pub fn render(&mut self, frame: &mut Frame<'_>, data_manager: &mut DataManager) {
        // Get terminal size and update layout if needed
        let area = frame.area();
        self.layout_manager.update_size(area.width, area.height);

        match self.layout_manager.layout_type() {
            LayoutType::Full => self.render_full_layout(frame, area, data_manager),
            LayoutType::Compact => self.render_compact_layout(frame, area, data_manager),
            LayoutType::Mobile => self.render_mobile_layout(frame, area, data_manager),
        }

        // Render help overlay if active
        if self.show_help {
            self.help_overlay.render(frame, self.layout_manager.layout_type(), data_manager);
        }

        // Render error popup if active
        if let Some(ref error_msg) = self.error_popup {
            self.render_error_popup(frame, area, error_msg, data_manager);
        }
    }

    /// Update application state
    pub async fn update(&mut self) -> Result<()> {
        // Perform periodic health checks
        self.check_connection_health().await;

        Ok(())
    }

    /// Perform periodic health check on RPC connection
    async fn check_connection_health(&mut self) {
        let now = Instant::now();

        // Check if it's time for a health check (every 5 seconds)
        let should_check = match self.last_health_check {
            None => true, // First check
            Some(last) => now.duration_since(last) >= Duration::from_secs(5),
        };

        if !should_check {
            return;
        }

        self.last_health_check = Some(now);

        // Perform health check
        let start_time = Instant::now();
        match self.rpc_client.health_check().await {
            Ok(_) => {
                let response_time = start_time.elapsed().as_millis() as u64;
                self.connection_status.mark_success(response_time);
                debug!("Health check successful: {}ms", response_time);
            }
            Err(e) => {
                let error_msg = format!("{e}");
                self.connection_status.mark_failure(error_msg.clone());
                warn!("Health check failed: {}", error_msg);
            }
        }
    }

    /// Render the full 3-panel layout (Code/Trace toggle on left, Display and Terminal on right)
    fn render_full_layout(
        &mut self,
        frame: &mut Frame<'_>,
        area: Rect,
        data_manager: &mut DataManager,
    ) {
        // First split for status bar
        let layout_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Status bar
                Constraint::Fill(1),   // Main content
            ])
            .split(area);

        // Render status bar
        self.render_status_bar(frame, layout_chunks[0]);

        // Split main content area horizontally: left (Code/Trace) | right (Display/Terminal)
        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(self.vertical_split), // Left: Code/Trace (toggles)
                Constraint::Percentage(100 - self.vertical_split), // Right: Display + Terminal
            ])
            .split(layout_chunks[1]);

        // Split right side vertically for Display and Terminal
        let right_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(self.horizontal_split), // Display panel
                Constraint::Percentage(100 - self.horizontal_split), // Terminal panel
            ])
            .split(main_chunks[1]);

        // Set focus for panels
        self.update_panel_focus();

        // Render left panel (Code or Trace based on full_left_panel)
        if let Some(panel) = self.panels.get_mut(&self.full_left_panel) {
            panel.render(frame, main_chunks[0], data_manager);
        }

        // Render right panels (Display and Terminal)
        if let Some(panel) = self.panels.get_mut(&PanelType::Display) {
            panel.render(frame, right_chunks[0], data_manager);
        }
        if let Some(panel) = self.panels.get_mut(&PanelType::Terminal) {
            panel.render(frame, right_chunks[1], data_manager);
        }
    }

    /// Render the compact 3-panel stacked layout
    fn render_compact_layout(
        &mut self,
        frame: &mut Frame<'_>,
        area: Rect,
        data_manager: &mut DataManager,
    ) {
        // First split for status bar
        let layout_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Status bar
                Constraint::Fill(1),   // Main content
            ])
            .split(area);

        // Render status bar
        self.render_status_bar(frame, layout_chunks[0]);

        // Split main content for 2-panel layout: Main (cycles Trace/Code/Display) + Terminal (fixed)
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(self.horizontal_split), // Main panel (Trace/Code/Display cycle)
                Constraint::Percentage(100 - self.horizontal_split), // Terminal panel (fixed)
            ])
            .split(layout_chunks[1]);

        // Set focus for panels
        self.update_panel_focus();

        // Render main panel (cycles between Trace/Code/Display)
        if let Some(panel) = self.panels.get_mut(&self.compact_main_panel) {
            panel.render(frame, chunks[0], data_manager);
        }

        // Always render Terminal panel in compact mode
        if let Some(panel) = self.panels.get_mut(&PanelType::Terminal) {
            panel.render(frame, chunks[1], data_manager);
        }
    }

    /// Render the mobile single-panel layout
    fn render_mobile_layout(
        &mut self,
        frame: &mut Frame<'_>,
        area: Rect,
        data_manager: &mut DataManager,
    ) {
        // First split for status bar
        let layout_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Status bar
                Constraint::Fill(1),   // Main content
            ])
            .split(area);

        // Render status bar
        self.render_status_bar(frame, layout_chunks[0]);

        // Set focus for panels
        self.update_panel_focus();

        // Render only the current panel
        if let Some(panel) = self.panels.get_mut(&self.current_panel) {
            panel.render(frame, layout_chunks[1], data_manager);
        }
    }

    /// Render the status bar at the top of the screen
    fn render_status_bar(&mut self, frame: &mut Frame<'_>, area: Rect) {
        use ratatui::{
            style::{Color, Style},
            text::{Line, Span},
            widgets::Paragraph,
        };

        // Update RPC spinner animation
        self.rpc_client.tick();

        // Create status line content
        let status_text = self.connection_status.status_display();
        let server_url = self.rpc_client.server_url();

        // Current panel indicator
        let panel_name = format!("{:?}", self.current_panel);

        // Layout information
        let layout_type = match self.layout_manager.layout_type() {
            LayoutType::Full => "Full",
            LayoutType::Compact => "Compact",
            LayoutType::Mobile => "Mobile",
        };

        // RPC spinner information
        let mut status_spans = vec![Span::styled(
            status_text,
            Style::default().fg(if self.connection_status.connected {
                Color::Green
            } else {
                Color::Yellow
            }),
        )];

        // Add RPC spinner if loading
        if self.rpc_client.is_loading() {
            let spinner_text = self.rpc_client.spinner_display();
            status_spans.push(Span::raw(" | "));
            status_spans.push(Span::styled(spinner_text, Style::default().fg(Color::Cyan)));
        }

        status_spans.extend_from_slice(&[
            Span::raw(" | "),
            Span::styled(format!("Server: {server_url}"), Style::default().fg(Color::Cyan)),
            Span::raw(" | "),
            Span::styled(format!("Panel: {panel_name}"), Style::default().fg(Color::White)),
            Span::raw(" | "),
            Span::styled(format!("Layout: {layout_type}"), Style::default().fg(Color::Gray)),
            Span::raw(" | "),
            Span::styled("Press ? for help", Style::default().fg(Color::Gray)),
        ]);

        let status_line = Line::from(status_spans);

        let status_paragraph =
            Paragraph::new(status_line).style(Style::default().bg(Color::DarkGray));

        frame.render_widget(status_paragraph, area);
    }

    /// Render compact error popup overlay
    fn render_error_popup(
        &self,
        frame: &mut Frame<'_>,
        area: Rect,
        error_msg: &str,
        data_manager: &mut DataManager,
    ) {
        // Calculate available width for popup (leave some margin)
        let max_popup_width = area.width.saturating_sub(4); // 2 chars margin on each side
        let max_popup_height = area.height.saturating_sub(2); // 1 line margin on top and bottom

        // Calculate optimal popup width (minimum 30 chars, maximum 1/3 of available width)
        let min_width = 30;
        let max_width = (max_popup_width / 3).max(min_width); // Use 1/3 of available width
        let content_width = (max_width.saturating_sub(4) as usize).max(min_width as usize); // -4 for borders and padding

        // Split error message into lines and wrap long lines
        let lines: Vec<&str> = error_msg.lines().collect();
        let mut wrapped_lines = Vec::new();

        for line in lines {
            if line.len() <= content_width {
                wrapped_lines.push(line.to_string());
            } else {
                // Break long lines into multiple lines
                let mut remaining = line;
                while !remaining.is_empty() {
                    if remaining.len() <= content_width {
                        wrapped_lines.push(remaining.to_string());
                        break;
                    } else {
                        // Find a good break point (prefer breaking at spaces)
                        let mut break_point = content_width;
                        if let Some(space_pos) = remaining[..content_width].rfind(' ') {
                            if space_pos > content_width / 2 {
                                break_point = space_pos;
                            }
                        }

                        wrapped_lines.push(remaining[..break_point].to_string());
                        remaining = remaining[break_point..].trim_start();
                    }
                }
            }
        }

        // Calculate popup dimensions based on wrapped content
        let popup_width = (content_width + 4).min(max_popup_width as usize) as u16; // +4 for borders and padding
        let popup_height = (wrapped_lines.len() + 4).min(max_popup_height as usize) as u16; // +4 for title, empty line, and instructions

        // Center the popup
        let popup_area = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length((area.height.saturating_sub(popup_height)) / 2),
                Constraint::Length(popup_height),
                Constraint::Min(0),
            ])
            .split(area)[1];

        let popup_area = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length((area.width.saturating_sub(popup_width)) / 2),
                Constraint::Length(popup_width),
                Constraint::Min(0),
            ])
            .split(popup_area)[1];

        // Clear the background
        frame.render_widget(Clear, popup_area);

        // Create error content
        let mut error_lines = vec![
            Line::from(vec![Span::styled(
                "❌ Error",
                Style::default().fg(data_manager.theme.error_color),
            )]),
            Line::from(""),
        ];

        // Add wrapped error message lines
        for line in wrapped_lines {
            error_lines.push(Line::from(line));
        }

        error_lines.push(Line::from(""));
        error_lines.push(Line::from(vec![
            Span::styled("Press ", Style::default().fg(data_manager.theme.comment_color)),
            Span::styled("Esc", Style::default().fg(data_manager.theme.accent_color)),
            Span::styled(", ", Style::default().fg(data_manager.theme.comment_color)),
            Span::styled("Enter", Style::default().fg(data_manager.theme.accent_color)),
            Span::styled(", or ", Style::default().fg(data_manager.theme.comment_color)),
            Span::styled("Space", Style::default().fg(data_manager.theme.accent_color)),
            Span::styled(" to dismiss", Style::default().fg(data_manager.theme.comment_color)),
        ]));

        // Create popup block
        let popup_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(data_manager.theme.error_color))
            .style(Style::default());

        // Create popup paragraph
        let popup_paragraph = Paragraph::new(error_lines)
            .block(popup_block)
            .wrap(Wrap { trim: false })
            .alignment(Alignment::Left);

        frame.render_widget(popup_paragraph, popup_area);
    }

    /// Update panel focus states
    fn update_panel_focus(&mut self) {
        for (panel_type, panel) in &mut self.panels {
            if *panel_type == self.current_panel {
                panel.on_focus();
            } else {
                panel.on_blur();
            }
        }
    }

    /// Handle keyboard events
    pub async fn handle_key_event(
        &mut self,
        key: KeyEvent,
        data_manager: &mut DataManager,
    ) -> Result<EventResponse> {
        // Only handle key press events
        if key.kind != KeyEventKind::Press {
            return Ok(EventResponse::NotHandled);
        }

        debug!("Key pressed: {:?}", key);

        // If error popup is showing, handle error-specific keys
        if self.error_popup.is_some() {
            match key.code {
                KeyCode::Esc | KeyCode::Enter | KeyCode::Char(' ') => {
                    self.error_popup = None;
                    return Ok(EventResponse::Handled);
                }
                _ => return Ok(EventResponse::Handled), // Consume all other keys when error is shown
            }
        }

        // If help is showing, handle help-specific keys
        if self.show_help {
            match key.code {
                KeyCode::Char('?') | KeyCode::Esc => {
                    self.show_help = false;
                    self.help_overlay.reset_scroll();
                    return Ok(EventResponse::Handled);
                }
                KeyCode::Char('j') | KeyCode::Down => {
                    self.help_overlay.scroll_down(1);
                    return Ok(EventResponse::Handled);
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.help_overlay.scroll_up(1);
                    return Ok(EventResponse::Handled);
                }
                KeyCode::PageDown => {
                    self.help_overlay.scroll_down(10);
                    return Ok(EventResponse::Handled);
                }
                KeyCode::PageUp => {
                    self.help_overlay.scroll_up(10);
                    return Ok(EventResponse::Handled);
                }
                _ => return Ok(EventResponse::Handled), // Consume all other keys when help is open
            }
        }

        // First, handle global keys
        match key.code {
            KeyCode::Char('q') | KeyCode::Char('Q')
                if key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.should_exit = true;
                Ok(EventResponse::Exit)
            }
            KeyCode::Char('?') => {
                // Open help overlay with '?'
                self.show_help = true;
                self.help_overlay.reset_scroll();
                Ok(EventResponse::Handled)
            }
            KeyCode::Esc => {
                // ESC: Context-aware navigation
                if self.current_panel == PanelType::Terminal {
                    // If we're in terminal, let the terminal handle ESC (INSERT -> VIM mode)
                    if let Some(panel) = self.panels.get_mut(&PanelType::Terminal) {
                        return match panel.handle_key_event(key, data_manager) {
                            Ok(response) => Ok(response),
                            Err(e) => {
                                self.error_popup = Some(format!("{e}"));
                                Ok(EventResponse::Handled)
                            }
                        };
                    }
                } else {
                    // If we're in other panels, ESC returns to terminal
                    self.current_panel = PanelType::Terminal;
                    return Ok(EventResponse::Handled);
                }
                Ok(EventResponse::NotHandled)
            }
            KeyCode::Char(' ') => {
                match self.layout_manager.layout_type() {
                    LayoutType::Full => {
                        // In full layout, Space toggles between Code and Trace only when focused on the left panel
                        if self.current_panel == PanelType::Code
                            || self.current_panel == PanelType::Trace
                        {
                            self.full_left_panel = match self.full_left_panel {
                                PanelType::Code => PanelType::Trace,
                                PanelType::Trace => PanelType::Code,
                                _ => PanelType::Code, // Fallback (shouldn't happen)
                            };
                            self.current_panel = self.full_left_panel;
                            Ok(EventResponse::Handled)
                        } else {
                            // Forward to current panel if not on Code/Trace
                            if let Some(panel) = self.panels.get_mut(&self.current_panel) {
                                return match panel.handle_key_event(key, data_manager) {
                                    Ok(response) => Ok(response),
                                    Err(e) => {
                                        self.error_popup = Some(format!("{e}"));
                                        Ok(EventResponse::Handled)
                                    }
                                };
                            }
                            Ok(EventResponse::NotHandled)
                        }
                    }
                    LayoutType::Compact => {
                        // In compact mode, Space cycles through Trace → Code → Display only when focused on main panel
                        if self.current_panel != PanelType::Terminal {
                            self.compact_main_panel = match self.compact_main_panel {
                                PanelType::Trace => PanelType::Code,
                                PanelType::Code => PanelType::Display,
                                PanelType::Display => PanelType::Trace,
                                _ => PanelType::Code, // Fallback
                            };
                            self.current_panel = self.compact_main_panel;
                            Ok(EventResponse::Handled)
                        } else {
                            // Forward to terminal if focused on terminal
                            if let Some(panel) = self.panels.get_mut(&PanelType::Terminal) {
                                return match panel.handle_key_event(key, data_manager) {
                                    Ok(response) => Ok(response),
                                    Err(e) => {
                                        self.error_popup = Some(format!("{e}"));
                                        Ok(EventResponse::Handled)
                                    }
                                };
                            }
                            Ok(EventResponse::NotHandled)
                        }
                    }
                    _ => {
                        // In mobile mode, forward to current panel
                        if let Some(panel) = self.panels.get_mut(&self.current_panel) {
                            return match panel.handle_key_event(key, data_manager) {
                                Ok(response) => Ok(response),
                                Err(e) => {
                                    self.error_popup = Some(format!("{e}"));
                                    Ok(EventResponse::Handled)
                                }
                            };
                        }
                        Ok(EventResponse::NotHandled)
                    }
                }
            }
            KeyCode::Tab => {
                self.cycle_panels(false);
                Ok(EventResponse::Handled)
            }
            KeyCode::BackTab => {
                self.cycle_panels(true);
                Ok(EventResponse::Handled)
            }

            // Function keys for mobile layout
            KeyCode::F(1) => {
                if self.layout_manager.layout_type() != LayoutType::Compact {
                    self.compact_main_panel = PanelType::Trace;
                }
                self.current_panel = PanelType::Trace;
                Ok(EventResponse::Handled)
            }
            KeyCode::F(2) => {
                if self.layout_manager.layout_type() != LayoutType::Compact {
                    self.compact_main_panel = PanelType::Code;
                }
                self.current_panel = PanelType::Code;
                Ok(EventResponse::Handled)
            }
            KeyCode::F(3) => {
                if self.layout_manager.layout_type() != LayoutType::Compact {
                    self.compact_main_panel = PanelType::Display;
                }
                self.current_panel = PanelType::Display;
                Ok(EventResponse::Handled)
            }
            KeyCode::F(4) => {
                self.current_panel = PanelType::Terminal;
                Ok(EventResponse::Handled)
            }

            // Global exit shortcuts
            KeyCode::Char('c')
                if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                // First Ctrl+C clears input, second exits (handled by terminal panel)
                if self.current_panel == PanelType::Terminal {
                    // Forward to terminal to handle double-press logic
                    if let Some(panel) = self.panels.get_mut(&PanelType::Terminal) {
                        match panel.handle_key_event(key, data_manager) {
                            Ok(response) => Ok(response),
                            Err(e) => {
                                self.error_popup = Some(format!("{e}"));
                                Ok(EventResponse::Handled)
                            }
                        }
                    } else {
                        Ok(EventResponse::NotHandled)
                    }
                } else {
                    // From other panels, Ctrl+C switches to terminal
                    self.current_panel = PanelType::Terminal;
                    Ok(EventResponse::Handled)
                }
            }
            KeyCode::Char('d')
                if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                // Ctrl+D is immediate exit (EOF signal)
                Ok(EventResponse::Exit)
            }
            KeyCode::Char('q') if key.modifiers.contains(crossterm::event::KeyModifiers::ALT) => {
                // Alt+Q for quick exit
                Ok(EventResponse::Exit)
            }

            // Panel boundary resize with Ctrl+Shift+arrow keys
            KeyCode::Left
                if key.modifiers.contains(
                    crossterm::event::KeyModifiers::CONTROL | crossterm::event::KeyModifiers::SHIFT,
                ) =>
            {
                if self.layout_manager.layout_type() == LayoutType::Full {
                    self.handle_boundary_resize(ResizeDirection::Left);
                }
                Ok(EventResponse::Handled)
            }
            KeyCode::Right
                if key.modifiers.contains(
                    crossterm::event::KeyModifiers::CONTROL | crossterm::event::KeyModifiers::SHIFT,
                ) =>
            {
                if self.layout_manager.layout_type() == LayoutType::Full {
                    self.handle_boundary_resize(ResizeDirection::Right);
                }
                Ok(EventResponse::Handled)
            }
            KeyCode::Up
                if key.modifiers.contains(
                    crossterm::event::KeyModifiers::CONTROL | crossterm::event::KeyModifiers::SHIFT,
                ) =>
            {
                if self.layout_manager.layout_type() != LayoutType::Mobile {
                    self.handle_boundary_resize(ResizeDirection::Up);
                }
                Ok(EventResponse::Handled)
            }
            KeyCode::Down
                if key.modifiers.contains(
                    crossterm::event::KeyModifiers::CONTROL | crossterm::event::KeyModifiers::SHIFT,
                ) =>
            {
                if self.layout_manager.layout_type() != LayoutType::Mobile {
                    self.handle_boundary_resize(ResizeDirection::Down);
                }
                Ok(EventResponse::Handled)
            }

            _ => {
                // Forward to the current panel
                if let Some(panel) = self.panels.get_mut(&self.current_panel) {
                    match panel.handle_key_event(key, data_manager) {
                        Ok(response) => Ok(response),
                        Err(e) => {
                            // Store error for popup display
                            self.error_popup = Some(format!("{e}"));
                            Ok(EventResponse::Handled) // Don't crash, just show error
                        }
                    }
                } else {
                    Ok(EventResponse::NotHandled)
                }
            }
        }
    }

    /// Cycle through panels (Tab key)
    fn cycle_panels(&mut self, reversed: bool) {
        match self.layout_manager.layout_type() {
            LayoutType::Full => {
                // In Full layout, cycle through the 3 visible panels
                // The left panel shows either Code or Trace (based on full_left_panel)
                if !reversed {
                    self.current_panel = match self.current_panel {
                        PanelType::Code | PanelType::Trace => PanelType::Display,
                        PanelType::Display => PanelType::Terminal,
                        PanelType::Terminal => self.full_left_panel,
                    };
                } else {
                    self.current_panel = match self.current_panel {
                        PanelType::Code | PanelType::Trace => PanelType::Terminal,
                        PanelType::Display => self.full_left_panel,
                        PanelType::Terminal => PanelType::Display,
                    };
                }
            }
            LayoutType::Compact => {
                self.current_panel = match self.current_panel {
                    PanelType::Terminal => self.compact_main_panel,
                    _ => PanelType::Terminal,
                };
            }
            LayoutType::Mobile => {
                // Mobile and other layouts cycle through all 4 panels
                if !reversed {
                    self.current_panel = match self.current_panel {
                        PanelType::Trace => PanelType::Code,
                        PanelType::Code => PanelType::Display,
                        PanelType::Display => PanelType::Terminal,
                        PanelType::Terminal => PanelType::Trace,
                    };
                } else {
                    self.current_panel = match self.current_panel {
                        PanelType::Trace => PanelType::Terminal,
                        PanelType::Code => PanelType::Trace,
                        PanelType::Display => PanelType::Code,
                        PanelType::Terminal => PanelType::Display,
                    };
                }
            }
        }
        debug!("Switched to panel: {:?}", self.current_panel);
    }

    /// Handle panel boundary resize with Ctrl+Shift+arrow keys
    pub fn handle_boundary_resize(&mut self, direction: ResizeDirection) {
        const STEP: u16 = 5; // 5% increments

        match direction {
            ResizeDirection::Left => {
                self.vertical_split = self.vertical_split.saturating_sub(STEP).max(20);
            }
            ResizeDirection::Right => {
                self.vertical_split = (self.vertical_split + STEP).min(80);
            }
            ResizeDirection::Up => {
                self.horizontal_split = self.horizontal_split.saturating_sub(STEP).max(30);
            }
            ResizeDirection::Down => {
                self.horizontal_split = (self.horizontal_split + STEP).min(80);
            }
        }

        if self.current_panel == PanelType::Terminal {
            // If currently in terminal, we do not show feedback in terminal
            return;
        }

        // Show intuitive, contextual feedback in terminal
        if let Some(terminal_panel) = self.panels.get_mut(&PanelType::Terminal) {
            if let Some(terminal) = terminal_panel.as_any_mut().downcast_mut::<TerminalPanel>() {
                let message = match direction {
                    ResizeDirection::Left => format!(
                        "Left panels narrowed to {}% (right panels expanded to {}%)",
                        self.vertical_split,
                        100 - self.vertical_split
                    ),
                    ResizeDirection::Right => format!(
                        "Left panels expanded to {}% (right panels narrowed to {}%)",
                        self.vertical_split,
                        100 - self.vertical_split
                    ),
                    ResizeDirection::Up => format!(
                        "Top panels shortened to {}% (bottom panels expanded to {}%)",
                        self.horizontal_split,
                        100 - self.horizontal_split
                    ),
                    ResizeDirection::Down => format!(
                        "Top panels expanded to {}% (bottom panels shortened to {}%)",
                        self.horizontal_split,
                        100 - self.horizontal_split
                    ),
                };
                terminal.add_system(&message);
            }
        }
    }

    /// Handle terminal resize
    pub fn handle_resize(&mut self, width: u16, height: u16) {
        self.layout_manager.update_size(width, height);
        debug!("Terminal resized to {}x{}", width, height);
    }

    /// Handle mouse events
    pub async fn handle_mouse_event(
        &mut self,
        _event: MouseEvent,
        _data_manager: &mut DataManager,
    ) -> Result<()> {
        // Mouse event handling can be implemented here
        Ok(())
    }

    /// Get current panel for external access
    pub fn current_panel(&self) -> PanelType {
        self.current_panel
    }

    /// Check if the app should exit
    pub fn should_exit(&self) -> bool {
        self.should_exit
    }

    /// Get current connection status for display
    pub fn connection_status(&self) -> &ConnectionStatus {
        &self.connection_status
    }

    /// Change focus to a specific panel
    /// Handles layout-specific logic to ensure the panel is visible
    pub fn change_focus(&mut self, target_panel: PanelType) {
        match self.layout_manager.layout_type() {
            LayoutType::Full => {
                // In Full layout, handle Code/Trace visibility
                match target_panel {
                    PanelType::Code | PanelType::Trace => {
                        // Make sure the target panel is visible on the left
                        self.full_left_panel = target_panel;
                        self.current_panel = target_panel;
                    }
                    PanelType::Display | PanelType::Terminal => {
                        // These are always visible in Full layout
                        self.current_panel = target_panel;
                    }
                }
            }
            LayoutType::Compact => {
                // In Compact layout, handle main panel visibility
                match target_panel {
                    PanelType::Terminal => {
                        // Terminal is always visible
                        self.current_panel = PanelType::Terminal;
                    }
                    PanelType::Trace | PanelType::Code | PanelType::Display => {
                        // Switch the main panel to show the target
                        self.compact_main_panel = target_panel;
                        self.current_panel = target_panel;
                    }
                }
            }
            LayoutType::Mobile => {
                // In Mobile layout, just switch to the target panel
                self.current_panel = target_panel;
            }
        }

        // Update panel focus states
        self.update_panel_focus();

        debug!("Changed focus to {:?} panel", target_panel);
    }
}
