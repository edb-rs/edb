//! EDB Engine - Core analysis and instrumentation logic
//!
//! This crate provides the core functionality for debugging Ethereum transactions
//! including source code instrumentation, recompilation, and state snapshot collection.
//!
//! The engine accepts a forked database and EVM configuration as inputs (prepared by edb binary)
//! and focuses on the instrumentation and analysis workflow.

#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use alloy_primitives::{Address, TxHash};
use eyre::Result;
use revm::{
    context::ContextTr,
    database::{AlloyDB, CacheDB, EmptyDB},
};
use std::collections::HashMap;

use edb_utils::ForkResult;

mod analysis;
pub use analysis::*;

pub mod compiler;
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
    pub async fn prepare(
        &self,
        fork_result: ForkResult<impl ContextTr>,
    ) -> Result<rpc::RpcServerHandle> {
        tracing::info!(
            "Starting engine preparation for transaction: {:?}",
            fork_result.target_tx_hash
        );

        if self.config.quick {
            tracing::info!("Quick mode enabled - some analysis steps may be skipped");
        }

        // Step 1: Replay the target transaction to collect touched contracts

        // Step 2: Download verified source code for each contract

        // Step 3: Analyze source code to identify instrumentation points
        let analysis_result = AnalysisResult::default();

        // Step 4: Instrument source code

        // Step 5: Recompile instrumented contracts

        // Step 6: Replace original bytecode with instrumented versions

        // Step 7: Re-execute the transaction with snapshot collection
        let snapshots: Vec<CacheDB<EmptyDB>> = vec![]; // Placeholder for collected snapshots

        // Step 8: Start RPC server with analysis results and snapshots
        tracing::info!("Starting JSON-RPC server on port {}", self.config.rpc_port);
        let rpc_handle =
            rpc::start_server(self.config.rpc_port, analysis_result, snapshots).await?;

        Ok(rpc_handle)
    }
}
