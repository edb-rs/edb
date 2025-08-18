//! RPC Proxy management for EDB

use crate::Cli;
use eyre::{eyre, Result};
use serde_json::{json, Value};
use std::process::{Command, Stdio};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, info, warn};

const PROXY_HEARTBEAT_INTERVAL: u64 = 10;
const PROXY_GRACE_PERIOD: u64 = 30;

pub async fn ensure_proxy_running(cli: &Cli) -> Result<()> {
    // Check if proxy already exists and is healthy
    match proxy_health_check(cli.proxy_port).await {
        Ok(_) => {
            info!("Found healthy proxy at port {}", cli.proxy_port);
            register_with_proxy(cli.proxy_port).await?;
            start_heartbeat_task(cli.proxy_port, PROXY_HEARTBEAT_INTERVAL);
            return Ok(());
        }
        Err(e) => {
            debug!("Proxy health check failed: {}", e);
        }
    }

    // Spawn new one
    info!("No healthy proxy found, spawning new instance");
    spawn_proxy(cli).await?;

    // Wait for proxy to be ready
    wait_for_proxy_ready(cli.proxy_port).await?;

    // Register with the new proxy
    register_with_proxy(cli.proxy_port).await?;

    // Start heartbeat task
    start_heartbeat_task(cli.proxy_port, PROXY_HEARTBEAT_INTERVAL);

    Ok(())
}

async fn proxy_health_check(port: u16) -> Result<Value> {
    let client = reqwest::Client::new();
    let request = json!({
        "jsonrpc": "2.0",
        "method": "edb_ping",
        "params": [],
        "id": 1
    });

    let response = client
        .post(&format!("http://127.0.0.1:{}", port))
        .json(&request)
        .timeout(Duration::from_secs(2))
        .send()
        .await?;

    let response_json: Value = response.json().await?;

    if response_json.get("error").is_some() {
        return Err(eyre!("Proxy returned error: {:?}", response_json["error"]));
    }

    Ok(response_json)
}

async fn register_with_proxy(port: u16) -> Result<()> {
    let client = reqwest::Client::new();
    let pid = std::process::id();
    let timestamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs();

    let request = json!({
        "jsonrpc": "2.0",
        "method": "edb_register",
        "params": [pid, timestamp],
        "id": 1
    });

    let response = client
        .post(&format!("http://127.0.0.1:{}", port))
        .json(&request)
        .timeout(Duration::from_secs(5))
        .send()
        .await?;

    let response_json: Value = response.json().await?;

    if let Some(error) = response_json.get("error") {
        return Err(eyre!("Failed to register with proxy: {:?}", error));
    }

    info!("Successfully registered with proxy (PID: {})", pid);
    Ok(())
}

fn start_heartbeat_task(port: u16, interval: u64) {
    tokio::spawn(async move {
        let client = reqwest::Client::new();
        let pid = std::process::id();
        let mut interval_timer = tokio::time::interval(Duration::from_secs(interval));

        loop {
            interval_timer.tick().await;

            let request = json!({
                "jsonrpc": "2.0",
                "method": "edb_heartbeat",
                "params": [pid],
                "id": 1
            });

            if let Err(e) = client
                .post(&format!("http://127.0.0.1:{}", port))
                .json(&request)
                .timeout(Duration::from_secs(5))
                .send()
                .await
            {
                warn!("Heartbeat failed: {}", e);
            } else {
                debug!("Sent heartbeat to proxy");
            }
        }
    });
}

async fn spawn_proxy(cli: &Cli) -> Result<()> {
    let proxy_binary = find_proxy_binary()?;

    info!("Spawning proxy binary: {:?}", proxy_binary);

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;

        let mut args = vec![
            "server".to_string(), // Add the server subcommand
            "--port".to_string(),
            cli.proxy_port.to_string(),
            "--grace-period".to_string(),
            PROXY_GRACE_PERIOD.to_string(),
            "--heartbeat-interval".to_string(),
            PROXY_HEARTBEAT_INTERVAL.to_string(),
        ];

        // Add RPC URLs if provided, otherwise proxy will use defaults
        if let Some(rpc_urls) = &cli.rpc_urls {
            args.push("--rpc-urls".to_string());
            args.push(rpc_urls.clone());
        }

        Command::new(&proxy_binary)
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0) // Create new process group (detached)
            .spawn()
            .map_err(|e| eyre!("Failed to spawn proxy: {}", e))?;
    }

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;

        let mut args = vec![
            "server".to_string(), // Add the server subcommand
            "--port".to_string(),
            cli.proxy_port.to_string(),
            "--grace-period".to_string(),
            PROXY_GRACE_PERIOD.to_string(),
            "--heartbeat-interval".to_string(),
            PROXY_HEARTBEAT_INTERVAL.to_string(),
        ];

        // Add RPC URLs if provided, otherwise proxy will use defaults
        if let Some(rpc_urls) = &cli.rpc_urls {
            args.push("--rpc-urls".to_string());
            args.push(rpc_urls.clone());
        }

        Command::new(&proxy_binary)
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(0x00000008) // DETACHED_PROCESS
            .spawn()
            .map_err(|e| eyre!("Failed to spawn proxy: {}", e))?;
    }

    info!("Proxy process spawned successfully");
    Ok(())
}

fn find_proxy_binary() -> Result<std::path::PathBuf> {
    // Try to find edb-rpc-proxy binary next to the current executable
    let current_exe = std::env::current_exe()?;
    let proxy_binary = current_exe
        .parent()
        .ok_or_else(|| eyre!("Could not get parent directory of current executable"))?
        .join("edb-rpc-proxy");

    if proxy_binary.exists() {
        return Ok(proxy_binary);
    }

    // Try with .exe extension on Windows
    #[cfg(windows)]
    {
        let proxy_binary_exe = proxy_binary.with_extension("exe");
        if proxy_binary_exe.exists() {
            return Ok(proxy_binary_exe);
        }
    }

    // Try to find it in PATH
    if let Ok(output) = Command::new("which").arg("edb-rpc-proxy").output() {
        if output.status.success() {
            let path = String::from_utf8(output.stdout)?.trim().to_string();
            return Ok(std::path::PathBuf::from(path));
        }
    }

    Err(eyre!("Could not find edb-rpc-proxy binary. Make sure it's built and in the same directory as edb or in PATH."))
}

async fn wait_for_proxy_ready(port: u16) -> Result<()> {
    let max_attempts = 30; // 30 seconds total

    for attempt in 1..=max_attempts {
        match proxy_health_check(port).await {
            Ok(_) => {
                info!("Proxy is ready on port {}", port);
                return Ok(());
            }
            Err(e) => {
                debug!("Proxy not ready (attempt {}/{}): {}", attempt, max_attempts, e);

                if attempt == max_attempts {
                    return Err(eyre!("Proxy failed to start within {} seconds", max_attempts));
                }

                sleep(Duration::from_secs(1)).await;
            }
        }
    }

    unreachable!()
}
