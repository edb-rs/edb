//! Trace panel for displaying execution trace
//!
//! This panel shows the call trace and allows navigation through trace entries.

use super::{EventResponse, Panel, PanelType};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use eyre::Result;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};
use std::collections::VecDeque;
use tracing::debug;

/// Trace panel implementation (stub)
#[derive(Debug)]
pub struct TracePanel {
    /// Mock trace entries for display
    trace_entries: Vec<String>,
    /// Currently selected trace entry index
    selected_index: usize,
    /// Whether this panel is focused
    focused: bool,
}

impl TracePanel {
    /// Create a new trace panel
    pub fn new() -> Self {
        Self {
            trace_entries: vec![
                "📞 CALL 0x123...abc → 0x456...def".to_string(),
                "  📞 CALL 0x456...def → 0x789...ghi".to_string(),
                "    📞 CREATE 0x789...ghi [Contract]".to_string(),
                "    ✅ SUCCESS output: 0x...".to_string(),
                "  ✅ SUCCESS output: 0x...".to_string(),
                "📞 CALL 0x123...abc → 0xaaa...bbb".to_string(),
                "  ❌ REVERT reason: insufficient balance".to_string(),
                "✅ SUCCESS final result".to_string(),
            ],
            selected_index: 0,
            focused: false,
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
        if self.selected_index < self.trace_entries.len().saturating_sub(1) {
            self.selected_index += 1;
        }
    }

    /// Get currently selected trace entry
    pub fn selected_entry(&self) -> Option<&String> {
        self.trace_entries.get(self.selected_index)
    }
}

impl Panel for TracePanel {
    fn panel_type(&self) -> PanelType {
        PanelType::Trace
    }

    fn title(&self) -> String {
        format!("Trace ({} entries)", self.trace_entries.len())
    }

    fn render(&mut self, frame: &mut Frame, area: Rect) {
        let border_color = if self.focused { Color::Cyan } else { Color::Gray };

        if self.trace_entries.is_empty() {
            let paragraph = Paragraph::new("No trace data available").block(
                Block::default()
                    .title(self.title())
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(border_color)),
            );
            frame.render_widget(paragraph, area);
            return;
        }

        // Create list items with selection highlighting
        let items: Vec<ListItem> = self
            .trace_entries
            .iter()
            .enumerate()
            .map(|(i, entry)| {
                let style = if i == self.selected_index && self.focused {
                    Style::default().bg(Color::Blue).fg(Color::White)
                } else if i == self.selected_index {
                    Style::default().bg(Color::DarkGray)
                } else {
                    Style::default()
                };
                ListItem::new(entry.as_str()).style(style)
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
            let help_text =
                "↑/↓: Navigate • Enter: Select • Tab: Next panel • Ctrl+T: Return terminal";
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
            KeyCode::Up => {
                self.move_up();
                Ok(EventResponse::Handled)
            }
            KeyCode::Down => {
                self.move_down();
                Ok(EventResponse::Handled)
            }
            KeyCode::Enter => {
                if let Some(entry) = self.selected_entry() {
                    debug!("Selected trace entry: {}", entry);
                    // TODO: Update current snapshot based on selected trace entry
                }
                Ok(EventResponse::Handled)
            }
            KeyCode::PageUp => {
                // Jump up by multiple entries
                self.selected_index = self.selected_index.saturating_sub(5);
                Ok(EventResponse::Handled)
            }
            KeyCode::PageDown => {
                // Jump down by multiple entries
                self.selected_index =
                    (self.selected_index + 5).min(self.trace_entries.len().saturating_sub(1));
                Ok(EventResponse::Handled)
            }
            KeyCode::Home => {
                self.selected_index = 0;
                Ok(EventResponse::Handled)
            }
            KeyCode::End => {
                self.selected_index = self.trace_entries.len().saturating_sub(1);
                Ok(EventResponse::Handled)
            }
            _ => Ok(EventResponse::NotHandled),
        }
    }

    fn on_focus(&mut self) {
        self.focused = true;
        debug!("Trace panel gained focus");
    }

    fn on_blur(&mut self) {
        self.focused = false;
        debug!("Trace panel lost focus");
    }
}

impl Default for TracePanel {
    fn default() -> Self {
        Self::new()
    }
}
