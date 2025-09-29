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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_execution_frame_id_new() {
        let frame_id = ExecutionFrameId::new(42, 7);
        assert_eq!(frame_id.0, 42);
        assert_eq!(frame_id.1, 7);
        assert_eq!(frame_id.trace_entry_id(), 42);
        assert_eq!(frame_id.re_entry_count(), 7);
    }

    #[test]
    fn test_execution_frame_id_display() {
        let frame_id = ExecutionFrameId::new(123, 456);
        assert_eq!(format!("{frame_id}"), "123.456");

        let zero_frame = ExecutionFrameId::new(0, 0);
        assert_eq!(format!("{zero_frame}"), "0.0");

        let max_frame = ExecutionFrameId::new(usize::MAX, usize::MAX);
        assert_eq!(format!("{max_frame}"), format!("{}.{}", usize::MAX, usize::MAX));
    }

    #[test]
    fn test_execution_frame_id_increment_re_entry() {
        let mut frame_id = ExecutionFrameId::new(10, 5);
        assert_eq!(frame_id.re_entry_count(), 5);

        frame_id.increment_re_entry();
        assert_eq!(frame_id.re_entry_count(), 6);
        assert_eq!(frame_id.trace_entry_id(), 10); // Should not change

        frame_id.increment_re_entry();
        assert_eq!(frame_id.re_entry_count(), 7);
    }

    #[test]
    fn test_execution_frame_id_serialization() {
        let frame_id = ExecutionFrameId::new(123, 789);

        let json = serde_json::to_string(&frame_id).expect("Failed to serialize ExecutionFrameId");
        let deserialized: ExecutionFrameId =
            serde_json::from_str(&json).expect("Failed to deserialize ExecutionFrameId");

        assert_eq!(deserialized, frame_id);
        assert_eq!(deserialized.trace_entry_id(), 123);
        assert_eq!(deserialized.re_entry_count(), 789);
    }

    #[test]
    fn test_execution_frame_id_serialization_edge_cases() {
        let test_cases = vec![
            ExecutionFrameId::new(0, 0),
            ExecutionFrameId::new(usize::MAX, usize::MAX),
            ExecutionFrameId::new(1, 0),
            ExecutionFrameId::new(0, 1),
        ];

        for frame_id in test_cases {
            let json =
                serde_json::to_string(&frame_id).expect("Failed to serialize ExecutionFrameId");
            let deserialized: ExecutionFrameId =
                serde_json::from_str(&json).expect("Failed to deserialize ExecutionFrameId");
            assert_eq!(deserialized, frame_id);
        }
    }

    #[test]
    fn test_execution_frame_id_equality() {
        let frame1 = ExecutionFrameId::new(10, 20);
        let frame2 = ExecutionFrameId::new(10, 20);
        let frame3 = ExecutionFrameId::new(10, 21);
        let frame4 = ExecutionFrameId::new(11, 20);

        assert_eq!(frame1, frame2);
        assert_ne!(frame1, frame3);
        assert_ne!(frame1, frame4);
        assert_ne!(frame3, frame4);
    }

    #[test]
    fn test_execution_frame_id_hash() {
        let frame1 = ExecutionFrameId::new(10, 20);
        let frame2 = ExecutionFrameId::new(10, 20);
        let frame3 = ExecutionFrameId::new(10, 21);

        let mut set = HashSet::new();
        set.insert(frame1);
        set.insert(frame2); // Should not increase size due to equality
        set.insert(frame3);

        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_execution_frame_id_ordering() {
        let frame1 = ExecutionFrameId::new(1, 1);
        let frame2 = ExecutionFrameId::new(1, 2);
        let frame3 = ExecutionFrameId::new(2, 1);
        let frame4 = ExecutionFrameId::new(2, 2);

        let mut frames = vec![frame4, frame1, frame3, frame2];
        frames.sort();

        assert_eq!(frames, vec![frame1, frame2, frame3, frame4]);
    }

    #[test]
    fn test_execution_frame_id_partial_ord() {
        let frame1 = ExecutionFrameId::new(5, 10);
        let frame2 = ExecutionFrameId::new(5, 20);
        let frame3 = ExecutionFrameId::new(10, 5);

        assert!(frame1 < frame2);
        assert!(frame2 < frame3);
        assert!(frame1 < frame3);
        assert!(frame2 > frame1);
        assert!(frame3 > frame2);
        assert!(frame3 > frame1);
    }

    #[test]
    fn test_execution_frame_id_clone() {
        let frame = ExecutionFrameId::new(100, 200);
        let cloned = frame;

        assert_eq!(frame, cloned);
        assert_eq!(cloned.trace_entry_id(), 100);
        assert_eq!(cloned.re_entry_count(), 200);
    }

    #[test]
    fn test_execution_frame_id_copy() {
        let frame = ExecutionFrameId::new(42, 84);
        let copied = frame; // Should work due to Copy trait

        assert_eq!(frame, copied);
        assert_eq!(frame.trace_entry_id(), copied.trace_entry_id());
        assert_eq!(frame.re_entry_count(), copied.re_entry_count());
    }

    #[test]
    fn test_execution_frame_id_debug() {
        let frame = ExecutionFrameId::new(123, 456);
        let debug_str = format!("{frame:?}");
        assert!(debug_str.contains("ExecutionFrameId"));
        assert!(debug_str.contains("123"));
        assert!(debug_str.contains("456"));
    }

    #[test]
    fn test_execution_frame_id_increment_overflow_behavior() {
        // Test that increment handles potential overflow gracefully
        // Note: This might panic in debug mode, but should wrap in release mode
        let mut frame = ExecutionFrameId::new(0, usize::MAX);

        // This is a behavior test - in production, care should be taken to avoid overflow
        // The test documents the current behavior
        #[cfg(debug_assertions)]
        {
            // In debug mode, this might panic
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                frame.increment_re_entry();
            }));
            // Either panics or wraps to 0
            if result.is_ok() {
                assert_eq!(frame.re_entry_count(), 0); // Wrapped around
            }
        }

        #[cfg(not(debug_assertions))]
        {
            // In release mode, should wrap to 0
            frame.increment_re_entry();
            assert_eq!(frame.re_entry_count(), 0);
        }
    }

    #[test]
    fn test_execution_frame_id_large_values_serialization() {
        let large_frame = ExecutionFrameId::new(usize::MAX / 2, usize::MAX / 3);

        let json = serde_json::to_string(&large_frame)
            .expect("Failed to serialize large ExecutionFrameId");
        let deserialized: ExecutionFrameId =
            serde_json::from_str(&json).expect("Failed to deserialize large ExecutionFrameId");

        assert_eq!(deserialized, large_frame);
        assert_eq!(deserialized.trace_entry_id(), usize::MAX / 2);
        assert_eq!(deserialized.re_entry_count(), usize::MAX / 3);
    }

    #[test]
    fn test_execution_frame_id_json_format() {
        let frame = ExecutionFrameId::new(42, 17);
        let json = serde_json::to_string(&frame).expect("Failed to serialize");

        // Verify the JSON format - should be a tuple
        assert!(json.contains("42"));
        assert!(json.contains("17"));

        // Verify it's deserializable back to the same value
        let deserialized: ExecutionFrameId =
            serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(deserialized, frame);
    }
}
