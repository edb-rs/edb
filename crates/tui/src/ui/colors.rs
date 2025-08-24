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

//! Color schemes and theming system
//!
//! This module provides comprehensive color schemes for the TUI,
//! including multiple professional themes and semantic color mapping.

use ratatui::style::Color;
use serde::{Deserialize, Serialize};

/// Semantic color mapping for consistent theming
#[derive(Debug, Clone)]
pub struct ColorScheme {
    // Panel states
    pub focused_border: Color,
    pub unfocused_border: Color,
    pub panel_bg: Color,

    // Code highlighting
    pub keyword_color: Color,
    pub string_color: Color,
    pub comment_color: Color,
    pub number_color: Color,
    pub identifier_color: Color,
    pub operator_color: Color,

    // Line numbers
    pub line_number: Color,
    pub line_number_bg: Color,

    // Syntax highlighting (detailed)
    pub syntax_keyword_color: Color,
    pub syntax_type_color: Color,
    pub syntax_string_color: Color,
    pub syntax_number_color: Color,
    pub syntax_comment_color: Color,
    pub syntax_identifier_color: Color,
    pub syntax_operator_color: Color,
    pub syntax_punctuation_color: Color,
    pub syntax_address_color: Color,
    pub syntax_pragma_color: Color,
    pub syntax_opcode_color: Color,

    // General highlighting
    pub highlight_bg: Color,
    pub highlight_fg: Color,

    // Help and UI text
    pub help_text_color: Color,

    // Debugging states
    pub current_line_bg: Color,
    pub current_line_fg: Color,
    pub breakpoint_color: Color,
    pub error_color: Color,
    pub success_color: Color,
    pub warning_color: Color,
    pub info_color: Color,

    // Trace visualization
    pub call_color: Color,
    pub return_color: Color,
    pub revert_color: Color,
    pub create_color: Color,

    // Terminal
    pub prompt_color: Color,
    pub command_color: Color,
    pub output_color: Color,
    pub cursor_color: Color,

    // Interactive elements
    pub selection_bg: Color,
    pub selection_fg: Color,
    pub hover_color: Color,
    pub accent_color: Color,
}

/// Available themes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Theme {
    /// Dark cyberpunk theme with purples and neon colors
    CyberpunkDark,
    /// Classic terminal hacker theme with green on black
    TerminalHacker,
    /// Modern IDE theme with soft grays and blues
    ModernIDE,
    /// High contrast theme for accessibility
    HighContrast,
    /// Solarized dark theme with warm orange and yellow accents
    SolarizedDark,
    /// Monokai Pro theme with vibrant colors
    MonokaiPro,
    /// Nord dark theme with cool blue-gray colors inspired by Arctic
    NordDark,
    /// Dracula theme with purple background and pink accents
    DraculaDark,
    /// VS Code light theme with light background
    VSCodeLight,
}

impl Default for Theme {
    fn default() -> Self {
        Theme::ModernIDE
    }
}

impl Theme {
    /// Get all available themes
    pub fn all() -> &'static [Theme] {
        &[
            Theme::CyberpunkDark,
            Theme::TerminalHacker,
            Theme::ModernIDE,
            Theme::HighContrast,
            Theme::SolarizedDark,
            Theme::MonokaiPro,
            Theme::NordDark,
            Theme::DraculaDark,
            Theme::VSCodeLight,
        ]
    }

    /// Get internal name for configuration files
    pub fn name(&self) -> &'static str {
        match self {
            Theme::CyberpunkDark => "neon_purple",
            Theme::TerminalHacker => "matrix_green",
            Theme::ModernIDE => "vscode_dark",
            Theme::HighContrast => "accessibility",
            Theme::SolarizedDark => "solarized_dark",
            Theme::MonokaiPro => "monokai_pro",
            Theme::NordDark => "nord_dark",
            Theme::DraculaDark => "dracula_dark",
            Theme::VSCodeLight => "vscode_light",
        }
    }

    /// Get display name for UI
    pub fn display_name(&self) -> &'static str {
        match self {
            Theme::CyberpunkDark => "Neon Purple",
            Theme::TerminalHacker => "Matrix Green",
            Theme::ModernIDE => "VS Code Dark",
            Theme::HighContrast => "High Contrast",
            Theme::SolarizedDark => "Solarized Dark",
            Theme::MonokaiPro => "Monokai Pro",
            Theme::NordDark => "Nord Dark",
            Theme::DraculaDark => "Dracula",
            Theme::VSCodeLight => "VS Code Light",
        }
    }

    /// Get detailed theme description
    pub fn description(&self) -> &'static str {
        match self {
            Theme::CyberpunkDark => "Neon purple cyberpunk theme",
            Theme::TerminalHacker => "Classic green terminal theme",
            Theme::ModernIDE => "Professional VS Code dark theme",
            Theme::HighContrast => "High contrast accessibility theme",
            Theme::SolarizedDark => "Warm Solarized dark theme",
            Theme::MonokaiPro => "Vibrant Monokai theme",
            Theme::NordDark => "Cool Arctic-inspired theme",
            Theme::DraculaDark => "Purple Dracula theme",
            Theme::VSCodeLight => "Clean VS Code light theme",
        }
    }

    /// Get the next theme in the cycle
    pub fn next(&self) -> Theme {
        match self {
            Theme::CyberpunkDark => Theme::TerminalHacker,
            Theme::TerminalHacker => Theme::ModernIDE,
            Theme::ModernIDE => Theme::HighContrast,
            Theme::HighContrast => Theme::SolarizedDark,
            Theme::SolarizedDark => Theme::MonokaiPro,
            Theme::MonokaiPro => Theme::NordDark,
            Theme::NordDark => Theme::DraculaDark,
            Theme::DraculaDark => Theme::VSCodeLight,
            Theme::VSCodeLight => Theme::CyberpunkDark,
        }
    }
}

