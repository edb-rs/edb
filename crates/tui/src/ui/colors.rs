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
#[derive(Debug, Clone, Copy, PartialEq, Eq,  Serialize, Deserialize)]
pub enum Theme {
    /// Dark cyberpunk theme with purples and neon colors
    CyberpunkDark,
    /// Classic terminal hacker theme with green on black
    TerminalHacker,
    /// Modern IDE theme with soft grays and blues
    ModernIDE,
    /// High contrast theme for accessibility
    HighContrast,
}

impl Default for Theme {
    fn default() -> Self {
        Theme::CyberpunkDark
    }
}

impl Theme {
    /// Get all available themes
    pub fn all() -> &'static [Theme] {
        &[Theme::CyberpunkDark, Theme::TerminalHacker, Theme::ModernIDE, Theme::HighContrast]
    }

    /// Get name for display (short and lowercases)
    pub fn name(&self) -> &'static str {
        match self {
            Theme::CyberpunkDark => "cyberpunk_dark",
            Theme::TerminalHacker => "terminal_hacker",
            Theme::ModernIDE => "modern_ide",
            Theme::HighContrast => "high_contrast",
        }
    }

    /// Get theme description for display
    pub fn description(&self) -> &'static str {
        match self {
            Theme::CyberpunkDark => "Cyberpunk Dark",
            Theme::TerminalHacker => "Terminal Hacker",
            Theme::ModernIDE => "Modern IDE",
            Theme::HighContrast => "High Contrast",
        }
    }

    /// Get the next theme in the cycle
    pub fn next(&self) -> Theme {
        match self {
            Theme::CyberpunkDark => Theme::TerminalHacker,
            Theme::TerminalHacker => Theme::ModernIDE,
            Theme::ModernIDE => Theme::HighContrast,
            Theme::HighContrast => Theme::CyberpunkDark,
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
        }
    }
}

impl Default for ColorScheme {
    fn default() -> Self {
        Theme::CyberpunkDark.into()
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

        // Debugging states
        current_line_bg: Color::Rgb(75, 0, 130),    // Indigo
        current_line_fg: Color::Rgb(255, 255, 255), // White
        breakpoint_color: Color::Rgb(255, 69, 0),   // Red-orange
        error_color: Color::Rgb(220, 20, 60),       // Crimson
        success_color: Color::Rgb(50, 205, 50),     // Lime green
        warning_color: Color::Rgb(255, 215, 0),     // Gold
        info_color: Color::Rgb(30, 144, 255),       // Dodger blue

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
        selection_bg: Color::Rgb(72, 61, 139), // Dark slate blue
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

        // Debugging states
        current_line_bg: Color::Rgb(0, 64, 0), // Dark green
        current_line_fg: Color::Rgb(255, 255, 255), // White
        breakpoint_color: Color::Rgb(255, 0, 0), // Red
        error_color: Color::Rgb(255, 0, 0),    // Red
        success_color: Color::Rgb(0, 255, 0),  // Lime
        warning_color: Color::Rgb(255, 255, 0), // Yellow
        info_color: Color::Rgb(0, 255, 255),   // Cyan

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
        selection_bg: Color::Rgb(0, 128, 0),     // Green
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

        // Debugging states
        current_line_bg: Color::Rgb(0, 122, 204), // VS Code blue
        current_line_fg: Color::Rgb(255, 255, 255), // White
        breakpoint_color: Color::Rgb(244, 71, 71), // Red
        error_color: Color::Rgb(244, 71, 71),     // Red
        success_color: Color::Rgb(106, 153, 85),  // Green
        warning_color: Color::Rgb(255, 204, 102), // Yellow
        info_color: Color::Rgb(86, 156, 214),     // Blue

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
        selection_bg: Color::Rgb(0, 122, 204),   // VS Code blue
        selection_fg: Color::Rgb(255, 255, 255), // White
        hover_color: Color::Rgb(60, 60, 60),     // Dark gray
        accent_color: Color::Rgb(0, 122, 204),   // VS Code blue
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

        // Debugging states
        current_line_bg: Color::Rgb(255, 255, 255), // White
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
        selection_bg: Color::Rgb(255, 255, 255), // White
        selection_fg: Color::Rgb(0, 0, 0),       // Black
        hover_color: Color::Rgb(128, 128, 128),  // Gray
        accent_color: Color::Rgb(255, 255, 255), // White
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
        assert_eq!(Theme::HighContrast.next(), Theme::CyberpunkDark);
    }

    #[test]
    fn test_theme_names() {
        assert_eq!(Theme::CyberpunkDark.name(), "Cyberpunk Dark");
        assert_eq!(Theme::TerminalHacker.name(), "Terminal Hacker");
        assert_eq!(Theme::ModernIDE.name(), "Modern IDE");
        assert_eq!(Theme::HighContrast.name(), "High Contrast");
    }

    #[test]
    fn test_color_scheme_conversion() {
        let scheme: ColorScheme = Theme::CyberpunkDark.into();
        // Just verify it doesn't panic and has reasonable values
        assert_ne!(scheme.focused_border, scheme.unfocused_border);
    }
}
