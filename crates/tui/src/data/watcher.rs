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

use std::collections::HashSet;

/// Watcher for monitoring user-defined expressions
#[derive(Debug, Clone, Default)]
pub struct Watcher {
    /// Expressions being monitored
    expressions: Vec<String>,
    /// Keys of the expressions in the current state
    expression_keys: HashSet<String>,
}

fn normalize_expression(expr: &str) -> String {
    expr.chars().filter(|&c| !c.is_whitespace()).collect()
}

impl Watcher {
    /// Add a new expression to the watcher
    pub fn add_expression(&mut self, expr: String) -> Option<usize> {
        let expr_key = normalize_expression(&expr);
        if !self.expression_keys.contains(&expr_key) {
            self.expression_keys.insert(expr_key);
            self.expressions.push(expr);
            Some(self.expressions.len()) // Return 1-based index
        } else {
            None
        }
    }

    /// Remove an expression from the watcher
    pub fn remove_expression(&mut self, expr_id: usize) -> Option<String> {
        if expr_id > 0 && expr_id <= self.expressions.len() {
            let expr = self.expressions.remove(expr_id.saturating_sub(1));
            let expr_key = normalize_expression(&expr);
            self.expression_keys.remove(&expr_key);
            Some(expr)
        } else {
            None
        }
    }

    /// Return a list of all watched expressions with their IDs
    pub fn list_expressions(&self) -> impl Iterator<Item = (usize, &String)> {
        // Return 1-based index for user-friendly display
        self.expressions.iter().enumerate().map(|(i, expr)| (i + 1, expr))
    }

    /// Get the count of watched expressions
    pub fn count(&self) -> usize {
        self.expressions.len()
    }

    /// Clear all expressions from the watcher
    pub fn clear(&mut self) {
        self.expressions.clear();
    }
}
