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
//! This module implements a two-tier architecture for theme management:
//!
//! - `ThemeManager`: Per-thread instance with cached data for immediate reads during rendering
//! - `ThemeManagerCore`: Shared core that handles complex operations and data persistence
//!
//! This design ensures rendering threads never block on I/O or complex operations while
//! maintaining consistency across the application.

use crate::{
    config::{Config, PanelConfig},
    ColorScheme,
};
use eyre::{Ok, Result};
use std::{
    ops::{Deref, DerefMut},
    sync::{Arc, RwLock},
};
use tracing::{debug, info};

/// Per-thread theme manager providing immediate read access for rendering
///
/// # Design Philosophy
///
/// `ThemeManager` acts as a local cache for rendering operations, ensuring that:
/// - All reads are immediate and non-blocking (direct field access)
/// - Data synchronization happens explicitly via `fetch_data()`
/// - Rendering threads never wait on locks or I/O operations
///
/// # Usage Pattern
///
/// ```ignore
/// // In rendering loop
/// let color = theme_manager.color_scheme.primary; // Immediate read
///
/// // When theme changes are needed
/// theme_manager.fetch_data().await?; // Sync with core
/// ```
#[derive(Debug, Clone)]
pub struct ThemeManager {
    pub color_scheme: ColorScheme,
    generation: u64,
    core: Arc<RwLock<ThemeManagerCore>>,
}

impl Deref for ThemeManager {
    type Target = Arc<RwLock<ThemeManagerCore>>;

    fn deref(&self) -> &Self::Target {
        &self.core
    }
}

impl DerefMut for ThemeManager {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.core
    }
}

impl ThemeManager {
    /// Create a new theme manager with a shared core
    pub fn new(core: Arc<RwLock<ThemeManagerCore>>) -> Self {
        let color_scheme: ColorScheme = ColorScheme::default();
        Self { color_scheme, generation: 0, core }
    }

    /// Synchronize local cache with the shared core
    ///
    /// This is the only async operation in ThemeManager, designed to:
    /// - Pull latest theme data from ThemeManagerCore
    /// - Update local cache for immediate reads
    /// - Be called after theme changes or on initialization
    pub async fn fetch_data(&mut self) -> Result<()> {
        let core = self.core.read().unwrap();

        if self.generation != core.get_current_generation() {
            self.color_scheme = core.get_current_colors();
            self.generation = core.get_current_generation();
        }
        Ok(())
    }
}

/// Centralized theme state manager handling complex operations and persistence
///
/// # Design Philosophy
///
/// `ThemeManagerCore` is the single source of truth for theme state, responsible for:
/// - Complex theme operations (switching, loading, saving)
/// - Configuration file I/O
/// - Cache management and data fetching
/// - Thread-safe state updates via `Arc<RwLock<>>`
///
/// All methods are synchronous and protected by RwLock for thread safety.
/// UI threads access this through ThemeManager which caches data locally.
///
/// # Architecture Benefits
///
/// This separation provides:
/// - **Non-blocking UI**: Rendering never waits on I/O or complex operations
/// - **Consistency**: Single source of truth for theme state
/// - **Flexibility**: Complex operations isolated from rendering concerns
/// - **Thread Safety**: RwLock ensures safe concurrent access
#[derive(Debug, Clone)]
pub struct ThemeManagerCore {
    config: Config,
    generation: u64,
}

impl ThemeManagerCore {
    /// Create a new theme manager core, loading configuration from disk
    pub fn new() -> Self {
        let config = Config::load().unwrap_or_default();
        Self { config, generation: 0 }
    }

    /// Get the currently active theme's color scheme
    ///
    /// This is typically called by ThemeManager::fetch_data() to update local caches
    pub fn get_current_colors(&self) -> ColorScheme {
        if let Some(theme) = self.config.get_active_theme() {
            theme.clone().into()
        } else {
            ColorScheme::default()
        }
    }

    /// Get the current theme's generation
    pub fn get_current_generation(&self) -> u64 {
        self.generation
    }

    /// List all available themes with metadata
    pub fn list_themes(&self) -> Vec<(String, String, String)> {
        self.config
            .list_themes()
            .into_iter()
            .map(|(name, theme)| {
                (name.clone(), theme.display_name().to_string(), theme.description().to_string())
            })
            .collect()
    }

    /// Switch to a different theme and persist the change
    ///
    /// This operation:
    /// 1. Updates the active theme in configuration
    /// 2. Saves to disk for persistence
    /// 3. Requires ThemeManager instances to call fetch_data() to see changes
    pub fn switch_theme(&mut self, theme_name: &str) -> Result<()> {
        self.config.set_theme(theme_name)?;
        self.config.save()?;
        self.generation += 1;
        info!("Theme switched to: {} (generation: {})", theme_name, self.generation);
        Ok(())
    }

    /// Reload configuration from disk
    ///
    /// Useful for picking up external configuration changes
    pub fn reload(&mut self) -> Result<()> {
        let new_config = Config::load()?;
        self.config = new_config;
        self.generation += 1;
        debug!("Theme manager configuration reloaded (generation: {})", self.generation);
        Ok(())
    }

    /// Get current panel configuration
    pub fn get_panel_config(&self) -> PanelConfig {
        self.config.panels.clone()
    }

    /// Get active theme name
    pub fn get_active_theme_name(&self) -> String {
        self.config.theme.active.clone()
    }

    /// Fetch data
    pub async fn fetch_data(&mut self) -> eyre::Result<()> {
        Ok(())
    }
}
