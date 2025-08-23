use crate::{
    analysis::SourceAnalysis, slice_source_location, source_string_at_location, AnalysisResult,
    USID, UVID,
};
use eyre::Result;
use foundry_compilers::artifacts::{ast::SourceLocation, StateMutability, Visibility};

#[derive(Debug, Clone, derive_more::From)]
pub enum Modification {
    Instrument(#[from] InstrumentAction),
    Remove(#[from] RemoveAction),
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

#[derive(Debug, Clone)]
pub enum InstrumentContent {
    /// The code to instrument. The plain code can be directly inserted into the source code as a string.
    Plain(String),
    /// A `before_step` hook. The debugger will pause here during step-by-step execution.
    BeforeStepHook(USID),
    /// A `variable_update` hook. The debugger will record the value of the variable when it is updated.
    VariableUpdateHook(UVID),
}

/// Collect the modifications on the source code given the analysis result.
pub fn collect_modifications(
    source_id: u32,
    source: &str,
    analysis: &SourceAnalysis,
) -> Result<Vec<Modification>> {
    let mut modifications = Vec::new();

    // Collect the modifications on the visibility and mutability of state variables and functions
    modifications
        .extend(collect_visibility_and_mutability_modifications(source_id, source, analysis)?);

    // TODO: collect modifications to patch single-statement if/for/while/try/catch/etc.

    // TODO: collect more modifications for each step.

    // Collect the modifications on the function calls
    Ok(vec![])
}

/// Collect the modifications on the visibility of state variables and functions given the source analysis result.
fn collect_visibility_and_mutability_modifications(
    source_id: u32,
    source: &str,
    analysis: &SourceAnalysis,
) -> Result<Vec<Modification>> {
    let mut modifications = Vec::new();

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

    let add_visibility = |src: SourceLocation, add: &str, at_index: usize| -> InstrumentAction {
        let new_visibility_str = format!(" {add} ");
        let instrument_action = InstrumentAction {
            file_id: source_id,
            loc: src.start.unwrap_or(0) + at_index,
            content: InstrumentContent::Plain(new_visibility_str),
        };
        instrument_action
    };

    for private_state_variable in &analysis.private_state_variables {
        let declaration_str =
            source_string_at_location(source_id, source, &private_state_variable.src);
        // Remove the existing visibility of the state variable
        if let Some(remove_action) = remove_visibility(
            declaration_str,
            private_state_variable.src,
            &serde_json::to_string(&private_state_variable.visibility)
                .expect("failed to serialize visibility"),
        ) {
            modifications.push(remove_action.into());
        }

        // Add the new visibility of the state variable before the variable name, i.e., public
        let at_index = declaration_str
            .find(&private_state_variable.name)
            .unwrap_or_else(|| panic!("{}", "variable name not found in declaration".to_string()));
        let instrument_action = add_visibility(
            private_state_variable.src,
            &serde_json::to_string(&Visibility::Public).expect("failed to serialize visibility"),
            at_index,
        );
        modifications.push(instrument_action.into());
    }

    for private_function in &analysis.private_functions {
        let definition_str = source_string_at_location(source_id, source, &private_function.src);
        // Remove the existing visibility of the function
        if let Some(remove_action) = remove_visibility(
            definition_str,
            private_function.src,
            &serde_json::to_string(&private_function.visibility)
                .expect("failed to serialize visibility"),
        ) {
            modifications.push(remove_action.into());
        }

        // Add the new visibility of the function before the function name, i.e., public
        let at_index = definition_str.find(")").expect("function parament list not found") + 1;
        let instrument_action = add_visibility(
            private_function.src,
            &serde_json::to_string(&Visibility::Public).expect("failed to serialize visibility"),
            at_index,
        );
        modifications.push(instrument_action.into());
    }

    for immutable_function in &analysis.immutable_functions {
        let definition_str = source_string_at_location(source_id, source, &immutable_function.src);
        if let Some(mutability) = &immutable_function.state_mutability {
            // Remove the existing pure or view mutability of the function
            if matches!(mutability, StateMutability::Pure | StateMutability::View) {
                if let Some(remove_action) = remove_visibility(
                    definition_str,
                    immutable_function.src,
                    &serde_json::to_string(mutability).expect("failed to serialize mutability"),
                ) {
                    modifications.push(remove_action.into());
                }
            }
        }
    }

    Ok(modifications)
}
