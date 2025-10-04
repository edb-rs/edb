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

use std::{collections::BTreeMap, fmt::Display, sync::Arc};

use crate::{
    analysis::{stmt_src, AnalysisTypes, IStep, IVariable, SourceAnalysis, StepKind, VariableRef},
    ast::{BlkOrStmt, Stmt},
    find_index_of_first_statement_in_block, find_index_of_first_statement_in_block_or_statement,
    find_next_index_of_last_statement_in_block, find_next_index_of_source_location,
    find_next_index_of_statement,
    instrumentation::codegen,
    USID, UVID,
};

use eyre::Result;
use foundry_compilers::artifacts::{ast::SourceLocation, BlockOrStatement, Statement};
use semver::Version;

const LEFT_BRACKET_PRIORITY: u8 = 255; // used for the left bracket of the block
const FUNCTION_ENTRY_PRIORITY: u8 = 191; // used for the before step hook of function and modifier entry
const VISIBILITY_PRIORITY: u8 = 128; // used for the visibility of state variables and functions
const VARIABLE_UPDATE_PRIORITY: u8 = 127; // used for the variable update hook
const BEFORE_STEP_PRIORITY: u8 = 63; // used for the before step hook of statements other than function and modifier entry.
const RIGHT_BRACKET_PRIORITY: u8 = 0; // used for the right bracket of the block

/// A reference to a version.
pub type VersionRef = Arc<Version>;

/// The collections of modifications on a source file.
pub struct SourceModifications<VAR> {
    source_id: u32,
    /// The modifications on the source file. The key is the location of modification in the original source code.
    modifications: BTreeMap<usize, Modification<VAR>>,
}

impl<VAR: IVariable> SourceModifications<VAR> {
    /// Creates a new source modifications.
    pub fn new(source_id: u32) -> Self {
        Self { source_id, modifications: BTreeMap::new() }
    }

    /// Adds a modification to the source modifications.
    ///
    /// # Panics
    ///
    /// Panics if the modification overlaps with the previous or next modification.
    pub fn add_modification(&mut self, modification: Modification<VAR>) {
        assert_eq!(modification.source_id(), self.source_id, "modification source id mismatch");

        let loc = modification.loc();
        // Check if the modification overlaps with the previous modification
        if let Some((immediate_prev_loc, immediate_prev)) =
            self.modifications.range_mut(..loc).next_back()
        {
            assert!(
                immediate_prev_loc + immediate_prev.modified_length() <= loc,
                "modification location overlaps with previous modification"
            );
        }
        // Check if the modification overlaps with the next modification
        if let Some((immediate_next_loc, immediate_next)) =
            self.modifications.range_mut(loc..).next()
        {
            assert!(
                loc + modification.modified_length() <= *immediate_next_loc,
                "modification location overlaps with next modification"
            );
            // if both of them are instrument actions and instrument at the same location, merge them. The later comming modification will be appended after the earlier one.
            if immediate_next.is_instrument()
                && modification.is_instrument()
                && *immediate_next_loc == loc
            {
                immediate_next.modify_instrument_action(|act| {
                    act.content = if act.priority >= modification.as_instrument_action().priority {
                        InstrumentContent::Plain(format!(
                            "{} {}",
                            act.content,
                            modification.as_instrument_action().content,
                        ))
                    } else {
                        InstrumentContent::Plain(format!(
                            "{} {}",
                            modification.as_instrument_action().content,
                            act.content,
                        ))
                    };
                });
                return;
            }
        }
        // Insert the modification
        self.modifications.insert(loc, modification);
    }

    /// Extends the modifications with the given modifications.
    pub fn extend_modifications(&mut self, modifications: Vec<Modification<VAR>>) {
        for modification in modifications {
            self.add_modification(modification);
        }
    }

    /// Modifies the source code with the modifications.
    pub fn modify_source(&self, source: &str) -> String {
        let mut modified_source = source.to_string();
        // Apply the modifications in reverse order to avoid index shifting
        for (_, modification) in self.modifications.iter().rev() {
            match modification {
                Modification::Instrument(instrument_action) => {
                    modified_source.insert_str(
                        instrument_action.loc,
                        instrument_action.content.to_string().as_str(),
                    );
                }
                Modification::Remove(remove_action) => {
                    modified_source.replace_range(remove_action.start()..remove_action.end(), "");
                }
            }
        }
        modified_source
    }
}

