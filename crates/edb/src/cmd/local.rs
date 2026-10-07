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

//! Local command - automated local debugging workflow

use std::path::{Path, PathBuf};

use alloy_primitives::{Address, Bytes, TxHash};
use edb_common::fork_and_prepare;
use edb_engine::{DiscoveredContract, Engine, LocalContractConfig, LocalSourceConfig};
use eyre::{Context, Result, bail};

use crate::{Ui, anvil, proxy, utils};

/// Run the complete local debugging workflow.
pub async fn run_local_workflow(
    project_path: PathBuf,
    no_anvil: bool,
    anvil_port: u16,
    no_deploy: bool,
    contract: Option<String>,
    tx_hash: Option<String>,
    script: Option<String>,
    cli: &crate::Cli,
) -> Result<()> {
    tracing::info!("Starting local debugging workflow for {:?}", project_path);

    // 1. Validate project path
    let project_root = validate_project_path(&project_path)?;
    tracing::info!("Project root: {:?}", project_root);

    // 2. Start anvil if needed
    let anvil_handle = if no_anvil {
        tracing::info!("Skipping anvil startup (--no-anvil)");
        // Verify anvil is running
        anvil::ensure_anvil_running(anvil_port).await?
    } else {
        anvil::ensure_anvil_running(anvil_port).await?
    };

    let rpc_url = anvil_handle.rpc_url();
    tracing::info!("Using RPC URL: {rpc_url}");

    // 3. Ensure proxy is running (configured to forward to local anvil)
    tracing::info!("Ensuring RPC proxy is running...");
    proxy::ensure_proxy_running_with_rpc_url(cli, Some(&rpc_url)).await?;
    let proxy_rpc_url = format!("http://127.0.0.1:{}", cli.proxy_port);

    // 4. Discover or load contracts
    let local_config = if no_deploy {
        tracing::info!("Skipping deployment (--no-deploy)");
        load_existing_config(&project_root)?
    } else {
        discover_contracts(&project_root, contract.as_deref(), script.as_deref()).await?
    };

    tracing::info!(
        "Loaded {} contracts for debugging",
        local_config.contracts.len()
    );

    // 5. Execute test transaction or use provided tx hash
    let target_tx_hash = if let Some(hash_str) = tx_hash {
        hash_str.parse::<TxHash>().wrap_err("invalid transaction hash")?
    } else {
        execute_test_transaction(&project_root, &rpc_url).await?
    };

    tracing::info!("Target transaction: {target_tx_hash}");

    // 6. Build engine config with local source
    let mut engine_config = cli.to_engine_config(&proxy_rpc_url);
    engine_config = engine_config.with_local_source(local_config);

    // 7. Fork and prepare
    tracing::info!("Forking chain and preparing database...");
    let fork_result = fork_and_prepare(&proxy_rpc_url, target_tx_hash, cli.quick).await?;
    tracing::info!(
        "Forked at block {} for transaction replay",
        fork_result.fork_info.block_number
    );

    // 8. Create engine and prepare
    let engine = Engine::new(engine_config);
    let rpc_server_addr = match cli.ui {
        Ui::Tui => engine.prepare(fork_result, None).await?,
        Ui::Web => engine.prepare_with_router(fork_result, None, Some(edb_web::router())).await?,
    };

    // 9. Launch UI
    match cli.ui {
        Ui::Tui => {
            utils::start_tui(&cli.tui_options, rpc_server_addr).await?;
        }
        Ui::Web => {
            let url = format!("http://{rpc_server_addr}/");
            utils::open_browser(&url);
            tracing::info!("Web UI ready. Press Ctrl+C to exit.");
            tokio::signal::ctrl_c().await?;
        }
    }

    // 10. Cleanup
    tracing::info!("Shutting down EDB...");
    engine.shutdown_rpc_server(&target_tx_hash)?;

    // Anvil handle drops here, killing the process if we spawned it
    drop(anvil_handle);

    Ok(())
}

