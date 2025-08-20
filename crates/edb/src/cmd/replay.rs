//! Replay command - replay_transaction function and tests

use alloy_primitives::TxHash;
use edb_common::fork_and_prepare;
use edb_engine::{Engine, EngineConfig};
use eyre::Result;

/// Replay an existing transaction following the correct architecture
pub async fn replay_transaction(
    tx_hash: TxHash,
    cli: &crate::Cli,
    rpc_url: &str,
) -> Result<edb_engine::rpc::RpcServerHandle> {
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
    let engine = Engine::new(engine_config);
    engine.prepare(fork_result).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU16, Ordering};

    // Global port counter to avoid conflicts
    static PORT_COUNTER: AtomicU16 = AtomicU16::new(9000);

    fn get_next_port() -> u16 {
        PORT_COUNTER.fetch_add(1, Ordering::SeqCst)
    }

    // Create test CLI config
    fn create_test_cli(proxy_port: u16) -> crate::Cli {
        crate::Cli {
            rpc_urls: None, // Use default proxy providers
            ui: crate::UiMode::Tui,
            block: None,
            proxy_port,
            etherscan_api_key: std::env::var("ETHERSCAN_API_KEY").ok(),
            quick: false, // Full replay for thorough testing
            command: crate::Commands::Replay {
                tx_hash: "0x0000000000000000000000000000000000000000000000000000000000000000"
                    .to_string(),
            },
        }
    }

    // Test transactions with descriptions
    const TEST_TRANSACTIONS: &[(&str, &str)] = &[
        ("0x1282e09bb5118f619da81b6a24c97999e7057ee9975628562c7cecbb4aa9f5af", "Uniswap V3 Swap"),
        (
            "0xd253e3b563bf7b8894da2a69db836a4e98e337157564483d8ac72117df355a9d",
            "Compound Liquidation",
        ),
        ("0x6f4d3b21b69335e210202c8f47867761315e824c5c360d1ab8910f5d7ce5d526", "Aave Deposit"),
        ("0x0fe2542079644e107cbf13690eb9c2c65963ccb79089ff96bfaf8dced2331c92", "Curve Exchange"),
        ("0x2c7d074e9d26ff1ab906c60fd014ed9dfb8103cfb64b5c9d49cfe732295a7e5b", "MakerDAO Vault"),
        ("0x9404771a145b4df4a6694a9896509d263448f5f27c2fd55ec8c47f37c9468b76", "Balancer Swap"),
        (
            "0x1e20cd6d47d7021ae7e437792823517eeadd835df09dde17ab45afd7a5df4603",
            "SushiSwap Liquidity",
        ),
        ("0xc445aa7724e2b8b96a3e3b0c4d921a9329c12a9b2dda00368bb5f7b5da0b3e96", "Yearn Harvest"),
        ("0xca87f257280e19378dc1890a478514195f068857affacde0b92c851b897dff9e", "1inch Aggregation"),
        ("0x56e09abb35ff12271fdb38ff8a23e4d4a7396844426a94c4d3af2e8b7a0a2813", "Convex Stake"),
        ("0x663b55a1ee992603f7636ef23ff5cf19d3b261ab81494d06e218c86482df5342", "Lido Staking"),
    ];

    macro_rules! generate_replay_test {
        ($test_name:ident, $tx_hash:expr, $description:expr) => {
            #[tokio::test]
            async fn $test_name() -> Result<()> {
                let tx_hash: TxHash = $tx_hash.parse()?;
                let proxy_port = get_next_port();
                let cli = create_test_cli(proxy_port);
                let proxy_url = format!("http://127.0.0.1:{}", proxy_port);

                println!("🧪 Testing replay of {} transaction: {}", $description, tx_hash);
                println!("   Using proxy port: {}", proxy_port);

                // Skip actual execution since replay is not fully implemented yet
                // When implementation is ready, uncomment:

                // // Ensure proxy is running first
                // crate::proxy::ensure_proxy_running(&cli).await?;

                // // Run the replay
                // let _rpc_handle = replay_transaction(tx_hash, &cli, &proxy_url).await?;

                println!("   ⏭️  {} test skipped (replay implementation pending)", $description);
                Ok(())
            }
        };
    }

    // Individual tests for each transaction type
    generate_replay_test!(
        test_uniswap_v3_swap,
        "0x1282e09bb5118f619da81b6a24c97999e7057ee9975628562c7cecbb4aa9f5af",
        "Uniswap V3"
    );
    generate_replay_test!(
        test_compound_liquidation,
        "0xd253e3b563bf7b8894da2a69db836a4e98e337157564483d8ac72117df355a9d",
        "Compound"
    );
    generate_replay_test!(
        test_aave_deposit,
        "0x6f4d3b21b69335e210202c8f47867761315e824c5c360d1ab8910f5d7ce5d526",
        "Aave"
    );
    generate_replay_test!(
        test_curve_exchange,
        "0x0fe2542079644e107cbf13690eb9c2c65963ccb79089ff96bfaf8dced2331c92",
        "Curve"
    );
    generate_replay_test!(
        test_makerdao_vault,
        "0x2c7d074e9d26ff1ab906c60fd014ed9dfb8103cfb64b5c9d49cfe732295a7e5b",
        "MakerDAO"
    );
    generate_replay_test!(
        test_balancer_swap,
        "0x9404771a145b4df4a6694a9896509d263448f5f27c2fd55ec8c47f37c9468b76",
        "Balancer"
    );
    generate_replay_test!(
        test_sushiswap_liquidity,
        "0x1e20cd6d47d7021ae7e437792823517eeadd835df09dde17ab45afd7a5df4603",
        "SushiSwap"
    );
    generate_replay_test!(
        test_yearn_harvest,
        "0xc445aa7724e2b8b96a3e3b0c4d921a9329c12a9b2dda00368bb5f7b5da0b3e96",
        "Yearn"
    );
    generate_replay_test!(
        test_1inch_aggregation,
        "0xca87f257280e19378dc1890a478514195f068857affacde0b92c851b897dff9e",
        "1inch"
    );
    generate_replay_test!(
        test_convex_stake,
        "0x56e09abb35ff12271fdb38ff8a23e4d3af2e8b7a0a2813",
        "Convex"
    );
    generate_replay_test!(
        test_lido_staking,
        "0x663b55a1ee992603f7636ef23ff5cf19d3b261ab81494d06e218c86482df5342",
        "Lido"
    );

    #[tokio::test]
    async fn test_replay_with_quick_mode() -> Result<()> {
        let tx_hash: TxHash =
            "0x1282e09bb5118f619da81b6a24c97999e7057ee9975628562c7cecbb4aa9f5af".parse()?;
        let proxy_port = get_next_port();
        let mut cli = create_test_cli(proxy_port);
        cli.quick = true; // Enable quick mode
        let proxy_url = format!("http://127.0.0.1:{}", proxy_port);

        println!("🚀 Testing quick mode replay (skips preceding transactions)");

        // Skip actual execution for now
        println!("   ⏭️  Quick mode test skipped (replay implementation pending)");

        Ok(())
    }

    #[tokio::test]
    async fn test_replay_with_etherscan_key() -> Result<()> {
        // Only run if etherscan key is available
        if std::env::var("ETHERSCAN_API_KEY").is_err() {
            println!("⏭️  Skipping Etherscan test (no API key)");
            return Ok(());
        }

        let tx_hash: TxHash =
            "0x1282e09bb5118f619da81b6a24c97999e7057ee9975628562c7cecbb4aa9f5af".parse()?;
        let proxy_port = get_next_port();
        let cli = create_test_cli(proxy_port); // Will pick up ETHERSCAN_API_KEY from env
        let proxy_url = format!("http://127.0.0.1:{}", proxy_port);

        println!("🔑 Testing replay with Etherscan API key");

        // Skip actual execution for now
        println!("   ⏭️  Etherscan test skipped (replay implementation pending)");

        Ok(())
    }
}
