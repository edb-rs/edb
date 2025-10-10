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

//! EDB - Ethereum Debugger
//!
//! A step-by-step debugger for Ethereum transactions.

use std::env;

use alloy_primitives::TxHash;
use clap::{Args, Parser, Subcommand, ValueEnum};
use eyre::Result;

mod cmd;
mod proxy;
mod utils;

/// Command-line interface for EDB
#[derive(Debug, Parser)]
#[command(name = "edb")]
#[command(
    about = "Ethereum Debugger - Source-level time-travel debugger for Ethereum smart contracts"
)]
#[command(version)]
pub struct Cli {
    /// Upstream RPC URLs (comma-separated, overrides defaults if provided).
    ///
    /// Example: --rpc-urls "https://eth.llamarpc.com,https://rpc.ankr.com/eth"
    #[arg(long)]
    rpc_urls: Option<String>,

    /// User interface to use
    #[arg(long, value_enum, default_value = "tui")]
    pub ui: UiMode,

    /// Port for the RPC proxy server
    #[arg(long, default_value = "8546")]
    pub proxy_port: u16,

    /// Etherscan API key for source code download
    #[arg(long, env = "ETHERSCAN_API_KEY")]
    pub etherscan_api_key: Option<String>,

    /// Quick mode - skip replaying preceding transactions in the block
    #[arg(long)]
    pub quick: bool,

    /// Disable cache - do not use cached RPC responses
    #[arg(long)]
    pub disable_cache: bool,

    /// The cache directory
    #[arg(long, env = edb_common::env::EDB_CACHE_DIR)]
    pub cache_dir: Option<String>,

    /// TUI-specific options
    #[command(flatten)]
    pub tui_options: TuiOptions,

    /// Command to execute
    #[command(subcommand)]
    pub command: Commands,
}

impl Cli {
    /// Validate CLI arguments and warn about misused options
    pub fn validate(&self) {
        // Warn if TUI options are used with non-TUI mode
        if !matches!(self.ui, UiMode::Tui) && self.tui_options.disable_mouse {
            tracing::warn!("--disable-mouse flag has no effect when not using TUI mode");
            eprintln!("Warning: --disable-mouse flag has no effect when not using TUI mode");
        }
    }
}

/// TUI-specific options
#[derive(Debug, Args)]
#[command(next_help_heading = "Terminal UI Options (only apply with --ui=tui)")]
pub struct TuiOptions {
    /// Disable mouse support in the terminal UI
    #[arg(long)]
    pub disable_mouse: bool,
}

/// Available UI modes
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum UiMode {
    /// Terminal User Interface
    Tui,
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

        /// Block number to fork at (default: latest)
        block: Option<u64>,
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

    // Validate CLI arguments
    cli.validate();

    if let Some(cache_dir) = &cli.cache_dir {
        tracing::info!("Using cache directory: {cache_dir}");
        env::set_var(edb_common::env::EDB_CACHE_DIR, cache_dir);
    }

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
        Commands::Test { test_name, block } => {
            tracing::info!("Debugging test: {}", test_name);
            cmd::debug_foundry_test(test_name, *block, &cli, &effective_rpc_url).await?
        }
        Commands::ProxyStatus => unreachable!(), // Handled above
    };

    println!("Engine preparation complete. RPC server is running on {}", rpc_server_handle.addr);

    // Launch Terminal UI
    tracing::info!("Launching Terminal UI...");

    // Find the edb-tui binary
    let tui_binary = utils::find_tui_binary()?;
    tracing::debug!("Found TUI binary at: {:?}", tui_binary);

    // Spawn TUI as a child process with inherited stdio
    let mut cmd = std::process::Command::new(&tui_binary);
    cmd.arg("--url").arg(format!("http://{}", rpc_server_handle.addr));

    // Only pass --mouse flag if requested and using TUI mode
    if matches!(cli.ui, UiMode::Tui) && !cli.tui_options.disable_mouse {
        cmd.arg("--mouse");
    }

    let mut child = cmd
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .spawn()
        .map_err(|e| eyre::eyre!("Failed to spawn TUI: {}", e))?;

    // Wait for TUI to exit
    let status = child.wait()?;
    tracing::info!("TUI exited with status: {:?}", status);

    // Return a dummy handle since we're waiting synchronously
    let ui_handle = tokio::spawn(async {});

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