impl From<Theme> for ColorScheme {
    fn from(theme: Theme) -> Self {
        match theme {
            Theme::CyberpunkDark => cyberpunk_dark(),
            Theme::TerminalHacker => terminal_hacker(),
            Theme::ModernIDE => modern_ide(),
            Theme::HighContrast => high_contrast(),
            Theme::SolarizedDark => solarized_dark(),
            Theme::MonokaiPro => monokai_pro(),
            Theme::NordDark => nord_dark(),
            Theme::DraculaDark => dracula_dark(),
            Theme::VSCodeLight => vscode_light(),
        }
    }
}

impl Default for ColorScheme {
    fn default() -> Self {
        Theme::ModernIDE.into()
    }
}

/// Cyberpunk dark theme with deep purples, electric blues, and neon greens
fn cyberpunk_dark() -> ColorScheme {
    ColorScheme {
        // Panel states
        focused_border: Color::Rgb(138, 43, 226), // Blue-violet
        unfocused_border: Color::Rgb(75, 75, 75), // Dim gray
        panel_bg: Color::Rgb(20, 20, 30),         // Very dark blue-gray

        // Code highlighting
        keyword_color: Color::Rgb(255, 20, 147),  // Deep pink
        string_color: Color::Rgb(0, 255, 127),    // Spring green
        comment_color: Color::Rgb(108, 108, 108), // Gray
        number_color: Color::Rgb(255, 165, 0),    // Orange
        identifier_color: Color::Rgb(135, 206, 250), // Light sky blue
        operator_color: Color::Rgb(255, 255, 255), // White

        // Line numbers
        line_number: Color::Rgb(100, 100, 100), // Muted gray
        line_number_bg: Color::Rgb(20, 20, 30), // Same as panel bg

        // Syntax highlighting (detailed)
        syntax_keyword_color: Color::Rgb(255, 20, 147), // Deep pink (keywords)
        syntax_type_color: Color::Rgb(255, 105, 180),   // Hot pink (types)
        syntax_string_color: Color::Rgb(0, 255, 127),   // Spring green (strings)
        syntax_number_color: Color::Rgb(255, 165, 0),   // Orange (numbers)
        syntax_comment_color: Color::Rgb(108, 108, 108), // Gray (comments)
        syntax_identifier_color: Color::Rgb(135, 206, 250), // Light sky blue (identifiers)
        syntax_operator_color: Color::Rgb(255, 255, 255), // White (operators)
        syntax_punctuation_color: Color::Rgb(220, 220, 220), // Light gray (punctuation)
        syntax_address_color: Color::Rgb(255, 215, 0),  // Gold (addresses)
        syntax_pragma_color: Color::Rgb(186, 85, 211),  // Medium orchid (pragmas)
        syntax_opcode_color: Color::Rgb(0, 255, 255),   // Cyan (opcodes)

        // General highlighting
        highlight_bg: Color::Rgb(60, 40, 100), // Darker slate blue
        highlight_fg: Color::Rgb(255, 255, 255), // White

        // Help and UI text
        help_text_color: Color::Rgb(160, 160, 160), // Light gray

        // Debugging states
        current_line_bg: Color::Rgb(100, 20, 50), // Dark magenta
        current_line_fg: Color::Rgb(255, 255, 255), // White
        breakpoint_color: Color::Rgb(255, 69, 0), // Red-orange
        error_color: Color::Rgb(220, 20, 60),     // Crimson
        success_color: Color::Rgb(50, 205, 50),   // Lime green
        warning_color: Color::Rgb(255, 215, 0),   // Gold
        info_color: Color::Rgb(30, 144, 255),     // Dodger blue

        // Trace visualization
        call_color: Color::Rgb(0, 255, 255),    // Cyan
        return_color: Color::Rgb(50, 205, 50),  // Lime green
        revert_color: Color::Rgb(255, 69, 0),   // Red-orange
        create_color: Color::Rgb(255, 20, 147), // Deep pink

        // Terminal
        prompt_color: Color::Rgb(138, 43, 226), // Blue-violet
        command_color: Color::Rgb(255, 255, 255), // White
        output_color: Color::Rgb(192, 192, 192), // Silver
        cursor_color: Color::Rgb(0, 255, 0),    // Lime

        // Interactive elements
        selection_bg: Color::Rgb(60, 40, 100), // Darker slate blue
        selection_fg: Color::Rgb(255, 255, 255), // White
        hover_color: Color::Rgb(147, 112, 219), // Medium purple
        accent_color: Color::Rgb(0, 255, 255), // Cyan
    }
}

