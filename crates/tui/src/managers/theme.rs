//! Theme management system for EDB TUI
//!
//! Provides centralized theme management across all panels with real-time switching.

use crate::config::{ColorScheme, Config};
use eyre::Result;
use ratatui::style::Color;
use std::sync::{Arc, RwLock};
use tracing::{debug, info};

/// Thread-safe theme manager for real-time theme switching
#[derive(Debug, Clone)]
pub struct ThemeManager {
    config: Arc<RwLock<Config>>,
}

impl ThemeManager {
    /// Create a new theme manager
    pub fn new() -> Self {
        let config = Config::load().unwrap_or_default();
        Self { config: Arc::new(RwLock::new(config)) }
    }

    /// Get the currently active theme's color scheme
    pub fn get_current_colors(&self) -> ColorScheme {
        let config = self.config.read().unwrap();
        if let Some(theme) = config.get_active_theme() {
            theme.colors.clone()
        } else {
            // Fallback to default theme colors
            Config::default().theme.themes["default"].colors.clone()
        }
    }

    /// Get active theme name
    pub fn get_active_theme_name(&self) -> String {
        self.config.read().unwrap().theme.active.clone()
    }

    /// List all available themes
    pub fn list_themes(&self) -> Vec<(String, String, String)> {
        let config = self.config.read().unwrap();
        config
            .list_themes()
            .into_iter()
            .map(|(name, theme)| (name.clone(), theme.name.clone(), theme.description.clone()))
            .collect()
    }

    /// Switch to a different theme
    pub fn switch_theme(&self, theme_name: &str) -> Result<()> {
        {
            let mut config = self.config.write().unwrap();
            config.set_theme(theme_name)?;
            config.save()?;
        }

        info!("Theme switched to: {}", theme_name);
        Ok(())
    }

    /// Get focused border color
    pub fn focused_border_color(&self) -> Color {
        self.get_current_colors().focused_border()
    }

    /// Get unfocused border color
    pub fn unfocused_border_color(&self) -> Color {
        self.get_current_colors().unfocused_border()
    }

    /// Get selected background color
    pub fn selected_bg_color(&self) -> Color {
        self.get_current_colors().selected_bg()
    }

    /// Get selected foreground color
    pub fn selected_fg_color(&self) -> Color {
        self.get_current_colors().selected_fg()
    }

    /// Get highlight background color
    pub fn highlight_bg_color(&self) -> Color {
        self.get_current_colors().highlight_bg()
    }

    /// Get highlight foreground color
    pub fn highlight_fg_color(&self) -> Color {
        self.get_current_colors().highlight_fg()
    }

    /// Get help text color
    pub fn help_text_color(&self) -> Color {
        self.get_current_colors().help_text()
    }

    /// Get success color
    pub fn success_color(&self) -> Color {
        self.get_current_colors().success()
    }

    /// Get error color
    pub fn error_color(&self) -> Color {
        self.get_current_colors().error()
    }

    /// Get warning color
    pub fn warning_color(&self) -> Color {
        self.get_current_colors().warning()
    }

    /// Get info color
    pub fn info_color(&self) -> Color {
        self.get_current_colors().info()
    }

    /// Get accent color (for status bars and highlighting)
    pub fn accent_color(&self) -> Color {
        self.get_current_colors().accent_color()
    }

    /// Reload configuration from disk
    pub fn reload(&self) -> Result<()> {
        let new_config = Config::load()?;
        *self.config.write().unwrap() = new_config;
        debug!("Theme manager configuration reloaded");
        Ok(())
    }

    /// Get current panel configuration
    pub fn get_panel_config(&self) -> crate::config::PanelConfig {
        self.config.read().unwrap().panels.clone()
    }
}
