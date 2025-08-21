//! Terminal panel for command input and output
//!
//! This panel provides a command-line interface for debugging commands.

use super::{EventResponse, Panel, PanelType};
use crate::managers::ExecutionManager;
use crate::ui::colors::ColorScheme;
use crate::ui::icons::Icons;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use eyre::Result;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};
use std::time::Instant;
use std::{collections::VecDeque, fs::OpenOptions, io::Write};
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
    /// VIM mode cursor position: (line_index, column_index) in the displayed content
    vim_cursor_position: (usize, usize),
    /// Shared execution state manager
    execution_manager: Option<ExecutionManager>,
}

impl TerminalPanel {
    /// Create a new terminal panel
    pub fn new() -> Self {
        Self::new_with_execution_manager(None)
    }

    /// Create a new terminal panel with execution manager
    pub fn new_with_execution_manager(execution_manager: Option<ExecutionManager>) -> Self {
        let mut panel = Self {
            lines: Vec::new(),
            mode: TerminalMode::Insert,
            input_buffer: String::new(),
            cursor_position: 0,
            command_history: VecDeque::new(),
            history_position: None,
            scroll_offset: 0,
            focused: false,
            color_scheme: ColorScheme::default(),
            connected: true,
            snapshot_info: Some((127, 348)),
            last_ctrl_c: None,
            vim_number_prefix: String::new(),
            vim_cursor_position: (0, 0), // Start at top-left of displayed content
            execution_manager,
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
        // Auto-scroll to bottom when new content is added
        self.scroll_offset = 0;
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
        self.add_line(line, LineType::System);
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
                if let Some(exec_mgr) = &self.execution_manager {
                    let current = exec_mgr.current_snapshot();
                    let total = exec_mgr.total_snapshots();
                    if current < total.saturating_sub(1) {
                        exec_mgr.update_state(current + 1, total, Some(current + 10), None);
                        self.add_output(&format!(
                            "✅ Stepped to snapshot {}/{}",
                            current + 1,
                            total
                        ));
                    } else {
                        self.add_output("⚠️ Already at last snapshot");
                    }
                } else {
                    self.add_output("(RPC call would be made here)");
                }
            }
            "prev" | "p" => {
                self.add_system("Stepping to previous snapshot...");
                if let Some(exec_mgr) = &self.execution_manager {
                    let current = exec_mgr.current_snapshot();
                    let total = exec_mgr.total_snapshots();
                    if current > 0 {
                        exec_mgr.update_state(current - 1, total, Some(current + 8), None);
                        self.add_output(&format!(
                            "✅ Stepped to snapshot {}/{}",
                            current - 1,
                            total
                        ));
                    } else {
                        self.add_output("⚠️ Already at first snapshot");
                    }
                } else {
                    self.add_output("(RPC call would be made here)");
                }
            }
            "step" | "s" => {
                let count =
                    if parts.len() > 1 { parts[1].parse::<usize>().unwrap_or(1) } else { 1 };
                self.add_output(&format!("Stepping {} snapshots...", count));
                if let Some(exec_mgr) = &self.execution_manager {
                    let current = exec_mgr.current_snapshot();
                    let total = exec_mgr.total_snapshots();
                    let new_pos = (current + count).min(total.saturating_sub(1));
                    exec_mgr.update_state(new_pos, total, Some(new_pos + 9), None);
                    self.add_output(&format!(
                        "✅ Stepped {} snapshots to {}/{}",
                        count, new_pos, total
                    ));
                } else {
                    self.add_output("(RPC call would be made here)");
                }
            }
            "reverse" | "r" => {
                let count =
                    if parts.len() > 1 { parts[1].parse::<usize>().unwrap_or(1) } else { 1 };
                self.add_output(&format!("Reverse stepping {} snapshots...", count));
                if let Some(exec_mgr) = &self.execution_manager {
                    let current = exec_mgr.current_snapshot();
                    let total = exec_mgr.total_snapshots();
                    let new_pos = current.saturating_sub(count);
                    exec_mgr.update_state(new_pos, total, Some(new_pos + 9), None);
                    self.add_output(&format!(
                        "✅ Reverse stepped {} snapshots to {}/{}",
                        count, new_pos, total
                    ));
                } else {
                    self.add_output("(RPC call would be made here)");
                }
            }
            "call" | "c" => {
                self.add_system("Stepping to next function call...");
                if let Some(exec_mgr) = &self.execution_manager {
                    let current = exec_mgr.current_snapshot();
                    let total = exec_mgr.total_snapshots();
                    // Simulate jumping to next significant call (larger step)
                    let new_pos = (current + 10).min(total.saturating_sub(1));
                    exec_mgr.update_state(new_pos, total, Some(new_pos + 9), None);
                    self.add_output(&format!(
                        "✅ Stepped to next call at snapshot {}/{}",
                        new_pos, total
                    ));
                } else {
                    self.add_output("(RPC call would be made here)");
                }
            }
            "rcall" | "rc" => {
                self.add_system("Stepping back from function call...");
                if let Some(exec_mgr) = &self.execution_manager {
                    let current = exec_mgr.current_snapshot();
                    let total = exec_mgr.total_snapshots();
                    // Simulate jumping back to previous significant call
                    let new_pos = current.saturating_sub(10);
                    exec_mgr.update_state(new_pos, total, Some(new_pos + 9), None);
                    self.add_output(&format!(
                        "✅ Stepped back to previous call at snapshot {}/{}",
                        new_pos, total
                    ));
                } else {
                    self.add_output("(RPC call would be made here)");
                }
            }
            "goto" => {
                if parts.len() > 1 {
                    if let Ok(index) = parts[1].parse::<usize>() {
                        self.add_output(&format!("Jumping to snapshot {}...", index));
                        if let Some(exec_mgr) = &self.execution_manager {
                            let total = exec_mgr.total_snapshots();
                            if index < total {
                                exec_mgr.update_state(index, total, Some(index + 9), None);
                                self.add_output(&format!(
                                    "✅ Jumped to snapshot {}/{}",
                                    index, total
                                ));
                            } else {
                                self.add_output(&format!(
                                    "⚠️ Invalid snapshot index. Range: 0-{}",
                                    total.saturating_sub(1)
                                ));
                            }
                        } else {
                            self.add_output("(RPC call would be made here)");
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
        self.add_output("  (VIM mode) j/k/↑/↓ - Move cursor vertically (with auto-scroll)");
        self.add_output("  (VIM mode) h/l/←/→ - Move cursor horizontally");
        self.add_output("  (VIM mode) 5j/3↓   - Move multiple lines with number prefix");
        self.add_output("  (VIM mode) gg/G    - Go to top/bottom");
        self.add_output("  (VIM mode) {/}     - Jump between commands");
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

    /// Render status line with connection and snapshot info    
    fn render_status_line(&self, frame: &mut Frame<'_>, area: Rect) {
        let status_text = if let Some((current, total)) = self.snapshot_info {
            format!(
                "{} Connected {} Snapshot {}/{} {} Gas: 2,847,293",
                if self.connected { Icons::SUCCESS } else { Icons::ERROR },
                Icons::BULLET,
                current,
                total,
                Icons::BULLET
            )
        } else {
            format!(
                "{} {} Status: Ready",
                if self.connected { Icons::CONNECTED } else { Icons::DISCONNECTED },
                Icons::BULLET
            )
        };

        let status_paragraph = Paragraph::new(Line::from(vec![Span::styled(
            status_text,
            Style::default().fg(self.color_scheme.info_color),
        )]))
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(self.color_scheme.unfocused_border)),
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

            // Horizontal cursor movement in VIM mode
            KeyCode::Char('h') | KeyCode::Left => {
                self.vim_move_cursor_left();
                Ok(EventResponse::Handled)
            }
            KeyCode::Char('l') | KeyCode::Right => {
                self.vim_move_cursor_right();
                Ok(EventResponse::Handled)
            }

            // Handle number prefixes for multi-line scrolling
            KeyCode::Char(c) if c.is_ascii_digit() => {
                self.vim_number_prefix.push(c);
                Ok(EventResponse::Handled)
            }

            // Command boundaries { and }
            KeyCode::Char('{') => {
                self.vim_jump_to_prev_command();
                Ok(EventResponse::Handled)
            }
            KeyCode::Char('}') => {
                self.vim_jump_to_next_command();
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

    /// Scroll down in VIM mode
    fn vim_scroll_down(&mut self, count: usize) {
        let multiplier = if self.vim_number_prefix.is_empty() {
            count
        } else {
            self.vim_number_prefix.parse::<usize>().unwrap_or(1) * count
        };

        let max_scroll = self.lines.len().saturating_sub(1);
        self.scroll_offset = (self.scroll_offset + multiplier).min(max_scroll);
        self.vim_number_prefix.clear();
    }

    /// Scroll up in VIM mode
    fn vim_scroll_up(&mut self, count: usize) {
        let multiplier = if self.vim_number_prefix.is_empty() {
            count
        } else {
            self.vim_number_prefix.parse::<usize>().unwrap_or(1) * count
        };

        self.scroll_offset = self.scroll_offset.saturating_sub(multiplier);
        self.vim_number_prefix.clear();
    }

    /// Move VIM cursor up (k key) with auto-scrolling
    fn vim_move_cursor_up(&mut self, count: usize) {
        let multiplier = if self.vim_number_prefix.is_empty() {
            count
        } else {
            self.vim_number_prefix.parse::<usize>().unwrap_or(1) * count
        };

        // Move cursor up in the visible area
        if self.vim_cursor_position.0 >= multiplier {
            self.vim_cursor_position.0 -= multiplier;
        } else {
            // Need to scroll up to show more content above
            let scroll_needed = multiplier - self.vim_cursor_position.0;
            self.scroll_offset =
                (self.scroll_offset + scroll_needed).min(self.lines.len().saturating_sub(1));
            self.vim_cursor_position.0 = 0;
        }

        // Clamp horizontal position to new line length
        self.vim_clamp_cursor_to_line();
        self.vim_number_prefix.clear();
    }

    /// Move VIM cursor down (j key) with auto-scrolling  
    fn vim_move_cursor_down(&mut self, count: usize) {
        let multiplier = if self.vim_number_prefix.is_empty() {
            count
        } else {
            self.vim_number_prefix.parse::<usize>().unwrap_or(1) * count
        };

        let visible_lines = self.get_visible_lines_for_vim();
        let max_line_idx = visible_lines.len().saturating_sub(1);

        if self.vim_cursor_position.0 + multiplier <= max_line_idx {
            self.vim_cursor_position.0 += multiplier;
        } else {
            // Need to scroll down or hit bottom
            let overflow = (self.vim_cursor_position.0 + multiplier) - max_line_idx;
            if self.scroll_offset >= overflow {
                self.scroll_offset -= overflow;
                self.vim_cursor_position.0 = max_line_idx;
            } else {
                // Hit bottom of content
                self.scroll_offset = 0;
                self.vim_cursor_position.0 =
                    self.get_visible_lines_for_vim().len().saturating_sub(1);
            }
        }

        // Clamp horizontal position to new line length
        self.vim_clamp_cursor_to_line();
        self.vim_number_prefix.clear();
    }

    /// Move VIM cursor left (h key)
    fn vim_move_cursor_left(&mut self) {
        if self.vim_cursor_position.1 > 0 {
            self.vim_cursor_position.1 -= 1;
        }
    }

    /// Move VIM cursor right (l key)
    fn vim_move_cursor_right(&mut self) {
        // Get the current line to check its length
        let visible_lines = self.get_visible_lines_for_vim();
        if let Some(current_line) = visible_lines.get(self.vim_cursor_position.0) {
            let line_length = current_line.content.chars().count();
            if self.vim_cursor_position.1 < line_length.saturating_sub(1) {
                self.vim_cursor_position.1 += 1;
            }
        }
    }

    /// Clamp VIM cursor horizontal position to the current line length
    fn vim_clamp_cursor_to_line(&mut self) {
        let visible_lines = self.get_visible_lines_for_vim();
        if let Some(current_line) = visible_lines.get(self.vim_cursor_position.0) {
            let line_length = current_line.content.chars().count();
            if line_length > 0 {
                self.vim_cursor_position.1 =
                    self.vim_cursor_position.1.min(line_length.saturating_sub(1));
            } else {
                self.vim_cursor_position.1 = 0;
            }
        }
    }

    /// Get the currently visible lines for VIM cursor navigation
    fn get_visible_lines_for_vim(&self) -> Vec<&TerminalLine> {
        // In VIM mode, we only show terminal history (no input line)
        let start_idx = if self.scroll_offset >= self.lines.len() {
            0
        } else {
            self.lines.len().saturating_sub(self.scroll_offset + 1)
        };

        self.lines.iter().skip(start_idx).collect()
    }

    /// Jump to previous command boundary
    fn vim_jump_to_prev_command(&mut self) {
        let current_line = self.lines.len().saturating_sub(self.scroll_offset + 1);

        // Search backwards for a command line
        for i in (0..current_line).rev() {
            if matches!(self.lines[i].line_type, LineType::Command) {
                self.scroll_offset = self.lines.len().saturating_sub(i + 1);
                break;
            }
        }
        self.vim_number_prefix.clear();
    }

    /// Jump to next command boundary
    fn vim_jump_to_next_command(&mut self) {
        let current_line = self.lines.len().saturating_sub(self.scroll_offset + 1);

        // Search forwards for a command line
        for i in (current_line + 1)..self.lines.len() {
            if matches!(self.lines[i].line_type, LineType::Command) {
                self.scroll_offset = self.lines.len().saturating_sub(i + 1);
                break;
            }
        }
        self.vim_number_prefix.clear();
    }

    /// Go to top of terminal
    fn vim_goto_top(&mut self) {
        self.scroll_offset = self.lines.len().saturating_sub(1);
    }

    /// Go to bottom of terminal
    fn vim_goto_bottom(&mut self) {
        self.scroll_offset = 0;
    }

    /// Render a line with VIM cursor (block cursor)
    fn render_line_with_vim_cursor<'a>(&self, content: &'a str, base_style: Style) -> Line<'a> {
        let chars: Vec<char> = content.chars().collect();
        let cursor_col = self.vim_cursor_position.1;

        if chars.is_empty() {
            // Empty line - show cursor as a space with background
            return Line::from(vec![Span::styled(
                " ",
                Style::default().bg(Color::White).fg(Color::Black),
            )]);
        }

        if cursor_col >= chars.len() {
            // Cursor at end of line - show the content plus a cursor space
            return Line::from(vec![
                Span::styled(content, base_style),
                Span::styled(" ", Style::default().bg(Color::White).fg(Color::Black)),
            ]);
        }

        // Cursor is within the line - split content around cursor
        let before: String = chars.iter().take(cursor_col).collect();
        let at_cursor = chars[cursor_col];
        let after: String = chars.iter().skip(cursor_col + 1).collect();

        let mut spans = Vec::new();

        // Add content before cursor
        if !before.is_empty() {
            spans.push(Span::styled(before, base_style));
        }

        // Add cursor character with inverted colors (block cursor)
        spans.push(Span::styled(
            at_cursor.to_string(),
            Style::default().bg(Color::White).fg(Color::Black),
        ));

        // Add content after cursor
        if !after.is_empty() {
            spans.push(Span::styled(after, base_style));
        }

        Line::from(spans)
    }

    /// Render the unified bash-like terminal view
    fn render_unified_terminal(&mut self, frame: &mut Frame<'_>, area: Rect, border_color: Color) {
        // Start with all terminal history
        let mut all_content = self.lines.clone();

        // Add current input line only in INSERT mode when focused
        if self.mode == TerminalMode::Insert && self.focused {
            let prompt = format!(
                "{} edb{} {}",
                if self.connected { Icons::CONNECTED } else { Icons::DISCONNECTED },
                Icons::ARROW_RIGHT,
                if self.focused {
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
        }

        // Calculate visible area (leave space for help text if needed)
        let help_height = if self.focused && area.height > 8 { 1 } else { 0 };
        let content_height = area.height.saturating_sub(2 + help_height) as usize; // Account for borders + help

        // Calculate visible lines based on scroll_offset
        let total_content = all_content.len();
        let end_idx = total_content.saturating_sub(self.scroll_offset);
        let start_idx = end_idx.saturating_sub(content_height);

        // Create unified terminal content items with VIM cursor support
        let terminal_items: Vec<ListItem<'_>> = all_content
            .iter()
            .skip(start_idx)
            .take(content_height)
            .enumerate()
            .map(|(display_row, terminal_line)| {
                let base_style = match terminal_line.line_type {
                    LineType::Command => Style::default().fg(self.color_scheme.command_color),
                    LineType::Output => Style::default().fg(self.color_scheme.output_color),
                    LineType::Error => Style::default().fg(self.color_scheme.error_color),
                    LineType::System => Style::default().fg(self.color_scheme.info_color),
                };

                // Check if this line has the VIM cursor and we're in VIM mode
                let styled_line = if self.mode == TerminalMode::Vim
                    && self.focused
                    && display_row == self.vim_cursor_position.0
                {
                    self.render_line_with_vim_cursor(&terminal_line.content, base_style)
                } else {
                    Line::from(Span::styled(&terminal_line.content, base_style))
                };

                ListItem::new(styled_line)
            })
            .collect();

        // Show scroll indicator and mode in title
        let scroll_indicator = if self.scroll_offset > 0 {
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

        let terminal_block = Block::default()
            .title(terminal_title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));

        // Create the main terminal area (leave space for help text at bottom)
        let main_area = if self.focused && area.height > 8 {
            Rect {
                x: area.x,
                y: area.y,
                width: area.width,
                height: area.height - 1, // Leave one line for help
            }
        } else {
            area
        };

        let terminal_list = List::new(terminal_items).block(terminal_block);
        frame.render_widget(terminal_list, main_area);

        // Render help text at the very bottom, outside the terminal border
        if self.focused && area.height > 8 {
            self.render_help_text(frame, area);
        }
    }

    /// Render mode-specific help text
    fn render_help_text(&self, frame: &mut Frame<'_>, area: Rect) {
        let help_area =
            Rect { x: area.x, y: area.y + area.height - 1, width: area.width, height: 1 };

        let help_text = match self.mode {
            TerminalMode::Insert => "INSERT mode: Type commands • ↑/↓: History • Esc: VIM mode",
            TerminalMode::Vim => "VIM mode: j/k/↑/↓: Move cursor • h/l/←/→: Horizontal • gg/G: Top/Bottom • i/Enter: INSERT",
        };

        let help_paragraph = Paragraph::new(help_text).style(Style::default().fg(Color::Yellow));
        frame.render_widget(help_paragraph, help_area);
    }
}

impl Panel for TerminalPanel {
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
            self.color_scheme.focused_border
        } else {
            self.color_scheme.unfocused_border
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
}

impl Default for TerminalPanel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terminal_panel_creation() {
        let panel = TerminalPanel::new();
        assert_eq!(panel.panel_type(), PanelType::Terminal);
        assert!(!panel.lines.is_empty()); // Should have welcome message
        assert_eq!(panel.mode, TerminalMode::Insert);
    }

    #[test]
    fn test_command_execution() {
        let mut panel = TerminalPanel::new();
        let initial_count = panel.lines.len();

        panel.execute_command("help").unwrap();
        assert!(panel.lines.len() > initial_count);
    }
}
