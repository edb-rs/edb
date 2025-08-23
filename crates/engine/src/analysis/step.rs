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


use std::{
    // collections::BTreeMap,
    fmt::Display,
    sync::{Arc, Mutex},
};

use alloy_primitives::ruint::FromUintError;
// use derive_more::{Deref, DerefMut};
use foundry_compilers::artifacts::{
    ast::SourceLocation,
    BlockOrStatement,
    Expression,
    ExpressionOrVariableDeclarationStatement,
    ExpressionStatement,
    FunctionCall,
    FunctionDefinition,
    Statement, // SourceUnit,
};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};

use crate::{
    analysis::{StepHook, UVID},
    // Visitor, Walk,
};

lazy_static! {
    /// The next USID to be assigned.
    pub static ref NEXT_USID: Mutex<USID> = Mutex::new(USID(0));
}

/// A Universal Step Identifier (USID) is a unique identifier for a step in contract execution.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Hash, Default, PartialOrd, Ord, Serialize, Deserialize,
)]
pub struct USID(u64);

impl USID {
    /// Increment the USID and return the previous value.
    pub fn inc(&mut self) -> Self {
        let v = *self;
        self.0 += 1;
        v
    }
}

impl From<USID> for u64 {
    fn from(usid: USID) -> Self {
        usid.0
    }
}

impl From<USID> for alloy_primitives::U256 {
    fn from(usid: USID) -> Self {
        Self::from(usid.0)
    }
}

impl From<u64> for USID {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl TryFrom<alloy_primitives::U256> for USID {
    type Error = FromUintError<u64>;

    fn try_from(value: alloy_primitives::U256) -> Result<Self, FromUintError<u64>> {
        value.try_into().map(USID)
    }
}

impl Display for USID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Generate a new USID.
pub fn new_usid() -> USID {
    let mut usid = NEXT_USID.lock().unwrap();
    usid.inc()
}

/// A reference-counted pointer to a Step for efficient sharing across multiple contexts.
///
/// This type alias provides thread-safe reference counting for Step instances,
/// allowing them to be shared between different parts of the analysis system
/// without copying the entire step data.
pub type StepRef = Arc<Step>;

/// Represents a single executable step in Solidity source code.
///
/// A Step represents a unit of execution that can be debugged, such as a statement,
/// expression, or control flow construct. Each step contains information about
/// its location in the source code and any hooks that should be executed before
/// or after the step.
///
/// # Fields
///
/// - `usid`: Unique step identifier for this execution step
/// - `variant`: The specific type of step (statement, expression, etc.)
/// - `src`: Source location information (file, line, column)
/// - `pre_hooks`: Hooks to execute before this step
/// - `post_hooks`: Hooks to execute after this step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    /// Unique step identifier for this execution step
    pub usid: USID,
    /// The specific type of step (statement, expression, etc.)
    pub variant: StepVariant,
    /// Source location information (file, line, column)
    pub src: SourceLocation,
    /// Hooks to execute before this step
    pub pre_hooks: Vec<StepHook>,
    /// Hooks to execute after this step
    pub post_hooks: Vec<StepHook>,
}

impl Step {
    /// Creates a new Step with the given variant and source location.
    ///
    /// # Arguments
    ///
    /// * `variant` - The type of step (statement, expression, etc.)
    /// * `src` - Source location information
    ///
    /// # Returns
    ///
    /// A new Step instance with a unique USID and default hooks.
    pub fn new(variant: StepVariant, src: SourceLocation) -> Self {
        let usid = new_usid();
        Self { usid, variant, src, pre_hooks: vec![StepHook::BeforeStep(usid)], post_hooks: vec![] }
    }

    /// Adds a variable out-of-scope hook to this step.
    ///
    /// This method adds hooks for variables that go out of scope when this step
    /// is executed. For control flow statements like `break`, `continue`, `return`,
    /// or `revert`, the hook is added as a pre-hook. For other statements, it's
    /// added as a post-hook.
    ///
    /// # Arguments
    ///
    /// * `uvids` - List of variable identifiers that go out of scope
    pub fn add_variable_out_of_scope_hook(&mut self, uvids: Vec<UVID>) {
        // for steps that result in a "jump" in the control flow (e.g., `break`, `continue`, `return`, `revert`, `throw`, etc.), the variable out of scope should be added as pre-hooks.
        let add_as_pre_hook = match &self.variant {
            StepVariant::Statement(statement) => matches!(
                statement,
                Statement::Break(_)
                    | Statement::Continue(_)
                    | Statement::Return(_)
                    | Statement::RevertStatement(_)
            ),
            _ => false,
        };
        let hooks = uvids.into_iter().map(StepHook::VariableOutOfScope).collect::<Vec<_>>();
        if add_as_pre_hook {
            self.pre_hooks.extend(hooks);
        } else {
            self.post_hooks.extend(hooks);
        }
    }
}