/// Terminal hacker theme with classic green on black
fn terminal_hacker() -> ColorScheme {
    ColorScheme {
        // Panel states
        focused_border: Color::Rgb(0, 255, 0),   // Lime
        unfocused_border: Color::Rgb(0, 128, 0), // Green
        panel_bg: Color::Rgb(0, 0, 0),           // Black

        // Code highlighting
        keyword_color: Color::Rgb(0, 255, 0),        // Lime
        string_color: Color::Rgb(0, 255, 127),       // Spring green
        comment_color: Color::Rgb(0, 128, 0),        // Green
        number_color: Color::Rgb(0, 255, 255),       // Cyan
        identifier_color: Color::Rgb(255, 255, 255), // White
        operator_color: Color::Rgb(0, 255, 0),       // Lime

        // Line numbers
        line_number: Color::Rgb(0, 128, 0),  // Green
        line_number_bg: Color::Rgb(0, 0, 0), // Black

        // Syntax highlighting (detailed)
        syntax_keyword_color: Color::Rgb(0, 255, 0), // Lime (keywords)
        syntax_type_color: Color::Rgb(0, 255, 127),  // Spring green (types)
        syntax_string_color: Color::Rgb(0, 255, 127), // Spring green (strings)
        syntax_number_color: Color::Rgb(0, 255, 255), // Cyan (numbers)
        syntax_comment_color: Color::Rgb(0, 128, 0), // Green (comments)
        syntax_identifier_color: Color::Rgb(255, 255, 255), // White (identifiers)
        syntax_operator_color: Color::Rgb(0, 255, 0), // Lime (operators)
        syntax_punctuation_color: Color::Rgb(0, 192, 0), // Light green (punctuation)
        syntax_address_color: Color::Rgb(255, 255, 0), // Yellow (addresses)
        syntax_pragma_color: Color::Rgb(0, 255, 255), // Cyan (pragmas)
        syntax_opcode_color: Color::Rgb(0, 255, 255), // Cyan (opcodes)

        // General highlighting
        highlight_bg: Color::Rgb(0, 80, 0),      // Darker green
        highlight_fg: Color::Rgb(255, 255, 255), // White

        // Help and UI text
        help_text_color: Color::Rgb(0, 192, 0), // Light green

        // Debugging states
        current_line_bg: Color::Rgb(0, 100, 50), // Darker green-teal
        current_line_fg: Color::Rgb(255, 255, 255), // White
        breakpoint_color: Color::Rgb(255, 0, 0), // Red
        error_color: Color::Rgb(255, 0, 0),      // Red
        success_color: Color::Rgb(0, 255, 0),    // Lime
        warning_color: Color::Rgb(255, 255, 0),  // Yellow
        info_color: Color::Rgb(0, 255, 255),     // Cyan

        // Trace visualization
        call_color: Color::Rgb(0, 255, 255),   // Cyan
        return_color: Color::Rgb(0, 255, 0),   // Lime
        revert_color: Color::Rgb(255, 0, 0),   // Red
        create_color: Color::Rgb(255, 255, 0), // Yellow

        // Terminal
        prompt_color: Color::Rgb(0, 255, 0),      // Lime
        command_color: Color::Rgb(255, 255, 255), // White
        output_color: Color::Rgb(0, 255, 0),      // Lime
        cursor_color: Color::Rgb(0, 255, 0),      // Lime

        // Interactive elements
        selection_bg: Color::Rgb(0, 80, 0),      // Darker green
        selection_fg: Color::Rgb(255, 255, 255), // White
        hover_color: Color::Rgb(0, 192, 0),      // Light green
        accent_color: Color::Rgb(0, 255, 255),   // Cyan
    }
}

/// Modern IDE theme with soft grays and subtle colors
fn modern_ide() -> ColorScheme {
    ColorScheme {
        // Panel states
        focused_border: Color::Rgb(0, 122, 204), // VS Code blue
        unfocused_border: Color::Rgb(60, 60, 60), // Dark gray
        panel_bg: Color::Rgb(30, 30, 30),        // Dark gray

        // Code highlighting
        keyword_color: Color::Rgb(86, 156, 214), // Light blue
        string_color: Color::Rgb(206, 145, 120), // Light orange
        comment_color: Color::Rgb(106, 153, 85), // Green
        number_color: Color::Rgb(181, 206, 168), // Light green
        identifier_color: Color::Rgb(220, 220, 170), // Light yellow
        operator_color: Color::Rgb(212, 212, 212), // Light gray

        // Line numbers
        line_number: Color::Rgb(128, 128, 128), // Gray
        line_number_bg: Color::Rgb(30, 30, 30), // Same as panel bg

        // Syntax highlighting (detailed)
        syntax_keyword_color: Color::Rgb(86, 156, 214), // Light blue (keywords)
        syntax_type_color: Color::Rgb(78, 201, 176),    // Teal (types)
        syntax_string_color: Color::Rgb(206, 145, 120), // Light orange (strings)
        syntax_number_color: Color::Rgb(181, 206, 168), // Light green (numbers)
        syntax_comment_color: Color::Rgb(106, 153, 85), // Green (comments)
        syntax_identifier_color: Color::Rgb(220, 220, 170), // Light yellow (identifiers)
        syntax_operator_color: Color::Rgb(212, 212, 212), // Light gray (operators)
        syntax_punctuation_color: Color::Rgb(200, 200, 200), // Light gray (punctuation)
        syntax_address_color: Color::Rgb(255, 204, 102), // Yellow (addresses)
        syntax_pragma_color: Color::Rgb(197, 134, 192), // Purple (pragmas)
        syntax_opcode_color: Color::Rgb(86, 156, 214),  // Light blue (opcodes)

        // General highlighting
        highlight_bg: Color::Rgb(0, 90, 160), // Darker VS Code blue
        highlight_fg: Color::Rgb(255, 255, 255), // White

        // Help and UI text
        help_text_color: Color::Rgb(160, 160, 160), // Light gray

        // Debugging states
        current_line_bg: Color::Rgb(80, 60, 0), // Dark orange
        current_line_fg: Color::Rgb(255, 255, 255), // White
        breakpoint_color: Color::Rgb(244, 71, 71), // Red
        error_color: Color::Rgb(244, 71, 71),   // Red
        success_color: Color::Rgb(106, 153, 85), // Green
        warning_color: Color::Rgb(255, 204, 102), // Yellow
        info_color: Color::Rgb(86, 156, 214),   // Blue

        // Trace visualization
        call_color: Color::Rgb(78, 201, 176),    // Teal
        return_color: Color::Rgb(106, 153, 85),  // Green
        revert_color: Color::Rgb(244, 71, 71),   // Red
        create_color: Color::Rgb(197, 134, 192), // Purple

        // Terminal
        prompt_color: Color::Rgb(0, 122, 204), // VS Code blue
        command_color: Color::Rgb(212, 212, 212), // Light gray
        output_color: Color::Rgb(204, 204, 204), // Gray
        cursor_color: Color::Rgb(255, 255, 255), // White

        // Interactive elements
        selection_bg: Color::Rgb(0, 90, 160), // Darker VS Code blue
        selection_fg: Color::Rgb(255, 255, 255), // White
        hover_color: Color::Rgb(60, 60, 60),  // Dark gray
        accent_color: Color::Rgb(0, 122, 204), // VS Code blue
    }
}

