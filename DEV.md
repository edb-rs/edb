# EDB Development Guide

This document provides comprehensive guidance for developing the Ethereum Debugger (EDB).

## Prerequisites

### System Requirements
- **Rust**: 1.88+ (upgraded from 1.87 for latest Foundry compatibility)
- **Git**: For version control
- **Ethereum RPC Access**: For testing with real transactions

### Dependencies
EDB uses Foundry's exact dependency versions to ensure compatibility:
- **alloy-primitives**: 1.3.0
- **alloy-provider**: 1.0.23 (with `reqwest` feature)
- **alloy-rpc-types**: 1.0.23
- **alloy-transport-http**: 1.0.23 (with `reqwest` feature)
- **revm**: 27.1.0
- **foundry-compilers**: 0.18.2

## Project Structure

```
EDB/
├── Cargo.toml              # Workspace configuration
├── crates/
│   ├── edb/                # Main binary crate
│   │   ├── src/main.rs     # CLI and orchestration
│   │   └── Cargo.toml
│   ├── engine/             # Analysis and instrumentation
│   │   ├── src/
│   │   │   ├── lib.rs      # Core analyze() function
│   │   │   ├── source.rs   # Source code download
│   │   │   ├── compiler.rs # Contract recompilation
│   │   │   ├── analysis/   # Analysis modules
│   │   │   ├── instrumentation/ # Standalone directory
│   │   │   └── rpc/        # JSON-RPC server (standalone)
│   │   ├── tests/
│   │   │   └── config_tests.rs # Configuration tests
│   │   └── Cargo.toml
│   ├── utils/              # Chain interaction utilities
│   │   ├── src/
│   │   │   ├── lib.rs      # Types and public interface
│   │   │   ├── forking.rs  # Chain forking with REVM execution
│   │   │   └── spec_id.rs  # Ethereum hardfork mapping
│   │   ├── tests/
│   │   │   └── forking_tests.rs # Comprehensive forking tests
│   │   └── Cargo.toml
│   ├── tui/                # Terminal UI (skeleton)
│   │   └── src/lib.rs
│   └── webui/              # Web UI (skeleton)
│       └── src/lib.rs
├── ARCH.md                 # Architecture documentation
├── DEV.md                  # This file
└── README.md               # User-facing documentation
```

## Development Workflow

### Before Pushing to GitHub

Please ensure the following commands pass if you have changed the code:

```bash
# Check compilation
cargo check --all

# Run tests
cargo test --all --all-features

# Run specific test suites
cargo test -p edb-common --test forking_tests
cargo test -p edb-engine --test config_tests

# Format code
cargo +nightly fmt -- --check

# Lint code
cargo clippy --all --all-targets --all-features -- -D warnings

# Build release version
cargo build --release
```

### Testing

#### Running the Debugger
```bash
# Basic usage - replay a transaction
cargo run -- replay 0x<transaction_hash>

# With debug logging
RUST_LOG=debug cargo run -- replay 0x<transaction_hash>

# Use web UI instead of TUI
cargo run -- --ui web replay 0x<transaction_hash>

# Specify custom RPC endpoint
cargo run -- --rpc-url https://mainnet.infura.io/v3/YOUR_KEY replay 0x<transaction_hash>
```

#### Testing Chain Forking
```bash
# Test with a known transaction (requires RPC access)
RUST_LOG=debug cargo run -- replay 0x5c504ed432cb51138bcf09aa5e8a410dd4a1e204ef84bfed1be16dfba1b22060

# Run forking integration tests
ETH_RPC_URL=https://eth.llamarpc.com cargo test -p edb-common --test forking_tests -- --ignored
```

## Development Areas

### 1. Utils Crate (`crates/utils`)

**Status: ✅ Complete with Tests**

The utils crate handles all chain interaction with actual REVM execution:

#### Key Components:
- **`fork_and_prepare()`**: Creates chain forks and executes preceding transactions
  - Returns `ForkResult` with `context`, `fork_info`, and `target_tx_env`
  - Uses REVM's `transact_commit()` for actual transaction execution
  - Properly sets up block environment and chain configuration
  
- **`get_tx_env_from_tx()`**: Converts Alloy Transaction to REVM TxEnv
  - Handles regular transactions, contract creation, and EIP-2930 access lists
  - Properly converts all transaction fields including gas, value, and nonce

- **`get_mainnet_spec_id()`**: Maps block numbers to Ethereum hardfork SpecIds
  - Uses global `LazyLock<BTreeMap>` for efficient lookups
  - Correctly handles Constantinople/Petersburg at same block height
  - Covers all mainnet hardforks from Frontier to Cancun

#### Implementation Details:
- Uses `CacheDB` with `AlloyDB` backend for forked state
- Executes transactions with `MainnetEvm::transact_commit()`
- Tracks execution results (Success, Revert, Halt)
- Progress bar integration for better UX during replay

#### Test Coverage:
- Unit tests for transaction conversion and SpecId mapping
- Integration tests with real mainnet transactions
- Tests for contract creation and EIP-2930 access lists

### 2. Engine Crate (`crates/engine`)

**Status: 🚧 Partial Implementation**

