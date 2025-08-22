//! Terminal panel for command input and output
//!
//! This panel provides a command-line interface for debugging commands.

use super::{EventResponse, PanelTr, PanelType};
use crate::managers::{ExecutionManager, ResourceManager, ThemeManager};
use crate::ui::borders::BorderPresets;
use crate::ui::colors::ColorScheme;
use crate::ui::icons::Icons;
use crate::ui::status::{ConnectionStatus, ExecutionStatus, StatusBar};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use eyre::Result;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};
use std::collections::VecDeque;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::time::Instant;
use tracing::debug;

/// Maximum number of terminal lines to keep in history
const MAX_TERMINAL_LINES: usize = 1000;

/// Maximum number of command history entries
const MAX_COMMAND_HISTORY: usize = 100;

/// Type of terminal line
#[derive(Debug, Clone)]
pub enum LineType {
    /// User command (prefixed with ">")
    Command,
    /// Command output
    Output,
    /// Error message (red color)
    Error,
    /// System message (blue color)
    System,
}

/// Terminal line with content and type
#[derive(Debug, Clone)]
pub struct TerminalLine {
    /// Line content
    pub content: String,
    /// Type of line for styling
    pub line_type: LineType,
}

/// Terminal interaction mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalMode {
    /// Normal typing mode (default)
    Insert,
    /// Navigation mode for scrolling (vim-style)
    Vim,
}

/// Terminal panel implementation with vim-style navigation
#[derive(Debug)]
pub struct TerminalPanel {
    /// All terminal content (commands + output intermixed)
    lines: Vec<TerminalLine>,
    /// Current interaction mode
    mode: TerminalMode,
    /// Current input buffer (only active in INSERT mode)
    input_buffer: String,
    /// Cursor position in input buffer
    cursor_position: usize,
    /// Command history for ↑/↓ navigation in INSERT mode
    command_history: VecDeque<String>,
    /// Current position in command history (None = no history browsing)
    history_position: Option<usize>,
    /// Scroll position in terminal history (0 = bottom/latest)
    scroll_offset: usize,
    /// Content height for the current area
    content_height: usize,
    /// Whether this panel is focused
    focused: bool,
    /// Color scheme for styling
    color_scheme: ColorScheme,
    /// Whether we're connected to the RPC server
    connected: bool,
    /// Current snapshot info (current_index, total_count)
    snapshot_info: Option<(usize, usize)>,
    /// Last Ctrl+C press time for double-press detection
    last_ctrl_c: Option<Instant>,
    /// Number prefix for vim navigation (e.g., "5" in "5j")
    vim_number_prefix: String,
    /// VIM mode cursor absolute line number in terminal history (1-based, like code panel)
    vim_cursor_line: usize,
    /// Shared execution state manager
    execution_manager: Arc<RwLock<ExecutionManager>>,
    /// Shared resource manager
    resource_manager: Arc<RwLock<ResourceManager>>,
    /// Theme manager for centralized theme management
    theme_manager: Arc<RwLock<ThemeManager>>,
}

impl TerminalPanel {
    /// Create a new terminal panel
    pub fn new(
        execution_manager: Arc<RwLock<ExecutionManager>>,
        resource_manager: Arc<RwLock<ResourceManager>>,
        theme_manager: Arc<RwLock<ThemeManager>>,
    ) -> Self {
        let mut panel = Self {
            lines: Vec::new(),
            mode: TerminalMode::Insert,
            input_buffer: String::new(),
            cursor_position: 0,
            command_history: VecDeque::new(),
            history_position: None,
            scroll_offset: 0,
            content_height: 0,
            focused: false,
            color_scheme: ColorScheme::default(),
            connected: true,
            snapshot_info: Some((127, 348)),
            last_ctrl_c: None,
            vim_number_prefix: String::new(),
            vim_cursor_line: 1, // Start at first line (1-based like code panel)
            execution_manager,
            resource_manager,
            theme_manager,
        };

        // Add welcome message with fancy styling
        panel.add_output(&format!("{} EDB Time-Travel Debugger v1.0", Icons::TARGET_REACHED));
        panel.add_output(&format!("{} Connected to RPC server", Icons::CONNECTED));
        panel.add_output(&format!("{} Type 'help' for available commands", Icons::INFO));
        panel.add_output("");

        panel
    }

    /// Add a line to the terminal with specified type
    pub fn add_line(&mut self, content: &str, line_type: LineType) {
        if self.lines.len() >= MAX_TERMINAL_LINES {
            self.lines.remove(0);
        }
        self.lines.push(TerminalLine { content: content.to_string(), line_type });
    }

