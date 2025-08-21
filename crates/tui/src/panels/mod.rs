//! Panel framework and implementations
//!
//! This module contains the panel trait and all panel implementations.

use crate::managers::BreakpointManager;
use crossterm::event::{Event, KeyEvent};
use eyre::Result;
use ratatui::{layout::Rect, Frame};
use std::fmt::Debug;

/// Panel types for identification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PanelType {
    /// Trace panel showing execution trace
    Trace,
    /// Code panel showing source code or opcodes
    Code,
    /// Display panel showing variables, stack, memory, etc.
    Display,
    /// Terminal panel for command input/output
    Terminal,
}

/// Response from panel event handling
#[derive(Debug)]
pub enum EventResponse {
    /// Event was handled, no further action needed
    Handled,
    /// Event was not handled, pass to next handler
    NotHandled,
    /// Request focus change to another panel
    ChangeFocus(PanelType),
    /// Request application exit
    Exit,
}

/// Trait for UI panels
pub trait Panel: Debug + Send {
    /// Render the panel content
    fn render(&mut self, frame: &mut Frame, area: Rect);

    /// Handle keyboard events
    fn handle_key_event(&mut self, event: KeyEvent) -> Result<EventResponse> {
        let _ = event; // Suppress unused parameter warning
        Ok(EventResponse::NotHandled)
    }

    /// Handle other events (mouse, resize, etc.)
    fn handle_event(&mut self, event: Event) -> Result<EventResponse> {
        let _ = event; // Suppress unused parameter warning
        Ok(EventResponse::NotHandled)
    }

    /// Called when this panel gains focus
    fn on_focus(&mut self) {}

    /// Called when this panel loses focus
    fn on_blur(&mut self) {}

    /// Get the panel type
    fn panel_type(&self) -> PanelType;

    /// Get panel title for display
    fn title(&self) -> String {
        format!("{:?} Panel", self.panel_type())
    }
}

// Re-export all panel implementations
pub mod code;
pub mod display;
pub mod terminal;
pub mod trace;

pub use code::CodePanel;
pub use display::DisplayPanel;
pub use terminal::TerminalPanel;
pub use trace::TracePanel;
