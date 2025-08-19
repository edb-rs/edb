//! Variable analysis and representation for Ethereum smart contract analysis.
//!
//! This module provides the core data structures and utilities for representing
//! and tracking variables during smart contract analysis. It includes:
//!
//! - **UVID (Universal Variable Identifier)**: A unique identifier system for
//!   tracking variables across different scopes and contexts
//! - **Variable**: The main data structure representing a smart contract variable
//! - **VariableType**: Enumeration of supported Solidity variable types
//! - **VariableScope**: Structure for managing variable scope information
//!
//! The module is designed to work with the broader analysis framework to provide
//! comprehensive variable tracking and type information during contract analysis.

use std::sync::{Arc, Mutex};

use alloy_primitives::{map::foldhash::HashMap, U256};
use derive_more::From;
use foundry_compilers::artifacts::{
    ast::SourceLocation, Block, ContractDefinition, Expression, ForStatement, SourceUnit,
    UncheckedBlock, VariableDeclaration,
};
use lazy_static::lazy_static;

use crate::{Visitor, Walk};

lazy_static! {
    /// Global counter for generating unique variable identifiers (UVIDs). It is
    /// also the storage slot that a variable should be stored in storage during debugging.
    ///
    /// This mutex-protected counter ensures thread-safe generation of unique
    /// identifiers across multiple analysis contexts. The counter starts from
    /// `EDB_RUNTIME_VALUE_OFFSET` to avoid conflicts with other identifier spaces.
    pub static ref NEXT_UVID: Mutex<UVID> = Mutex::new(UVID(EDB_RUNTIME_VALUE_OFFSET));
}

/// The slot where the `edb_runtime_values` mapping is stored.
///
/// This constant represents the first 8 bytes of the keccak256 hash of the string
/// "EDB_RUNTIME_VALUE_OFFSET". It serves as the starting point for UVID generation
/// to ensure unique identifier spaces across different analysis contexts.
pub const EDB_RUNTIME_VALUE_OFFSET: u64 = 0x234c6dfc3bf8fed1;

/// A Universal Variable Identifier (UVID) is a unique identifier for a variable in a contract.
///
/// UVIDs provide a way to uniquely identify variables across different scopes,
/// contexts, and analysis passes. They are used internally by the analysis engine
/// to track variable relationships and dependencies.
///
/// # Examples
///
/// ```rust
/// use edb::analysis::variable::{UVID, new_uvid};
///
/// let uvid1 = new_uvid();
/// let uvid2 = new_uvid();
/// assert_ne!(uvid1, uvid2);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct UVID(u64);

impl UVID {
    /// Increment the UVID and return the previous value.
    ///
    /// This method atomically increments the internal counter and returns
    /// the previous value, ensuring each call produces a unique identifier.
    ///
    /// # Returns
    ///
    /// The previous UVID value before incrementing.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use edb::analysis::variable::UVID;
    ///
    /// let mut uvid = UVID(42);
    /// let previous = uvid.inc();
    /// assert_eq!(previous, UVID(42));
    /// assert_eq!(uvid, UVID(43));
    /// ```
    pub fn inc(&mut self) -> Self {
        let v = *self;
        self.0 += 1;
        v
    }
}

impl From<UVID> for u64 {
    /// Convert a UVID to its underlying u64 representation.
    fn from(uvid: UVID) -> Self {
        uvid.0
    }
}

impl From<UVID> for U256 {
    /// Convert a UVID to a U256 representation for use in Ethereum-related operations.
    fn from(uvid: UVID) -> Self {
        Self::from(uvid.0)
    }
}

/// Generate a new unique variable identifier (UVID).
///
/// This function provides a thread-safe way to generate unique identifiers
/// for variables. Each call returns a new UVID that is guaranteed to be
/// unique within the current analysis session.
///
/// # Returns
///
/// A new unique UVID.
///
/// # Examples
///
/// ```rust
/// use edb::analysis::variable::new_uvid;
///
/// let uvid1 = new_uvid();
/// let uvid2 = new_uvid();
/// assert_ne!(uvid1, uvid2);
/// ```
pub fn new_uvid() -> UVID {
    let mut uvid = NEXT_UVID.lock().unwrap();
    uvid.inc()
}

