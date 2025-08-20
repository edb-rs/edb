//! Terminal panel for command input and output
//!
//! This panel provides a command-line interface for debugging commands.

use super::{EventResponse, Panel, PanelType};
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
use std::collections::VecDeque;
use std::time::Instant;
use tracing::debug;

/// Maximum number of output lines to keep in history
const MAX_OUTPUT_LINES: usize = 1000;

/// Maximum number of command history entries
const MAX_COMMAND_HISTORY: usize = 100;

/// Terminal panel implementation
#[derive(Debug)]
pub struct TerminalPanel {
    /// Current input buffer
    input_buffer: String,
    /// Cursor position in input buffer
    cursor_position: usize,
    /// Command history
    command_history: VecDeque<String>,
    /// Current position in command history (None = no history browsing)
    history_position: Option<usize>,
    /// Output lines
    output_lines: VecDeque<String>,
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
}

impl TerminalPanel {
    /// Create a new terminal panel
    pub fn new() -> Self {
        let mut panel = Self {
            input_buffer: String::new(),
            cursor_position: 0,
            command_history: VecDeque::new(),
            history_position: None,
            output_lines: VecDeque::new(),
            focused: false,
            color_scheme: ColorScheme::default(),
            connected: true,
            snapshot_info: Some((127, 348)),
            last_ctrl_c: None,
        };

        // Add welcome message with fancy styling
        panel.add_output(&format!("{} EDB Time-Travel Debugger v1.0", Icons::TARGET_REACHED));
        panel.add_output(&format!("{} Connected to RPC server", Icons::CONNECTED));
        panel.add_output(&format!("{} Type 'help' for available commands", Icons::INFO));
        panel.add_output("");

        panel
    }

