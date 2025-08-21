//! Chain forking and transaction replay utilities
//!
//! This module provides ACTUAL REVM TRANSACTION EXECUTION with transact_commit()

use crate::{get_blob_base_fee_update_fraction_by_spec_id, get_mainnet_spec_id, EdbContext, EdbDB};
use alloy_network::Network;
use alloy_primitives::{TxHash, TxKind, B256, U256};
use alloy_provider::{
    fillers::{FillProvider, TxFiller},
    layers::{CacheProvider, SharedCache},
    Provider, ProviderBuilder,
};
use alloy_rpc_types::{BlockNumberOrTag, Transaction, TransactionTrait};
use eyre::Result;
use indicatif::ProgressBar;
use revm::{
    context::{ContextTr, TxEnv},
    context_interface::block::BlobExcessGasAndPrice,
    database::{AlloyDB, CacheDB, StateBuilder},
    Context, Database, DatabaseCommit, DatabaseRef, ExecuteCommitEvm, MainBuilder, MainContext,
};
use std::sync::Arc;
use tracing::{debug, error, field::debug, info, warn};

use revm::{
    // Use re-exported primitives from revm
    context::result::ExecutionResult,
    database_interface::WrapDatabaseAsync,
    primitives::hardfork::SpecId,
};

/// Fork configuration details
#[derive(Debug, Clone)]
pub struct ForkInfo {
    /// Block number that was forked
    pub block_number: u64,
    /// Block hash
    pub block_hash: B256,
    /// Timestamp of the block
    pub timestamp: u64,
    /// Chain ID
    pub chain_id: u64,
    /// Spec ID for the hardfork
    pub spec_id: SpecId,
}

/// Result of forking operation containing comprehensive replay information
pub struct ForkResult<DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone + Send + Sync + 'static,
    <CacheDB<DB> as Database>::Error: Clone + Send + Sync,
    <DB as Database>::Error: Clone + Send + Sync,
{
    /// Fork information
    pub fork_info: ForkInfo,
    /// Revm context with executed state
    pub context: EdbContext<DB>,
    /// Transaction environment for the target transaction
    pub target_tx_env: TxEnv,
    /// Target transaction hash
    pub target_tx_hash: TxHash,
}

/// Get chain id by querying RPC
pub async fn get_chain_id(rpc_url: &str) -> Result<u64> {
    let provider = ProviderBuilder::new().connect(rpc_url).await?;
    let chain_id = provider.get_chain_id().await?;
    Ok(chain_id)
}

/// Fork the chain and ACTUALLY EXECUTE preceding transactions with revm.transact_commit()
///
/// This function:
/// 1. Creates revm database and environment
/// 2. Actually executes each preceding transaction with revm (unless quick mode is enabled)
/// 3. Commits each transaction
/// 4. Returns forked state ready for target transaction
pub async fn fork_and_prepare(
    rpc_url: &str,
    target_tx_hash: TxHash,
    quick: bool,
) -> Result<
    ForkResult<EdbDB<impl Clone + Database + DatabaseCommit + DatabaseRef + Send + Sync + 'static>>,