/// A reference-counted pointer to a Variable.
///
/// This type alias provides shared ownership of Variable instances, allowing
/// multiple parts of the analysis system to reference the same variable
/// without copying the data.
pub type VariableRef = Arc<Variable>;

/// Represents a variable in a smart contract with its metadata and type information.
///
/// Currently, only local variables are supported.
///
/// The Variable struct contains all the information needed to track and analyze
/// a variable during contract analysis, including its unique identifier, name,
/// declaration details, type, and scope information.
///
/// # Examples
///
/// ```rust
/// use edb::analysis::variable::{Variable, UVID, VariableType, VariableScope};
/// use foundry_compilers::artifacts::VariableDeclaration;
///
/// let variable = Variable {
///     uvid: UVID(1),
///     name: "balance".to_string(),
///     declare: VariableDeclaration::default(),
///     ty: VariableType::Uint(256),
///     scope: VariableScope {},
/// };
/// ```
#[derive(Debug, Clone)]
#[non_exhaustive]
#[allow(clippy::large_enum_variant)]
pub enum Variable {
    /// A plain variable with a direct declaration.
    Plain {
        /// The unique variable identifier.
        uvid: UVID,
        /// The variable declaration from the AST.
        declaration: VariableDeclaration,
        /// Whether this is a state variable (true) or local variable (false).
        state_variable: bool,
    },
    /// A member access variable (e.g., `obj.field`).
    Member {
        /// The base variable being accessed.
        base: VariableRef,
        /// The name of the member being accessed.
        member: String,
    },
    /// An array or mapping index access variable (e.g., `arr[index]`).
    Index {
        /// The base variable being indexed.
        base: VariableRef,
        /// The index expression.
        index: Expression,
    },
    /// An array slice access variable (e.g., `arr[start:end]`).
    IndexRange {
        /// The base variable being sliced.
        base: VariableRef,
        /// The start index expression.
        start: Expression,
        /// The end index expression.
        end: Expression,
    },
}

impl Variable {
    /// Returns a human-readable string representation of the variable.
    ///
    /// This method provides a concise display format for variables:
    /// - Plain variables show their declaration name
    /// - Member access shows `base.member`
    /// - Index access shows `base[.]`
    /// - Index range shows `base[..]`
    pub fn pretty_display(&self) -> String {
        match self {
            Self::Plain { declaration, .. } => declaration.name.clone(),
            Self::Member { base, member } => format!("{}.{}", base.pretty_display(), member),
            Self::Index { base, .. } => format!("{}[.]", base.pretty_display()),
            Self::IndexRange { base, .. } => {
                format!("{}[..]", base.pretty_display())
            }
        }
    }
}

/// Represents the type of a smart contract variable.
///
/// This enum covers the basic Solidity types that are commonly used in
/// smart contract analysis. The types are designed to be extensible for
/// future additions.
///
/// # Examples
///
/// ```rust
/// use edb::analysis::variable::VariableType;
///
/// let uint_type = VariableType::Uint(256);
/// let address_type = VariableType::Address;
/// let bool_type = VariableType::Bool;
/// ```
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum VariableType {
    /// A `uint` type variable. The number of bits is specified by the parameter.
    ///
    /// For instance, `Uint(8)` denotes a `uint8` Solidity type, while `Uint(256)`
    /// represents a `uint256` (the default uint type in Solidity).
    Uint(u8),
    /// An `address` type variable representing an Ethereum address.
    ///
    /// This type is used for variables that store 20-byte Ethereum addresses.
    Address,
    /// A `bool` type variable representing a boolean value.
    ///
    /// This type is used for variables that can be either `true` or `false`.
    Bool,
}

/// Analyzes variables in a Solidity source unit and returns a variable analyzer.
///
/// This function creates a new `VariableAnalyzer` and walks through the provided
/// Abstract Syntax Tree (AST) to extract and analyze all variable declarations
/// and their scopes.
///
/// # Arguments
///
/// * `ast` - The source unit AST to analyze
///
/// # Returns
///
/// Returns a `VariableAnalyzer` containing the analyzed variables and scope hierarchy,
/// or an error if the analysis fails.
///
/// # Examples
///
/// ```rust,no_run
/// use edb_engine::analysis::variable::analyze_variables;
/// use foundry_compilers::artifacts::SourceUnit;
///
/// let ast: SourceUnit = // ... get AST from compilation
/// let analyzer = analyze_variables(&ast)?;
/// # Ok::<(), eyre::Error>(())
/// ```
pub fn analyze_variables(ast: &SourceUnit) -> eyre::Result<VariableAnalyzer> {
    let mut analyzer = VariableAnalyzer::new();
    ast.walk(&mut analyzer)?;
    Ok(analyzer)
}

