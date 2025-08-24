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

//! Theme management system for EDB TUI
//!
//! Provides centralized theme management across all panels with real-time switching.

use crate::{
    config::{Config, PanelConfig},
    ColorScheme, Theme,
};
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
            ColorScheme::default()
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
            .map(|(name, theme)| {
                (name.clone(), theme.display_name().to_string(), theme.description().to_string())
            })
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
