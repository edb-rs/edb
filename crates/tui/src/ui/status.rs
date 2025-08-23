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


//! Enhanced status icon system for the TUI
//!
//! Provides comprehensive status indicators with contextual icons and animations

use crate::ui::icons::Icons;
use ratatui::style::{Color, Style};

/// Connection status with appropriate icons and colors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStatus {
    /// Connected to RPC server
    Connected,
    /// Connecting to RPC server
    Connecting,
    /// Disconnected from RPC server
    Disconnected,
    /// Connection error occurred
    Error,
}

impl ConnectionStatus {
    /// Get the appropriate icon for this connection status
    pub fn icon(&self) -> &'static str {
        match self {
            ConnectionStatus::Connected => "🟢",
            ConnectionStatus::Connecting => "🟡",
            ConnectionStatus::Disconnected => "🔴",
            ConnectionStatus::Error => "❌",
        }
    }

    /// Get the appropriate color for this connection status
    pub fn color(&self) -> Color {
        match self {
            ConnectionStatus::Connected => Color::Green,
            ConnectionStatus::Connecting => Color::Yellow,
            ConnectionStatus::Disconnected => Color::Red,
            ConnectionStatus::Error => Color::Red,
        }
    }

    /// Get a descriptive text for this connection status
    pub fn text(&self) -> &'static str {
        match self {
            ConnectionStatus::Connected => "Connected",
            ConnectionStatus::Connecting => "Connecting",
            ConnectionStatus::Disconnected => "Disconnected",
            ConnectionStatus::Error => "Connection Error",
        }
    }

    /// Get formatted status display with icon and text
    pub fn display(&self) -> String {
        format!("{} {}", self.icon(), self.text())
    }
}

/// RPC operation status with contextual feedback
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RpcStatus {
    /// Operation completed successfully
    Success,
    /// Operation failed with error
    Error,
    /// Operation in progress
    Loading,
    /// Operation timed out
    Timeout,
    /// No operation (idle state)
    Idle,
}

impl RpcStatus {
    /// Get the appropriate icon for this RPC status
    pub fn icon(&self) -> &'static str {
        match self {
            RpcStatus::Success => Icons::SUCCESS,
            RpcStatus::Error => Icons::ERROR,
            RpcStatus::Loading => Icons::PROCESSING,
            RpcStatus::Timeout => Icons::WARNING,
            RpcStatus::Idle => "⏸️",
        }
    }

    /// Get the appropriate color for this RPC status
    pub fn color(&self) -> Color {
        match self {
            RpcStatus::Success => Color::Green,
            RpcStatus::Error => Color::Red,
            RpcStatus::Loading => Color::Blue,
            RpcStatus::Timeout => Color::Yellow,
            RpcStatus::Idle => Color::Gray,
        }
    }

    /// Get formatted display with icon
    pub fn display(&self, operation: &str) -> String {
        match self {
            RpcStatus::Success => format!("{} {}", self.icon(), operation),
            RpcStatus::Error => format!("{} {} Failed", self.icon(), operation),
            RpcStatus::Loading => format!("{} {}...", self.icon(), operation),
            RpcStatus::Timeout => format!("{} {} Timeout", self.icon(), operation),
            RpcStatus::Idle => "Ready".to_string(),
        }
    }
}

/// Debug execution status with appropriate visual feedback
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionStatus {
    /// Currently executing/stepping through code
    Running,
    /// Paused at breakpoint
    Paused,
    /// Execution completed successfully
    Finished,
    /// Execution failed with error
    Failed,
    /// At start of execution
    Start,
    /// At end of execution
    End,
}

impl ExecutionStatus {
    /// Get the appropriate icon for this execution status
    pub fn icon(&self) -> &'static str {
        match self {
            ExecutionStatus::Running => "▶️",
            ExecutionStatus::Paused => "⏸️",
            ExecutionStatus::Finished => Icons::SUCCESS,
            ExecutionStatus::Failed => Icons::ERROR,
            ExecutionStatus::Start => "🏁",
            ExecutionStatus::End => "🏁",
        }
    }

    /// Get the appropriate color for this execution status
    pub fn color(&self) -> Color {
        match self {
            ExecutionStatus::Running => Color::Green,
            ExecutionStatus::Paused => Color::Yellow,
            ExecutionStatus::Finished => Color::Green,
            ExecutionStatus::Failed => Color::Red,
            ExecutionStatus::Start => Color::Blue,
            ExecutionStatus::End => Color::Blue,
        }
    }

    /// Get formatted display text
    pub fn display(&self) -> String {
        match self {
            ExecutionStatus::Running => format!("{} Running", self.icon()),
            ExecutionStatus::Paused => format!("{} Paused", self.icon()),
            ExecutionStatus::Finished => format!("{} Finished", self.icon()),
            ExecutionStatus::Failed => format!("{} Failed", self.icon()),
            ExecutionStatus::Start => format!("{} At Start", self.icon()),
            ExecutionStatus::End => format!("{} At End", self.icon()),
        }
    }
}

/// Panel focus status with visual indicators
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelStatus {
    /// Panel is currently focused
    Focused,
    /// Panel is not focused
    Unfocused,
    /// Panel has pending updates
    HasUpdates,
    /// Panel is in error state
    Error,
}

