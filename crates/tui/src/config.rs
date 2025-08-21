//! Configuration system for EDB TUI
//!
//! Manages user preferences including color schemes and other settings.

use eyre::{Context, Result};
use ratatui::style::Color;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tracing::{debug, info, warn};

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Current theme configuration
    pub theme: ThemeConfig,
    /// Panel-specific settings
    pub panels: PanelConfig,
}

/// Theme configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeConfig {
    /// Current active theme name
    pub active: String,
    /// Available themes
    pub themes: std::collections::HashMap<String, Theme>,
}

/// Individual theme definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    /// Theme display name
    pub name: String,
    /// Theme description
    pub description: String,
    /// Color scheme for different UI elements
    pub colors: ColorScheme,
}

/// Color scheme definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorScheme {
    /// Focused panel border color
    pub focused_border: String,
    /// Unfocused panel border color
    pub unfocused_border: String,
    /// Selected item background
    pub selected_bg: String,
    /// Selected item foreground
    pub selected_fg: String,
    /// Highlighted line background
    pub highlight_bg: String,
    /// Highlighted line foreground
    pub highlight_fg: String,
    /// Help text color
    pub help_text: String,
    /// Success/positive color
    pub success: String,
    /// Error/negative color
    pub error: String,
    /// Warning color
    pub warning: String,
    /// Information color
    pub info: String,
    /// Accent color for status bars and highlights
    pub accent: String,

    // Syntax highlighting colors
    /// Keywords (contract, function, if, etc.)
    pub keyword: String,
    /// Type names (uint256, address, bool, etc.)
    pub type_name: String,
    /// String literals
    pub string: String,
    /// Number literals
    pub number: String,
    /// Comments
    pub comment: String,
    /// Identifiers (variable names, function names)
    pub identifier: String,
    /// Operators (+, -, *, ==, etc.)
    pub operator: String,
    /// Punctuation ((), {}, ;, etc.)
    pub punctuation: String,
    /// Ethereum addresses (0x...)
    pub address: String,
    /// Pragma statements
    pub pragma: String,
    /// EVM opcodes
    pub opcode: String,

    // Line number styling
    /// Line numbers
    pub line_number: String,
    /// Line number background
    pub line_number_bg: String,
}

/// Panel-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelConfig {
    /// Terminal panel settings
    pub terminal: TerminalPanelConfig,
    /// Code panel settings
    pub code: CodePanelConfig,
    /// Trace panel settings
    pub trace: TracePanelConfig,
    /// Display panel settings
    pub display: DisplayPanelConfig,
}

/// Terminal panel configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalPanelConfig {
    /// Maximum number of history lines to keep
    pub max_history: usize,
    /// Show timestamps in output
    pub show_timestamps: bool,
}

/// Code panel configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodePanelConfig {
    /// Show line numbers
    pub show_line_numbers: bool,
    /// Highlight current line
    pub highlight_current_line: bool,
}

/// Trace panel configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TracePanelConfig {
    /// Show trace depth indicators
    pub show_depth_indicators: bool,
    /// Maximum trace entries to display
    pub max_entries: usize,
}

/// Display panel configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayPanelConfig {
    /// Default display mode on startup
    pub default_mode: String,
    /// Show variable types
    pub show_types: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self { theme: ThemeConfig::default(), panels: PanelConfig::default() }
    }
}

