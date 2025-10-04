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

use std::path::PathBuf;

use foundry_compilers::artifacts::{
    ast::SourceLocation, Assignment, Block, ContractDefinition, EnumDefinition, ErrorDefinition,
    EventDefinition, Expression, ForStatement, FunctionCall, FunctionCallKind, FunctionDefinition,
    ModifierDefinition, Mutability, PragmaDirective, SourceUnit, StateMutability, Statement,
    StructDefinition, TypeName, UncheckedBlock, UserDefinedValueTypeDefinition,
    VariableDeclaration, Visibility,
};
use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{debug, error};

use crate::{
    // new_usid, AnnotationsToChange,
    analysis::{
        visitor::VisitorAction, Contract, ContractRef, Function, FunctionRef, FunctionTypeNameRef,
        ScopeNode, Step, StepRef, StepVariant, UserDefinedType, UserDefinedTypeRef,
        UserDefinedTypeVariant, Variable, VariableScope, VariableScopeRef, Visitor, Walk, UCID,
        UFID, UTID,
    },
    block_or_stmt_src,
    contains_user_defined_type,
    sloc_ldiff,
    sloc_rdiff,
    VariableRef,
    USID,
    UVID,
};

/// Analysis results for a single source file.
///
/// Contains all the analysis data for one Solidity source file, including the original
/// source content, parsed AST, and step-by-step analysis results.
///
/// # Fields
///
/// - `id`: Unique identifier for this source file
/// - `path`: File system path to the source file
/// - `source`: Original source content and metadata
/// - `ast`: Parsed Abstract Syntax Tree
/// - `unit`: Processed source unit ready for analysis
/// - `steps`: List of analyzed execution steps in this file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceAnalysis {
    /// Unique identifier for this source file
    pub id: u32,
    /// File system path to the source file
    pub path: PathBuf,
    /// Processed source unit ready for analysis
    pub unit: SourceUnit,
    /// Global variable scope of the source file
    pub global_scope: VariableScopeRef,
    /// List of analyzed execution steps in this file
    pub steps: Vec<StepRef>,
    /// State variables that should be made public
    pub private_state_variables: Vec<VariableRef>,
    /// List of all contracts in this file.
    pub contracts: Vec<ContractRef>,
    /// List of all functions in this file.
    pub functions: Vec<FunctionRef>,
    /// List of all state variables in this file.
    pub state_variables: Vec<VariableRef>,
    /// Functions that should be made public
    pub private_functions: Vec<FunctionRef>,
    /// Functions that should be made mutable (i.e., neither pure nor view)
    pub immutable_functions: Vec<FunctionRef>,
    /// Variables that are defined as function types
    pub function_types: Vec<FunctionTypeNameRef>,
    /// User defined types defined in this file.
    pub user_defined_types: Vec<UserDefinedTypeRef>,
}

impl SourceAnalysis {
    /// Returns a mapping of all variables in this source file by their UVID.
    ///
    /// This method traverses the entire variable scope tree and collects all
    /// variables into a flat HashMap for efficient lookup.
    ///
    /// # Returns
    ///
    /// A HashMap mapping UVIDs to their corresponding VariableRef instances.
    pub fn variable_table(&self) -> HashMap<UVID, VariableRef> {
        let mut table = HashMap::default();
        fn walk_scope(scope: &VariableScopeRef, table: &mut HashMap<UVID, VariableRef>) {
            for variable in scope.declared_variables() {
                table.insert(variable.read().id(), variable.clone());
            }
            for child in scope.children() {
                walk_scope(child, table);
            }
        }
        walk_scope(&self.global_scope, &mut table);
        table
    }

    /// Returns a mapping of all steps in this source file by their USID.
    ///
    /// This method creates a HashMap for efficient step lookup by their
    /// unique step identifiers.
    ///
    /// # Returns
    ///
    /// A HashMap mapping USIDs to their corresponding StepRef instances.
    pub fn step_table(&self) -> HashMap<USID, StepRef> {
        let mut table = HashMap::default();
        for step in &self.steps {
            table.insert(step.read().usid, step.clone());
        }
        table
    }

    /// Returns a mapping of all functions in this source file by their UFID.
    pub fn function_table(&self) -> HashMap<UFID, FunctionRef> {
        let mut table = HashMap::default();
        for function in &self.functions {
            table.insert(function.read().ufid, function.clone());
        }
        table
    }

    /// Returns a mapping of all contracts in this source file by their UCID.
    pub fn contract_table(&self) -> HashMap<UCID, ContractRef> {
        let mut table = HashMap::default();
        for contract in &self.contracts {
            table.insert(contract.read().ucid, contract.clone());
        }
        table
    }

    /// Returns a mapping of all user defined types in this source file by their UTID.
    pub fn user_defined_type_table(&self) -> HashMap<UTID, UserDefinedTypeRef> {
        let mut table = HashMap::default();
        for user_defined_type in &self.user_defined_types {
            table.insert(user_defined_type.read().utid, user_defined_type.clone());
        }
        table
    }

    /// Returns a mapping of all user defined types in this source file by their AST ID.
    ///
    /// # Returns
    ///
    /// A HashMap mapping AST IDs to their corresponding UserDefinedTypeRef instances.
    pub fn user_defined_types(&self) -> HashMap<usize, UserDefinedTypeRef> {
        let mut table = HashMap::default();
        for user_defined_type in &self.user_defined_types {
            table.insert(user_defined_type.ast_id(), user_defined_type.clone());
        }
        table
    }
}

/// Main analyzer for processing Solidity source code and extracting execution steps.
///
/// The Analyzer walks through the Abstract Syntax Tree (AST) of Solidity source code
/// and identifies executable steps, manages variable scopes, and tracks function
/// visibility and mutability requirements.
///
/// # Fields
///
/// - `scope_stack`: Stack of variable scopes for managing variable visibility
/// - `finished_steps`: Completed execution steps that have been fully analyzed
/// - `current_step`: Currently being analyzed step (if any)
/// - `private_state_variables`: State variables that should be made public
/// - `private_functions`: Functions that should be made public
/// - `immutable_functions`: Functions that should be made mutable
#[derive(Debug, Clone)]
pub struct Analyzer {
    source_id: u32,
    version_requirements: Vec<String>,

    scope_stack: Vec<VariableScopeRef>,

    finished_steps: Vec<StepRef>,
    current_step: Option<StepRef>,
    current_function: Option<FunctionRef>,
    current_contract: Option<ContractRef>,
    /// List of all contracts in this file.
    contracts: Vec<ContractRef>,
    /// List of all functions in this file.
    functions: Vec<FunctionRef>,
    /// A mapping from the `VariableDeclaration` AST node ID to the variable reference.
    variables: HashMap<usize, VariableRef>,
    /// List of all state variables in this file.
    state_variables: Vec<VariableRef>,
    /// State variables that should be made public
    private_state_variables: Vec<VariableRef>,
    /// Functions that should be made public
    private_functions: Vec<FunctionRef>,
    /// Functions that should be made mutable (i.e., neither pure nor view)
    immutable_functions: Vec<FunctionRef>,
    /// Function types defined in this file.
    function_types: Vec<FunctionTypeNameRef>,
    /// User defined types defined in this file.
    user_defined_types: Vec<UserDefinedTypeRef>,
}

