//! EDB Engine - Core analysis and instrumentation logic
//!
//! This crate provides the core functionality for debugging Ethereum transactions
//! including source code instrumentation, recompilation, and state snapshot collection.
//!
//! The engine accepts a forked database and EVM configuration as inputs (prepared by edb binary)
//! and focuses on the instrumentation and analysis workflow.a

use alloy_primitives::Address;
use eyre::Result;
use foundry_block_explorers::Client;
use foundry_compilers::{
    artifacts::{Severity, SolcInput},
    solc::Solc,
};
use indicatif::{ProgressBar, ProgressStyle};
use revm::{
    context::{
        result::{ExecutionResult, HaltReason},
        Host, TxEnv,
    },
    database::{CacheDB, EmptyDB},
    Database, DatabaseCommit, InspectEvm, MainBuilder,
};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    time::Duration,
};
use tracing::{debug, error, info, warn};

use edb_common::{CachePath, EDBCachePath, EDBContext, ForkResult, DEFAULT_ETHERSCAN_CACHE_TTL};

use crate::{
    analysis::AnalysisResult,
    analyze,
    inspector::{CallTracer, SnapshotInspector, StateSnapshot, TraceReplayResult},
    instrument,
    rpc::{start_server, RpcServerHandle},
    utils::{next_etherscan_api_key, Artifact, OnchainCompiler},
    ExecutionFrameId, ExecutionStepInspector, ExecutionStepRecord, ExecutionStepRecords,
};

/// Configuration for the engine (reduced scope - no RPC URL or forking config)
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// Port for the JSON-RPC server
    pub rpc_port: u16,
    /// Etherscan API key for source code download
    pub etherscan_api_key: Option<String>,
    /// Quick mode - skip certain operations for faster analysis
    pub quick: bool,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self { rpc_port: 8545, etherscan_api_key: None, quick: false }
    }
}

/// The main Engine struct that performs transaction analysis
#[derive(Debug)]
pub struct Engine {
    /// Configuration for the engine
    config: EngineConfig,
}

impl Engine {
    /// Create a new Engine instance from configuration
    pub fn new(config: EngineConfig) -> Self {
        Self { config }
    }

    /// Create an Engine with default configuration
    pub fn default() -> Self {
        Self::new(EngineConfig::default())
    }

    /// Main preparation method for the engine
    ///
    /// This method accepts a forked database and EVM configuration prepared by the edb binary.
    /// It focuses on the core debugging workflow:
    /// 1. Replays the target transaction to collect touched contracts
    /// 2. Downloads verified source code for each contract
    /// 3. Collect opcode-level step execution results
    /// 4. Analyzes the source code to identify instrumentation points
    /// 5. Instruments and recompiles the source code
    /// 6. Re-executes the transaction with state snapshots
    /// 7. Starts a JSON-RPC server with the analysis results and snapshots
    pub async fn prepare<DB>(&self, fork_result: ForkResult<DB>) -> Result<RpcServerHandle>
    where
        DB: Database + DatabaseCommit + Clone,
        <DB as Database>::Error: Clone,
    {
        info!("Starting engine preparation for transaction: {:?}", fork_result.target_tx_hash);

        // Step 0: Initialize context and database
        let ForkResult { context: ctx, target_tx_env: tx, .. } = fork_result;

        // Step 1: Replay the target transaction to collect call trace and touched contracts
        let replay_result = self.replay_and_collect_trace(ctx.clone(), tx.clone())?;

        // Step 2: Download verified source code for each contract
        let artifacts =
            self.download_verified_source_code(&replay_result, ctx.chain_id().to::<u64>()).await?;

        // Step 3: Collect opcode-level step execution results
        let opcode_time_travel = self.time_travel_at_opcode_level(
            ctx.clone(),
            tx.clone(),
            artifacts.keys().into_iter().cloned().collect(),
        )?;

        // Step 4: Analyze source code to identify instrumentation points
        let analysis_result = self.analyze_source_code(&artifacts)?;

        // Step 5: Instrument source code
        let recompiled_artifacts =
            self.instrument_and_recompile_source_code(&artifacts, &analysis_result)?;
        unimplemented!("Implement logic to handle replay result");

        // Step 6: Replace original bytecode with instrumented versions

        // Step 7: Re-execute the transaction with snapshot collection
        let snapshots: Vec<CacheDB<EmptyDB>> = vec![]; // Placeholder for collected snapshots

        // Step 8: Start RPC server with analysis results and snapshots
        info!("Starting JSON-RPC server on port {}", self.config.rpc_port);
        let rpc_handle = start_server(self.config.rpc_port, analysis_result, snapshots).await?;

        Ok(rpc_handle)
    }