/// Validate that the project path exists and contains foundry.toml.
fn validate_project_path(path: &Path) -> Result<PathBuf> {
    if !path.exists() {
        bail!("Project path does not exist: {:?}", path);
    }

    let project_root = if path.is_file() {
        // If a file was provided, use its parent directory
        path.parent()
            .ok_or_else(|| eyre::eyre!("invalid project path"))?
            .to_path_buf()
    } else {
        path.to_path_buf()
    };

    let foundry_toml = project_root.join("foundry.toml");
    if !foundry_toml.exists() {
        bail!(
            "Not a Foundry project (no foundry.toml): {:?}\n\
             Please run this command from a Foundry project directory.",
            project_root
        );
    }

    Ok(project_root)
}

/// Load an existing edb.local.json config file.
fn load_existing_config(project_root: &Path) -> Result<LocalSourceConfig> {
    let config_path = project_root.join("edb.local.json");
    if !config_path.exists() {
        bail!(
            "No edb.local.json found at {:?}\n\
             Please create the config file or remove --no-deploy.",
            config_path
        );
    }

    tracing::info!("Loading config from {:?}", config_path);
    LocalSourceConfig::load(&config_path)
}

/// Discover contracts from various sources.
async fn discover_contracts(
    project_root: &Path,
    contract_name: Option<&str>,
    script: Option<&str>,
) -> Result<LocalSourceConfig> {
    // Strategy 1: Check for edb.local.json
    let config_path = project_root.join("edb.local.json");
    if config_path.exists() {
        tracing::info!("Found edb.local.json, using it");
        return LocalSourceConfig::load(&config_path);
    }

    // Strategy 2: Parse broadcast directory
    let broadcast_dir = project_root.join("broadcast");
    if broadcast_dir.exists() {
        tracing::info!("Parsing broadcast directory for deployed contracts");
        if let Ok(contracts) = parse_broadcast_dir(&broadcast_dir, contract_name) {
            if !contracts.is_empty() {
                return Ok(LocalSourceConfig::from_discovered(
                    project_root.to_path_buf(),
                    None,
                    contracts,
                ));
            }
        }
    }

    // Strategy 3: Run forge script if specified
    if let Some(script_path) = script {
        tracing::info!("Running forge script: {script_path}");
        let contracts = run_forge_script(project_root, script_path).await?;
        return Ok(LocalSourceConfig::from_discovered(
            project_root.to_path_buf(),
            None,
            contracts,
        ));
    }

    bail!(
        "No contracts found. Please provide one of:\n\
         - edb.local.json in the project root\n\
         - Deployed contracts in broadcast/ directory\n\
         - --script <path> to run a deployment script"
    )
}

/// Parse Foundry's broadcast directory to extract deployed contracts.
fn parse_broadcast_dir(broadcast_dir: &Path, contract_name: Option<&str>) -> Result<Vec<DiscoveredContract>> {
    let mut contracts = Vec::new();

    // Walk through broadcast/<script>/<chain-id>/run-latest.json
    for entry in std::fs::read_dir(broadcast_dir).wrap_err("failed to read broadcast directory")? {
        let entry = entry?;
        let script_dir = entry.path();
        if !script_dir.is_dir() {
            continue;
        }

        // Look for chain ID directories (e.g., 31337 for anvil)
        for chain_entry in std::fs::read_dir(&script_dir)? {
            let chain_entry = chain_entry?;
            let chain_dir = chain_entry.path();
            if !chain_dir.is_dir() {
                continue;
            }

            let run_latest = chain_dir.join("run-latest.json");
            if !run_latest.exists() {
                continue;
            }

            // Parse the run-latest.json file
            let content = std::fs::read_to_string(&run_latest)?;
            let run_data: serde_json::Value = serde_json::from_str(&content)?;

            if let Some(transactions) = run_data.get("transactions").and_then(|t| t.as_array()) {
                for tx in transactions {
                    let tx_type = tx.get("transactionType").and_then(|t| t.as_str()).unwrap_or("");

                    // Only interested in CREATE transactions (contract deployments)
                    if tx_type != "CREATE" {
                        continue;
                    }

                    let contract_addr = tx
                        .get("contractAddress")
                        .and_then(|a| a.as_str())
                        .and_then(|s| s.parse::<Address>().ok());

                    let name = tx.get("contractName").and_then(|n| n.as_str()).map(String::from);

                    let tx_hash = tx
                        .get("hash")
                        .and_then(|h| h.as_str())
                        .and_then(|s| s.parse::<TxHash>().ok());

                    if let (Some(address), Some(name), Some(hash)) = (contract_addr, name, tx_hash) {
                        // Filter by contract name if specified
                        if let Some(filter) = contract_name {
                            if name != filter {
                                continue;
                            }
                        }

                        // Try to find the source file
                        let source = find_source_file(tx.get("arguments"), &name)?;

                        // Extract constructor arguments (hex-encoded)
                        let constructor_args = tx
                            .get("arguments")
                            .and_then(|a| a.as_str())
                            .map(|s| {
                                let hex_str = s.trim_start_matches("0x");
                                format!("0x{hex_str}").parse::<Bytes>().unwrap_or_default()
                            })
                            .unwrap_or_default();

                        contracts.push(DiscoveredContract {
                            address,
                            name,
                            source,
                            creation_tx: hash,
                            constructor_args,
                        });
                    }
                }
            }
        }
    }

    Ok(contracts)
}

