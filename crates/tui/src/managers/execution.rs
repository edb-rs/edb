//! Execution state management for TUI panels

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
#[derive(Debug)]
pub struct ExecutionManager {
    /// Current execution state
    state: ExecutionState,
}

impl ExecutionManager {
    /// Create a new execution manager
    pub fn new() -> Self {
        Self { state: ExecutionState::new() }
    }

    /// Get current snapshot index
    pub fn current_snapshot(&self) -> usize {
        self.state.current_snapshot
    }

    /// Get total snapshot count
    pub fn total_snapshots(&self) -> usize {
        self.state.total_snapshots
    }

    /// Get current execution line
    pub fn current_line(&self) -> Option<usize> {
        self.state.current_line
    }

    /// Get current file
    pub fn current_file(&self) -> Option<String> {
        self.state.current_file.clone()
    }

    /// Check if execution is paused
    pub fn is_paused(&self) -> bool {
        self.state.is_paused
    }

    /// Update execution state from RPC response
    pub fn update_state(
        &mut self,
        snapshot: usize,
        total: usize,
        line: Option<usize>,
        file: Option<String>,
    ) {
        self.state.current_snapshot = snapshot;
        self.state.total_snapshots = total;
        self.state.current_line = line;
        self.state.current_file = file;
    }

    /// Set pause state
    pub fn set_paused(&mut self, paused: bool) {
        self.state.is_paused = paused;
    }

    /// Get a copy of the full execution state
    pub fn get_state(&self) -> ExecutionState {
        self.state.clone()
    }
}
