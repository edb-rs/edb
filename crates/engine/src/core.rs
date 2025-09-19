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

//! Core engine functionality for transaction analysis and debugging.
//!
//! This module provides the main engine implementation that orchestrates the complete
//! debugging workflow for Ethereum transactions. It handles source code analysis,
//! contract instrumentation, transaction execution with debugging inspectors,
//! and RPC server management.
//!
//! # Workflow Overview
//!
//! 1. **Preparation**: Accept forked database and transaction configuration
//! 2. **Analysis**: Download and analyze contract source code
//! 3. **Instrumentation**: Inject debugging hooks into contract bytecode
//! 4. **Execution**: Replay transaction with comprehensive debugging inspectors
//! 5. **Collection**: Gather execution snapshots and trace data
//! 6. **API**: Start RPC server for debugging interface
//!
//! # Key Components
//!
//! - [`EngineConfig`] - Engine configuration and settings
//! - [`run_transaction_analysis`] - Main analysis workflow function
//! - Inspector coordination for comprehensive data collection
//! - Source code fetching and compilation management
//! - Snapshot generation and organization
//!
//! # Supported Features
//!
//! - **Multi-contract analysis**: Analyze all contracts involved in execution
//! - **Source fetching**: Automatic download from Etherscan and verification
//! - **Quick mode**: Fast analysis with reduced operations
//! - **Instrumentation**: Automatic debugging hook injection
//! - **Comprehensive inspection**: Opcode and source-level snapshot collection

use alloy_primitives::{Address, Bytes, TxHash};
use eyre::Result;
use foundry_block_explorers::Client;
use foundry_compilers::{
    artifacts::{Contract, SolcInput},
    solc::Solc,
};
use indicatif::{ProgressBar, ProgressStyle};
use revm::{
    context::{
        result::{ExecutionResult, HaltReason},
        Host, TxEnv,
    },
    database::CacheDB,
    Database, DatabaseCommit, DatabaseRef, InspectEvm, MainBuilder,
};
use semver::Version;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::PathBuf,
    time::Duration,
};
use tracing::{debug, error, info, warn};

use edb_common::{
    relax_evm_constraints, types::Trace, CachePath, EdbCachePath, EdbContext, ForkResult,
    DEFAULT_ETHERSCAN_CACHE_TTL,
};

use crate::{
    analysis::AnalysisResult,
    analyze,
    inspector::{CallTracer, TraceReplayResult},
    instrument,
    rpc::RpcServerHandle,
    start_debug_server,
    utils::{next_etherscan_api_key, Artifact, OnchainCompiler},
    CodeTweaker, EngineContext, HookSnapshotInspector, HookSnapshots, OpcodeSnapshotInspector,
    OpcodeSnapshots, SnapshotAnalysis, Snapshots,
};

/// Configuration for the EDB debugging engine.
///
/// Contains settings that control the engine's behavior during transaction analysis,
/// source code fetching, and debugging operations.
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// RPC provider URL for blockchain interaction (typically a proxy or archive node)
    pub rpc_proxy_url: String,
    /// Optional Etherscan API key for automatic source code downloading and verification
    pub etherscan_api_key: Option<String>,
    /// Quick mode flag - when enabled, skips time-intensive operations for faster analysis
    pub quick: bool,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            rpc_proxy_url: "http://localhost:8545".into(),
            etherscan_api_key: None,
            quick: false,
        }
    }
}

impl EngineConfig {
    /// Set the Etherscan API key for source code download
    pub fn with_etherscan_api_key(mut self, key: String) -> Self {
        self.etherscan_api_key = Some(key);
        self
    }

    /// Enable or disable quick mode for faster analysis
    pub fn with_quick_mode(mut self, quick: bool) -> Self {
        self.quick = quick;
        self
    }

    /// Set the RPC proxy URL for blockchain interactions
    pub fn with_rpc_proxy_url(mut self, url: String) -> Self {
        self.rpc_proxy_url = url;
        self
    }
}

/// The main Engine struct that performs transaction analysis
#[derive(Debug)]
pub struct Engine {
    /// RPC Provider URL
    pub rpc_proxy_url: String,
    /// Port for the JSON-RPC server
    pub host_port: Option<u16>,
    /// Etherscan API key for source code download
    pub etherscan_api_key: Option<String>,
    /// Quick mode - skip certain operations for faster analysis
    pub quick: bool,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new(EngineConfig::default())
    }
}