/// High contrast theme for accessibility
fn high_contrast() -> ColorScheme {
    ColorScheme {
        // Panel states
        focused_border: Color::Rgb(255, 255, 255), // White
        unfocused_border: Color::Rgb(128, 128, 128), // Gray
        panel_bg: Color::Rgb(0, 0, 0),             // Black

        // Code highlighting
        keyword_color: Color::Rgb(255, 255, 0),      // Yellow
        string_color: Color::Rgb(0, 255, 0),         // Green
        comment_color: Color::Rgb(128, 128, 128),    // Gray
        number_color: Color::Rgb(0, 255, 255),       // Cyan
        identifier_color: Color::Rgb(255, 255, 255), // White
        operator_color: Color::Rgb(255, 255, 255),   // White

        // Line numbers
        line_number: Color::Rgb(255, 255, 255), // White
        line_number_bg: Color::Rgb(0, 0, 0),    // Black

        // Syntax highlighting (detailed)
        syntax_keyword_color: Color::Rgb(255, 255, 0), // Yellow (keywords)
        syntax_type_color: Color::Rgb(255, 0, 255),    // Magenta (types)
        syntax_string_color: Color::Rgb(0, 255, 0),    // Green (strings)
        syntax_number_color: Color::Rgb(0, 255, 255),  // Cyan (numbers)
        syntax_comment_color: Color::Rgb(128, 128, 128), // Gray (comments)
        syntax_identifier_color: Color::Rgb(255, 255, 255), // White (identifiers)
        syntax_operator_color: Color::Rgb(255, 255, 255), // White (operators)
        syntax_punctuation_color: Color::Rgb(255, 255, 255), // White (punctuation)
        syntax_address_color: Color::Rgb(255, 255, 0), // Yellow (addresses)
        syntax_pragma_color: Color::Rgb(255, 0, 255),  // Magenta (pragmas)
        syntax_opcode_color: Color::Rgb(0, 255, 255),  // Cyan (opcodes)

        // General highlighting
        highlight_bg: Color::Rgb(200, 200, 200), // Light gray
        highlight_fg: Color::Rgb(0, 0, 0),       // Black

        // Help and UI text
        help_text_color: Color::Rgb(255, 255, 255), // White

        // Debugging states
        current_line_bg: Color::Rgb(180, 180, 180), // Medium gray
        current_line_fg: Color::Rgb(0, 0, 0),       // Black
        breakpoint_color: Color::Rgb(255, 0, 0),    // Red
        error_color: Color::Rgb(255, 0, 0),         // Red
        success_color: Color::Rgb(0, 255, 0),       // Green
        warning_color: Color::Rgb(255, 255, 0),     // Yellow
        info_color: Color::Rgb(0, 0, 255),          // Blue

        // Trace visualization
        call_color: Color::Rgb(0, 255, 255),   // Cyan
        return_color: Color::Rgb(0, 255, 0),   // Green
        revert_color: Color::Rgb(255, 0, 0),   // Red
        create_color: Color::Rgb(255, 0, 255), // Magenta

        // Terminal
        prompt_color: Color::Rgb(255, 255, 255),  // White
        command_color: Color::Rgb(255, 255, 255), // White
        output_color: Color::Rgb(255, 255, 255),  // White
        cursor_color: Color::Rgb(255, 255, 255),  // White

        // Interactive elements
        selection_bg: Color::Rgb(200, 200, 200), // Light gray
        selection_fg: Color::Rgb(0, 0, 0),       // Black
        hover_color: Color::Rgb(128, 128, 128),  // Gray
        accent_color: Color::Rgb(255, 255, 255), // White
    }
}