/// Find the source file for a contract.
fn find_source_file(_arguments: Option<&serde_json::Value>, contract_name: &str) -> Result<PathBuf> {
    // Simple heuristic: assume src/<ContractName>.sol
    // In a real implementation, we'd parse the compilation artifacts
    Ok(PathBuf::from(format!("src/{contract_name}.sol")))
}

/// Run a forge script and parse the output for deployed contracts.
async fn run_forge_script(project_root: &Path, script_path: &str) -> Result<Vec<DiscoveredContract>> {
    // This is a placeholder - in a real implementation, we'd:
    // 1. Run `forge script <script_path> --broadcast --json`
    // 2. Parse the output for deployed contracts
    // 3. Extract addresses and tx hashes

    bail!(
        "forge script execution not yet implemented.\n\
         Please use edb.local.json or broadcast/ directory instead.\n\
         Script requested: {script_path}"
    )
}

/// Execute a test transaction and return its hash.
async fn execute_test_transaction(project_root: &Path, rpc_url: &str) -> Result<TxHash> {
    // Check if there's a test transaction defined in edb.local.json
    let config_path = project_root.join("edb.local.json");
    if config_path.exists() {
        let content = std::fs::read_to_string(&config_path)?;
        let config: serde_json::Value = serde_json::from_str(&content)?;

        if let Some(test_tx) = config.get("test_transaction") {
            tracing::info!("Executing test transaction from config");
            return execute_cast_command(test_tx, rpc_url).await;
        }
    }

    // Try to run a test script
    let test_script = project_root.join("script/Test.s.sol");
    if test_script.exists() {
        tracing::info!("Running test script: script/Test.s.sol");
        return run_test_script(project_root, "script/Test.s.sol", rpc_url).await;
    }

    bail!(
        "No test transaction found. Please provide one of:\n\
         - test_transaction in edb.local.json\n\
         - script/Test.s.sol\n\
         - --tx-hash <hash> to specify the transaction directly"
    )
}

