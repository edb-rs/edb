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
use delegate::delegate;
use foundry_compilers::artifacts::{
    ast::SourceLocation,
    BlockOrStatement,
    DoWhileStatement,
    Expression,
    ExpressionOrVariableDeclarationStatement,
    ExpressionStatement,
    ForStatement,
    FunctionCall,
    FunctionDefinition,
    IfStatement,
    ModifierDefinition,
    Statement,
    TryStatement,
    VariableDeclaration,
    WhileStatement, // SourceUnit,
};
use lazy_static::lazy_static;
use once_cell::sync::OnceCell;
use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use serde::{Deserialize, Serialize};

use crate::analysis::{macros::universal_id, StepHook, VariableScope, VariableScopeRef, UVID};

universal_id! {
    /// A Universal Step Identifier (USID) is a unique identifier for a step in contract execution.
    USID => 0
}

universal_id! {
    /// A Universal Function Identifier (UFID) is a unique identifier for a function in contract execution.
    UFID => 0
}

/// A reference-counted pointer to a Step for efficient sharing across multiple contexts.
///
/// This type alias provides thread-safe reference counting for Step instances,
/// allowing them to be shared between different parts of the analysis system
/// without copying the entire step data.
#[derive(Debug, Clone)]
pub struct StepRef {
    inner: Arc<RwLock<Step>>,
    /* cached readonly fields*/
    usid: OnceCell<USID>,
    variant: OnceCell<StepVariant>,
    function_calls: OnceCell<usize>,
}

impl From<Step> for StepRef {
    fn from(step: Step) -> Self {
        Self::new(step)
    }
}

impl Serialize for StepRef {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        // Serialize the inner Step directly
        self.inner.read().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for StepRef {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // Deserialize as Step and wrap it in StepRef
        let step = Step::deserialize(deserializer)?;
        Ok(Self::new(step))
    }
}

impl StepRef {
    pub fn new(inner: Step) -> Self {
        Self {
            inner: Arc::new(RwLock::new(inner)),
            usid: OnceCell::new(),
            variant: OnceCell::new(),
            function_calls: OnceCell::new(),
        }
    }

    pub(crate) fn read(&self) -> RwLockReadGuard<'_, Step> {
        self.inner.read()
    }

    pub(crate) fn write(&self) -> RwLockWriteGuard<'_, Step> {
        self.inner.write()
    }

    /// Returns the USID of this step.
    pub fn usid(&self) -> USID {
        *self.usid.get_or_init(|| self.inner.read().usid)
    }

    /// Returns the variant of this step.
    pub fn variant(&self) -> &StepVariant {
        self.variant.get_or_init(|| self.inner.read().variant.clone())
    }

    /// Returns the number of function calls made in this step.
    pub fn function_calls(&self) -> usize {
        // XXX (ZZ): a relatively hacky way to handle corner cases
        let calls = &self.inner.read().function_calls;
        let mut function_calls = calls.len();

        // Corner case 1: emit statement(s)
        // In EmitStatement, an event is also considered as a function call, for which
        // we need to reduce the count by 1.
        match self.variant() {
            StepVariant::Statement(Statement::EmitStatement { .. }) => {
                function_calls = function_calls.saturating_sub(1);
            }
            StepVariant::Statements(ref stmts) => {
                let emit_n = stmts
                    .iter()
                    .filter(|stmt| matches!(stmt, Statement::EmitStatement { .. }))
                    .count();
                function_calls = function_calls.saturating_sub(emit_n);
            }
            _ => {}
        }

        // Corner case 2: require statements
        let require_n = calls
            .iter()
            .filter(|call| {
                if let Expression::Identifier(ref id) = call.expression {
                    id.name == "require"
                } else {
                    false
                }
            })
            .count();
        function_calls = function_calls.saturating_sub(require_n);

        *self.function_calls.get_or_init(|| function_calls)
    }
}

impl StepRef {
    delegate! {
        to self.inner.read() {
        }
    }
}

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
    /// The identifier of the function that this step belongs to.
    pub ufid: UFID,
    /// The specific type of step (statement, expression, etc.)
    pub variant: StepVariant,
    /// Source location information (file, line, column)
    pub src: SourceLocation,
    /// Function calls made in this step
    pub function_calls: Vec<FunctionCall>,
    /// Variables declared in this step
    pub declared_variables: Vec<VariableDeclaration>,
    /// The scope of this step
    pub scope: VariableScopeRef,
    /// Hooks to execute before this step
    #[deprecated]
    pub pre_hooks: Vec<StepHook>,
    /// Hooks to execute after this step
    #[deprecated]
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
    pub fn new(
        ufid: UFID,
        variant: StepVariant,
        src: SourceLocation,
        scope: VariableScopeRef,
    ) -> Self {
        let usid = USID::next();
        let mut this = Self {
            usid,
            ufid,
            variant,
            src,
            function_calls: vec![],
            declared_variables: vec![],
            scope,
            pre_hooks: vec![],
            post_hooks: vec![],
        };
        match &this.variant {
            StepVariant::FunctionEntry(_) => {}
            _ => {
                this.pre_hooks.push(StepHook::BeforeStep(usid));
            }
        }
        this
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
    #[deprecated]
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
    /// A modifier entry step.
    ModifierEntry(ModifierDefinition),
    /// A single statement that is executed in a single debug step.
    Statement(Statement),
    /// A consecutive list of statements that are executed in a single debug step.
    Statements(Vec<Statement>),
    /// The condition of an if statement that is executed in a single debug step.
    IfCondition(IfStatement),
    /// The header of a for loop that is executed in a single debug step.
    ForLoop(ForStatement),
    /// The condition of a while loop that is executed in a single debug step.
    WhileLoop(WhileStatement),
    /// The header of a do-while loop that is executed in a single debug step.
    DoWhileLoop(DoWhileStatement),
    /// The try external call
    Try(TryStatement),
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