/// Solarized dark theme with warm orange and yellow accents
fn solarized_dark() -> ColorScheme {
    ColorScheme {
        // Panel states
        focused_border: Color::Rgb(38, 139, 210), // Solarized blue
        unfocused_border: Color::Rgb(88, 110, 117), // Solarized base01
        panel_bg: Color::Rgb(0, 43, 54),          // Solarized base03

        // Code highlighting
        keyword_color: Color::Rgb(133, 153, 0), // Solarized green
        string_color: Color::Rgb(42, 161, 152), // Solarized cyan
        comment_color: Color::Rgb(101, 123, 131), // Solarized base00
        number_color: Color::Rgb(203, 75, 22),  // Solarized orange
        identifier_color: Color::Rgb(131, 148, 150), // Solarized base0
        operator_color: Color::Rgb(147, 161, 161), // Solarized base1

        // Line numbers
        line_number: Color::Rgb(88, 110, 117), // Solarized base01
        line_number_bg: Color::Rgb(0, 43, 54), // Solarized base03

        // Syntax highlighting (detailed)
        syntax_keyword_color: Color::Rgb(133, 153, 0), // Green (keywords)
        syntax_type_color: Color::Rgb(181, 137, 0),    // Yellow (types)
        syntax_string_color: Color::Rgb(42, 161, 152), // Cyan (strings)
        syntax_number_color: Color::Rgb(203, 75, 22),  // Orange (numbers)
        syntax_comment_color: Color::Rgb(101, 123, 131), // Base00 (comments)
        syntax_identifier_color: Color::Rgb(131, 148, 150), // Base0 (identifiers)
        syntax_operator_color: Color::Rgb(147, 161, 161), // Base1 (operators)
        syntax_punctuation_color: Color::Rgb(147, 161, 161), // Base1 (punctuation)
        syntax_address_color: Color::Rgb(203, 75, 22), // Orange (addresses)
        syntax_pragma_color: Color::Rgb(108, 113, 196), // Violet (pragmas)
        syntax_opcode_color: Color::Rgb(38, 139, 210), // Blue (opcodes)

        // General highlighting
        highlight_bg: Color::Rgb(30, 100, 160), // Darker solarized blue
        highlight_fg: Color::Rgb(253, 246, 227), // Solarized base3

        // Help and UI text
        help_text_color: Color::Rgb(131, 148, 150), // Solarized base0

        // Debugging states
        current_line_bg: Color::Rgb(100, 50, 0), // Dark orange-brown
        current_line_fg: Color::Rgb(253, 246, 227), // Solarized base3
        breakpoint_color: Color::Rgb(220, 50, 47), // Solarized red
        error_color: Color::Rgb(220, 50, 47),    // Solarized red
        success_color: Color::Rgb(133, 153, 0),  // Solarized green
        warning_color: Color::Rgb(181, 137, 0),  // Solarized yellow
        info_color: Color::Rgb(38, 139, 210),    // Solarized blue

        // Trace visualization
        call_color: Color::Rgb(42, 161, 152),    // Cyan
        return_color: Color::Rgb(133, 153, 0),   // Green
        revert_color: Color::Rgb(220, 50, 47),   // Red
        create_color: Color::Rgb(108, 113, 196), // Violet

        // Terminal
        prompt_color: Color::Rgb(38, 139, 210),   // Blue
        command_color: Color::Rgb(147, 161, 161), // Base1
        output_color: Color::Rgb(131, 148, 150),  // Base0
        cursor_color: Color::Rgb(220, 50, 47),    // Red

        // Interactive elements
        selection_bg: Color::Rgb(30, 100, 160), // Darker solarized blue
        selection_fg: Color::Rgb(253, 246, 227), // Base3
        hover_color: Color::Rgb(88, 110, 117),  // Base01
        accent_color: Color::Rgb(203, 75, 22),  // Orange
    }
}

