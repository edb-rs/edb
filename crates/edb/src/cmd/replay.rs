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

//! Replay command - replay_transaction function and tests

use alloy_primitives::TxHash;
use edb_common::fork_and_prepare;
use edb_engine::{Engine, EngineConfig, RpcServerHandle};
use eyre::Result;

/// Replay an existing transaction following the correct architecture
pub async fn replay_transaction(
    tx_hash: TxHash,
    cli: &crate::Cli,
    rpc_url: &str,
) -> Result<RpcServerHandle> {
    tracing::info!("Starting transaction replay workflow");

    // Step 1: Fork the chain and replay earlier transactions in the block
    // Fork and prepare the database/environment for the target transaction
    let fork_result = fork_and_prepare(rpc_url, tx_hash, cli.quick).await?;

    tracing::info!(
        "Forked chain and prepared database for transaction replay at block {}",
        fork_result.fork_info.block_number
    );

    // Step 2: Build inputs for the engine
    let mut engine_config =
        EngineConfig::default().with_quick_mode(cli.quick).with_rpc_proxy_url(rpc_url.into());
    if let Some(api_key) = &cli.etherscan_api_key {
        engine_config = engine_config.with_etherscan_api_key(api_key.clone());
    }

    // Step 3: Call engine::prepare with forked database and EVM config
    tracing::info!("Calling engine::prepare with prepared inputs");

    // Create the engine and run preparation
    let mut engine = Engine::new(engine_config);
    engine.prepare(fork_result).await
}
