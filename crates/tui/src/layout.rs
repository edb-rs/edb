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

//! Adaptive layout management
//!
//! This module handles responsive layout switching based on terminal size.

/// Configuration for layout manager
#[derive(Debug, Clone, Default)]
pub struct LayoutConfig {
    /// Enable mouse support
    pub enable_mouse: bool,
}

/// Layout types for different terminal sizes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutType {
    /// Full 4-panel quad layout (≥120 columns)
    Full,
    /// Compact 3-panel stacked layout (80-119 columns)
    Compact,
    /// Single panel mode with F-key switching (<80 columns)
    Mobile,
}

/// Layout manager for responsive design
#[derive(Debug)]
pub struct LayoutManager {
    current_layout: LayoutType,
    terminal_width: u16,
    terminal_height: u16,
}

impl LayoutManager {
    /// Create a new layout manager with default values
    pub fn new() -> Self {
        Self { current_layout: LayoutType::Full, terminal_width: 80, terminal_height: 24 }
    }

    /// Update terminal dimensions and recalculate layout
    pub fn update_size(&mut self, width: u16, height: u16) {
        self.terminal_width = width;
        self.terminal_height = height;
        self.current_layout = self.calculate_layout_type();
    }

    /// Calculate appropriate layout type based on current dimensions
    fn calculate_layout_type(&self) -> LayoutType {
        if self.terminal_width >= 120 {
            LayoutType::Full
        } else if self.terminal_width >= 80 {
            LayoutType::Compact
        } else {
            LayoutType::Mobile
        }
    }

    /// Get current layout type
    pub fn layout_type(&self) -> LayoutType {
        self.current_layout
    }

    /// Get current terminal width
    pub fn width(&self) -> u16 {
        self.terminal_width
    }

    /// Get current terminal height  
    pub fn height(&self) -> u16 {
        self.terminal_height
    }

    /// Check if current layout supports multiple visible panels
    pub fn supports_multiple_panels(&self) -> bool {
        matches!(self.current_layout, LayoutType::Full | LayoutType::Compact)
    }

    /// Get minimum width for this layout type
    pub fn min_width_for_layout(layout: LayoutType) -> u16 {
        match layout {
            LayoutType::Full => 120,
            LayoutType::Compact => 80,
            LayoutType::Mobile => 1,
        }
    }
}

impl Default for LayoutManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout_calculation() {
        let mut manager = LayoutManager::new();

        // Test full layout
        manager.update_size(120, 30);
        assert_eq!(manager.layout_type(), LayoutType::Full);

        // Test compact layout
        manager.update_size(100, 30);
        assert_eq!(manager.layout_type(), LayoutType::Compact);

        // Test mobile layout
        manager.update_size(60, 20);
        assert_eq!(manager.layout_type(), LayoutType::Mobile);
    }

    #[test]
    fn test_multiple_panels_support() {
        let mut manager = LayoutManager::new();

        manager.update_size(120, 30);
        assert!(manager.supports_multiple_panels());

        manager.update_size(100, 30);
        assert!(manager.supports_multiple_panels());

        manager.update_size(60, 20);
        assert!(!manager.supports_multiple_panels());
    }

    #[test]
    fn test_min_widths() {
        assert_eq!(LayoutManager::min_width_for_layout(LayoutType::Full), 120);
        assert_eq!(LayoutManager::min_width_for_layout(LayoutType::Compact), 80);
        assert_eq!(LayoutManager::min_width_for_layout(LayoutType::Mobile), 1);
    }
}