impl Engine {
    /// Create a new Engine instance from configuration
    pub fn new(config: EngineConfig) -> Self {
        let EngineConfig { rpc_proxy_url, etherscan_api_key, quick } = config;
        Self { rpc_proxy_url, host_port: None, etherscan_api_key, quick }
    }

    /// Main preparation method for the engine
    ///
    /// This method accepts a forked database and EVM configuration prepared by the edb binary.
    /// It focuses on the core debugging workflow:
    /// 1. Replays the target transaction to collect touched contracts
    /// 2. Downloads verified source code for each contract
    /// 3. Analyzes the source code to identify instrumentation points
    /// 4. Instruments and recompiles the source code
    /// 5. Collect opcode-level step execution results
    /// 6. Re-executes the transaction with state snapshots
    /// 7. Starts a JSON-RPC server with the analysis results and snapshots
    pub async fn prepare<DB>(&self, fork_result: ForkResult<DB>) -> Result<RpcServerHandle>
    where
        DB: Database + DatabaseCommit + DatabaseRef + Clone + Send + Sync + 'static,
        <CacheDB<DB> as Database>::Error: Clone + Send + Sync,
        <DB as Database>::Error: Clone + Send + Sync,
    {
        info!("Starting engine preparation for transaction: {:?}", fork_result.target_tx_hash);

        // Step 0: Initialize context and database
        let ForkResult { context: mut ctx, target_tx_env: tx, target_tx_hash: tx_hash, fork_info } =
            fork_result;

        // Step 1: Replay the target transaction to collect call trace and touched contracts
        info!("Replaying transaction to collect call trace and touched contracts");
        let replay_result = self.replay_and_collect_trace(ctx.clone(), tx.clone())?;

        // Step 2: Download verified source code for each contract
        info!("Downloading verified source code for each contract");
        let artifacts =
            self.download_verified_source_code(&replay_result, ctx.chain_id().to::<u64>()).await?;

        // Step 3: Analyze source code to identify instrumentation points
        info!("Analyzing source code");
        let analysis_results = self.analyze_source_code(&artifacts)?;

        // Step 4: Instrument source code
        info!("Instrumenting source code");
        let recompiled_artifacts =
            self.instrument_and_recompile_source_code(&artifacts, &analysis_results)?;

        // Step 5: Collect opcode-level step execution results
        info!("Collecting opcode-level step execution results");
        let opcode_snapshots = self.capture_opcode_level_snapshots(
            ctx.clone(),
            tx.clone(),
            artifacts.keys().cloned().collect(),
            &replay_result.execution_trace,
        )?;

        // Step 6: Replace original bytecode with instrumented versions
        info!("Tweaking bytecode");
        let contracts_in_tx =
            self.tweak_bytecode(&mut ctx, &artifacts, &recompiled_artifacts, tx_hash).await?;

        // Step 7: Re-execute the transaction with snapshot collection
        info!("Re-executing transaction with snapshot collection");
        let hook_creation =
            self.collect_creation_hooks(&artifacts, &recompiled_artifacts, contracts_in_tx)?;
        let hook_snapshots = self.capture_hook_snapshots(
            ctx.clone(),
            tx.clone(),
            hook_creation,
            &replay_result.execution_trace,
            &analysis_results,
        )?;

        // Step 8: Start RPC server with analysis results and snapshots
        info!("Starting RPC server with analysis results and snapshots");
        let mut snapshots = self.get_time_travel_snapshots(opcode_snapshots, hook_snapshots)?;
        snapshots.analyze(&replay_result.execution_trace, &analysis_results)?;
        // Let's pack the debug context
        let context = EngineContext::build(
            fork_info,
            ctx.cfg.clone(),
            ctx.block.clone(),
            tx,
            tx_hash,
            snapshots,
            artifacts,
            recompiled_artifacts,
            analysis_results,
            replay_result.execution_trace,
        )?;

        let rpc_handle = start_debug_server(context).await?;
        info!("Debug RPC server started on port {}", rpc_handle.port());

        Ok(rpc_handle)
    }

