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

//! End-to-end integration tests for the EDB engine
//!
//! These tests verify the complete transaction replay workflow including:
//! - Forking and preparing the blockchain state
//! - Engine initialization and preparation
//! - Error detection in logs during the process

use alloy_primitives::TxHash;
use edb_integration_tests::test_utils::{engine, init, proxy};
use tracing::info;

/// Test transaction hashes - PLACEHOLDER: Replace with actual transaction hashes
mod test_transactions {
    /// Simple transaction
    pub const SIMPLE_TRANSACTION: &str =
        "0x608d79d71287ca8a0351955a92fa4dce74d2c75cbfccfa08ed331b33de0ce4c2";

    /// Large transaction
    pub const LARGE_TRANSACTION: &str =
        "0x0886e768f9310a753e360738e1aa6647847aca57c2ce05f09ca1333e8cf81e8c";

    /// Uniswap V3 swap transaction
    pub const UNISWAP_V3_SWAP: &str =
        "0x1282e09bb5118f619da81b6a24c97999e7057ee9975628562c7cecbb4aa9f5af";
}

async fn replay_transaction_and_analyze(tx_hash: &str) {
    // Setup proxy for reliable testing
    let proxy_url =
        proxy::setup_test_proxy_configurable(3600).await.expect("Failed to setup test proxy");

    proxy::register_with_proxy(&proxy_url).await.ok();

    // Test with a simple ETH transfer
    let tx_hash: TxHash = tx_hash.parse().expect("valid tx hash");

    match engine::replay_transaction_test(tx_hash, &proxy_url, false, None).await {
        Ok(result) => {
            // Check that no errors were logged
            if !result.success {
                panic!("Errors found during replay: {:?}", result.errors);
            }
            info!("Transaction {tx_hash} replay completed without errors");
        }
        Err(e) => {
            panic!("Failed to replay transaction: {e:?}");
        }
    }

    proxy::shutdown_test_proxy(&proxy_url).await.ok();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_engine_replay_simple_transaction() {
    init::init_test_environment();
    info!("Testing engine replay with simple transaction");

    replay_transaction_and_analyze(test_transactions::SIMPLE_TRANSACTION).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_engine_replay_large_transaction() {
    init::init_test_environment();
    info!("Testing engine replay with large transaction");

    replay_transaction_and_analyze(test_transactions::LARGE_TRANSACTION).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_engine_replay_uniswap_v3_swap() {
    init::init_test_environment();
    info!("Testing engine replay with Uniswap V3 swap transaction");

    replay_transaction_and_analyze(test_transactions::UNISWAP_V3_SWAP).await;
}
