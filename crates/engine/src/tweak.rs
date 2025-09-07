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

use std::path::PathBuf;

use alloy_primitives::{Address, Bytes, TxHash};
use edb_common::{
    fork_and_prepare, relax_context_constraints, Cache, CachePath, EdbCache, EdbCachePath,
    EdbContext, ForkResult,
};
use eyre::Result;
use foundry_block_explorers::{contract::ContractCreationData, Client};
use foundry_compilers::Artifact as _;
use revm::{
    context::{Cfg, ContextTr},
    database::CacheDB,
    primitives::KECCAK_EMPTY,
    state::Bytecode,
    Database, DatabaseCommit, DatabaseRef, InspectEvm, MainBuilder,
};

use crate::{next_etherscan_api_key, Artifact, TweakInspector};

/// A utility for modifying deployed contract bytecode by replaying their creation transactions
/// with replacement bytecode from compiled artifacts.
///
/// This struct handles the process of finding contract creation transactions,
/// replaying them with modified init code, and updating the contract's runtime bytecode.
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
    /// Creates a new `CodeTweaker` instance.
    ///
    /// # Arguments
    ///
    /// * `ctx` - Mutable reference to the EDB context containing the database
    /// * `rpc_url` - RPC endpoint URL for fetching blockchain data
    /// * `etherscan_api_key` - Optional Etherscan API key for fetching contract creation data
    pub fn new(
        ctx: &'a mut EdbContext<DB>,
        rpc_url: String,
        etherscan_api_key: Option<String>,
    ) -> Self {
        Self { ctx, rpc_url, etherscan_api_key }
    }

    /// Replaces the bytecode of a deployed contract with bytecode from a compiled artifact.
    ///
    /// # Arguments
    ///
    /// * `addr` - Address of the deployed contract to tweak
    /// * `artifact` - Compiled artifact containing the replacement bytecode
    /// * `quick` - Whether to use quick mode for faster but less accurate replay
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the bytecode was successfully replaced, or an error if the operation failed.
    pub async fn tweak(
        &mut self,
        addr: &Address,
        artifact: &Artifact,
        recompiled_artifact: &Artifact,
        quick: bool,
    ) -> Result<()> {
        let tweaked_code =
            self.get_tweaked_code(addr, artifact, recompiled_artifact, quick).await?;

        let db = self.ctx.db_mut();

        let mut info = db
            .basic(*addr)
            .map_err(|e| eyre::eyre!("Failed to get account info for {}: {}", addr, e))?
            .unwrap_or_default();
        // Code hash will be update within `db.insert_account_info(&mut info);`
        info.code_hash = KECCAK_EMPTY;
        info.code = Some(Bytecode::new_raw(tweaked_code));
        db.insert_account_info(*addr, info);

        Ok(())
    }

    async fn get_tweaked_code(
        &self,
        addr: &Address,
        artifact: &Artifact,
        recompiled_artifact: &Artifact,
        quick: bool,
    ) -> Result<Bytes> {
        let creation_tx_hash = self.get_creation_tx(addr).await?;

        // Create replay environment
        let ForkResult { context: mut replay_ctx, target_tx_env: mut creation_tx_env, .. } =
            fork_and_prepare(&self.rpc_url, creation_tx_hash, quick).await?;
        relax_context_constraints(&mut replay_ctx, &mut creation_tx_env);

        // Get init code
        let contract = artifact.contract().ok_or(eyre::eyre!("Failed to get contract"))?;

        let recompiled_contract =
            recompiled_artifact.contract().ok_or(eyre::eyre!("Failed to get contract"))?;

        let constructor_args = recompiled_artifact.constructor_arguments();

        let mut inspector =
            TweakInspector::new(*addr, contract, recompiled_contract, constructor_args);

        let mut evm = replay_ctx.build_mainnet_with_inspector(&mut inspector);

        evm.inspect_one_tx(creation_tx_env)
            .map_err(|e| eyre::eyre!("Failed to inspect the target transaction: {:?}", e))?;

        inspector.into_deployed_code()
    }

    /// Retrieves the transaction hash that created a contract at the given address.
    ///
    /// This method first checks the local cache for the creation transaction data.
    /// If not cached, it queries Etherscan API and caches the result for future use.
    ///
    /// # Arguments
    ///
    /// * `addr` - Address of the deployed contract
    ///
    /// # Returns
    ///
    /// Returns the transaction hash that deployed the contract, or an error if not found.
    pub async fn get_creation_tx(&self, addr: &Address) -> Result<TxHash> {
        let chain_id = self.ctx.cfg().chain_id();

        // Cache directory
        let etherscan_cache_dir =
            EdbCachePath::new(None as Option<PathBuf>).etherscan_chain_cache_dir(chain_id);

        let cache = EdbCache::<ContractCreationData>::new(etherscan_cache_dir, None)?;
        let label = format!("contract_creation_{}", addr);

        if let Some(creation_data) = cache.load_cache(&label) {
            return Ok(creation_data.transaction_hash.into());
        } else {
            let etherscan_api_key =
                self.etherscan_api_key.clone().unwrap_or(next_etherscan_api_key());

            // Build client
            let etherscan = Client::builder()
                .with_api_key(etherscan_api_key)
                .chain(chain_id.into())?
                .build()?;

            // Get creation tx
            let creation_data = etherscan.contract_creation_data(*addr).await?;
            cache.save_cache(&label, &creation_data)?;
            Ok(creation_data.transaction_hash.into())
        }
    }
}
