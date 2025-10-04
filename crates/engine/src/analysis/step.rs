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

use foundry_compilers::artifacts::{
    ast::SourceLocation,
    BlockOrStatement,
    DoWhileStatement,
    Expression,
    ForStatement,
    FunctionCall,
    FunctionDefinition,
    IfStatement,
    ModifierDefinition,
    Statement,
    TryStatement,
    WhileStatement, // SourceUnit,
};
use serde::{Deserialize, Serialize};

use crate::analysis::{
    macros::{define_ref, universal_id},
    VariableRef, VariableScopeRef, UFID,
};

universal_id! {
    /// A Universal Step Identifier (USID) is a unique identifier for a step in contract execution.
    USID => 0
}

define_ref! {
    /// A reference-counted pointer to a Step for efficient sharing across multiple contexts.
    ///
    /// This type alias provides thread-safe reference counting for Step instances,
    /// allowing them to be shared between different parts of the analysis system
    /// without copying the entire step data.
    StepRef(Step) {
        clone_field: {
            usid: USID,
            ufid: UFID,
            src: SourceLocation,
        }
        cached_field: {
            variant: StepVariant,
            updated_variables: Vec<VariableRef>,
            accessible_variables: Vec<VariableRef>,
        }
        additional_cache: {
            function_calls: usize,
        }
    }
}

impl StepRef {
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

        // Corner case 2: built-in statements
        static BUILT_IN_FUNCTIONS: &[&str] =
            &["require", "assert", "keccak256", "sha256", "ripemd160", "ecrecover", "type"];
        let built_in_n = calls
            .iter()
            .filter(|call| {
                if let Expression::Identifier(ref id) = call.expression {
                    BUILT_IN_FUNCTIONS.contains(&id.name.as_str())
                } else {
                    false
                }
            })
            .count();
        function_calls = function_calls.saturating_sub(built_in_n);

        *self.function_calls.get_or_init(|| function_calls)
    }

    /// Check whether this step is an entry of a function
    pub fn function_entry(&self) -> Option<UFID> {
        if let StepVariant::FunctionEntry(_) = self.variant() {
            Some(self.read().ufid)
        } else {
            None
        }
    }

    /// Check whether this step is an entry of a modifier
    pub fn modifier_entry(&self) -> Option<UFID> {
        if let StepVariant::ModifierEntry(_) = self.variant() {
            Some(self.read().ufid)
        } else {
            None
        }
    }

    /// Check whether this step contains return statements
    pub fn contains_return(&self) -> bool {
        match self.variant() {
            StepVariant::Statement(Statement::Return(..)) => true,
            StepVariant::Statements(stmts) => {
                stmts.iter().any(|s| matches!(s, Statement::Return(..)))
            }
            // Other variants will do tag a return as a single step
            _ => false,
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
    /// Variables accessible in this step (excluding those declared in this step)
    pub accessible_variables: Vec<VariableRef>,
    /// Variables declared in this step
    pub declared_variables: Vec<VariableRef>,
    /// Variables updated in this step
    pub updated_variables: Vec<VariableRef>,
    /// The scope of this step
    pub scope: VariableScopeRef,
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
        accessible_variables: Vec<VariableRef>,
    ) -> Self {
        let usid = USID::next();
        Self {
            usid,
            ufid,
            variant,
            src,
            function_calls: vec![],
            accessible_variables,
            declared_variables: vec![],
            updated_variables: vec![],
            scope,
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