impl Analyzer {
    /// Creates a new instance of the Analyzer.
    ///
    /// This method initializes a fresh analyzer with default state, ready to analyze
    /// Solidity source code.
    ///
    /// # Returns
    ///
    /// A new `Analyzer` instance with empty scope stack and step collections.
    pub fn new(source_id: u32) -> Self {
        Self {
            source_id,
            version_requirements: Vec::new(),
            scope_stack: Vec::new(),
            finished_steps: Vec::new(),
            current_step: None,
            current_function: None,
            current_contract: None,
            contracts: Vec::new(),
            functions: Vec::new(),
            variables: HashMap::default(),
            state_variables: Vec::new(),
            private_state_variables: Vec::new(),
            private_functions: Vec::new(),
            immutable_functions: Vec::new(),
            function_types: Vec::new(),
            user_defined_types: Vec::new(),
        }
    }

    /// Analyzes a source unit and returns the analysis results.
    ///
    /// This method walks through the AST of the source unit, identifies execution steps,
    /// manages variable scopes, and collects recommendations for code improvements.
    ///
    /// # Arguments
    ///
    /// * `source_id` - Unique identifier for the source file
    /// * `source_path` - File system path to the source file
    /// * `source_unit` - The source unit to analyze
    ///
    /// # Returns
    ///
    /// A `Result` containing the `SourceAnalysis` on success, or an `AnalysisError` on failure.
    ///
    /// # Errors
    ///
    /// Returns an error if the AST walk fails or if there are issues with step partitioning.
    pub fn analyze(
        mut self,
        source_id: u32,
        source_path: &PathBuf,
        source_unit: &SourceUnit,
    ) -> Result<SourceAnalysis, AnalysisError> {
        debug!(path=?source_path, "start walking the AST");
        source_unit.walk(&mut self).map_err(AnalysisError::Other)?;
        debug!(path=?source_path, "finished walking the AST");

        assert!(self.scope_stack.len() == 1, "scope stack should have exactly one scope");
        assert!(self.current_step.is_none(), "current step should be none");
        let global_scope = self.scope_stack.pop().expect("global scope should not be empty");
        let steps = self.finished_steps;
        let functions = self.functions;
        Ok(SourceAnalysis {
            id: source_id,
            path: source_path.clone(),
            unit: source_unit.clone(),
            global_scope,
            steps,
            contracts: self.contracts,
            private_state_variables: self.private_state_variables,
            state_variables: self.state_variables,
            functions,
            private_functions: self.private_functions,
            immutable_functions: self.immutable_functions,
            function_types: self.function_types,
            user_defined_types: self.user_defined_types,
        })
    }
}

/* Scope analysis utils */
impl Analyzer {
    fn current_scope(&self) -> VariableScopeRef {
        self.scope_stack.last().expect("scope stack is empty").clone()
    }

    fn enter_new_scope(&mut self, node: ScopeNode) -> eyre::Result<()> {
        let new_scope = VariableScope {
            node,
            declared_variables: Vec::default(),
            children: vec![],
            parent: self.scope_stack.last().cloned(),
        }
        .into();
        self.scope_stack.push(new_scope);
        Ok(())
    }

    fn declare_variable(&mut self, declaration: &VariableDeclaration) -> eyre::Result<()> {
        if declaration.name.is_empty() {
            // if a variable has no name, we skip the variable declaration
            return Ok(());
        }
        if declaration.mutability == Some(Mutability::Immutable)
            || declaration.mutability == Some(Mutability::Constant)
            || declaration.constant
        {
            // constant and immutable variables are excluded.
            return Ok(());
        }

        // collect function types from this variable declaration
        self.collect_function_types_from_variable(declaration)?;

        // add a new variable to the current scope
        let scope = self.current_scope();
        let function = self.current_function.clone();
        let contract = self.current_contract.clone();
        let uvid = UVID::next();
        let state_variable = declaration.state_variable;
        let variable: VariableRef = Variable::Plain {
            uvid,
            declaration: declaration.clone(),
            state_variable,
            function,
            contract,
        }
        .into();
        self.check_state_variable_visibility(&variable)?;
        if state_variable {
            self.state_variables.push(variable.clone());
        }
        scope.write().declared_variables.push(variable.clone());

        // add the variable to the variable_declarations map
        self.variables.insert(declaration.id, variable.clone());

        if let Some(step) = self.current_step.as_mut() {
            // add the variable to the current step
            step.write().declared_variables.push(variable.clone());
        }
        Ok(())
    }

    fn exit_current_scope(&mut self, src: SourceLocation) -> eyre::Result<()> {
        assert_eq!(
            self.current_scope().src(),
            src,
            "scope mismatch: the post-visit block's source location does not match the current scope's location"
        );
        // close the scope
        let closed_scope = self.scope_stack.pop().expect("scope stack is empty");
        if let Some(parent) = self.scope_stack.last_mut() {
            parent.write().children.push(closed_scope);
        }
        Ok(())
    }

    /// Collects function types from a variable declaration.
    ///
    /// This function recursively walks through the type structure of a variable declaration
    /// and collects any FunctionTypeName instances found within the type hierarchy.
    ///
    /// # Arguments
    /// * `declaration` - The variable declaration to analyze
    ///
    /// # Returns
    /// * `Result<(), eyre::Report>` - Ok if successful, Err if an error occurs during analysis
    fn collect_function_types_from_variable(
        &mut self,
        declaration: &VariableDeclaration,
    ) -> eyre::Result<()> {
        if let Some(type_name) = &declaration.type_name {
            self.collect_function_types_recursive(type_name);
        }
        Ok(())
    }

    /// Recursively collects function types from a TypeName.
    ///
    /// This function traverses the type hierarchy and adds any FunctionTypeName
    /// instances to the function_types collection.
    ///
    /// # Arguments
    /// * `type_name` - The type to analyze for function types
    fn collect_function_types_recursive(&mut self, type_name: &TypeName) {
        match type_name {
            TypeName::FunctionTypeName(function_type) => {
                // Found a function type - add it to our collection
                self.function_types.push((*function_type.clone()).into());
            }
            TypeName::ArrayTypeName(array_type) => {
                // Recursively check the array's base type
                self.collect_function_types_recursive(&array_type.base_type);
            }
            TypeName::Mapping(mapping) => {
                // Recursively check both key and value types
                self.collect_function_types_recursive(&mapping.key_type);
                self.collect_function_types_recursive(&mapping.value_type);
            }
            TypeName::ElementaryTypeName(_) | TypeName::UserDefinedTypeName(_) => {
                // These types don't contain function types, so nothing to do
            }
        }
    }

