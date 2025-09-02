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

//! EDB Engine - Core analysis and instrumentation logic
//!
//! This crate provides the core functionality for debugging Ethereum transactions
//! including source code instrumentation, recompilation, and state snapshot collection.
//!
//! The engine accepts a forked database and EVM configuration as inputs (prepared by edb binary)
//! and focuses on the instrumentation and analysis workflow.a

use alloy_primitives::{Address, Bytes, TxHash};
use eyre::Result;
use foundry_block_explorers::Client;
use foundry_compilers::{artifacts::Contract, solc::Solc};
use indicatif::{ProgressBar, ProgressStyle};
use revm::{
    context::{
        result::{ExecutionResult, HaltReason},
        Host, TxEnv,
    },
    database::CacheDB,
    Database, DatabaseCommit, DatabaseRef, InspectEvm, MainBuilder,
};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    time::Duration,
};
use tracing::{debug, error, info, warn};

use edb_common::{
    relax_context_constraints, CachePath, EdbCachePath, EdbContext, ForkResult,
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
    OpcodeSnapshots, Snapshots,
};

/// Configuration for the engine (reduced scope - no RPC URL or forking config)
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// RPC Provider URL
    pub rpc_proxy_url: String,
    /// Etherscan API key for source code download
    pub etherscan_api_key: Option<String>,
    /// Quick mode - skip certain operations for faster analysis
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

impl Engine {
    /// Create a new Engine instance from configuration
    pub fn new(config: EngineConfig) -> Self {
        let EngineConfig { rpc_proxy_url, etherscan_api_key, quick } = config;
        Self { rpc_proxy_url, host_port: None, etherscan_api_key, quick }
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

        // Step 3: Collect opcode-level step execution results
        info!("Collecting opcode-level step execution results");
        let opcode_snapshots = self.capture_opcode_level_snapshots(
            ctx.clone(),
            tx.clone(),
            artifacts.keys().into_iter().cloned().collect(),
        )?;

        // Step 4: Analyze source code to identify instrumentation points
        info!("Analyzing source code");
        let analysis_results = self.analyze_source_code(&artifacts)?;

        // Step 5: Instrument source code
        info!("Instrumenting source code");
        let recompiled_artifacts =
            self.instrument_and_recompile_source_code(&artifacts, &analysis_results)?;

        // Step 6: Replace original bytecode with instrumented versions
        info!("Tweaking bytecode");
        let contracts_in_tx = self.tweak_bytecode(&mut ctx, &recompiled_artifacts, tx_hash).await?;

        // Step 7: Re-execute the transaction with snapshot collection
        info!("Re-executing transaction with snapshot collection");
        let hook_creation =
            self.collect_creation_hooks(&artifacts, &recompiled_artifacts, contracts_in_tx)?;
        let hook_snapshots = self.capture_hook_snapshots(ctx.clone(), tx.clone(), hook_creation)?;

        // Step 8: Start RPC server with analysis results and snapshots
        info!("Starting RPC server with analysis results and snapshots");
        let mut snapshots = self.get_time_travel_snapshots(opcode_snapshots, hook_snapshots)?;
        snapshots.analyze_next_step(&replay_result.execution_trace, &analysis_results)?;
        // Let's pack the debug context
        let mut context = EngineContext {
            cfg: ctx.cfg.clone(),
            block: ctx.block.clone(),
            tx,
            tx_hash,
            fork_info,
            snapshots,
            artifacts,
            recompiled_artifacts,
            analysis_results,
            trace: replay_result.execution_trace,
        };
        context.finalize();

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

    /// Time travel (i.e., snapshotting) at hooks for contracts we have source code
    fn capture_hook_snapshots<'a, DB>(
        &self,
        mut ctx: EdbContext<DB>,
        mut tx: TxEnv,
        creation_hooks: Vec<(&'a Contract, &'a Contract, &'a Bytes)>,
    ) -> Result<HookSnapshots<DB>>
    where
        DB: Database + DatabaseCommit + DatabaseRef + Clone,
        <CacheDB<DB> as Database>::Error: Clone,
        <DB as Database>::Error: Clone,
    {
        // We need to relax execution constraints for hook snapshots
        relax_context_constraints(&mut ctx, &mut tx);

        info!("Collecting hook snapshots for source code contracts");

        let mut inspector = HookSnapshotInspector::new();
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
        touched_addresses: HashSet<Address>,
    ) -> Result<OpcodeSnapshots<DB>>
    where
        DB: Database + DatabaseCommit + DatabaseRef + Clone,
        <CacheDB<DB> as Database>::Error: Clone,
        <DB as Database>::Error: Clone,
    {
        info!("Collecting opcode-level step execution results");

        let mut inspector = OpcodeSnapshotInspector::new(&ctx);
        inspector.with_excluded_addresses(touched_addresses);
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

        for (address, artifact) in artifacts {
            let creation_tx_hash = tweaker.get_creation_tx(address).await?;
            if creation_tx_hash == tx_hash {
                debug!("Skip tweaking contract {}, since it was created by the transaction under investigation", address);
                contracts_in_tx.push(*address);
                continue;
            }

            tweaker.tweak(address, artifact, self.quick).await.map_err(|e| {
                eyre::eyre!("Failed to tweak bytecode for contract {}: {}", address, e)
            })?;
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

            hook_creation.extend(artifact.find_creation_hooks(&recompiled_artifact));
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
