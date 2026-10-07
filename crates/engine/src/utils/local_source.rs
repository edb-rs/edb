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

//! Local source provider for contracts that are not verified on any explorer.
//!
//! Instead of downloading verified source code from Etherscan, this module builds the
//! same [`Artifact`] values from a local Foundry-style project: it resolves the import
//! graph through `foundry_compilers`, compiles the entry file's source closure with the
//! configured `solc`, and fabricates the explorer [`Metadata`] (name, compiler version,
//! constructor arguments, ABI) that the rest of the pipeline consumes.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    str::FromStr,
};

use alloy_primitives::{Address, Bytes, TxHash};
use eyre::{Context as _, Result, bail};
use foundry_block_explorers::contract::{Metadata, SourceCodeMetadata};
use foundry_compilers::{
    ConfigurableArtifacts, ProjectBuilder, ProjectPathsConfig,
    artifacts::{
        EvmVersion, Optimizer, SolcInput, SolcLanguage, Sources,
        output_selection::OutputSelection, remappings::Remapping,
    },
    solc::{SolcCompiler, SolcSettings},
};
use serde::Deserialize;
use tracing::{debug, info, warn};

use crate::{Artifact, utils::find_or_install_solc};

/// A contract discovered from broadcast artifacts or forge script output.
#[derive(Debug, Clone)]
pub struct DiscoveredContract {
    pub address: Address,
    pub name: String,
    pub source: PathBuf,
    pub creation_tx: TxHash,
    pub constructor_args: Bytes,
}

/// One contract to debug, described in the local config file.
#[derive(Debug, Clone, Deserialize)]
pub struct LocalContractConfig {
    /// Deployed address on the target (local) chain.
    pub address: Address,
    /// Contract name, as it appears in the compiler output (e.g. `"MyContract"`).
    pub name: String,
    /// Path to the entry source file, relative to `project_root`
    /// (e.g. `"src/MyContract.sol"`).
    pub source: PathBuf,
    /// Hash of the transaction that created this contract. For CREATE2 deployments,
    /// this is the transaction that invoked the factory/deployer contract.
    pub creation_tx: TxHash,
    /// ABI-encoded constructor arguments appended to the init code (may be empty).
    #[serde(default)]
    pub constructor_args: Bytes,
}

/// Configuration for local (explorer-free) contract source resolution.
///
/// Loaded from a JSON file passed via `--local`:
/// ```json
/// {
///   "project_root": "/path/to/foundry/project",
///   "solc_version": "0.8.26",
///   "contracts": [
///     {
///       "address": "0x…",
///       "name": "MyContract",
///       "source": "src/MyContract.sol",
///       "creation_tx": "0x…",
///       "constructor_args": "0x…"
///     }
///   ]
/// }
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct LocalSourceConfig {
    /// Root of the Foundry project (must contain `foundry.toml` and `src/`).
    pub project_root: PathBuf,
    /// Override the solc version used to compile. Defaults to the `solc` value in
    /// `foundry.toml`, or `0.8.26`.
    #[serde(default)]
    pub solc_version: Option<String>,
    /// Contracts to instrument, keyed by address.
    pub contracts: Vec<LocalContractConfig>,
}

impl LocalSourceConfig {
    /// Load a local source configuration from a JSON file.
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .wrap_err_with(|| format!("failed to read local source config {path:?}"))?;
        serde_json::from_str(&content)
            .wrap_err_with(|| format!("failed to parse local source config {path:?}"))
    }

    /// Map from contract address to the transaction hash that created it.
    pub fn creation_txs(&self) -> HashMap<Address, TxHash> {
        self.contracts.iter().map(|c| (c.address, c.creation_tx)).collect()
    }

    /// Build a config from discovered contracts.
    pub fn from_discovered(
        project_root: PathBuf,
        solc_version: Option<String>,
        contracts: Vec<DiscoveredContract>,
    ) -> Self {
        let contracts = contracts
            .into_iter()
            .map(|d| LocalContractConfig {
                address: d.address,
                name: d.name,
                source: d.source,
                creation_tx: d.creation_tx,
                constructor_args: d.constructor_args,
            })
            .collect();

        Self { project_root, solc_version, contracts }
    }
}

