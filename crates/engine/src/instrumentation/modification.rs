use std::{collections::BTreeMap, fmt::Display};

use crate::{
    analysis::{stmt_src, SourceAnalysis},
    find_next_semicolon_after_source_location, mutability_to_str, next_index_of_source_location,
    slice_source_location, source_string_at_location, visibility_to_str, AnalysisResult, USID,
    UVID,
};
use eyre::Result;
use foundry_compilers::artifacts::{
    ast::SourceLocation, BlockOrStatement, StateMutability, Statement, Visibility,
};

const NORMAL_PRIORITY: u8 = 127;
const LOWEST_PRIORITY: u8 = 0;
const HIGHEST_PRIORITY: u8 = 255;

/// The collections of modifications on a source file.
pub struct SourceModifications {
    source_id: u32,
    /// The modifications on the source file. The key is the location of modification in the original source code.
    modifications: BTreeMap<usize, Modification>,
}

impl SourceModifications {
    /// Creates a new source modifications.
    pub fn new(source_id: u32) -> Self {
        Self { source_id, modifications: BTreeMap::new() }
    }

    /// Adds a modification to the source modifications.
    ///
    /// # Panics
    ///
    /// Panics if the modification overlaps with the previous or next modification.
    pub fn add_modification(&mut self, modification: Modification) {
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
                    if act.priority >= modification.as_instrument_action().priority {
                        act.content = InstrumentContent::Plain(format!(
                            "{} {}",
                            act.content.to_string(),
                            modification.as_instrument_action().content.to_string(),
                        ))
                    } else {
                        act.content = InstrumentContent::Plain(format!(
                            "{} {}",
                            modification.as_instrument_action().content.to_string(),
                            act.content.to_string(),
                        ))
                    }
                });
            }
        }
        // Insert the modification
        self.modifications.insert(loc, modification);
    }

    pub fn extend_modifications(&mut self, modifications: Vec<Modification>) {
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
                        format!("\n{}\n", instrument_action.content).as_str(),
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

#[derive(Debug, Clone, derive_more::From)]
pub enum Modification {
    Instrument(#[from] InstrumentAction),
    Remove(#[from] RemoveAction),
}

impl Modification {
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
    pub fn as_instrument_action(&self) -> &InstrumentAction {
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
    pub fn modify_instrument_action(&mut self, f: impl FnOnce(&mut InstrumentAction)) {
        match self {
            Self::Instrument(instrument_action) => {
                f(instrument_action);
            }
            Self::Remove(_) => {}
        }
    }
}

#[derive(Debug, Clone)]
pub struct InstrumentAction {
    /// The source ID of the source file to instrument
    pub source_id: u32,
    /// The location of the code to instrument. This is the offset of the code at which the instrumented code should be inserted.
    pub loc: usize,
    /// The code to instrument
    pub content: InstrumentContent,
    /// The priority of the instrument action. If two `InstrumentAction`s have the same `loc`, the one with higher priority will be applied first.
    pub priority: u8,
}

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

#[derive(Debug, Clone)]
pub enum InstrumentContent {
    /// The code to instrument. The plain code can be directly inserted into the source code as a string.
    Plain(String),
    /// A `before_step` hook. The debugger will pause here during step-by-step execution.
    BeforeStepHook {
        /// The USID of the step.
        usid: USID,
        /// The number of function calls made in the step.
        function_calls: usize,
    },
    /// A `variable_update` hook. The debugger will record the value of the variable when it is updated.
    VariableUpdateHook(UVID),
}

impl Display for InstrumentContent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Plain(content) => write!(f, "{content}"),
            Self::BeforeStepHook { usid, function_calls } => write!(
                f,
                "address(0x0000000000000000000000000000000000023333).staticcall(abi.encode({}, {}));",
                u64::from(*usid),
                function_calls
            ),
            Self::VariableUpdateHook(_vid) => todo!(),
        }
    }
}

impl SourceModifications {
    /// Collects the modifications on the source code given the analysis result.
    pub fn collect_modifications(&mut self, source: &str, analysis: &SourceAnalysis) -> Result<()> {
        // Collect the modifications on the visibility and mutability of state variables and functions
        self.collect_visibility_and_mutability_modifications(source, analysis)?;

        // Collect the modifications to patch single-statement if/for/while/try/catch/etc.
        self.collect_statement_to_block_modifications(source, analysis)?;

        // Collect the before step hook modifications for each step.
        self.collect_before_step_hook_modifications(source, analysis)?;

        // TODO: Collect the variable update hook modifications for each step.

        Ok(())
    }

    /// Collects the modifications on the visibility of state variables and functions given the source analysis result.
    fn collect_visibility_and_mutability_modifications(
        &mut self,
        source: &str,
        analysis: &SourceAnalysis,
    ) -> Result<()> {
        let source_id = self.source_id;
        let remove_visibility =
            |content: &str, src: SourceLocation, remove: &str| -> Option<RemoveAction> {
                if let Some(visibility_str_index) = content.find(remove) {
                    let visibility_loc =
                        slice_source_location(&src, visibility_str_index, remove.len());
                    let remove_action = RemoveAction { src: visibility_loc };
                    Some(remove_action)
                } else {
                    None
                }
            };

        let add_visibility =
            |src: SourceLocation, add: &str, at_index: usize| -> InstrumentAction {
                let new_visibility_str = format!(" {add} ");

                InstrumentAction {
                    source_id,
                    loc: src.start.unwrap_or(0) + at_index,
                    content: InstrumentContent::Plain(new_visibility_str),
                    priority: NORMAL_PRIORITY,
                }
            };

        for private_state_variable in &analysis.private_state_variables {
            let declaration_str =
                source_string_at_location(source_id, source, &private_state_variable.src);
            // Remove the existing visibility of the state variable
            if let Some(remove_action) = remove_visibility(
                declaration_str,
                private_state_variable.src,
                visibility_to_str(&private_state_variable.visibility),
            ) {
                self.add_modification(remove_action.into());
            }

            // Add the new visibility of the state variable before the variable name, i.e., public
            let at_index =
                declaration_str.find(&private_state_variable.name).unwrap_or_else(|| {
                    panic!("{}", "variable name not found in declaration".to_string())
                });
            let instrument_action = add_visibility(
                private_state_variable.src,
                visibility_to_str(&Visibility::Public),
                at_index,
            );
            self.add_modification(instrument_action.into());
        }

        for private_function in &analysis.private_functions {
            let definition_str =
                source_string_at_location(source_id, source, &private_function.src);
            // Remove the existing visibility of the function
            if let Some(remove_action) = remove_visibility(
                definition_str,
                private_function.src,
                visibility_to_str(&private_function.visibility),
            ) {
                self.add_modification(remove_action.into());
            }

            // Add the new visibility of the function before the function name, i.e., public
            let at_index = definition_str.find(")").expect("function parament list not found") + 1;
            let instrument_action = add_visibility(
                private_function.src,
                visibility_to_str(&Visibility::Public),
                at_index,
            );
            self.add_modification(instrument_action.into());
        }

        for immutable_function in &analysis.immutable_functions {
            let definition_str =
                source_string_at_location(source_id, source, &immutable_function.src);
            if let Some(mutability) = &immutable_function.state_mutability {
                // Remove the existing pure or view mutability of the function
                if matches!(mutability, StateMutability::Pure | StateMutability::View) {
                    if let Some(remove_action) = remove_visibility(
                        definition_str,
                        immutable_function.src,
                        mutability_to_str(mutability),
                    ) {
                        self.add_modification(remove_action.into());
                    }
                }
            }
        }

        Ok(())
    }

    /// Collects the modifications to convert a statement to a block. Some control flow structures, such as if/for/while/try/catch/etc., may have their body as a single statement. We need to convert them to a block.
    fn collect_statement_to_block_modifications(
        &mut self,
        source: &str,
        analysis: &SourceAnalysis,
    ) -> Result<()> {
        let source_id = self.source_id;

        let left_bracket = |loc: usize| -> InstrumentAction {
            InstrumentAction {
                source_id,
                loc,
                content: InstrumentContent::Plain("{".to_string()),
                priority: HIGHEST_PRIORITY,
            }
        };
        let right_bracket = |loc: usize| -> InstrumentAction {
            InstrumentAction {
                source_id,
                loc,
                content: InstrumentContent::Plain("}".to_string()),
                priority: LOWEST_PRIORITY,
            }
        };
        let wrap_statement_as_block = |stmt_src: &SourceLocation| -> Vec<Modification> {
            // The left bracket is inserted just before the statement.
            let left_bracket =
                left_bracket(stmt_src.start.expect("statement start location not found"));

            // The right bracket is inserted just after the end of the statement. However, the `;` of the statement is not included in the source location, so we search the source code for the next `;` after the statement and insert the right bracket after it.
            let right_bracket = right_bracket(
                find_next_semicolon_after_source_location(source, stmt_src)
                    .expect("statement end not found")
                    + 1,
            );

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
            match &step.variant() {
                crate::analysis::StepVariant::IfCondition(if_stmt) => {
                    // modify the true body if needed
                    if let Some(stmt) = indeed_statement(&if_stmt.true_body) {
                        let modifications = wrap_statement_as_block(&stmt_src(stmt));
                        self.extend_modifications(modifications);
                    }

                    // modify the false body if needed
                    if let Some(stmt) =
                        if_stmt.false_body.as_ref().and_then(|body| indeed_statement(body))
                    {
                        let modifications = wrap_statement_as_block(&stmt_src(stmt));
                        self.extend_modifications(modifications);
                    }
                }
                crate::analysis::StepVariant::ForLoop(for_stmt) => {
                    // modify the body if needed
                    if let Some(stmt) = indeed_statement(&for_stmt.body) {
                        let modifications = wrap_statement_as_block(&stmt_src(stmt));
                        self.extend_modifications(modifications);
                    }
                }
                crate::analysis::StepVariant::WhileLoop(while_stmt) => {
                    // modify the body if needed
                    if let Some(stmt) = indeed_statement(&while_stmt.body) {
                        let modifications = wrap_statement_as_block(&stmt_src(stmt));
                        self.extend_modifications(modifications);
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn collect_before_step_hook_modifications(
        &mut self,
        _source: &str,
        analysis: &SourceAnalysis,
    ) -> Result<()> {
        let source_id = self.source_id;
        for step in &analysis.steps {
            let usid = step.usid();
            let variant = step.variant();
            let function_calls = step.function_calls();
            let loc = match variant {
                crate::analysis::StepVariant::FunctionEntry(function_definition) => {
                    // the before step hook should be instrumented before the first statement of the function
                    let Some(body) = &function_definition.body else {
                        // skip the step if the function has no body
                        continue;
                    };
                    // the first char of function body is the '{', so we insert after that.
                    body.src.start.expect("function body start location not found") + 1
                }
                crate::analysis::StepVariant::ModifierEntry(modifier_definition) => {
                    // the before step hook should be instrumented before the first statement of the modifier
                    let Some(body) = &modifier_definition.body else {
                        // skip the step if the modifier has no body
                        continue;
                    };
                    body.src.start.expect("modifier body start location not found") + 1
                }
                crate::analysis::StepVariant::Statement(statement) => {
                    // the before step hook should be instrumented before the statement
                    stmt_src(statement).start.expect("statement start location not found")
                }
                crate::analysis::StepVariant::Statements(statements) => {
                    // the before step hook should be instrumented before the first statement
                    stmt_src(&statements[0]).start.expect("statement start location not found")
                }
                crate::analysis::StepVariant::IfCondition(if_statement) => {
                    // the before step hook should be instrumented before the if statement
                    if_statement.src.start.expect("if statement start location not found")
                }
                crate::analysis::StepVariant::ForLoop(for_statement) => {
                    // the before step hook should be instrumented before the for statement
                    for_statement.src.start.expect("for statement start location not found")
                }
                crate::analysis::StepVariant::WhileLoop(while_statement) => {
                    // the before step hook should be instrumented before the while statement
                    while_statement.src.start.expect("while statement start location not found")
                }
                crate::analysis::StepVariant::DoWhileLoop(do_while_statement) => {
                    // the before step hook should be instrumented before the do-while statement
                    do_while_statement
                        .src
                        .start
                        .expect("do-while statement start location not found")
                }
                crate::analysis::StepVariant::Try(try_statement) => {
                    // the before step hook should be instrumented before the try statement
                    try_statement.src.start.expect("try statement start location not found")
                }
            };
            let instrument_action = InstrumentAction {
                source_id,
                loc,
                content: InstrumentContent::BeforeStepHook { usid, function_calls },
                priority: NORMAL_PRIORITY,
            };
            self.add_modification(instrument_action.into());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::analysis;

    use super::*;

    #[test]
    fn test_collect_state_variable_visibility_modifications() {
        let source = r#"
        contract C {
            uint256 public _x;
            uint256 internal _y;
            uint256 private _z;
        }
        "#;
        let (_sources, analysis) = analysis::tests::compile_and_analyze(source);

        let mut modifications = SourceModifications::new(analysis::tests::TEST_CONTRACT_SOURCE_ID);
        modifications.collect_visibility_and_mutability_modifications(source, &analysis).unwrap();
        assert_eq!(modifications.modifications.len(), 4);
        let modified_source = modifications.modify_source(source);

        // Check there should be no private state variables after modification
        let (_, analysis2) = analysis::tests::compile_and_analyze(&modified_source);
        assert_eq!(analysis2.private_state_variables.len(), 0);
    }

    #[test]
    fn test_collect_function_visibility_modifications() {
        let source = r#"
        contract C {
            function a() public returns (uint256) {}
            function b() external {}
            function c() internal virtual returns (uint256) {}
        }
        "#;
        let (_sources, analysis) = analysis::tests::compile_and_analyze(source);

        let mut modifications = SourceModifications::new(analysis::tests::TEST_CONTRACT_SOURCE_ID);
        modifications.collect_visibility_and_mutability_modifications(source, &analysis).unwrap();
        assert_eq!(modifications.modifications.len(), 2);
        let modified_source = modifications.modify_source(source);

        // Check there should be no private functions after modification
        let (_, analysis2) = analysis::tests::compile_and_analyze(&modified_source);
        assert_eq!(analysis2.private_functions.len(), 0);
    }

    #[test]
    fn test_collect_function_mutability_modifications() {
        let source = r#"
        contract C {
            function a() public pure returns (uint256) {}
            function b() public view returns (uint256) {}
            function c() public payable returns (uint256) {}
        }
        "#;

        let (_sources, analysis) = analysis::tests::compile_and_analyze(source);

        let mut modifications = SourceModifications::new(analysis::tests::TEST_CONTRACT_SOURCE_ID);
        modifications.collect_visibility_and_mutability_modifications(source, &analysis).unwrap();
        assert_eq!(modifications.modifications.len(), 2);
        let modified_source = modifications.modify_source(source);

        // Check there should be no immutable functions after modification
        let (_, analysis2) = analysis::tests::compile_and_analyze(&modified_source);
        assert_eq!(analysis2.immutable_functions.len(), 0);
    }

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
    fn test_collect_function_entry_step_hook_modifications() {
        let source = r#"
        abstract contract C {
            function v() public virtual returns (uint256);

            function a() public returns (uint256) {}
        }
        "#;

        let (_sources, analysis) = analysis::tests::compile_and_analyze(source);

        let mut modifications = SourceModifications::new(analysis::tests::TEST_CONTRACT_SOURCE_ID);
        modifications.collect_before_step_hook_modifications(source, &analysis).unwrap();
        assert_eq!(modifications.modifications.len(), 1);
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
        modifications.collect_before_step_hook_modifications(source, &analysis).unwrap();
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
        modifications.collect_before_step_hook_modifications(source, &analysis).unwrap();
        assert_eq!(modifications.modifications.len(), 1);
        let modified_source = modifications.modify_source(source);

        // The modified source should be able to be compiled and analyzed.
        let (_sources, _analysis2) = analysis::tests::compile_and_analyze(&modified_source);
    }
}