    /// Replay the target transaction and collect call trace with all touched addresses
    fn replay_and_collect_trace<DB>(
        &self,
        ctx: EdbContext<DB>,
        tx: TxEnv,
    ) -> Result<TraceReplayResult>
    where
        DB: Database + DatabaseCommit + DatabaseRef + Clone,
        <CacheDB<DB> as Database>::Error: Clone,
        <DB as Database>::Error: Clone,
    {
        info!("Replaying transaction to collect call trace and touched addresses");

        let mut tracer = CallTracer::new();
        let mut evm = ctx.build_mainnet_with_inspector(&mut tracer);

        let result = evm
            .inspect_one_tx(tx)
            .map_err(|e| eyre::eyre!("Failed to inspect the target transaction: {:?}", e))?;

        if let ExecutionResult::Halt { reason, .. } = result {
            if matches!(reason, HaltReason::OutOfGas { .. }) {
                error!("EDB cannot debug out-of-gas errors. Proceed at your own risk.")
            }
        }

        let result = tracer.into_replay_result();

        for (address, deployed) in &result.visited_addresses {
            if *deployed {
                debug!("Contract {} was deployed during transaction replay", address);
            } else {
                debug!("Address {} was touched during transaction replay", address);
            }
        }

        // Print the trace tree structure
        result.execution_trace.print_trace_tree();

        Ok(result)
    }

    /// Download and compile verified source code for each contract
    async fn download_verified_source_code(
        &self,
        replay_result: &TraceReplayResult,
        chain_id: u64,
    ) -> Result<HashMap<Address, Artifact>> {
        info!("Downloading verified source code for touched contracts");

        let compiler = OnchainCompiler::new(None)?;

        let compiler_cache_root =
            EdbCachePath::new(None as Option<PathBuf>).compiler_chain_cache_dir(chain_id);

        // Create fancy progress bar with blockchain-themed styling
        // NOTE: For multi-threaded usage, wrap in Arc<ProgressBar> to share across threads
        // Example: let console_bar = Arc::new(ProgressBar::new(...));
        // Then clone the Arc for each thread: let bar_clone = console_bar.clone();
        let addresses: Vec<_> = replay_result.visited_addresses.keys().copied().collect();
        let total_contracts = addresses.len();

        let console_bar = std::sync::Arc::new(ProgressBar::new(total_contracts as u64));
        console_bar.set_style(
            ProgressStyle::with_template(
                "{spinner:.green} 📜 Downloading & compiling contracts [{bar:40.cyan/blue}] {pos:>3}/{len:3} 🔧 {msg}"
            )?
            .progress_chars("🟩🟦⬜")
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
        );

        let mut artifacts = HashMap::new();
        for (i, address) in addresses.iter().enumerate() {
            let short_addr = &address.to_string()[2..10]; // Skip 0x, take 8 chars
            console_bar.set_message(format!("contract {}: 0x{}...", i + 1, short_addr));

            // We use the default API key if none is provided
            let api_key = self.get_etherscan_api_key();

            let etherscan = Client::builder()
                .with_api_key(api_key)
                .with_cache(
                    compiler_cache_root.clone(),
                    Duration::from_secs(DEFAULT_ETHERSCAN_CACHE_TTL),
                ) // 24 hours
                .chain(chain_id.into())?
                .build()?;

            match compiler.compile(&etherscan, *address).await {
                Ok(Some(artifact)) => {
                    console_bar.set_message(format!("✅ 0x{short_addr}... compiled"));
                    artifacts.insert(*address, artifact);
                }
                Ok(None) => {
                    console_bar.set_message(format!("⚠️  0x{short_addr}... no source"));
                    debug!("No source code available for contract {}", address);
                }
                Err(e) => {
                    console_bar.set_message(format!("❌ 0x{short_addr}... failed"));
                    warn!("Failed to compile contract {}: {:?}", address, e);
                }
            }

            console_bar.inc(1);
        }

        console_bar.finish_with_message(format!(
            "✨ Done! Compiled {} out of {} contracts",
            artifacts.len(),
            total_contracts
        ));

        Ok(artifacts)
    }