    /// Add a line to the output
    pub fn add_output(&mut self, line: &str) {
        if self.output_lines.len() >= MAX_OUTPUT_LINES {
            self.output_lines.pop_front();
        }
        self.output_lines.push_back(line.to_string());
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

        // Echo command
        self.add_output(&format!("> {}", command));

        // Handle built-in commands
        match command.trim() {
            "" => {}
            "quit" | "q" | "exit" => {
                self.add_output("🚪 Exiting debugger...");
                return Ok(EventResponse::Exit);
            }
            "help" | "h" => {
                self.show_help();
            }
            "clear" | "cls" => {
                self.output_lines.clear();
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
                self.add_output("Stepping to next snapshot...");
                self.add_output("(RPC call would be made here)");
            }
            "prev" | "p" => {
                self.add_output("Stepping to previous snapshot...");
                self.add_output("(RPC call would be made here)");
            }
            "step" => {
                let count =
                    if parts.len() > 1 { parts[1].parse::<usize>().unwrap_or(1) } else { 1 };
                self.add_output(&format!("Stepping {} snapshots...", count));
                self.add_output("(RPC call would be made here)");
            }
            "goto" => {
                if parts.len() > 1 {
                    if let Ok(index) = parts[1].parse::<usize>() {
                        self.add_output(&format!("Jumping to snapshot {}...", index));
                        self.add_output("(RPC call would be made here)");
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
        self.add_output("Available commands:");
        self.add_output("");
        self.add_output("Navigation:");
        self.add_output("  next, n          - Step to next snapshot");
        self.add_output("  prev, p          - Step to previous snapshot");
        self.add_output("  step <count>     - Step multiple snapshots");
        self.add_output("  goto <index>     - Jump to specific snapshot");
        self.add_output("");
        self.add_output("Inspection:");
        self.add_output("  stack            - Show current stack");
        self.add_output("  memory [offset]  - Show memory");
        self.add_output("  variables, vars  - Show variables in scope");
        self.add_output("  reset            - Reset display panel");
        self.add_output("");
        self.add_output("Breakpoints:");
        self.add_output("  break <location> - Set breakpoint");
        self.add_output("");
        self.add_output("Solidity expressions (prefix with $):");
        self.add_output("  $balance         - Evaluate variable");
        self.add_output("  $msg.sender      - Evaluate expression");
        self.add_output("");
        self.add_output("Other:");
        self.add_output("  help, h          - Show this help");
        self.add_output("  clear, cls       - Clear output");
        self.add_output("  history          - Show command history");
        self.add_output("  quit, q, exit    - Exit debugger");
        self.add_output("  Tab              - Switch panels");
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
}

impl Panel for TerminalPanel {
    fn panel_type(&self) -> PanelType {
        PanelType::Terminal
    }

    fn title(&self) -> String {
        if let Some((current, total)) = self.snapshot_info {
            format!("{} Debug Terminal [{}/{}]", Icons::PROCESSING, current, total)
        } else {
            format!("{} Debug Terminal", Icons::PROCESSING)
        }
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

        // Split main area into output and input sections
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(3),    // Output area
                Constraint::Length(3), // Input area
            ])
            .split(main_area);

        // Render status line if available
        if let Some(status_rect) = status_area {
            self.render_status_line(frame, status_rect);
        }

        // Render output area with enhanced styling
        let output_items: Vec<ListItem<'_>> = self
            .output_lines
            .iter()
            .map(|line| {
                // Add color coding for different types of output
                let styled_line = if line.starts_with(&format!("{}", Icons::SUCCESS)) {
                    Line::from(Span::styled(
                        line.clone(),
                        Style::default().fg(self.color_scheme.success_color),
                    ))
                } else if line.starts_with(&format!("{}", Icons::ERROR)) {
                    Line::from(Span::styled(
                        line.clone(),
                        Style::default().fg(self.color_scheme.error_color),
                    ))
                } else if line.starts_with(&format!("{}", Icons::WARNING)) {
                    Line::from(Span::styled(
                        line.clone(),
                        Style::default().fg(self.color_scheme.warning_color),
                    ))
                } else if line.starts_with(&format!("{}", Icons::INFO)) {
                    Line::from(Span::styled(
                        line.clone(),
                        Style::default().fg(self.color_scheme.info_color),
                    ))
                } else if line.starts_with(">") {
                    Line::from(Span::styled(
                        line.clone(),
                        Style::default().fg(self.color_scheme.command_color),
                    ))
                } else {
                    Line::from(Span::styled(
                        line.clone(),
                        Style::default().fg(self.color_scheme.output_color),
                    ))
                };
                ListItem::new(styled_line)
            })
            .collect();

        let output_block = Block::default()
            .title(format!("{} Output", Icons::FILE))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));

        let output_list = List::new(output_items).block(output_block);

        frame.render_widget(output_list, chunks[0]);

        // Render input area with enhanced prompt
        let prompt = &format!(
            "{} edb{} ",
            if self.connected { Icons::CONNECTED } else { Icons::DISCONNECTED },
            Icons::ARROW_RIGHT
        );
        let input_text = if self.focused && self.cursor_position <= self.input_buffer.len() {
            // Show enhanced cursor
            let mut chars: Vec<char> = self.input_buffer.chars().collect();
            if self.cursor_position == chars.len() {
                chars.push('█'); // Block cursor at end
            } else if self.cursor_position < chars.len() {
                chars[self.cursor_position] = '█'; // Block cursor in middle
            }
            format!("{}{}", prompt, chars.into_iter().collect::<String>())
        } else {
            format!("{}{}", prompt, self.input_buffer)
        };

        let input_paragraph = Paragraph::new(Line::from(vec![
            Span::styled(prompt, Style::default().fg(self.color_scheme.prompt_color)),
            Span::styled(&self.input_buffer, Style::default().fg(self.color_scheme.command_color)),
            if self.focused {
                Span::styled("█", Style::default().fg(self.color_scheme.cursor_color))
            } else {
                Span::styled("", Style::default())
            },
        ]))
        .block(
            Block::default()
                .title(format!("{} Command Input", Icons::CODE))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color)),
        );

        frame.render_widget(input_paragraph, chunks[1]);
    }

    fn handle_key_event(&mut self, event: KeyEvent) -> Result<EventResponse> {
        if event.kind != KeyEventKind::Press {
            return Ok(EventResponse::NotHandled);
        }

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
            KeyCode::Char('l')
                if event.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                self.output_lines.clear();
                Ok(EventResponse::Handled)
            }
            KeyCode::Char('c')
                if event.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                // Handle Ctrl+C double-press for exit
                let now = Instant::now();
                if let Some(last_time) = self.last_ctrl_c {
                    if now.duration_since(last_time).as_secs() < 2 {
                        // Double-press within 2 seconds = exit
                        self.add_output("🚪 Exiting debugger (Ctrl+C double-press)...");
                        return Ok(EventResponse::Exit);
                    }
                }
                // First press - clear input and record time
                self.last_ctrl_c = Some(now);
                self.input_buffer.clear();
                self.cursor_position = 0;
                self.history_position = None;
                self.add_output("^C (press again quickly to exit)");
                Ok(EventResponse::Handled)
            }
            KeyCode::Char(c) => {
                self.input_buffer.insert(self.cursor_position, c);
                self.cursor_position += 1;
                self.history_position = None;
                Ok(EventResponse::Handled)
            }
            _ => Ok(EventResponse::NotHandled),
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
        assert!(!panel.output_lines.is_empty()); // Should have welcome message
    }

    #[test]
    fn test_command_execution() {
        let mut panel = TerminalPanel::new();
        let initial_count = panel.output_lines.len();

        panel.execute_command("help").unwrap();
        assert!(panel.output_lines.len() > initial_count);
    }
}