/// Monokai Pro theme with vibrant colors
fn monokai_pro() -> ColorScheme {
    ColorScheme {
        // Panel states
        focused_border: Color::Rgb(102, 217, 239), // Monokai cyan
        unfocused_border: Color::Rgb(90, 90, 90),  // Dark gray
        panel_bg: Color::Rgb(45, 42, 46),          // Monokai background

        // Code highlighting
        keyword_color: Color::Rgb(249, 38, 114), // Monokai pink
        string_color: Color::Rgb(230, 219, 116), // Monokai yellow
        comment_color: Color::Rgb(117, 113, 94), // Monokai comment
        number_color: Color::Rgb(174, 129, 255), // Monokai purple
        identifier_color: Color::Rgb(248, 248, 242), // Monokai foreground
        operator_color: Color::Rgb(249, 38, 114), // Monokai pink

        // Line numbers
        line_number: Color::Rgb(90, 90, 90),    // Gray
        line_number_bg: Color::Rgb(45, 42, 46), // Monokai background

        // Syntax highlighting (detailed)
        syntax_keyword_color: Color::Rgb(249, 38, 114), // Pink (keywords)
        syntax_type_color: Color::Rgb(102, 217, 239),   // Cyan (types)
        syntax_string_color: Color::Rgb(230, 219, 116), // Yellow (strings)
        syntax_number_color: Color::Rgb(174, 129, 255), // Purple (numbers)
        syntax_comment_color: Color::Rgb(117, 113, 94), // Comment gray
        syntax_identifier_color: Color::Rgb(248, 248, 242), // Foreground
        syntax_operator_color: Color::Rgb(249, 38, 114), // Pink (operators)
        syntax_punctuation_color: Color::Rgb(248, 248, 242), // Foreground
        syntax_address_color: Color::Rgb(230, 219, 116), // Yellow (addresses)
        syntax_pragma_color: Color::Rgb(174, 129, 255), // Purple (pragmas)
        syntax_opcode_color: Color::Rgb(166, 226, 46),  // Green (opcodes)

        // General highlighting
        highlight_bg: Color::Rgb(60, 60, 65), // Even darker gray
        highlight_fg: Color::Rgb(248, 248, 242), // Foreground

        // Help and UI text
        help_text_color: Color::Rgb(248, 248, 242), // Foreground

        // Debugging states
        current_line_bg: Color::Rgb(80, 40, 80), // Dark purple
        current_line_fg: Color::Rgb(248, 248, 242), // Foreground
        breakpoint_color: Color::Rgb(249, 38, 114), // Pink
        error_color: Color::Rgb(249, 38, 114),   // Pink
        success_color: Color::Rgb(166, 226, 46), // Green
        warning_color: Color::Rgb(230, 219, 116), // Yellow
        info_color: Color::Rgb(102, 217, 239),   // Cyan

        // Trace visualization
        call_color: Color::Rgb(102, 217, 239),   // Cyan
        return_color: Color::Rgb(166, 226, 46),  // Green
        revert_color: Color::Rgb(249, 38, 114),  // Pink
        create_color: Color::Rgb(174, 129, 255), // Purple

        // Terminal
        prompt_color: Color::Rgb(249, 38, 114),   // Pink
        command_color: Color::Rgb(248, 248, 242), // Foreground
        output_color: Color::Rgb(248, 248, 242),  // Foreground
        cursor_color: Color::Rgb(102, 217, 239),  // Cyan

        // Interactive elements
        selection_bg: Color::Rgb(60, 60, 65), // Even darker gray
        selection_fg: Color::Rgb(248, 248, 242), // Foreground
        hover_color: Color::Rgb(90, 90, 90),  // Gray
        accent_color: Color::Rgb(249, 38, 114), // Pink
    }
}

/// Nord dark theme with cool blue-gray colors
fn nord_dark() -> ColorScheme {
    ColorScheme {
        // Panel states
        focused_border: Color::Rgb(136, 192, 208), // Nord frost
        unfocused_border: Color::Rgb(76, 86, 106), // Nord polar night
        panel_bg: Color::Rgb(46, 52, 64),          // Nord polar night

        // Code highlighting
        keyword_color: Color::Rgb(129, 161, 193), // Nord frost
        string_color: Color::Rgb(163, 190, 140),  // Nord aurora green
        comment_color: Color::Rgb(76, 86, 106),   // Nord polar night
        number_color: Color::Rgb(180, 142, 173),  // Nord aurora purple
        identifier_color: Color::Rgb(216, 222, 233), // Nord snow storm
        operator_color: Color::Rgb(236, 239, 244), // Nord snow storm

        // Line numbers
        line_number: Color::Rgb(76, 86, 106), // Nord polar night
        line_number_bg: Color::Rgb(46, 52, 64), // Nord polar night

        // Syntax highlighting (detailed)
        syntax_keyword_color: Color::Rgb(129, 161, 193), // Frost (keywords)
        syntax_type_color: Color::Rgb(136, 192, 208),    // Frost (types)
        syntax_string_color: Color::Rgb(163, 190, 140),  // Aurora green (strings)
        syntax_number_color: Color::Rgb(180, 142, 173),  // Aurora purple (numbers)
        syntax_comment_color: Color::Rgb(76, 86, 106),   // Polar night (comments)
        syntax_identifier_color: Color::Rgb(216, 222, 233), // Snow storm (identifiers)
        syntax_operator_color: Color::Rgb(236, 239, 244), // Snow storm (operators)
        syntax_punctuation_color: Color::Rgb(236, 239, 244), // Snow storm (punctuation)
        syntax_address_color: Color::Rgb(235, 203, 139), // Aurora yellow (addresses)
        syntax_pragma_color: Color::Rgb(180, 142, 173),  // Aurora purple (pragmas)
        syntax_opcode_color: Color::Rgb(136, 192, 208),  // Frost (opcodes)

        // General highlighting
        highlight_bg: Color::Rgb(50, 56, 70), // Even darker polar night
        highlight_fg: Color::Rgb(236, 239, 244), // Snow storm

        // Help and UI text
        help_text_color: Color::Rgb(216, 222, 233), // Snow storm

        // Debugging states
        current_line_bg: Color::Rgb(80, 60, 40), // Dark brown-orange
        current_line_fg: Color::Rgb(236, 239, 244), // Snow storm
        breakpoint_color: Color::Rgb(191, 97, 106), // Aurora red
        error_color: Color::Rgb(191, 97, 106),   // Aurora red
        success_color: Color::Rgb(163, 190, 140), // Aurora green
        warning_color: Color::Rgb(235, 203, 139), // Aurora yellow
        info_color: Color::Rgb(129, 161, 193),   // Frost

        // Trace visualization
        call_color: Color::Rgb(136, 192, 208),   // Frost
        return_color: Color::Rgb(163, 190, 140), // Aurora green
        revert_color: Color::Rgb(191, 97, 106),  // Aurora red
        create_color: Color::Rgb(180, 142, 173), // Aurora purple

        // Terminal
        prompt_color: Color::Rgb(129, 161, 193),  // Frost
        command_color: Color::Rgb(236, 239, 244), // Snow storm
        output_color: Color::Rgb(216, 222, 233),  // Snow storm
        cursor_color: Color::Rgb(136, 192, 208),  // Frost

        // Interactive elements
        selection_bg: Color::Rgb(50, 56, 70), // Even darker polar night
        selection_fg: Color::Rgb(236, 239, 244), // Snow storm
        hover_color: Color::Rgb(76, 86, 106), // Polar night
        accent_color: Color::Rgb(136, 192, 208), // Frost
    }
}

