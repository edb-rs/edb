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

// Copyright (C) 2024 Zhuo Zhang and Wuqi Zhang
// SPDX-License-Identifier: AGPL-3.0
//! Common type definitions used across EDB components

use std::fmt;

use serde::{Deserialize, Serialize};

/// Execution frame identifier for tracking nested call contexts
///
/// A frame ID is a tuple (trace_entry_id, re_entry_count) where:
/// - `trace_entry_id`: Unique identifier for the trace entry
/// - `re_entry_count`: Number of times this frame has been re-entered
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
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