    /// Add output line (convenience method)
    pub fn add_output(&mut self, line: &str) {
        self.add_line(line, LineType::Output);
    }

    /// Add error line (convenience method)
    pub fn add_error(&mut self, line: &str) {
        self.add_line(line, LineType::Error);
    }

    /// Add system message (convenience method)
    pub fn add_system(&mut self, line: &str) {
        self.add_line(&format!("⚡ {}", line), LineType::System);
    }

    /// Add command line (convenience method)
    pub fn add_command(&mut self, command: &str) {
        self.add_line(&format!("> {}", command), LineType::Command);
    }

    /// Execute a command
    fn execute_command(&mut self, command: &str) -> Result<EventResponse> {
        debug!("Executing command: {}", command);

        // Add command to history
        if !command.trim().is_empty()
            && !self.command_history.back().map_or(false, |last| last == command)
        {
            if self.command_history.len() >= MAX_COMMAND_HISTORY {
                self.command_history.pop_front();
            }
            self.command_history.push_back(command.to_string());
        }

        // Add command to terminal history
        self.add_command(command);

        // Handle built-in commands
        match command.trim() {
            "" => {}
            "quit" | "q" | "exit" => {
                self.add_system("🚪 Exiting debugger...");
                return Ok(EventResponse::Exit);
            }
            "help" | "h" => {
                self.show_help();
            }
            "clear" | "cls" => {
                self.lines.clear();
                self.add_system("Terminal cleared");
            }
            "history" => {
                self.show_history();
            }
            "theme" => {
                self.show_themes();
            }
            cmd if cmd.starts_with("theme ") => {
                self.handle_theme_command(&cmd[6..]);
            }
            cmd if cmd.starts_with('$') => {
                // Solidity expression evaluation
                self.add_output(&format!("Evaluating: {}", &cmd[1..]));
                self.add_output("Expression evaluation not yet implemented");
            }
            cmd => {
                // Debug commands
                if let Err(e) = self.handle_debug_command(cmd) {
                    let error_msg = format!("Error: {}", e);
                    self.add_output(&error_msg);
                }
            }
        }

        Ok(EventResponse::Handled)
    }

    /// Handle debug commands
    fn handle_debug_command(&mut self, command: &str) -> Result<()> {
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            return Ok(());
        }

