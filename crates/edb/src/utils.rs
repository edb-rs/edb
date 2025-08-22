//! Utility functions for the EDB binary

use eyre::{eyre, Result};
use std::path::PathBuf;
use std::process::Command;
use tracing::debug;

/// Find a binary in the following order:
/// 1. Next to the current executable
/// 2. With .exe extension on Windows
/// 3. In the system PATH
pub fn find_binary(name: &str) -> Result<PathBuf> {
    // Try to find binary next to the current executable
    let current_exe = std::env::current_exe()?;
    let binary_path = current_exe
        .parent()
        .ok_or_else(|| eyre!("Could not get parent directory of current executable"))?
        .join(name);

    if binary_path.exists() {
        debug!("Found {} at {:?}", name, binary_path);
        return Ok(binary_path);
    }

    // Try with .exe extension on Windows
    #[cfg(windows)]
    {
        let binary_path_exe = binary_path.with_extension("exe");
        if binary_path_exe.exists() {
            debug!("Found {} at {:?}", name, binary_path_exe);
            return Ok(binary_path_exe);
        }
    }

    // Try to find it in PATH
    #[cfg(unix)]
    {
        if let Ok(output) = Command::new("which").arg(name).output() {
            if output.status.success() {
                let path = String::from_utf8(output.stdout)?.trim().to_string();
                debug!("Found {} in PATH at {}", name, path);
                return Ok(PathBuf::from(path));
            }
        }
    }

    #[cfg(windows)]
    {
        if let Ok(output) = Command::new("where").arg(name).output() {
            if output.status.success() {
                let path = String::from_utf8(output.stdout)?
                    .lines()
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if !path.is_empty() {
                    debug!("Found {} in PATH at {}", name, path);
                    return Ok(PathBuf::from(path));
                }
            }
        }
    }

    Err(eyre!(
        "Could not find {} binary. Make sure it's built and in the same directory as edb or in PATH.",
        name
    ))
}

/// Helper function to find the edb-rpc-proxy binary
pub fn find_proxy_binary() -> Result<PathBuf> {
    find_binary("edb-rpc-proxy")
}

/// Helper function to find the edb-tui binary
pub fn find_tui_binary() -> Result<PathBuf> {
    find_binary("edb-tui")
}

/// Helper function to find the edb-webui binary
pub fn find_webui_binary() -> Result<PathBuf> {
    find_binary("edb-webui")
}