The engine accepts pre-forked inputs and performs analysis:

#### Current State:
- `EngineConfig` reduced to only `rpc_port` and `etherscan_api_key`
- `analyze()` function uses placeholder types (to be replaced with real Context/TxEnv)
- RPC server structure is in place

#### Areas for Development:
- **Source Download** (`source.rs`): Implement Etherscan API integration
- **Instrumentation** (`instrumentation/`): Solidity parsing and precompile injection
- **Compiler** (`compiler.rs`): Contract recompilation with foundry-compilers
- **Analysis** (`analysis/`): Complete visitor pattern implementation
- **Inspector** (`inspector/`): Create custom REVM inspector for collecting visited addresses

### 3. EDB Binary (`crates/edb`)

**Status: ✅ Updated for New API**

The main binary orchestrates the workflow:
- CLI parsing with clap
- Calls `fork_and_prepare()` directly with RPC URL
- Extracts fork info to create placeholders for engine
- Launches selected UI (TUI or Web)

### 4. UI Crates

**Status: 📋 Skeleton Only**

Both TUI and WebUI crates have basic structure but need implementation.

## Key Development Patterns

### REVM Transaction Execution
```rust
// Create context with database
let ctx = Context::mainnet()
    .with_db(state)
    .modify_block_chained(|b| {
        b.number = U256::from(block_number);
        b.timestamp = U256::from(timestamp);
        // ... other block setup
    });

// Build and execute EVM
let mut evm = ctx.build_mainnet();
match evm.transact_commit(tx_env) {
    Ok(ExecutionResult::Success { gas_used, .. }) => {
        info!("Transaction executed successfully");
    }
    Ok(ExecutionResult::Revert { output, .. }) => {
        warn!("Transaction reverted: {:?}", output);
    }
    Ok(ExecutionResult::Halt { reason, .. }) => {
        error!("Transaction halted: {:?}", reason);
    }
    Err(e) => {
        error!("Transaction failed: {:?}", e);
    }
}
```

### Error Handling
```rust
use eyre::Result;

pub async fn my_function() -> Result<ReturnType> {
    // Use ? operator for error propagation
    let result = some_operation().await?;
    Ok(result)
}
```

### Logging
```rust
use tracing::{info, debug, warn, error};

pub fn my_function() {
    info!("High-level operation started");
    debug!("Detailed debugging info");
    warn!("Something concerning happened");
    error!("Something went wrong: {}", error_message);
}
```

### Async Provider Usage
```rust
use alloy_provider::ProviderBuilder;

let provider = ProviderBuilder::new()
    .connect(&rpc_url)
    .await?;

let tx = provider
    .get_transaction_by_hash(tx_hash)
    .await?
    .ok_or_else(|| eyre::eyre!("Transaction not found"))?;
```

## Common Issues and Solutions

### 1. REVM v27 API Changes
**Problem**: REVM v27 has significant API changes from earlier versions
**Solution**: 
- Use `Context` instead of separate `Env` and `Database`
- Access EVM context via `evm.ctx` not `evm.into_context()`
- Use `MainnetEvm` type alias for standard Ethereum setup

### 2. Alloy Transport Features
**Problem**: "No transports enabled" error when connecting to RPC
**Solution**: Enable `reqwest` feature in alloy-provider and alloy-transport-http

### 3. Transaction Field Access
**Problem**: Transaction fields moved to `tx.inner.*`
**Solution**: 
- Use trait methods like `tx.gas_limit()`, `tx.value()`, `tx.nonce()`
- For signer, use `tx.inner.signer()`

### 4. SpecId Variants
**Problem**: SpecId enum variants are ALL_CAPS in revm v27
**Solution**: Use `SpecId::LONDON` not `SpecId::London`

### 5. Access List Structure
**Problem**: AccessListItem fields changed
**Solution**: Use `.address` and `.storage_keys` fields instead of tuple access

## Testing Guidelines

### Unit Tests
- Test pure functions and type conversions
- Mock external dependencies
- Focus on edge cases and error conditions

### Integration Tests
- Use `#[ignore]` attribute for tests requiring RPC access
- Provide ETH_RPC_URL environment variable for running
- Test with known mainnet transactions for reproducibility

Example:
```bash
# Run all tests including ignored integration tests
ETH_RPC_URL=https://eth.llamarpc.com cargo test -- --ignored
```

## Git Commit Guidelines

Use conventional commit messages:

+ **feat**: A new feature for the user
+ **fix**: A bug fix
+ **docs**: Documentation only changes
+ **style**: Code style changes (formatting, etc.)
+ **refactor**: Code restructuring without behavior change
+ **perf**: Performance improvements
+ **test**: Adding or updating tests
+ **chore**: Build system or tooling changes
+ **ci**: CI configuration changes
+ **build**: Build system or dependency changes
+ **revert**: Reverting a previous commit

Examples:
- `feat: implement REVM transaction execution in forking.rs`
- `fix: correct SpecId mapping for Constantinople/Petersburg`
- `test: add comprehensive forking tests with real transactions`
- `refactor: use global BTreeMap for SpecId lookups`
- `docs: update development guide with REVM v27 patterns`