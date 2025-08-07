//! EDB - Ethereum Debugger
//!
//! A step-by-step debugger for Ethereum transactions.

use alloy_primitives::TxHash;
use clap::{Parser, Subcommand, ValueEnum};
use eyre::Result;
use tracing_subscriber::EnvFilter;

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

    /// Etherscan API key for source code download
    #[arg(long, env = "ETHERSCAN_API_KEY")]
    pub etherscan_api_key: Option<String>,

    /// Quick mode - skip replaying preceding transactions in the block
    #[arg(long)]
    pub quick: bool,

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

    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive("edb=debug".parse()?)
                .add_directive("edb_engine=debug".parse()?)
                .add_directive("edb_tui=debug".parse()?)
                .add_directive("edb_webui=debug".parse()?)
                .add_directive("edb_utils=debug".parse()?),
        )
        .init();

    // Parse CLI arguments
    let cli = Cli::parse();

    // Execute the command
    let analysis_result = match &cli.command {
        Commands::Replay { tx_hash } => {
            tracing::info!("Replaying transaction: {}", tx_hash);
            let tx_hash: TxHash = tx_hash.parse()?;
            replay_transaction(tx_hash, &cli).await?
        }
        Commands::Test { test_name } => {
            tracing::info!("Debugging test: {}", test_name);
            debug_foundry_test(test_name, &cli).await?
        }
    };

    tracing::info!(
        "Analysis complete. Found {} touched contracts",
        analysis_result.touched_contracts.len()
    );

    // Launch the selected UI
    match cli.ui {
        UiMode::Tui => {
            tracing::info!("Launching Terminal UI...");
            let tui_config = edb_tui::TuiConfig {
                rpc_url: format!("http://localhost:{}", cli.port),
                ..Default::default()
            };
            edb_tui::api::start_tui(tui_config).await?;
        }
        UiMode::Web => {
            tracing::info!("Launching Web UI...");
            let webui_config = edb_webui::WebUiConfig {
                port: 3000,
                engine_rpc_url: format!("http://localhost:{}", cli.port),
            };

            // Open browser
            if let Err(e) = webbrowser::open(&format!("http://localhost:{}", webui_config.port)) {
                tracing::warn!("Failed to open browser: {}", e);
                println!("Please open http://localhost:{} in your browser", webui_config.port);
            }

            edb_webui::api::start_webui(webui_config).await?;
        }
    }

    Ok(())
}

/// Replay an existing transaction following the correct architecture
async fn replay_transaction(tx_hash: TxHash, cli: &Cli) -> Result<edb_engine::AnalysisResult> {
    tracing::info!("Starting transaction replay workflow");

    // Step 1: Fork the chain and replay earlier transactions in the block
    // Fork and prepare the database/environment for the target transaction
    let fork_result = edb_utils::fork_and_prepare(&cli.rpc_url, tx_hash, cli.quick).await?;

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

    // Step 3: Call engine::analyze with forked database and EVM config
    tracing::info!("Calling engine::analyze with prepared inputs");

    // Convert utils types to engine placeholders
    // TODO: Once engine is updated to use real types, pass context and tx_env directly
    let database_placeholder = format!(
        "forked_db_chain_{}_block_{}",
        fork_result.fork_info.chain_id, fork_result.fork_info.block_number
    );
    let env_placeholder = format!(
        "env_chain_{}_block_{}_spec_{:?}",
        fork_result.fork_info.chain_id,
        fork_result.fork_info.block_number,
        fork_result.fork_info.spec_id
    );
    let handler_cfg_placeholder = format!("handler_{:?}", fork_result.fork_info.spec_id);

    // Create the engine and run analysis
    let engine = edb_engine::Engine::new(engine_config);
    engine.analyze(tx_hash, database_placeholder, env_placeholder, handler_cfg_placeholder).await
}

/// Debug a Foundry test case
async fn debug_foundry_test(test_name: &str, cli: &Cli) -> Result<edb_engine::AnalysisResult> {
    tracing::info!("Starting Foundry test debug workflow");

    // Step 1: Find the transaction hash for the test
    let tx_hash = find_test_transaction(test_name)?;

    // Step 2: Use the same replay workflow as regular transactions
    replay_transaction(tx_hash, cli).await
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
