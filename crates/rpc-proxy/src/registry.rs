//! EDB instance registry for tracking connected debugging sessions

use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, info, warn};

#[derive(Clone)]
struct EDBInstance {
    pid: u32,
    registered_at: u64,
    last_heartbeat: u64,
}

impl EDBInstance {
    fn new(pid: u32) -> Self {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();

        Self { pid, registered_at: now, last_heartbeat: now }
    }

    fn is_alive(&self, heartbeat_timeout: u64) -> bool {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();

        (now - self.last_heartbeat) <= heartbeat_timeout
    }

    fn update_heartbeat(&mut self) {
        self.last_heartbeat =
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    }
}

/// Registry for tracking connected EDB debugging instances
///
/// Manages the lifecycle of EDB instances that use the proxy, including:
/// - Registration and heartbeat tracking
/// - Grace period management for auto-shutdown
/// - Process liveness detection
pub struct EDBRegistry {
    instances: Arc<RwLock<HashMap<u32, EDBInstance>>>,
    grace_period: u64,
    grace_period_start: Arc<RwLock<Option<u64>>>,
    shutdown_tx: broadcast::Sender<()>,
}

impl EDBRegistry {
    /// Creates a new EDB instance registry
    ///
    /// # Arguments
    /// * `grace_period` - Seconds to wait before auto-shutdown when no instances (0 = no auto-shutdown)
    /// * `shutdown_tx` - Channel to send shutdown signals
    ///
    /// # Returns
    /// A new EDBRegistry instance
    pub fn new(grace_period: u64, shutdown_tx: broadcast::Sender<()>) -> Self {
        Self {
            instances: Arc::new(RwLock::new(HashMap::new())),
            grace_period,
            grace_period_start: Arc::new(RwLock::new(None)),
            shutdown_tx,
        }
    }

    /// Registers a new EDB instance with the proxy
    ///
    /// # Arguments
    /// * `pid` - Process ID of the EDB instance
    /// * `_timestamp` - Registration timestamp (currently unused)
    ///
    /// # Returns
    /// JSON-RPC response confirming registration
    pub async fn register_edb_instance(&self, pid: u32, _timestamp: u64) -> Value {
        let mut instances = self.instances.write().await;
        let instance = EDBInstance::new(pid);

        instances.insert(pid, instance);
        info!("Registered EDB instance: PID {}", pid);

        // Cancel grace period if we were in one
        let mut grace_start = self.grace_period_start.write().await;
        if grace_start.is_some() {
            info!("Cancelled grace period due to new EDB instance");
            *grace_start = None;
        }

        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "status": "registered",
                "pid": pid
            }
        })
    }

    /// Processes a heartbeat from an EDB instance
    ///
    /// Updates the last heartbeat time for the instance to keep it alive.
    ///
    /// # Arguments
    /// * `pid` - Process ID of the EDB instance sending heartbeat
    ///
    /// # Returns
    /// JSON-RPC response with heartbeat acknowledgment or error
    pub async fn heartbeat(&self, pid: u32) -> Value {
        let mut instances = self.instances.write().await;

        if let Some(instance) = instances.get_mut(&pid) {
            instance.update_heartbeat();
            debug!("Heartbeat from EDB instance: PID {}", pid);

            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "result": {
                    "status": "ok",
                    "pid": pid,
                    "registered_at": instance.registered_at
                }
            })
        } else {
            warn!("Heartbeat from unknown EDB instance: PID {}", pid);

            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "error": {
                    "code": -32000,
                    "message": "Unknown EDB instance"
                }
            })
        }
    }

    /// Starts the background heartbeat monitoring task
    ///
    /// Runs indefinitely to:
    /// - Clean up dead/unresponsive EDB instances
    /// - Manage grace period for auto-shutdown
    /// - Monitor process liveness
    ///
    /// # Arguments
    /// * `check_interval` - Seconds between heartbeat checks
    pub async fn start_heartbeat_monitor(&self, check_interval: u64) {
        let instances = Arc::clone(&self.instances);
        let grace_period_start = Arc::clone(&self.grace_period_start);
        let grace_period = self.grace_period;

        let mut interval = tokio::time::interval(Duration::from_secs(check_interval));

        loop {
            interval.tick().await;

            // Clean up dead instances
            let _dead_instances = {
                let mut instances = instances.write().await;
                let heartbeat_timeout = check_interval * 3; // 3 missed heartbeats = dead

                let dead_pids: Vec<u32> = instances
                    .iter()
                    .filter(|(_, instance)| {
                        !instance.is_alive(heartbeat_timeout)
                            || !Self::is_process_alive(instance.pid)
                    })
                    .map(|(pid, _)| *pid)
                    .collect();

                for pid in &dead_pids {
                    instances.remove(pid);
                    info!("Removed dead EDB instance: PID {}", pid);
                }

                dead_pids
            };

            // Check if we should start grace period
            let should_start_grace = {
                let instances = instances.read().await;
                instances.is_empty()
            };

            if should_start_grace {
                let mut grace_start = grace_period_start.write().await;

                if grace_start.is_none() {
                    let now =
                        SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();

                    *grace_start = Some(now);
                    info!("Started grace period - no active EDB instances");
                }

                // Check if grace period has expired (0 means no auto-shutdown)
                if grace_period > 0 {
                    if let Some(start_time) = *grace_start {
                        let now = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();

                        if (now - start_time) >= grace_period {
                            warn!("Grace period expired - sending shutdown signal");
                            let _ = self.shutdown_tx.send(());
                        }
                    }
                }
            }
        }
    }

    fn is_process_alive(pid: u32) -> bool {
        #[cfg(unix)]
        {
            use std::process::Command;

            // Use `kill -0` to check if process exists without actually killing it
            Command::new("kill")
                .args(["-0", &pid.to_string()])
                .output()
                .map(|output| output.status.success())
                .unwrap_or(false)
        }

        #[cfg(windows)]
        {
            use std::process::Command;

            // Use tasklist to check if process exists
            Command::new("tasklist")
                .args(["/FI", &format!("PID eq {}", pid)])
                .output()
                .map(|output| {
                    output.status.success()
                        && String::from_utf8_lossy(&output.stdout).contains(&pid.to_string())
                })
                .unwrap_or(false)
        }
    }

    /// Returns a list of currently active EDB instance PIDs
    ///
    /// # Returns
    /// Vector of process IDs for all registered instances
    pub async fn get_active_instances(&self) -> Vec<u32> {
        let instances = self.instances.read().await;
        instances.keys().cloned().collect()
    }
}
