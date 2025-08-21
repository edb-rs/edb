//! Trace panel for displaying execution trace
//!
//! This panel shows the call trace and allows navigation through trace entries.

use super::{EventResponse, Panel, PanelType};
use crate::managers::{ExecutionManager, ThemeManager};
use crate::ui::borders::BorderPresets;
use crate::ui::status::StatusBar;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use eyre::Result;
use ratatui::{
    layout::Rect,
    style::Style,
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};
use tracing::debug;

/// Trace panel implementation (stub)
#[derive(Debug)]
pub struct TracePanel {
    /// Mock trace entries for display
    trace_entries: Vec<String>,
    /// Currently selected trace entry index
    selected_index: usize,
    /// Scroll offset
    scroll_offset: usize,
    /// Current content height
    context_height: usize,
    /// Whether this panel is focused
    focused: bool,
    /// Shared execution state manager
    execution_manager: ExecutionManager,
    /// Theme manager for styling
    theme_manager: ThemeManager,
}

impl TracePanel {
    /// Create a new trace panel with required managers
    pub fn new_with_managers(
        execution_manager: ExecutionManager,
        theme_manager: ThemeManager,
    ) -> Self {
        Self {
            trace_entries: vec![
                "📞 CALL 0x123...abc → 0x456...def".to_string(),
                "  📞 CALL 0x456...def → 0x789...ghi".to_string(),
                "    📞 CREATE 0x789...ghi [Contract]".to_string(),
                "    ✅ SUCCESS output: 0x...".to_string(),
                "  ✅ SUCCESS output: 0x...".to_string(),
                "📞 CALL 0x123...abc → 0xaaa...bbb".to_string(),
                "  ❌ REVERT reason: insufficient balance".to_string(),
                "✅ SUCCESS final result 1".to_string(),
                "✅ SUCCESS final result 2".to_string(),
                "✅ SUCCESS final result 3".to_string(),
                "✅ SUCCESS final result 4".to_string(),
            ],
            selected_index: 0,
            focused: false,
            scroll_offset: 0,
            context_height: 0,
            execution_manager,
            theme_manager,
        }
    }

    /// Move selection up
    fn move_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
            if self.selected_index < self.scroll_offset {
                self.scroll_offset = self.selected_index;
            }
        }
    }

    /// Move selection down
    fn move_down(&mut self) {
        let max_lines = self.trace_entries.len();
        if self.selected_index < max_lines.saturating_sub(1) {
            self.selected_index += 1;
            let viewport_height = self.context_height;
            if self.selected_index >= self.scroll_offset + viewport_height {
                self.scroll_offset = (self.selected_index + 1).saturating_sub(viewport_height);
            }
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
        let border_color = if self.focused {
            self.theme_manager.focused_border_color()
        } else {
            self.theme_manager.unfocused_border_color()
        };

        self.context_height = if self.focused && area.height > 10 {
            area.height.saturating_sub(4) // Account for borders and status lines
        } else {
            area.height.saturating_sub(2) // Just borders
        } as usize;

        if self.trace_entries.is_empty() {
            let paragraph = Paragraph::new("No trace data available").block(BorderPresets::trace(
                self.focused,
                self.title(),
                self.theme_manager.focused_border_color(),
                self.theme_manager.unfocused_border_color(),
            ));
            frame.render_widget(paragraph, area);
            return;
        }

        // Create list items with selection highlighting
        let items: Vec<ListItem> = self
            .trace_entries
            .iter()
            .enumerate()
            .skip(self.scroll_offset)
            .take(self.context_height)
            .map(|(i, entry)| {
                let style = if i == self.selected_index && self.focused {
                    Style::default()
                        .bg(self.theme_manager.selected_bg_color())
                        .fg(self.theme_manager.selected_fg_color())
                } else if i == self.selected_index {
                    Style::default().bg(self.theme_manager.highlight_bg_color())
                } else {
                    Style::default()
                };
                ListItem::new(entry.as_str()).style(style)
            })
            .collect();

        let list = List::new(items)
            .block(BorderPresets::trace(
                self.focused,
                self.title(),
                self.theme_manager.focused_border_color(),
                self.theme_manager.unfocused_border_color(),
            ))
            .highlight_style(Style::default().bg(self.theme_manager.selected_bg_color()));

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

            let status_bar = StatusBar::new().current_panel("Trace".to_string()).message(format!(
                "Entry: {}/{}",
                self.selected_index + 1,
                self.trace_entries.len()
            ));

            let status_text = status_bar.build();
            let status_paragraph = Paragraph::new(status_text)
                .style(Style::default().fg(self.theme_manager.accent_color()));
            frame.render_widget(status_paragraph, status_area);

            let help_area = Rect {
                x: area.x + 1,
                y: area.y + area.height - 2,
                width: area.width - 2,
                height: 1,
            };
            let help_text = "↑/↓: Navigate • Enter: Select • Tab: Next panel";
            let help_paragraph = Paragraph::new(help_text)
                .style(Style::default().fg(self.theme_manager.help_text_color()));
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

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