        match parts[0] {
            "next" | "n" => {
                self.add_system("Stepping to next snapshot...");
                let current = self.exec_mgr_mut().current_snapshot();
                let total = self.exec_mgr_mut().total_snapshots();
                if current < total.saturating_sub(1) {
                    self.exec_mgr_mut().update_state(current + 1, total, Some(current + 10), None);
                    self.add_output(&format!("✅ Stepped to snapshot {}/{}", current + 1, total));
                } else {
                    self.add_output("⚠️ Already at last snapshot");
                }
            }
            "prev" | "p" => {
                self.add_system("Stepping to previous snapshot...");
                let current = self.exec_mgr_mut().current_snapshot();
                let total = self.exec_mgr_mut().total_snapshots();
                if current > 0 {
                    self.exec_mgr_mut().update_state(current - 1, total, Some(current + 8), None);
                    self.add_output(&format!("✅ Stepped to snapshot {}/{}", current - 1, total));
                } else {
                    self.add_output("⚠️ Already at first snapshot");
                }
            }
            "step" | "s" => {
                let count =
                    if parts.len() > 1 { parts[1].parse::<usize>().unwrap_or(1) } else { 1 };
                self.add_output(&format!("Stepping {} snapshots...", count));
                let current = self.exec_mgr_mut().current_snapshot();
                let total = self.exec_mgr_mut().total_snapshots();
                let new_pos = (current + count).min(total.saturating_sub(1));
                self.exec_mgr_mut().update_state(new_pos, total, Some(new_pos + 9), None);
                self.add_output(&format!(
                    "✅ Stepped {} snapshots to {}/{}",
                    count, new_pos, total
                ));
            }
            "reverse" | "r" => {
                let count =
                    if parts.len() > 1 { parts[1].parse::<usize>().unwrap_or(1) } else { 1 };
                self.add_output(&format!("Reverse stepping {} snapshots...", count));
                let current = self.exec_mgr_mut().current_snapshot();
                let total = self.exec_mgr_mut().total_snapshots();
                let new_pos = current.saturating_sub(count);
                self.exec_mgr_mut().update_state(new_pos, total, Some(new_pos + 9), None);
                self.add_output(&format!(
                    "✅ Reverse stepped {} snapshots to {}/{}",
                    count, new_pos, total
                ));
            }
            "call" | "c" => {
                self.add_system("Stepping to next function call...");
                let current = self.exec_mgr_mut().current_snapshot();
                let total = self.exec_mgr_mut().total_snapshots();
                // Simulate jumping to next significant call (larger step)
                let new_pos = (current + 10).min(total.saturating_sub(1));
                self.exec_mgr_mut().update_state(new_pos, total, Some(new_pos + 9), None);
                self.add_output(&format!(
                    "✅ Stepped to next call at snapshot {}/{}",
                    new_pos, total
                ));
            }
            "rcall" | "rc" => {
                self.add_system("Stepping back from function call...");
                let current = self.exec_mgr_mut().current_snapshot();
                let total = self.exec_mgr_mut().total_snapshots();
                // Simulate jumping back to previous significant call
                let new_pos = current.saturating_sub(10);
                self.exec_mgr_mut().update_state(new_pos, total, Some(new_pos + 9), None);
                self.add_output(&format!(
                    "✅ Stepped back to previous call at snapshot {}/{}",
                    new_pos, total
                ));
            }
            "goto" => {
                if parts.len() > 1 {
                    if let Ok(index) = parts[1].parse::<usize>() {
                        self.add_output(&format!("Jumping to snapshot {}...", index));
                        let total = self.exec_mgr_mut().total_snapshots();
                        if index < total {
                            self.exec_mgr_mut().update_state(index, total, Some(index + 9), None);
                            self.add_output(&format!("✅ Jumped to snapshot {}/{}", index, total));
                        } else {
                            self.add_output(&format!(
                                "⚠️ Invalid snapshot index. Range: 0-{}",
                                total.saturating_sub(1)
                            ));
                        }
                    } else {
                        self.add_output("Invalid snapshot index");
                    }
                } else {
                    self.add_output("Usage: goto <index>");
                }
            }
            "break" => {
                if parts.len() > 1 {
                    self.add_output(&format!("Setting breakpoint at: {}", parts[1]));
                    self.add_output("(Breakpoint functionality not yet implemented)");
                } else {
                    self.add_output("Usage: break <location>");
                }
            }
            "stack" => {
                self.add_output("Displaying stack...");
                self.add_output("(Stack display would switch display panel)");
            }
            "memory" => {
                self.add_output("Displaying memory...");
                self.add_output("(Memory display would switch display panel)");
            }
            "variables" | "vars" => {
                self.add_output("Displaying variables...");
                self.add_output("(Variable display would switch display panel)");
            }
            "reset" => {
                self.add_output("Resetting display panel to default view");
            }
            _ => {
                self.add_output(&format!("Unknown command: {}", parts[0]));
                self.add_output("Type 'help' for available commands");
            }
        }

