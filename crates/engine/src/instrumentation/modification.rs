use std::{collections::BTreeMap, fmt::Display};

use crate::{
    analysis::{stmt_src, SourceAnalysis, VariableRef},
    contains_function_type, contains_mapping_type, contains_user_defined_type,
    find_index_of_first_statement_in_block, find_index_of_first_statement_in_block_or_statement,
    find_next_index_of_last_statement_in_block, find_next_index_of_source_location,
    find_next_index_of_statement,
    instrumentation::codegen,
    mutability_to_str, slice_source_location, source_string_at_location, HOOK_TRIGGER_ADDRESS,
    USID, UVID, VARIABLE_UPDATE_ADDRESS,
};

use eyre::Result;
use foundry_compilers::artifacts::{
    ast::SourceLocation, BlockOrStatement, Mutability, StateMutability, Statement,
};
use semver::Version;

const LEFT_BRACKET_PRIORITY: u8 = 255; // used for the left bracket of the block
const FUNCTION_ENTRY_PRIORITY: u8 = 191; // used for the before step hook of function and modifier entry
const VISIBILITY_PRIORITY: u8 = 128; // used for the visibility of state variables and functions
const VARIABLE_UPDATE_PRIORITY: u8 = 127; // used for the variable update hook
const BEFORE_STEP_PRIORITY: u8 = 63; // used for the before step hook of statements other than function and modifier entry.
const RIGHT_BRACKET_PRIORITY: u8 = 0; // used for the right bracket of the block

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
    VariableUpdateHook {
        /// The UVID of the variable.
        uvid: UVID,
        /// The variable that is updated.
        variable: VariableRef,
    },
}

impl Display for InstrumentContent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Plain(content) => write!(f, "{content}"),
            Self::BeforeStepHook { usid, .. } => write!(
                f,
                "address({:?}).call(hex\"{:064x}\");",
                HOOK_TRIGGER_ADDRESS,
                u64::from(*usid),
            ),
            Self::VariableUpdateHook { uvid, variable } => {
                let base_var = variable.base();
                let base_name = &base_var.declaration().name;
                write!(
                    f,
                    "address({:?}).call(abi.encode(hex\"{:064x}\", {}));",
                    VARIABLE_UPDATE_ADDRESS,
                    u64::from(*uvid),
                    base_name,
                )
            }
        }
    }
}

