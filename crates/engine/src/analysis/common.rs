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

//! Common analysis types and functionality for the Ethereum Debug Bridge (EDB) engine.
//!
//! This module provides the core data structures and analysis logic for processing Solidity source code
//! and extracting debugging information. It handles compilation, AST parsing, and step-by-step analysis
//! of smart contract execution paths.
//!
//! # Overview
//!
//! The analysis module performs the following key operations:
//! 1. **Compilation**: Compiles Solidity source code using the Solc compiler
//! 2. **AST Processing**: Parses and prunes the Abstract Syntax Tree (AST) for analysis
//! 3. **Step Partitioning**: Divides the source code into executable steps
//! 4. **Hook Analysis**: Identifies pre and post-execution hooks for each step
//! 5. **Variable Tracking**: Maps variables to their execution contexts
//!
//! # Key Components
//!
//! - [`AnalysisResult`]: The main result container holding all analysis data
//! - [`SourceResult`]: Per-source file analysis results
//! - [`StepAnalysisResult`]: Individual step analysis with hooks
//! - [`analyze()`]: Main analysis function that orchestrates the entire process
//!
//! # Example Usage
//!
//! ```rust
//! use foundry_compilers::solc::SolcVersionedInput;
//! use crate::analysis::common::analyze;
//!
//! // Create a SolcVersionedInput with your Solidity source
//! let input = /* your solc input */;
//!
//! // Run the analysis
//! let result = analyze(input)?;
//!
//! // Access analysis results
//! for (path, source_result) in &result.sources {
//!     println!("Analyzed: {}", path.display());
//!     for step in &source_result.steps {
//!         println!("Step: {}", step.source_step.variant_name());
//!     }
//! }
//! ```

use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::warn;

use crate::{
    analysis::{AnalysisError, Analyzer, FunctionRef, SourceAnalysis, StepRef, UFID},
    ASTPruner, Artifact, VariableRef, USID, UVID,
};

/// Main analysis result containing debugging information from source code analysis.
///
/// This structure holds the complete analysis results for all source files processed
/// during the analysis phase. It provides mappings from unique identifiers to their
/// corresponding source steps and variables for efficient lookup during debugging.
///
/// # Fields
///
/// - `sources`: Maps file paths to their individual analysis results
/// - `usid_to_step`: Maps unique step identifiers (USID) to their source step references
/// - `uvid_to_variable`: Maps unique variable identifiers (UVID) to their variable references
///
/// # Example
///
/// ```rust
/// let analysis_result = analyze(input)?;
///
/// // Access source results by file path
/// for (path, source_result) in &analysis_result.sources {
///     println!("File: {}", path.display());
///     println!("Steps found: {}", source_result.steps.len());
/// }
///
/// // Look up a specific step by USID
/// if let Some(step) = analysis_result.usid_to_step.get(&some_usid) {
///     println!("Found step: {}", step.variant_name());
/// }
/// ```
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    /// Maps source index to their corresponding source analysis results
    pub sources: HashMap<u32, SourceAnalysis>,
    /// Maps unique function identifiers to their function references
    pub ufid_to_function: HashMap<UFID, FunctionRef>,
    /// Maps unique step identifiers to their source step references for quick lookup
    pub usid_to_step: HashMap<USID, StepRef>,
    /// Maps unique variable identifiers to their variable references (currently unimplemented)
    pub uvid_to_variable: HashMap<UVID, VariableRef>,
}