        Ok(())
    }

    /// Show help information
    fn show_help(&mut self) {
        self.add_output("📋 EDB Terminal Help");
        self.add_output("");
        self.add_output("🔄 Terminal Navigation (Vim-Style):");
        self.add_output("  Esc              - Switch to VIM mode for navigation");
        self.add_output("  (VIM mode) j/k/↑/↓ - Navigate lines (with auto-scroll)");
        self.add_output("  (VIM mode) 5j/3↓   - Move multiple lines with number prefix");
        self.add_output("  (VIM mode) gg/G    - Go to top/bottom");
        self.add_output("  (VIM mode) i       - Return to INSERT mode");
        self.add_output("");
        self.add_output("🧭 Panel Navigation:");
        self.add_output(
            "  ESC              - Switch to terminal from any panel (or VIM mode in terminal)",
        );
        self.add_output("  Tab              - Cycle through panels");
        self.add_output("  F1/F2/F3/F4      - Direct panel access (mobile layout)");
        self.add_output("");
        self.add_output("🚀 Debug Commands:");
        self.add_output("  next, n          - Step to next snapshot");
        self.add_output("  prev, p          - Step to previous snapshot");
        self.add_output("  step, s <count>  - Step multiple snapshots");
        self.add_output("  reverse, r <count> - Reverse step multiple snapshots");
        self.add_output("  call, c          - Step to next function call");
        self.add_output("  rcall, rc        - Step back from function call");
        self.add_output("  goto <index>     - Jump to specific snapshot");
        self.add_output("");
        self.add_output("🔍 Inspection:");
        self.add_output("  stack            - Show current stack");
        self.add_output("  memory [offset]  - Show memory");
        self.add_output("  variables, vars  - Show variables in scope");
        self.add_output("  reset            - Reset display panel");
        self.add_output("");
        self.add_output("🔴 Breakpoints:");
        self.add_output("  break <location> - Set breakpoint");
        self.add_output("");
        self.add_output("💻 Solidity expressions (prefix with $):");
        self.add_output("  $balance         - Evaluate variable");
        self.add_output("  $msg.sender      - Evaluate expression");
        self.add_output("");
        self.add_output("⚙️  Other:");
        self.add_output("  help, h          - Show this help");
        self.add_output("  clear, cls       - Clear terminal");
        self.add_output("  theme            - Switch theme");
        self.add_output("  history          - Show command history");
        self.add_output("  quit, q, exit    - Exit debugger");
        self.add_output("");
    }

    /// Show command history
    fn show_history(&mut self) {
        if self.command_history.is_empty() {
            self.add_output("No command history");
        } else {
            self.add_output("Command history:");
            let history_lines: Vec<String> = self
                .command_history
                .iter()
                .enumerate()
                .map(|(i, cmd)| format!("  {}: {}", i + 1, cmd))
                .collect();
            for line in history_lines {
                self.add_output(&line);
            }
        }
    }

    /// Show available themes
    fn show_themes(&mut self) {
        let themes = self.theme_mgr().list_themes();
        let active_theme = self.theme_mgr().get_active_theme_name();

        self.add_output("Available themes:");
        for (name, display_name, description) in themes {
            let marker = if name == active_theme { "→" } else { " " };
            self.add_output(&format!("{} {} - {}", marker, display_name, description));
        }
        self.add_output("");
        self.add_output("Usage:");
        self.add_output("  theme <name>    Switch to theme");
        self.add_output("  theme           List available themes");
    }

    /// Handle theme switching command
    fn handle_theme_command(&mut self, theme_name: &str) {
        let theme_name = theme_name.to_lowercase();
        let theme_name = theme_name.trim();

        if theme_name.is_empty() {
            self.show_themes();
            return;
        }

        let theme = self.theme_mgr_mut().switch_theme(theme_name);
        match theme {
            Ok(_) => {
                self.add_system(&format!(
                    "Switched to '{}' theme - changes will apply immediately",
                    theme_name
                ));
                // Theme changes take effect immediately via the shared ThemeManager
            }
            Err(e) => {
                self.add_error(&format!("Failed to switch theme: {}", e));
                self.add_output("Available themes:");
                let themes = self.theme_mgr().list_themes();
                for (_name, display_name, description) in themes {
                    self.add_output(&format!("  {} - {}", display_name, description));
                }
            }
        }
    }

    /// Handle cursor movement
    fn move_cursor_left(&mut self) {
        if self.cursor_position > 0 {
            self.cursor_position -= 1;
        }
    }

    fn move_cursor_right(&mut self) {
        if self.cursor_position < self.input_buffer.len() {
            self.cursor_position += 1;
        }
    }

    /// Navigate command history
    fn history_up(&mut self) {
        if self.command_history.is_empty() {
            return;
        }

        match self.history_position {
            None => {
                // Start browsing history from the end
                self.history_position = Some(self.command_history.len() - 1);
            }
            Some(pos) if pos > 0 => {
                self.history_position = Some(pos - 1);
            }
            _ => return, // Already at the beginning
        }

        if let Some(pos) = self.history_position {
            if let Some(cmd) = self.command_history.get(pos) {
                self.input_buffer = cmd.clone();
                self.cursor_position = self.input_buffer.len();
            }
        }
    }

    fn history_down(&mut self) {
        match self.history_position {
            Some(pos) if pos < self.command_history.len() - 1 => {
                self.history_position = Some(pos + 1);
                if let Some(cmd) = self.command_history.get(pos + 1) {
                    self.input_buffer = cmd.clone();
                    self.cursor_position = self.input_buffer.len();
                }
            }
            Some(_) => {
                // Go back to current input
                self.history_position = None;
                self.input_buffer.clear();
                self.cursor_position = 0;
            }
            None => {} // Not browsing history
        }
    }

    /// Render enhanced status line with comprehensive status information    
    fn render_status_line(&self, frame: &mut Frame<'_>, area: Rect) {
        // Build comprehensive status using StatusBar
        let connection_status = if self.connected {
            ConnectionStatus::Connected
        } else {
            ConnectionStatus::Disconnected
        };

        let execution_status = if let Some((current, total)) = self.snapshot_info {
            if current == 0 {
                ExecutionStatus::Start
            } else if current >= total.saturating_sub(1) {
                ExecutionStatus::End
            } else {
                ExecutionStatus::Running
            }
        } else {
            ExecutionStatus::Start
        };

        let mut status_bar = StatusBar::new()
            .connection(connection_status)
            .execution(execution_status)
            .current_panel("Terminal".to_string());

        // Add snapshot info if available
        if let Some((current, total)) = self.snapshot_info {
            status_bar = status_bar.message(format!("Snapshot {}/{}", current + 1, total + 1));
        }

        // Add gas info
        status_bar = status_bar.message("Gas: 2,847,293".to_string());

        let status_text = status_bar.build();

        let status_paragraph = Paragraph::new(Line::from(vec![Span::styled(
            status_text,
            Style::default().fg(self.theme_mgr().info_color()),
        )]))
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(self.theme_mgr().unfocused_border_color())),
        );

        frame.render_widget(status_paragraph, area);
    }

    /// Handle keys in INSERT mode (normal typing)
    fn handle_insert_mode_key(&mut self, event: KeyEvent) -> Result<EventResponse> {
        match event.code {
            KeyCode::Enter => {
                let command = self.input_buffer.clone();
                self.input_buffer.clear();
                self.cursor_position = 0;
                self.history_position = None;
                let response = self.execute_command(&command)?;
                Ok(response)
            }
            KeyCode::Backspace => {
                if self.cursor_position > 0 {
                    self.input_buffer.remove(self.cursor_position - 1);
                    self.cursor_position -= 1;
                    self.history_position = None;
                }
                Ok(EventResponse::Handled)
            }
            KeyCode::Delete => {
                if self.cursor_position < self.input_buffer.len() {
                    self.input_buffer.remove(self.cursor_position);
                    self.history_position = None;
                }
                Ok(EventResponse::Handled)
            }
            KeyCode::Left => {
                self.move_cursor_left();
                Ok(EventResponse::Handled)
            }
            KeyCode::Right => {
                self.move_cursor_right();
                Ok(EventResponse::Handled)
            }
            KeyCode::Up => {
                self.history_up();
                Ok(EventResponse::Handled)
            }
            KeyCode::Down => {
                self.history_down();
                Ok(EventResponse::Handled)
            }
            KeyCode::Home => {
                self.cursor_position = 0;
                Ok(EventResponse::Handled)
            }
            KeyCode::End => {
                self.cursor_position = self.input_buffer.len();
                Ok(EventResponse::Handled)
            }
            KeyCode::Char(c) => {
                self.input_buffer.insert(self.cursor_position, c);
                self.cursor_position += 1;
                self.history_position = None;
                // Reset scroll to bottom when typing
                self.scroll_offset = 0;
                Ok(EventResponse::Handled)
            }
            _ => Ok(EventResponse::NotHandled),
        }
    }

    /// Handle keys in VIM mode (navigation)
    fn handle_vim_mode_key(&mut self, event: KeyEvent) -> Result<EventResponse> {
        match event.code {
            // Return to INSERT mode
            KeyCode::Char('i') | KeyCode::Enter => {
                // Update cursor position to bottom
                self.vim_goto_bottom();
                self.vim_number_prefix.clear();
                self.mode = TerminalMode::Insert;
                Ok(EventResponse::Handled)
            }

            // Vim navigation commands - both j/k and arrow keys
            KeyCode::Char('j') | KeyCode::Down => {
                self.vim_move_cursor_down(1);
                Ok(EventResponse::Handled)
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.vim_move_cursor_up(1);
                Ok(EventResponse::Handled)
            }

            // Handle number prefixes for multi-line scrolling
            KeyCode::Char(c) if c.is_ascii_digit() => {
                self.vim_number_prefix.push(c);
                Ok(EventResponse::Handled)
            }

            // Go to top/bottom
            KeyCode::Char('g') => {
                // Handle 'gg' sequence
                if self.vim_number_prefix == "g" {
                    self.vim_goto_top();
                    self.vim_number_prefix.clear();
                } else {
                    self.vim_number_prefix = String::from("g");
                }
                Ok(EventResponse::Handled)
            }
            KeyCode::Char('G') => {
                self.vim_goto_bottom();
                self.vim_number_prefix.clear();
                Ok(EventResponse::Handled)
            }

            _ => {
                // Clear number prefix on unrecognized key
                self.vim_number_prefix.clear();
                Ok(EventResponse::NotHandled)
            }
        }
    }

    /// Move VIM cursor up (k key) with auto-scrolling (exactly like code panel)
    fn vim_move_cursor_up(&mut self, count: usize) {
        let multiplier = if self.vim_number_prefix.is_empty() {
            count
        } else {
            self.vim_number_prefix.parse::<usize>().unwrap_or(1) * count
        };

        // Move cursor up in absolute terminal history (1-based like code panel)
        if self.vim_cursor_line > multiplier {
            self.vim_cursor_line -= multiplier;
        } else {
            self.vim_cursor_line = 1; // Can't go above first line
        }

        // Auto-scroll if cursor moves out of view (EXACTLY like code panel logic)
        if self.vim_cursor_line < self.scroll_offset + 1 {
            self.scroll_offset = (self.vim_cursor_line - 1).max(0);
        }

        self.vim_number_prefix.clear();
    }

    /// Move VIM cursor down (j key) with auto-scrolling (exactly like code panel)
    fn vim_move_cursor_down(&mut self, count: usize) {
        let multiplier = if self.vim_number_prefix.is_empty() {
            count
        } else {
            self.vim_number_prefix.parse::<usize>().unwrap_or(1) * count
        };

        let max_lines = self.lines.len();
        if max_lines == 0 {
            self.vim_number_prefix.clear();
            return;
        }

        // Move cursor down in absolute terminal history (considering input buffer)
        let new_line = (self.vim_cursor_line + multiplier).min(max_lines + 1);
        self.vim_cursor_line = new_line;

        // Auto-scroll if cursor moves out of view
        let viewport_height = self.content_height;
        if self.vim_cursor_line > self.scroll_offset + viewport_height {
            self.scroll_offset = self.vim_cursor_line.saturating_sub(viewport_height);
        }

        self.vim_number_prefix.clear();
    }

    /// Go to top of terminal
    fn vim_goto_top(&mut self) {
        self.vim_cursor_line = 1; // First line (1-based)
        self.scroll_offset = 0; // Show from beginning (like code panel)
    }

    /// Go to bottom of terminal
    fn vim_goto_bottom(&mut self) {
        if !self.lines.is_empty() {
            self.vim_cursor_line = self.lines.len() + 1; // Include prompt line

            let max_lines = self.lines.len() + 1;

            if max_lines <= self.content_height {
                self.scroll_offset = 0; // No scroll needed if content fits
            } else {
                // Scroll to show last line at bottom
                // This is like code panel logic where we show the last lines
                self.scroll_offset = max_lines.saturating_sub(self.content_height);
            }
        }
    }

    /// Render the unified bash-like terminal view
    fn render_unified_terminal(&mut self, frame: &mut Frame<'_>, area: Rect, _border_color: Color) {
        // Start with all terminal history
        let mut all_content = self.lines.clone();

        // Add current input line only in both INSERT and VIM modes
        let prompt = format!(
            "{} edb{} {}",
            if self.connected { Icons::CONNECTED } else { Icons::DISCONNECTED },
            Icons::ARROW_RIGHT,
            if self.focused && self.mode == TerminalMode::Insert {
                // Show line cursor in INSERT mode
                let chars: Vec<char> = self.input_buffer.chars().collect();
                if self.cursor_position == chars.len() {
                    format!("{}│", self.input_buffer)
                } else if self.cursor_position < chars.len() {
                    let before: String = chars.iter().take(self.cursor_position).collect();
                    let after: String = chars.iter().skip(self.cursor_position).collect();
                    format!("{}│{}", before, after)
                } else {
                    self.input_buffer.clone()
                }
            } else {
                self.input_buffer.clone()
            }
        );
        all_content.push(TerminalLine { content: prompt, line_type: LineType::Command });

        // Calculate visible area (leave space for status and help text if needed)
        let status_help_height = if self.focused && area.height > 10 { 2 } else { 0 };
        self.content_height = area.height.saturating_sub(2 + status_help_height) as usize; // Account for borders + status/help

        // In INSERT mode, always stay at bottom to show most recent content
        // In VIM mode, respect user's scroll position
        let total_content = all_content.len();
        let (start_idx, end_idx) = if self.mode == TerminalMode::Insert {
            // INSERT mode: always show the most recent content (bottom)
            if total_content <= self.content_height {
                (0, total_content)
            } else {
                let start = total_content - self.content_height;
                (start, total_content)
            }
        } else {
            // VIM mode: respect scroll_offset (like code panel)
            let start = self.scroll_offset;
            let end = (start + self.content_height).min(total_content);
            (start, end)
        };

        // Create unified terminal content items with VIM cursor support
        let terminal_items: Vec<ListItem<'_>> = all_content
            .iter()
            .skip(start_idx)
            .take(self.content_height)
            .enumerate()
            .map(|(display_row, terminal_line)| {
                let base_style = match terminal_line.line_type {
                    LineType::Command => Style::default().fg(self.theme_mgr().info_color()),
                    LineType::Output => Style::default(),
                    LineType::Error => Style::default().fg(self.theme_mgr().error_color()),
                    LineType::System => Style::default().fg(self.theme_mgr().success_color()),
                };

                // Create the line content with base style
                let styled_line = Line::from(Span::styled(&terminal_line.content, base_style));

                // Apply full-width highlighting to the ListItem if this is the current VIM cursor line
                let list_item = ListItem::new(styled_line);

                // Convert absolute vim_cursor_line to display row (EXACTLY like code panel)
                let cursor_display_row =
                    if self.vim_cursor_line > 0 && self.mode == TerminalMode::Vim {
                        // Convert to 0-based
                        let cursor_absolute_idx = self.vim_cursor_line - 1;
                        // Check if cursor is within visible range
                        if cursor_absolute_idx >= start_idx && cursor_absolute_idx < end_idx {
                            Some(cursor_absolute_idx - start_idx) // Display row within visible content
                        } else {
                            None // Cursor not in visible area
                        }
                    } else {
                        None
                    };

                if self.mode == TerminalMode::Vim
                    && self.focused
                    && Some(display_row) == cursor_display_row
                {
                    // Apply highlighting to entire ListItem (full width like code panel)
                    list_item.style(
                        Style::default()
                            .bg(self.theme_mgr().highlight_bg_color())
                            .fg(self.theme_mgr().highlight_fg_color()),
                    )
                } else {
                    list_item
                }
            })
            .collect();

        // Show scroll indicator and mode in title (only show scroll indicator in VIM mode)
        let scroll_indicator = if self.mode == TerminalMode::Vim && self.scroll_offset > 0 {
            format!(" [↑{}]", self.scroll_offset)
        } else {
            String::new()
        };

        let mode_indicator = match self.mode {
            TerminalMode::Insert => "INSERT",
            TerminalMode::Vim => "VIM",
        };

        let terminal_title = format!(
            "{} Debug Terminal{} [{}]",
            Icons::PROCESSING,
            scroll_indicator,
            mode_indicator
        );

        let terminal_block = BorderPresets::terminal(
            self.focused,
            terminal_title,
            self.theme_mgr().focused_border_color(),
            self.theme_mgr().unfocused_border_color(),
        );

        // Use the full area since status/help are handled separately
        let main_area = area;

        let terminal_list = List::new(terminal_items).block(terminal_block);
        frame.render_widget(terminal_list, main_area);

        // Add status and help text inside the terminal border like other panels
        if self.focused && area.height > 10 {
            // Status line
            let status_area = Rect {
                x: area.x + 1,
                y: area.y + area.height - 3,
                width: area.width - 2,
                height: 1,
            };

            let status_bar = StatusBar::new()
                .current_panel("Terminal".to_string())
                .message(format!("Mode: {:?}", self.mode));

            let status_text = status_bar.build();
            let status_paragraph = Paragraph::new(status_text)
                .style(Style::default().fg(self.theme_mgr().accent_color()));
            frame.render_widget(status_paragraph, status_area);

            // Help text
            let help_area = Rect {
                x: area.x + 1,
                y: area.y + area.height - 2,
                width: area.width - 2,
                height: 1,
            };

            let help_text = match self.mode {
                TerminalMode::Insert => "INSERT mode: Type commands • ↑/↓: History • Esc: VIM mode",
                TerminalMode::Vim => "VIM mode: j/k/↑/↓: Navigate • gg/G: Top/Bottom • {/}: Commands • i/Enter: INSERT"
            };

            let help_paragraph = Paragraph::new(help_text)
                .style(Style::default().fg(self.theme_mgr().help_text_color()));
            frame.render_widget(help_paragraph, help_area);
        }
    }
}

