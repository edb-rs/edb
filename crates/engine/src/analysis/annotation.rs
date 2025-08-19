use foundry_compilers::artifacts::{
    FunctionDefinition, StateMutability, VariableDeclaration, Visibility,
};

use crate::Visitor;

/// Enum representing different types of annotation changes that can be made to Solidity code
#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum AnnotationsToChange {
    /// Change the visibility of a state variable
    ///
    /// # Arguments
    ///
    /// * `declaration` - The variable declaration to change
    /// * `visibility` - The new visibility
    StateVariable {
        /// The variable declaration to be modified
        declaration: VariableDeclaration,
        /// The new visibility level to apply
        visibility: Visibility,
    },
    /// Change the visibility and mutability of a function
    ///
    /// # Arguments
    ///
    /// * `definition` - The function definition to change
    /// * `visibility` - The new visibility
    /// * `mutability` - The new mutability
    Function {
        /// The function definition to be modified
        definition: FunctionDefinition,
        /// The new visibility level to apply
        visibility: Visibility,
        /// The new mutability to apply (None to remove mutability)
        mutability: Option<StateMutability>,
    },
}

/// Analyzer that detects and tracks annotation changes needed for Solidity code
#[derive(Debug, Clone, Default)]
pub struct AnnotationAnalyzer {
    /// List of annotation changes detected during analysis
    changes: Vec<AnnotationsToChange>,
}

impl AnnotationAnalyzer {
    /// Creates a new `AnnotationAnalyzer` instance
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the list of annotation changes that were detected during analysis
    pub fn changes(&self) -> &[AnnotationsToChange] {
        &self.changes
    }
}

impl Visitor for AnnotationAnalyzer {
    fn visit_variable_declaration(
        &mut self,
        declaration: &VariableDeclaration,
    ) -> eyre::Result<()> {
        if declaration.state_variable {
            // we need to change the visibility of the state variable to public
            if declaration.visibility != Visibility::Public {
                self.changes.push(AnnotationsToChange::StateVariable {
                    declaration: declaration.clone(),
                    visibility: Visibility::Public,
                });
            }
        }
        Ok(())
    }

    fn visit_function_definition(&mut self, definition: &FunctionDefinition) -> eyre::Result<()> {
        if definition.visibility != Visibility::Public
            || definition
                .state_mutability
                .as_ref()
                .is_some_and(|mu| *mu == StateMutability::View || *mu == StateMutability::Pure)
        {
            self.changes.push(AnnotationsToChange::Function {
                definition: definition.clone(),
                visibility: Visibility::Public,
                mutability: None,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::analysis::visitor::Walk;
    use crate::utils::compile_contract_source_to_source_unit;
    use semver::Version;

    use super::*;

    #[test]
    fn test_annotation_analyzer() {
        let source = r#"
        contract C {
            uint256 private stateVar;
            uint256 public stateVar2;
            function f() public view {}
            function f2() public pure {}
            function f3() public {}
        }
        "#;
        let version = Version::parse("0.8.19").unwrap();

        // Compile the source code to get the AST
        let source_unit = compile_contract_source_to_source_unit(version, source, true)
            .expect("Failed to compile contract");

        // Create the analyzer
        let mut analyzer = AnnotationAnalyzer::new();

        // Walk through the AST using the visitor pattern
        source_unit.walk(&mut analyzer).expect("Failed to walk AST");

        // Get the changes that were detected
        let changes = analyzer.changes();

        // Assert that we detected the expected changes
        // We should have 3 changes:
        // 1. The private state variable should be changed to public
        // 2. The view function should be changed to remove view modifier
        // 3. The pure function should be changed to remove pure modifier

        assert_eq!(changes.len(), 3, "Expected 3 changes, got {}", changes.len());

        // Check that the private state variable is marked for change
        let state_var_change = changes
            .iter()
            .find(|change| matches!(change, AnnotationsToChange::StateVariable { .. }));
        assert!(state_var_change.is_some(), "Should detect state variable visibility change");

        // Check that the view and pure functions are marked for change
        let function_changes: Vec<_> = changes
            .iter()
            .filter(|change| matches!(change, AnnotationsToChange::Function { .. }))
            .collect();
        assert_eq!(function_changes.len(), 2, "Should detect 2 function mutability changes");

        // Verify the specific changes
        for change in changes {
            match change {
                AnnotationsToChange::StateVariable { declaration, visibility } => {
                    assert_eq!(
                        visibility,
                        &Visibility::Public,
                        "State variable should be changed to public"
                    );
                    assert_eq!(
                        declaration.visibility,
                        Visibility::Private,
                        "Original state variable should be private"
                    );
                }
                AnnotationsToChange::Function { definition, visibility, mutability } => {
                    assert_eq!(
                        visibility,
                        &Visibility::Public,
                        "Function should be changed to public"
                    );
                    assert_eq!(mutability, &None, "Function mutability should be removed");
                    // The function should be either the view or pure function
                    assert!(
                        definition.name == "f" || definition.name == "f2",
                        "Should be view or pure function"
                    );
                }
            }
        }
    }
}
