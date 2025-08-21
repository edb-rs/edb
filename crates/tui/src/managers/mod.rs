//! Shared state managers for TUI panels
//!
//! This module contains managers that handle shared state between panels.

pub mod breakpoint;
pub mod execution;
pub mod theme;

pub use breakpoint::BreakpointManager;
pub use execution::ExecutionManager;
pub use theme::ThemeManager;