impl PanelTr for TerminalPanel {
    fn panel_type(&self) -> PanelType {
        PanelType::Terminal
    }

    fn title(&self) -> String {
        let status = if let Some((current, total)) = self.snapshot_info {
            format!(" [{}/{}]", current, total)
        } else {
            String::new()
        };

        let mode_info = match self.mode {
            TerminalMode::Insert => " - INSERT mode",
            TerminalMode::Vim => " - VIM mode",
        };

        format!("{} Debug Terminal{}{}", Icons::PROCESSING, status, mode_info)
    }

    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let border_color = if self.focused {
            self.theme_mgr().focused_border_color()
        } else {
            self.theme_mgr().unfocused_border_color()
        };

        // Add status line if there's enough space
        let (main_area, status_area) = if area.height > 8 {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1), // Status line
                    Constraint::Min(7),    // Main content
                ])
                .split(area);
            (chunks[1], Some(chunks[0]))
        } else {
            (area, None)
        };

        // Render status line if available
        if let Some(status_rect) = status_area {
            self.render_status_line(frame, status_rect);
        }

        // Unified terminal rendering (bash-style)
        self.render_unified_terminal(frame, main_area, border_color);
    }

    fn handle_key_event(&mut self, event: KeyEvent) -> Result<EventResponse> {
        if !self.focused || event.kind != KeyEventKind::Press {
            return Ok(EventResponse::NotHandled);
        }

        debug!("Terminal panel received key event: {:?} in mode {:?}", event, self.mode);

        // Global key handlers (work in both modes)
        match event.code {
            // Esc: Switch from INSERT to VIM mode
            KeyCode::Esc => {
                if self.mode == TerminalMode::Insert {
                    self.vim_goto_bottom();
                    self.vim_number_prefix.clear();
                    self.mode = TerminalMode::Vim;
                    return Ok(EventResponse::Handled);
                }
                // In VIM mode, Esc does nothing (already in VIM mode)
                return Ok(EventResponse::Handled);
            }

            // Ctrl+C double-press for exit (works in both modes)
            KeyCode::Char('c')
                if event.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                let now = Instant::now();
                if let Some(last_time) = self.last_ctrl_c {
                    if now.duration_since(last_time).as_secs() < 2 {
                        self.add_system("🚪 Exiting debugger (Ctrl+C double-press)...");
                        return Ok(EventResponse::Exit);
                    }
                }
                self.last_ctrl_c = Some(now);
                if self.mode == TerminalMode::Insert {
                    self.input_buffer.clear();
                    self.cursor_position = 0;
                    self.history_position = None;
                    self.add_system("^C (press again quickly to exit)");
                } else {
                    self.add_system("^C (press again quickly to exit)");
                }
                return Ok(EventResponse::Handled);
            }

            // Ctrl+L to clear terminal (works in both modes)
            KeyCode::Char('l')
                if event.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                self.lines.clear();
                self.add_system("Terminal cleared");
                return Ok(EventResponse::Handled);
            }

            _ => {}
        }

        // Mode-specific key handlers
        match self.mode {
            TerminalMode::Insert => self.handle_insert_mode_key(event),
            TerminalMode::Vim => self.handle_vim_mode_key(event),
        }
    }

    fn on_focus(&mut self) {
        self.focused = true;
        debug!("Terminal panel gained focus");
    }

    fn on_blur(&mut self) {
        self.focused = false;
        debug!("Terminal panel lost focus");
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    /// Get execution manager read-only reference
    fn exec_mgr(&self) -> RwLockReadGuard<'_, ExecutionManager> {
        self.execution_manager.read().expect("ExecutionManager lock poisoned")
    }

    /// Get execution manager reference
    fn exec_mgr_mut(&self) -> RwLockWriteGuard<'_, ExecutionManager> {
        self.execution_manager.write().expect("ExecutionManager lock poisoned")
    }

    /// Get resource manager read-only reference
    fn res_mgr(&self) -> RwLockReadGuard<'_, ResourceManager> {
        self.resource_manager.read().expect("ResourceManager lock poisoned")
    }

    /// Get resource manager reference
    fn res_mgr_mut(&self) -> RwLockWriteGuard<'_, ResourceManager> {
        self.resource_manager.write().expect("ResourceManager lock poisoned")
    }

    /// Get theme manager reference
    fn theme_mgr(&self) -> RwLockReadGuard<'_, ThemeManager> {
        self.theme_manager.read().expect("ThemeManager lock poisoned")
    }

    /// Get theme manager reference
    fn theme_mgr_mut(&self) -> RwLockWriteGuard<'_, ThemeManager> {
        self.theme_manager.write().expect("ThemeManager lock poisoned")
    }
}
