//! EDB - Ethereum Debugger
//!
//! A step-by-step debugger for Ethereum transactions.

use alloy_primitives::TxHash;
use clap::{Parser, Subcommand, ValueEnum};
use eyre::Result;

mod proxy;

/// Command-line interface for EDB
#[derive(Debug, Parser)]
#[command(name = "edb")]
#[command(about = "Ethereum Debugger - A step-by-step debugger for Ethereum transactions")]
#[command(version)]
pub struct Cli {
    /// Upstream RPC URLs (comma-separated, overrides defaults if provided)
    /// Example: --rpc-urls "https://eth.llamarpc.com,https://rpc.ankr.com/eth"
    #[arg(long)]
    rpc_urls: Option<String>,

    /// User interface to use
    #[arg(long, value_enum, default_value = "tui")]
    pub ui: UiMode,

    /// Block number to fork at (default: latest)
    #[arg(long)]
    pub block: Option<u64>,

    /// Port for the RPC proxy server
    #[arg(long, default_value = "8546")]
    pub proxy_port: u16,

    /// Etherscan API key for source code download
    #[arg(long, env = "ETHERSCAN_API_KEY")]
    pub etherscan_api_key: Option<String>,

    /// Quick mode - skip replaying preceding transactions in the block
    #[arg(long)]
    pub quick: bool,

    /// Command to execute
    #[command(subcommand)]
    pub command: Commands,
}

/// Available UI modes
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum UiMode {
    /// Terminal User Interface
    Tui,
    /// Web User Interface
    Web,
}

/// Available commands
#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Replay an existing transaction
    Replay {
        /// Transaction hash to replay
        tx_hash: String,
    },
    /// Debug a Foundry test case
    Test {
        /// Test name to debug
        test_name: String,
    },
    /// Show RPC proxy provider status
    ProxyStatus,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables
    dotenv::dotenv().ok();

    // Initialize logging
    edb_utils::logging::init_logging("edb", true)?;

    // Parse CLI arguments
    let cli = Cli::parse();

    // Set up RPC endpoint (proxy or direct)
    let effective_rpc_url = {
        tracing::info!("Ensuring RPC proxy is running...");
        proxy::ensure_proxy_running(&cli).await?;
        format!("http://127.0.0.1:{}", cli.proxy_port)
    };

    tracing::info!("Using RPC endpoint: {}", effective_rpc_url);

    // Handle proxy status command separately (doesn't need engine)
    if let Commands::ProxyStatus = &cli.command {
        return show_proxy_status(&cli).await;
    }

    // Execute the command to get RPC server handle
    let rpc_server_handle = match &cli.command {
        Commands::Replay { tx_hash } => {
            tracing::info!("Replaying transaction: {}", tx_hash);
            let tx_hash: TxHash = tx_hash.parse()?;
            replay_transaction(tx_hash, &cli, &effective_rpc_url).await?
        }
        Commands::Test { test_name } => {
            tracing::info!("Debugging test: {}", test_name);
            debug_foundry_test(test_name, &cli, &effective_rpc_url).await?
        }
        Commands::ProxyStatus => unreachable!(), // Handled above
    };

    tracing::info!(
        "Engine preparation complete. RPC server is running on {}",
        rpc_server_handle.addr
    );

    // Launch the selected UI concurrently with the RPC server
    let ui_handle = match cli.ui {
        UiMode::Tui => {
            tracing::info!("Launching Terminal UI...");
            let tui_config = edb_tui::TuiConfig {
                rpc_url: format!("http://{}", rpc_server_handle.addr),
                ..Default::default()
            };

            // Spawn TUI in a separate task
            tokio::spawn(async move {
                if let Err(e) = edb_tui::api::start_tui(tui_config).await {
                    tracing::error!("TUI failed: {}", e);
                }
            })
        }
        UiMode::Web => {
            tracing::info!("Launching Web UI...");
            let webui_config = edb_webui::WebUiConfig {
                port: 3000,
                engine_rpc_url: format!("http://{}", rpc_server_handle.addr),
            };

            // Open browser
            if let Err(e) = webbrowser::open(&format!("http://localhost:{}", webui_config.port)) {
                tracing::warn!("Failed to open browser: {}", e);
                println!("Please open http://localhost:{} in your browser", webui_config.port);
            }

            // Spawn Web UI in a separate task
            tokio::spawn(async move {
                if let Err(e) = edb_webui::api::start_webui(webui_config).await {
                    tracing::error!("Web UI failed: {}", e);
                }
            })
        }
    };

    tracing::info!("Both RPC server and UI are running. Press Ctrl+C to exit.");

    // Wait for either:
    // 1. Ctrl+C signal
    // 2. UI task completion
    // 3. Any other termination signal
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("Received Ctrl+C, shutting down...");
        }
        _ = ui_handle => {
            tracing::info!("UI task completed, shutting down...");
        }
    }

    tracing::info!("Shutting down EDB...");

    // Gracefully shutdown the RPC server
    if let Err(e) = rpc_server_handle.shutdown() {
        tracing::error!("Failed to shutdown RPC server: {}", e);
    } else {
        tracing::info!("RPC server shut down successfully");
    }

    Ok(())
}

