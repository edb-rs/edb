//! Common utilities for instrumentation

use std::path::PathBuf;

use crate::{AnalysisResult, SourceResult, StepAnalysisResult};
use eyre::Result;
use foundry_compilers::artifacts::{SolcInput, Source};

pub fn instrument(input: &SolcInput, analysis: &AnalysisResult) -> Result<SolcInput> {
    let mut instrumented_input = input.clone();
    for (_, analysis_data) in &analysis.sources {
        let source = instrumented_input.sources.get(&analysis_data.path).ok_or(eyre::eyre!(
            "Source code for path {:?} not found in input sources",
            analysis_data.path
        ))?;

        let instrumented_source = instrument_inner(&analysis_data.path, source, analysis_data)?;
        instrumented_input.sources.insert(analysis_data.path.clone(), instrumented_source);
    }

    Ok(instrumented_input)
}

fn instrument_inner(
    path: &PathBuf,
    source: &Source,
    analysis_result: &SourceResult,
) -> Result<Source> {
    let mut source_text = source.content.to_string();

    for step_result in &analysis_result.steps {
        // We first handle post hooks
        let hooks = step_result.post_hooks.iter().chain(step_result.pre_hooks.iter());
        for hook in hooks {
            match hook {
                crate::StepHook::BeforeStep(_) => {
                    instrument_before_step(&mut source_text, step_result);
                }
                crate::StepHook::VariableInScope(_) => {
                    instrument_variable_in_scope(&mut source_text, step_result);
                }
                crate::StepHook::VariableOutOfScope(_) => {
                    instrument_variable_out_of_scope(&mut source_text, step_result);
                }
                crate::StepHook::VariableUpdate(_) => {
                    instrument_variable_update(&mut source_text, step_result);
                }
            }
        }
    }

    Ok(Source::new(source_text))
}

fn instrument_before_step(source_text: &mut String, step_result: &StepAnalysisResult) {
    let checkpoint_call = format!(
        "address(0x0000000000000000000000000000000000023333).staticcall(abi.encode({}));\n",
        step_result.source_step.usid,
    );

    let start = step_result.source_step.source_location.start.unwrap_or(0);
    source_text.insert_str(start, checkpoint_call.as_str());
}

fn instrument_variable_in_scope(source_text: &mut String, step_result: &StepAnalysisResult) {
    // TODO (ZZ): Implement variable in scope instrumentation
}

fn instrument_variable_out_of_scope(source_text: &mut String, step_result: &StepAnalysisResult) {
    // TODO (ZZ): Implement variable out of scope instrumentation
}

fn instrument_variable_update(source_text: &mut String, step_result: &StepAnalysisResult) {
    // TODO (ZZ): Implement variable update instrumentation
}
