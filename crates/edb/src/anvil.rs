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

//! Anvil process management for local debugging workflows.

use std::process::Stdio;

use eyre::{Context, Result, bail};
use tokio::process::{Child, Command};

/// Handle to an anvil process. Drops and kills the process when owned.
pub struct AnvilHandle {
    child: Option<Child>,
    port: u16,
    owned: bool,
}

impl AnvilHandle {
    /// Get the RPC URL for this anvil instance.
    pub fn rpc_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// Get the port this anvil instance is running on.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Check if this handle owns the anvil process (i.e., we spawned it).
    pub fn is_owned(&self) -> bool {
        self.owned
    }
}

impl Drop for AnvilHandle {
    fn drop(&mut self) {
        if self.owned {
            if let Some(mut child) = self.child.take() {
                tracing::info!("Killing anvil process on port {}", self.port);
                let _ = child.start_kill();
            }
        }
    }
}

/// Ensure anvil is running on the specified port.
///
/// If anvil is already running and healthy, returns a handle with `owned = false`.
/// Otherwise, spawns a new anvil process with appropriate flags for debugging.
pub async fn ensure_anvil_running(port: u16) -> Result<AnvilHandle> {
    // Check if anvil is already running
    if anvil_health_check(port).await.is_ok() {
        tracing::info!("Anvil already running on port {port}");
        return Ok(AnvilHandle { child: None, port, owned: false });
    }

    // Spawn a new anvil instance
    tracing::info!("Starting anvil on port {port}...");
    let child = spawn_anvil(port).await?;

    // Wait for anvil to be ready
    wait_for_anvil_ready(port).await?;

    tracing::info!("Anvil ready on port {port}");
    Ok(AnvilHandle { child: Some(child), port, owned: true })
}

/// Check if anvil is running and responsive on the given port.
async fn anvil_health_check(port: u16) -> Result<()> {
    let rpc_url = format!("http://127.0.0.1:{port}");

    // Use cast to check block number as a health check
    let output = tokio::process::Command::new("cast")
        .args(["block-number", "--rpc-url", &rpc_url])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .wrap_err("failed to execute cast")?;

    if output.success() {
        Ok(())
    } else {
        bail!("anvil not responding on port {port}")
    }
}

/// Spawn a new anvil process with debugging-appropriate flags.
async fn spawn_anvil(port: u16) -> Result<Child> {
    // Check if anvil is installed
    if Command::new("anvil").arg("--version").output().await.is_err() {
        bail!(
            "anvil not found. Please install Foundry: https://book.getfoundry.sh/getting-started/installation"
        );
    }

    let child = Command::new("anvil")
        .args([
            "--port",
            &port.to_string(),
            "--steps-tracing",
            "--code-size-limit",
            "1000000",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .wrap_err("failed to spawn anvil")?;

    Ok(child)
}

/// Wait for anvil to become ready by polling the health check.
async fn wait_for_anvil_ready(port: u16) -> Result<()> {
    const MAX_ATTEMPTS: usize = 30;
    const DELAY_MS: u64 = 500;

    for attempt in 1..=MAX_ATTEMPTS {
        if anvil_health_check(port).await.is_ok() {
            return Ok(());
        }

        if attempt == MAX_ATTEMPTS {
            bail!("anvil did not become ready after {} attempts", MAX_ATTEMPTS);
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(DELAY_MS)).await;
    }

    Ok(())
}
