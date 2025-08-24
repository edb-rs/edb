use std::collections::BTreeMap;

use crate::{
    analysis::SourceAnalysis, mutability_to_str, slice_source_location, source_string_at_location,
    visibility_to_str, AnalysisResult, USID, UVID,
};
use eyre::Result;
use foundry_compilers::artifacts::{ast::SourceLocation, StateMutability, Visibility};

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
            self.modifications.range(..loc).next_back()
        {
            assert!(
                immediate_prev_loc + immediate_prev.len() <= loc,
                "modification location overlaps with previous modification"
            );
        }
        // Check if the modification overlaps with the next modification
        if let Some((immediate_next_loc, _immediate_next)) = self.modifications.range(loc..).next()
        {
            assert!(
                loc + modification.len() <= *immediate_next_loc,
                "modification location overlaps with next modification"
            );
        }
        // Insert the modification
        self.modifications.insert(loc, modification);
    }

    /// Modifies the source code with the modifications.
    pub fn modify_source(&self, source: &str) -> String {
        let mut modified_source = source.to_string();
        // Apply the modifications in reverse order to avoid index shifting
        for (_, modification) in self.modifications.iter().rev() {
            match modification {
                Modification::Instrument(instrument_action) => {
                    modified_source
                        .insert_str(instrument_action.loc, instrument_action.content.as_str());
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
            Self::Instrument(instrument_action) => instrument_action.file_id,
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
    #[allow(clippy::len_without_is_empty)]
    pub const fn len(&self) -> usize {
        match self {
            Self::Instrument(_) => 0,
            Self::Remove(remove_action) => {
                remove_action.src.length.expect("remove action length not found")
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct InstrumentAction {
    /// The file ID of the source file to instrument
    pub file_id: u32,
    /// The location of the code to instrument. This is the offset of the code at which the instrumented code should be inserted.
    pub loc: usize,
    /// The code to instrument
    pub content: InstrumentContent,
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
    BeforeStepHook(USID),
    /// A `variable_update` hook. The debugger will record the value of the variable when it is updated.
    VariableUpdateHook(UVID),
}

impl InstrumentContent {
    /// Gets the string representation of the instrument content.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Plain(content) => content,
            Self::BeforeStepHook(_sid) => todo!(),
            Self::VariableUpdateHook(_vid) => todo!(),
        }
    }
}

impl SourceModifications {
    /// Collect the modifications on the source code given the analysis result.
    pub fn collect_modifications(&mut self, source: &str, analysis: &SourceAnalysis) -> Result<()> {
        // Collect the modifications on the visibility and mutability of state variables and functions
        self.collect_visibility_and_mutability_modifications(source, analysis)?;

        // TODO: collect modifications to patch single-statement if/for/while/try/catch/etc.

        // TODO: collect more modifications for each step.

        Ok(())
    }

    /// Collect the modifications on the visibility of state variables and functions given the source analysis result.
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
                    file_id: source_id,
                    loc: src.start.unwrap_or(0) + at_index,
                    content: InstrumentContent::Plain(new_visibility_str),
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
}

#[cfg(test)]
mod tests {
    use crate::{analysis, compile_contract_source_to_source_unit};

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
        analysis.pretty_display(&_sources);

        let mut modifications = SourceModifications::new(analysis::tests::TEST_CONTRACT_SOURCE_ID);
        modifications.collect_visibility_and_mutability_modifications(source, &analysis).unwrap();
        modifications.modify_source(source);

        println!("{}", modifications.modify_source(source));
    }
}