/// The modifications on a source file.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, derive_more::From)]
pub enum Modification<VAR> {
    /// An action to instrument a code in the source file.
    Instrument(#[from] InstrumentAction<VAR>),
    /// An action to remove a code in the source file.
    Remove(#[from] RemoveAction),
}

impl<VAR: IVariable> Modification<VAR> {
    /// Gets the source ID of the modification.
    pub fn source_id(&self) -> u32 {
        match self {
            Self::Instrument(instrument_action) => instrument_action.source_id,
            Self::Remove(remove_action) => {
                remove_action.src.index.expect("remove action index not found") as u32
            }
        }
    }

    /// Gets the location of the modification.
    pub fn loc(&self) -> usize {
        match self {
            Self::Instrument(instrument_action) => instrument_action.loc,
            Self::Remove(remove_action) => remove_action.src.start.unwrap_or(0),
        }
    }

    /// Gets the length of the original code that is modified.
    pub const fn modified_length(&self) -> usize {
        match self {
            Self::Instrument(_) => 0,
            Self::Remove(remove_action) => {
                remove_action.src.length.expect("remove action length not found")
            }
        }
    }

    /// Checks if the modification is an instrument action.
    pub const fn is_instrument(&self) -> bool {
        matches!(self, Self::Instrument(_))
    }

    /// Checks if the modification is a remove action.
    pub const fn is_remove(&self) -> bool {
        matches!(self, Self::Remove(_))
    }

    /// Gets the instrument action if it is an instrument action.
    pub fn as_instrument_action(&self) -> &InstrumentAction<VAR> {
        match self {
            Self::Instrument(instrument_action) => instrument_action,
            Self::Remove(_) => panic!("cannot get instrument action from remove action"),
        }
    }

    /// Gets the remove action if it is a remove action.
    pub fn as_remove_action(&self) -> &RemoveAction {
        match self {
            Self::Instrument(_) => panic!("cannot get remove action from instrument action"),
            Self::Remove(remove_action) => remove_action,
        }
    }

    /// Modifies the remove action if it is a remove action.
    pub fn modify_remove_action(&mut self, f: impl FnOnce(&mut RemoveAction)) {
        match self {
            Self::Instrument(_) => {}
            Self::Remove(remove_action) => {
                f(remove_action);
            }
        }
    }

    /// Modifies the instrument action if it is an instrument action.
    pub fn modify_instrument_action(&mut self, f: impl FnOnce(&mut InstrumentAction<VAR>)) {
        match self {
            Self::Instrument(instrument_action) => {
                f(instrument_action);
            }
            Self::Remove(_) => {}
        }
    }
}

/// An action to instrument a code in the source file.
#[derive(Debug, Clone)]
pub struct InstrumentAction<VAR> {
    /// The source ID of the source file to instrument
    pub source_id: u32,
    /// The location of the code to instrument. This is the offset of the code at which the instrumented code should be inserted.
    pub loc: usize,
    /// The code to instrument
    pub content: InstrumentContent<VAR>,
    /// The priority of the instrument action. If two `InstrumentAction`s have the same `loc`, the one with higher priority will be applied first.
    pub priority: u8,
}

/// An action to remove a code in the source file.
#[derive(Debug, Clone)]
pub struct RemoveAction {
    /// The source location of the code to remove
    pub src: SourceLocation,
}

impl RemoveAction {
    /// Gets the start index of the code to remove.
    pub fn start(&self) -> usize {
        self.src.start.expect("remove action start not found")
    }

    /// Gets the end index of the code to remove (exclusive).
    pub fn end(&self) -> usize {
        self.start() + self.src.length.expect("remove action length not found")
    }
}

/// The content to instrument.
#[derive(Debug, Clone)]
pub enum InstrumentContent<VAR> {
    /// The code to instrument. The plain code can be directly inserted into the source code as a string.
    Plain(String),
    /// View method for state variables
    ViewMethod {
        /// The state variable being accessed.
        variable: VAR,
    },
    /// A `before_step` hook. The debugger will pause here during step-by-step execution.
    BeforeStepHook {
        /// Compiler Version
        version: VersionRef,
        /// The USID of the step.
        usid: USID,
        /// The number of function calls made in the step.
        function_calls: usize,
    },
    /// A `variable_update` hook. The debugger will record the value of the variable when it is updated.
    VariableUpdateHook {
        /// Compiler Version
        version: VersionRef,
        /// The variable that is updated.
        variable: VAR,
    },
}

impl<V: IVariable> Display for InstrumentContent<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let content = match self {
            Self::Plain(content) => content.clone(),
            Self::ViewMethod { variable } => {
                codegen::generate_view_method(variable).unwrap_or_default()
            }
            Self::BeforeStepHook { version, usid, .. } => {
                codegen::generate_step_hook(version, *usid).unwrap_or_default()
            }
            Self::VariableUpdateHook { version, variable } => {
                codegen::generate_variable_update_hook(version, variable).unwrap_or_default()
            }
        };
        write!(f, "{content}")
    }
}

