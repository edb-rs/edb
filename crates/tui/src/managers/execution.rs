//! Execution state management for TUI panels

use std::sync::{Arc, RwLock};

/// Current execution state information
#[derive(Debug, Clone)]
pub struct ExecutionState {
    /// Current snapshot index
    pub current_snapshot: usize,
    /// Total number of snapshots
    pub total_snapshots: usize,
    /// Current execution line (1-based line number)
    pub current_line: Option<usize>,
    /// Current file path
    pub current_file: Option<String>,
    /// Whether execution is paused/stopped
    pub is_paused: bool,
}

impl ExecutionState {
    /// Create new execution state
    pub fn new() -> Self {
        Self {
            current_snapshot: 0,
            total_snapshots: 0,
            current_line: None,
            current_file: None,
            is_paused: true,
        }
    }
}

/// Shared execution manager for communication between panels
#[derive(Debug, Clone)]
pub struct ExecutionManager {
    /// Shared execution state
    state: Arc<RwLock<ExecutionState>>,
}

impl ExecutionManager {
    /// Create a new execution manager
    pub fn new() -> Self {
        Self { state: Arc::new(RwLock::new(ExecutionState::new())) }
    }

    /// Get current snapshot index
    pub fn current_snapshot(&self) -> usize {
        if let Ok(state) = self.state.read() {
            state.current_snapshot
        } else {
            0
        }
    }

    /// Get total snapshot count
    pub fn total_snapshots(&self) -> usize {
        if let Ok(state) = self.state.read() {
            state.total_snapshots
        } else {
            0
        }
    }

    /// Get current execution line
    pub fn current_line(&self) -> Option<usize> {
        if let Ok(state) = self.state.read() {
            state.current_line
        } else {
            None
        }
    }

    /// Get current file
    pub fn current_file(&self) -> Option<String> {
        if let Ok(state) = self.state.read() {
            state.current_file.clone()
        } else {
            None
        }
    }

    /// Check if execution is paused
    pub fn is_paused(&self) -> bool {
        if let Ok(state) = self.state.read() {
            state.is_paused
        } else {
            true
        }
    }

    /// Update execution state from RPC response
    pub fn update_state(
        &self,
        snapshot: usize,
        total: usize,
        line: Option<usize>,
        file: Option<String>,
    ) {
        if let Ok(mut state) = self.state.write() {
            state.current_snapshot = snapshot;
            state.total_snapshots = total;
            state.current_line = line;
            state.current_file = file;
        }
    }

    /// Set pause state
    pub fn set_paused(&self, paused: bool) {
        if let Ok(mut state) = self.state.write() {
            state.is_paused = paused;
        }
    }

    /// Get a copy of the full execution state
    pub fn get_state(&self) -> ExecutionState {
        if let Ok(state) = self.state.read() {
            state.clone()
        } else {
            ExecutionState::new()
        }
    }
}
