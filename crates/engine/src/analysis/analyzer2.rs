use std::collections::HashMap;

use crate::analysis::ScopeNode;

use super::AnalysisTypes;

/// An analyzer to analyze a single source file.
#[derive(Debug, Clone)]
pub struct SourceAnalyzer<T: AnalysisTypes> {
    source_id: u32,
    version_requirements: Vec<String>,

    /* internal states, varying as analysis goes */
    scope_stack: Vec<T::Scope>,
    finished_steps: Vec<T::Step>,
    current_step: Option<T::Step>,
    current_function: Option<T::Function>,
    current_contract: Option<T::Contract>,
    // List of all contracts in this file.
    contracts: Vec<T::Contract>,
    /// List of all functions in this file.
    functions: Vec<T::Function>,
    /// A mapping from the `VariableDeclaration` AST node ID to the variable reference.
    variables: HashMap<usize, T::Variable>,
    /// List of all state variables in this file.
    state_variables: Vec<T::Variable>,
    /// State variables that should be made public
    private_state_variables: Vec<T::Variable>,
    /// Functions that should be made public
    private_functions: Vec<T::Function>,
    /// Functions that should be made mutable (i.e., neither pure nor view)
    immutable_functions: Vec<T::Function>,
    /// User defined types defined in this file.
    defined_types: Vec<T::UserDefinedType>,
}