impl<VAR: IVariable> SourceModifications<VAR> {
    /// Collects the modifications on the source code given the analysis result.
    pub fn collect_modifications<T: AnalysisTypes<Variable = VAR>>(
        &mut self,
        compiler_version: VersionRef,
        source: &str,
        analysis: &SourceAnalysis<T>,
    ) -> Result<()> {
        // Collect the modifications to generate view methods for state variables.
        self.collect_view_method_modifications(analysis);

        // Collect the modifications to patch single-statement if/for/while/try/catch/etc.
        self.collect_statement_to_block_modifications(source, analysis)?;

        // Collect the before step hook modifications for each step.
        self.collect_before_step_hook_modifications(compiler_version.clone(), source, analysis)?;

        // Collect the variable update hook modifications for each step.
        self.collect_variable_update_hook_modifications(compiler_version, source, analysis)?;

        Ok(())
    }

    /// Collects the modifications to generate view methods for state variables.
    fn collect_view_method_modifications<T: AnalysisTypes<Variable = VAR>>(
        &mut self,
        analysis: &SourceAnalysis<T>,
    ) {
        let source_id = self.source_id;
        for state_variable in &analysis.state_variables {
            let src = state_variable.declaration_src();
            let loc = src.next_loc() + 1; // XXX (ZZ): we may need to check last char
            let instrument_action = InstrumentAction {
                source_id,
                loc,
                content: InstrumentContent::ViewMethod { variable: state_variable.clone() },
                priority: VISIBILITY_PRIORITY,
            };
            self.add_modification(instrument_action.into());
        }
    }

