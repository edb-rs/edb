//! Context-related types and traits
//! This module provides types and traits for working with the EVM context.

use revm::{
    context::{BlockEnv, CfgEnv, TxEnv},
    database_interface::DBErrorMarker,
    primitives::{Address, HashMap, B256, U256},
    state::{Account, AccountInfo, Bytecode},
    Context, Database, DatabaseCommit,
};
use std::fmt;

/// Type alias for the EDB context
pub type EDBContext<DB> = Context<BlockEnv, TxEnv, CfgEnv, DB>;

/// A cloneable error type for DebugDB
#[derive(Clone, Debug)]
pub struct DebugDBError {
    message: String,
}

impl DebugDBError {
    /// Create a new error with a message
    pub fn new(message: impl Into<String>) -> Self {
        Self { message: message.into() }
    }

    /// Create from any error type
    pub fn from_error<E: std::error::Error>(err: E) -> Self {
        Self::new(err.to_string())
    }
}

impl fmt::Display for DebugDBError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DebugDB Error: {}", self.message)
    }
}

impl std::error::Error for DebugDBError {}

impl DBErrorMarker for DebugDBError {}

/// A wrapper database that provides a cloneable error type
/// This allows the database to be used in contexts requiring Clone
#[derive(Clone)]
pub struct DebugDB<DB> {
    inner: DB,
}

impl<DB> DebugDB<DB> {
    /// Create a new DebugDB wrapping an inner database
    pub fn new(inner: DB) -> Self {
        Self { inner }
    }

    /// Get a reference to the inner database
    pub fn inner(&self) -> &DB {
        &self.inner
    }

    /// Get a mutable reference to the inner database
    pub fn inner_mut(&mut self) -> &mut DB {
        &mut self.inner
    }

    /// Consume self and return the inner database
    pub fn into_inner(self) -> DB {
        self.inner
    }
}

impl<DB> Database for DebugDB<DB>
where
    DB: Database,
    <DB as Database>::Error: std::error::Error,
{
    type Error = DebugDBError;

    fn basic(&mut self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        self.inner.basic(address).map_err(DebugDBError::from_error)
    }

    fn code_by_hash(&mut self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        self.inner.code_by_hash(code_hash).map_err(DebugDBError::from_error)
    }

    fn storage(&mut self, address: Address, index: U256) -> Result<U256, Self::Error> {
        self.inner.storage(address, index).map_err(DebugDBError::from_error)
    }

    fn block_hash(&mut self, block: u64) -> Result<B256, Self::Error> {
        self.inner.block_hash(block).map_err(DebugDBError::from_error)
    }
}

impl<DB> DatabaseCommit for DebugDB<DB>
where
    DB: DatabaseCommit + Database,
    <DB as Database>::Error: std::error::Error,
{
    fn commit(&mut self, changes: HashMap<Address, Account>) {
        self.inner.commit(changes)
    }
}