    fn check_state_variable_visibility(&mut self, variable: &VariableRef) -> eyre::Result<()> {
        let declaration = variable.declaration();
        if declaration.state_variable {
            // FIXME: this is a temporary workaround on user defined struct types.
            // Struct may not be able to be declared as a public state variable.
            // So here when we encounter a state variable with a user defined type, we skip the visibility check.
            // In the future, we may further consider to support user defined struct types as public state variables
            // under the condition that it does not contain inner recursive types (array or mapping fields).
            if declaration.type_name.as_ref().map(contains_user_defined_type).unwrap_or(false) {
                return Ok(());
            }

            // we need to change the visibility of the state variable to public
            if declaration.visibility != Visibility::Public {
                self.private_state_variables.push(variable.clone());
            }
        }
        Ok(())
    }
}

/* Contract analysis utils */
impl Analyzer {
    fn enter_new_contract(&mut self, contract: &ContractDefinition) -> eyre::Result<VisitorAction> {
        assert!(self.current_contract.is_none(), "Contract cannot be nested");
        let new_contract: ContractRef = Contract::new(contract.clone()).into();
        self.current_contract = Some(new_contract);
        Ok(VisitorAction::Continue)
    }

    fn exit_current_contract(&mut self) -> eyre::Result<()> {
        assert!(self.current_contract.is_some(), "current contract should be set");
        let contract = self.current_contract.take().unwrap();
        self.contracts.push(contract);
        Ok(())
    }
}

/* Function analysis utils */
impl Analyzer {
    fn current_function(&self) -> FunctionRef {
        self.current_function.as_ref().expect("current function should be set").clone()
    }

    fn enter_new_function(&mut self, function: &FunctionDefinition) -> eyre::Result<VisitorAction> {
        assert!(self.current_function.is_none(), "Function cannot be nested");
        let new_func: FunctionRef =
            Function::new_function(self.current_contract.clone(), function.clone()).into();
        self.check_function_visibility_and_mutability(&new_func)?;
        self.current_function = Some(new_func.clone());
        Ok(VisitorAction::Continue)
    }

    fn exit_current_function(&mut self) -> eyre::Result<()> {
        assert!(self.current_function.is_some(), "current function should be set");
        let function = self.current_function.take().unwrap();
        self.functions.push(function);
        Ok(())
    }

    fn enter_new_modifier(&mut self, modifier: &ModifierDefinition) -> eyre::Result<VisitorAction> {
        assert!(self.current_function.is_none(), "Function cannot be nested");
        let current_contract =
            self.current_contract.as_ref().expect("current contract should be set");
        let new_func: FunctionRef =
            Function::new_modifier(current_contract.clone(), modifier.clone()).into();
        self.current_function = Some(new_func);
        Ok(VisitorAction::Continue)
    }

    fn exit_current_modifier(&mut self) -> eyre::Result<()> {
        assert!(self.current_function.is_some(), "current function should be set");
        let function = self.current_function.take().unwrap();
        self.functions.push(function);
        Ok(())
    }

    fn check_function_visibility_and_mutability(&mut self, func: &FunctionRef) -> eyre::Result<()> {
        if func.visibility() != Visibility::Public && func.visibility() != Visibility::External {
            self.private_functions.push(func.clone());
        }

        if func
            .state_mutability()
            .as_ref()
            .is_some_and(|mu| *mu == StateMutability::View || *mu == StateMutability::Pure)
        {
            self.immutable_functions.push(func.clone());
        }
        Ok(())
    }
}

/* Step partition utils */
impl Analyzer {
    fn enter_new_statement_step(&mut self, statement: &Statement) -> eyre::Result<VisitorAction> {
        assert!(self.current_step.is_none(), "Step cannot be nested");
        let current_function = self.current_function();
        let current_scope = self.current_scope();

        macro_rules! step {
            ($variant:ident, $stmt:expr, $loc:expr) => {{
                let variables_in_scope = current_scope.read().variables_recursive();
                let new_step: StepRef = Step::new(
                    current_function.ufid(),
                    StepVariant::$variant($stmt),
                    $loc,
                    current_scope.clone(),
                    variables_in_scope.clone(),
                )
                .into();
                self.current_step = Some(new_step.clone());
                // add the step to the current function
                current_function.write().steps.push(new_step);
            }};
        }
        macro_rules! simple_stmt_to_step {
            ($stmt:expr) => {
                step!(Statement, statement.clone(), $stmt.src)
            };
        }
        match statement {
            Statement::Block(_) => {}
            Statement::Break(break_stmt) => simple_stmt_to_step!(break_stmt),
            Statement::Continue(continue_stmt) => simple_stmt_to_step!(continue_stmt),
            Statement::DoWhileStatement(do_while_statement) => {
                // the step is the `while(...)`
                let loc = sloc_rdiff(do_while_statement.src, do_while_statement.body.src);
                step!(DoWhileLoop, *do_while_statement.clone(), loc);

                // we take over the walk of the sub ast tree in the do-while statement step.
                let mut single_step_walker = AnalyzerSingleStepWalker { analyzer: self };
                do_while_statement.condition.walk(&mut single_step_walker)?;

                // end the do-while statement step early and then walk the body of the do-while statement.
                self.exit_current_statement_step(statement)?;
                do_while_statement.body.walk(self)?;

                // skip the subtree of the do-while statement since we have already walked it
                return Ok(VisitorAction::SkipSubtree);
            }
            Statement::EmitStatement(emit_statement) => simple_stmt_to_step!(emit_statement),
            Statement::ExpressionStatement(expr_stmt) => simple_stmt_to_step!(expr_stmt),
            Statement::ForStatement(for_statement) => {
                // the step is the `for(...)`
                let loc = sloc_ldiff(for_statement.src, block_or_stmt_src(&for_statement.body));
                step!(ForLoop, *for_statement.clone(), loc);

                // we take over the walk of the sub ast tree in the for statement step.
                let mut single_step_walker = AnalyzerSingleStepWalker { analyzer: self };
                if let Some(initialization_expression) = &for_statement.initialization_expression {
                    initialization_expression.walk(&mut single_step_walker)?;
                }
                if let Some(condition) = &for_statement.condition {
                    condition.walk(&mut single_step_walker)?;
                }
                if let Some(loop_expression) = &for_statement.loop_expression {
                    loop_expression.walk(&mut single_step_walker)?;
                }

                // end the for statement step early and then walk the body of the for statement.
                self.exit_current_statement_step(statement)?;
                for_statement.body.walk(self)?;

                // skip the subtree of the for statement since we have already walked it
                return Ok(VisitorAction::SkipSubtree);
            }
            Statement::IfStatement(if_statement) => {
                // the step is the `if(...)`
                let loc = sloc_ldiff(if_statement.src, block_or_stmt_src(&if_statement.true_body));
                step!(IfCondition, *if_statement.clone(), loc);

                // we take over the walk of the sub ast tree in the if statement step.
                let mut single_step_walker = AnalyzerSingleStepWalker { analyzer: self };
                if_statement.condition.walk(&mut single_step_walker)?;

                // end the if statement step early and then walk the true and false body of the if statement.
                self.exit_current_statement_step(statement)?;
                if_statement.true_body.walk(self)?;
                if let Some(false_body) = &if_statement.false_body {
                    false_body.walk(self)?;
                }

                // skip the subtree of the if statement since we have already walked it
                return Ok(VisitorAction::SkipSubtree);
            }
            Statement::InlineAssembly(inline_assembly) => simple_stmt_to_step!(inline_assembly),
            Statement::PlaceholderStatement(_) => {}
            Statement::Return(return_stmt) => simple_stmt_to_step!(return_stmt),
            Statement::RevertStatement(revert_statement) => simple_stmt_to_step!(revert_statement),
            Statement::TryStatement(try_statement) => {
                // the step is the `try`
                let first_clause = &try_statement.clauses[0];
                let loc = sloc_ldiff(try_statement.src, first_clause.block.src);
                step!(Try, *try_statement.clone(), loc);

                // we take over the walk of the sub ast tree in the try statement step.
                let mut single_step_walker = AnalyzerSingleStepWalker { analyzer: self };
                try_statement.external_call.walk(&mut single_step_walker)?;

                // end the try statement step early and then walk the clauses of the try statement.
                self.exit_current_statement_step(statement)?;
                for clause in &try_statement.clauses {
                    clause.block.walk(self)?;
                }

                // skip the subtree of the try statement since we have already walked it
                return Ok(VisitorAction::SkipSubtree);
            }
            Statement::UncheckedBlock(_) => { /* walk in the block */ }
            Statement::VariableDeclarationStatement(variable_declaration_statement) => {
                simple_stmt_to_step!(variable_declaration_statement)
            }
            Statement::WhileStatement(while_statement) => {
                // the step is the `while(...)`
                let loc = sloc_ldiff(while_statement.src, block_or_stmt_src(&while_statement.body));
                step!(WhileLoop, *while_statement.clone(), loc);

                // we take over the walk of the sub ast tree in the while statement step.
                let mut single_step_walker = AnalyzerSingleStepWalker { analyzer: self };
                while_statement.condition.walk(&mut single_step_walker)?;

                // end the while statement step early and then walk the body of the while statement.
                self.exit_current_statement_step(statement)?;
                while_statement.body.walk(self)?;

                // skip the subtree of the while statement since we have already walked it
                return Ok(VisitorAction::SkipSubtree);
            }
        };
        Ok(VisitorAction::Continue)
    }

