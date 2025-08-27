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

//! Panel framework and implementations
//!
//! This module contains the panel trait and all panel implementations.

use crate::managers::DataManager;
use crossterm::event::{KeyEvent, MouseEvent};
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
pub trait PanelTr: Debug + Send {
    /// Render the panel content
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect, data_manager: &mut DataManager);

    /// Handle keyboard events
    fn handle_key_event(
        &mut self,
        event: KeyEvent,
        data_manager: &mut DataManager,
    ) -> Result<EventResponse> {
        let _ = event; // Suppress unused parameter warning
        let _ = data_manager;
        Ok(EventResponse::NotHandled)
    }

    /// Handle mouse events
    fn handle_mouse_event(
        &mut self,
        event: MouseEvent,
        data_manager: &mut DataManager,
    ) -> Result<EventResponse> {
        let _ = event; // Suppress unused parameter warning
        let _ = data_manager;
        Ok(EventResponse::NotHandled)
    }

    /// Called when this panel gains focus
    fn on_focus(&mut self) {}

    /// Called when this panel loses focus
    fn on_blur(&mut self) {}

    /// Get the panel type
    fn panel_type(&self) -> PanelType;

    /// Get panel title for display
    fn title(&self, _data_manager: &mut DataManager) -> String {
        format!("{:?} Panel", self.panel_type())
    }

    /// Allow downcasting to concrete types
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
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
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect, data_manager: &mut DataManager) {
        match self {
            Panel::Code(panel) => panel.render(frame, area, data_manager),
            Panel::Display(panel) => panel.render(frame, area, data_manager),
            Panel::Terminal(panel) => panel.render(frame, area, data_manager),
            Panel::Trace(panel) => panel.render(frame, area, data_manager),
        }
    }

    fn handle_key_event(
        &mut self,
        event: KeyEvent,
        data_manager: &mut DataManager,
    ) -> Result<EventResponse> {
        match self {
            Panel::Code(panel) => panel.handle_key_event(event, data_manager),
            Panel::Display(panel) => panel.handle_key_event(event, data_manager),
            Panel::Terminal(panel) => panel.handle_key_event(event, data_manager),
            Panel::Trace(panel) => panel.handle_key_event(event, data_manager),
        }
    }

    fn handle_mouse_event(
        &mut self,
        event: MouseEvent,
        data_manager: &mut DataManager,
    ) -> Result<EventResponse> {
        match self {
            Panel::Code(panel) => panel.handle_mouse_event(event, data_manager),
            Panel::Display(panel) => panel.handle_mouse_event(event, data_manager),
            Panel::Terminal(panel) => panel.handle_mouse_event(event, data_manager),
            Panel::Trace(panel) => panel.handle_mouse_event(event, data_manager),
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

    fn title(&self, _dm: &mut DataManager) -> String {
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
}
