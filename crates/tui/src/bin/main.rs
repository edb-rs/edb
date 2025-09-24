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

//! EDB TUI - Terminal User Interface for EDB
//!
//! This binary provides a standalone TUI client that connects to an EDB RPC server.

use clap::Parser;
use edb_common::logging;
use edb_tui::{Config, TuiConfig};
use eyre::Result;
use std::path::PathBuf;

/// EDB Terminal User Interface
#[derive(Debug, Parser)]
#[command(name = "edb-tui")]
#[command(about = "Terminal User Interface for EDB debugger", version)]
struct Args {
    /// RPC server URL
    #[arg(long, default_value = "http://localhost:3030")]
    url: String,

    /// Config file path (uses ~/.edb.toml if not specified)
    #[arg(long)]
    config: Option<PathBuf>,

    /// Enable mouse support
    #[arg(long)]
    mouse: bool,

    /// Terminal refresh interval in milliseconds
    #[arg(long, default_value = "50")]
    refresh_interval: u64,

    /// Data fetch interval in milliseconds
    #[arg(long, default_value = "100")]
    data_fetch_interval: u64,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Setup file-only logging for TUI (logs go to file, not terminal)
    let log_file_path = logging::init_file_only_logging("edb-tui")?;

    // Print log file location to stderr (so user knows where logs are)
    // Use stderr so it doesn't interfere with TUI if there are issues
    eprintln!("EDB TUI logs: {}", log_file_path.display());

    // TODO: use the config in TUI
    // Load configuration
    let _config = if let Some(config_path) = args.config {
        // Load from specified path
        Config::load_from_path(config_path)?
    } else {
        // Load from default path or create default
        Config::load().unwrap_or_default()
    };

    // Create TUI configuration
    let tui_config = TuiConfig {
        rpc_url: args.url.clone(),
        refresh_interval: std::time::Duration::from_millis(args.refresh_interval),
        data_fetch_interval: std::time::Duration::from_millis(args.data_fetch_interval),
        enable_mouse: args.mouse,
    };

    tracing::info!("Starting EDB TUI");
    tracing::info!("Connecting to RPC server at: {}", args.url);

    // Start the TUI
    match edb_tui::api::start_tui(tui_config).await {
        Ok(_) => {
            tracing::info!("TUI exited normally");
            Ok(())
        }
        Err(e) => {
            tracing::error!("TUI error: {}", e);
            Err(e)
        }
    }
}
