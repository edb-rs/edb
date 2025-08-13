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
    /// Ethereum RPC endpoint
    #[arg(long, env = "ETH_RPC_URL", default_value = "http://localhost:8545")]
    pub rpc_url: String,

    /// User interface to use
    #[arg(long, value_enum, default_value = "tui")]
    pub ui: UiMode,

    /// Block number to fork at (default: latest)
    #[arg(long)]
    pub block: Option<u64>,

    /// Port for the JSON-RPC server
    #[arg(long, default_value = "8545")]
    pub port: u16,

    /// Port for the RPC proxy server
    #[arg(long, default_value = "8546")]
    pub proxy_port: u16,

    /// Disable RPC caching and connect directly to upstream RPC
    #[arg(long)]
    pub disable_cache: bool,

    /// Grace period in seconds before proxy shutdown when no EDB instances
    #[arg(long, default_value = "30")]
    pub proxy_grace_period: u64,

    /// Force starting a new proxy instance instead of reusing existing
    #[arg(long)]
    pub force_new_proxy: bool,

    /// Heartbeat interval in seconds for EDB instance registration
    #[arg(long, default_value = "10")]
    pub proxy_heartbeat_interval: u64,

    /// Cache directory for RPC proxy (default: ~/.edb/cache/rpc/1)
    #[arg(long)]
    pub cache_dir: Option<String>,

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
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables
    dotenv::dotenv().ok();

    // Parse CLI arguments
    let cli = Cli::parse();

    // Set up RPC endpoint (proxy or direct)
    let effective_rpc_url = if cli.disable_cache {
        tracing::info!("Cache disabled, connecting directly to RPC");
        cli.rpc_url.clone()
    } else {
        tracing::info!("Ensuring RPC proxy is running...");
        proxy::ensure_proxy_running(&cli).await?;
        format!("http://127.0.0.1:{}", cli.proxy_port)
    };

    tracing::info!("Using RPC endpoint: {}", effective_rpc_url);

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
        rpc_port: cli.port,
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