impl Default for ThemeConfig {
    fn default() -> Self {
        let mut themes = std::collections::HashMap::new();

        // Default theme
        themes.insert(
            "default".to_string(),
            Theme {
                name: "Default".to_string(),
                description: "Default EDB theme with blue accents".to_string(),
                colors: ColorScheme {
                    focused_border: "cyan".to_string(),
                    unfocused_border: "gray".to_string(),
                    selected_bg: "blue".to_string(),
                    selected_fg: "white".to_string(),
                    highlight_bg: "dark_gray".to_string(),
                    highlight_fg: "white".to_string(),
                    help_text: "yellow".to_string(),
                    success: "green".to_string(),
                    error: "red".to_string(),
                    warning: "yellow".to_string(),
                    info: "cyan".to_string(),
                    accent: "bright_cyan".to_string(),
                    // Syntax highlighting
                    keyword: "light_blue".to_string(),
                    type_name: "light_green".to_string(),
                    string: "green".to_string(),
                    number: "light_yellow".to_string(),
                    comment: "dark_gray".to_string(),
                    identifier: "white".to_string(),
                    operator: "light_magenta".to_string(),
                    punctuation: "gray".to_string(),
                    address: "light_cyan".to_string(),
                    pragma: "magenta".to_string(),
                    opcode: "light_blue".to_string(),
                    line_number: "gray".to_string(),
                    line_number_bg: "black".to_string(),
                },
            },
        );

        // Dark theme
        themes.insert(
            "dark".to_string(),
            Theme {
                name: "Dark".to_string(),
                description: "Dark theme with minimal colors".to_string(),
                colors: ColorScheme {
                    focused_border: "white".to_string(),
                    unfocused_border: "dark_gray".to_string(),
                    selected_bg: "dark_gray".to_string(),
                    selected_fg: "white".to_string(),
                    highlight_bg: "black".to_string(),
                    highlight_fg: "gray".to_string(),
                    help_text: "gray".to_string(),
                    success: "green".to_string(),
                    error: "red".to_string(),
                    warning: "yellow".to_string(),
                    info: "white".to_string(),
                    accent: "bright_white".to_string(),
                    // Syntax highlighting
                    keyword: "white".to_string(),
                    type_name: "light_green".to_string(),
                    string: "green".to_string(),
                    number: "yellow".to_string(),
                    comment: "dark_gray".to_string(),
                    identifier: "gray".to_string(),
                    operator: "white".to_string(),
                    punctuation: "gray".to_string(),
                    address: "cyan".to_string(),
                    pragma: "magenta".to_string(),
                    opcode: "white".to_string(),
                    line_number: "dark_gray".to_string(),
                    line_number_bg: "black".to_string(),
                },
            },
        );

        // Light theme
        themes.insert(
            "light".to_string(),
            Theme {
                name: "Light".to_string(),
                description: "Light theme with dark text on light backgrounds".to_string(),
                colors: ColorScheme {
                    focused_border: "blue".to_string(),
                    unfocused_border: "gray".to_string(),
                    selected_bg: "light_blue".to_string(),
                    selected_fg: "black".to_string(),
                    highlight_bg: "light_gray".to_string(),
                    highlight_fg: "black".to_string(),
                    help_text: "dark_gray".to_string(),
                    success: "green".to_string(),
                    error: "red".to_string(),
                    warning: "yellow".to_string(),
                    info: "blue".to_string(),
                    accent: "bright_blue".to_string(),
                    // Syntax highlighting
                    keyword: "blue".to_string(),
                    type_name: "green".to_string(),
                    string: "red".to_string(),
                    number: "magenta".to_string(),
                    comment: "gray".to_string(),
                    identifier: "black".to_string(),
                    operator: "blue".to_string(),
                    punctuation: "black".to_string(),
                    address: "cyan".to_string(),
                    pragma: "magenta".to_string(),
                    opcode: "blue".to_string(),
                    line_number: "gray".to_string(),
                    line_number_bg: "white".to_string(),
                },
            },
        );

        // Monokai theme
        themes.insert(
            "monokai".to_string(),
            Theme {
                name: "Monokai".to_string(),
                description: "Popular dark theme with vibrant colors".to_string(),
                colors: ColorScheme {
                    focused_border: "magenta".to_string(),
                    unfocused_border: "dark_gray".to_string(),
                    selected_bg: "magenta".to_string(),
                    selected_fg: "black".to_string(),
                    highlight_bg: "dark_gray".to_string(),
                    highlight_fg: "green".to_string(),
                    help_text: "cyan".to_string(),
                    success: "green".to_string(),
                    error: "red".to_string(),
                    warning: "yellow".to_string(),
                    info: "cyan".to_string(),
                    accent: "bright_magenta".to_string(),
                    // Syntax highlighting (Monokai-inspired)
                    keyword: "magenta".to_string(),
                    type_name: "light_blue".to_string(),
                    string: "yellow".to_string(),
                    number: "light_magenta".to_string(),
                    comment: "gray".to_string(),
                    identifier: "white".to_string(),
                    operator: "red".to_string(),
                    punctuation: "white".to_string(),
                    address: "light_green".to_string(),
                    pragma: "magenta".to_string(),
                    opcode: "light_cyan".to_string(),
                    line_number: "gray".to_string(),
                    line_number_bg: "black".to_string(),
                },
            },
        );

        Self { active: "default".to_string(), themes }
    }
}

impl Default for PanelConfig {
    fn default() -> Self {
        Self {
            terminal: TerminalPanelConfig { max_history: 1000, show_timestamps: false },
            code: CodePanelConfig { show_line_numbers: true, highlight_current_line: true },
            trace: TracePanelConfig { show_depth_indicators: true, max_entries: 500 },
            display: DisplayPanelConfig { default_mode: "Variables".to_string(), show_types: true },
        }
    }
}

impl Config {
    /// Get the config file path (~/.edb.toml)
    pub fn config_path() -> Result<PathBuf> {
        let home =
            dirs::home_dir().ok_or_else(|| eyre::eyre!("Unable to determine home directory"))?;
        Ok(home.join(".edb.toml"))
    }

    /// Load configuration from file, creating default if it doesn't exist
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;

        if !config_path.exists() {
            info!("Config file not found, creating default at {:?}", config_path);
            let default_config = Self::default();
            default_config.save()?;
            return Ok(default_config);
        }

        let content = fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read config file: {:?}", config_path))?;

        let config: Config =
            toml::from_str(&content).with_context(|| "Failed to parse config file as TOML")?;

