//! Panel framework and implementations
//!
//! This module contains the panel trait and all panel implementations.

use crate::{managers::{ExecutionManager, ResourceManager, ThemeManager}, ColorScheme};
use crossterm::event::{Event, KeyEvent};
use eyre::Result;
use ratatui::{layout::Rect, Frame};
use std::{
    fmt::Debug,
    sync::{RwLockReadGuard, RwLockWriteGuard},
};

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
pub trait PanelTr: Debug + Send {
    /// Render the panel content
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect);

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

    /// Allow downcasting to concrete types
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;

    /// Get execution manager read-only reference
    fn exec_mgr(&self) -> RwLockReadGuard<'_, ExecutionManager>;

    /// Get execution manager reference
    fn exec_mgr_mut(&self) -> RwLockWriteGuard<'_, ExecutionManager>;

    /// Get resource manager read-only reference
    fn res_mgr(&self) -> RwLockReadGuard<'_, ResourceManager>;

    /// Get resource manager reference
    fn res_mgr_mut(&self) -> RwLockWriteGuard<'_, ResourceManager>;

    /// Get theme manager reference
    fn theme_mgr(&self) -> RwLockReadGuard<'_, ThemeManager>;

    /// Get theme manager reference
    fn theme_mgr_mut(&self) -> RwLockWriteGuard<'_, ThemeManager>;

    /// Fetch data from manager
    async fn fetch_data(&mut self) -> Result<()> {
        Ok(())
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

#[derive(Debug)]
pub enum Panel {
    Code(CodePanel),
    Display(DisplayPanel),
    Terminal(TerminalPanel),
    Trace(TracePanel),
}

impl PanelTr for Panel {
    fn render(&mut self, frame: &mut Frame, area: Rect) {
        match self {
            Panel::Code(panel) => panel.render(frame, area),
            Panel::Display(panel) => panel.render(frame, area),
            Panel::Terminal(panel) => panel.render(frame, area),
            Panel::Trace(panel) => panel.render(frame, area),
        }
    }

    fn handle_key_event(&mut self, event: KeyEvent) -> Result<EventResponse> {
        match self {
            Panel::Code(panel) => panel.handle_key_event(event),
            Panel::Display(panel) => panel.handle_key_event(event),
            Panel::Terminal(panel) => panel.handle_key_event(event),
            Panel::Trace(panel) => panel.handle_key_event(event),
        }
    }

    fn handle_event(&mut self, event: Event) -> Result<EventResponse> {
        match self {
            Panel::Code(panel) => panel.handle_event(event),
            Panel::Display(panel) => panel.handle_event(event),
            Panel::Terminal(panel) => panel.handle_event(event),
            Panel::Trace(panel) => panel.handle_event(event),
        }
    }

    fn on_focus(&mut self) {
        match self {
            Panel::Code(panel) => panel.on_focus(),
            Panel::Display(panel) => panel.on_focus(),
            Panel::Terminal(panel) => panel.on_focus(),
            Panel::Trace(panel) => panel.on_focus(),
        }
    }

    fn on_blur(&mut self) {
        match self {
            Panel::Code(panel) => panel.on_blur(),
            Panel::Display(panel) => panel.on_blur(),
            Panel::Terminal(panel) => panel.on_blur(),
            Panel::Trace(panel) => panel.on_blur(),
        }
    }

    fn panel_type(&self) -> PanelType {
        match self {
            Panel::Code(_) => PanelType::Code,
            Panel::Display(_) => PanelType::Display,
            Panel::Terminal(_) => PanelType::Terminal,
            Panel::Trace(_) => PanelType::Trace,
        }
    }

    fn title(&self) -> String {
        match self {
            Panel::Code(_) => "Code".to_string(),
            Panel::Display(_) => "Display".to_string(),
            Panel::Terminal(_) => "Terminal".to_string(),
            Panel::Trace(_) => "Trace".to_string(),
        }
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        match self {
            Panel::Code(panel) => panel.as_any_mut(),
            Panel::Display(panel) => panel.as_any_mut(),
            Panel::Terminal(panel) => panel.as_any_mut(),
            Panel::Trace(panel) => panel.as_any_mut(),
        }
    }

    fn exec_mgr(&self) -> RwLockReadGuard<'_, ExecutionManager> {
        match self {
            Panel::Code(panel) => panel.exec_mgr(),
            Panel::Display(panel) => panel.exec_mgr(),
            Panel::Terminal(panel) => panel.exec_mgr(),
            Panel::Trace(panel) => panel.exec_mgr(),
        }
    }

    fn exec_mgr_mut(&self) -> RwLockWriteGuard<'_, ExecutionManager> {
        match self {
            Panel::Code(panel) => panel.exec_mgr_mut(),
            Panel::Display(panel) => panel.exec_mgr_mut(),
            Panel::Terminal(panel) => panel.exec_mgr_mut(),
            Panel::Trace(panel) => panel.exec_mgr_mut(),
        }
    }

    fn res_mgr(&self) -> RwLockReadGuard<'_, ResourceManager> {
        match self {
            Panel::Code(panel) => panel.res_mgr(),
            Panel::Display(panel) => panel.res_mgr(),
            Panel::Terminal(panel) => panel.res_mgr(),
            Panel::Trace(panel) => panel.res_mgr(),
        }
    }

    fn res_mgr_mut(&self) -> RwLockWriteGuard<'_, ResourceManager> {
        match self {
            Panel::Code(panel) => panel.res_mgr_mut(),
            Panel::Display(panel) => panel.res_mgr_mut(),
            Panel::Terminal(panel) => panel.res_mgr_mut(),
            Panel::Trace(panel) => panel.res_mgr_mut(),
        }
    }

    fn theme_mgr(&self) -> RwLockReadGuard<'_, ThemeManager> {
        match self {
            Panel::Code(panel) => panel.theme_mgr(),
            Panel::Display(panel) => panel.theme_mgr(),
            Panel::Terminal(panel) => panel.theme_mgr(),
            Panel::Trace(panel) => panel.theme_mgr(),
        }
    }

    fn theme_mgr_mut(&self) -> RwLockWriteGuard<'_, ThemeManager> {
        match self {
            Panel::Code(panel) => panel.theme_mgr_mut(),
            Panel::Display(panel) => panel.theme_mgr_mut(),
            Panel::Terminal(panel) => panel.theme_mgr_mut(),
            Panel::Trace(panel) => panel.theme_mgr_mut(),
        }
    }

    async fn fetch_data(&mut self) -> Result<()> {
        match self {
            Panel::Code(panel) => panel.fetch_data().await,
            Panel::Display(panel) => panel.fetch_data().await,
            Panel::Terminal(panel) => panel.fetch_data().await,
            Panel::Trace(panel) => panel.fetch_data().await,
        }
    }
}