impl PanelStatus {
    /// Get the appropriate indicator for this panel status
    pub fn indicator(&self) -> &'static str {
        match self {
            PanelStatus::Focused => "●",
            PanelStatus::Unfocused => "○",
            PanelStatus::HasUpdates => "◉",
            PanelStatus::Error => "⚠",
        }
    }

    /// Get the appropriate color for this panel status
    pub fn color(&self) -> Color {
        match self {
            PanelStatus::Focused => Color::Cyan,
            PanelStatus::Unfocused => Color::Gray,
            PanelStatus::HasUpdates => Color::Yellow,
            PanelStatus::Error => Color::Red,
        }
    }
}

/// File status with contextual icons
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStatus {
    /// Source code is available
    SourceAvailable,
    /// Only opcodes available
    OpcodesOnly,
    /// File contains current execution
    HasExecution,
    /// File has been modified
    Modified,
    /// File is read-only
    ReadOnly,
    /// File not found
    NotFound,
}

impl FileStatus {
    /// Get the appropriate icon for this file status
    pub fn icon(&self) -> &'static str {
        match self {
            FileStatus::SourceAvailable => Icons::FILE,
            FileStatus::OpcodesOnly => "🔧",
            FileStatus::HasExecution => "►",
            FileStatus::Modified => "📝",
            FileStatus::ReadOnly => "🔒",
            FileStatus::NotFound => "❓",
        }
    }

    /// Get the appropriate color for this file status
    pub fn color(&self) -> Color {
        match self {
            FileStatus::SourceAvailable => Color::Green,
            FileStatus::OpcodesOnly => Color::Yellow,
            FileStatus::HasExecution => Color::Cyan,
            FileStatus::Modified => Color::Blue,
            FileStatus::ReadOnly => Color::Gray,
            FileStatus::NotFound => Color::Red,
        }
    }

    /// Get formatted display with icon
    pub fn display(&self, filename: &str) -> String {
        format!("{} {}", self.icon(), filename)
    }
}

/// Breakpoint status with visual feedback
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakpointStatus {
    /// Breakpoint is active
    Active,
    /// Breakpoint is disabled
    Disabled,
    /// Breakpoint hit during execution
    Hit,
    /// Invalid breakpoint location
    Invalid,
}

impl BreakpointStatus {
    /// Get the appropriate icon for this breakpoint status
    pub fn icon(&self) -> &'static str {
        match self {
            BreakpointStatus::Active => "●",
            BreakpointStatus::Disabled => "○",
            BreakpointStatus::Hit => "◉",
            BreakpointStatus::Invalid => "⚠",
        }
    }

    /// Get the appropriate color for this breakpoint status
    pub fn color(&self) -> Color {
        match self {
            BreakpointStatus::Active => Color::Red,
            BreakpointStatus::Disabled => Color::Gray,
            BreakpointStatus::Hit => Color::Yellow,
            BreakpointStatus::Invalid => Color::Red,
        }
    }

    /// Get styled span for rendering
    pub fn styled_span(&self) -> ratatui::text::Span<'static> {
        ratatui::text::Span::styled(self.icon(), Style::default().fg(self.color()))
    }
}

/// Comprehensive status bar builder
pub struct StatusBar {
    /// Connection status
    connection: Option<ConnectionStatus>,
    /// RPC operation status
    rpc: Option<(RpcStatus, String)>,
    /// Execution status
    execution: Option<ExecutionStatus>,
    /// Current panel
    current_panel: Option<String>,
    /// Additional status messages
    messages: Vec<String>,
}

impl StatusBar {
    /// Create a new status bar builder
    pub fn new() -> Self {
        Self {
            connection: None,
            rpc: None,
            execution: None,
            current_panel: None,
            messages: Vec::new(),
        }
    }

    /// Set connection status
    pub fn connection(mut self, status: ConnectionStatus) -> Self {
        self.connection = Some(status);
        self
    }

    /// Set RPC operation status
    pub fn rpc(mut self, status: RpcStatus, operation: String) -> Self {
        self.rpc = Some((status, operation));
        self
    }

    /// Set execution status
    pub fn execution(mut self, status: ExecutionStatus) -> Self {
        self.execution = Some(status);
        self
    }

    /// Set current panel
    pub fn current_panel(mut self, panel: String) -> Self {
        self.current_panel = Some(panel);
        self
    }

    /// Add a status message
    pub fn message<S: Into<String>>(mut self, msg: S) -> Self {
        self.messages.push(msg.into());
        self
    }

    /// Build the complete status line
    pub fn build(&self) -> String {
        let mut parts = Vec::new();

        // Connection status (always first if present)
        if let Some(conn) = self.connection {
            parts.push(conn.display());
        }

        // RPC operation status
        if let Some((status, op)) = &self.rpc {
            parts.push(status.display(op));
        }

        // Execution status
        if let Some(exec) = self.execution {
            parts.push(exec.display());
        }

        // Current panel
        if let Some(panel) = &self.current_panel {
            parts.push(format!("Panel: {}", panel));
        }

        // Additional messages
        parts.extend(self.messages.clone());

        parts.join(" | ")
    }
}

impl Default for StatusBar {
    fn default() -> Self {
        Self::new()
    }
}
