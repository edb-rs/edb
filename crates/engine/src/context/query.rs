use std::collections::{HashMap, HashSet};

use alloy_primitives::Address;
use revm::{database::CacheDB, Database, DatabaseCommit, DatabaseRef};

use crate::EngineContext;

/// Trait providing query capabilities on the EngineContext.
/// This trait allows querying various aspects of the execution context,
/// such as retrieving addresses associated with snapshots and
/// determining parent-child relationships in the execution trace.  
pub trait ContextQueryTr {
    /// Get the bytecode address for a snapshot.
    ///
    /// Returns the address where the executing bytecode is stored, which may differ
    /// from the target address in cases of delegatecall or proxy contracts.
    fn get_bytecode_address(&self, snapshot_id: usize) -> Option<Address>;

    /// Get the target address for a snapshot.
    ///
    /// Returns the address that was the target of the call, which is the address
    /// receiving the call in the current execution frame.
    fn get_target_address(&self, snapshot_id: usize) -> Option<Address>;

    /// Check if one trace entry is the parent of another.
    ///
    /// This method determines the parent-child relationship between trace entries,
    /// useful for understanding call hierarchy during debugging.
    fn is_parent_trace(&self, parent_id: usize, child_id: usize) -> bool;

    /// Get the address to code address mapping.
    ///
    /// Returns a cached mapping from target addresses to all code addresses that
    /// have been executed for each target. This is useful for understanding
    /// proxy patterns and delegatecall relationships.
    fn address_code_address_map(&self) -> &HashMap<Address, HashSet<Address>>;
}

impl<DB> ContextQueryTr for EngineContext<DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone + Send + Sync + 'static,
    <CacheDB<DB> as Database>::Error: Clone + Send + Sync,
    <DB as Database>::Error: Clone + Send + Sync,
{
    fn get_bytecode_address(&self, snapshot_id: usize) -> Option<Address> {
        let (frame_id, _) = self.snapshots.get(snapshot_id)?;
        self.trace.get(frame_id.trace_entry_id()).map(|entry| entry.code_address)
    }

    fn get_target_address(&self, snapshot_id: usize) -> Option<Address> {
        let (frame_id, _) = self.snapshots.get(snapshot_id)?;
        self.trace.get(frame_id.trace_entry_id()).map(|entry| entry.target)
    }

    fn is_parent_trace(&self, parent_id: usize, child_id: usize) -> bool {
        match self.trace.get(child_id) {
            Some(child_entry) => child_entry.parent_id == Some(parent_id),
            None => false,
        }
    }

    fn address_code_address_map(&self) -> &HashMap<Address, HashSet<Address>> {
        self.address_code_address_map.get_or_init(|| {
            let mut map: HashMap<Address, HashSet<Address>> = HashMap::new();
            for entry in &self.trace {
                map.entry(entry.target).or_default().insert(entry.code_address);
            }
            map
        })
    }
}