/// Minimal subset of `foundry.toml` `[profile.default]` that we need.
#[derive(Debug, Default, Clone, Deserialize)]
struct FoundryProfile {
    #[serde(default)]
    remappings: Vec<String>,
    #[serde(default)]
    solc: Option<String>,
    #[serde(default)]
    solc_version: Option<String>,
    #[serde(default)]
    optimizer: Option<bool>,
    #[serde(default)]
    optimizer_runs: Option<usize>,
    #[serde(default)]
    via_ir: Option<bool>,
    #[serde(default)]
    evm_version: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct FoundryToml {
    #[serde(default)]
    profile: HashMap<String, FoundryProfile>,
}

/// Read compiler-relevant settings from `<root>/foundry.toml` (`[profile.default]`).
fn read_foundry_profile(root: &Path) -> Result<FoundryProfile> {
    let path = root.join("foundry.toml");
    if !path.exists() {
        warn!("foundry.toml not found at {path:?}; using default compiler settings");
        return Ok(FoundryProfile::default());
    }
    let content = std::fs::read_to_string(&path)?;
    let parsed: FoundryToml = toml::from_str(&content)
        .wrap_err_with(|| format!("failed to parse {path:?}"))?;
    Ok(parsed.profile.get("default").cloned().unwrap_or_default())
}

/// Build `Artifact`s for locally compiled contracts, replacing the Etherscan download
/// step of the debugging pipeline.
///
/// Each configured contract gets an [`Artifact`] whose `input` is the solc standard-JSON
/// covering the import closure of its `source` file, and whose `output` is the
/// compilation of that input. Contracts that share an entry file share one compilation.
pub fn load_local_source_code(
    config: &LocalSourceConfig,
) -> Result<HashMap<Address, Artifact>> {
    let root = config.project_root.canonicalize().wrap_err_with(|| {
        format!("project_root {:?} does not exist", config.project_root)
    })?;

    let profile = read_foundry_profile(&root)?;

    let solc_version = config
        .solc_version
        .clone()
        .or(profile.solc_version.clone())
        .or(profile.solc.clone())
        .unwrap_or_else(|| "0.8.26".to_string());
    let solc_version = solc_version.trim_start_matches('v').to_string();

    // Parse remappings from foundry.toml and resolve relative targets against the
    // project root. Remapping entries may carry an optional `context:` prefix.
    let remappings: Vec<Remapping> = profile
        .remappings
        .iter()
        .map(|r| {
            let r = r.rsplit(':').next().unwrap_or(r);
            let mut parsed = Remapping::from_str(r)
                .wrap_err_with(|| format!("invalid remapping {r:?}"))?;
            if Path::new(&parsed.path).is_relative() {
                parsed.path = root.join(&parsed.path).to_string_lossy().to_string();
            }
            Ok(parsed)
        })
        .collect::<Result<_>>()?;

    let paths = ProjectPathsConfig::builder()
        .root(root.clone())
        .sources(root.join("src"))
        .tests(root.join("test"))
        .scripts(root.join("script"))
        .lib(root.join("lib"))
        .remappings(remappings)
        .build()
        .map_err(|e| eyre::eyre!("failed to build project paths: {e}"))?;

    let mut settings = foundry_compilers::artifacts::Settings::default();
    settings.optimizer = Optimizer {
        enabled: Some(profile.optimizer.unwrap_or(true)),
        runs: Some(profile.optimizer_runs.unwrap_or(200)),
        ..Default::default()
    };
    settings.via_ir = profile.via_ir;
    if let Some(evm) = &profile.evm_version {
        settings.evm_version =
            EvmVersion::from_str(&evm.to_lowercase()).ok().or(settings.evm_version);
    }
    settings.output_selection = OutputSelection::complete_output_selection();

    let project = ProjectBuilder::<SolcCompiler, ConfigurableArtifacts>::new(
        ConfigurableArtifacts::default(),
    )
        .paths(paths)
        .settings(SolcSettings { settings, ..Default::default() })
        .no_artifacts()
        .build(SolcCompiler::default())
        .map_err(|e| eyre::eyre!("failed to build foundry project: {e}"))?;

    let version = semver::Version::parse(&solc_version)
        .wrap_err_with(|| format!("invalid solc version {solc_version:?}"))?;
    let compiler = find_or_install_solc(&version)?;

    // Compile once per unique entry source file; contracts sharing an entry share the
    // compiled input/output (which is what Etherscan looks like anyway: one input per
    // verified contract).
    let mut compiled: HashMap<PathBuf, (SolcInput, foundry_compilers::artifacts::CompilerOutput)> =
        HashMap::new();

    let mut artifacts = HashMap::new();
    for contract in &config.contracts {
        let entry = root.join(&contract.source);
        if !compiled.contains_key(&contract.source) {
            let std_input = project
                .standard_json_input(&entry)
                .map_err(|e| eyre::eyre!("failed to resolve imports of {entry:?}: {e}"))?;

            let sources: Sources = std_input.sources.into_iter().collect();
            let mut settings = std_input.settings;
            settings.output_selection = OutputSelection::complete_output_selection();
            let input = SolcInput::new(SolcLanguage::Solidity, sources, settings);

            info!(
                name = %contract.name,
                source = %contract.source.display(),
                "compiling {} source units with solc {solc_version}",
                input.sources.len()
            );
            let output = compiler
                .compile_exact(&input)
                .map_err(|e| eyre::eyre!("solc failed for {entry:?}: {e}"))?;
            if output.errors.iter().any(|e| e.is_error()) {
                for e in output.errors.iter().filter(|e| e.is_error()) {
                    warn!("{}", e.message);
                }
                bail!("compilation of {} produced errors", contract.source.display());
            }
            compiled.insert(contract.source.clone(), (input, output));
        }
        let (input, output) = compiled.get(&contract.source).unwrap();

        // Locate the compiled contract to pull its ABI for the fabricated metadata.
        let compiled_contract = output
            .contracts
            .values()
            .find_map(|contracts| contracts.get(&contract.name))
            .ok_or_else(|| {
                eyre::eyre!(
                    "contract {} not found in compilation output of {}",
                    contract.name,
                    contract.source.display()
                )
            })?;
        let abi = compiled_contract
            .abi
            .as_ref()
            .map(|abi| serde_json::to_string(abi).unwrap_or_else(|_| "[]".to_string()))
            .unwrap_or_else(|| "[]".to_string());

        let meta = fabricate_metadata(contract, &solc_version, &profile, abi, input)?;

        debug!(
            addr = %contract.address,
            name = %contract.name,
            "built local artifact"
        );
        artifacts.insert(
            contract.address,
            Artifact { meta, input: input.clone(), output: output.clone() },
        );
    }

    Ok(artifacts)
}

/// Fabricate an Etherscan-style [`Metadata`] for a locally compiled contract. Only the
/// fields consumed by the engine (name, compiler version, constructor arguments, ABI,
/// optimizer flags) are meaningful.
fn fabricate_metadata(
    contract: &LocalContractConfig,
    solc_version: &str,
    profile: &FoundryProfile,
    abi: String,
    input: &SolcInput,
) -> Result<Metadata> {
    let optimizer_used = profile.optimizer.unwrap_or(true);
    let value = serde_json::json!({
        "SourceCode": {},
        "ABI": abi,
        "ContractName": contract.name,
        "CompilerVersion": format!("v{solc_version}"),
        "OptimizationUsed": if optimizer_used { "1" } else { "0" },
        "Runs": profile.optimizer_runs.unwrap_or(200).to_string(),
        "ConstructorArguments": contract.constructor_args.to_string().trim_start_matches("0x"),
        "EVMVersion": profile.evm_version.clone().unwrap_or_else(|| "Default".to_string()),
        "Library": "",
        "LicenseType": "",
        "Proxy": "0",
        "SwarmSource": "",
    });
    let mut meta: Metadata = serde_json::from_value(value)
        .wrap_err("failed to fabricate explorer metadata for local contract")?;
    // Provide the real sources so consumers of `meta.sources()`/`source_tree()` (e.g.
    // UIs rendering source text) see the actual files, keyed exactly like the compiler
    // input.
    meta.source_code = SourceCodeMetadata::Sources(
        input
            .sources
            .iter()
            .map(|(path, source)| {
                (path.to_string_lossy().to_string(), source.content.as_ref().clone().into())
            })
            .collect(),
    );
    Ok(meta)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::{address, bytes};

    fn make_discovered(suffix: u8) -> DiscoveredContract {
        let mut addr_bytes = [0u8; 20];
        addr_bytes[19] = suffix;
        DiscoveredContract {
            address: Address::from(addr_bytes),
            name: format!("Contract{suffix}"),
            source: PathBuf::from(format!("src/Contract{suffix}.sol")),
            creation_tx: TxHash::from([suffix; 32]),
            constructor_args: Bytes::from(vec![suffix; 4]),
        }
    }

    #[test]
    fn test_from_discovered_empty() {
        let config = LocalSourceConfig::from_discovered(
            PathBuf::from("/tmp/project"),
            Some("0.8.26".to_string()),
            vec![],
        );
        assert!(config.contracts.is_empty());
        assert_eq!(config.project_root, PathBuf::from("/tmp/project"));
        assert_eq!(config.solc_version, Some("0.8.26".to_string()));
    }

    #[test]
    fn test_from_discovered_single() {
        let d = make_discovered(1);
        let config = LocalSourceConfig::from_discovered(
            PathBuf::from("/tmp/project"),
            None,
            vec![d.clone()],
        );
        assert_eq!(config.contracts.len(), 1);
        assert_eq!(config.contracts[0].address, d.address);
        assert_eq!(config.contracts[0].name, "Contract1");
        assert_eq!(config.contracts[0].source, PathBuf::from("src/Contract1.sol"));
        assert_eq!(config.contracts[0].creation_tx, d.creation_tx);
        assert_eq!(config.contracts[0].constructor_args, d.constructor_args);
    }

    #[test]
    fn test_from_discovered_multiple() {
        let contracts = vec![make_discovered(1), make_discovered(2), make_discovered(3)];
        let config = LocalSourceConfig::from_discovered(
            PathBuf::from("/tmp/project"),
            Some("0.8.28".to_string()),
            contracts,
        );
        assert_eq!(config.contracts.len(), 3);
        assert_eq!(config.solc_version, Some("0.8.28".to_string()));
    }

    #[test]
    fn test_creation_txs() {
        let d1 = make_discovered(1);
        let d2 = make_discovered(2);
        let config = LocalSourceConfig::from_discovered(
            PathBuf::from("/tmp/project"),
            None,
            vec![d1.clone(), d2.clone()],
        );
        let txs = config.creation_txs();
        assert_eq!(txs.len(), 2);
        assert_eq!(txs[&d1.address], d1.creation_tx);
        assert_eq!(txs[&d2.address], d2.creation_tx);
    }

    #[test]
    fn test_from_discovered_preserves_constructor_args() {
        let d = DiscoveredContract {
            address: address!("0x5fbdb2315678afecb367f032d93f642f64180aa3"),
            name: "MyContract".to_string(),
            source: PathBuf::from("src/MyContract.sol"),
            creation_tx: TxHash::from([0xAB; 32]),
            constructor_args: bytes!("0000000000000000000000005fbdb2315678afecb367f032d93f642f64180aa3"),
        };
        let config = LocalSourceConfig::from_discovered(
            PathBuf::from("/tmp/project"),
            None,
            vec![d],
        );
        assert_eq!(
            config.contracts[0].constructor_args,
            bytes!("0000000000000000000000005fbdb2315678afecb367f032d93f642f64180aa3")
        );
    }
}