/// A reference-counted pointer to a VariableScope.
pub type VariableScopeRef = Arc<VariableScope>;

/// Represents the scope and visibility information for a variable.
///
/// This structure contains information about where a variable is defined
/// and how it can be accessed. Currently, this is a placeholder structure
/// that can be extended with additional scope-related information as needed.
///
/// # Future Extensions
///
/// This structure may be extended to include:
/// - Function scope information
/// - Contract scope information
/// - Visibility modifiers (public, private, internal, external)
/// - Storage location (storage, memory, calldata)
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct VariableScope {
    node: ScopeNode,
    variables: HashMap<UVID, VariableRef>,
    children: Vec<VariableScopeRef>,
}

impl VariableScope {
    /// Returns the source location of this scope's AST node.
    pub fn src(&self) -> SourceLocation {
        self.node.src()
    }

    /// Returns a human-readable string representation of the scope hierarchy.
    ///
    /// This method displays the scope and all its child scopes in a tree-like format,
    /// showing the variables contained in each scope.
    pub fn pretty_display(&self) -> String {
        self.pretty_display_with_indent(0)
    }

    fn pretty_display_with_indent(&self, indent_level: usize) -> String {
        let mut result = String::new();
        let indent = "  ".repeat(indent_level);

        // Print current scope's variables
        if self.variables.is_empty() {
            result.push_str(&format!("{}Scope({}): {{}}", indent, self.node.variant_name()));
        } else {
            let mut variable_names: Vec<String> =
                self.variables.values().map(|var| var.pretty_display()).collect();
            variable_names.sort(); // Sort for consistent output
            result.push_str(&format!(
                "{}Scope({}): {{{}}}",
                indent,
                self.node.variant_name(),
                variable_names.join(", ")
            ));
        }

        // Print children scopes recursively with increased indentation
        for child in &self.children {
            result.push('\n');
            result.push_str(&child.pretty_display_with_indent(indent_level + 1));
        }

        result
    }
}