    fn enter_new_function_step(
        &mut self,
        function: &FunctionDefinition,
    ) -> eyre::Result<VisitorAction> {
        assert!(self.current_step.is_none(), "Step cannot be nested");
        let current_function = self.current_function();

        if function.body.is_none() {
            // if a function has no body, we skip the function step
            return Ok(VisitorAction::SkipSubtree);
        }

        // step is the function header
        let current_scope = self.current_scope();
        let accessible_variables = current_scope.read().variables_recursive();
        let loc = sloc_ldiff(function.src, function.body.as_ref().unwrap().src);
        let new_step: StepRef = Step::new(
            current_function.ufid(),
            StepVariant::FunctionEntry(function.clone()),
            loc,
            current_scope,
            accessible_variables,
        )
        .into();
        self.current_step = Some(new_step.clone());
        current_function.write().steps.push(new_step);

        // we take over the walk of the sub ast tree in the function step.
        let mut single_step_walker = AnalyzerSingleStepWalker { analyzer: self };
        function.parameters.walk(&mut single_step_walker)?;
        function.return_parameters.walk(&mut single_step_walker)?;

        // end the function step early and then walk the body of the function.
        let step = self.current_step.take().unwrap();
        self.finished_steps.push(step);
        if let Some(body) = &function.body {
            body.walk(self)?;
        }

        // skip the subtree of the function since we have already walked it
        Ok(VisitorAction::SkipSubtree)
    }

    fn enter_new_modifier_step(
        &mut self,
        modifier: &ModifierDefinition,
    ) -> eyre::Result<VisitorAction> {
        assert!(self.current_step.is_none(), "Step cannot be nested");
        let current_function = self.current_function();

        if modifier.body.is_none() {
            // if a modifier has no body, we skip the modifier step
            return Ok(VisitorAction::SkipSubtree);
        }

        // step is the modifier header
        let current_scope = self.current_scope();
        let accessible_variables = current_scope.read().variables_recursive();
        let loc = sloc_ldiff(modifier.src, modifier.body.as_ref().unwrap().src);
        let new_step: StepRef = Step::new(
            current_function.ufid(),
            StepVariant::ModifierEntry(modifier.clone()),
            loc,
            current_scope,
            accessible_variables,
        )
        .into();
        self.current_step = Some(new_step.clone());
        current_function.write().steps.push(new_step);

        // we take over the walk of the sub ast tree in the modifier step.
        let mut single_step_walker = AnalyzerSingleStepWalker { analyzer: self };
        modifier.parameters.walk(&mut single_step_walker)?;

        // end the modifier step early and then walk the body of the modifier.
        let step = self.current_step.take().unwrap();
        self.finished_steps.push(step);
        if let Some(body) = &modifier.body {
            body.walk(self)?;
        }

        // skip the subtree of the modifier since we have already walked it
        Ok(VisitorAction::SkipSubtree)
    }

    /// Add a function call to the current step, if we are in a step.
    fn add_function_call(&mut self, call: &FunctionCall) -> eyre::Result<()> {
        if let Some(step) = self.current_step.as_mut() {
            if call.kind == FunctionCallKind::FunctionCall {
                step.write().function_calls.push(call.clone());
            }
        }
        Ok(())
    }

    fn exit_current_statement_step(&mut self, statement: &Statement) -> eyre::Result<()> {
        if self.current_step.is_none() {
            return Ok(());
        }

        match statement {
            Statement::Block(_)
            | Statement::PlaceholderStatement(_)
            | Statement::UncheckedBlock(_) => {}
            _ => {
                let step = self.current_step.take().unwrap();
                self.finished_steps.push(step);
            }
        }
        Ok(())
    }
}

