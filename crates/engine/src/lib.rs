//! EDB Engine - Core analysis and instrumentation logic
//!
//! This crate provides the core functionality for debugging Ethereum transactions
//! including source code instrumentation, recompilation, and state snapshot collection.
//!
//! The engine accepts a forked database and EVM configuration as inputs (prepared by edb binary)
//! and focuses on the instrumentation and analysis workflow.

#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use alloy_chains::Chain;
use alloy_primitives::{Address, ChainId, TxHash};
use eyre::Result;
use foundry_block_explorers::Client;
use revm::{
    context::{
        result::{ExecutionResult, HaltReason}, ContextTr, CreateScheme, Host, TxEnv
    }, database::{AlloyDB, CacheDB, EmptyDB}, handler::api, interpreter::CallScheme, Database, DatabaseCommit, InspectEvm, MainBuilder
};
use std::{collections::{HashMap, HashSet}, path::PathBuf, time::Duration};
use tracing::{debug, error, info, warn};

use edb_utils::{CachePath, EDBCache, EDBCachePath, EDBContext, ForkResult, DEFAULT_ETHERSCAN_CACHE_TTL};

mod analysis;
pub use analysis::*;

use crate::{inspector::{CallTracer, TraceReplayResult}, utils::{next_etherscan_api_key, Artifact, OnchainCompiler}};

pub mod inspector;
pub mod instrumentation;
pub mod rpc;
pub mod source;
pub mod utils;

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
    /// 3. Analyzes the source code to identify instrumentation points
    /// 4. Instruments the source code
    /// 5. Recompiles and redeploys the instrumented contracts
    /// 6. Re-executes the transaction with state snapshots
    /// 7. Starts a JSON-RPC server with the analysis results and snapshots
    pub async fn prepare<DB: Database + DatabaseCommit>(
        &self,
        fork_result: ForkResult<DB>,
    ) -> Result<rpc::RpcServerHandle> {
        info!("Starting engine preparation for transaction: {:?}", fork_result.target_tx_hash);

        // Step 0: Initialize context and database
        let ForkResult { fork_info, context: ctx, target_tx_env: tx, target_tx_hash: tx_hash } =
            fork_result;

        // Step 1: Replay the target transaction to collect call trace and touched contracts
        let (replay_result, ctx) = self.replay_and_collect_trace(ctx, tx.clone())?;

        unimplemented!("Implement logic to handle replay result");

        // Step 2: Download verified source code for each contract
        let artifacts = self.download_verified_source_code(&replay_result, ctx.chain_id().to::<u64>()).await?;

        // Step 3: Analyze source code to identify instrumentation points
        let analysis_result = AnalysisResult::default();

        // Step 4: Instrument source code

        // Step 5: Recompile instrumented contracts

        // Step 6: Replace original bytecode with instrumented versions

        // Step 7: Re-execute the transaction with snapshot collection
        let snapshots: Vec<CacheDB<EmptyDB>> = vec![]; // Placeholder for collected snapshots

        // Step 8: Start RPC server with analysis results and snapshots
        info!("Starting JSON-RPC server on port {}", self.config.rpc_port);
        let rpc_handle =
            rpc::start_server(self.config.rpc_port, analysis_result, snapshots).await?;

        Ok(rpc_handle)
    }

    /// Replay the target transaction and collect call trace with all touched addresses
    fn replay_and_collect_trace<DB: Database>(
        &self,
        ctx: EDBContext<DB>,
        tx: TxEnv,
    ) -> Result<(TraceReplayResult, EDBContext<DB>)> {
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

        let ctx = evm.ctx;
        let result = tracer.into_replay_result();

        for (address, deployed) in &result.visited_addresses {
            if *deployed {
                info!("Contract {} was deployed during transaction replay", address);
            } else {
                info!("Address {} was touched during transaction replay", address);
            }
        }

        Ok((result, ctx))
    }

    /// Download verified source code for each contract
    async fn download_verified_source_code(&self, replay_result: &TraceReplayResult, chain_id: u64) -> Result<HashMap<Address, Artifact>> {
        info!("Downloading verified source code for touched contracts");

        let api_key_env = std::env::var("ETHERSCAN_API_KEY").ok();
        let compiler = OnchainCompiler::new(None)?;

        let compiler_cache_root = EDBCachePath::new(None as Option<PathBuf>).compiler_chain_cache_dir(chain_id);

        let mut artifacts = HashMap::new();
        for address in replay_result.visited_addresses.keys() {
            // We use the default API key if none is provided
            let api_key = if let Some(api_key) = &api_key_env {
                api_key.clone()
            } else {
                next_etherscan_api_key()
            };

            let etherscan = Client::builder()
                .with_api_key(api_key)
                .with_cache(compiler_cache_root.clone(), Duration::from_secs(DEFAULT_ETHERSCAN_CACHE_TTL)) // 24 hours
                .chain(chain_id.into())?
                .build()?;

            let Some(artifact) = compiler.compile(&etherscan, *address).await? else { continue };

            artifacts.insert(*address, artifact);
        }

        Ok(artifacts)
    }
}
