use std::{path::PathBuf, time::Duration};

use alloy_primitives::{keccak256, Address, Bytes, TxHash, B256};
use edb_common::{
    fork_and_prepare, relax_context_constraints, CachePath, EdbCachePath, EdbContext, ForkResult,
};
use eyre::Result;
use foundry_block_explorers::Client;
use foundry_compilers::Artifact as _;
use revm::{
    context::{Cfg, ContextTr},
    database::CacheDB,
    primitives::KECCAK_EMPTY,
    state::Bytecode,
    Database, DatabaseCommit, DatabaseRef, InspectEvm, MainBuilder,
};

use crate::{next_etherscan_api_key, tweak, Artifact, TweakInspectorBuilder};

pub struct CodeTweaker<'a, DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone,
    <CacheDB<DB> as Database>::Error: Clone,
    <DB as Database>::Error: Clone,
{
    ctx: &'a mut EdbContext<DB>,
    rpc_url: String,
    etherscan_api_key: Option<String>,
}

impl<'a, DB> CodeTweaker<'a, DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone,
    <CacheDB<DB> as Database>::Error: Clone,
    <DB as Database>::Error: Clone,
{
    pub fn new(
        ctx: &'a mut EdbContext<DB>,
        rpc_url: String,
        etherscan_api_key: Option<String>,
    ) -> Self {
        Self { ctx, rpc_url, etherscan_api_key }
    }

    pub async fn tweak(&mut self, addr: &Address, artifact: &Artifact, quick: bool) -> Result<()> {
        let tweaked_code = self.get_tweaked_code(addr, artifact, quick).await?;

        let db = self.ctx.db_mut();

        let mut info = db
            .basic(*addr)
            .map_err(|e| eyre::eyre!("Failed to get account info for {}: {}", addr, e))?
            .unwrap_or_default();
        let code_hash = if tweaked_code.as_ref().is_empty() {
            KECCAK_EMPTY
        } else {
            B256::from_slice(&keccak256(tweaked_code.as_ref())[..])
        };
        info.code_hash = code_hash;
        info.code = Some(Bytecode::new_raw(tweaked_code));
        db.insert_account_info(*addr, info);
        Ok(())
    }

    async fn get_tweaked_code(
        &self,
        addr: &Address,
        artifact: &Artifact,
        quick: bool,
    ) -> Result<Bytes> {
        let creation_tx_hash = self.get_creation_tx(addr).await?;

        // Create replay environment
        let ForkResult { context: mut replay_ctx, target_tx_env: mut creation_tx_env, .. } =
            fork_and_prepare(&self.rpc_url, creation_tx_hash, quick).await?;
        relax_context_constraints(&mut replay_ctx, &mut creation_tx_env);

        // Get init code
        let init_code = artifact
            .contract()
            .ok_or(eyre::eyre!("Failed to get contract"))?
            .get_bytecode_bytes()
            .ok_or(eyre::eyre!("Failed to get bytecode for contract {}", artifact.contract_name()))?
            .as_ref()
            .clone();

        let mut inspector = TweakInspectorBuilder::new()
            .target_address(*addr)
            .init_code(init_code)
            .constructor_args(artifact.constructor_arguments().clone())
            .build()
            .map_err(|e| {
                eyre::eyre!("Failed to build tweak inspector for address {}: {}", addr, e)
            })?;

        let mut evm = replay_ctx.build_mainnet_with_inspector(&mut inspector);

        evm.inspect_one_tx(creation_tx_env)
            .map_err(|e| eyre::eyre!("Failed to inspect the target transaction: {:?}", e))?;

        inspector.into_deployed_code()
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