/* Variable update analysis */
impl Analyzer {
    fn record_assignment(&mut self, variable: &Assignment) -> eyre::Result<VisitorAction> {
        fn get_varaiable(this: &Analyzer, expr: &Expression) -> Option<VariableRef> {
            match expr {
                Expression::Identifier(identifier) => {
                    if let Some(declaration_id) = &identifier.referenced_declaration {
                        if declaration_id >= &0 {
                            if let Some(variable) = this.variables.get(&(*declaration_id as usize))
                            {
                                return Some(variable.clone());
                            }
                        }
                    }
                    None
                }
                Expression::IndexAccess(index_access) => {
                    if let Some(base_variable) = get_varaiable(this, &index_access.base_expression)
                    {
                        if let Some(index) = &index_access.index_expression {
                            let var = Variable::Index { base: base_variable, index: index.clone() };
                            return Some(var.into());
                        }
                    }
                    None
                }
                Expression::IndexRangeAccess(index_range_access) => {
                    if let Some(base_variable) =
                        get_varaiable(this, &index_range_access.base_expression)
                    {
                        let var = Variable::IndexRange {
                            base: base_variable,
                            start: index_range_access.start_expression.clone(),
                            end: index_range_access.end_expression.clone(),
                        };
                        return Some(var.into());
                    }
                    None
                }
                Expression::MemberAccess(member_access) => {
                    if let Some(base_variable) = get_varaiable(this, &member_access.expression) {
                        let var = Variable::Member {
                            base: base_variable,
                            member: member_access.member_name.clone(),
                        };
                        return Some(var.into());
                    }
                    None
                }
                Expression::TupleExpression(_) => unreachable!(),
                _ => None,
            }
        }

        let updated_variables: Vec<VariableRef> = match &variable.lhs {
            Expression::Identifier(_)
            | Expression::IndexAccess(_)
            | Expression::IndexRangeAccess(_)
            | Expression::MemberAccess(_) => {
                if let Some(var) = get_varaiable(self, &variable.lhs) {
                    vec![var]
                } else {
                    vec![]
                }
            }
            Expression::TupleExpression(tuple_expression) => {
                let mut vars = vec![];
                for comp in tuple_expression.components.iter().flatten() {
                    if let Some(var) = get_varaiable(self, comp) {
                        vars.push(var);
                    }
                }
                vars
            }
            _ => vec![],
        };

        if let Some(step) = self.current_step.as_mut() {
            step.write().updated_variables.extend(updated_variables);
        }
        Ok(VisitorAction::Continue)
    }

    /// Record a declared variable's initial value to the current step's updated variables.
    fn record_declared_varaible(&mut self, declaration: &VariableDeclaration) -> eyre::Result<()> {
        let Some(step) = self.current_step.as_mut() else {
            return Ok(());
        };
        if declaration.name.is_empty() {
            // if the variable has no name, we skip the variable declaration
            return Ok(());
        }
        let variable: VariableRef = self.variables.get(&declaration.id).unwrap().clone();
        step.write().updated_variables.push(variable);
        Ok(())
    }
}

/* User defined type analysis */
impl Analyzer {
    fn record_user_defined_value_type(
        &mut self,
        type_definition: &UserDefinedValueTypeDefinition,
    ) -> eyre::Result<()> {
        let user_defined_type = UserDefinedType::new(
            self.source_id,
            UserDefinedTypeVariant::UserDefinedValueType(type_definition.clone()),
        );
        self.user_defined_types.push(user_defined_type.into());
        Ok(())
    }

    fn record_struct_type(&mut self, struct_definition: &StructDefinition) -> eyre::Result<()> {
        let user_defined_type = UserDefinedType::new(
            self.source_id,
            UserDefinedTypeVariant::Struct(struct_definition.clone()),
        );
        self.user_defined_types.push(user_defined_type.into());
        Ok(())
    }

    fn record_enum_type(&mut self, enum_definition: &EnumDefinition) -> eyre::Result<()> {
        let user_defined_type = UserDefinedType::new(
            self.source_id,
            UserDefinedTypeVariant::Enum(enum_definition.clone()),
        );
        self.user_defined_types.push(user_defined_type.into());
        Ok(())
    }

    fn record_contract_type(
        &mut self,
        contract_definition: &ContractDefinition,
    ) -> eyre::Result<()> {
        let user_defined_type = UserDefinedType::new(
            self.source_id,
            UserDefinedTypeVariant::Contract(contract_definition.clone()),
        );
        self.user_defined_types.push(user_defined_type.into());
        Ok(())
    }
}

impl Visitor for Analyzer {
    fn visit_source_unit(&mut self, source_unit: &SourceUnit) -> eyre::Result<VisitorAction> {
        // enter a global scope
        self.enter_new_scope(ScopeNode::SourceUnit(source_unit.clone()))?;
        Ok(VisitorAction::Continue)
    }

    fn post_visit_source_unit(&mut self, _source_unit: &SourceUnit) -> eyre::Result<()> {
        assert_eq!(
            self.scope_stack.len(),
            1,
            "Scope stack should only have one scope (the global scope)"
        );
        assert!(self.current_step.is_none(), "Step should be finished");
        Ok(())
    }

    fn visit_pragma_directive(
        &mut self,
        directive: &PragmaDirective,
    ) -> eyre::Result<VisitorAction> {
        let literals = &directive.literals;
        if literals.len() > 1 && literals[0].trim() == "solidity" {
            let mut version_str = vec![];
            let mut current_req = String::new();
            let mut i = 1;
            while i < literals.len() {
                let literal = &literals[i];
                if literal.starts_with('.') {
                    current_req.push_str(literal);
                } else if ["=", "<", ">", "~", "^"].iter().any(|p| literal.starts_with(p)) {
                    version_str.push(current_req);
                    current_req = literal.clone();
                    i += 1;
                    current_req.push_str(&literals[i]);
                } else {
                    version_str.push(current_req);
                    current_req = literal.clone();
                }
                i += 1;
            }
            version_str.push(current_req);

            let version_str =
                version_str.into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(",");
            // one source file may have multiple `pragma solidity` directives, we collect all of them
            self.version_requirements.push(version_str);
        }
        Ok(VisitorAction::Continue)
    }

    fn visit_contract_definition(
        &mut self,
        _definition: &ContractDefinition,
    ) -> eyre::Result<VisitorAction> {
        // record the contract type
        self.record_contract_type(_definition)?;

        // enter a new contract
        self.enter_new_contract(_definition)?;

        // enter a contract scope
        self.enter_new_scope(ScopeNode::ContractDefinition(_definition.clone()))?;
        Ok(VisitorAction::Continue)
    }

    fn post_visit_contract_definition(
        &mut self,
        _definition: &ContractDefinition,
    ) -> eyre::Result<()> {
        // exit the contract scope
        self.exit_current_scope(_definition.src)?;

        // exit the contract
        self.exit_current_contract()?;
        Ok(())
    }

    fn visit_user_defined_value_type(
        &mut self,
        _value_type: &UserDefinedValueTypeDefinition,
    ) -> eyre::Result<VisitorAction> {
        self.record_user_defined_value_type(_value_type)?;
        Ok(VisitorAction::Continue)
    }

    fn visit_struct_definition(
        &mut self,
        _definition: &StructDefinition,
    ) -> eyre::Result<VisitorAction> {
        self.record_struct_type(_definition)?;
        Ok(VisitorAction::Continue)
    }