    /// Collects the modifications to convert a statement to a block. Some control flow structures, such as if/for/while/try/catch/etc., may have their body as a single statement. We need to convert them to a block.
    fn collect_statement_to_block_modifications<T: AnalysisTypes<Variable = VAR>>(
        &mut self,
        source: &str,
        analysis: &SourceAnalysis<T>,
    ) -> Result<()> {
        let source_id = self.source_id;

        let left_bracket = |loc: usize| -> InstrumentAction<VAR> {
            InstrumentAction {
                source_id,
                loc,
                content: InstrumentContent::Plain("{".to_string()),
                priority: LEFT_BRACKET_PRIORITY,
            }
        };
        let right_bracket = |loc: usize| -> InstrumentAction<VAR> {
            InstrumentAction {
                source_id,
                loc,
                content: InstrumentContent::Plain("}".to_string()),
                priority: RIGHT_BRACKET_PRIORITY,
            }
        };
        let wrap_statement_as_block = |stmt: &Stmt| -> Vec<Modification<VAR>> {
            let stmt_src = stmt.src();
            // The left bracket is inserted just before the statement.
            let start_pos = stmt_src.start;
            let left_bracket = left_bracket(start_pos);

            // The right bracket is inserted just after the statement.
            let end_pos = stmt_src.next_loc();
            let right_bracket = right_bracket(end_pos);

            vec![left_bracket.into(), right_bracket.into()]
        };

        fn indeed_statement(block_or_stmt: &BlockOrStatement) -> Option<&Statement> {
            match block_or_stmt {
                BlockOrStatement::Statement(stmt) => match stmt {
                    Statement::Block(_) => None,
                    _ => Some(stmt),
                },
                BlockOrStatement::Block(_) => None,
            }
        }

        for step in &analysis.steps {
            match &step.kind() {
                StepKind::If(if_stmt) => {
                    // modify the true body if needed
                    if let BlkOrStmt::Stmt(stmt) = &if_stmt.true_branch {
                        let modifications = wrap_statement_as_block(stmt);
                        self.extend_modifications(modifications);
                    }

                    // modify the false body if needed
                    if let Some(stmt) = if_stmt.false_branch.as_ref() {
                        if let BlkOrStmt::Stmt(stmt) = stmt {
                            let modifications = wrap_statement_as_block(stmt);
                            self.extend_modifications(modifications);
                        }
                    }
                }
                StepKind::Loop(loop_stmt) => {
                    // modify the body if needed
                    if let BlkOrStmt::Stmt(stmt) = &loop_stmt.body {
                        let modifications = wrap_statement_as_block(stmt);
                        self.extend_modifications(modifications);
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn collect_before_step_hook_modifications<T: AnalysisTypes<Variable = VAR>>(
        &mut self,
        compiler_version: VersionRef,
        source: &str,
        analysis: &SourceAnalysis<T>,
    ) -> Result<()> {
        let source_id = self.source_id;
        for step in &analysis.steps {
            let usid = step.id();
            let kind = step.kind();
            let function_calls = step.function_call_count();
            let (loc, priority) = match kind {
                StepKind::FuncEntry(func) => {
                    // the before step hook should be instrumented before the first statement of the function
                    let Some(body) = &func.body else {
                        // skip the step if the function has no body
                        continue;
                    };
                    // the first char of function body is the '{', so we insert after that.
                    let loc = body.first_stmt_loc();
                    (loc, FUNCTION_ENTRY_PRIORITY)
                }
                StepKind::Stmt(stmt) => {
                    // the before step hook should be instrumented before the statement
                    let loc = stmt.src().start;
                    (loc, BEFORE_STEP_PRIORITY)
                }
                StepKind::If(if_stmt) => {
                    // the before step hook should be instrumented before the if statement
                    let loc = if_stmt.src.start;
                    (loc, BEFORE_STEP_PRIORITY)
                }
                StepKind::Loop(loop_stmt) => {
                    let loc = if loop_stmt.is_do_while {
                        // the before step hook should be instrumented after the last statement of the do-while statement
                        match &loop_stmt.body {
                            BlkOrStmt::Blk(blk) => blk.last_stmt_next_loc(),
                            BlkOrStmt::Stmt(stmt) => stmt.src().next_loc(),
                        }
                    } else {
                        // the before step hook should be instrumented before the for statement
                        loop_stmt.src.start
                    };
                    (loc, BEFORE_STEP_PRIORITY)
                }
                StepKind::Try(try_stmt) => {
                    // the before step hook should be instrumented before the try statement
                    let loc = try_stmt.src.start;
                    (loc, BEFORE_STEP_PRIORITY)
                }
            };
            let instrument_action = InstrumentAction {
                source_id,
                loc,
                content: InstrumentContent::BeforeStepHook {
                    version: compiler_version.clone(),
                    usid,
                    function_calls,
                },
                priority,
            };
            self.add_modification(instrument_action.into());
        }

        Ok(())
    }

    fn collect_variable_update_hook_modifications<T: AnalysisTypes<Variable = VAR>>(
        &mut self,
        compiler_version: VersionRef,
        source: &str,
        analysis: &SourceAnalysis<T>,
    ) -> Result<()> {
        let source_id = self.source_id;
        for step in &analysis.steps {
            let updated_variables = step.updated_variables();
            let locs: Vec<usize> = match step.kind() {
                StepKind::FuncEntry(func) => {
                    // the variable update hook should be instrumented before the first statement of the function
                    let Some(body) = &func.body else {
                        // skip the step if the function has no body
                        continue;
                    };
                    // the first char of function body is the '{', so we insert after that.
                    vec![body.src.start + 1]
                }
                StepKind::Stmt(stmt) => {
                    match stmt {
                        Stmt::If(_) | Stmt::Loop(_) | Stmt::Try(_) => {
                            unreachable!("should not be a statement step")
                        }
                        Stmt::Jump(_) => {
                            // these statement does not have any variable update, so we skip it
                            vec![]
                        }
                        Stmt::Declaration(stmt) => {
                            // the variable update hook should be instrumented after the declaration statement
                            vec![stmt.src.next_loc()]
                        }
                        Stmt::Expression(stmt) => {
                            // the variable update hook should be instrumented after the expression statement
                            vec![stmt.src.next_loc()]
                        }
                        Stmt::InlineAssembly(stmt) => {
                            // the variable update hook should be instrumented after the inline assembly statement
                            vec![stmt.src.next_loc()]
                        }
                        Stmt::Emit(stmt) => {
                            // the variable update hook should be instrumented after the emit statement
                            vec![stmt.src.next_loc()]
                        }
                    }
                }
                StepKind::If(if_stmt) => {
                    // the variable update hook should be instrumented before the first statement of both the true and false bodies
                    let mut locs = vec![];

                    // true branch
                    let loc = if_stmt.true_branch.first_stmt_loc();
                    locs.push(loc);

                    // false branch
                    if let Some(false_branch) = &if_stmt.false_branch {
                        let loc = false_branch.first_stmt_loc();
                        locs.push(loc);
                    }

                    locs
                }
                StepKind::Loop(loop_stmt) => {
                    let loc = if loop_stmt.is_do_while {
                        // the variable update hook should be instrumented after the do-while statement
                        loop_stmt.src.next_loc()
                    } else {
                        // the variable update hook should be instrumented before the first statement of the loop body
                        loop_stmt.body.first_stmt_loc()
                    };
                    vec![loc]
                }
                StepKind::Try(try_stmt) => {
                    // the variable update hook should be instrumented before the first statement in all catch blocks
                    try_stmt.clauses.iter().map(|clause| clause.first_stmt_loc()).collect()
                }
            };
            for loc in locs {
                for updated_variable in &updated_variables {
                    let instrument_action = InstrumentAction {
                        source_id,
                        loc,
                        content: InstrumentContent::VariableUpdateHook {
                            version: compiler_version.clone(),
                            variable: updated_variable.clone(),
                        },
                        priority: VARIABLE_UPDATE_PRIORITY,
                    };

                    self.add_modification(instrument_action.into());
                }
            }
        }
        Ok(())
    }
}

/// Trait to extend Option with better context for instrumentation failures
trait ExpectWithContext<T> {
    fn expect_with_context(
        self,
        error_msg: &str,
        source_id: u32,
        source: &str,
        src_loc: &SourceLocation,
    ) -> T;
}

impl<T> ExpectWithContext<T> for Option<T> {
    fn expect_with_context(
        self,
        error_msg: &str,
        source_id: u32,
        source: &str,
        src_loc: &SourceLocation,
    ) -> T {
        match self {
            Some(value) => value,
            None => {
                // Simple source dump to temp file
                let temp_dir = std::env::temp_dir();
                let dump_path = temp_dir.join(format!("edb_fail_source_{source_id}.sol"));
                let _ = std::fs::write(&dump_path, source);

                // Extract context around the error location with line numbers
                let context = if let Some(start) = src_loc.start {
                    let context_start = start.saturating_sub(200);
                    let context_end = (start + 200).min(source.len());
                    let context_slice = &source[context_start..context_end];

                    // Find line number of the error position
                    let lines_before_context: Vec<&str> =
                        source[..context_start].split('\n').collect();
                    let context_lines: Vec<&str> = context_slice.split('\n').collect();
                    let start_line_num = lines_before_context.len();
                    let error_pos_in_context = start - context_start;

                    // Find which line contains the error
                    let mut current_pos = 0;
                    let mut error_line_idx = 0;
                    let mut error_col = 0;

                    for (idx, line) in context_lines.iter().enumerate() {
                        let line_end = current_pos + line.len();
                        if error_pos_in_context >= current_pos && error_pos_in_context <= line_end {
                            error_line_idx = idx;
                            error_col = error_pos_in_context - current_pos;
                            break;
                        }
                        current_pos = line_end + 1; // +1 for newline
                    }

                    let mut formatted_context = format!(
                        "\n  Source context around line {}:",
                        start_line_num + error_line_idx
                    );

                    for (idx, line) in context_lines.iter().enumerate() {
                        if line.trim().is_empty() && idx != error_line_idx {
                            continue; // Skip empty lines except the error line
                        }

                        let line_num = start_line_num + idx;
                        let marker = if idx == error_line_idx { " --> " } else { "     " };
                        formatted_context.push_str(&format!("\n{marker}{line_num:4} | {line}"));

                        // Add error pointer for the error line
                        if idx == error_line_idx {
                            let pointer = format!(
                                "\n     {} | {}{}^ error here",
                                " ".repeat(4),
                                "_".repeat(error_col),
                                ""
                            );
                            formatted_context.push_str(&pointer);
                        }
                    }

                    formatted_context
                } else {
                    String::new()
                };

                panic!(
                    "{}\n  Source ID: {}\n  Source location: {:?}{}\n  Full source dumped to: {}",
                    error_msg,
                    source_id,
                    src_loc,
                    context,
                    dump_path.display()
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::analysis::{self, tests::compile_and_analyze};

    use super::*;

    #[test]
    fn test_collect_statement_to_block_modifications() {
        let source = r#"
        contract C {
            function a() public returns (uint256) {
                if (false )return 0;

                if (true)return 0;
                else    return 1 ;
            }

            function b() public returns (uint256 x) {
                for (uint256 i = 0; i < 10; i++)x += i
                ;
            }

            function c() public returns (uint256) {
                while (true) return 0;
            }
        }
        "#;

        let (_sources, analysis) = analysis::tests::compile_and_analyze(source);

        let mut modifications = SourceModifications::new(analysis::tests::TEST_CONTRACT_SOURCE_ID);
        modifications.collect_statement_to_block_modifications(source, &analysis).unwrap();
        assert_eq!(modifications.modifications.len(), 10);
        let modified_source = modifications.modify_source(source);

        // The modified source should be able to be compiled and analyzed.
        let (_sources, _analysis2) = analysis::tests::compile_and_analyze(&modified_source);
    }

    #[test]
    fn test_do_while_loop_step_modification() {
        let source = r#"
        contract C {
            function a() public returns (uint256) {
                do {
                    uint x = 1;
                    return 0;
                } while (false);
            }
        }
        "#;

        let (_sources, analysis) = analysis::tests::compile_and_analyze(source);

        let mut modifications = SourceModifications::new(analysis::tests::TEST_CONTRACT_SOURCE_ID);
        let version = Arc::new(Version::parse("0.8.0").unwrap());
        modifications.collect_before_step_hook_modifications(version, source, &analysis).unwrap();
        assert_eq!(modifications.modifications.len(), 4);
        let modified_source = modifications.modify_source(source);

        // The modified source should be able to be compiled and analyzed.
        let (_sources, _analysis2) = analysis::tests::compile_and_analyze(&modified_source);
    }

    #[test]
    fn test_collect_function_entry_step_hook_modifications() {
        let source = r#"
        abstract contract C {
            function v() public virtual returns (uint256);

            function a() public returns (uint256) {
                uint x = 1;
            }
        }
        "#;

        let (_sources, analysis) = analysis::tests::compile_and_analyze(source);

        let mut modifications = SourceModifications::new(analysis::tests::TEST_CONTRACT_SOURCE_ID);
        let version = Arc::new(Version::parse("0.8.0").unwrap());
        modifications.collect_before_step_hook_modifications(version, source, &analysis).unwrap();
        // assert_eq!(modifications.modifications.len(), 1);
        let modified_source = modifications.modify_source(source);

        // The modified source should be able to be compiled and analyzed.
        let (_sources, _analysis2) = analysis::tests::compile_and_analyze(&modified_source);
    }

    #[test]
    fn test_collect_before_step_hook_modifications() {
        let source = r#"
        abstract contract C {
            function a() public returns (uint256) {
                if (false) {return 0;}
                else    {return 1;}
                for (uint256 i = 0; i < 10; i++) {
                    return 0;
                }
                while (true) {
                    return 0;
                }
                do {
                    return 0;
                } while (false);
                try this.a() {
                    return 0;
                }
                catch {}
                return 0;
            }
        }
        "#;

        let (_sources, analysis) = analysis::tests::compile_and_analyze(source);

        let mut modifications = SourceModifications::new(analysis::tests::TEST_CONTRACT_SOURCE_ID);
        let version = Arc::new(Version::parse("0.8.0").unwrap());
        modifications.collect_before_step_hook_modifications(version, source, &analysis).unwrap();
        assert_eq!(modifications.modifications.len(), 13);
        let modified_source = modifications.modify_source(source);

        // The modified source should be able to be compiled and analyzed.
        let (_sources, _analysis2) = analysis::tests::compile_and_analyze(&modified_source);
    }

    #[test]
    fn test_modifier_is_not_step() {
        let source = r#"
        contract C {
            modifier m(uint x) {
                _;
            }

            function a() public m(1) {}
        }
        "#;
        let (_sources, analysis) = analysis::tests::compile_and_analyze(source);

        let mut modifications = SourceModifications::new(analysis::tests::TEST_CONTRACT_SOURCE_ID);
        let version = Arc::new(Version::parse("0.8.0").unwrap());
        modifications.collect_before_step_hook_modifications(version, source, &analysis).unwrap();
        assert_eq!(modifications.modifications.len(), 1);
        let modified_source = modifications.modify_source(source);

        // The modified source should be able to be compiled and analyzed.
        let (_sources, _analysis2) = analysis::tests::compile_and_analyze(&modified_source);
    }

    #[test]
    fn test_else_if_statement_to_block() {
        let source = r#"
contract TestContract {
    function foo() public {
        if (true)
            revert();
        else if (false)
            return;
        else {
            require(true, "error");
        }
    }
}
"#;
        let (_sources, analysis) = compile_and_analyze(source);

        let mut modifications = SourceModifications::new(analysis::tests::TEST_CONTRACT_SOURCE_ID);
        modifications.collect_statement_to_block_modifications(source, &analysis).unwrap();
        assert_eq!(modifications.modifications.len(), 6);
        let modified_source = modifications.modify_source(source);

        // The modified source should be able to be compiled and analyzed.
        let (_sources, _analysis2) = analysis::tests::compile_and_analyze(&modified_source);
    }

    #[test]
    fn test_if_for_statement_to_block() {
        let source = r#"
contract TestContract {
    function foo() public {
        if (true)
            for (uint256 i = 0; i < 10; i++)
                return;
        else
            while (true)
                return;
    }
}
"#;
        let (_sources, analysis) = compile_and_analyze(source);

        let mut modifications = SourceModifications::new(analysis::tests::TEST_CONTRACT_SOURCE_ID);
        modifications.collect_statement_to_block_modifications(source, &analysis).unwrap();
        assert_eq!(modifications.modifications.len(), 6);
        let modified_source = modifications.modify_source(source);

        // The modified source should be able to be compiled and analyzed.
        let (_sources, _analysis2) = analysis::tests::compile_and_analyze(&modified_source);
    }

    #[test]
    fn test_variable_update_hook_modification_for_for_loop() {
        let source = r#"
        contract C {
            function a() public returns (uint256) {
                for (uint i = 0; i < 10; i++) {
                    return i;
                }
            }
        }
        "#;
        let (_sources, analysis) = compile_and_analyze(source);

        let mut modifications = SourceModifications::new(analysis::tests::TEST_CONTRACT_SOURCE_ID);
        modifications
            .collect_variable_update_hook_modifications(
                Arc::new(Version::parse("0.8.0").unwrap()),
                source,
                &analysis,
            )
            .unwrap();
        assert_eq!(modifications.modifications.len(), 1);
        let modified_source = modifications.modify_source(source);

        // The modified source should be able to be compiled and analyzed.
        let (_sources, _analysis2) = analysis::tests::compile_and_analyze(&modified_source);
    }
}
