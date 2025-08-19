//! EDB - Ethereum Debugger
//!
//! A step-by-step debugger for Ethereum transactions.

use alloy_primitives::TxHash;
use clap::{Parser, Subcommand, ValueEnum};
use eyre::Result;

mod cmd;
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
    edb_common::logging::init_logging("edb", true)?;

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
        return cmd::show_proxy_status(&cli).await;
    }

    // Execute the command to get RPC server handle
    let rpc_server_handle = match &cli.command {
        Commands::Replay { tx_hash } => {
            tracing::info!("Replaying transaction: {}", tx_hash);
            let tx_hash: TxHash = tx_hash.parse()?;
            cmd::replay_transaction(tx_hash, &cli, &effective_rpc_url).await?
        }
        Commands::Test { test_name } => {
            tracing::info!("Debugging test: {}", test_name);
            cmd::debug_foundry_test(test_name, &cli, &effective_rpc_url).await?
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
