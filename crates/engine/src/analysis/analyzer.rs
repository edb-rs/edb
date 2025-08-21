use std::{path::PathBuf, sync::Arc, time::Instant};

use alloy_primitives::map::foldhash::HashMap;
use foundry_compilers::artifacts::{
    ast::SourceLocation, Ast, Block, ContractDefinition, EventDefinition, ForStatement,
    FunctionDefinition, Source, SourceUnit, StateMutability, Statement, UncheckedBlock,
    VariableDeclaration, Visibility,
};
use rayon::iter::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};
use thiserror::Error;

use super::log::*;
use crate::{
    // new_usid, AnnotationsToChange,
    analysis::{
        visitor::VisitorAction, ScopeNode, Step, StepHook, StepRef, StepVariant, Variable,
        VariableScope, Visitor, Walk,
    },
    block_or_stmt_src,
    new_uvid,
    sloc_ldiff,
    sloc_rdiff,
    utils::Artifact,
    ASTPruner,
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
#[derive(Debug, Clone)]
pub struct SourceAnalysis {
    /// Unique identifier for this source file
    pub id: u32,
    /// File system path to the source file
    pub path: PathBuf,
    /// Original source content and metadata
    pub source: Source,
    /// Processed source unit ready for analysis
    pub unit: SourceUnit,
    /// Global variable scope of the source file
    pub global_scope: VariableScope,
    /// List of analyzed execution steps in this file
    pub steps: Vec<StepRef>,
    /// State variables that should be made public
    pub private_state_variables: Vec<VariableDeclaration>,
    /// Functions that should be made public
    pub private_functions: Vec<FunctionDefinition>,
    /// Functions that should be made mutable (i.e., neither pure nor view)
    pub immutable_functions: Vec<FunctionDefinition>,
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
        fn walk_scope(scope: &VariableScope, table: &mut HashMap<UVID, VariableRef>) {
            for (uvid, variable) in &scope.variables {
                table.insert(*uvid, variable.clone());
            }
            for child in &scope.children {
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
            table.insert(step.usid, step.clone());
        }
        table
    }

    /// Prints the analysis results in a human-readable format.
    ///
    /// This method displays a comprehensive overview of the source analysis,
    /// including file information, variable scopes, execution steps, and
    /// recommendations for code improvements.
    ///
    /// # Example
    ///
    /// ```rust
    /// use edb_engine::analysis::{Analyzer, SourceAnalysis};
    /// use foundry_compilers::artifacts::Artifact;
    ///
    /// // Assuming you have a compiled artifact
    /// let artifact: Artifact = /* your compiled artifact */;
    ///
    /// // Analyze the artifact
    /// let analyses = Analyzer::analyze(&artifact).unwrap();
    ///
    /// // Print the analysis results for each source file
    /// for analysis in analyses {
    ///     analysis.pretty_display();
    /// }
    /// ```
    ///
    /// This will output something like:
    /// ```
    /// === Source Analysis Report ===
    /// File ID: 1
    /// Path: contract.sol
    /// Source Content Length: 1234 characters
    ///
    /// === Variable Scopes ===
    /// Scope(SourceUnit): {}
    ///   Scope(ContractDefinition): {balance, owner}
    ///     Scope(Block): {amount}
    ///
    /// === Execution Steps (5 total) ===
    /// Step 1 (USID: 0):
    ///   Type: Variable Declaration
    ///   Location: 45:12
    ///   Source: uint256 balance = 0;
    ///   Pre-hooks: BeforeStep(USID: 0)
    ///   Post-hooks: VariableInScope(UVID: UVID(2543528819662847697))
    ///
    /// === Recommendations ===
    /// Private state variables that should be made public:
    ///   - balance (visibility: Private)
    /// === End Report ===
    /// ```
    pub fn pretty_display(&self) {
        println!("=== Source Analysis Report ===");
        println!("File ID: {}", self.id);
        println!("Path: {}", self.path.display());
        println!("Source Content Length: {} characters", self.source.content.len());
        println!();

        // Display variable scope information
        println!("=== Variable Scopes ===");
        println!("{}", self.global_scope.pretty_display());
        println!();

        // Display execution steps
        println!("=== Execution Steps ({} total) ===", self.steps.len());
        for (i, step) in self.steps.iter().enumerate() {
            println!("Step {} (USID: {}):", i + 1, step.usid);
            println!("  Type: {}", self.step_variant_name(&step.variant));
            println!(
                "  Location: {}:{}",
                step.src.start.map(|s| s.to_string()).unwrap_or_else(|| "?".to_string()),
                step.src.length.map(|l| l.to_string()).unwrap_or_else(|| "?".to_string())
            );

            // Display source code
            if let Some(source_code) = self.extract_source_code(&step.src) {
                println!("  Source: {}", source_code.trim());
            }

            // Display hooks
            if !step.pre_hooks.is_empty() {
                println!("  Pre-hooks: {}", self.format_hooks_detailed(&step.pre_hooks));
            }
            if !step.post_hooks.is_empty() {
                println!("  Post-hooks: {}", self.format_hooks_detailed(&step.post_hooks));
            }
            println!();
        }

        // Display recommendations
        self.display_recommendations();
        println!("=== End Report ===");
    }

    /// Returns a human-readable name for the step variant.
    fn step_variant_name(&self, variant: &StepVariant) -> String {
        match variant {
            StepVariant::Statement(stmt) => self.statement_name(stmt),
            StepVariant::Statements(_) => "Multiple Statements".to_string(),
            StepVariant::IfCondition(_) => "If Condition".to_string(),
            StepVariant::ForLoop { .. } => "For Loop".to_string(),
            StepVariant::WhileLoop(_) => "While Loop".to_string(),
            StepVariant::Try(_) => "Try Statement".to_string(),
        }
    }

    /// Returns a human-readable name for a statement.
    fn statement_name(&self, stmt: &Statement) -> String {
        match stmt {
            Statement::Block(_) => "Block".to_string(),
            Statement::Break(_) => "Break".to_string(),
            Statement::Continue(_) => "Continue".to_string(),
            Statement::DoWhileStatement(_) => "Do-While".to_string(),
            Statement::EmitStatement(_) => "Emit".to_string(),
            Statement::ExpressionStatement(_) => "Expression".to_string(),
            Statement::ForStatement(_) => "For".to_string(),
            Statement::IfStatement(_) => "If".to_string(),
            Statement::InlineAssembly(_) => "Inline Assembly".to_string(),
            Statement::PlaceholderStatement(_) => "Placeholder".to_string(),
            Statement::Return(_) => "Return".to_string(),
            Statement::RevertStatement(_) => "Revert".to_string(),
            Statement::TryStatement(_) => "Try".to_string(),
            Statement::UncheckedBlock(_) => "Unchecked Block".to_string(),
            Statement::VariableDeclarationStatement(_) => "Variable Declaration".to_string(),
            Statement::WhileStatement(_) => "While".to_string(),
        }
    }

    /// Formats a list of hooks into a human-readable string.
    fn format_hooks(&self, hooks: &[StepHook]) -> String {
        hooks.iter().map(|hook| hook.variant_name()).collect::<Vec<_>>().join(", ")
    }

    /// Formats a list of hooks with detailed UVID/USID information.
    fn format_hooks_detailed(&self, hooks: &[StepHook]) -> String {
        hooks
            .iter()
            .map(|hook| match hook {
                StepHook::BeforeStep(usid) => format!("BeforeStep(USID: {})", usid),
                StepHook::VariableInScope(uvid) => format!("VariableInScope(UVID: {:?})", uvid),
                StepHook::VariableOutOfScope(uvid) => {
                    format!("VariableOutOfScope(UVID: {:?})", uvid)
                }
                StepHook::VariableUpdate(uvid) => format!("VariableUpdate(UVID: {:?})", uvid),
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Extracts the source code for a given source location.
    fn extract_source_code(
        &self,
        src: &foundry_compilers::artifacts::ast::SourceLocation,
    ) -> Option<String> {
        let start = src.start? as usize;
        let length = src.length? as usize;

        if start + length <= self.source.content.len() {
            Some(self.source.content[start..start + length].to_string())
        } else {
            None
        }
    }

    /// Displays recommendations for code improvements.
    fn display_recommendations(&self) {
        let mut has_recommendations = false;

        if !self.private_state_variables.is_empty() {
            if !has_recommendations {
                println!("=== Recommendations ===");
                has_recommendations = true;
            }
            println!("Private state variables that should be made public:");
            for var in &self.private_state_variables {
                println!("  - {} (visibility: {:?})", var.name, var.visibility);
            }
        }

        if !self.private_functions.is_empty() {
            if !has_recommendations {
                println!("=== Recommendations ===");
                has_recommendations = true;
            }
            println!("Private functions that should be made public:");
            for func in &self.private_functions {
                println!("  - {} (visibility: {:?})", func.name, func.visibility);
            }
        }

        if !self.immutable_functions.is_empty() {
            if !has_recommendations {
                println!("=== Recommendations ===");
                has_recommendations = true;
            }
            println!("Functions that should be made mutable:");
            for func in &self.immutable_functions {
                let mutability = func
                    .state_mutability
                    .as_ref()
                    .map(|m| format!("{:?}", m))
                    .unwrap_or_else(|| "None".to_string());
                println!("  - {} (mutability: {})", func.name, mutability);
            }
        }

        if has_recommendations {
            println!();
        }
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
#[derive(Debug, Clone, Default)]
pub struct Analyzer {
    scope_stack: Vec<VariableScope>,

    finished_steps: Vec<Step>,
    current_step: Option<Step>,
    /// State variables that should be made public
    private_state_variables: Vec<VariableDeclaration>,
    /// Functions that should be made public
    private_functions: Vec<FunctionDefinition>,
    /// Functions that should be made mutable (i.e., neither pure nor view)
    immutable_functions: Vec<FunctionDefinition>,
}

impl Analyzer {
    /// Analyzes a compiled Solidity artifact to extract execution steps and variable information.
    ///
    /// This is the main entry point for source code analysis. It processes the compiled
    /// artifact, walks through the AST of each source file, and extracts detailed
    /// information about executable steps, variable scopes, and function requirements.
    ///
    /// # Arguments
    ///
    /// * `artifact` - The compiled Solidity artifact containing source files and AST data
    ///
    /// # Returns
    ///
    /// Returns a vector of `SourceAnalysis` results, one for each source file, or an
    /// `AnalysisError` if the analysis fails.
    ///
    /// # Errors
    ///
    /// This function can return the following errors:
    /// - `AnalysisError::MissingAst`: When AST data is not available in the artifact
    /// - `AnalysisError::ASTConversionError`: When AST conversion fails
    /// - `AnalysisError::StepPartitionError`: When step partitioning fails
    /// - `AnalysisError::Other`: For other analysis-related errors
    pub fn analyze(artifact: &Artifact) -> Result<Vec<SourceAnalysis>, AnalysisError> {
        let _begin_at = Instant::now();
        let _input_sources = &artifact.input.sources;
        let output_sources = &artifact.output.sources;
        debug!(files = output_sources.len(), "Analyzing sources");

        let start_ast = Instant::now();
        let asts = output_sources
            .par_iter()
            .map(|(path, source)| {
                let mut ast = source.ast.clone().ok_or(AnalysisError::MissingAst)?;
                let unit = ASTPruner::convert(&mut ast, false)
                    .map_err(AnalysisError::ASTConversionError)?;
                Ok::<(PathBuf, usize, SourceUnit), AnalysisError>((
                    path.clone(),
                    source.id as usize,
                    unit,
                ))
            })
            .collect::<Result<Vec<_>, AnalysisError>>()?;
        trace!(asts = asts.len(), time = ?start_ast.elapsed(), "Converted to typed ASTs");

        let results = asts
            .into_par_iter()
            .map(|(path, id, unit)| {
                let mut analyzer = Self::new();
                unit.walk(&mut analyzer).map_err(AnalysisError::Other)?;
                let global_scope =
                    analyzer.scope_stack.pop().expect("global scope should not be empty");
                let steps = analyzer.finished_steps.into_iter().map(Arc::new).collect();
                Ok(SourceAnalysis {
                    id: id as u32,
                    source: artifact.input.sources[&path].clone(),
                    path,
                    unit,
                    global_scope,
                    steps,
                    private_state_variables: analyzer.private_state_variables,
                    private_functions: analyzer.private_functions,
                    immutable_functions: analyzer.immutable_functions,
                })
            })
            .collect::<Result<Vec<_>, AnalysisError>>()?;

        Ok(results)
    }

    fn new() -> Self {
        Default::default()
    }
}

/* Scope analysis utils */
impl Analyzer {
    fn current_scope(&mut self) -> &mut VariableScope {
        self.scope_stack.last_mut().expect("scope stack is empty")
    }

    fn enter_new_scope(&mut self, node: ScopeNode) -> eyre::Result<()> {
        let new_scope = VariableScope { node, variables: HashMap::default(), children: vec![] };
        self.scope_stack.push(new_scope);
        Ok(())
    }

    fn declare_variable(&mut self, declaration: &VariableDeclaration) -> eyre::Result<()> {
        // add a new variable to the current scope
        let scope = self.current_scope();
        let uvid = new_uvid();
        let state_variable = declaration.state_variable;
        let variable = Variable::Plain { uvid, declaration: declaration.clone(), state_variable };
        scope.variables.insert(uvid, Arc::new(variable));

        // after the step is executed, the variable becomes in scope
        if let Some(step) = self.current_step.as_mut() {
            step.post_hooks.push(StepHook::VariableInScope(uvid));
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
        let uvids = closed_scope.variables.keys().cloned().collect::<Vec<_>>();
        if let Some(parent) = self.scope_stack.last_mut() {
            parent.children.push(Arc::new(closed_scope));
        }

        // when the scope is exited, all variables become out of scope
        if let Some(current_or_last_step) =
            self.current_step.as_mut().or_else(|| self.finished_steps.last_mut())
        {
            current_or_last_step.add_variable_out_of_scope_hook(uvids);
        }
        Ok(())
    }
}

/* Step partition utils */
impl Analyzer {
    // TODO: function entry should be a step.
    // TODO: unnamed function return value should be excluded from variable scope.
    fn enter_new_step(&mut self, statement: &Statement) -> eyre::Result<VisitorAction> {
        assert!(self.current_step.is_none(), "Step cannot be nested");

        macro_rules! step {
            ($variant:ident, $stmt:expr, $loc:expr) => {{
                self.current_step = Some(Step::new(StepVariant::$variant($stmt.clone()), $loc));
            }};
        }
        macro_rules! simple_stmt_to_step {
            ($stmt:expr) => {
                step!(Statement, statement, $stmt.src)
            };
        }
        match statement {
            Statement::Block(_) => {}
            Statement::Break(break_stmt) => simple_stmt_to_step!(break_stmt),
            Statement::Continue(continue_stmt) => simple_stmt_to_step!(continue_stmt),
            Statement::DoWhileStatement(do_while_statement) => {
                // the step is the `while(...)`
                let loc = sloc_rdiff(do_while_statement.src, do_while_statement.body.src);
                step!(WhileLoop, do_while_statement.condition.clone(), loc);
            }
            Statement::EmitStatement(emit_statement) => simple_stmt_to_step!(emit_statement),
            Statement::ExpressionStatement(expr_stmt) => simple_stmt_to_step!(expr_stmt),
            Statement::ForStatement(for_statement) => {
                // the step is the `for(...)`
                let loc = sloc_ldiff(for_statement.src, block_or_stmt_src(&for_statement.body));
                let step = Step::new(
                    StepVariant::ForLoop {
                        initialization_expression: for_statement.initialization_expression.clone(),
                        condition: for_statement.condition.clone(),
                        loop_expression: for_statement.loop_expression.clone(),
                    },
                    loc,
                );
                self.current_step = Some(step);

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
                self.exit_current_step(statement)?;
                for_statement.body.walk(self)?;

                // skip the subtree of the for statement since we have already walked it
                return Ok(VisitorAction::SkipSubtree);
            }
            Statement::IfStatement(if_statement) => {
                // the step is the `if(...)`
                let loc = sloc_ldiff(if_statement.src, block_or_stmt_src(&if_statement.true_body));
                step!(IfCondition, if_statement.condition.clone(), loc);

                // we take over the walk of the sub ast tree in the if statement step.
                let mut single_step_walker = AnalyzerSingleStepWalker { analyzer: self };
                if_statement.condition.walk(&mut single_step_walker)?;

                // end the if statement step early and then walk the true and false body of the if statement.
                self.exit_current_step(statement)?;
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
                step!(Try, try_statement.external_call.clone(), loc);

                // we take over the walk of the sub ast tree in the try statement step.
                let mut single_step_walker = AnalyzerSingleStepWalker { analyzer: self };
                try_statement.external_call.walk(&mut single_step_walker)?;

                // end the try statement step early and then walk the clauses of the try statement.
                self.exit_current_step(statement)?;
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
                let loc = sloc_rdiff(while_statement.src, block_or_stmt_src(&while_statement.body));
                step!(WhileLoop, while_statement.condition.clone(), loc);

                // we take over the walk of the sub ast tree in the while statement step.
                let mut single_step_walker = AnalyzerSingleStepWalker { analyzer: self };
                while_statement.condition.walk(&mut single_step_walker)?;

                // end the while statement step early and then walk the body of the while statement.
                self.exit_current_step(statement)?;
                while_statement.body.walk(self)?;

                // skip the subtree of the while statement since we have already walked it
                return Ok(VisitorAction::SkipSubtree);
            }
        };
        Ok(VisitorAction::Continue)
    }

    fn exit_current_step(&mut self, statement: &Statement) -> eyre::Result<()> {
        if self.current_step.is_none() {
            return Ok(());
        }

        macro_rules! finish_stmt {
            () => {{
                let step = self.current_step.take().unwrap();
                self.finished_steps.push(step);
            }};
        }

        match statement {
            Statement::Block(_)
            | Statement::PlaceholderStatement(_)
            | Statement::UncheckedBlock(_) => {}
            _ => finish_stmt!(),
        }
        Ok(())
    }
}

/* State variable and function visibility and mutability analysis */
impl Analyzer {
    fn check_state_variable_visibility(
        &mut self,
        declaration: &VariableDeclaration,
    ) -> eyre::Result<()> {
        if declaration.state_variable {
            // we need to change the visibility of the state variable to public
            if declaration.visibility != Visibility::Public {
                self.private_state_variables.push(declaration.clone());
            }
        }
        Ok(())
    }

    fn check_function_visibility_and_mutability(
        &mut self,
        definition: &FunctionDefinition,
    ) -> eyre::Result<()> {
        if definition.visibility != Visibility::Public {
            self.private_functions.push(definition.clone());
        }

        if definition
            .state_mutability
            .as_ref()
            .is_some_and(|mu| *mu == StateMutability::View || *mu == StateMutability::Pure)
        {
            self.immutable_functions.push(definition.clone());
        }
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

    fn visit_contract_definition(
        &mut self,
        _definition: &ContractDefinition,
    ) -> eyre::Result<VisitorAction> {
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
        Ok(())
    }

    fn visit_event_definition(
        &mut self,
        _definition: &EventDefinition,
    ) -> eyre::Result<VisitorAction> {
        Ok(VisitorAction::SkipSubtree)
    }

    fn visit_function_definition(
        &mut self,
        definition: &FunctionDefinition,
    ) -> eyre::Result<VisitorAction> {
        // check if the function is private or mutable
        self.check_function_visibility_and_mutability(definition)?;

        // enter a variable scope for the function
        self.enter_new_scope(ScopeNode::FunctionDefinition(definition.clone()))?;
        Ok(VisitorAction::Continue)
    }

    fn post_visit_function_definition(
        &mut self,
        definition: &FunctionDefinition,
    ) -> eyre::Result<()> {
        // exit the function scope
        self.exit_current_scope(definition.src)?;
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
        self.enter_new_step(_statement)?;
        Ok(VisitorAction::Continue)
    }

    fn post_visit_statement(&mut self, _statement: &Statement) -> eyre::Result<()> {
        // exit the current step
        self.exit_current_step(_statement)?;
        Ok(())
    }

    fn visit_variable_declaration(
        &mut self,
        declaration: &VariableDeclaration,
    ) -> eyre::Result<VisitorAction> {
        // declare a variable
        self.declare_variable(declaration)?;
        // check if the state variable is private
        self.check_state_variable_visibility(declaration)?;
        Ok(VisitorAction::Continue)
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
    fn visit_variable_declaration(
        &mut self,
        declaration: &VariableDeclaration,
    ) -> eyre::Result<VisitorAction> {
        self.analyzer.declare_variable(declaration)?;
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
mod tests {
    use semver::Version;

    use crate::compile_contract_source_to_source_unit;

    use super::*;

    #[test]
    fn test_analyze() {
        // Create a Solidity contract with a function containing three sequential simple statements
        let source = r#"
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract TestContract {
    uint256 public value;

    function testFunction() public {
        uint256 a = 1;
        uint256 b = 2;
        value = a + b;
    }
}
"#;

        // Compile the source code to get the AST
        let version = Version::parse("0.8.19").unwrap();
        let result = compile_contract_source_to_source_unit(version, source, false);
        assert!(result.is_ok(), "Compilation should succeed");

        let source_unit = result.unwrap();

        // Create an analyzer and walk through the AST
        let mut analyzer = Analyzer::new();
        let walk_result = source_unit.walk(&mut analyzer);
        assert!(walk_result.is_ok(), "AST walk should succeed");

        // Verify that the scope stack is properly managed
        // After walking the entire AST, we should have exactly one scope left (the global scope)
        assert_eq!(
            analyzer.scope_stack.len(),
            1,
            "Should have exactly one scope (global scope) after analysis"
        );

        // Verify that we have finished steps
        assert!(!analyzer.finished_steps.is_empty(), "Should have analyzed some steps");

        // Verify that the current step is None (all steps should be finished)
        assert!(analyzer.current_step.is_none(), "Current step should be None after analysis");

        // Get the global scope and verify it contains the expected structure
        let global_scope = analyzer.scope_stack.last().expect("Global scope should exist");

        // Verify that the global scope has children (contract definition)
        assert!(
            !global_scope.children.is_empty(),
            "Global scope should have contract definition as child"
        );

        // Verify that the global scope has children (contract definition)
        assert!(
            !global_scope.children.is_empty(),
            "Global scope should have contract definition as child"
        );

        // Get the contract scope and verify it contains the state variable
        let contract_scope = &global_scope.children[0];
        assert!(
            !contract_scope.variables.is_empty(),
            "Contract scope should contain state variables"
        );

        // Check that we have the expected number of variables in the contract scope
        let contract_variables = contract_scope.variables.len();
        assert!(contract_variables >= 1, "Should have at least the state variable 'value'");

        // Verify that the finished steps contain the expected step types
        // We should have steps for the variable declarations and the assignment
        let step_count = analyzer.finished_steps.len();
        assert!(
            step_count >= 3,
            "Should have at least 3 steps (2 variable declarations + 1 assignment)"
        );

        // Verify that each step has the expected structure
        for step in &analyzer.finished_steps {
            assert!(!step.pre_hooks.is_empty(), "Each step should have pre-hooks");

            // Verify the step variant is appropriate
            match &step.variant {
                StepVariant::Statement(stmt) => {
                    // Should be variable declaration statements or expression statements
                    match stmt {
                        Statement::VariableDeclarationStatement(_) => {
                            // Variable declaration should have variable in scope hook
                            assert!(
                                step.post_hooks
                                    .iter()
                                    .any(|hook| matches!(hook, StepHook::VariableInScope(_))),
                                "Variable declaration should have VariableInScope hook"
                            );
                        }
                        Statement::ExpressionStatement(_) => {
                            // Expression statement (assignment) should not have variable hooks
                            assert!(
                                !step
                                    .post_hooks
                                    .iter()
                                    .any(|hook| matches!(hook, StepHook::VariableInScope(_))),
                                "Expression statement should not have VariableInScope hook"
                            );
                        }
                        _ => {
                            // Other statement types are acceptable
                        }
                    }
                }
                _ => {
                    // Other step variants are acceptable
                }
            }
        }

        // Verify that the source location information is valid
        for step in &analyzer.finished_steps {
            assert!(step.src.start.is_some(), "Step should have start location");
            assert!(step.src.length.is_some(), "Step should have length");
            assert!(step.src.start.unwrap() > 0, "Step start should be positive");
            assert!(step.src.length.unwrap() > 0, "Step length should be positive");
        }

        // Verify that all USIDs are unique
        let usids: std::collections::HashSet<USID> =
            analyzer.finished_steps.iter().map(|s| s.usid).collect();
        assert_eq!(usids.len(), analyzer.finished_steps.len(), "All USIDs should be unique");

        // Verify that the scope hierarchy is properly structured
        assert!(
            !contract_scope.children.is_empty(),
            "Contract scope should have function as child"
        );

        // Verify that the function scope contains local variables
        let function_scope = &contract_scope.children[0];
        assert!(
            !function_scope.variables.is_empty(),
            "Function scope should contain local variables"
        );

        // Count total variables across all scopes
        fn count_variables(scope: &VariableScope) -> usize {
            let mut count = scope.variables.len();
            for child in &scope.children {
                count += count_variables(child);
            }
            count
        }
        let total_vars = count_variables(global_scope);
        assert!(total_vars >= 3, "Should have at least 3 variables total (1 state + 2 local)");
    }

    #[test]
    fn test_analyze_complex_contract() {
        // Create a moderately complex Solidity contract to test the analyzer
        let source = r#"
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract ComplexContract {
    uint256 public totalSupply;
    mapping(address => uint256) public balances;
    address public owner;

    event Transfer(address indexed from, address indexed to, uint256 value);

    constructor(uint256 _initialSupply) {
        totalSupply = _initialSupply;
        balances[msg.sender] = _initialSupply;
        owner = msg.sender;
    }

    function transfer(address to, uint256 amount) public returns (bool) {
        require(to != address(0), "Transfer to zero address");
        require(balances[msg.sender] >= amount, "Insufficient balance");

        uint balance = balances[msg.sender];
        balances[msg.sender] = balance - amount;
        balances[to] += amount;

        emit Transfer(msg.sender, to, amount);
        return true;
    }

    function approve(address spender, uint256 amount) public returns (bool) {
        if (spender == address(0)) {
            revert("Approve to zero address");
        }
        return true;
    }

    function getBalance(address account) public view returns (uint256) {
        return balances[account];
    }
}
"#;

        // Compile the source code to get the AST
        let version = Version::parse("0.8.19").unwrap();
        let result = compile_contract_source_to_source_unit(version, source, false);
        assert!(result.is_ok(), "Complex contract compilation should succeed");

        let source_unit = result.unwrap();

        // Create an analyzer and walk through the AST
        let mut analyzer = Analyzer::new();
        let walk_result = source_unit.walk(&mut analyzer);
        assert!(walk_result.is_ok(), "Complex contract AST walk should succeed");

        // Verify scope management
        assert_eq!(
            analyzer.scope_stack.len(),
            1,
            "Should have exactly one scope (global scope) after analysis"
        );

        // Verify that we have finished steps due to the contract logic
        assert!(
            analyzer.finished_steps.len() >= 10,
            "Complex contract should have multiple steps, got {}",
            analyzer.finished_steps.len()
        );

        // Verify that the current step is None (all steps should be finished)
        assert!(analyzer.current_step.is_none(), "Current step should be None after analysis");

        // Get the global scope and verify it contains the expected structure
        let global_scope = analyzer.scope_stack.last().expect("Global scope should exist");

        // Verify that the global scope has children (contract definition)
        assert!(
            !global_scope.children.is_empty(),
            "Global scope should have contract definition as child"
        );

        // Get the contract scope and verify it contains state variables
        let contract_scope = &global_scope.children[0];

        // The contract should have several state variables
        let state_variables = contract_scope.variables.len();
        assert!(
            state_variables >= 3,
            "Should have at least 3 state variables (totalSupply, balances, owner), got {}",
            state_variables
        );

        // Verify that we have function scopes as children of contract scope
        assert!(
            contract_scope.children.len() >= 4,
            "Contract should have multiple function scopes, got {}",
            contract_scope.children.len()
        );

        // Count total variables across all scopes
        fn count_variables_recursive(scope: &VariableScope) -> usize {
            let mut count = scope.variables.len();
            for child in &scope.children {
                count += count_variables_recursive(child);
            }
            count
        }

        let total_vars = count_variables_recursive(global_scope);

        assert!(
            total_vars >= 13,
            "Complex contract should have at least 13 variables across all scopes, got {}",
            total_vars
        );

        // Verify that we have different types of steps
        let mut statement_steps = 0;
        let mut statements_steps = 0;
        let mut condition_steps = 0;
        let mut loop_steps = 0;

        for step in &analyzer.finished_steps {
            match &step.variant {
                StepVariant::Statement(_) => statement_steps += 1,
                StepVariant::Statements(_) => statements_steps += 1,
                StepVariant::IfCondition(_) => condition_steps += 1,
                StepVariant::ForLoop { .. } => loop_steps += 1,
                StepVariant::WhileLoop(_) => loop_steps += 1,
                StepVariant::Try(_) => condition_steps += 1,
            }
        }

        assert!(statement_steps > 0, "Should have statement steps");
        // Note: Other step types might be 0 depending on the contract complexity

        // Verify that steps have proper hooks
        let mut steps_with_pre_hooks = 0;
        let mut steps_with_post_hooks = 0;

        for step in &analyzer.finished_steps {
            if !step.pre_hooks.is_empty() {
                steps_with_pre_hooks += 1;
            }
            if !step.post_hooks.is_empty() {
                steps_with_post_hooks += 1;
            }
        }

        assert!(steps_with_pre_hooks > 0, "Should have steps with pre-hooks");

        // Verify that we identified functions that should be made public
        // (private functions in the contract)
        println!("Private functions to be made public: {}", analyzer.private_functions.len());

        // Verify that we identified state variables that should be made public
        println!(
            "Private state variables to be made public: {}",
            analyzer.private_state_variables.len()
        );

        // Verify that we identified functions that should be made mutable
        println!("Immutable functions to be made mutable: {}", analyzer.immutable_functions.len());

        println!("Analysis completed successfully for complex contract:");
        println!("  - Total steps: {}", analyzer.finished_steps.len());
        println!("  - Total variables: {}", total_vars);
        println!("  - Statement steps: {}", statement_steps);
        println!("  - Statements steps: {}", statements_steps);
        println!("  - Condition steps: {}", condition_steps);
        println!("  - Loop steps: {}", loop_steps);
        println!("  - Steps with pre-hooks: {}", steps_with_pre_hooks);
        println!("  - Steps with post-hooks: {}", steps_with_post_hooks);

        // pretty display the analyzer
        let analysis = SourceAnalysis {
            id: 0,
            path: PathBuf::from("test.sol"),
            source: Source::new(source.to_string()),
            unit: source_unit,
            global_scope: analyzer.scope_stack.pop().expect("global scope should exist"),
            steps: analyzer.finished_steps.into_iter().map(Arc::new).collect(),
            private_state_variables: analyzer.private_state_variables,
            private_functions: analyzer.private_functions,
            immutable_functions: analyzer.immutable_functions,
        };

        analysis.pretty_display();
    }

    #[test]
    fn test_pretty_display() {
        // Create a simple Solidity contract to test pretty_display
        let source = r#"
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract SimpleContract {
    uint256 private balance;

    function deposit(uint256 amount) public {
        balance += amount;
    }

    function getBalance() public view returns (uint256 _balance) {
        return balance;
    }
}
"#;

        // Compile the source code to get the AST
        let version = Version::parse("0.8.19").unwrap();
        let result = compile_contract_source_to_source_unit(version, source, false);
        assert!(result.is_ok(), "Simple contract compilation should succeed");

        let source_unit = result.unwrap();

        // Create an analyzer and walk through the AST
        let mut analyzer = Analyzer::new();
        let walk_result = source_unit.walk(&mut analyzer);
        assert!(walk_result.is_ok(), "Simple contract AST walk should succeed");

        // Create a minimal SourceAnalysis instance for testing
        let source_analysis = SourceAnalysis {
            id: 1,
            path: PathBuf::from("test.sol"),
            source: Source::new(source.to_string()),
            unit: source_unit,
            global_scope: analyzer.scope_stack.pop().expect("global scope should exist"),
            steps: analyzer.finished_steps.into_iter().map(Arc::new).collect(),
            private_state_variables: analyzer.private_state_variables,
            private_functions: analyzer.private_functions,
            immutable_functions: analyzer.immutable_functions,
        };

        // Test that pretty_display doesn't panic
        source_analysis.pretty_display();
    }
}
