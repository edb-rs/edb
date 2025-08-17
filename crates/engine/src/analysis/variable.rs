//! Variable analysis and representation for Ethereum smart contract analysis.
//!
//! This module provides the core data structures and utilities for representing
//! and tracking variables during smart contract analysis. It includes:
//!
//! - **UVID (Universal Variable Identifier)**: A unique identifier system for
//!   tracking variables across different scopes and contexts
//! - **Variable**: The main data structure representing a smart contract variable
//! - **VariableType**: Enumeration of supported Solidity variable types
//! - **VariableScope**: Structure for managing variable scope information
//!
//! The module is designed to work with the broader analysis framework to provide
//! comprehensive variable tracking and type information during contract analysis.

use std::sync::{Arc, Mutex};

use alloy_primitives::U256;
use foundry_compilers::artifacts::VariableDeclaration;
use lazy_static::lazy_static;

lazy_static! {
    /// Global counter for generating unique variable identifiers (UVIDs). It is
    /// also the storage slot that a variable should be stored in storage during debugging.
    ///
    /// This mutex-protected counter ensures thread-safe generation of unique
    /// identifiers across multiple analysis contexts. The counter starts from
    /// `EDB_RUNTIME_VALUE_OFFSET` to avoid conflicts with other identifier spaces.
    pub static ref NEXT_UVID: Mutex<UVID> = Mutex::new(UVID(EDB_RUNTIME_VALUE_OFFSET));
}

/// The slot where the `edb_runtime_values` mapping is stored.
///
/// This constant represents the first 8 bytes of the keccak256 hash of the string
/// "EDB_RUNTIME_VALUE_OFFSET". It serves as the starting point for UVID generation
/// to ensure unique identifier spaces across different analysis contexts.
pub const EDB_RUNTIME_VALUE_OFFSET: u64 = 0x234c6dfc3bf8fed1;

/// A Universal Variable Identifier (UVID) is a unique identifier for a variable in a contract.
///
/// UVIDs provide a way to uniquely identify variables across different scopes,
/// contexts, and analysis passes. They are used internally by the analysis engine
/// to track variable relationships and dependencies.
///
/// # Examples
///
/// ```rust
/// use edb::analysis::variable::{UVID, new_uvid};
///
/// let uvid1 = new_uvid();
/// let uvid2 = new_uvid();
/// assert_ne!(uvid1, uvid2);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct UVID(u64);

impl UVID {
    /// Increment the UVID and return the previous value.
    ///
    /// This method atomically increments the internal counter and returns
    /// the previous value, ensuring each call produces a unique identifier.
    ///
    /// # Returns
    ///
    /// The previous UVID value before incrementing.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use edb::analysis::variable::UVID;
    ///
    /// let mut uvid = UVID(42);
    /// let previous = uvid.inc();
    /// assert_eq!(previous, UVID(42));
    /// assert_eq!(uvid, UVID(43));
    /// ```
    pub fn inc(&mut self) -> Self {
        let v = *self;
        self.0 += 1;
        v
    }
}

impl From<UVID> for u64 {
    /// Convert a UVID to its underlying u64 representation.
    fn from(uvid: UVID) -> Self {
        uvid.0
    }
}

impl From<UVID> for U256 {
    /// Convert a UVID to a U256 representation for use in Ethereum-related operations.
    fn from(uvid: UVID) -> Self {
        Self::from(uvid.0)
    }
}

/// Generate a new unique variable identifier (UVID).
///
/// This function provides a thread-safe way to generate unique identifiers
/// for variables. Each call returns a new UVID that is guaranteed to be
/// unique within the current analysis session.
///
/// # Returns
///
/// A new unique UVID.
///
/// # Examples
///
/// ```rust
/// use edb::analysis::variable::new_uvid;
///
/// let uvid1 = new_uvid();
/// let uvid2 = new_uvid();
/// assert_ne!(uvid1, uvid2);
/// ```
pub fn new_uvid() -> UVID {
    let mut uvid = NEXT_UVID.lock().unwrap();
    uvid.inc()
}

/// A reference-counted pointer to a Variable.
///
/// This type alias provides shared ownership of Variable instances, allowing
/// multiple parts of the analysis system to reference the same variable
/// without copying the data.
pub type VariableRef = Arc<Variable>;

/// Represents a variable in a smart contract with its metadata and type information.
///
/// The Variable struct contains all the information needed to track and analyze
/// a variable during contract analysis, including its unique identifier, name,
/// declaration details, type, and scope information.
///
/// # Examples
///
/// ```rust
/// use edb::analysis::variable::{Variable, UVID, VariableType, VariableScope};
/// use foundry_compilers::artifacts::VariableDeclaration;
///
/// let variable = Variable {
///     uvid: UVID(1),
///     name: "balance".to_string(),
///     declare: VariableDeclaration::default(),
///     ty: VariableType::Uint(256),
///     scope: VariableScope {},
/// };
/// ```
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Variable {
    /// Unique identifier for this variable.
    pub uvid: UVID,
    /// The variable's name as it appears in the source code.
    pub name: String,
    /// The original variable declaration from the compiler artifacts.
    pub declare: VariableDeclaration,
    /// The variable's type information.
    pub ty: VariableType,
    /// Information about the variable's scope and visibility.
    pub scope: VariableScope,
}

/// Represents the type of a smart contract variable.
///
/// This enum covers the basic Solidity types that are commonly used in
/// smart contract analysis. The types are designed to be extensible for
/// future additions.
///
/// # Examples
///
/// ```rust
/// use edb::analysis::variable::VariableType;
///
/// let uint_type = VariableType::Uint(256);
/// let address_type = VariableType::Address;
/// let bool_type = VariableType::Bool;
/// ```
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum VariableType {
    /// A `uint` type variable. The number of bits is specified by the parameter.
    ///
    /// For instance, `Uint(8)` denotes a `uint8` Solidity type, while `Uint(256)`
    /// represents a `uint256` (the default uint type in Solidity).
    Uint(u8),
    /// An `address` type variable representing an Ethereum address.
    ///
    /// This type is used for variables that store 20-byte Ethereum addresses.
    Address,
    /// A `bool` type variable representing a boolean value.
    ///
    /// This type is used for variables that can be either `true` or `false`.
    Bool,
}

/// Represents the scope and visibility information for a variable.
///
/// This structure contains information about where a variable is defined
/// and how it can be accessed. Currently, this is a placeholder structure
/// that can be extended with additional scope-related information as needed.
///
/// # Future Extensions
///
/// This structure may be extended to include:
/// - Function scope information
/// - Contract scope information
/// - Visibility modifiers (public, private, internal, external)
/// - Storage location (storage, memory, calldata)
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct VariableScope {}