    /// Analyze the source code for instrumentation points and variable usage
    fn analyze_source_code(
        &self,
        artifacts: &HashMap<Address, Artifact>,
    ) -> Result<HashMap<Address, AnalysisResult>> {
        info!("Analyzing source code to identify instrumentation points");

        let mut analysis_result = HashMap::new();
        for (address, artifact) in artifacts {
            debug!("Analyzing contract at address: {}", address);
            let analysis = analyze(artifact)?;
            analysis_result.insert(*address, analysis);
        }

        Ok(analysis_result)
    }

    /// Time travel (i.e., snapshotting) at hooks for contracts we have source code
    fn capture_hook_snapshots<'a, DB>(
        &self,
        mut ctx: EdbContext<DB>,
        mut tx: TxEnv,
        creation_hooks: Vec<(&'a Contract, &'a Contract, &'a Bytes)>,
        trace: &Trace,
        analysis_results: &HashMap<Address, AnalysisResult>,
    ) -> Result<HookSnapshots<DB>>
    where
        DB: Database + DatabaseCommit + DatabaseRef + Clone,
        <CacheDB<DB> as Database>::Error: Clone,
        <DB as Database>::Error: Clone,
    {
        // We need to relax execution constraints for hook snapshots
        relax_evm_constraints(&mut ctx, &mut tx);

        info!("Collecting hook snapshots for source code contracts");

        let mut inspector = HookSnapshotInspector::new(trace, analysis_results);
        inspector.with_creation_hooks(creation_hooks)?;
        let mut evm = ctx.build_mainnet_with_inspector(&mut inspector);

        evm.inspect_one_tx(tx)
            .map_err(|e| eyre::eyre!("Failed to inspect the target transaction: {:?}", e))?;

        let snapshots = inspector.into_snapshots();

        snapshots.print_summary();

        Ok(snapshots)
    }

    /// Time travel (i.e., snapshotting) at the opcode level for contracts we do not
    /// have source code.
    fn capture_opcode_level_snapshots<DB>(
        &self,
        ctx: EdbContext<DB>,
        tx: TxEnv,
        excluded_addresses: HashSet<Address>,
        trace: &Trace,
    ) -> Result<OpcodeSnapshots<DB>>
    where
        DB: Database + DatabaseCommit + DatabaseRef + Clone,
        <CacheDB<DB> as Database>::Error: Clone,
        <DB as Database>::Error: Clone,
    {
        info!("Collecting opcode-level step execution results");

        let mut inspector = OpcodeSnapshotInspector::new(&ctx, trace);
        inspector.with_excluded_addresses(excluded_addresses);
        let mut evm = ctx.build_mainnet_with_inspector(&mut inspector);

        evm.inspect_one_tx(tx)
            .map_err(|e| eyre::eyre!("Failed to inspect the target transaction: {:?}", e))?;

        let snapshots = inspector.into_snapshots();

        snapshots.print_summary();

        Ok(snapshots)
    }

    /// Instrument and recompile the source code
    fn instrument_and_recompile_source_code(
        &self,
        artifacts: &HashMap<Address, Artifact>,
        analysis_result: &HashMap<Address, AnalysisResult>,
    ) -> Result<HashMap<Address, Artifact>> {
        info!("Instrumenting source code based on analysis results");

        let mut recompiled_artifacts = HashMap::new();
        for (address, artifact) in artifacts {
            let compiler_version =
                Version::parse(artifact.compiler_version().trim_start_matches('v'))?;

            let analysis = analysis_result
                .get(address)
                .ok_or_else(|| eyre::eyre!("No analysis result found for address {}", address))?;

            let input = instrument(&compiler_version, &artifact.input, analysis)?;
            let meta = artifact.meta.clone();

            // prepare the compiler
            let version = meta.compiler_version()?;
            let compiler = Solc::find_or_install(&version)?;

            // compile the source code
            let output = match compiler.compile_exact(&input) {
                Ok(output) => output,
                Err(e) => {
                    // Dump source code for debugging
                    let (original_dir, instrumented_dir) =
                        dump_source_for_debugging(address, &artifact.input, &input)?;

                    return Err(eyre::eyre!(
                        "Failed to recompile contract {}\n\nCompiler error: {}\n\nDebug info:\n  Original source: {}\n  Instrumented source: {}",
                        address,
                        e,
                        original_dir.display(),
                        instrumented_dir.display()
                    ));
                }
            };
            if output.errors.iter().any(|e| e.is_error()) {
                // Dump source code for debugging
                let (original_dir, instrumented_dir) =
                    dump_source_for_debugging(address, &artifact.input, &input)?;

                // Format errors with better source location info
                let formatted_errors = format_compiler_errors(&output.errors, &instrumented_dir);

                return Err(eyre::eyre!(
                    "Recompilation failed for contract {}\n\nCompilation errors:{}\n\nDebug info:\n  Original source: {}\n  Instrumented source: {}",
                    address,
                    formatted_errors,
                    original_dir.display(),
                    instrumented_dir.display()
                ));
            }

            debug!(
                "Recompiled Contract {}: {} vs {}",
                address,
                artifact.output.contracts.len(),
                output.contracts.len()
            );

            recompiled_artifacts.insert(*address, Artifact { meta, input, output });
        }

        Ok(recompiled_artifacts)
    }

