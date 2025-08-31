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

        let modified_source = modifications.modify_source(&source.content);
        let instrumented_source = Source::new(modified_source);
        instrumented_input.sources.insert(source_path, instrumented_source);
    }

    Ok(instrumented_input)
}
