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

//! Unicode icons and symbols for visual enhancement
//!
//! This module provides a comprehensive set of Unicode symbols and icons
//! used throughout the TUI for visual appeal and clarity.

/// Collection of Unicode icons used throughout the TUI
#[derive(Debug, Clone)]
pub struct Icons;

impl Icons {
    // Status indicators
    /// Icon for successful operations and completed actions
    pub const SUCCESS: &'static str = "✅";
    /// Icon for errors and failed operations
    pub const ERROR: &'static str = "❌";
    /// Icon for warnings and caution messages
    pub const WARNING: &'static str = "⚠️";
    /// Icon for informational messages
    pub const INFO: &'static str = "ℹ️";
    /// Icon for ongoing processing and loading states
    pub const PROCESSING: &'static str = "🔄";

    // Execution states
    /// Icon for function or contract calls in transaction traces
    pub const CALL: &'static str = "📞";
    /// Icon for function returns in transaction traces
    pub const RETURN: &'static str = "↩️";
    /// Icon for transaction reverts and failed operations
    pub const REVERT: &'static str = "❌";
    /// Icon for contract creation operations
    pub const CREATE: &'static str = "🏗️";
    /// Icon indicating the current execution position
    pub const CURRENT_EXECUTION: &'static str = "🔸";
    /// Icon for breakpoints in the debugger
    pub const BREAKPOINT: &'static str = "🔹";
    /// Icon indicating when a target execution point is reached
    pub const TARGET_REACHED: &'static str = "🎯";

    // File and code
    /// Icon for individual source files
    pub const FILE: &'static str = "📄";
    /// Icon for directories and folders
    pub const FOLDER: &'static str = "📁";
    /// Icon for compiled code and bytecode
    pub const CODE: &'static str = "💾";
    /// Icon for functions and methods
    pub const FUNCTION: &'static str = "⚙️";
    /// Icon for variables and storage items
    pub const VARIABLE: &'static str = "📊";
    /// Icon for mappings and key-value structures
    pub const MAPPING: &'static str = "📈";

    // Connection states
    /// Icon for established RPC connections
    pub const CONNECTED: &'static str = "🔗";
    /// Icon for disconnected or failed connections
    pub const DISCONNECTED: &'static str = "💔";
    /// Icon for connection attempts in progress
    pub const CONNECTING: &'static str = "🔄";

    // Navigation
    /// Up arrow for navigation and scrolling
    pub const ARROW_UP: &'static str = "↑";
    /// Down arrow for navigation and scrolling
    pub const ARROW_DOWN: &'static str = "↓";
    /// Left arrow for navigation and hierarchy
    pub const ARROW_LEFT: &'static str = "←";
    /// Right arrow for navigation and hierarchy
    pub const ARROW_RIGHT: &'static str = "→";
    /// Indicator for the current line in code view
    pub const CURRENT_LINE: &'static str = "►";

    // Box drawing characters for elegant borders
    /// Top-left corner character for rounded boxes
    pub const BOX_TOP_LEFT: &'static str = "╭";
    /// Top-right corner character for rounded boxes
    pub const BOX_TOP_RIGHT: &'static str = "╮";
    /// Bottom-left corner character for rounded boxes
    pub const BOX_BOTTOM_LEFT: &'static str = "╰";
    /// Bottom-right corner character for rounded boxes
    pub const BOX_BOTTOM_RIGHT: &'static str = "╯";
    /// Horizontal line character for box borders
    pub const BOX_HORIZONTAL: &'static str = "─";
    /// Vertical line character for box borders
    pub const BOX_VERTICAL: &'static str = "│";

    // Tree characters for hierarchical displays
    /// Tree branch character for intermediate items
    pub const TREE_BRANCH: &'static str = "├─";
    /// Tree branch character for the last item in a group
    pub const TREE_LAST_BRANCH: &'static str = "└─";
    /// Vertical line character for tree structure continuation
    pub const TREE_VERTICAL: &'static str = "│";
    /// Nested branch character for hierarchical structures
    pub const TREE_NESTED_BRANCH: &'static str = "┌─";

    // Activity indicators (animated)
    /// Animation frames for the loading spinner
    pub const SPINNER_FRAMES: &'static [&'static str] =
        &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

    // Progress bars
    /// Full block character for completed progress sections
    pub const PROGRESS_FULL: &'static str = "█";
    /// Empty block character for incomplete progress sections
    pub const PROGRESS_EMPTY: &'static str = "░";
    /// Partial block characters for fractional progress display
    pub const PROGRESS_PARTIAL: &'static [&'static str] = &["▏", "▎", "▍", "▌", "▋", "▊", "▉"];

    // Special characters
    /// Bullet point character for lists and emphasis
    pub const BULLET: &'static str = "•";
    /// Diamond character for special markers
    pub const DIAMOND: &'static str = "◆";
    /// Filled circle character for active states
    pub const CIRCLE: &'static str = "●";
    /// Empty circle character for inactive states
    pub const CIRCLE_EMPTY: &'static str = "○";
    /// Filled square character for solid indicators
    pub const SQUARE: &'static str = "■";
    /// Empty square character for outline indicators
    pub const SQUARE_EMPTY: &'static str = "□";

    // Expand/collapse indicators
    /// Down arrow indicating an expanded section
    pub const EXPANDED: &'static str = "▼";
    /// Right arrow indicating a collapsed section
    pub const COLLAPSED: &'static str = "►";
    /// Plus sign indicator for expandable content
    pub const EXPANDABLE: &'static str = "[+]";
    /// Minus sign indicator for collapsible content
    pub const COLLAPSIBLE: &'static str = "[-]";
}
