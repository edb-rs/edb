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

//! Common utilities for instrumentation

use std::path::PathBuf;

use crate::{AnalysisResult, SourceAnalysis, SourceModifications, StepRef};
use eyre::Result;
use foundry_compilers::artifacts::{SolcInput, Source};

pub fn instrument(input: &SolcInput, analysis: &AnalysisResult) -> Result<SolcInput> {
    let mut instrumented_input = input.clone();
    for (source_id, analysis_data) in &analysis.sources {
        let source_path = analysis_data.path.clone();
        let source = instrumented_input.sources.get(&source_path).ok_or(eyre::eyre!(
            "Source code for path {:?} not found in input sources",
            source_path
        ))?;

        let mut modifications = SourceModifications::new(*source_id);
        modifications.collect_modifications(&source.content, analysis_data)?;

        let instrumented_source = Source::new(modifications.modify_source(&source.content));
        instrumented_input.sources.insert(source_path, instrumented_source);
    }

    Ok(instrumented_input)
}

#[deprecated]
fn instrument_inner(
    path: &PathBuf,
    source: &Source,
    analysis_result: &SourceAnalysis,
) -> Result<Source> {
    let mut source_text = source.content.to_string();

    for step_result in &analysis_result.steps {
        // We first handle post hooks
        let post_hooks = &step_result.read().post_hooks;
        let pre_hooks = &step_result.read().pre_hooks;
        let hooks = post_hooks.iter().chain(pre_hooks.iter());
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

#[deprecated]
fn instrument_before_step(source_text: &mut String, step_result: &StepRef) {
    let checkpoint_call = format!(
        "address(0x0000000000000000000000000000000000023333).staticcall(abi.encode({}));\n",
        step_result.read().usid,
    );

    let start = step_result.read().src.start.unwrap_or(0);
    source_text.insert_str(start, checkpoint_call.as_str());
}

#[deprecated]
fn instrument_variable_in_scope(source_text: &mut String, step_result: &StepRef) {
    // TODO (ZZ): Implement variable in scope instrumentation
}

#[deprecated]
fn instrument_variable_out_of_scope(source_text: &mut String, step_result: &StepRef) {
    // TODO (ZZ): Implement variable out of scope instrumentation
}

#[deprecated]
fn instrument_variable_update(source_text: &mut String, step_result: &StepRef) {
    // TODO (ZZ): Implement variable update instrumentation
}
