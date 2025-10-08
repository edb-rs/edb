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

//! Snapshotting module that captures the EVM state at various points during
//! transaction execution to enable time travel debugging.
use std::collections::{HashMap, HashSet};

use alloy_primitives::{Address, Bytes};
use edb_common::{relax_evm_constraints, types::Trace, EdbContext};
use eyre::Result;
use foundry_compilers::artifacts::Contract;
use revm::{
    context::TxEnv, database::CacheDB, Database, DatabaseCommit, DatabaseRef, InspectEvm,
    MainBuilder,
};
use tracing::info;

use crate::{
    analysis::AnalysisResult, Artifact, HookSnapshotInspector, HookSnapshots,
    OpcodeSnapshotInspector, OpcodeSnapshots, Snapshots,
};

/// Time travel (i.e., snapshotting) at the opcode level for contracts we do not
/// have source code.
pub fn capture_opcode_level_snapshots<DB>(
    ctx: EdbContext<DB>,
    tx: TxEnv,
    excluded_addresses: HashSet<Address>,
    trace: &Trace,
) -> Result<OpcodeSnapshots<DB>>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone,
    <CacheDB<DB> as Database>::Error: Clone,
    <DB as Database>::Error: Clone,
{
    info!("Collecting opcode-level step execution results");

    let mut inspector = OpcodeSnapshotInspector::new(&ctx, trace);
    inspector.with_excluded_addresses(excluded_addresses);
    let mut evm = ctx.build_mainnet_with_inspector(&mut inspector);

    evm.inspect_one_tx(tx)
        .map_err(|e| eyre::eyre!("Failed to inspect the target transaction: {:?}", e))?;

    let snapshots = inspector.into_snapshots();

    snapshots.print_summary();

    Ok(snapshots)
}

/// Collect creation hooks for contracts we have source code.
pub fn collect_creation_hooks<'a>(
    artifacts: &'a HashMap<Address, Artifact>,
    recompiled_artifacts: &'a HashMap<Address, Artifact>,
    contracts_in_tx: Vec<Address>,
) -> Result<Vec<(&'a Contract, &'a Contract, &'a Bytes)>> {
    info!("Collecting creation hooks for contracts in transaction");

    let mut hook_creation = Vec::new();
    for address in contracts_in_tx {
        let Some(artifact) = artifacts.get(&address) else {
            eyre::bail!("No original artifact found for address {}", address);
        };

        let Some(recompiled_artifact) = recompiled_artifacts.get(&address) else {
            eyre::bail!("No recompiled artifact found for address {}", address);
        };

        hook_creation.extend(artifact.find_creation_hooks(recompiled_artifact));
    }

    Ok(hook_creation)
}

/// Time travel (i.e., snapshotting) at hooks for contracts we have source code
pub fn capture_hook_snapshots<'a, DB>(
    mut ctx: EdbContext<DB>,
    mut tx: TxEnv,
    creation_hooks: Vec<(&'a Contract, &'a Contract, &'a Bytes)>,
    trace: &Trace,
    analysis_results: &HashMap<Address, AnalysisResult>,
) -> Result<HookSnapshots<DB>>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone,
    <CacheDB<DB> as Database>::Error: Clone,
    <DB as Database>::Error: Clone,
{
    info!("Re-executing transaction with snapshot collection");

    // We need to relax execution constraints for hook snapshots
    relax_evm_constraints(&mut ctx, &mut tx);

    info!("Collecting hook snapshots for source code contracts");

    let mut inspector = HookSnapshotInspector::new(&ctx, trace, analysis_results);
    inspector.with_creation_hooks(creation_hooks)?;
    let mut evm = ctx.build_mainnet_with_inspector(&mut inspector);

    evm.inspect_one_tx(tx)
        .map_err(|e| eyre::eyre!("Failed to inspect the target transaction: {:?}", e))?;

    let snapshots = inspector.into_snapshots();

    snapshots.print_summary();

    Ok(snapshots)
}

/// Merge opcode-level and hook-level snapshots into a unified snapshot structure
/// that supports time travel across the entire execution trace.
pub fn get_time_travel_snapshots<DB>(
    opcode_snapshots: OpcodeSnapshots<DB>,
    hook_snapshots: HookSnapshots<DB>,
) -> Result<Snapshots<DB>>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone,
    <CacheDB<DB> as Database>::Error: Clone,
    <DB as Database>::Error: Clone,
{
    info!("Merging opcode-level and hook-level snapshots");

    let snapshots = Snapshots::merge(opcode_snapshots, hook_snapshots);
    snapshots.print_summary();

    Ok(snapshots)
}
