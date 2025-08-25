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

//! UI utilities and visual components
//!
//! This module contains reusable UI components, color schemes, animations,
//! and other visual enhancements for the TUI.

pub mod borders;
pub mod colors;
pub mod icons;
pub mod spinner;
pub mod status;
pub mod syntax;

pub use borders::{BorderPresets, EnhancedBorder, EnhancedBorderStyle};
pub use colors::{ColorScheme, Theme};
pub use icons::Icons;
pub use spinner::{Spinner, SpinnerAnimation, SpinnerStyles};
pub use status::{
    BreakpointStatus, ConnectionStatus, ExecutionStatus, FileStatus, PanelStatus, RpcStatus,
    StatusBar,
};
pub use syntax::{SyntaxHighlighter, SyntaxToken, SyntaxType, TokenType};
