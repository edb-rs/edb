//! Theme management system for EDB TUI
//!
//! Provides centralized theme management across all panels with real-time switching.

use crate::{config::{Config, PanelConfig}, ColorScheme, Theme};
use eyre::Result;
use ratatui::style::Color;
use std::sync::{Arc, RwLock};
use tracing::{debug, info};

/// Thread-safe theme manager for real-time theme switching
#[derive(Debug, Clone)]
pub struct ThemeManager {
    config: Config,
}

impl ThemeManager {
    /// Create a new theme manager
    pub fn new() -> Self {
        let config = Config::load().unwrap_or_default();
        Self { config }
    }

    /// Get the currently active theme's color scheme
    pub fn get_current_colors(&self) -> ColorScheme {
        if let Some(theme) = self.config.get_active_theme() {
            theme.clone().into()
        } else {
            Theme::default().into()
        }
    }

    /// Get active theme name
    pub fn get_active_theme_name(&self) -> String {
        self.config.theme.active.clone()
    }

    /// List all available themes
    pub fn list_themes(&self) -> Vec<(String, String, String)> {
        self.config
            .list_themes()
            .into_iter()
            .map(|(name, theme)| (name.clone(), theme.name().to_string(), theme.description().to_string()))
            .collect()
    }

    /// Switch to a different theme
    pub fn switch_theme(&mut self, theme_name: &str) -> Result<()> {
        self.config.set_theme(theme_name)?;
        self.config.save()?;
        info!("Theme switched to: {}", theme_name);
        Ok(())
    }

    /// Reload configuration from disk
    pub fn reload(&mut self) -> Result<()> {
        let new_config = Config::load()?;
        self.config = new_config;
        debug!("Theme manager configuration reloaded");
        Ok(())
    }

    /// Get current panel configuration
    pub fn get_panel_config(&self) -> PanelConfig {
        self.config.panels.clone()
    }
}
