//! Snapshot Inspector - Captures EVM state at every call/create boundary
//!
//! This inspector takes snapshots of the entire EVM context including journaled state
//! at each call/create/call_end/create_end event, allowing for "time travel"
//! debugging where you can inspect the state at any point during execution.
//!
//! # Key Features
//!
//! - **Journal-Aware State Snapshots**: Properly captures journaled changes, not just initial DB
//! - **Context Preservation**: Saves block/transaction environment for each snapshot
//! - **EVM Reconstruction**: Each snapshot can create a new EVM instance at that state
//! - **Committed State**: Actually commits journal changes to create true snapshots
//!
//! # Technical Implementation
//!
//! REVM uses a journal system where changes are accumulated during execution.
//! To capture true state snapshots, we need to:
//! 1. Clone the database
//! 2. Apply all journaled changes to the cloned database
//! 3. Store the committed state as our snapshot

use alloy_primitives::{Address, Bytes, U256};
use edb_utils::EDBContext;
use eyre::Result;
use revm::{
    context::{ContextTr, CreateScheme, TxEnv},
    database::{Database, DatabaseCommit},
    interpreter::{CallScheme, InstructionResult},
    Inspector,
};
use std::collections::VecDeque;
use tracing::{debug, info};

/// Type of EVM operation that triggered a snapshot
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnapshotTrigger {
    /// Call operation started
    CallStart { caller: Address, target: Address, scheme: CallScheme, input: Bytes, value: U256 },
    /// Call operation ended
    CallEnd {
        caller: Address,
        target: Address,
        result: InstructionResult,
        gas_used: u64,
        output: Vec<u8>,
    },
    /// Create operation started
    CreateStart { caller: Address, scheme: CreateScheme, value: U256, init_code: Bytes },
    /// Create operation ended
    CreateEnd {
        caller: Address,
        created: Option<Address>,
        result: InstructionResult,
        gas_used: u64,
    },
}

/// A complete snapshot of EVM state at a specific point in execution
#[derive(Debug, Clone)]
pub struct StateSnapshot<DB: Database + DatabaseCommit + Clone> {
    /// Unique identifier for this snapshot
    pub id: usize,
    /// What triggered this snapshot
    pub trigger: SnapshotTrigger,
    /// Current call depth when snapshot was taken
    pub call_depth: usize,
    /// Complete database state at this point
    pub database: DB,
    /// Block environment at this point
    pub block_env: revm::context::BlockEnv,
    /// Transaction environment at this point
    pub tx_env: TxEnv,
    /// Configuration environment at this point
    pub cfg_env: revm::context::CfgEnv,
    /// Gas used up to this point
    pub gas_used: u64,
    /// Program counter at snapshot (if in call)
    pub pc: Option<usize>,
    /// Current contract address (if in call)
    pub contract_address: Option<Address>,
}

impl<DB: Database + DatabaseCommit + Clone> StateSnapshot<DB> {
    /// Get a description of this snapshot
    pub fn description(&self) -> String {
        match &self.trigger {
            SnapshotTrigger::CallStart { caller, target, scheme, value, .. } => {
                format!("Call {} -> {} ({:?}) value: {}", caller, target, scheme, value)
            }
            SnapshotTrigger::CallEnd { caller, target, result, gas_used, .. } => {
                format!("Call {} -> {} ended ({:?}) gas: {}", caller, target, result, gas_used)
            }
            SnapshotTrigger::CreateStart { caller, value, .. } => {
                format!("Create from {} value: {}", caller, value)
            }
            SnapshotTrigger::CreateEnd { caller, created, result, gas_used, .. } => {
                format!("Create from {} -> {:?} ({:?}) gas: {}", caller, created, result, gas_used)
            }
        }
    }
}

/// Inspector that captures complete state snapshots at call/create boundaries
///
/// This inspector maintains a list of state snapshots that can be used to reconstruct
/// the EVM state at any point during transaction execution.
pub struct SnapshotInspector<DB: Database + DatabaseCommit + Clone> {
    /// List of captured snapshots in execution order
    snapshots: VecDeque<StateSnapshot<DB>>,
    /// Current snapshot ID counter
    snapshot_id: usize,
    /// Current call depth for tracking
    call_depth: usize,
    /// Maximum number of snapshots to keep (prevents memory exhaustion)
    max_snapshots: Option<usize>,
}

impl<DB: Database + DatabaseCommit + Clone> SnapshotInspector<DB> {
    /// Create a new snapshot inspector
    pub fn new() -> Self {
        Self {
            snapshots: VecDeque::new(),
            snapshot_id: 0,
            call_depth: 0,
            max_snapshots: Some(10000), // Reasonable default to prevent OOM
        }
    }

    /// Create a new snapshot inspector with unlimited snapshots
    ///
    /// ⚠️ Warning: This can lead to memory exhaustion on long-running transactions
    pub fn new_unlimited() -> Self {
        Self { snapshots: VecDeque::new(), snapshot_id: 0, call_depth: 0, max_snapshots: None }
    }

    /// Set maximum number of snapshots to keep
    pub fn with_max_snapshots(mut self, max: usize) -> Self {
        self.max_snapshots = Some(max);
        self
    }

