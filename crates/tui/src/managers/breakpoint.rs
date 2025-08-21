//! Breakpoint management for TUI panels

use std::collections::HashSet;
use std::sync::{Arc, RwLock};

/// Shared breakpoint manager for communication between panels
#[derive(Debug, Clone)]
pub struct BreakpointManager {
    /// Shared breakpoint data (line numbers, 1-based)
    breakpoints: Arc<RwLock<HashSet<usize>>>,
}

impl BreakpointManager {
    /// Create a new breakpoint manager
    pub fn new() -> Self {
        Self { breakpoints: Arc::new(RwLock::new(HashSet::new())) }
    }

    /// Add a breakpoint at the given line
    pub fn add_breakpoint(&self, line: usize) -> bool {
        if let Ok(mut breakpoints) = self.breakpoints.write() {
            breakpoints.insert(line)
        } else {
            false
        }
    }

    /// Remove a breakpoint at the given line
    pub fn remove_breakpoint(&self, line: usize) -> bool {
        if let Ok(mut breakpoints) = self.breakpoints.write() {
            breakpoints.remove(&line)
        } else {
            false
        }
    }

    /// Toggle a breakpoint at the given line
    pub fn toggle_breakpoint(&self, line: usize) -> bool {
        if let Ok(mut breakpoints) = self.breakpoints.write() {
            if breakpoints.contains(&line) {
                breakpoints.remove(&line);
                false
            } else {
                breakpoints.insert(line);
                true
            }
        } else {
            false
        }
    }

    /// Check if a breakpoint exists at the given line
    pub fn has_breakpoint(&self, line: usize) -> bool {
        if let Ok(breakpoints) = self.breakpoints.read() {
            breakpoints.contains(&line)
        } else {
            false
        }
    }

    /// Get all breakpoints as a sorted vector
    pub fn get_all_breakpoints(&self) -> Vec<usize> {
        if let Ok(breakpoints) = self.breakpoints.read() {
            let mut sorted: Vec<usize> = breakpoints.iter().cloned().collect();
            sorted.sort();
            sorted
        } else {
            Vec::new()
        }
    }

    /// Get breakpoint count
    pub fn count(&self) -> usize {
        if let Ok(breakpoints) = self.breakpoints.read() {
            breakpoints.len()
        } else {
            0
        }
    }
}