    /// Tweak the bytecode of the contracts
    async fn tweak_bytecode<DB>(
        &self,
        ctx: &mut EdbContext<DB>,
        artifacts: &HashMap<Address, Artifact>,
        recompiled_artifacts: &HashMap<Address, Artifact>,
        tx_hash: TxHash,
    ) -> Result<Vec<Address>>
    where
        DB: Database + DatabaseCommit + DatabaseRef + Clone,
        <CacheDB<DB> as Database>::Error: Clone,
        <DB as Database>::Error: Clone,
    {
        let mut tweaker =
            CodeTweaker::new(ctx, self.rpc_proxy_url.clone(), self.etherscan_api_key.clone());

        let mut contracts_in_tx = Vec::new();

        for (address, recompiled_artifact) in recompiled_artifacts {
            let creation_tx_hash = tweaker.get_creation_tx(address).await?;
            if creation_tx_hash == tx_hash {
                debug!("Skip tweaking contract {}, since it was created by the transaction under investigation", address);
                contracts_in_tx.push(*address);
                continue;
            }

            let artifact = artifacts
                .get(address)
                .ok_or_else(|| eyre::eyre!("No original artifact found for address {}", address))?;

            tweaker.tweak(address, artifact, recompiled_artifact, self.quick).await.map_err(
                |e| eyre::eyre!("Failed to tweak bytecode for contract {}: {}", address, e),
            )?;
        }

        Ok(contracts_in_tx)
    }

    // Collect creation code that will be hooked
    fn collect_creation_hooks<'a>(
        &self,
        artifacts: &'a HashMap<Address, Artifact>,
        recompiled_artifacts: &'a HashMap<Address, Artifact>,
        contracts_in_tx: Vec<Address>,
    ) -> Result<Vec<(&'a Contract, &'a Contract, &'a Bytes)>> {
        info!("Collecting creation hooks for contracts in transaction");

        let mut hook_creation = Vec::new();
        for address in contracts_in_tx {
            let Some(artifact) = artifacts.get(&address) else {
                eyre::bail!("No original artifact found for address {}", address);
            };

            let Some(recompiled_artifact) = recompiled_artifacts.get(&address) else {
                eyre::bail!("No recompiled artifact found for address {}", address);
            };

            hook_creation.extend(artifact.find_creation_hooks(recompiled_artifact));
        }

        Ok(hook_creation)
    }

    // Create snapshots for time travel
    fn get_time_travel_snapshots<DB>(
        &self,
        opcode_snapshots: OpcodeSnapshots<DB>,
        hook_snapshots: HookSnapshots<DB>,
    ) -> Result<Snapshots<DB>>
    where
        DB: Database + DatabaseCommit + DatabaseRef + Clone,
        <CacheDB<DB> as Database>::Error: Clone,
        <DB as Database>::Error: Clone,
    {
        let snapshots = Snapshots::merge(opcode_snapshots, hook_snapshots);
        snapshots.print_summary();

        Ok(snapshots)
    }
}

// Helper functions
impl Engine {
    fn get_etherscan_api_key(&self) -> String {
        self.etherscan_api_key.clone().unwrap_or(next_etherscan_api_key())
    }
}