/// Performs comprehensive analysis of Solidity source code.
///
/// This is the main entry point for source code analysis. It compiles the provided
/// Solidity input, processes the AST, and performs step-by-step analysis to extract
/// debugging information.
///
/// # Arguments
///
/// * `input` - The Solc versioned input containing source files and compilation settings
///
/// # Returns
///
/// Returns an `AnalysisResult` containing all analysis data, or an `AnalysisError`
/// if compilation or analysis fails.
///
/// # Process Overview
///
/// 1. **Compilation**: Uses the Solc compiler to compile the source code
/// 2. **AST Processing**: Parses and prunes the AST for each source file
/// 3. **Parallel Analysis**: Analyzes each source file in parallel
/// 4. **Step Partitioning**: Divides source code into executable steps
/// 5. **Hook Generation**: Creates pre and post-execution hooks for each step
/// 6. **Index Building**: Builds lookup tables for steps and variables
///
/// # Example
///
/// ```rust
/// use foundry_compilers::solc::SolcVersionedInput;
/// use crate::analysis::common::analyze;
///
/// // Create your SolcVersionedInput
/// let input = SolcVersionedInput::build(/* ... */);
///
/// // Run the analysis
/// match analyze(input) {
///     Ok(result) => {
///         println!("Analysis completed successfully");
///         println!("Files analyzed: {}", result.sources.len());
///         println!("Total steps: {}", result.usid_to_step.len());
///     }
///     Err(e) => eprintln!("Analysis failed: {}", e),
/// }
/// ```
///
/// # Errors
///
/// This function can return the following errors:
/// - `AnalysisError::StepPartitionError`: When step partitioning fails
pub fn analyze(artifact: &Artifact) -> Result<AnalysisResult, AnalysisError> {
    let source_results: Vec<SourceAnalysis> = artifact
        .output
        .sources
        .par_iter()
        .map(|(path, source_result)| {
            let source_id = source_result.id;
            let mut source_ast = source_result.ast.clone().ok_or(AnalysisError::MissingAst)?;
            let source_unit = ASTPruner::convert(&mut source_ast, false)
                .map_err(AnalysisError::ASTConversionError)?;

            let analyzer = Analyzer::new();
            let mut source_result = analyzer.analyze(source_id, path, &source_unit)?;

            // sort steps in reverse order
            source_result.steps.sort_unstable_by_key(|step| step.read().src.start);
            source_result.steps.reverse();

            // ensure we do not have overlapped steps
            // XXX (ZZ): the check failed for the following command, need to investigate
            //  ./target/debug/edb replay 0xd253e3b563bf7b8894da2a69db836a4e98e337157564483d8ac72117df355a9d
            //  overlap happens on USDC (0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48)
            for (i, step) in source_result.steps.iter().enumerate() {
                if i > 0 {
                    let prev = &source_result.steps[i - 1];
                    let end = step
                        .read()
                        .src
                        .start
                        .map(|start| start + step.read().src.length.unwrap_or(0));

                    if end > prev.read().src.start {
                        warn!("Overlapping steps detected: {:?}", step);
                        // return Err(AnalysisError::StepPartitionError(eyre::eyre!(
                        //     "Overlapping steps detected"
                        // )));
                    }
                }
            }

            Ok(source_result)
        })
        .collect::<Result<Vec<_>, AnalysisError>>()?;

    // build lookup tables
    let mut ufid_to_function = HashMap::new();
    let mut usid_to_step = HashMap::new();
    let mut uvid_to_variable = HashMap::new();
    for result in source_results.iter() {
        ufid_to_function.extend(result.function_table().into_iter());
        usid_to_step.extend(result.step_table().into_iter());
        uvid_to_variable.extend(result.variable_table().into_iter());
    }
    let sources = source_results.into_iter().map(|s| (s.id, s)).collect();

    Ok(AnalysisResult { sources, ufid_to_function, usid_to_step, uvid_to_variable })
}

#[cfg(test)]
mod tests {
    use super::*;
    use foundry_block_explorers::contract::Metadata;
    use foundry_compilers::{
        artifacts::{
            output_selection::OutputSelection, EvmVersion, Settings, SolcInput, Source, Sources,
        },
        solc::{Solc, SolcLanguage},
    };
    use std::path::PathBuf;

    #[test]
    #[allow(clippy::field_reassign_with_default)]
    fn test_analyze_contract_with_three_statements() {
        // Create a simple Solidity contract with three statements
        let contract_source = r#"
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract SimpleContract {
    uint256 public value;

    function setValue(uint256 newValue) public {
        value = newValue;           // Statement 1: Assignment
        emit ValueSet(newValue);    // Statement 2: Event emission
        value = value + 1;          // Statement 3: Increment
    }

    event ValueSet(uint256 value);
}
"#;

        // Create the solc input using the correct types
        let file_path = PathBuf::from("SimpleContract.sol");
        let sources =
            Sources::from_iter([(file_path.clone(), Source::new(contract_source.to_string()))]);

        let mut settings = Settings::default();
        settings.output_selection = OutputSelection::complete_output_selection();
        // Set a valid EVM version for Solidity 0.8.19
        settings.evm_version = Some(EvmVersion::Paris);

        let input = SolcInput::new(SolcLanguage::Solidity, sources, settings);

        // Compile using Solc
        let version = semver::Version::new(0, 8, 19);
        let compiler = Solc::find_or_install(&version).expect("Failed to find or install Solc");
        let output = compiler.compile_exact(&input).expect("Compilation failed");
        println!("input {input:?}");
        println!("output {output:?}");

        // Create fake metadata for testing
        // Note: SourceCodeMetadata doesn't have Default impl, so we need to create it manually
        let source_code_meta = foundry_block_explorers::contract::SourceCodeMetadata::SourceCode(
            contract_source.to_string(),
        );

        let meta = Metadata {
            source_code: source_code_meta,
            abi: String::new(),
            contract_name: "SimpleContract".to_string(),
            compiler_version: "0.8.19".to_string(),
            optimization_used: 200,
            runs: 200,
            constructor_arguments: Default::default(),
            evm_version: "paris".to_string(),
            library: String::new(),
            license_type: String::new(),
            proxy: 0,
            implementation: None,
            swarm_source: String::new(),
        };

        let artifact = Artifact { meta, input, output };

        // Run the analysis
        let result = analyze(&artifact).expect("Analysis should succeed");

        // Verify the analysis result
        assert!(!result.sources.is_empty(), "Should have analyzed at least one source file");

        let source_result = result.sources.get(&0).expect("Should find the source file");
        assert_eq!(source_result.path, file_path);
        assert!(!source_result.steps.is_empty(), "Should have analyzed steps in the contract");

        // Verify that we have analysis results for the function
        // Since we can't directly access source content from SourceStep, we'll check the number of steps
        let step_count = source_result.steps.len();
        assert!(step_count > 0, "Should have found steps in the contract");

        // Verify that the steps have hooks
        for step in &source_result.steps {
            assert!(!step.read().pre_hooks.is_empty(), "Each step should have pre-hooks");
            // println!("{}\n", step.pretty_display(&source_result.source));
        }
    }
}
