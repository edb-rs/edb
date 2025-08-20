//! Onchain compiler utilities.

use std::path::PathBuf;

use alloy_primitives::{Address, Bytes};
use edb_common::{Cache, EdbCache};
use eyre::Result;
use foundry_block_explorers::{contract::Metadata, errors::EtherscanError, Client};
use foundry_compilers::{
    artifacts::{output_selection::OutputSelection, CompilerOutput, Contract, SolcInput, Source},
    solc::{Solc, SolcLanguage},
};
use serde::{Deserialize, Serialize};
use tracing::trace;

use crate::{etherscan_rate_limit_guard, Artifact};

/// Onchain compiler.
#[derive(Debug, Clone)]
pub struct OnchainCompiler {
    /// Cache for the compiled contracts.
    pub cache: Option<EdbCache<Option<Artifact>>>,
}

impl OnchainCompiler {
    /// New onchain compiler.
    pub fn new(cache_root: Option<PathBuf>) -> Result<Self> {
        Ok(Self {
            // None for no expiry
            cache: EdbCache::new(cache_root, None)?,
        })
    }

    /// Compile the contract at the given address.
    /// Returns `Some`` if the contract is successfully compiled.
    /// Returns `None` if the contract is not verified, is a Vyper contract, or it is a Solidity
    /// 0.4.x contract which does not support --stand-json option.
    pub async fn compile(&self, etherscan: &Client, addr: Address) -> Result<Option<Artifact>> {
        // Get the cache_root. If not provided, use the default cache directory.
        if let Some(output) = self.cache.load_cache(addr.to_string()) {
            Ok(output)
        } else {
            let mut meta =
                match etherscan_rate_limit_guard!(etherscan.contract_source_code(addr).await) {
                    Ok(meta) => meta,
                    Err(EtherscanError::ContractCodeNotVerified(_)) => {
                        // We also cache the fact that the contract is not verified.
                        let artifact = None;
                        self.cache.save_cache(addr.to_string(), &artifact)?;
                        return Ok(artifact);
                    }
                    Err(e) => return Err(e.into()),
                };
            eyre::ensure!(meta.items.len() == 1, "contract not found or ill-formed");
            let meta = meta.items.remove(0);

            if meta.is_vyper() {
                // We do not cache if the contract is a Vyper contract.
                return Ok(None);
            }

            // prepare the input for solc
            let mut settings = meta.settings()?;
            // enforce compiler output all possible outputs
            settings.output_selection = OutputSelection::complete_output_selection();
            trace!(addr=?addr, settings=?settings, "using settings");

            // prepare the sources
            let sources = meta
                .sources()
                .into_iter()
                .map(|(k, v)| (k.into(), Source::new(v.content)))
                .collect();
            let input = SolcInput::new(SolcLanguage::Solidity, sources, settings);

            // prepare the compiler
            let version = meta.compiler_version()?;
            let compiler = Solc::find_or_install(&version)?;
            trace!(addr=?addr, compiler=?compiler, "using compiler");

            // compile the source code
            let output = match compiler.compile_exact(&input) {
                Ok(output) => Some(Artifact { meta, input, output }),
                Err(_) if version.major == 0 && version.minor == 4 => None,
                Err(e) => {
                    return Err(eyre::eyre!("failed to compile contract: {}", e));
                }
            };

            self.cache.save_cache(addr.to_string(), &output)?;
            Ok(output)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{str::FromStr, time::Duration};

    use alloy_chains::Chain;
    use serial_test::serial;

    use crate::utils::next_etherscan_api_key;

    use super::*;

    async fn run_compile(chain_id: Chain, addr: &str) -> eyre::Result<Option<Artifact>> {
        let etherscan_cache_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../testdata/cache/etherscan")
            .join(chain_id.to_string());
        let etherscan = Client::builder()
            .with_api_key(next_etherscan_api_key())
            .with_cache(Some(etherscan_cache_root), Duration::from_secs(24 * 60 * 60)) // 24 hours
            .chain(chain_id)?
            .build()?;

        // We disable the cache for testing.
        let compiler = OnchainCompiler::new(None)?;
        compiler.compile(&etherscan, Address::from_str(addr)?).await
    }

    #[tokio::test(flavor = "multi_thread")]
    #[serial]
    async fn test_tailing_slash() {
        run_compile(Chain::mainnet(), "0x22F9dCF4647084d6C31b2765F6910cd85C178C18").await.unwrap();
    }
}