    /// Replay the target transaction and collect call trace with all touched addresses
    fn replay_and_collect_trace<DB>(
        &self,
        ctx: EDBContext<DB>,
        tx: TxEnv,
    ) -> Result<TraceReplayResult>
    where
        DB: Database + Clone,
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

        // Print the trace tree structure for debugging
        #[cfg(debug_assertions)]
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
            EDBCachePath::new(None as Option<PathBuf>).compiler_chain_cache_dir(chain_id);

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
                    console_bar.set_message(format!("✅ 0x{}... compiled", short_addr));
                    artifacts.insert(*address, artifact);
                }
                Ok(None) => {
                    console_bar.set_message(format!("⚠️  0x{}... no source", short_addr));
                    debug!("No source code available for contract {}", address);
                }
                Err(e) => {
                    console_bar.set_message(format!("❌ 0x{}... failed", short_addr));
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

    /// Time travel (i.e., snapshotting) at the opcode level for contracts we do not
    /// have source code.
    fn time_travel_at_opcode_level<DB>(
        &self,
        ctx: EDBContext<DB>,
        tx: TxEnv,
        touched_addresses: HashSet<Address>,
    ) -> Result<ExecutionStepRecords>
    where
        DB: Database + Clone,
        <DB as Database>::Error: Clone,
    {
        info!("Collecting opcode-level step execution results");

        let mut inspector = ExecutionStepInspector::with_excluded_addresses(touched_addresses);
        let mut evm = ctx.build_mainnet_with_inspector(&mut inspector);

        evm.inspect_one_tx(tx)
            .map_err(|e| eyre::eyre!("Failed to inspect the target transaction: {:?}", e))?;

        let snapshots = inspector.into_step_records();

        #[cfg(debug_assertions)]
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
            let analysis = analysis_result
                .get(address)
                .ok_or_else(|| eyre::eyre!("No analysis result found for address {}", address))?;

            let input = instrument(&artifact.input, analysis)?;
            let meta = artifact.meta.clone();

            // prepare the compiler
            let version = meta.compiler_version()?;
            let compiler = Solc::find_or_install(&version)?;

            // compile the source code
            let output = match compiler.compile_exact(&input) {
                Ok(output) => output,
                Err(e) => {
                    return Err(eyre::eyre!("failed to recompile contract: {}", e));
                }
            };
            if output.errors.iter().any(|e| e.is_error()) {
                return Err(eyre::eyre!(
                    "Recompilation failed for contract {}: {}",
                    address,
                    output
                        .errors
                        .iter()
                        .filter(|e| e.is_error())
                        .map(|e| format!("\n{}", e))
                        .collect::<Vec<_>>()
                        .join(""),
                ));
            }

            println!(
                "Recompiled Contract {}: {} vs {}",
                address,
                artifact.output.contracts.len(),
                output.contracts.len()
            );

            recompiled_artifacts.insert(*address, Artifact { meta, input, output });
        }

        Ok(recompiled_artifacts)
    }
}

// Helper functions
impl Engine {
    fn get_etherscan_api_key(&self) -> String {
        self.config.etherscan_api_key.clone().unwrap_or(next_etherscan_api_key())
    }
}
