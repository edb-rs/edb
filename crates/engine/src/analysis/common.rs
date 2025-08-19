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

use foundry_compilers::artifacts::{Ast, Source, SourceUnit};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use tracing::warn;
use std::{collections::HashMap, path::PathBuf};
use thiserror::Error;

use crate::{
    utils::ASTPruner, Artifact, SourceStepRef, SourceSteps, StepHook, VariableRef, USID, UVID,
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
#[derive(Debug, Default)]
pub struct AnalysisResult {
    /// Maps file paths to their corresponding source analysis results
    pub sources: HashMap<PathBuf, SourceResult>,
    /// Maps unique step identifiers to their source step references for quick lookup
    pub usid_to_step: HashMap<USID, SourceStepRef>,
    /// Maps unique variable identifiers to their variable references (currently unimplemented)
    pub uvid_to_variable: HashMap<UVID, VariableRef>,
}

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
#[derive(Debug)]
pub struct SourceResult {
    /// Unique identifier for this source file
    pub id: u32,
    /// File system path to the source file
    pub path: PathBuf,
    /// Original source content and metadata
    pub source: Source,
    /// Parsed Abstract Syntax Tree
    pub ast: Ast,
    /// Processed source unit ready for analysis
    pub unit: SourceUnit,
    /// List of analyzed execution steps in this file
    pub steps: Vec<StepAnalysisResult>,
}

/// Analysis result for a single execution step.
///
/// Represents the analysis of one executable step in the source code, including
/// all pre and post-execution hooks that are triggered when this step is executed.
///
/// # Fields
///
/// - `source_step`: Reference to the source step being analyzed
/// - `pre_hooks`: Hooks that execute before this step
/// - `post_hooks`: Hooks that execute after this step
///
/// # Example
///
/// ```rust
/// let step_result = &source_result.steps[0];
/// println!("Step type: {}", step_result.source_step.variant_name());
/// println!("Pre-hooks: {}", step_result.pre_hooks.len());
/// println!("Post-hooks: {}", step_result.post_hooks.len());
///
/// // Pretty print the step analysis
/// println!("{}", step_result.pretty_display(&source_result.source));
/// ```
#[derive(Debug)]
pub struct StepAnalysisResult {
    /// Reference to the source step being analyzed
    pub source_step: SourceStepRef,
    /// Hooks that execute before this step
    pub pre_hooks: Vec<StepHook>,
    /// Hooks that execute after this step
    pub post_hooks: Vec<StepHook>,
}

impl StepAnalysisResult {
    /// Generates a human-readable representation of this step analysis.
    ///
    /// This method creates a formatted string containing:
    /// - The step type and variant name
    /// - Source code location (line numbers and character ranges)
    /// - The actual source code snippet for this step
    /// - All pre-hooks with their details
    /// - All post-hooks with their details
    ///
    /// # Arguments
    ///
    /// * `source` - The source file containing this step
    ///
    /// # Returns
    ///
    /// A formatted string representation of the step analysis
    ///
    /// # Example
    ///
    /// ```rust
    /// let display = step_result.pretty_display(&source_result.source);
    /// println!("{}", display);
    /// ```
    ///
    /// # Output Format
    ///
    /// ```
    /// Step Type: Assignment
    /// Location: 10:100-120
    /// Code: "value = newValue;"
    /// Pre-hooks:
    ///   1: BeforeStep("value = newValue;")
    ///   2: VariableInScope("value")
    /// Post-hooks:
    ///   1: VariableUpdate("value")
    /// ```
    pub fn pretty_display(&self, source: &Source) -> String {
        let mut output = String::new();

        // Get the source content
        let source_content = &source.content;

        // Get the source location from the step
        let location = &self.source_step.source_location;

        // Get the step type
        let step_type = self.source_step.variant_name();

        output.push_str(&format!("Step Type: {step_type}\n"));

        // Extract and display the source code snippet
        if let (Some(start), Some(length)) = (location.start, location.length) {
            let end = start + length;
            if end <= source_content.len() {
                let snippet = &source_content[start..end];
                let cleaned_snippet = snippet.trim();
                if !cleaned_snippet.is_empty() {
                    output.push_str(&format!(
                        "Location: {}:{}-{}\n",
                        location.index.unwrap_or(0),
                        start,
                        end
                    ));
                    output.push_str(&format!("Code: \"{cleaned_snippet}\"\n"));
                }
            }
        }

        // Display pre-hooks
        if !self.pre_hooks.is_empty() {
            output.push_str("Pre-hooks:\n");
            for (i, hook) in self.pre_hooks.iter().enumerate() {
                let hook_display = match hook {
                    StepHook::BeforeStep(step) => {
                        // For BeforeStep, show the source code snippet
                        let step_location = &step.source_location;

                        if let (Some(start), Some(length)) =
                            (step_location.start, step_location.length)
                        {
                            let end = start + length;
                            if end <= source_content.len() {
                                let snippet = &source_content[start..end];
                                let cleaned_snippet = snippet.trim();
                                if !cleaned_snippet.is_empty() {
                                    format!(
                                        "  {}: {}(\"{}\")",
                                        i + 1,
                                        hook.variant_name(),
                                        cleaned_snippet
                                    )
                                } else {
                                    format!("  {}: {}", i + 1, hook.variant_name())
                                }
                            } else {
                                format!("  {}: {}", i + 1, hook.variant_name())
                            }
                        } else {
                            format!("  {}: {}", i + 1, hook.variant_name())
                        }
                    }
                    StepHook::VariableInScope(var) => {
                        format!("  {}: {}(\"{}\")", i + 1, hook.variant_name(), var.name)
                    }
                    StepHook::VariableOutOfScope(var) => {
                        format!("  {}: {}(\"{}\")", i + 1, hook.variant_name(), var.name)
                    }
                    StepHook::VariableUpdate(var) => {
                        format!("  {}: {}(\"{}\")", i + 1, hook.variant_name(), var.name)
                    }
                };
                output.push_str(&hook_display);
                output.push('\n');
            }
        }

        // Display post-hooks
        if !self.post_hooks.is_empty() {
            output.push_str("Post-hooks:\n");
            for (i, hook) in self.post_hooks.iter().enumerate() {
                let hook_display = match hook {
                    StepHook::BeforeStep(step) => {
                        // For BeforeStep, show the source code snippet
                        let step_location = &step.source_location;

                        if let (Some(start), Some(length)) =
                            (step_location.start, step_location.length)
                        {
                            let end = start + length;
                            if end <= source_content.len() {
                                let snippet = &source_content[start..end];
                                let cleaned_snippet = snippet.trim();
                                if !cleaned_snippet.is_empty() {
                                    format!(
                                        "  {}: {}(\"{}\")",
                                        i + 1,
                                        hook.variant_name(),
                                        cleaned_snippet
                                    )
                                } else {
                                    format!("  {}: {}", i + 1, hook.variant_name())
                                }
                            } else {
                                format!("  {}: {}", i + 1, hook.variant_name())
                            }
                        } else {
                            format!("  {}: {}", i + 1, hook.variant_name())
                        }
                    }
                    StepHook::VariableInScope(var) => {
                        format!("  {}: {}(\"{}\")", i + 1, hook.variant_name(), var.name)
                    }
                    StepHook::VariableOutOfScope(var) => {
                        format!("  {}: {}(\"{}\")", i + 1, hook.variant_name(), var.name)
                    }
                    StepHook::VariableUpdate(var) => {
                        format!("  {}: {}(\"{}\")", i + 1, hook.variant_name(), var.name)
                    }
                };
                output.push_str(&hook_display);
                output.push('\n');
            }
        }

        output
    }
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
    // parse ast of each file into SourceUnit
    // TODO: can we avoid cloning?
    let source_results = artifact
        .output
        .sources
        .par_iter()
        .map(|(path, source)| {
            let mut ast = source.ast.clone().expect("AST is not selected as output");
            let unit = ASTPruner::convert(&mut ast, false).expect("Failed to convert AST");

            // do analysis
            let mut steps = analyze_source(&unit)?;

            // sort steps in reverse order
            steps.sort_unstable_by_key(|step| step.source_step.source_location.start);
            steps.reverse();

            // ensure we do not have overlapped steps
            // XXX (ZZ): the check failed for the following command, need to investigate
            //  ./target/debug/edb replay 0xd253e3b563bf7b8894da2a69db836a4e98e337157564483d8ac72117df355a9d
            //  overlap happens on USDC (0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48)
            for (i, step) in steps.iter().enumerate() {
                if i > 0 {
                    let prev = &steps[i - 1];
                    let end =
                        step.source_step.source_location.start.map(|start| {
                            start + step.source_step.source_location.length.unwrap_or(0)
                        });

                    if end > prev.source_step.source_location.start {
                        warn!("Overlapping steps detected: {:?}", step);
                        // return Err(AnalysisError::StepPartitionError(eyre::eyre!(
                        //     "Overlapping steps detected"
                        // )));
                    }
                }
            }

            let result = SourceResult {
                id: source.id,
                path: path.clone(),
                source: artifact.input.sources.get(path).expect("Source not found").clone(),
                ast,
                unit,
                steps,
            };

            Ok((path.clone(), result))
        })
        .collect::<Result<HashMap<_, _>, AnalysisError>>()?;

    let mut usid_to_step = HashMap::new();
    let uvid_to_variable = HashMap::new(); // TODO: implement
    for (_, result) in source_results.iter() {
        for step in result.steps.iter() {
            usid_to_step.insert(step.source_step.usid, step.source_step.clone());
        }
    }

    Ok(AnalysisResult { sources: source_results, usid_to_step, uvid_to_variable })
}

/// Analyzes a single source unit to extract step information.
///
/// This function processes a `SourceUnit` (representing a single Solidity file)
/// and extracts all executable steps along with their associated hooks.
///
/// # Arguments
///
/// * `unit` - The source unit to analyze
///
/// # Returns
///
/// Returns a vector of `StepAnalysisResult` containing all steps found in the source unit,
/// or an `AnalysisError` if analysis fails.
///
/// # Current Implementation
///
/// The current implementation is a mock that:
/// 1. Partitions the source unit into steps using `SourceSteps::partition_from`
/// 2. Creates a `StepAnalysisResult` for each step
/// 3. Adds a `BeforeStep` hook to each step's pre-hooks
/// 4. Leaves post-hooks empty (to be implemented)
///
/// # Future Enhancements
///
/// This function will be enhanced to:
/// - Analyze variable scoping and create appropriate variable hooks
/// - Identify control flow changes and create corresponding hooks
/// - Analyze function calls and their side effects
/// - Track state changes and create update hooks
fn analyze_source(unit: &SourceUnit) -> Result<Vec<StepAnalysisResult>, AnalysisError> {
    // mock a step analysis result
    let steps = SourceSteps::partition_from(unit)?;
    let step_analysis_results = steps
        .into_steps()
        .into_iter()
        .map(SourceStepRef::new)
        .map(|step| StepAnalysisResult {
            pre_hooks: vec![StepHook::BeforeStep(step.clone())],
            source_step: step,
            post_hooks: vec![],
        })
        .collect();

    Ok(step_analysis_results)
}

/// Errors that can occur during source code analysis.
///
/// This enum represents all possible error conditions that can arise during
/// the analysis process, from compilation failures to step partitioning errors.
#[derive(Debug, Error)]
pub enum AnalysisError {
    /// Error during step partitioning of source code
    #[error("failed to partition source steps: {0}")]
    StepPartitionError(#[from] eyre::Report),
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
        println!("input {:?}", input);
        println!("output {:?}", output);

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

        let source_result = result.sources.get(&file_path).expect("Should find the source file");
        assert_eq!(source_result.path, file_path);
        assert!(!source_result.steps.is_empty(), "Should have analyzed steps in the contract");

        // Verify that we have analysis results for the function
        // Since we can't directly access source content from SourceStep, we'll check the number of steps
        let step_count = source_result.steps.len();
        assert!(step_count > 0, "Should have found steps in the contract");

        // Verify that the steps have hooks
        for step in &source_result.steps {
            assert!(!step.pre_hooks.is_empty(), "Each step should have pre-hooks");
            println!("{}\n", step.pretty_display(&source_result.source));
        }
    }
}