    /// Take a snapshot of the current state INCLUDING journaled changes
    ///
    /// This is the critical method that properly captures the state with all
    /// journaled changes applied, not just the initial database state.
    fn take_snapshot(&mut self, context: &EDBContext<DB>, trigger: SnapshotTrigger) -> Result<()> {
        let mut inner = context.journal().to_inner();
        let changes = inner.finalize();
        let mut snap = context.db().clone();
        snap.commit(changes);

        let snapshot = StateSnapshot {
            id: self.snapshot_id,
            trigger,
            call_depth: self.call_depth,
            database: snap, // This now contains the committed state!
            block_env: context.block().clone(),
            tx_env: context.tx().clone(),
            cfg_env: context.cfg().clone(),
            gas_used: 0,            // TODO: Get actual gas used from context
            pc: None,               // TODO: Get PC if available
            contract_address: None, // TODO: Get current contract
        };

        debug!(
            "📸 Journal-aware snapshot {} taken at depth {}: {}",
            snapshot.id,
            snapshot.call_depth,
            snapshot.description()
        );

        self.snapshots.push_back(snapshot);
        self.snapshot_id += 1;

        // Enforce maximum snapshots limit
        if let Some(max) = self.max_snapshots {
            while self.snapshots.len() > max {
                let removed = self.snapshots.pop_front();
                if let Some(removed) = removed {
                    debug!("🗑️  Removed old snapshot {} to stay under limit", removed.id);
                }
            }
        }

        Ok(())
    }

    /// Get the number of snapshots captured
    pub fn snapshot_count(&self) -> usize {
        self.snapshots.len()
    }

    /// Get a reference to all snapshots
    pub fn snapshots(&self) -> &VecDeque<StateSnapshot<DB>> {
        &self.snapshots
    }

    /// Get the latest snapshot
    pub fn latest_snapshot(&self) -> Option<&StateSnapshot<DB>> {
        self.snapshots.back()
    }

    /// Find a snapshot by ID
    pub fn snapshot_by_id(&self, id: usize) -> Option<&StateSnapshot<DB>> {
        self.snapshots.iter().find(|s| s.id == id)
    }

    /// Extract all snapshots, consuming the inspector
    pub fn into_snapshots(self) -> VecDeque<StateSnapshot<DB>> {
        info!("🏁 Snapshot inspector collected {} snapshots", self.snapshots.len());
        self.snapshots
    }

    /// Clear all snapshots
    pub fn clear_snapshots(&mut self) {
        self.snapshots.clear();
        self.snapshot_id = 0;
        debug!("🧹 Cleared all snapshots");
    }
}

impl<DB: Database + DatabaseCommit + Clone> Default for SnapshotInspector<DB> {
    fn default() -> Self {
        Self::new()
    }
}

// Implementation of Inspector trait for SnapshotInspector
impl<DB> Inspector<EDBContext<DB>> for SnapshotInspector<DB>
where
    DB: Database + DatabaseCommit + Clone,
{
    /// Called when a call operation starts
    fn call(
        &mut self,
        context: &mut EDBContext<DB>,
        call: &mut revm::interpreter::CallInputs,
    ) -> Option<revm::interpreter::CallOutcome> {
        self.call_depth += 1;

        let trigger = SnapshotTrigger::CallStart {
            caller: call.caller,
            target: call.target_address,
            scheme: call.scheme,
            input: call.input.bytes(context).clone(),
            value: call.transfer_value().unwrap_or(U256::ZERO),
        };

        if let Err(e) = self.take_snapshot(context, trigger) {
            debug!("Failed to take snapshot on call start: {:?}", e);
        }

        None // Don't override call execution
    }

    /// Called when a call operation ends
    fn call_end(
        &mut self,
        context: &mut EDBContext<DB>,
        call: &revm::interpreter::CallInputs,
        outcome: &mut revm::interpreter::CallOutcome,
    ) {
        let trigger = SnapshotTrigger::CallEnd {
            caller: call.caller,
            target: call.target_address,
            result: outcome.result.result,
            gas_used: call.gas_limit - outcome.result.gas.remaining(),
            output: outcome.result.output.to_vec(),
        };

        if let Err(e) = self.take_snapshot(context, trigger) {
            debug!("Failed to take snapshot on call end: {:?}", e);
        }

        self.call_depth = self.call_depth.saturating_sub(1);
    }

    /// Called when a create operation starts
    fn create(
        &mut self,
        context: &mut EDBContext<DB>,
        create: &mut revm::interpreter::CreateInputs,
    ) -> Option<revm::interpreter::CreateOutcome> {
        self.call_depth += 1;

        let trigger = SnapshotTrigger::CreateStart {
            caller: create.caller,
            scheme: create.scheme,
            value: create.value,
            init_code: create.init_code.clone(),
        };

        if let Err(e) = self.take_snapshot(context, trigger) {
            debug!("Failed to take snapshot on create start: {:?}", e);
        }

        None // Don't override create execution
    }

    /// Called when a create operation ends
    fn create_end(
        &mut self,
        context: &mut EDBContext<DB>,
        create: &revm::interpreter::CreateInputs,
        outcome: &mut revm::interpreter::CreateOutcome,
    ) {
        let trigger = SnapshotTrigger::CreateEnd {
            caller: create.caller,
            created: outcome.address,
            result: outcome.result.result,
            gas_used: create.gas_limit - outcome.result.gas.remaining(),
        };

        if let Err(e) = self.take_snapshot(context, trigger) {
            debug!("Failed to take snapshot on create end: {:?}", e);
        }

        self.call_depth = self.call_depth.saturating_sub(1);
    }
}
