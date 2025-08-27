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

use crate::layout::{LayoutConfig, LayoutManager, LayoutType};
use crate::managers::execution::ExecutionManager;
use crate::managers::resolve::Resolver;
use crate::managers::theme::ThemeManager;
use crate::managers::{ExecutionManagerCore, ResolverCore, ThemeManagerCore};
use crate::panels::{
    CodePanel, DisplayPanel, EventResponse, Panel, PanelTr, PanelType, TerminalPanel, TracePanel,
};
use crate::rpc::RpcClient;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent};
use eyre::Result;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    Frame,
};
use std::sync::{Arc, RwLock};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
use tracing::{debug, info, warn};

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
                format!("🟢 Connected ({}ms)", response_time)
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
            format!("{} - {}", base, error)
        } else {
            base
        }
    }
}

/// Main application state
pub struct App {
    /// RPC client for communicating with debug server
    rpc_client: Arc<RpcClient>,
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
    /// Shared execution state manager
    _execution_core: Arc<RwLock<ExecutionManagerCore>>,
    /// Shared resource manager
    _infomatino_core: Arc<RwLock<ResolverCore>>,
    /// Shared theme manager
    _theme_core: Arc<RwLock<ThemeManagerCore>>,
    /// RPC connection status and health monitoring
    connection_status: ConnectionStatus,
    /// Last health check time for periodic monitoring
    last_health_check: Option<Instant>,
    /// Panel resize ratios
    vertical_split: u16, // Left panel width % (default: 50)
    horizontal_split: u16, // Top panels height % (default: 60)
}

impl App {
    /// Create a new application instance
    pub async fn new(rpc_client: Arc<RpcClient>, _config: LayoutConfig) -> Result<Self> {
        let layout_manager = LayoutManager::new();
        let current_panel = PanelType::Terminal;

        // Create managers wrapped in Arc<RwLock<T>>
        let exec_core = Arc::new(RwLock::new(ExecutionManagerCore::new(rpc_client.clone())));
        let resolver_core = Arc::new(RwLock::new(ResolverCore::new(rpc_client.clone())));
        let theme_core = Arc::new(RwLock::new(ThemeManagerCore::new()));

        // Initialize panels - they start with no managers and will get them set later
        let mut panels: HashMap<PanelType, Panel> = HashMap::new();
        panels.insert(
            PanelType::Trace,
            Panel::Trace(TracePanel::new(
                ExecutionManager::new(exec_core.clone()),
                Resolver::new(resolver_core.clone()),
                ThemeManager::new(theme_core.clone()),
            )),
        );
        panels.insert(
            PanelType::Code,
            Panel::Code(CodePanel::new(
                ExecutionManager::new(exec_core.clone()),
                Resolver::new(resolver_core.clone()),
                ThemeManager::new(theme_core.clone()),
            )),
        );
        panels.insert(
            PanelType::Display,
            Panel::Display(DisplayPanel::new(
                ExecutionManager::new(exec_core.clone()),
                Resolver::new(resolver_core.clone()),
                ThemeManager::new(theme_core.clone()),
            )),
        );
        panels.insert(
            PanelType::Terminal,
            Panel::Terminal(TerminalPanel::new(
                ExecutionManager::new(exec_core.clone()),
                Resolver::new(resolver_core.clone()),
                ThemeManager::new(theme_core.clone()),
            )),
        );

        Ok(Self {
            rpc_client,
            layout_manager,
            current_panel,
            panels,
            should_exit: false,
            compact_main_panel: PanelType::Code, // Default to Code in compact mode
            _execution_core: exec_core,
            _infomatino_core: resolver_core,
            _theme_core: theme_core,
            connection_status: ConnectionStatus::new(),
            last_health_check: None,
            vertical_split: 50,   // 50% left panel width
            horizontal_split: 50, // 50% top panels height
        })
    }

    /// Render the application
    pub fn render(&mut self, frame: &mut Frame<'_>) -> Result<()> {
        // Get terminal size and update layout if needed
        let area = frame.area();
        self.layout_manager.update_size(area.width, area.height);

        match self.layout_manager.layout_type() {
            LayoutType::Full => self.render_full_layout(frame, area)?,
            LayoutType::Compact => self.render_compact_layout(frame, area)?,
            LayoutType::Mobile => self.render_mobile_layout(frame, area)?,
        }

        Ok(())
    }