> {
    info!("forking chain and executing transactions with revm for {:?}", target_tx_hash);

    let provider = ProviderBuilder::new().connect(rpc_url).await?;

    let chain_id = provider
        .get_chain_id()
        .await
        .map_err(|e| eyre::eyre!("Failed to get chain ID: {:?}", e))?;
    if chain_id != 1 {
        warn!("We currently only support mainnet (chain ID 1), got {chain_id}. Use it at your own risk.");
    }

    // Get the target transaction to find which block it's in
    let target_tx = provider
        .get_transaction_by_hash(target_tx_hash)
        .await?
        .ok_or_else(|| eyre::eyre!("Target transaction not found: {:?}", target_tx_hash))?;

    let target_block_number = target_tx
        .block_number
        .ok_or_else(|| eyre::eyre!("Target transaction not mined: {:?}", target_tx_hash))?;

    info!("Target transaction is in block {}", target_block_number);

    // Get the full block with transactions
    let block = provider
        .get_block_by_number(BlockNumberOrTag::Number(target_block_number))
        .full()
        .await?
        .ok_or_else(|| eyre::eyre!("Block {} not found", target_block_number))?;

    // Get the transactions in the block
    let transactions = block.transactions.as_transactions().unwrap_or_default();

    // Find target transaction index
    let target_index = transactions
        .iter()
        .position(|tx| *tx.inner.hash() == target_tx_hash)
        .ok_or_else(|| eyre::eyre!("Target transaction not found in block"))?;

    // Get all transactions before the target
    let preceding_txs: Vec<&Transaction> = transactions.iter().take(target_index).collect();

    // Get the spec ID for the block using our mainnet mapping
    let spec_id = get_mainnet_spec_id(target_block_number);

    // Create fork info
    let fork_info = ForkInfo {
        block_number: target_block_number,
        block_hash: block.header.hash,
        timestamp: block.header.timestamp,
        chain_id,
        spec_id,
    };

    // Create revm database: we start with AlloyDB.
    let state_db = WrapDatabaseAsync::new(AlloyDB::new(provider, (target_block_number - 1).into()))
        .ok_or(eyre::eyre!("Failed to create AlloyDB"))?;
    let debug_db = EdbDB::new(CacheDB::new(Arc::new(state_db)));
    let cache_db: CacheDB<_> = CacheDB::new(debug_db);

    let ctx = Context::mainnet()
        .with_db(cache_db)
        .modify_block_chained(|b| {
            b.number = U256::from(target_block_number);
            b.timestamp = U256::from(block.header.timestamp);
            b.basefee = block.header.base_fee_per_gas.unwrap_or_default();
            b.difficulty = block.header.difficulty;
            b.gas_limit = block.header.gas_limit;
            b.prevrandao = Some(block.header.mix_hash);
            // Note: blob_excess_gas_and_price might not be available in older blocks
            b.blob_excess_gas_and_price = block.header.excess_blob_gas.map(|g| {
                BlobExcessGasAndPrice::new(g, get_blob_base_fee_update_fraction_by_spec_id(spec_id))
            })
        })
        .modify_cfg_chained(|c| {
            c.chain_id = chain_id;
            c.spec = spec_id;
        });

    let mut evm = ctx.build_mainnet();

    // Skip replaying preceding transactions if quick mode is enabled
    if quick {
        info!(
            "Quick mode enabled - skipping replay of {} preceding transactions",
            preceding_txs.len()
        );
    } else {
        debug!("Executing {} preceding transactions", preceding_txs.len());

        // Actually execute each transaction with revm
        let console_bar = Arc::new(ProgressBar::new(preceding_txs.len() as u64));
        let template = format!("{{spinner:.green}} 🔮 Replaying blockchain history for {} [{{bar:40.cyan/blue}}] {{pos:>3}}/{{len:3}} ⛽ {{msg}}", &target_tx_hash.to_string()[2..10]);
        console_bar.set_style(
            indicatif::ProgressStyle::with_template(&template)?
                .progress_chars("🟩🟦⬜")
                .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"),
        );

        for (i, tx) in preceding_txs.iter().enumerate() {
            let short_hash = &tx.inner.hash().to_string()[2..10]; // Skip 0x, take 8 chars
            console_bar.set_message(format!("tx {}: 0x{}...", i + 1, short_hash));

            debug!(
                "Executing transaction {}/{}: {:?}",
                i + 1,
                preceding_txs.len(),
                tx.inner.hash()
            );

            let tx = get_tx_env_from_tx(tx, chain_id)?;

            // Actually execute the transaction with commit
            match evm.transact_commit(tx) {
                Ok(result) => match result {
                    ExecutionResult::Success { gas_used, .. } => {
                        console_bar
                            .set_message(format!("✅ 0x{}... gas: {}", short_hash, gas_used));
                        debug!(
                            "Transaction {} executed and committed successfully, gas used: {}",
                            i + 1,
                            gas_used
                        );
                    }
                    ExecutionResult::Revert { gas_used, output } => {
                        console_bar.set_message(format!("⚠️  0x{}... reverted", short_hash));
                        debug!(
                            "Transaction {} reverted but committed, gas used: {}, output: {:?}",
                            i + 1,
                            gas_used,
                            output
                        );
                    }
                    ExecutionResult::Halt { reason, gas_used } => {
                        console_bar.set_message(format!("❌ 0x{}... halted", short_hash));
                        debug!(
                            "Transaction {} halted, gas used: {}, reason: {:?}",
                            i + 1,
                            gas_used,
                            reason
                        );
                    }
                },
                Err(e) => {
                    error!("Failed to execute transaction {}: {:?}", i + 1, e);
                    return Err(eyre::eyre!(
                        "Transaction execution failed at index {}: {:?}",
                        i,
                        e
                    ));
                }
            }

            console_bar.inc(1);
        }

        console_bar.finish_with_message(format!(
            "✨ Ready! Replayed {} transactions before {}",
            preceding_txs.len(),
            &target_tx_hash.to_string()[2..10]
        ));
    }

    // Get the target transaction environment
    let target_tx_env = get_tx_env_from_tx(&target_tx, chain_id)?;

    // Extract the context from the EVM
    let context = evm.ctx;

    Ok(ForkResult { fork_info, context, target_tx_env, target_tx_hash })
}

/// Get the transaction environment from the transaction.
pub fn get_tx_env_from_tx(tx: &Transaction, chain_id: u64) -> Result<TxEnv> {
    TxEnv::builder()
        .caller(tx.inner.signer())
        .gas_limit(tx.gas_limit())
        .gas_price(tx.gas_price().unwrap_or(tx.inner.max_fee_per_gas()))
        .value(tx.value())
        .data(tx.input().to_owned())
        .gas_priority_fee(tx.max_priority_fee_per_gas())
        .chain_id(Some(chain_id))
        .nonce(tx.nonce())
        .access_list(tx.access_list().cloned().unwrap_or_default())
        .kind(match tx.to() {
            Some(to) => TxKind::Call(to),
            None => TxKind::Create,
        })
        .build()
        .map_err(|e| eyre::eyre!("Failed to build transaction environment: {:?}", e))
}