/// Execute a cast command to send a transaction.
async fn execute_cast_command(test_tx: &serde_json::Value, rpc_url: &str) -> Result<TxHash> {
    let to = test_tx
        .get("to")
        .and_then(|t| t.as_str())
        .ok_or_else(|| eyre::eyre!("test_transaction.to is required"))?;

    let function = test_tx
        .get("function")
        .and_then(|f| f.as_str())
        .ok_or_else(|| eyre::eyre!("test_transaction.function is required"))?;

    let args = test_tx
        .get("args")
        .and_then(|a| a.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    // Build cast command
    let mut cmd = tokio::process::Command::new("cast");
    cmd.args(["send", "--rpc-url", rpc_url, "--private-key", "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"]);
    cmd.arg(to);
    cmd.arg(function);
    cmd.args(args);
    cmd.arg("--json");

    let output = cmd.output().await.wrap_err("failed to execute cast")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("cast send failed: {stderr}");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let result: serde_json::Value = serde_json::from_str(&stdout)?;

    let tx_hash = result
        .get("transactionHash")
        .and_then(|h| h.as_str())
        .and_then(|s| s.parse::<TxHash>().ok())
        .ok_or_else(|| eyre::eyre!("failed to parse transaction hash from cast output"))?;

    Ok(tx_hash)
}

/// Run a test script and extract the transaction hash.
async fn run_test_script(project_root: &Path, script_path: &str, rpc_url: &str) -> Result<TxHash> {
    // This is a placeholder - in a real implementation, we'd:
    // 1. Run `forge script <script_path> --rpc-url <rpc_url> --broadcast --json`
    // 2. Parse the output for the transaction hash

    bail!(
        "test script execution not yet implemented.\n\
         Please use --tx-hash to specify the transaction directly,\n\
         or define test_transaction in edb.local.json."
    )
}

/// Generate template setup scripts for local debugging.
///
/// Creates an `edb.local.json` config template and a `edb-setup.sh` script in the
/// project root. Users customize these to match their contracts and test transactions.
pub fn init_local_project(project_path: PathBuf) -> Result<()> {
    let project_root = validate_project_path(&project_path)?;

    let config_path = project_root.join("edb.local.json");
    let setup_path = project_root.join("edb-setup.sh");

    if config_path.exists() && setup_path.exists() {
        bail!(
            "Template files already exist:\n\
             - {:?}\n\
             - {:?}\n\
             Remove them first to regenerate.",
            config_path,
            setup_path
        );
    }

    if !config_path.exists() {
        let config_template = r#"{
  "project_root": ".",
  "solc_version": "0.8.26",
  "contracts": [
    {
      "address": "0x_REPLACE_WITH_DEPLOYED_ADDRESS",
      "name": "YourContract",
      "source": "src/YourContract.sol",
      "creation_tx": "0x_REPLACE_WITH_DEPLOY_TX_HASH",
      "constructor_args": "0x"
    }
  ],
  "test_transaction": {
    "to": "0x_REPLACE_WITH_CONTRACT_ADDRESS",
    "function": "yourFunction(uint256)",
    "args": ["42"]
  }
}
"#;
        std::fs::write(&config_path, config_template)
            .wrap_err_with(|| format!("failed to write {:?}", config_path))?;
        println!("Created {:?}", config_path);
    }

    if !setup_path.exists() {
        let setup_template = r#"#!/usr/bin/env bash
# EDB local debugging setup script
#
# This script:
#   1. Starts anvil (local Ethereum node)
#   2. Deploys your contracts
#   3. Executes a test transaction
#   4. Outputs the transaction hash for `edb local --tx-hash`
#
# Customize the TODO sections below for your project.
set -euo pipefail

ANVIL_PORT="${ANVIL_PORT:-8545}"
RPC_URL="http://127.0.0.1:${ANVIL_PORT}"

# --- 1. Start anvil ---
echo "Starting anvil on port ${ANVIL_PORT}..."
anvil --port "${ANVIL_PORT}" &
ANVIL_PID=$!
trap 'kill $ANVIL_PID 2>/dev/null' EXIT
sleep 2

# --- 2. Deploy contracts ---
# TODO: Replace with your deployment command.
# Option A: forge create
#   CONTRACT_ADDR=$(forge create src/YourContract.sol:YourContract \
#     --rpc-url "$RPC_URL" \
#     --private-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 \
#     --json | jq -r '.deployedTo')
#   echo "Deployed at: $CONTRACT_ADDR"
#
# Option B: forge script
#   forge script script/Deploy.s.sol \
#     --rpc-url "$RPC_URL" \
#     --broadcast
echo "TODO: Add your deployment commands"

# --- 3. Execute a test transaction ---
# TODO: Replace with the transaction you want to debug.
# cast send "$CONTRACT_ADDR" \
#   "yourFunction(uint256)" \
#   42 \
#   --rpc-url "$RPC_URL" \
#   --private-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80
echo "TODO: Add your test transaction commands"

echo ""
echo "Setup complete. Now run:"
echo "  edb local . --no-anvil --no-deploy --tx-hash <TX_HASH>"
"#;
        std::fs::write(&setup_path, setup_template)
            .wrap_err_with(|| format!("failed to write {:?}", setup_path))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&setup_path)?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&setup_path, perms)?;
        }
        println!("Created {:?}", setup_path);
    }

    println!();
    println!("Next steps:");
    println!("  1. Edit edb.local.json with your contract details");
    println!("  2. Edit edb-setup.sh with your deployment & test transaction");
    println!("  3. Run: ./edb-setup.sh");
    println!("  4. Run: edb local . --tx-hash <TX_HASH>");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_validate_project_path_valid() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("foundry.toml"), "[profile.default]\n").unwrap();
        let result = validate_project_path(tmp.path());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), tmp.path());
    }

    #[test]
    fn test_validate_project_path_missing() {
        let result = validate_project_path(Path::new("/nonexistent/path"));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("does not exist"));
    }

    #[test]
    fn test_validate_project_path_no_foundry_toml() {
        let tmp = TempDir::new().unwrap();
        let result = validate_project_path(tmp.path());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Not a Foundry project"));
    }

    #[test]
    fn test_validate_project_path_file_input() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("foundry.toml"), "[profile.default]\n").unwrap();
        let config_file = tmp.path().join("edb.local.json");
        fs::write(&config_file, "{}").unwrap();
        let result = validate_project_path(&config_file);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), tmp.path());
    }

    #[test]
    fn test_parse_broadcast_dir_empty() {
        let tmp = TempDir::new().unwrap();
        let broadcast_dir = tmp.path().join("broadcast");
        fs::create_dir(&broadcast_dir).unwrap();
        let result = parse_broadcast_dir(&broadcast_dir, None);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_parse_broadcast_dir_with_deployments() {
        let tmp = TempDir::new().unwrap();
        let broadcast_dir = tmp.path().join("broadcast");
        let script_dir = broadcast_dir.join("Deploy.s.sol");
        let chain_dir = script_dir.join("31337");
        fs::create_dir_all(&chain_dir).unwrap();

        let run_latest = serde_json::json!({
            "transactions": [
                {
                    "transactionType": "CREATE",
                    "contractAddress": "0x5fbdb2315678afecb367f032d93f642f64180aa3",
                    "contractName": "MyContract",
                    "hash": "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"
                }
            ]
        });
        fs::write(chain_dir.join("run-latest.json"), run_latest.to_string()).unwrap();

        let result = parse_broadcast_dir(&broadcast_dir, None);
        assert!(result.is_ok());
        let contracts = result.unwrap();
        assert_eq!(contracts.len(), 1);
        assert_eq!(contracts[0].name, "MyContract");
        assert_eq!(
            contracts[0].address.to_string(),
            "0x5FbDB2315678afecb367f032d93F642f64180aa3"
        );
    }

    #[test]
    fn test_parse_broadcast_dir_with_filter() {
        let tmp = TempDir::new().unwrap();
        let broadcast_dir = tmp.path().join("broadcast");
        let script_dir = broadcast_dir.join("Deploy.s.sol");
        let chain_dir = script_dir.join("31337");
        fs::create_dir_all(&chain_dir).unwrap();

        let run_latest = serde_json::json!({
            "transactions": [
                {
                    "transactionType": "CREATE",
                    "contractAddress": "0x5fbdb2315678afecb367f032d93f642f64180aa3",
                    "contractName": "ContractA",
                    "hash": "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"
                },
                {
                    "transactionType": "CREATE",
                    "contractAddress": "0xe7f1725e7734ce288f8367e1bb143e90bb3f0512",
                    "contractName": "ContractB",
                    "hash": "0xabcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890"
                }
            ]
        });
        fs::write(chain_dir.join("run-latest.json"), run_latest.to_string()).unwrap();

        let result = parse_broadcast_dir(&broadcast_dir, Some("ContractB"));
        assert!(result.is_ok());
        let contracts = result.unwrap();
        assert_eq!(contracts.len(), 1);
        assert_eq!(contracts[0].name, "ContractB");
    }

    #[test]
    fn test_parse_broadcast_dir_ignores_calls() {
        let tmp = TempDir::new().unwrap();
        let broadcast_dir = tmp.path().join("broadcast");
        let script_dir = broadcast_dir.join("Deploy.s.sol");
        let chain_dir = script_dir.join("31337");
        fs::create_dir_all(&chain_dir).unwrap();

        let run_latest = serde_json::json!({
            "transactions": [
                {
                    "transactionType": "CREATE",
                    "contractAddress": "0x5fbdb2315678afecb367f032d93f642f64180aa3",
                    "contractName": "MyContract",
                    "hash": "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"
                },
                {
                    "transactionType": "CALL",
                    "contractAddress": "0xe7f1725e7734ce288f8367e1bb143e90bb3f0512",
                    "contractName": "OtherContract",
                    "hash": "0xabcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890"
                }
            ]
        });
        fs::write(chain_dir.join("run-latest.json"), run_latest.to_string()).unwrap();

        let result = parse_broadcast_dir(&broadcast_dir, None);
        assert!(result.is_ok());
        let contracts = result.unwrap();
        assert_eq!(contracts.len(), 1);
        assert_eq!(contracts[0].name, "MyContract");
    }

    #[test]
    fn test_load_existing_config_missing() {
        let tmp = TempDir::new().unwrap();
        let result = load_existing_config(tmp.path());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No edb.local.json"));
    }

    #[test]
    fn test_load_existing_config_valid() {
        let tmp = TempDir::new().unwrap();
        let config = serde_json::json!({
            "project_root": tmp.path().to_str().unwrap(),
            "solc_version": "0.8.26",
            "contracts": [
                {
                    "address": "0x5fbdb2315678afecb367f032d93f642f64180aa3",
                    "name": "MyContract",
                    "source": "src/MyContract.sol",
                    "creation_tx": "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef",
                    "constructor_args": "0x"
                }
            ]
        });
        fs::write(tmp.path().join("edb.local.json"), config.to_string()).unwrap();

        let result = load_existing_config(tmp.path());
        assert!(result.is_ok());
        let loaded = result.unwrap();
        assert_eq!(loaded.contracts.len(), 1);
        assert_eq!(loaded.contracts[0].name, "MyContract");
    }

    #[test]
    fn test_init_local_project_creates_files() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("foundry.toml"), "[profile.default]\n").unwrap();

        let result = init_local_project(tmp.path().to_path_buf());
        assert!(result.is_ok());

        assert!(tmp.path().join("edb.local.json").exists());
        assert!(tmp.path().join("edb-setup.sh").exists());

        let config_content = fs::read_to_string(tmp.path().join("edb.local.json")).unwrap();
        assert!(config_content.contains("YourContract"));

        let setup_content = fs::read_to_string(tmp.path().join("edb-setup.sh")).unwrap();
        assert!(setup_content.contains("#!/usr/bin/env bash"));
        assert!(setup_content.contains("anvil"));
    }

    #[test]
    fn test_init_local_project_skips_existing_files() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("foundry.toml"), "[profile.default]\n").unwrap();
        fs::write(tmp.path().join("edb.local.json"), "{}").unwrap();
        fs::write(tmp.path().join("edb-setup.sh"), "#!/bin/bash\n").unwrap();

        let result = init_local_project(tmp.path().to_path_buf());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already exist"));
    }

    #[test]
    fn test_init_local_project_partial_existing() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("foundry.toml"), "[profile.default]\n").unwrap();
        fs::write(tmp.path().join("edb.local.json"), "{}").unwrap();

        let result = init_local_project(tmp.path().to_path_buf());
        assert!(result.is_ok());
        assert!(tmp.path().join("edb-setup.sh").exists());
    }
}
