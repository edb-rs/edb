//! Unicode icons and symbols for visual enhancement
//!
//! This module provides a comprehensive set of Unicode symbols and icons
//! used throughout the TUI for visual appeal and clarity.

/// Collection of Unicode icons used throughout the TUI
#[derive(Debug, Clone)]
pub struct Icons;

impl Icons {
    // Status indicators
    pub const SUCCESS: &'static str = "✅";
    pub const ERROR: &'static str = "❌";
    pub const WARNING: &'static str = "⚠️";
    pub const INFO: &'static str = "ℹ️";
    pub const PROCESSING: &'static str = "🔄";

    // Execution states
    pub const CALL: &'static str = "📞";
    pub const RETURN: &'static str = "↩️";
    pub const REVERT: &'static str = "❌";
    pub const CREATE: &'static str = "🏗️";
    pub const CURRENT_EXECUTION: &'static str = "🔸";
    pub const BREAKPOINT: &'static str = "🔹";
    pub const TARGET_REACHED: &'static str = "🎯";

    // File and code
    pub const FILE: &'static str = "📄";
    pub const FOLDER: &'static str = "📁";
    pub const CODE: &'static str = "💾";
    pub const FUNCTION: &'static str = "⚙️";
    pub const VARIABLE: &'static str = "📊";
    pub const MAPPING: &'static str = "📈";

    // Connection states
    pub const CONNECTED: &'static str = "🔗";
    pub const DISCONNECTED: &'static str = "💔";
    pub const CONNECTING: &'static str = "🔄";

    // Navigation
    pub const ARROW_UP: &'static str = "↑";
    pub const ARROW_DOWN: &'static str = "↓";
    pub const ARROW_LEFT: &'static str = "←";
    pub const ARROW_RIGHT: &'static str = "→";
    pub const CURRENT_LINE: &'static str = "►";

    // Box drawing characters for elegant borders
    pub const BOX_TOP_LEFT: &'static str = "╭";
    pub const BOX_TOP_RIGHT: &'static str = "╮";
    pub const BOX_BOTTOM_LEFT: &'static str = "╰";
    pub const BOX_BOTTOM_RIGHT: &'static str = "╯";
    pub const BOX_HORIZONTAL: &'static str = "─";
    pub const BOX_VERTICAL: &'static str = "│";

    // Tree characters for hierarchical displays
    pub const TREE_BRANCH: &'static str = "├─";
    pub const TREE_LAST_BRANCH: &'static str = "└─";
    pub const TREE_VERTICAL: &'static str = "│";
    pub const TREE_NESTED_BRANCH: &'static str = "┌─";

    // Activity indicators (animated)
    pub const SPINNER_FRAMES: &'static [&'static str] =
        &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

    // Progress bars
    pub const PROGRESS_FULL: &'static str = "█";
    pub const PROGRESS_EMPTY: &'static str = "░";
    pub const PROGRESS_PARTIAL: &'static [&'static str] = &["▏", "▎", "▍", "▌", "▋", "▊", "▉"];

    // Special characters
    pub const BULLET: &'static str = "•";
    pub const DIAMOND: &'static str = "◆";
    pub const CIRCLE: &'static str = "●";
    pub const CIRCLE_EMPTY: &'static str = "○";
    pub const SQUARE: &'static str = "■";
    pub const SQUARE_EMPTY: &'static str = "□";

    // Expand/collapse indicators
    pub const EXPANDED: &'static str = "▼";
    pub const COLLAPSED: &'static str = "►";
    pub const EXPANDABLE: &'static str = "[+]";
    pub const COLLAPSIBLE: &'static str = "[-]";
}
