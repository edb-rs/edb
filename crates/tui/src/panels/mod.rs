//! Panel framework and implementations
//!
//! This module contains the panel trait and all panel implementations.

use crossterm::event::{Event, KeyEvent};
use eyre::Result;
use ratatui::{backend::Backend, layout::Rect, Frame};
use std::collections::HashSet;
use std::fmt::Debug;
use std::sync::{Arc, RwLock};

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

/// Shared breakpoint manager for communication between panels
#[derive(Debug, Clone)]
pub struct BreakpointManager {
    /// Shared breakpoint data (line numbers, 1-based)
    breakpoints: Arc<RwLock<HashSet<usize>>>,
}

impl BreakpointManager {
    /// Create a new breakpoint manager
    pub fn new() -> Self {
        Self { breakpoints: Arc::new(RwLock::new(HashSet::new())) }
    }

    /// Add a breakpoint at the given line
    pub fn add_breakpoint(&self, line: usize) -> bool {
        if let Ok(mut breakpoints) = self.breakpoints.write() {
            breakpoints.insert(line)
        } else {
            false
        }
    }

    /// Remove a breakpoint at the given line
    pub fn remove_breakpoint(&self, line: usize) -> bool {
        if let Ok(mut breakpoints) = self.breakpoints.write() {
            breakpoints.remove(&line)
        } else {
            false
        }
    }

    /// Toggle a breakpoint at the given line
    pub fn toggle_breakpoint(&self, line: usize) -> bool {
        if let Ok(mut breakpoints) = self.breakpoints.write() {
            if breakpoints.contains(&line) {
                breakpoints.remove(&line);
                false
            } else {
                breakpoints.insert(line);
                true
            }
        } else {
            false
        }
    }

    /// Check if a breakpoint exists at the given line
    pub fn has_breakpoint(&self, line: usize) -> bool {
        if let Ok(breakpoints) = self.breakpoints.read() {
            breakpoints.contains(&line)
        } else {
            false
        }
    }

    /// Get all breakpoints as a sorted vector
    pub fn get_all_breakpoints(&self) -> Vec<usize> {
        if let Ok(breakpoints) = self.breakpoints.read() {
            let mut sorted: Vec<usize> = breakpoints.iter().cloned().collect();
            sorted.sort();
            sorted
        } else {
            Vec::new()
        }
    }

    /// Get breakpoint count
    pub fn count(&self) -> usize {
        if let Ok(breakpoints) = self.breakpoints.read() {
            breakpoints.len()
        } else {
            0
        }
    }
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

// Export shared utilities
// Note: BreakpointManager is already accessible as it's defined in this module
