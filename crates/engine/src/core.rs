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

use alloy_primitives::TxHash;
use edb_common::ForkResult;
use eyre::Result;
use revm::{context::Host, database::CacheDB, Database, DatabaseCommit, DatabaseRef};
use std::{collections::HashMap, net::SocketAddr};
use tracing::info;

use crate::{
    orchestration,
    rpc::{start_debug_server, RpcServerHandle},
    utils::next_etherscan_api_key,
    EngineContext, SnapshotAnalysis,
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

    /// Get the Etherscan API key, either from config or rotate to the next available key
    pub fn get_etherscan_api_key(&self) -> String {
        self.etherscan_api_key.clone().unwrap_or(next_etherscan_api_key())
    }
}

/// The main Engine struct that performs transaction analysis
#[derive(Debug)]
pub struct Engine {
    /// Map of transaction hashes to their RPC server handles
    pub server_handles: HashMap<TxHash, SocketAddr>,

    /// Configuration for the engine
    config: EngineConfig,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new(EngineConfig::default())
    }
}

impl Engine {
    /// Create a new Engine instance from configuration
    pub fn new(config: EngineConfig) -> Self {
        Self { server_handles: HashMap::new(), config }
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
    pub async fn prepare<DB>(&mut self, fork_result: ForkResult<DB>) -> Result<RpcServerHandle>
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
        let replay_result = orchestration::replay_and_collect_trace(ctx.clone(), tx.clone())?;

        // Step 2: Download verified source code for each contract
        let artifacts = orchestration::download_verified_source_code(
            &self.config,
            &replay_result,
            ctx.chain_id().to::<u64>(),
        )
        .await?;

        // Step 3: Analyze source code to identify instrumentation points
        let analysis_results = orchestration::analyze_source_code(&artifacts)?;

        // Step 4: Instrument source code
        let recompiled_artifacts =
            orchestration::instrument_and_recompile_source_code(&artifacts, &analysis_results)?;

        // Step 5: Collect opcode-level step execution results
        let opcode_snapshots = orchestration::capture_opcode_level_snapshots(
            ctx.clone(),
            tx.clone(),
            artifacts.keys().cloned().collect(),
            &replay_result.execution_trace,
        )?;

        // Step 6: Replace original bytecode with instrumented versions
        let contracts_in_tx = orchestration::tweak_bytecode(
            &self.config,
            &mut ctx,
            &artifacts,
            &recompiled_artifacts,
            tx_hash,
        )
        .await?;

        // Step 7: Re-execute the transaction with snapshot collection
        let hook_creation = orchestration::collect_creation_hooks(
            &artifacts,
            &recompiled_artifacts,
            contracts_in_tx,
        )?;
        let hook_snapshots = orchestration::capture_hook_snapshots(
            ctx.clone(),
            tx.clone(),
            hook_creation,
            &replay_result.execution_trace,
            &analysis_results,
        )?;

        // Step 8: Start RPC server with analysis results and snapshots
        let mut snapshots =
            orchestration::get_time_travel_snapshots(opcode_snapshots, hook_snapshots)?;
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
        info!("Debug RPC server started on {}", rpc_handle.addr());

        // Store the server handle for future reference
        self.server_handles.insert(tx_hash, rpc_handle.addr());

        Ok(rpc_handle)
    }
}
