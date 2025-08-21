//! Main application state and logic
//!
//! This module contains the core application state management and event handling.

use crate::layout::{LayoutConfig, LayoutManager, LayoutType};
use crate::panels::{
    BreakpointManager, CodePanel, DisplayPanel, EventResponse, Panel, PanelType, TerminalPanel,
    TracePanel,
};
use crate::rpc::RpcClient;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, MouseEvent};
use eyre::Result;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    Frame,
};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tracing::{debug, warn};

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
    panels: HashMap<PanelType, Box<dyn Panel>>,
    /// Whether the application should exit
    should_exit: bool,
    /// Main panel type for compact layout (Trace or Code)
    compact_main_panel: PanelType,
    /// Shared breakpoint manager
    breakpoint_manager: BreakpointManager,
    /// RPC connection status and health monitoring
    connection_status: ConnectionStatus,
    /// Last health check time for periodic monitoring
    last_health_check: Option<Instant>,
}

impl App {
    /// Create a new application instance
    pub fn new(rpc_client: Arc<RpcClient>, _config: LayoutConfig) -> Result<Self> {
        let layout_manager = LayoutManager::new();
        let current_panel = PanelType::Terminal;
        let breakpoint_manager = BreakpointManager::new();

        // Initialize panels with shared breakpoint manager
        let mut panels: HashMap<PanelType, Box<dyn Panel>> = HashMap::new();
        panels.insert(PanelType::Trace, Box::new(TracePanel::new()));
        panels.insert(
            PanelType::Code,
            Box::new(CodePanel::new_with_breakpoints(Some(breakpoint_manager.clone()))),
        );
        panels.insert(
            PanelType::Display,
            Box::new(DisplayPanel::new_with_breakpoints(Some(breakpoint_manager.clone()))),
        );
        panels.insert(PanelType::Terminal, Box::new(TerminalPanel::new()));

        Ok(Self {
            rpc_client,
            layout_manager,
            current_panel,
            panels,
            should_exit: false,
            compact_main_panel: PanelType::Code, // Default to Code in compact mode
            breakpoint_manager,
            connection_status: ConnectionStatus::new(),
            last_health_check: None,
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

        // Split main content area for 4-panel layout
        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(70), // Top row (Trace | Code)
                Constraint::Percentage(30), // Bottom row (Display | Terminal)
            ])
            .split(layout_chunks[1]);

        let top_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(50), // Trace panel
                Constraint::Percentage(50), // Code panel
            ])
            .split(main_chunks[0]);

        let bottom_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(50), // Display panel
                Constraint::Percentage(50), // Terminal panel
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

        // Split main content for 3-panel layout
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(50), // Main panel (Trace/Code)
                Constraint::Percentage(30), // Display panel
                Constraint::Percentage(20), // Terminal panel
            ])
            .split(layout_chunks[1]);

        // Set focus for panels
        self.update_panel_focus();

        // Render main panel (switch between Trace/Code)
        if let Some(panel) = self.panels.get_mut(&self.compact_main_panel) {
            panel.render(frame, chunks[0]);
        }
        if let Some(panel) = self.panels.get_mut(&PanelType::Display) {
            panel.render(frame, chunks[1]);
        }
        if let Some(panel) = self.panels.get_mut(&PanelType::Terminal) {
            panel.render(frame, chunks[2]);
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

        let status_line = Line::from(vec![
            Span::styled(
                status_text,
                Style::default().fg(if self.connection_status.connected {
                    Color::Green
                } else {
                    Color::Yellow
                }),
            ),
            Span::raw(" | "),
            Span::styled(format!("Server: {}", server_url), Style::default().fg(Color::Cyan)),
            Span::raw(" | "),
            Span::styled(format!("Panel: {}", panel_name), Style::default().fg(Color::White)),
            Span::raw(" | "),
            Span::styled(format!("Layout: {}", layout_type), Style::default().fg(Color::Gray)),
        ]);

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
                if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                self.should_exit = true;
                return Ok(EventResponse::Exit);
            }
            KeyCode::Char('t')
                if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                // Ctrl+T: Switch to terminal from any panel
                self.current_panel = PanelType::Terminal;
                return Ok(EventResponse::Handled);
            }
            KeyCode::Tab => {
                match self.layout_manager.layout_type() {
                    LayoutType::Compact => {
                        // In compact mode, Tab switches main panel between Trace/Code
                        self.compact_main_panel = match self.compact_main_panel {
                            PanelType::Trace => PanelType::Code,
                            PanelType::Code => PanelType::Trace,
                            _ => PanelType::Code, // Fallback
                        };
                        debug!("Switched compact main panel to: {:?}", self.compact_main_panel);
                        return Ok(EventResponse::Handled);
                    }
                    _ => {
                        // In full/mobile mode, Tab cycles through panels
                        self.cycle_panels();
                        return Ok(EventResponse::Handled);
                    }
                }
            }

            // Function keys for mobile layout
            KeyCode::F(1) => {
                self.current_panel = PanelType::Trace;
                return Ok(EventResponse::Handled);
            }
            KeyCode::F(2) => {
                self.current_panel = PanelType::Code;
                return Ok(EventResponse::Handled);
            }
            KeyCode::F(3) => {
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

            KeyCode::Char(' ') => {
                // Space key toggles main panel in compact mode
                if matches!(self.layout_manager.layout_type(), LayoutType::Compact) {
                    self.compact_main_panel = match self.compact_main_panel {
                        PanelType::Trace => PanelType::Code,
                        PanelType::Code => PanelType::Trace,
                        _ => PanelType::Code,
                    };
                    debug!("Toggled compact main panel to: {:?}", self.compact_main_panel);
                    return Ok(EventResponse::Handled);
                }
                // Otherwise, forward to current panel
                if let Some(panel) = self.panels.get_mut(&self.current_panel) {
                    return panel.handle_key_event(key);
                }
                return Ok(EventResponse::NotHandled);
            }

            // Ctrl+number for direct panel access
            KeyCode::Char('1')
                if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                match self.layout_manager.layout_type() {
                    LayoutType::Compact => {
                        // In compact mode, Ctrl+1 focuses main panel (whichever is showing)
                        self.current_panel = self.compact_main_panel;
                    }
                    _ => {
                        self.current_panel = PanelType::Trace;
                    }
                }
                return Ok(EventResponse::Handled);
            }
            KeyCode::Char('2')
                if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                match self.layout_manager.layout_type() {
                    LayoutType::Compact => {
                        // In compact mode, Ctrl+2 focuses display panel
                        self.current_panel = PanelType::Display;
                    }
                    _ => {
                        self.current_panel = PanelType::Code;
                    }
                }
                return Ok(EventResponse::Handled);
            }
            KeyCode::Char('3')
                if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                match self.layout_manager.layout_type() {
                    LayoutType::Compact => {
                        // In compact mode, Ctrl+3 focuses terminal panel
                        self.current_panel = PanelType::Terminal;
                    }
                    _ => {
                        self.current_panel = PanelType::Display;
                    }
                }
                return Ok(EventResponse::Handled);
            }
            KeyCode::Char('4')
                if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                // Ctrl+4 only works in full layout
                if matches!(self.layout_manager.layout_type(), LayoutType::Full) {
                    self.current_panel = PanelType::Terminal;
                }
                return Ok(EventResponse::Handled);
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
    fn cycle_panels(&mut self) {
        self.current_panel = match self.current_panel {
            PanelType::Trace => PanelType::Code,
            PanelType::Code => PanelType::Display,
            PanelType::Display => PanelType::Terminal,
            PanelType::Terminal => PanelType::Trace,
        };
        debug!("Switched to panel: {:?}", self.current_panel);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_app_creation() {
        // This would need a running RPC server to test properly
        // For now, just test that the module compiles
    }
}