/// Represents different types of AST nodes that can define variable scopes.
///
/// This enum wraps various Solidity AST node types that create new variable scopes,
/// allowing the variable analyzer to track scope boundaries and variable visibility.
#[derive(Debug, Clone, From)]
pub enum ScopeNode {
    /// A source unit scope (file-level).
    SourceUnit(#[from] SourceUnit),
    /// A block statement scope.
    Block(#[from] Block),
    /// An unchecked block scope.
    UncheckedBlock(#[from] UncheckedBlock),
    /// A for loop scope.
    ForStatement(#[from] ForStatement),
    /// A contract definition scope.
    ContractDefinition(#[from] ContractDefinition),
}

impl ScopeNode {
    /// Returns the source location of the wrapped AST node.
    pub fn src(&self) -> SourceLocation {
        match self {
            Self::SourceUnit(source_unit) => source_unit.src,
            Self::Block(block) => block.src,
            Self::UncheckedBlock(unchecked_block) => unchecked_block.src,
            Self::ForStatement(for_statement) => for_statement.src,
            Self::ContractDefinition(contract_definition) => contract_definition.src,
        }
    }

    /// Returns a string representation of the scope node variant name.
    pub fn variant_name(&self) -> &'static str {
        match self {
            Self::SourceUnit(_) => "SourceUnit",
            Self::Block(_) => "Block",
            Self::UncheckedBlock(_) => "UncheckedBlock",
            Self::ForStatement(_) => "ForStatement",
            Self::ContractDefinition(_) => "ContractDefinition",
        }
    }
}

/// Analyzes variables and their scopes in Solidity contracts.
///
/// The `VariableAnalyzer` implements the `Visitor` trait to traverse the AST
/// and build a hierarchical representation of variable scopes and their contained variables.
/// It maintains a stack of scopes to properly track nesting and variable visibility.
pub struct VariableAnalyzer {
    scope_stack: Vec<VariableScope>,
}

impl VariableAnalyzer {
    /// Creates a new variable analyzer with an empty scope stack.
    pub fn new() -> Self {
        Self { scope_stack: vec![] }
    }

    fn current_scope(&mut self) -> &mut VariableScope {
        self.scope_stack.last_mut().expect("scope stack is empty")
    }

    fn enter_new_scope(&mut self, node: ScopeNode) -> eyre::Result<()> {
        let new_scope = VariableScope { node, variables: HashMap::default(), children: vec![] };
        self.scope_stack.push(new_scope);
        Ok(())
    }

    fn exit_current_scope(&mut self) -> eyre::Result<()> {
        let closed_scope = self.scope_stack.pop().expect("scope stack is empty");
        if let Some(parent) = self.scope_stack.last_mut() {
            parent.children.push(Arc::new(closed_scope));
        }
        Ok(())
    }
}

impl Default for VariableAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl Visitor for VariableAnalyzer {
    fn visit_source_unit(&mut self, _source_unit: &SourceUnit) -> eyre::Result<()> {
        self.enter_new_scope(ScopeNode::SourceUnit(_source_unit.clone()))?;
        // we don't need to exit the source unit scope
        Ok(())
    }

    fn visit_block(&mut self, block: &Block) -> eyre::Result<()> {
        self.enter_new_scope(ScopeNode::Block(block.clone()))?;
        Ok(())
    }

    fn post_visit_block(&mut self, block: &Block) -> eyre::Result<()> {
        assert_eq!(
            self.current_scope().src(),
            block.src,
            "scope mismatch: the post-visit block's source location does not match the current scope's location"
        );
        self.exit_current_scope()?;
        Ok(())
    }

    fn visit_unchecked_block(&mut self, unchecked_block: &UncheckedBlock) -> eyre::Result<()> {
        self.enter_new_scope(ScopeNode::UncheckedBlock(unchecked_block.clone()))?;
        Ok(())
    }

    fn post_visit_unchecked_block(&mut self, unchecked_block: &UncheckedBlock) -> eyre::Result<()> {
        assert_eq!(
            self.current_scope().src(),
            unchecked_block.src,
            "scope mismatch: the post-visit block's source location does not match the current scope's location"
        );
        self.exit_current_scope()?;
        Ok(())
    }

    fn visit_for_statement(&mut self, for_statement: &ForStatement) -> eyre::Result<()> {
        self.enter_new_scope(ScopeNode::ForStatement(for_statement.clone()))?;
        Ok(())
    }

    fn post_visit_for_statement(&mut self, for_statement: &ForStatement) -> eyre::Result<()> {
        assert_eq!(
            self.current_scope().src(),
            for_statement.src,
            "scope mismatch: the post-visit for statement's source location does not match the current scope's location"
        );
        self.exit_current_scope()?;
        Ok(())
    }

    fn visit_contract_definition(&mut self, _definition: &ContractDefinition) -> eyre::Result<()> {
        self.enter_new_scope(ScopeNode::ContractDefinition(_definition.clone()))?;
        Ok(())
    }

    fn post_visit_contract_definition(
        &mut self,
        definition: &ContractDefinition,
    ) -> eyre::Result<()> {
        assert_eq!(
            self.current_scope().src(),
            definition.src,
            "scope mismatch: the post-visit contract definition's source location does not match the current scope's location"
        );
        self.exit_current_scope()?;
        Ok(())
    }

    fn visit_variable_declaration(
        &mut self,
        declaration: &VariableDeclaration,
    ) -> eyre::Result<()> {
        let scope = self.current_scope();
        let uvid = new_uvid();
        let state_variable = declaration.state_variable;
        let variable = Variable::Plain { uvid, declaration: declaration.clone(), state_variable };
        scope.variables.insert(uvid, Arc::new(variable));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use semver::Version;

    use crate::utils::compile_contract_source_to_source_unit;

    use super::*;

    #[test]
    fn test_extract_variables_from_simple_declaration() {
        // Create a Solidity contract with a function containing three sequential simple statements
        let source = r#"
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract TestContract {
    uint256 public value;

    function testFunction() public {
        uint256 a = 1;
        uint256 b = 2;
    }
}
"#;

        // Compile the source code to get the AST
        let version = Version::parse("0.8.19").unwrap();
        let result = compile_contract_source_to_source_unit(version, source, false);
        assert!(result.is_ok(), "Compilation should succeed");

        let ast = result.unwrap();

        let mut analyzer = VariableAnalyzer::new();
        ast.walk(&mut analyzer).expect("Analysis should succeed");

        let scope = analyzer.current_scope();

        // Helper function to collect all variables from scope hierarchy
        fn collect_all_variables(scope: &VariableScope) -> Vec<(&UVID, &VariableRef)> {
            let mut variables = Vec::new();
            variables.extend(scope.variables.iter());
            for child in &scope.children {
                variables.extend(collect_all_variables(child));
            }
            variables
        }

        let all_variables = collect_all_variables(scope);
        assert_eq!(all_variables.len(), 3, "Should extract 3 variables total");

        // Check that all variables have unique UVIDs
        let uvid_set: std::collections::HashSet<UVID> =
            all_variables.iter().map(|(uvid, _)| **uvid).collect();
        assert_eq!(uvid_set.len(), 3, "All variables should have unique UVIDs");

        // Find and verify the state variable
        let state_variables: Vec<_> = all_variables
            .iter()
            .filter_map(|(_, var)| {
                if let Variable::Plain { state_variable: true, .. } = var.as_ref() {
                    Some(var)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(state_variables.len(), 1, "Should have exactly one state variable");

        let state_var = &state_variables[0];
        if let Variable::Plain { declaration, state_variable, .. } = state_var.as_ref() {
            assert_eq!(declaration.name, "value", "State variable should be named 'value'");
            assert!(*state_variable, "State variable flag should be true");
        } else {
            panic!("State variable should be a Plain variable");
        }

        // Find and verify the local variables
        let local_variables: Vec<_> = all_variables
            .iter()
            .filter_map(|(_, var)| {
                if let Variable::Plain { state_variable: false, .. } = var.as_ref() {
                    Some(var)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(local_variables.len(), 2, "Should have exactly two local variables");

        // Check variable names
        let variable_names: Vec<String> =
            all_variables.iter().map(|(_, var)| var.pretty_display()).collect();
        assert!(variable_names.contains(&"value".to_string()), "Should contain variable 'value'");
        assert!(variable_names.contains(&"a".to_string()), "Should contain variable 'a'");
        assert!(variable_names.contains(&"b".to_string()), "Should contain variable 'b'");

        // Verify UVID generation starts from the expected offset
        let uvid_values: Vec<u64> = all_variables.iter().map(|(uvid, _)| uvid.0).collect();
        for uvid_value in uvid_values {
            assert!(
                uvid_value >= EDB_RUNTIME_VALUE_OFFSET,
                "UVID should be >= EDB_RUNTIME_VALUE_OFFSET, got {uvid_value}"
            );
        }

        // Check that variables are properly stored in the scope
        for (uvid, variable) in all_variables {
            assert!(uvid.0 > 0, "UVID should be positive");
            assert!(
                !variable.pretty_display().is_empty(),
                "Variable should have a non-empty display name"
            );
        }

        // Check scope structure
        assert_eq!(scope.children.len(), 1, "Should have one contract definition child");
        let contract_scope = &scope.children[0];
        assert_eq!(
            contract_scope.variables.len(),
            1,
            "Contract scope should have 1 state variable"
        );
        assert_eq!(contract_scope.children.len(), 1, "Contract should have one function child");

        let function_scope = &contract_scope.children[0];
        assert_eq!(
            function_scope.variables.len(),
            2,
            "Function scope should have 2 local variables"
        );
    }

    #[test]
    fn test_extract_variables_with_different_types_and_scopes() {
        let source = r#"
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract ComplexContract {
    address public owner;
    bool public isActive;
    uint256 public balance;

    function complexFunction() public {
        uint256 counter = 0;
        address recipient = address(0x123);
        bool success = true;

        {
            uint256 innerVar = 42;
            address innerAddr = address(0x456);
        }

        for (uint256 i = 0; i < 10; i++) {
            uint256 loopVar = i * 2;
        }
    }
}
"#;

        let version = Version::parse("0.8.19").unwrap();
        let result = compile_contract_source_to_source_unit(version, source, false);
        assert!(result.is_ok(), "Compilation should succeed");

        let ast = result.unwrap();
        let mut analyzer = VariableAnalyzer::new();
        ast.walk(&mut analyzer).expect("Analysis should succeed");

        let scope = analyzer.current_scope();

        // Helper function to collect all variables from scope hierarchy
        fn collect_all_variables(scope: &VariableScope) -> Vec<(&UVID, &VariableRef)> {
            let mut variables = Vec::new();
            variables.extend(scope.variables.iter());
            for child in &scope.children {
                variables.extend(collect_all_variables(child));
            }
            variables
        }

        let all_variables = collect_all_variables(scope);

        // Should have 3 state variables + 7 local variables = 10 total
        assert_eq!(all_variables.len(), 10, "Should extract 10 variables total");

        // Check state variables
        let state_variables: Vec<_> = all_variables
            .iter()
            .filter_map(|(_, var)| {
                if let Variable::Plain { state_variable: true, .. } = var.as_ref() {
                    Some(var)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(state_variables.len(), 3, "Should have exactly 3 state variables");

        // Check local variables
        let local_variables: Vec<_> = all_variables
            .iter()
            .filter_map(|(_, var)| {
                if let Variable::Plain { state_variable: false, .. } = var.as_ref() {
                    Some(var)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(local_variables.len(), 7, "Should have exactly 7 local variables");

        // Verify all variables have unique UVIDs
        let uvid_set: std::collections::HashSet<UVID> =
            all_variables.iter().map(|(uvid, _)| **uvid).collect();
        assert_eq!(uvid_set.len(), 10, "All variables should have unique UVIDs");

        // Check variable names
        let variable_names: Vec<String> =
            all_variables.iter().map(|(_, var)| var.pretty_display()).collect();

        // State variables
        assert!(
            variable_names.contains(&"owner".to_string()),
            "Should contain state variable 'owner'"
        );
        assert!(
            variable_names.contains(&"isActive".to_string()),
            "Should contain state variable 'isActive'"
        );
        assert!(
            variable_names.contains(&"balance".to_string()),
            "Should contain state variable 'balance'"
        );

        // Local variables
        assert!(
            variable_names.contains(&"counter".to_string()),
            "Should contain local variable 'counter'"
        );
        assert!(
            variable_names.contains(&"recipient".to_string()),
            "Should contain local variable 'recipient'"
        );
        assert!(
            variable_names.contains(&"success".to_string()),
            "Should contain local variable 'success'"
        );
        assert!(
            variable_names.contains(&"innerVar".to_string()),
            "Should contain local variable 'innerVar'"
        );
        assert!(
            variable_names.contains(&"innerAddr".to_string()),
            "Should contain local variable 'innerAddr'"
        );
        assert!(variable_names.contains(&"i".to_string()), "Should contain local variable 'i'");
        assert!(
            variable_names.contains(&"loopVar".to_string()),
            "Should contain local variable 'loopVar'"
        );

        // Verify UVID properties
        for (uvid, variable) in all_variables {
            assert!(
                uvid.0 >= EDB_RUNTIME_VALUE_OFFSET,
                "UVID should be >= EDB_RUNTIME_VALUE_OFFSET, got {}",
                uvid.0
            );
            assert!(
                !variable.pretty_display().is_empty(),
                "Variable should have a non-empty display name"
            );

            // Check that each variable has a proper declaration
            if let Variable::Plain { declaration, .. } = variable.as_ref() {
                assert!(!declaration.name.is_empty(), "Variable declaration should have a name");
            }
        }

        // Check scope structure
        assert_eq!(scope.children.len(), 1, "Should have one contract definition child");
        let contract_scope = &scope.children[0];
        assert_eq!(
            contract_scope.variables.len(),
            3,
            "Contract scope should have 3 state variables"
        );
        assert_eq!(contract_scope.children.len(), 1, "Contract should have one function child");

        let function_scope = &contract_scope.children[0];
        assert_eq!(
            function_scope.variables.len(),
            3,
            "Function scope should have 3 local variables"
        );
        // The function has 2 block scopes (the for loop might not create a separate scope)
        assert_eq!(
            function_scope.children.len(),
            2,
            "Function should have 2 child scopes (2 blocks)"
        );

        // Verify the structure: function scope contains counter, recipient, success
        // Block 1 contains innerVar, innerAddr
        // Block 2 contains the for loop with i and loopVar
        let function_var_names: Vec<String> =
            function_scope.variables.values().map(|var| var.pretty_display()).collect();
        assert!(
            function_var_names.contains(&"counter".to_string()),
            "Function scope should contain 'counter'"
        );
        assert!(
            function_var_names.contains(&"recipient".to_string()),
            "Function scope should contain 'recipient'"
        );
        assert!(
            function_var_names.contains(&"success".to_string()),
            "Function scope should contain 'success'"
        );
    }

    #[test]
    fn test_variable_scope_hierarchy() {
        let source = r#"
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract ScopeTest {
    uint256 public globalVar;

    function testScopes() public {
        uint256 functionVar = 1;

        {
            uint256 blockVar = 2;
        }

        {
            uint256 anotherBlockVar = 3;
        }
    }
}
"#;

        let version = Version::parse("0.8.19").unwrap();
        let result = compile_contract_source_to_source_unit(version, source, false);
        assert!(result.is_ok(), "Compilation should succeed");

        let ast = result.unwrap();
        let mut analyzer = VariableAnalyzer::new();
        ast.walk(&mut analyzer).expect("Analysis should succeed");

        let scope = analyzer.current_scope();

        // Helper function to collect all variables from scope hierarchy
        fn collect_all_variables(scope: &VariableScope) -> Vec<(&UVID, &VariableRef)> {
            let mut variables = Vec::new();
            variables.extend(scope.variables.iter());
            for child in &scope.children {
                variables.extend(collect_all_variables(child));
            }
            variables
        }

        let all_variables = collect_all_variables(scope);

        // Should have 1 state variable + 3 local variables = 4 total
        assert_eq!(all_variables.len(), 4, "Should extract 4 variables total");

        // Check scope structure
        assert_eq!(scope.children.len(), 1, "Should have one contract definition child");

        let contract_scope = &scope.children[0];
        assert_eq!(
            contract_scope.variables.len(),
            1,
            "Contract scope should have 1 state variable"
        );
        assert_eq!(contract_scope.children.len(), 1, "Contract should have one function child");

        let function_scope = &contract_scope.children[0];
        assert_eq!(
            function_scope.variables.len(),
            1,
            "Function scope should have 1 local variable"
        );
        assert_eq!(function_scope.children.len(), 2, "Function should have 2 block children");

        // Verify variable names in each scope
        let global_var_names: Vec<String> =
            scope.variables.values().map(|var| var.pretty_display()).collect();
        // The global scope (SourceUnit) doesn't contain variables directly
        assert_eq!(global_var_names.len(), 0, "Global scope should not contain variables directly");

        let contract_var_names: Vec<String> =
            contract_scope.variables.values().map(|var| var.pretty_display()).collect();
        assert!(
            contract_var_names.contains(&"globalVar".to_string()),
            "Contract scope should contain 'globalVar'"
        );

        let function_var_names: Vec<String> =
            function_scope.variables.values().map(|var| var.pretty_display()).collect();
        assert!(
            function_var_names.contains(&"functionVar".to_string()),
            "Function scope should contain 'functionVar'"
        );

        // Check block scopes
        for block_scope in &function_scope.children {
            assert_eq!(block_scope.variables.len(), 1, "Each block should have exactly 1 variable");
        }

        // Verify all variables have unique UVIDs
        let uvid_set: std::collections::HashSet<UVID> =
            all_variables.iter().map(|(uvid, _)| **uvid).collect();
        assert_eq!(uvid_set.len(), 4, "All variables should have unique UVIDs");

        // Check variable names across all scopes
        let all_variable_names: Vec<String> =
            all_variables.iter().map(|(_, var)| var.pretty_display()).collect();
        assert!(
            all_variable_names.contains(&"globalVar".to_string()),
            "Should contain 'globalVar'"
        );
        assert!(
            all_variable_names.contains(&"functionVar".to_string()),
            "Should contain 'functionVar'"
        );
        assert!(all_variable_names.contains(&"blockVar".to_string()), "Should contain 'blockVar'");
        assert!(
            all_variable_names.contains(&"anotherBlockVar".to_string()),
            "Should contain 'anotherBlockVar'"
        );

        // Verify UVID properties
        for (uvid, variable) in all_variables {
            assert!(
                uvid.0 >= EDB_RUNTIME_VALUE_OFFSET,
                "UVID should be >= EDB_RUNTIME_VALUE_OFFSET, got {}",
                uvid.0
            );
            assert!(
                !variable.pretty_display().is_empty(),
                "Variable should have a non-empty display name"
            );
        }
    }
}