    fn visit_enum_definition(
        &mut self,
        _definition: &EnumDefinition,
    ) -> eyre::Result<VisitorAction> {
        self.record_enum_type(_definition)?;
        Ok(VisitorAction::Continue)
    }

    fn visit_event_definition(
        &mut self,
        _definition: &EventDefinition,
    ) -> eyre::Result<VisitorAction> {
        Ok(VisitorAction::SkipSubtree)
    }

    fn visit_error_definition(
        &mut self,
        _definition: &ErrorDefinition,
    ) -> eyre::Result<VisitorAction> {
        Ok(VisitorAction::SkipSubtree)
    }

    fn visit_function_definition(
        &mut self,
        definition: &FunctionDefinition,
    ) -> eyre::Result<VisitorAction> {
        // enter a new function
        self.enter_new_function(definition)?;

        // enter a variable scope for the function
        self.enter_new_scope(ScopeNode::FunctionDefinition(definition.clone()))?;

        // enter a function step
        self.enter_new_function_step(definition)
    }

    fn post_visit_function_definition(
        &mut self,
        definition: &FunctionDefinition,
    ) -> eyre::Result<()> {
        // exit the function scope
        self.exit_current_scope(definition.src)?;

        // exit the function
        self.exit_current_function()?;
        Ok(())
    }

    fn visit_modifier_definition(
        &mut self,
        definition: &ModifierDefinition,
    ) -> eyre::Result<VisitorAction> {
        // enter a new modifier
        self.enter_new_modifier(definition)?;

        // enter a variable scope for the modifier
        self.enter_new_scope(ScopeNode::ModifierDefinition(definition.clone()))?;

        // enter a modifier step
        self.enter_new_modifier_step(definition)
    }

    fn post_visit_modifier_definition(
        &mut self,
        definition: &ModifierDefinition,
    ) -> eyre::Result<()> {
        // exit the modifier scope
        self.exit_current_scope(definition.src)?;

        // exit the modifier
        self.exit_current_modifier()?;
        Ok(())
    }

    fn visit_block(&mut self, block: &Block) -> eyre::Result<VisitorAction> {
        // enter a block scope
        self.enter_new_scope(ScopeNode::Block(block.clone()))?;
        Ok(VisitorAction::Continue)
    }

    fn post_visit_block(&mut self, block: &Block) -> eyre::Result<()> {
        // exit the block scope
        self.exit_current_scope(block.src)?;
        Ok(())
    }

    fn visit_unchecked_block(
        &mut self,
        unchecked_block: &UncheckedBlock,
    ) -> eyre::Result<VisitorAction> {
        // enter an unchecked block scope
        self.enter_new_scope(ScopeNode::UncheckedBlock(unchecked_block.clone()))?;
        Ok(VisitorAction::Continue)
    }

    fn post_visit_unchecked_block(&mut self, unchecked_block: &UncheckedBlock) -> eyre::Result<()> {
        // exit the unchecked block scope
        self.exit_current_scope(unchecked_block.src)?;
        Ok(())
    }

    fn visit_for_statement(&mut self, for_statement: &ForStatement) -> eyre::Result<VisitorAction> {
        // enter a for statement scope
        self.enter_new_scope(ScopeNode::ForStatement(for_statement.clone()))?;
        Ok(VisitorAction::Continue)
    }

    fn post_visit_for_statement(&mut self, for_statement: &ForStatement) -> eyre::Result<()> {
        // exit the for statement scope
        self.exit_current_scope(for_statement.src)?;
        Ok(())
    }

    fn visit_statement(&mut self, _statement: &Statement) -> eyre::Result<VisitorAction> {
        // try to enter a new step
        self.enter_new_statement_step(_statement)
    }

    fn post_visit_statement(&mut self, _statement: &Statement) -> eyre::Result<()> {
        // exit the current step
        self.exit_current_statement_step(_statement)?;
        Ok(())
    }

    fn visit_function_call(&mut self, function_call: &FunctionCall) -> eyre::Result<VisitorAction> {
        self.add_function_call(function_call)?;
        Ok(VisitorAction::Continue)
    }

    fn visit_variable_declaration(
        &mut self,
        declaration: &VariableDeclaration,
    ) -> eyre::Result<VisitorAction> {
        // declare a variable
        self.declare_variable(declaration)?;
        // record the declared variable
        self.record_declared_varaible(declaration)?;
        Ok(VisitorAction::Continue)
    }

    fn visit_assignment(&mut self, assignment: &Assignment) -> eyre::Result<VisitorAction> {
        // record updated variables
        self.record_assignment(assignment)
    }
}

/// A walker wrapping [`Analyzer`] that only walks a single step.
#[derive(derive_more::Deref, derive_more::DerefMut)]
struct AnalyzerSingleStepWalker<'a> {
    #[deref]
    #[deref_mut]
    analyzer: &'a mut Analyzer,
}

impl<'a> Visitor for AnalyzerSingleStepWalker<'a> {
    fn visit_function_call(&mut self, function_call: &FunctionCall) -> eyre::Result<VisitorAction> {
        self.analyzer.add_function_call(function_call)?;
        Ok(VisitorAction::Continue)
    }

    fn visit_variable_declaration(
        &mut self,
        declaration: &VariableDeclaration,
    ) -> eyre::Result<VisitorAction> {
        self.analyzer.declare_variable(declaration)?;
        self.analyzer.record_declared_varaible(declaration)?;
        Ok(VisitorAction::Continue)
    }
}

/// Errors that can occur during source code analysis.
///
/// This enum represents all possible error conditions that can arise during
/// the analysis process, from compilation failures to step partitioning errors.
#[derive(Debug, Error)]
pub enum AnalysisError {
    /// AST data is not available in the compiled artifact
    #[error("AST is not selected as compiler output")]
    MissingAst,

    /// Error during AST conversion
    #[error("failed to convert AST: {0}")]
    ASTConversionError(eyre::Report),

    /// Error during step partitioning of source code
    #[error("failed to partition source steps: {0}")]
    StepPartitionError(eyre::Report),

    /// Other analysis-related errors
    #[error("other error: {0}")]
    Other(eyre::Report),
}

#[cfg(test)]
pub(crate) mod tests {
    use std::collections::BTreeMap;

    use foundry_compilers::{
        artifacts::{Severity, Source, Sources},
        solc::{SolcCompiler, SolcLanguage, SolcSettings, SolcVersionedInput},
        CompilationError, Compiler, CompilerInput,
    };
    use semver::Version;

    use crate::{
        compile_contract_source_to_source_unit, source_string_at_location_unchecked, ASTPruner,
    };

    use super::*;

    pub(crate) const TEST_CONTRACT_SOURCE_PATH: &str = "test.sol";
    pub(crate) const TEST_CONTRACT_SOURCE_ID: u32 = 0;

