//! EDB RPC Proxy Server
//!
//! A caching RPC proxy server that sits between EDB components and real Ethereum RPC endpoints.
//! Provides intelligent caching of immutable RPC responses to improve performance and reduce
//! network overhead for multiple debugging sessions.

use clap::Parser;
use eyre::Result;
use std::net::SocketAddr;
use tracing::{info, warn};

mod cache;
mod health;
mod proxy;
mod registry;
mod rpc;

use proxy::ProxyServer;

#[derive(Parser, Debug)]
#[command(name = "edb-rpc-proxy")]
#[command(about = "EDB RPC Caching Proxy Server")]
struct Args {
    /// Port to listen on
    #[arg(long, default_value = "8546")]
    port: u16,

    /// Upstream RPC URL
    #[arg(long)]
    rpc_url: String,

    /// Maximum number of cached items
    #[arg(long, default_value = "102400")]
    max_cache_items: u32,

    /// Cache directory (default: ~/.edb/cache/rpc/1)
    #[arg(long)]
    cache_dir: Option<String>,

    /// Grace period in seconds before shutdown when no EDB instances (0 = no auto-shutdown)
    #[arg(long, default_value = "30")]
    grace_period: u64,

    /// Heartbeat check interval in seconds
    #[arg(long, default_value = "10")]
    heartbeat_interval: u64,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    info!("Starting EDB RPC Proxy on port {} -> {}", args.port, args.rpc_url);

    // Create the proxy server
    let cache_dir = args.cache_dir.map(std::path::PathBuf::from);
    let proxy = ProxyServer::new(
        args.rpc_url.clone(),
        args.max_cache_items,
        cache_dir,
        args.grace_period,
        args.heartbeat_interval,
    )
    .await?;

    // Set up graceful shutdown
    let cache_manager = proxy.cache_manager().clone();

    // Start the server
    let addr = SocketAddr::from(([127, 0, 0, 1], args.port));

    // Set up shutdown signal handling
    tokio::select! {
        result = proxy.serve(addr) => {
            result?;
        }
        _ = tokio::signal::ctrl_c() => {
            info!("Received shutdown signal");
        }
    }

    // Save cache to disk before exiting
    if let Err(e) = cache_manager.save_to_disk().await {
        warn!("Failed to save cache to disk: {}", e);
    }

    Ok(())
}
