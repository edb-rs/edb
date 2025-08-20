// Copyright (C) 2024 Zhuo Zhang and Wuqi Zhang
// SPDX-License-Identifier: AGPL-3.0
//! Common type definitions used across EDB components

use std::fmt;

/// Execution frame identifier for tracking nested call contexts
///
/// A frame ID is a tuple (trace_entry_id, re_entry_count) where:
/// - `trace_entry_id`: Unique identifier for the trace entry
/// - `re_entry_count`: Number of times this frame has been re-entered
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExecutionFrameId(pub usize, pub usize);

impl fmt::Display for ExecutionFrameId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.0, self.1)
    }
}

impl ExecutionFrameId {
    /// Create a new execution frame ID
    pub fn new(trace_entry_id: usize, re_entry_count: usize) -> Self {
        Self(trace_entry_id, re_entry_count)
    }

    /// Get the trace entry ID
    pub fn trace_entry_id(&self) -> usize {
        self.0
    }

    /// Get the re-entry count
    pub fn re_entry_count(&self) -> usize {
        self.1
    }

    /// Increment the re-entry count
    pub fn increment_re_entry(&mut self) {
        self.1 += 1;
    }
}