/// Dracula theme with purple background and pink accents
fn dracula_dark() -> ColorScheme {
    ColorScheme {
        // Panel states
        focused_border: Color::Rgb(255, 121, 198), // Dracula pink
        unfocused_border: Color::Rgb(98, 114, 164), // Dracula comment
        panel_bg: Color::Rgb(40, 42, 54),          // Dracula background

        // Code highlighting
        keyword_color: Color::Rgb(255, 121, 198), // Dracula pink
        string_color: Color::Rgb(241, 250, 140),  // Dracula yellow
        comment_color: Color::Rgb(98, 114, 164),  // Dracula comment
        number_color: Color::Rgb(189, 147, 249),  // Dracula purple
        identifier_color: Color::Rgb(248, 248, 242), // Dracula foreground
        operator_color: Color::Rgb(255, 184, 108), // Dracula orange

        // Line numbers
        line_number: Color::Rgb(98, 114, 164), // Dracula comment
        line_number_bg: Color::Rgb(40, 42, 54), // Dracula background

        // Syntax highlighting (detailed)
        syntax_keyword_color: Color::Rgb(255, 121, 198), // Pink (keywords)
        syntax_type_color: Color::Rgb(139, 233, 253),    // Cyan (types)
        syntax_string_color: Color::Rgb(241, 250, 140),  // Yellow (strings)
        syntax_number_color: Color::Rgb(189, 147, 249),  // Purple (numbers)
        syntax_comment_color: Color::Rgb(98, 114, 164),  // Comment (comments)
        syntax_identifier_color: Color::Rgb(248, 248, 242), // Foreground (identifiers)
        syntax_operator_color: Color::Rgb(255, 184, 108), // Orange (operators)
        syntax_punctuation_color: Color::Rgb(248, 248, 242), // Foreground (punctuation)
        syntax_address_color: Color::Rgb(255, 184, 108), // Orange (addresses)
        syntax_pragma_color: Color::Rgb(189, 147, 249),  // Purple (pragmas)
        syntax_opcode_color: Color::Rgb(80, 250, 123),   // Green (opcodes)

        // General highlighting
        highlight_bg: Color::Rgb(55, 58, 75), // Darker dracula selection
        highlight_fg: Color::Rgb(248, 248, 242), // Dracula foreground

        // Help and UI text
        help_text_color: Color::Rgb(248, 248, 242), // Dracula foreground

        // Debugging states
        current_line_bg: Color::Rgb(80, 40, 60), // Dark purple-red
        current_line_fg: Color::Rgb(248, 248, 242), // Dracula foreground
        breakpoint_color: Color::Rgb(255, 85, 85), // Dracula red
        error_color: Color::Rgb(255, 85, 85),    // Dracula red
        success_color: Color::Rgb(80, 250, 123), // Dracula green
        warning_color: Color::Rgb(241, 250, 140), // Dracula yellow
        info_color: Color::Rgb(139, 233, 253),   // Dracula cyan

        // Trace visualization
        call_color: Color::Rgb(139, 233, 253),   // Cyan
        return_color: Color::Rgb(80, 250, 123),  // Green
        revert_color: Color::Rgb(255, 85, 85),   // Red
        create_color: Color::Rgb(189, 147, 249), // Purple

        // Terminal
        prompt_color: Color::Rgb(255, 121, 198),  // Pink
        command_color: Color::Rgb(248, 248, 242), // Foreground
        output_color: Color::Rgb(248, 248, 242),  // Foreground
        cursor_color: Color::Rgb(255, 121, 198),  // Pink

        // Interactive elements
        selection_bg: Color::Rgb(55, 58, 75), // Darker selection
        selection_fg: Color::Rgb(248, 248, 242), // Foreground
        hover_color: Color::Rgb(98, 114, 164), // Comment
        accent_color: Color::Rgb(255, 121, 198), // Pink
    }
}