/// Replay an existing transaction following the correct architecture
async fn replay_transaction(
    tx_hash: TxHash,
    cli: &Cli,
    rpc_url: &str,
) -> Result<edb_engine::rpc::RpcServerHandle> {
    tracing::info!("Starting transaction replay workflow");

    // Step 1: Fork the chain and replay earlier transactions in the block
    // Fork and prepare the database/environment for the target transaction
    let fork_result = edb_utils::fork_and_prepare(rpc_url, tx_hash, cli.quick).await?;

    tracing::info!(
        "Forked chain and prepared database for transaction replay at block {}",
        fork_result.fork_info.block_number
    );

    // Step 2: Build inputs for the engine
    let engine_config = edb_engine::EngineConfig {
        rpc_port: cli.proxy_port,
        etherscan_api_key: cli.etherscan_api_key.clone(),
        quick: cli.quick,
    };

    // Step 3: Call engine::prepare with forked database and EVM config
    tracing::info!("Calling engine::prepare with prepared inputs");

    // Create the engine and run preparation
    let engine = edb_engine::Engine::new(engine_config);
    engine.prepare(fork_result).await
}

/// Debug a Foundry test case
async fn debug_foundry_test(
    test_name: &str,
    cli: &Cli,
    rpc_url: &str,
) -> Result<edb_engine::rpc::RpcServerHandle> {
    tracing::info!("Starting Foundry test debug workflow");

    // Step 1: Find the transaction hash for the test
    let tx_hash = find_test_transaction(test_name)?;

    // Step 2: Use the same replay workflow as regular transactions
    replay_transaction(tx_hash, cli, rpc_url).await
}

/// Find the transaction hash for a Foundry test
fn find_test_transaction(_test_name: &str) -> Result<TxHash> {
    // TODO: Implement test transaction discovery
    // This would involve:
    // 1. Running the test with foundry
    // 2. Extracting the transaction hash from the test execution
    todo!("Test transaction discovery not yet implemented")
}

/// Show the status of RPC proxy providers
async fn show_proxy_status(cli: &Cli) -> Result<()> {
    use serde_json::json;
    use std::time::Duration;

    tracing::info!("Checking proxy status...");

    // Query provider status
    let client = reqwest::Client::new();
    let request = json!({
        "jsonrpc": "2.0",
        "method": "edb_providers",
        "params": [],
        "id": 1
    });

    let response = client
        .post(&format!("http://127.0.0.1:{}", cli.proxy_port))
        .json(&request)
        .timeout(Duration::from_secs(5))
        .send()
        .await?;

    let response_json: serde_json::Value = response.json().await?;

    if let Some(error) = response_json.get("error") {
        println!("❌ Error getting proxy status: {}", error);
        return Ok(());
    }

    if let Some(result) = response_json.get("result") {
        let healthy_count = result["healthy_count"].as_u64().unwrap_or(0);
        let total_count = result["total_count"].as_u64().unwrap_or(0);
        let empty_providers = vec![];
        let providers = result["providers"].as_array().unwrap_or(&empty_providers);

        println!("🌐 EDB RPC Proxy Status");
        println!("=====================");
        println!("📊 Provider Summary: {}/{} healthy", healthy_count, total_count);
        println!();

        for (i, provider) in providers.iter().enumerate() {
            let url = provider["url"].as_str().unwrap_or("unknown");
            let is_healthy = provider["is_healthy"].as_bool().unwrap_or(false);
            let response_time = provider["response_time_ms"].as_u64();
            let failures = provider["consecutive_failures"].as_u64().unwrap_or(0);
            let last_check = provider["last_health_check_seconds_ago"].as_u64();

            let status_emoji = if is_healthy { "✅" } else { "❌" };
            let status_text = if is_healthy { "Healthy" } else { "Unhealthy" };

            println!("{}. {} {}", i + 1, status_emoji, status_text);
            println!("   URL: {}", url);

            if let Some(rt) = response_time {
                println!("   Response Time: {}ms", rt);
            }

            if failures > 0 {
                println!("   Failures: {}", failures);
            }

            if let Some(last) = last_check {
                if last < 60 {
                    println!("   Last Check: {}s ago", last);
                } else if last < 3600 {
                    println!("   Last Check: {}m ago", last / 60);
                } else {
                    println!("   Last Check: {}h ago", last / 3600);
                }
            }
            println!();
        }

        if healthy_count == 0 {
            println!("⚠️  Warning: No healthy providers available!");
            println!("   The proxy will attempt to health-check providers automatically.");
        } else if healthy_count < total_count {
            println!("⚠️  Some providers are unhealthy but {} are still working.", healthy_count);
        } else {
            println!("✨ All providers are healthy!");
        }
    } else {
        println!("❌ Unexpected response format from proxy");
    }

    Ok(())
}

/// Helper module for browser opening
mod webbrowser {
    use std::process::Command;

    pub fn open(url: &str) -> std::io::Result<()> {
        #[cfg(target_os = "macos")]
        {
            Command::new("open").arg(url).spawn()?;
        }
        #[cfg(target_os = "linux")]
        {
            Command::new("xdg-open").arg(url).spawn()?;
        }
        #[cfg(target_os = "windows")]
        {
            Command::new("cmd").args(["/C", "start", url]).spawn()?;
        }
        Ok(())
    }
}