impl SourceModifications {
    /// Collects the modifications on the source code given the analysis result.
    pub fn collect_modifications(
        &mut self,
        compiler_version: &Version,
        source: &str,
        analysis: &SourceAnalysis,
    ) -> Result<()> {
        // Collect the modifications on the visibility and mutability of state variables and functions
        self.collect_visibility_and_mutability_modifications(source, analysis)?;

        // Collect the modifications to patch single-statement if/for/while/try/catch/etc.
        self.collect_statement_to_block_modifications(source, analysis)?;

        // Collect the before step hook modifications for each step.
        self.collect_before_step_hook_modifications(source, analysis)?;

        // Collect the variable update hook modifications for each step.
        self.collect_variable_update_hook_modifications(compiler_version, source, analysis)?;

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

        for state_variable in &analysis.state_variables {
            let Some(view_function) = codegen::generate_view_method(state_variable) else {
                // The state variable contains user-defined types.
                continue;
            };

            if state_variable.declaration().mutability == Some(Mutability::Constant) {
                // We do not need to output constant state variables
                continue;
            }

            let src = &state_variable.declaration().src;
            let loc = src.start.unwrap_or(0) + src.length.unwrap_or(0) + 1; // XXX (ZZ): we may need to check last char
            let instrument_action = InstrumentAction {
                source_id,
                loc,
                content: InstrumentContent::Plain(view_function),
                priority: VISIBILITY_PRIORITY,
            };
            self.add_modification(instrument_action.into());
        }

        // We will manually generate view functions for private state variables
        #[cfg(any())]
        let add_visibility =
            |src: SourceLocation, add: &str, at_index: usize| -> InstrumentAction {
                let new_visibility_str = format!(" {add} ");

                InstrumentAction {
                    source_id,
                    loc: src.start.unwrap_or(0) + at_index,
                    content: InstrumentContent::Plain(new_visibility_str),
                    priority: VISIBILITY_PRIORITY,
                }
            };

        // We will manually generate view functions for private state variables
        #[cfg(any())]
        for private_state_variable in &analysis.private_state_variables {
            let declaration_str = source_string_at_location(
                source_id,
                source,
                &private_state_variable.declaration().src,
            );
            // Remove the existing visibility of the state variable
            if let Some(remove_action) = remove_visibility(
                declaration_str,
                private_state_variable.declaration().src,
                visibility_to_str(&private_state_variable.declaration().visibility),
            ) {
                self.add_modification(remove_action.into());
            }

            // Add the new visibility of the state variable just before the variable name
            let at_index = declaration_str
                .rfind(&private_state_variable.declaration().name)
                .expect("variable name not found");
            let instrument_action = add_visibility(
                private_state_variable.declaration().src,
                visibility_to_str(&Visibility::Public),
                at_index,
            );
            self.add_modification(instrument_action.into());
        }

        // We temporarily disable private function visibility modifications
        #[cfg(any())]
        for private_function in &analysis.private_functions {
            let definition_str =
                source_string_at_location(source_id, source, &private_function.src());
            // Remove the existing visibility of the function
            if let Some(remove_action) = remove_visibility(
                definition_str,
                private_function.src(),
                visibility_to_str(&private_function.visibility()),
            ) {
                self.add_modification(remove_action.into());
            }

            // Add the new visibility of the function after the argument list
            let at_index = definition_str.find(")").expect("function parament list not found") + 1;
            let instrument_action = add_visibility(
                private_function.src(),
                visibility_to_str(&Visibility::Public),
                at_index,
            );
            self.add_modification(instrument_action.into());
        }

        for immutable_function in &analysis.immutable_functions {
            let definition_str =
                source_string_at_location(source_id, source, &immutable_function.src());
            if let Some(mutability) = &immutable_function.state_mutability() {
                // Remove the existing pure or view mutability of the function
                if matches!(mutability, StateMutability::Pure | StateMutability::View) {
                    if let Some(remove_action) = remove_visibility(
                        definition_str,
                        immutable_function.src(),
                        mutability_to_str(&mutability),
                    ) {
                        self.add_modification(remove_action.into());
                    }
                }
            }
        }

        for function_type in &analysis.function_types {
            let function_type_str =
                source_string_at_location(source_id, source, &function_type.src());

            let mutability = function_type.state_mutability();
            if matches!(mutability, StateMutability::Pure | StateMutability::View) {
                // Remove the existing visibility of the function type
                if let Some(remove_action) = remove_visibility(
                    function_type_str,
                    function_type.src(),
                    mutability_to_str(&mutability),
                ) {
                    self.add_modification(remove_action.into());
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
                priority: LEFT_BRACKET_PRIORITY,
            }
        };
        let right_bracket = |loc: usize| -> InstrumentAction {
            InstrumentAction {
                source_id,
                loc,
                content: InstrumentContent::Plain("}".to_string()),
                priority: RIGHT_BRACKET_PRIORITY,
            }
        };
        let wrap_statement_as_block = |stmt: &Statement| -> Vec<Modification> {
            let stmt_src = stmt_src(stmt);
            // The left bracket is inserted just before the statement.
            let start_pos = stmt_src.start.expect_with_context(
                "statement start location not found",
                source_id,
                source,
                &stmt_src,
            );
            let left_bracket = left_bracket(start_pos);

            // The right bracket is inserted just after the statement.
            let end_pos =
                find_next_index_of_statement(source, stmt).expect("statement end not found");
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
            match &step.variant() {
                crate::analysis::StepVariant::IfCondition(if_stmt) => {
                    // modify the true body if needed
                    if let Some(stmt) = indeed_statement(&if_stmt.true_body) {
                        let modifications = wrap_statement_as_block(stmt);
                        self.extend_modifications(modifications);
                    }

                    // modify the false body if needed
                    if let Some(stmt) =
                        if_stmt.false_body.as_ref().and_then(|body| indeed_statement(body))
                    {
                        let modifications = wrap_statement_as_block(stmt);
                        self.extend_modifications(modifications);
                    }
                }
                crate::analysis::StepVariant::ForLoop(for_stmt) => {
                    // modify the body if needed
                    if let Some(stmt) = indeed_statement(&for_stmt.body) {
                        let modifications = wrap_statement_as_block(stmt);
                        self.extend_modifications(modifications);
                    }
                }
                crate::analysis::StepVariant::WhileLoop(while_stmt) => {
                    // modify the body if needed
                    if let Some(stmt) = indeed_statement(&while_stmt.body) {
                        let modifications = wrap_statement_as_block(stmt);
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
        source: &str,
        analysis: &SourceAnalysis,
    ) -> Result<()> {
        let source_id = self.source_id;
        for step in &analysis.steps {
            let usid = step.usid();
            let variant = step.variant();
            let function_calls = step.function_calls();
            let (loc, priority) = match variant {
                crate::analysis::StepVariant::FunctionEntry(function_definition) => {
                    // the before step hook should be instrumented before the first statement of the function
                    let Some(body) = &function_definition.body else {
                        // skip the step if the function has no body
                        continue;
                    };
                    // the first char of function body is the '{', so we insert after that.
                    let loc = find_index_of_first_statement_in_block(body)
                        .expect("function body start location not found");
                    (loc, FUNCTION_ENTRY_PRIORITY)
                }
                crate::analysis::StepVariant::ModifierEntry(modifier_definition) => {
                    // the before step hook should be instrumented before the first statement of the modifier
                    let Some(body) = &modifier_definition.body else {
                        // skip the step if the modifier has no body
                        continue;
                    };
                    let loc = find_index_of_first_statement_in_block(body)
                        .expect("modifier body start location not found");
                    (loc, FUNCTION_ENTRY_PRIORITY)
                }
                crate::analysis::StepVariant::Statement(statement) => {
                    // the before step hook should be instrumented before the statement
                    let loc =
                        stmt_src(statement).start.expect("statement start location not found");
                    (loc, BEFORE_STEP_PRIORITY)
                }
                crate::analysis::StepVariant::Statements(statements) => {
                    // the before step hook should be instrumented before the first statement
                    let loc =
                        stmt_src(&statements[0]).start.expect("statement start location not found");
                    (loc, BEFORE_STEP_PRIORITY)
                }
                crate::analysis::StepVariant::IfCondition(if_statement) => {
                    // the before step hook should be instrumented before the if statement
                    let loc =
                        if_statement.src.start.expect("if statement start location not found");
                    (loc, BEFORE_STEP_PRIORITY)
                }
                crate::analysis::StepVariant::ForLoop(for_statement) => {
                    // the before step hook should be instrumented before the for statement
                    let loc =
                        for_statement.src.start.expect("for statement start location not found");
                    (loc, BEFORE_STEP_PRIORITY)
                }
                crate::analysis::StepVariant::WhileLoop(while_statement) => {
                    // the before step hook should be instrumented before the while statement
                    let loc = while_statement
                        .src
                        .start
                        .expect("while statement start location not found");
                    (loc, BEFORE_STEP_PRIORITY)
                }
                crate::analysis::StepVariant::DoWhileLoop(do_while_statement) => {
                    // the before step hook should be instrumented after the last statement of the do-while statement
                    let loc = find_next_index_of_last_statement_in_block(
                        source,
                        &do_while_statement.body,
                    )
                    .expect("do-while statement last statement location not found");
                    (loc, BEFORE_STEP_PRIORITY)
                }
                crate::analysis::StepVariant::Try(try_statement) => {
                    // the before step hook should be instrumented before the try statement
                    let loc =
                        try_statement.src.start.expect("try statement start location not found");
                    (loc, BEFORE_STEP_PRIORITY)
                }
            };
            let instrument_action = InstrumentAction {
                source_id,
                loc,
                content: InstrumentContent::BeforeStepHook { usid, function_calls },
                priority,
            };
            self.add_modification(instrument_action.into());
        }

        Ok(())
    }

    fn collect_variable_update_hook_modifications(
        &mut self,
        compiler_version: &Version,
        source: &str,
        analysis: &SourceAnalysis,
    ) -> Result<()> {
        if compiler_version > &Version::parse("0.4.24").unwrap() {
            // if the abi.encode function is not available, we skip the variable update hook
            // TODO: support solidity <0.4.24 in the future
            return Ok(());
        }

        let source_id = self.source_id;
        for step in &analysis.steps {
            let updated_variables = &step.read().updated_variables;
            let locs: Vec<usize> = match step.variant() {
                crate::analysis::StepVariant::FunctionEntry(function_definition) => {
                    // the variable update hook should be instrumented before the first statement of the function
                    let Some(body) = &function_definition.body else {
                        // skip the step if the function has no body
                        continue;
                    };
                    // the first char of function body is the '{', so we insert after that.
                    vec![body.src.start.expect("function body start location not found") + 1]
                }
                crate::analysis::StepVariant::ModifierEntry(modifier_definition) => {
                    // the variable update hook should be instrumented before the first statement of the modifier
                    let Some(body) = &modifier_definition.body else {
                        // skip the step if the modifier has no body
                        continue;
                    };
                    vec![body.src.start.expect("modifier body start location not found") + 1]
                }
                crate::analysis::StepVariant::Statement(statement) => {
                    match statement {
                        Statement::Block(_)
                        | Statement::UncheckedBlock(_)
                        | Statement::DoWhileStatement(_)
                        | Statement::ForStatement(_)
                        | Statement::IfStatement(_)
                        | Statement::TryStatement(_)
                        | Statement::WhileStatement(_) => {
                            unreachable!("should not be a statement step")
                        }
                        Statement::Break(_)
                        | Statement::Continue(_)
                        | Statement::PlaceholderStatement(_)
                        | Statement::Return(_)
                        | Statement::RevertStatement(_) => {
                            // these statement does not have any variable update, so we skip it
                            vec![]
                        }
                        Statement::EmitStatement(_)
                        | Statement::ExpressionStatement(_)
                        | Statement::InlineAssembly(_)
                        | Statement::VariableDeclarationStatement(_) => {
                            // the variable update hook should be instrumented after the emit statement
                            find_next_index_of_statement(source, statement)
                                .map(|loc| vec![loc])
                                .unwrap_or_default()
                        }
                    }
                }
                crate::analysis::StepVariant::Statements(statements) => {
                    // the variable update hook should be instrumented after the statments
                    statements
                        .last()
                        .and_then(|stmt| {
                            find_next_index_of_statement(source, stmt).map(|loc| vec![loc])
                        })
                        .unwrap_or_default()
                }
                crate::analysis::StepVariant::IfCondition(if_statement) => {
                    // the variable update hook should be instrumented before the first statement of both the true and false bodies
                    let mut locs = find_index_of_first_statement_in_block_or_statement(
                        &if_statement.true_body,
                    )
                    .map(|loc| vec![loc])
                    .unwrap_or_default();
                    if let Some(false_loc) = if_statement.false_body.as_ref().and_then(|body| {
                        find_index_of_first_statement_in_block_or_statement(body)
                            .map(|loc| vec![loc])
                    }) {
                        locs.extend(false_loc.into_iter());
                    }
                    locs
                }
                crate::analysis::StepVariant::ForLoop(for_statement) => {
                    // the variable update hook should be instrumented before the first statement of the for statement
                    find_index_of_first_statement_in_block_or_statement(&for_statement.body)
                        .map(|loc| vec![loc])
                        .unwrap_or_default()
                }
                crate::analysis::StepVariant::WhileLoop(while_statement) => {
                    // the variable update hook should be instrumented before the first statement of the while statement
                    find_index_of_first_statement_in_block_or_statement(&while_statement.body)
                        .map(|loc| vec![loc])
                        .unwrap_or_default()
                }
                crate::analysis::StepVariant::DoWhileLoop(do_while_statement) => {
                    // the variable update hook should be instrumented before the do-while statement
                    find_next_index_of_source_location(&do_while_statement.src)
                        .map(|loc| vec![loc])
                        .unwrap_or_default()
                }
                crate::analysis::StepVariant::Try(try_statement) => {
                    // the variable update hook should be instrumented before the first statement in all catch blocks
                    try_statement
                        .clauses
                        .iter()
                        .filter_map(|clause| find_index_of_first_statement_in_block(&clause.block))
                        .collect()
                }
            };
            for loc in locs {
                for updated_variable in updated_variables {
                    let uvid = updated_variable.id();
                    let instrument_action = InstrumentAction {
                        source_id,
                        loc,
                        content: InstrumentContent::VariableUpdateHook {
                            uvid,
                            variable: updated_variable.clone(),
                        },
                        priority: VARIABLE_UPDATE_PRIORITY,
                    };
                    let declaration = updated_variable.declaration();
                    let base_type = &declaration.type_name;
                    let is_state_variable = declaration.state_variable;
                    // we currently do not support recording variables involving user-defined types and arrays, as well as state variables.
                    // in addition, source code with 0.4.x solidity version is not supported due to the lack of the `abi.encode` function.
                    // TODO: support user-defined types and arrays, as well as state variables, solidity <0.4.24, in the future
                    if base_type.as_ref().is_some_and(|ty| {
                        !contains_user_defined_type(ty)
                            && !contains_function_type(ty)
                            && !contains_mapping_type(ty)
                            && !is_state_variable
                    }) {
                        self.add_modification(instrument_action.into());
                    }
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
                let dump_path = temp_dir.join(format!("edb_fail_source_{}.sol", source_id));
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
                        formatted_context
                            .push_str(&format!("\n{}{:4} | {}", marker, line_num, line));

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
        modifications.collect_before_step_hook_modifications(source, &analysis).unwrap();
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
        modifications.collect_before_step_hook_modifications(source, &analysis).unwrap();
        // assert_eq!(modifications.modifications.len(), 1);
        let modified_source = modifications.modify_source(source);
        println!("{}", modified_source);

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
    // possible fix: add `pragma abicoder v2;` to the contract
    fn test_struct_variable_with_array_field() {
        let source = r#"
        contract C {
            struct A {
                uint[] a;
            }
            A internal a;
        }
        "#;
        let (_sources, analysis) = compile_and_analyze(source);

        let mut modifications = SourceModifications::new(analysis::tests::TEST_CONTRACT_SOURCE_ID);
        modifications.collect_visibility_and_mutability_modifications(source, &analysis).unwrap();
        assert_eq!(modifications.modifications.len(), 0);
        let modified_source = modifications.modify_source(source);

        // The modified source should be able to be compiled and analyzed.
        let (_sources, _analysis2) = analysis::tests::compile_and_analyze(&modified_source);
    }

    #[test]
    fn test_struct_variable_with_mapping_field() {
        let source = r#"
        contract C {
            struct A {
                mapping(uint => address) a;
            }
            A internal a;
        }
        "#;
        let (_sources, analysis) = compile_and_analyze(source);

        let mut modifications = SourceModifications::new(analysis::tests::TEST_CONTRACT_SOURCE_ID);
        modifications.collect_visibility_and_mutability_modifications(source, &analysis).unwrap();
        assert_eq!(modifications.modifications.len(), 0);
        let modified_source = modifications.modify_source(source);

        // The modified source should be able to be compiled and analyzed.
        let (_sources, _analysis2) = analysis::tests::compile_and_analyze(&modified_source);
    }
}
