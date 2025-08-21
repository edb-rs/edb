//! UI utilities and visual components
//!
//! This module contains reusable UI components, color schemes, animations,
//! and other visual enhancements for the TUI.

pub mod borders;
pub mod colors;
pub mod icons;
pub mod spinner;
pub mod status;

pub use borders::{BorderPresets, EnhancedBorder, EnhancedBorderStyle};
pub use colors::{ColorScheme, Theme};
pub use icons::Icons;
pub use spinner::{RpcSpinner, Spinner, SpinnerStyles};
pub use status::{
    BreakpointStatus, ConnectionStatus, ExecutionStatus, FileStatus, PanelStatus, RpcStatus,
    StatusBar,
};