/// Sanitize a path to prevent directory traversal attacks
fn sanitize_path(path: &std::path::Path) -> PathBuf {
    use std::path::Component;

    let mut sanitized = PathBuf::new();

    for component in path.components() {
        match component {
            Component::Normal(name) => {
                // Only add normal path components (no .., ., or absolute paths)
                sanitized.push(name);
            }
            Component::CurDir => {
                // Skip "." components
            }
            Component::ParentDir => {
                // Skip ".." components - don't allow traversal
                warn!("Skipping parent directory component in path: {:?}", path);
            }
            Component::RootDir => {
                // Skip root directory "/" - don't allow absolute paths
                warn!("Skipping root directory component in path: {:?}", path);
            }
            Component::Prefix(_) => {
                // Skip Windows drive prefixes like "C:"
                warn!("Skipping prefix component in path: {:?}", path);
            }
        }
    }

    // If the path was completely stripped, use a default name
    if sanitized.as_os_str().is_empty() {
        sanitized.push("unnamed_source");
    }

    sanitized
}

/// Extract code context around an error position
fn extract_code_context(
    file_path: &std::path::Path,
    start_pos: i32,
    end_pos: i32,
    context_lines: usize,
) -> Option<String> {
    use std::io::{BufRead, BufReader};

    let file = fs::File::open(file_path).ok()?;
    let reader = BufReader::new(file);
    let lines: Vec<String> = reader.lines().map_while(Result::ok).collect();

    // Convert byte positions to line/column
    let mut current_pos = 0i32;
    let mut start_line = 0;
    let mut start_col = 0;
    let mut end_line = 0;
    let mut end_col = 0;

    for (line_num, line) in lines.iter().enumerate() {
        let line_start = current_pos;
        let line_end = current_pos + line.len() as i32 + 1; // +1 for newline

        if start_pos >= line_start && start_pos < line_end {
            start_line = line_num;
            start_col = (start_pos - line_start) as usize;
        }

        if end_pos >= line_start && end_pos <= line_end {
            end_line = line_num;
            end_col = (end_pos - line_start) as usize;
        }

        current_pos = line_end;
    }

    // Build context
    let mut context = String::new();
    let context_start = start_line.saturating_sub(context_lines);
    let context_end = (end_line + context_lines + 1).min(lines.len());

    for line_num in context_start..context_end {
        if line_num >= lines.len() {
            break;
        }

        let line_number = line_num + 1; // 1-indexed
        let line = &lines[line_num];

        // Format line with line number
        if line_num >= start_line && line_num <= end_line {
            // Error line - highlight it
            context.push_str(&format!("  {line_number} | {line}\n"));

            // Add underline for the error position on the first error line
            if line_num == start_line {
                let padding = format!("  {line_number} | ").len();
                let mut underline = " ".repeat(padding + start_col);
                let underline_len = if start_line == end_line {
                    (end_col - start_col).max(1)
                } else {
                    line.len() - start_col
                };
                underline.push_str(&"^".repeat(underline_len));
                context.push_str(&format!("{underline}\n"));
            }
        } else {
            // Context line
            context.push_str(&format!("  {line_number} | {line}\n"));
        }
    }

    Some(context)
}