    /// Utility function to compile Solidity source code and analyze it
    ///
    /// This function encapsulates the common pattern used across all tests:
    /// 1. Compile the source code to get the AST
    /// 2. Create an analyzer and analyze the contract
    /// 3. Return the analysis result
    ///
    /// # Arguments
    /// * `source` - The Solidity source code as a string
    ///
    /// # Returns
    /// * `SourceAnalysis` - The analysis result containing steps, scopes, and recommendations
    pub(crate) fn compile_and_analyze(source: &str) -> (BTreeMap<u32, Source>, SourceAnalysis) {
        // Compile the source code to get the AST
        let version = Version::parse("0.8.20").unwrap();
        let result = compile_contract_source_to_source_unit(version, source, false);
        assert!(result.is_ok(), "Source compilation should succeed: {}", result.unwrap_err());

        let source_unit = result.unwrap();
        let sources = BTreeMap::from([(TEST_CONTRACT_SOURCE_ID, Source::new(source))]);

        // Create an analyzer and analyze the contract
        let analyzer = Analyzer::new(TEST_CONTRACT_SOURCE_ID);
        let analysis = analyzer
            .analyze(
                TEST_CONTRACT_SOURCE_ID,
                &PathBuf::from(TEST_CONTRACT_SOURCE_PATH),
                &source_unit,
            )
            .unwrap();

        (sources, analysis)
    }

    macro_rules! count_step_by_variant {
        ($analysis:expr, $variant:ident()) => {
            $analysis
                .steps
                .iter()
                .filter(|s| matches!(s.read().variant, StepVariant::$variant(_)))
                .count()
        };

        ($analysis:expr, $variant:ident{}) => {
            $analysis
                .steps
                .iter()
                .filter(|s| matches!(s.read().variant, StepVariant::$variant { .. }))
                .count()
        };
    }

    macro_rules! count_updated_variables {
        ($analysis:expr) => {
            $analysis.steps.iter().map(|s| s.read().updated_variables.len()).sum::<usize>()
        };
    }

    #[test]
    fn test_function_step() {
        // Create a simple contract with a function to test function step extraction
        let source = r#"
abstract contract TestContract {
    function setValue(uint256 newValue) public {}

    function getValue() public view returns (uint256) {
        return 0;
    }

    function getBalance() public view returns (uint256 balance) {}

    function template() public virtual returns (uint256);
}
"#;

        // Use utility function to compile and analyze
        let (_sources, analysis) = compile_and_analyze(source);

        // Assert all non-empty functions are present as steps
        assert!(count_step_by_variant!(analysis, FunctionEntry()) == 3);
    }

    #[test]
    fn test_statement_step() {
        // Create a simple contract with a function to test statement step extraction
        let source = r#"
contract TestContract {
    function getValue() public view returns (uint256) {
        uint256 value = 0;
        return 0;
    }
}
"#;

        // Use utility function to compile and analyze
        let (_sources, analysis) = compile_and_analyze(source);

        // Assert that we have two statement steps
        assert!(count_step_by_variant!(analysis, Statement()) == 2);
    }

    #[test]
    fn test_if_step() {
        // Create a simple contract with a function to test if statement extraction
        let source = r#"
contract TestContract {
    function getValue() public view returns (uint256) {
        if (true) {
            return 0;
        } else {
            return 1;
        }
    }
}
"#;

        // Use utility function to compile and analyze
        let (_sources, analysis) = compile_and_analyze(source);

        // Assert that we have one if step, and two statement steps
        assert!(count_step_by_variant!(analysis, IfCondition()) == 1);
        assert!(count_step_by_variant!(analysis, Statement()) == 2);
    }

    #[test]
    fn test_for_step() {
        // Create a simple contract with a function to test for statement extraction
        let source = r#"
contract TestContract {
    function getValue() public view returns (uint256) {
        for (uint256 i = 0; i < 10; i++) {
            return 0;
        }
    }
}
"#;

        // Use utility function to compile and analyze
        let (_sources, analysis) = compile_and_analyze(source);

        // Assert that we have one for step, and one statement step
        assert!(count_step_by_variant!(analysis, ForLoop {}) == 1);
        assert!(count_step_by_variant!(analysis, Statement()) == 1);
    }

    #[test]
    fn test_while_step() {
        // Create a simple contract with a function to test while statement extraction
        let source = r#"
contract TestContract {
    function getValue() public view returns (uint256) {
        while (true) {
            return 0;

        }
    }
}
"#;

        // Use utility function to compile and analyze
        let (_sources, analysis) = compile_and_analyze(source);

        // Assert that we have one while step, and one statement step
        assert!(count_step_by_variant!(analysis, WhileLoop()) == 1);
        assert!(count_step_by_variant!(analysis, Statement()) == 1);
    }

    #[test]
    fn test_try_step() {
        // Create a simple contract with a function to test try statement extraction
        let source = r#"
contract TestContract {
    function getValue() public view returns (uint256) {
        try this.getValue() {
            revert();
        } catch {
            return 1;
        }
    }
}
"#;

        // Use utility function to compile and analyze
        let (_sources, analysis) = compile_and_analyze(source);

        // Assert that we have one try step, and two statement steps
        assert!(count_step_by_variant!(analysis, Try()) == 1);
        assert!(count_step_by_variant!(analysis, Statement()) == 2);
    }

    #[test]
    fn test_if_statement_body() {
        // Create a simple contract with a function to test if statement extraction
        let source = r#"
contract TestContract {
    function getValue() public view returns (uint256) {
        if (true) revert();
        return 0;
    }
}
"#;

        // Use utility function to compile and analyze
        let (_sources, analysis) = compile_and_analyze(source);

        // Assert that we have one if step, and one statement step
        assert!(count_step_by_variant!(analysis, IfCondition()) == 1);
        assert!(count_step_by_variant!(analysis, Statement()) == 2);
    }

    #[test]
    fn test_type_conversion_is_not_function_call() {
        let source = r#"
interface ITestContract {
    function getValue() external view returns (uint256);
}

contract TestContract {
    struct S {
        uint256 value;
    }
    function getValue() public view returns (uint256) {
        ITestContract I = ITestContract(msg.sender);
        S memory s = S({ value: 1 });
        getValue();
        this.getValue();
        return uint256(1);
    }
}
"#;

        // Use utility function to compile and analyze
        let (_sources, analysis) = compile_and_analyze(source);

        // Assert that we have one function call step
        let mut function_calls = 0;
        analysis.steps.iter().for_each(|step| {
            function_calls += step.read().function_calls.len();
        });
        assert_eq!(function_calls, 2);
    }

    #[test]
    fn test_steps_in_modifier() {
        let source = r#"
contract TestContract {
    modifier test() {
        uint x = 1;
        _;
        uint y = 2;
    }
}
"#;
        let (_sources, analysis) = compile_and_analyze(source);

        // Assert that we have one modifier step, and two statement steps
        assert!(count_step_by_variant!(analysis, ModifierEntry()) == 1);
        assert!(count_step_by_variant!(analysis, Statement()) == 2);
    }

