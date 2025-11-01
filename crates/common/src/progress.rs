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

//! Progress message types for tracking operation progress

use serde::{Deserialize, Serialize};

/// Progress message with optional step tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressMessage {
    /// The progress message text
    pub message: String,
    /// Current step number (1-indexed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_step: Option<usize>,
    /// Total number of steps
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_steps: Option<usize>,
}

impl ProgressMessage {
    /// Create a simple progress message without step tracking
    pub fn new(message: impl Into<String>) -> Self {
        Self { message: message.into(), current_step: None, total_steps: None }
    }

    /// Create a progress message with step tracking
    pub fn with_steps(message: impl Into<String>, current_step: usize, total_steps: usize) -> Self {
        Self {
            message: message.into(),
            current_step: Some(current_step),
            total_steps: Some(total_steps),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progress_message_new() {
        let msg = ProgressMessage::new("Test message");
        assert_eq!(msg.message, "Test message");
        assert_eq!(msg.current_step, None);
        assert_eq!(msg.total_steps, None);
    }

    #[test]
    fn test_progress_message_with_steps() {
        let msg = ProgressMessage::with_steps("Step 2 of 5", 2, 5);
        assert_eq!(msg.message, "Step 2 of 5");
        assert_eq!(msg.current_step, Some(2));
        assert_eq!(msg.total_steps, Some(5));
    }

    #[test]
    fn test_progress_message_serialization_without_steps() {
        let msg = ProgressMessage::new("Processing");
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"message\":\"Processing\""));
        assert!(!json.contains("current_step"));
        assert!(!json.contains("total_steps"));
    }

    #[test]
    fn test_progress_message_serialization_with_steps() {
        let msg = ProgressMessage::with_steps("Step 3", 3, 10);
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"message\":\"Step 3\""));
        assert!(json.contains("\"current_step\":3"));
        assert!(json.contains("\"total_steps\":10"));
    }
}