/// The variant types for source steps.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(clippy::large_enum_variant)]
pub enum StepVariant {
    /// A function entry step.
    FunctionEntry(FunctionDefinition),
    /// A single statement that is executed in a single debug step.
    Statement(Statement),
    /// A consecutive list of statements that are executed in a single debug step.
    Statements(Vec<Statement>),
    /// The condition of an if statement that is executed in a single debug step.
    IfCondition(Expression),
    /// The header of a for loop that is executed in a single debug step.
    ForLoop {
        /// The initialization expression of the for loop (optional)
        initialization_expression: Option<ExpressionOrVariableDeclarationStatement>,
        /// The condition expression of the for loop (optional)
        condition: Option<Expression>,
        /// The loop expression that executes at the end of each iteration (optional)
        loop_expression: Option<ExpressionStatement>,
    },
    /// The condition of a while loop that is executed in a single debug step.
    WhileLoop(Expression),
    /// The try external call
    Try(FunctionCall),
}

/// Computes the left difference of `a` and `b` (`a \ b`).
/// It takes the [SourceLocation] within `a` that is not in `b` and smaller than `b`.
pub fn sloc_ldiff(a: SourceLocation, b: SourceLocation) -> SourceLocation {
    assert_eq!(a.index, b.index, "The index of `a` and `b` must be the same");
    let length = b.start.zip(a.start).map(|(end, start)| end.saturating_sub(start));
    SourceLocation { start: a.start, length, index: a.index }
}

/// Computes the right difference of `a` and `b` (`a \ b`).
/// It takes the [SourceLocation] within `a` that is not in `b` and larger than `b`.
pub fn sloc_rdiff(a: SourceLocation, b: SourceLocation) -> SourceLocation {
    assert_eq!(a.index, b.index, "The index of `a` and `b` must be the same");
    let start = b.start.zip(b.length).map(|(start, length)| start + length);
    let length = a
        .start
        .zip(a.length)
        .map(|(start, length)| start + length)
        .zip(start)
        .map(|(end, start)| end.saturating_sub(start));
    SourceLocation { start, length, index: a.index }
}

/// Returns the source location of [Statement].
pub fn stmt_src(stmt: &Statement) -> SourceLocation {
    match stmt {
        Statement::Block(block) => block.src,
        Statement::ExpressionStatement(expression_statement) => expression_statement.src,
        Statement::Break(break_stmt) => break_stmt.src,
        Statement::Continue(continue_stmt) => continue_stmt.src,
        Statement::DoWhileStatement(do_while_statement) => do_while_statement.src,
        Statement::EmitStatement(emit_statement) => emit_statement.src,
        Statement::ForStatement(for_statement) => for_statement.src,
        Statement::IfStatement(if_statement) => if_statement.src,
        Statement::InlineAssembly(inline_assembly) => inline_assembly.src,
        Statement::PlaceholderStatement(placeholder_statement) => placeholder_statement.src,
        Statement::Return(return_stmt) => return_stmt.src,
        Statement::RevertStatement(revert_statement) => revert_statement.src,
        Statement::TryStatement(try_statement) => try_statement.src,
        Statement::UncheckedBlock(unchecked_block) => unchecked_block.src,
        Statement::VariableDeclarationStatement(variable_declaration_statement) => {
            variable_declaration_statement.src
        }
        Statement::WhileStatement(while_statement) => while_statement.src,
    }
}

/// Returns the source location of [BlockOrStatement].
pub fn block_or_stmt_src(block_or_stmt: &BlockOrStatement) -> SourceLocation {
    match block_or_stmt {
        BlockOrStatement::Block(block) => block.src,
        BlockOrStatement::Statement(statement) => stmt_src(statement),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! sloc {
        ($start:expr, $length:expr, $index:expr) => {
            SourceLocation { start: Some($start), length: Some($length), index: Some($index) }
        };
    }

    #[test]
    fn test_sloc_ldiff() {
        let a = sloc!(0, 10, 0);
        let b = sloc!(5, 5, 0);
        let c = sloc_ldiff(a, b);
        assert_eq!(c, sloc!(0, 5, 0));

        let a = sloc!(0, 10, 0);
        let b = sloc!(0, 10, 0);
        let c = sloc_ldiff(a, b);
        assert_eq!(c, sloc!(0, 0, 0));

        let a = sloc!(0, 10, 0);
        let b = sloc!(10, 10, 0);
        let c = sloc_ldiff(a, b);
        assert_eq!(c, sloc!(0, 10, 0));

        let a = sloc!(5, 5, 0);
        let b = sloc!(0, 10, 0);
        let c = sloc_ldiff(a, b);
        assert_eq!(c, sloc!(5, 0, 0));
    }

    #[test]
    fn test_sloc_rdiff() {
        let a = sloc!(0, 10, 0);
        let b = sloc!(5, 5, 0);
        let c = sloc_rdiff(a, b);
        assert_eq!(c, sloc!(10, 0, 0));

        let a = sloc!(0, 10, 0);
        let b = sloc!(0, 10, 0);
        let c = sloc_rdiff(a, b);
        assert_eq!(c, sloc!(10, 0, 0));

        let a = sloc!(0, 10, 0);
        let b = sloc!(0, 5, 0);
        let c = sloc_rdiff(a, b);
        assert_eq!(c, sloc!(5, 5, 0));

        let a = sloc!(5, 5, 0);
        let b = sloc!(0, 10, 0);
        let c = sloc_rdiff(a, b);
        assert_eq!(c, sloc!(10, 0, 0));
    }
}