/// VS Code light theme with light background
fn vscode_light() -> ColorScheme {
    ColorScheme {
        // Panel states
        focused_border: Color::Rgb(0, 122, 204), // VS Code blue
        unfocused_border: Color::Rgb(200, 200, 200), // Light gray
        panel_bg: Color::Rgb(255, 255, 255),     // White

        // Code highlighting
        keyword_color: Color::Rgb(0, 0, 255),  // Blue
        string_color: Color::Rgb(163, 21, 21), // Dark red
        comment_color: Color::Rgb(0, 128, 0),  // Green
        number_color: Color::Rgb(9, 134, 88),  // Dark green
        identifier_color: Color::Rgb(0, 0, 0), // Black
        operator_color: Color::Rgb(0, 0, 0),   // Black

        // Line numbers
        line_number: Color::Rgb(128, 128, 128),    // Gray
        line_number_bg: Color::Rgb(255, 255, 255), // White

        // Syntax highlighting (detailed)
        syntax_keyword_color: Color::Rgb(0, 0, 255), // Blue (keywords)
        syntax_type_color: Color::Rgb(43, 145, 175), // Teal (types)
        syntax_string_color: Color::Rgb(163, 21, 21), // Dark red (strings)
        syntax_number_color: Color::Rgb(9, 134, 88), // Dark green (numbers)
        syntax_comment_color: Color::Rgb(0, 128, 0), // Green (comments)
        syntax_identifier_color: Color::Rgb(0, 0, 0), // Black (identifiers)
        syntax_operator_color: Color::Rgb(0, 0, 0),  // Black (operators)
        syntax_punctuation_color: Color::Rgb(0, 0, 0), // Black (punctuation)
        syntax_address_color: Color::Rgb(121, 94, 38), // Brown (addresses)
        syntax_pragma_color: Color::Rgb(175, 0, 219), // Purple (pragmas)
        syntax_opcode_color: Color::Rgb(0, 0, 255),  // Blue (opcodes)

        // General highlighting
        highlight_bg: Color::Rgb(200, 220, 255), // Very light blue
        highlight_fg: Color::Rgb(0, 0, 0),       // Black

        // Help and UI text
        help_text_color: Color::Rgb(106, 106, 106), // Dark gray

        // Debugging states
        current_line_bg: Color::Rgb(255, 200, 100), // Light orange
        current_line_fg: Color::Rgb(0, 0, 0),       // Black
        breakpoint_color: Color::Rgb(205, 49, 49),  // Dark red
        error_color: Color::Rgb(205, 49, 49),       // Dark red
        success_color: Color::Rgb(0, 128, 0),       // Green
        warning_color: Color::Rgb(255, 140, 0),     // Dark orange
        info_color: Color::Rgb(0, 122, 204),        // Blue

        // Trace visualization
        call_color: Color::Rgb(43, 145, 175),  // Teal
        return_color: Color::Rgb(0, 128, 0),   // Green
        revert_color: Color::Rgb(205, 49, 49), // Dark red
        create_color: Color::Rgb(175, 0, 219), // Purple

        // Terminal
        prompt_color: Color::Rgb(0, 122, 204), // Blue
        command_color: Color::Rgb(0, 0, 0),    // Black
        output_color: Color::Rgb(0, 0, 0),     // Black
        cursor_color: Color::Rgb(0, 0, 0),     // Black

        // Interactive elements
        selection_bg: Color::Rgb(200, 220, 255), // Very light blue
        selection_fg: Color::Rgb(0, 0, 0),       // Black
        hover_color: Color::Rgb(229, 229, 229),  // Very light gray
        accent_color: Color::Rgb(0, 122, 204),   // Blue
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_cycle() {
        assert_eq!(Theme::CyberpunkDark.next(), Theme::TerminalHacker);
        assert_eq!(Theme::TerminalHacker.next(), Theme::ModernIDE);
        assert_eq!(Theme::ModernIDE.next(), Theme::HighContrast);
        assert_eq!(Theme::HighContrast.next(), Theme::SolarizedDark);
        assert_eq!(Theme::SolarizedDark.next(), Theme::MonokaiPro);
        assert_eq!(Theme::MonokaiPro.next(), Theme::NordDark);
        assert_eq!(Theme::NordDark.next(), Theme::DraculaDark);
        assert_eq!(Theme::DraculaDark.next(), Theme::VSCodeLight);
        assert_eq!(Theme::VSCodeLight.next(), Theme::CyberpunkDark);
    }

    #[test]
    fn test_theme_names() {
        assert_eq!(Theme::CyberpunkDark.name(), "neon_purple");
        assert_eq!(Theme::TerminalHacker.name(), "matrix_green");
        assert_eq!(Theme::ModernIDE.name(), "vscode_dark");
        assert_eq!(Theme::HighContrast.name(), "accessibility");
        assert_eq!(Theme::SolarizedDark.name(), "solarized_dark");
        assert_eq!(Theme::MonokaiPro.name(), "monokai_pro");
        assert_eq!(Theme::NordDark.name(), "nord_dark");
        assert_eq!(Theme::DraculaDark.name(), "dracula_dark");
        assert_eq!(Theme::VSCodeLight.name(), "vscode_light");
    }

    #[test]
    fn test_theme_display_names() {
        assert_eq!(Theme::CyberpunkDark.display_name(), "Neon Purple");
        assert_eq!(Theme::TerminalHacker.display_name(), "Matrix Green");
        assert_eq!(Theme::ModernIDE.display_name(), "VS Code Dark");
        assert_eq!(Theme::HighContrast.display_name(), "High Contrast");
        assert_eq!(Theme::SolarizedDark.display_name(), "Solarized Dark");
        assert_eq!(Theme::MonokaiPro.display_name(), "Monokai Pro");
        assert_eq!(Theme::NordDark.display_name(), "Nord Dark");
        assert_eq!(Theme::DraculaDark.display_name(), "Dracula");
        assert_eq!(Theme::VSCodeLight.display_name(), "VS Code Light");
    }

    #[test]
    fn test_color_scheme_conversion() {
        let scheme: ColorScheme = Theme::CyberpunkDark.into();
        // Just verify it doesn't panic and has reasonable values
        assert_ne!(scheme.focused_border, scheme.unfocused_border);
    }
}