/// Format compiler errors with better source location information
fn format_compiler_errors(
    errors: &[foundry_compilers::artifacts::Error],
    dump_dir: &std::path::Path,
) -> String {
    let mut formatted = String::new();

    for error in errors.iter().filter(|e| e.is_error()) {
        formatted.push_str("\n\n");

        // Add error severity and type
        if let Some(error_code) = &error.error_code {
            formatted.push_str(&format!("Error [{error_code}]: "));
        } else {
            formatted.push_str("Error: ");
        }

        // Add the main error message
        formatted.push_str(&error.message);

        // Add source location and code context if available
        if let Some(loc) = &error.source_location {
            formatted.push_str(&format!("\n  --> {}:{}:{}", loc.file.as_str(), loc.start, loc.end));

            // Try to extract code context from the dumped files
            let sanitized_path = sanitize_path(std::path::Path::new(&loc.file));
            let source_file = dump_dir.join(&sanitized_path);

            if let Some(context) = extract_code_context(&source_file, loc.start, loc.end, 5) {
                formatted.push_str("\n\n");
                formatted.push_str(&context);
            }
        }

        // If we have a formatted message with context, also include it
        // (as it might have additional information)
        if let Some(formatted_msg) = &error.formatted_message {
            if !formatted_msg.trim().is_empty() {
                formatted.push_str("\n\nCompiler's formatted output:\n");
                formatted.push_str(formatted_msg);
            }
        }

        // Add secondary locations if any
        if !error.secondary_source_locations.is_empty() {
            for sec_loc in &error.secondary_source_locations {
                if let Some(msg) = &sec_loc.message {
                    formatted.push_str(&format!("\n  Note: {msg}"));
                }
                if let Some(file) = &sec_loc.file {
                    formatted.push_str(&format!(
                        "\n    --> {}:{}:{}",
                        file,
                        sec_loc.start.map(|s| s.to_string()).unwrap_or_else(|| "?".to_string()),
                        sec_loc.end.map(|e| e.to_string()).unwrap_or_else(|| "?".to_string())
                    ));

                    // Try to show context for secondary locations too
                    if let (Some(start), Some(end)) = (sec_loc.start, sec_loc.end) {
                        let sanitized_path = sanitize_path(std::path::Path::new(file));
                        let source_file = dump_dir.join(&sanitized_path);

                        if let Some(context) = extract_code_context(&source_file, start, end, 1) {
                            formatted.push('\n');
                            formatted.push_str(&context);
                        }
                    }
                }
            }
        }
    }

    if formatted.is_empty() {
        formatted.push_str("\nNo specific error details available");
    }

    formatted
}

/// Dump source code to a temporary directory for debugging
fn dump_source_for_debugging(
    address: &Address,
    original_input: &SolcInput,
    instrumented_input: &SolcInput,
) -> Result<(PathBuf, PathBuf)> {
    use std::io::Write;

    // Create temp directories
    let temp_dir = std::env::temp_dir();
    let debug_dir = temp_dir.join(format!("edb_debug_{address}"));
    let original_dir = debug_dir.join("original");
    let instrumented_dir = debug_dir.join("instrumented");

    // Create directories
    fs::create_dir_all(&original_dir)?;
    fs::create_dir_all(&instrumented_dir)?;

    // Write original sources
    for (path_str, source) in &original_input.sources {
        let path = std::path::Path::new(path_str);
        let sanitized_path = sanitize_path(path);
        let file_path = original_dir.join(&sanitized_path);

        // Safety check: verify the resulting path is still within our directory
        // We use a path-based check rather than canonicalize to avoid TOCTOU issues
        if !file_path.starts_with(&original_dir) {
            return Err(eyre::eyre!(
                "Path traversal detected in source path: {}",
                path_str.display()
            ));
        }

        // Create parent directories if needed
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut file = fs::File::create(&file_path)?;
        file.write_all(source.content.as_bytes())?;
    }

    // Write original settings.json
    let settings_path = original_dir.join("settings.json");
    let mut settings_file = fs::File::create(&settings_path)?;
    settings_file.write_all(serde_json::to_string_pretty(&original_input.settings)?.as_bytes())?;

    // Write instrumented sources
    for (path_str, source) in &instrumented_input.sources {
        let path = std::path::Path::new(path_str);
        let sanitized_path = sanitize_path(path);
        let file_path = instrumented_dir.join(&sanitized_path);

        // Safety check: verify the resulting path is still within our directory
        // We use a path-based check rather than canonicalize to avoid TOCTOU issues
        if !file_path.starts_with(&instrumented_dir) {
            return Err(eyre::eyre!(
                "Path traversal detected in source path: {}",
                path_str.display()
            ));
        }

        // Create parent directories if needed
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut file = fs::File::create(&file_path)?;
        file.write_all(source.content.as_bytes())?;
    }

    // Write instrumented settings.json
    let settings_path = instrumented_dir.join("settings.json");
    let mut settings_file = fs::File::create(&settings_path)?;
    settings_file
        .write_all(serde_json::to_string_pretty(&instrumented_input.settings)?.as_bytes())?;

    Ok((original_dir, instrumented_dir))
}