    /// Update application state
    pub async fn update(&mut self) -> Result<()> {
        // Perform periodic health checks
        self.check_connection_health().await;
        // Perform data fetching for each panel
        for panel in self.panels.values_mut() {
            panel.fetch_data().await?;
        }
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
                let error_msg = format!("{}", e);
                self.connection_status.mark_failure(error_msg.clone());
                warn!("Health check failed: {}", error_msg);
            }
        }
    }

    /// Render the full 4-panel layout
    fn render_full_layout(&mut self, frame: &mut Frame<'_>, area: Rect) -> Result<()> {
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

        // Split main content area for 4-panel layout using dynamic ratios
        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(self.horizontal_split), // Top row (Trace | Code)
                Constraint::Percentage(100 - self.horizontal_split), // Bottom row (Display | Terminal)
            ])
            .split(layout_chunks[1]);

        let top_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(self.vertical_split), // Trace panel
                Constraint::Percentage(100 - self.vertical_split), // Code panel
            ])
            .split(main_chunks[0]);

        let bottom_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(self.vertical_split), // Display panel
                Constraint::Percentage(100 - self.vertical_split), // Terminal panel
            ])
            .split(main_chunks[1]);

        // Set focus for panels
        self.update_panel_focus();

        // Render panels
        if let Some(panel) = self.panels.get_mut(&PanelType::Trace) {
            panel.render(frame, top_chunks[0]);
        }
        if let Some(panel) = self.panels.get_mut(&PanelType::Code) {
            panel.render(frame, top_chunks[1]);
        }
        if let Some(panel) = self.panels.get_mut(&PanelType::Display) {
            panel.render(frame, bottom_chunks[0]);
        }
        if let Some(panel) = self.panels.get_mut(&PanelType::Terminal) {
            panel.render(frame, bottom_chunks[1]);
        }

        Ok(())
    }

    /// Render the compact 3-panel stacked layout
    fn render_compact_layout(&mut self, frame: &mut Frame<'_>, area: Rect) -> Result<()> {
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
            panel.render(frame, chunks[0]);
        }

        // Always render Terminal panel in compact mode
        if let Some(panel) = self.panels.get_mut(&PanelType::Terminal) {
            panel.render(frame, chunks[1]);
        }

        Ok(())
    }

    /// Render the mobile single-panel layout
    fn render_mobile_layout(&mut self, frame: &mut Frame<'_>, area: Rect) -> Result<()> {
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
            panel.render(frame, layout_chunks[1]);
        }

        Ok(())
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
            Span::styled(format!("Server: {}", server_url), Style::default().fg(Color::Cyan)),
            Span::raw(" | "),
            Span::styled(format!("Panel: {}", panel_name), Style::default().fg(Color::White)),
            Span::raw(" | "),
            Span::styled(format!("Layout: {}", layout_type), Style::default().fg(Color::Gray)),
        ]);

        let status_line = Line::from(status_spans);

        let status_paragraph =
            Paragraph::new(status_line).style(Style::default().bg(Color::DarkGray));

        frame.render_widget(status_paragraph, area);
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
    pub async fn handle_key_event(&mut self, key: KeyEvent) -> Result<EventResponse> {
        // Only handle key press events
        if key.kind != KeyEventKind::Press {
            return Ok(EventResponse::NotHandled);
        }

        debug!("Key pressed: {:?}", key);

        // First, handle global keys
        match key.code {
            KeyCode::Char('q') | KeyCode::Char('Q')
                if key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.should_exit = true;
                return Ok(EventResponse::Exit);
            }
            KeyCode::Esc => {
                // ESC: Context-aware navigation
                if self.current_panel == PanelType::Terminal {
                    // If we're in terminal, let the terminal handle ESC (INSERT -> VIM mode)
                    if let Some(panel) = self.panels.get_mut(&PanelType::Terminal) {
                        return panel.handle_key_event(key);
                    }
                } else {
                    // If we're in other panels, ESC returns to terminal
                    self.current_panel = PanelType::Terminal;
                    return Ok(EventResponse::Handled);
                }
                return Ok(EventResponse::NotHandled);
            }
            KeyCode::Tab => {
                match self.layout_manager.layout_type() {
                    LayoutType::Compact => {
                        // In compact mode, Tab switches main panel between Trace/Code
                        self.current_panel = match self.current_panel {
                            PanelType::Terminal => self.compact_main_panel,
                            _ => PanelType::Terminal,
                        };
                        return Ok(EventResponse::Handled);
                    }
                    _ => {
                        // In full/mobile mode, Tab cycles through panels
                        self.cycle_panels(false);
                        return Ok(EventResponse::Handled);
                    }
                }
            }
            KeyCode::Char('`') | KeyCode::Char('~') => {
                match self.layout_manager.layout_type() {
                    LayoutType::Compact => {
                        // In compact mode, `/~ switches main panel between Trace/Code
                        self.current_panel = match self.current_panel {
                            PanelType::Terminal => self.compact_main_panel,
                            _ => PanelType::Terminal,
                        };
                        return Ok(EventResponse::Handled);
                    }
                    _ => {
                        // In full/mobile mode, Tab cycles through panels
                        self.cycle_panels(true);
                        return Ok(EventResponse::Handled);
                    }
                }
            }

            // Function keys for mobile layout
            KeyCode::F(1) => {
                if self.layout_manager.layout_type() != LayoutType::Compact {
                    self.compact_main_panel = PanelType::Trace;
                }
                self.current_panel = PanelType::Trace;
                return Ok(EventResponse::Handled);
            }
            KeyCode::F(2) => {
                if self.layout_manager.layout_type() != LayoutType::Compact {
                    self.compact_main_panel = PanelType::Code;
                }
                self.current_panel = PanelType::Code;
                return Ok(EventResponse::Handled);
            }
            KeyCode::F(3) => {
                if self.layout_manager.layout_type() != LayoutType::Compact {
                    self.compact_main_panel = PanelType::Display;
                }
                self.current_panel = PanelType::Display;
                return Ok(EventResponse::Handled);
            }
            KeyCode::F(4) => {
                self.current_panel = PanelType::Terminal;
                return Ok(EventResponse::Handled);
            }

            // Global exit shortcuts
            KeyCode::Char('c')
                if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                // First Ctrl+C clears input, second exits (handled by terminal panel)
                if self.current_panel == PanelType::Terminal {
                    // Forward to terminal to handle double-press logic
                    if let Some(panel) = self.panels.get_mut(&PanelType::Terminal) {
                        return panel.handle_key_event(key);
                    } else {
                        return Ok(EventResponse::NotHandled);
                    }
                } else {
                    // From other panels, Ctrl+C switches to terminal
                    self.current_panel = PanelType::Terminal;
                    return Ok(EventResponse::Handled);
                }
            }
            KeyCode::Char('d')
                if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                // Ctrl+D is immediate exit (EOF signal)
                return Ok(EventResponse::Exit);
            }
            KeyCode::Char('q') if key.modifiers.contains(crossterm::event::KeyModifiers::ALT) => {
                // Alt+Q for quick exit
                return Ok(EventResponse::Exit);
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
                return Ok(EventResponse::Handled);
            }
            KeyCode::Right
                if key.modifiers.contains(
                    crossterm::event::KeyModifiers::CONTROL | crossterm::event::KeyModifiers::SHIFT,
                ) =>
            {
                if self.layout_manager.layout_type() == LayoutType::Full {
                    self.handle_boundary_resize(ResizeDirection::Right);
                }
                return Ok(EventResponse::Handled);
            }
            KeyCode::Up
                if key.modifiers.contains(
                    crossterm::event::KeyModifiers::CONTROL | crossterm::event::KeyModifiers::SHIFT,
                ) =>
            {
                if self.layout_manager.layout_type() != LayoutType::Mobile {
                    self.handle_boundary_resize(ResizeDirection::Up);
                }
                return Ok(EventResponse::Handled);
            }
            KeyCode::Down
                if key.modifiers.contains(
                    crossterm::event::KeyModifiers::CONTROL | crossterm::event::KeyModifiers::SHIFT,
                ) =>
            {
                if self.layout_manager.layout_type() != LayoutType::Mobile {
                    self.handle_boundary_resize(ResizeDirection::Down);
                }
                return Ok(EventResponse::Handled);
            }

            KeyCode::Right => {
                // Right arrow key cycles main panel in compact mode (Trace → Code → Display → Trace)
                if matches!(self.layout_manager.layout_type(), LayoutType::Compact) {
                    self.compact_main_panel = match self.compact_main_panel {
                        PanelType::Trace => PanelType::Code,
                        PanelType::Code => PanelType::Display,
                        PanelType::Display => PanelType::Trace,
                        _ => PanelType::Trace, // Default fallback
                    };
                    debug!("Cycled compact main panel to: {:?}", self.compact_main_panel);
                    if self.current_panel != PanelType::Terminal {
                        self.current_panel = self.compact_main_panel;
                    }
                    return Ok(EventResponse::Handled);
                }
                // Otherwise, forward to current panel
                if let Some(panel) = self.panels.get_mut(&self.current_panel) {
                    return panel.handle_key_event(key);
                }
                return Ok(EventResponse::NotHandled);
            }

            KeyCode::Left => {
                // Left arrow key cycles main panel in compact mode (Trace → Code → Display → Trace)
                if matches!(self.layout_manager.layout_type(), LayoutType::Compact) {
                    self.compact_main_panel = match self.compact_main_panel {
                        PanelType::Trace => PanelType::Display,
                        PanelType::Code => PanelType::Trace,
                        PanelType::Display => PanelType::Code,
                        _ => PanelType::Trace, // Default fallback
                    };
                    debug!("Cycled compact main panel to: {:?}", self.compact_main_panel);
                    if self.current_panel != PanelType::Terminal {
                        self.current_panel = self.compact_main_panel;
                    }
                    return Ok(EventResponse::Handled);
                }
                // Otherwise, forward to current panel
                if let Some(panel) = self.panels.get_mut(&self.current_panel) {
                    return panel.handle_key_event(key);
                }
                return Ok(EventResponse::NotHandled);
            }

            _ => {
                // Forward to the current panel
                if let Some(panel) = self.panels.get_mut(&self.current_panel) {
                    return panel.handle_key_event(key);
                }
                return Ok(EventResponse::NotHandled);
            }
        }
    }

    /// Cycle through panels (Tab key)
    fn cycle_panels(&mut self, reversed: bool) {
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
    pub async fn handle_mouse_event(&mut self, _event: MouseEvent) -> Result<()> {
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
}
