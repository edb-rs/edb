use std::{path::PathBuf, time::Duration};

use alloy_primitives::{Address, TxHash};
use edb_common::{fork_and_prepare, CachePath, EdbCachePath, EdbContext, ForkResult};
use eyre::Result;
use foundry_block_explorers::Client;
use foundry_compilers::Artifact as _;
use revm::{
    context::{Cfg, ContextTr},
    Database, DatabaseCommit,
};

use crate::{next_etherscan_api_key, Artifact, TweakInspectorBuilder};

pub struct CodeTweaker<'a, DB: Database + DatabaseCommit> {
    ctx: &'a mut EdbContext<DB>,
    rpc_url: String,
    etherscan_api_key: Option<String>,
}

impl<'a, DB: Database + DatabaseCommit> CodeTweaker<'a, DB> {
    pub fn new(
        ctx: &'a mut EdbContext<DB>,
        rpc_url: String,
        etherscan_api_key: Option<String>,
    ) -> Self {
        Self { ctx, rpc_url, etherscan_api_key }
    }

    pub async fn tweak(&mut self, addr: &Address, artifact: &Artifact, quick: bool) -> Result<()> {
        let creation_tx = self.get_creation_tx(addr).await?;

        // Create replay environment
        let ForkResult { context: replay_ctx, target_tx_env: creation_tx, .. } =
            fork_and_prepare(&self.rpc_url, creation_tx, quick, false).await?;

        // Get init code
        let init_code = artifact
            .contract()
            .ok_or(eyre::eyre!("Failed to get contract"))?
            .get_bytecode_bytes()
            .ok_or(eyre::eyre!("Failed to get bytecode for contract {}", artifact.contract_name()))?
            .as_ref()
            .clone();

        let inspector = TweakInspectorBuilder::new()
            .target_address(*addr)
            .init_code(init_code)
            .constructor_args(artifact.constructor_arguments().clone())
            .build()
            .map_err(|e| {
                eyre::eyre!("Failed to build tweak inspector for address {}: {}", addr, e)
            })?;

        Ok(())
    }

    async fn get_creation_tx(&self, addr: &Address) -> Result<TxHash> {
        let chain_id = self.ctx.cfg().chain_id();

        // Cache directory
        let etherscan_cache_dir =
            EdbCachePath::new(None as Option<PathBuf>).etherscan_chain_cache_dir(chain_id);

        let etherscan_api_key = self.etherscan_api_key.clone().unwrap_or(next_etherscan_api_key());

        // Build client
        let etherscan = Client::builder()
            .with_api_key(etherscan_api_key)
            .with_cache(
                etherscan_cache_dir,
                Duration::from_secs(u64::MAX), // No expiration for cache
            )
            .chain(chain_id.into())?
            .build()?;

        // Get creation tx
        let creation_data = etherscan.contract_creation_data(*addr).await?;
        Ok(creation_data.transaction_hash.into())
    }
}