    #[test]
    fn test_type_casting_multi_files() {
        let interface_file0 = "interface.sol";
        let source0 = r#"
interface ITestContract {
    function getValue() external view returns (uint256);
}
"#;
        let source1 = r#"
import { ITestContract } from "interface.sol";

contract TestContract {
    function foo() public {
        ITestContract I = ITestContract(msg.sender);
    }
}
"#;
        let version = Version::parse("0.8.19").unwrap();
        let sources = Sources::from_iter([
            (PathBuf::from(interface_file0), Source::new(source0)),
            (PathBuf::from(TEST_CONTRACT_SOURCE_PATH), Source::new(source1)),
        ]);
        let settings = SolcSettings::default();
        let solc_input =
            SolcVersionedInput::build(sources, settings, SolcLanguage::Solidity, version);
        let compiler = SolcCompiler::AutoDetect;
        let output = compiler.compile(&solc_input).unwrap();

        // return error if compiler error
        let errors = output
            .errors
            .iter()
            .filter(|e| e.severity() == Severity::Error)
            .map(|e| format!("{e}"))
            .collect::<Vec<_>>();
        if !errors.is_empty() {
            panic!("Compiler error: {}", errors.join("\n"));
        }

        let mut ast = output
            .sources
            .get(&PathBuf::from(TEST_CONTRACT_SOURCE_PATH))
            .unwrap()
            .ast
            .clone()
            .unwrap();
        let source_unit = ASTPruner::convert(&mut ast, false).unwrap();

        let analyzer = Analyzer::new(TEST_CONTRACT_SOURCE_ID);
        let analysis = analyzer
            .analyze(
                TEST_CONTRACT_SOURCE_ID,
                &PathBuf::from(TEST_CONTRACT_SOURCE_PATH),
                &source_unit,
            )
            .unwrap();

        let mut function_calls = 0;
        for step in analysis.steps {
            function_calls += step.read().function_calls.len();
        }
        assert_eq!(function_calls, 0);
    }

    #[test]
    fn test_statement_semicolon() {
        let source = r#"
contract TestContract {
    function foo() public returns (uint) {
        require(false, "error");
        revert();
        uint x = 1;
        x = 2;
        x + 1;
        return 1;
    }
}

"#;
        let (_sources, analysis) = compile_and_analyze(source);
        let source = _sources.get(&TEST_CONTRACT_SOURCE_ID).unwrap().content.as_str();
        for step in &analysis.steps {
            let s = source_string_at_location_unchecked(source, &step.read().src);
            println!("step: {s}");
        }
    }

    #[test]
    fn test_function_type_collection() {
        let source = r#"
        contract TestContract {
            // Function type as state variable
            function(uint256) returns (bool) private callback;

            // Function type in array
            function(uint256) external pure returns (bool)[] private validators;

            // Function type in mapping
            mapping(address => function(uint256) returns (bool)) private userCallbacks;

            // Function with function type parameters (internal function)
            function setCallback(function(uint256) returns (bool) _callback) internal {
                callback = _callback;
            }

            // Modifier with function type parameter
            modifier onlyValidated(function(uint256) returns (bool) _validator) {
                require(_validator(123), "Not validated");
                _;
            }
        }
        "#;

        let (_sources, analysis) = compile_and_analyze(source);

        // Should collect function types from:
        // 1. State variable 'callback'
        // 2. Array element type in 'validators'
        // 3. Mapping value type in 'userCallbacks'
        // 4. Function parameter in 'setCallback'
        // 5. Modifier parameter in 'onlyValidated'
        // Total: at least 5 function types
        assert!(
            analysis.function_types.len() >= 5,
            "Expected at least 5 function types, found: {}",
            analysis.function_types.len()
        );

        println!("Collected {} function types", analysis.function_types.len());

        // Print each function type for inspection
        for (i, func_type) in analysis.function_types.iter().enumerate() {
            println!(
                "Function type {}: visibility={:?}, stateMutability={:?}",
                i + 1,
                func_type.visibility(),
                func_type.state_mutability()
            );
        }
    }

    #[test]
    fn test_variable_assignment() {
        let source = r#"
        contract TestContract {
            struct S {
                uint256 a;
                uint[] b;
                mapping(address => uint256) c;
            }
            S[] internal s;
            function foo(bool b) public {
                uint256 x = 1;
                x = 2;

                s[x].c[msg.sender] = 3;
                s[x].b[0] = 4;
                s[x].a = x;
            }
        }
        "#;
        let (_sources, analysis) = compile_and_analyze(source);
        assert_eq!(count_updated_variables!(analysis), 4);
    }

    #[test]
    fn test_variable_declaration_is_updated() {
        let source = r#"
        contract TestContract {
            function foo() public {
                uint256 x = 1;
            }
        }
        "#;
        let (_sources, analysis) = compile_and_analyze(source);
        assert_eq!(count_updated_variables!(analysis), 1);
    }

    #[test]
    fn test_variable_accessible() {
        let source = r#"
        contract TestContract {
            uint256[] internal s;
            function foo(bool b) public {
                uint256 x = 1;
                x = 2;

                if (b) {
                    uint y = x;
                    x = 3;
                }

                uint z = s[x];
            }
        }
        "#;
        let (_sources, analysis) = compile_and_analyze(source);

        // no step should have `z` in its accessible variables
        for step in &analysis.steps {
            assert!(!step
                .read()
                .accessible_variables
                .iter()
                .any(|v| v.read().declaration().name == "z"));
        }
    }

    #[test]
    fn test_contract_collection() {
        let source = r#"
        function foo() {
        }
        contract TestContract {
            function bar() public {
            }
        }
        "#;
        let (_sources, analysis) = compile_and_analyze(source);

        let foo_func = analysis
            .functions
            .iter()
            .find(|c| c.read().definition.name() == "foo")
            .expect("foo function should be found");
        let bar_func = analysis
            .functions
            .iter()
            .find(|c| c.read().definition.name() == "bar")
            .expect("bar function should be found");
        assert!(foo_func.read().contract.is_none());
        assert!(bar_func.read().contract.as_ref().is_some_and(|c| c.name() == "TestContract"));
    }

    #[test]
    fn test_dyn_abi_using_variable_declaration() {
        let source = r#"
        contract C {
            function f() public {
                address[] memory b = new address[](0);
            }
        }
        "#;
        let (_sources, analysis) = compile_and_analyze(source);
        let variables = analysis
            .variable_table()
            .values()
            .map(|v| v.base().declaration().clone())
            .collect::<Vec<_>>();
        let variable = variables.first().unwrap();
        assert_eq!(variable.type_descriptions.type_string.as_ref().unwrap(), "address[]");
    }
}