        debug!("Loaded configuration from {:?}", config_path);
        Ok(config)
    }

    /// Save configuration to file
    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;

        let content =
            toml::to_string_pretty(self).with_context(|| "Failed to serialize config to TOML")?;

        fs::write(&config_path, content)
            .with_context(|| format!("Failed to write config file: {:?}", config_path))?;

        debug!("Saved configuration to {:?}", config_path);
        Ok(())
    }

    /// Get the currently active theme
    pub fn get_active_theme(&self) -> Option<&Theme> {
        self.theme.themes.get(&self.theme.active)
    }

    /// Switch to a different theme
    pub fn set_theme(&mut self, theme_name: &str) -> Result<()> {
        if !self.theme.themes.contains_key(theme_name) {
            return Err(eyre::eyre!("Theme '{}' not found", theme_name));
        }

        self.theme.active = theme_name.to_string();
        info!("Switched to theme: {}", theme_name);
        Ok(())
    }

    /// List available themes
    pub fn list_themes(&self) -> Vec<(&String, &Theme)> {
        self.theme.themes.iter().collect()
    }

    /// Convert color string to ratatui Color
    pub fn parse_color(color_str: &str) -> Color {
        match color_str.to_lowercase().as_str() {
            "black" => Color::Black,
            "red" => Color::Red,
            "green" => Color::Green,
            "yellow" => Color::Yellow,
            "blue" => Color::Blue,
            "magenta" => Color::Magenta,
            "cyan" => Color::Cyan,
            "gray" => Color::Gray,
            "dark_gray" => Color::DarkGray,
            "light_red" => Color::LightRed,
            "light_green" => Color::LightGreen,
            "light_yellow" => Color::LightYellow,
            "light_blue" => Color::LightBlue,
            "light_magenta" => Color::LightMagenta,
            "light_cyan" => Color::LightCyan,
            "white" => Color::White,
            "light_gray" => Color::Gray,
            _ => {
                warn!("Unknown color '{}', using default gray", color_str);
                Color::Gray
            }
        }
    }
}

impl ColorScheme {
    /// Get focused border color
    pub fn focused_border(&self) -> Color {
        Config::parse_color(&self.focused_border)
    }

    /// Get unfocused border color  
    pub fn unfocused_border(&self) -> Color {
        Config::parse_color(&self.unfocused_border)
    }

    /// Get selected background color
    pub fn selected_bg(&self) -> Color {
        Config::parse_color(&self.selected_bg)
    }

    /// Get selected foreground color
    pub fn selected_fg(&self) -> Color {
        Config::parse_color(&self.selected_fg)
    }

    /// Get highlight background color
    pub fn highlight_bg(&self) -> Color {
        Config::parse_color(&self.highlight_bg)
    }

    /// Get highlight foreground color
    pub fn highlight_fg(&self) -> Color {
        Config::parse_color(&self.highlight_fg)
    }

    /// Get help text color
    pub fn help_text(&self) -> Color {
        Config::parse_color(&self.help_text)
    }

    /// Get success color
    pub fn success(&self) -> Color {
        Config::parse_color(&self.success)
    }

    /// Get error color
    pub fn error(&self) -> Color {
        Config::parse_color(&self.error)
    }

    /// Get warning color
    pub fn warning(&self) -> Color {
        Config::parse_color(&self.warning)
    }
    /// Get info color
    pub fn info(&self) -> Color {
        Config::parse_color(&self.info)
    }
    /// Get accent color for status bars
    pub fn accent_color(&self) -> Color {
        Config::parse_color(&self.accent)
    }

    // Syntax highlighting color methods

    /// Get keyword color
    pub fn keyword_color(&self) -> Color {
        Config::parse_color(&self.keyword)
    }

    /// Get type name color
    pub fn type_color(&self) -> Color {
        Config::parse_color(&self.type_name)
    }

    /// Get string color
    pub fn string_color(&self) -> Color {
        Config::parse_color(&self.string)
    }

    /// Get number color
    pub fn number_color(&self) -> Color {
        Config::parse_color(&self.number)
    }

    /// Get comment color
    pub fn comment_color(&self) -> Color {
        Config::parse_color(&self.comment)
    }

    /// Get identifier color
    pub fn identifier_color(&self) -> Color {
        Config::parse_color(&self.identifier)
    }

    /// Get operator color
    pub fn operator_color(&self) -> Color {
        Config::parse_color(&self.operator)
    }

    /// Get punctuation color
    pub fn punctuation_color(&self) -> Color {
        Config::parse_color(&self.punctuation)
    }

    /// Get address color
    pub fn address_color(&self) -> Color {
        Config::parse_color(&self.address)
    }

    /// Get pragma color
    pub fn pragma_color(&self) -> Color {
        Config::parse_color(&self.pragma)
    }

    /// Get opcode color
    pub fn opcode_color(&self) -> Color {
        Config::parse_color(&self.opcode)
    }

    /// Get line number color
    pub fn line_number_color(&self) -> Color {
        Config::parse_color(&self.line_number)
    }

    /// Get line number background color
    pub fn line_number_bg_color(&self) -> Color {
        Config::parse_color(&self.line_number_bg)
    }
}
