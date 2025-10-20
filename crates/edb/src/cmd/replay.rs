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
use edb_engine::Engine;
use eyre::Result;

use crate::utils;

/// Replay an existing transaction following the correct architecture
pub async fn replay_transaction(tx_hash: TxHash, cli: &crate::Cli, rpc_url: &str) -> Result<()> {
    tracing::info!("Starting transaction replay workflow");

    // Step 1: Fork the chain and replay earlier transactions in the block
    // Fork and prepare the database/environment for the target transaction
    let fork_result = fork_and_prepare(rpc_url, tx_hash, cli.quick).await?;

    tracing::info!(
        "Forked chain and prepared database for transaction replay at block {}",
        fork_result.fork_info.block_number
    );

    // Step 2: Build inputs for the engine
    let engine_config = cli.to_engine_config(rpc_url);

    // Step 3: Call engine::prepare with forked database and EVM config
    tracing::info!("Calling engine::prepare with prepared inputs");
    let engine = Engine::new(engine_config);
    let rpc_server_addr = engine.prepare(fork_result).await?;

    // Step 4: Launch TUI and wait for user to exit
    utils::start_tui(&cli.tui_options, rpc_server_addr).await?;

    // Step 5: Shutdown EDB
    tracing::info!("Shutting down EDB...");
    engine.shutdown_rpc_server(&tx_hash)?;

    Ok(())
}
