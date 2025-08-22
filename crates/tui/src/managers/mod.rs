//! Shared state managers for TUI panels
//!
//! This module contains managers that handle shared state between panels.

pub mod execution;
pub mod resource;
pub mod theme;

pub use execution::ExecutionManager;
pub use resource::ResourceManager;
pub use theme::ThemeManager;
