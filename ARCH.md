# EDB Architecture

## Overview

EDB (Ethereum Debugger) is a step-by-step debugger for Ethereum transactions, designed to provide deep visibility into smart contract execution. It works by instrumenting contract bytecode with debugging hooks and replaying transactions in a controlled environment.

## System Architecture

```
┌─────────────────┐     ┌─────────────────┐
│   CLI (edb)     │     │   Web Browser   │
└────────┬────────┘     └────────┬────────┘
         │                       │
         │                       │
┌────────▼────────┐     ┌────────▼────────┐
│   TUI Module    │     │  WebUI Module   │
└────────┬────────┘     └────────┬────────┘
         │                       │
         └───────────┬───────────┘
                     │
                     │ JSON-RPC
                     │
            ┌────────▼────────┐
            │  Engine Module  │
            │                 │
            │ ┌─────────────┐ │
            │ │   Source    │ │
            │ │  Download   │ │
            │ ├─────────────┤ │
            │ │Instrumenter │ │
            │ ├─────────────┤ │
            │ │  Compiler   │ │
            │ ├─────────────┤ │
            │ │ RPC Server  │ │
            │ └─────────────┘ │
            └─────┬───────────┘
                  │
                  │ Uses
                  │
            ┌─────▼───────────┐
            │  Utils Module   │
            │                 │
            │ ┌─────────────┐ │
            │ │   Forking   │ │
            │ │   & Chain   │ │
            │ │   Replay    │ │
            │ └─────────────┘ │
            └─────┬───────────┘
                  │
                  │
            ┌─────▼───────────┐
            │ Ethereum Node   │
            │   (via RPC)     │
            └─────────────────┘
```

## Core Components

### 1. EDB Binary (`crates/edb`)

The main entry point that:
- Parses command-line arguments using clap
- **Handles chain forking and transaction replay** (calls utils crate)
- Prepares database and environment for debugging
- Calls engine with prepared inputs
- Launches the selected UI (TUI or Web)

**Key workflow:**
1. Fork chain at target transaction's block
2. Replay preceding transactions in the block
3. Call `engine::analyze()` with forked database and environment
4. Launch UI with results

### 2. Utils Module (`crates/utils`)

**NEW** - Shared utilities for chain interaction:

#### a. Forking Module (`forking.rs`)
- **`fork_and_prepare()`**: Creates chain fork and prepares environment
- **`replay_transaction()`**: Analyzes transactions to find touched contracts
- **`analyze_block_transactions()`**: Identifies transaction dependencies
- Uses Alloy provider for Ethereum RPC communication
- Returns `ForkResult` with database, environment, and fork info

### 3. Engine Module (`crates/engine`)

The core analysis and instrumentation engine containing:

#### a. Analysis Module (`analysis/`)
- **Accepts pre-forked database and environment as inputs**
- Performs instrumentation on already-identified contracts
- No longer handles chain forking (moved to utils)

#### b. Source Download Module (`source.rs`)
- Downloads verified source code from Etherscan and other explorers
- Supports multiple block explorers with fallback
- Caches downloaded sources

#### c. Instrumentation Directory (`instrumentation/`)
- **Standalone directory** (not embedded in analysis)
- Parses Solidity source code
- Injects debugging precompile calls at function entry points
- Preserves original contract semantics

#### d. Compiler Module (`compiler.rs`)
- Recompiles instrumented contracts using foundry-compilers
- Manages compilation artifacts
- Handles complex multi-file contracts

#### e. RPC Server Directory (`rpc/`)
- **Standalone directory** (not embedded in analysis)
- Provides JSON-RPC interface for UI communication
- Supports debugging commands (step, continue, inspect)
- Manages debugging session state

### 4. TUI Module (`crates/tui`)

Terminal-based user interface that:
- Connects to the engine via JSON-RPC
- Displays transaction execution state
- Provides interactive debugging controls
- **Currently skeleton implementation**

### 5. WebUI Module (`crates/webui`)

Browser-based user interface that:
- Serves a web application
- Connects to the engine via WebSocket/JSON-RPC
- Provides rich visualization of execution
- **Currently skeleton implementation**

## Debugging Workflow

1. **Transaction Selection**
   - User provides transaction hash via CLI: `edb replay <tx_hash>`
   - EDB binary parses arguments and identifies target transaction

2. **Chain Forking** (EDB Binary → Utils)
   - EDB calls `utils::fork_and_prepare()` with target transaction hash
   - Utils connects to Ethereum RPC using Alloy provider
   - Creates fork at target transaction's block
   - Identifies preceding transactions that need replay
   - Returns `ForkResult` with database, environment, and fork info

3. **Engine Analysis** (EDB Binary → Engine)
   - EDB calls `engine::analyze()` with forked database and environment
   - Engine accepts pre-prepared inputs (no chain interaction)

4. **Contract Analysis** (Engine)
   - Uses transaction receipts to collect touched contract addresses
   - Downloads source code for each contract from block explorers
   - Caches sources for future use

5. **Instrumentation** (Engine)
   - Parse Solidity source code using solang
   - Inject precompile calls at debugging points (address `0x000...023333`)
   - Recompile contracts with instrumentation using foundry-compilers

6. **State Preparation** (Engine)
   - Replace original bytecode with instrumented versions
   - Reconstruct deployment state in forked environment

7. **Debugging Execution** (Engine → UI)
   - Re-execute transaction with instrumentation
   - Capture state snapshots at each debugging point
   - Provide JSON-RPC interface for UI control

8. **User Interface**
   - Launch TUI or Web UI based on CLI flag
   - Connect to engine's JSON-RPC server
   - Provide step-by-step debugging controls

## Key Design Decisions

### Instrumentation via Precompiles

We use a special precompile address (`0x000...023333`) for debugging hooks. This approach:
- Minimizes gas overhead
- Preserves contract behavior
- Enables efficient state capture

### Fork-based Isolation

Each debugging session operates on an isolated fork, ensuring:
- No interference with live networks
- Reproducible debugging sessions
- Safe experimentation

### Source-level Debugging

By working with source code rather than bytecode:
- Developers debug in familiar Solidity
- Variable names and structure are preserved
- Complex logic is easier to follow

### Separation of Concerns

**NEW** - Clear architectural separation:
- **EDB Binary**: Orchestration and forking workflow
- **Utils Crate**: Chain interaction and transaction replay
- **Engine Crate**: Analysis and instrumentation (no RPC dependencies)
- **UI Crates**: Interface and visualization (skeleton only)

## Security Considerations

- Never expose debugging infrastructure to untrusted networks
- API keys for block explorers should be kept secure
- Instrumented contracts should never be deployed to mainnet

## Performance Considerations

- Source download is cached to avoid repeated API calls
- Compilation uses optimized settings for faster builds
- State snapshots are created on-demand to minimize overhead

## Implementation Status

### ✅ Completed
- **Project structure**: 5-crate workspace (edb, engine, utils, tui, webui)
- **CLI interface**: Argument parsing with clap, transaction replay command
- **Chain forking**: Full implementation with Alloy provider integration
- **Transaction replay**: Analysis of transaction receipts and touched contracts
- **Architecture**: Proper separation between forking (utils) and analysis (engine)
- **Compilation**: Works with Rust 1.88 and latest Foundry dependencies

### 🚧 In Development
- **Engine analysis**: Core `analyze()` function accepts forked inputs
- **Source download**: Basic structure for Etherscan integration
- **Instrumentation**: Directory structure for bytecode modification
- **RPC server**: JSON-RPC interface for UI communication

### 📋 Todo
- **Complete engine implementation**: Source download, instrumentation, compilation
- **UI implementations**: TUI and WebUI beyond skeleton
- **Real transaction testing**: Integration with live Ethereum data
- **Documentation completion**: Full dev.md and updated README

## Future Extensions

- Breakpoint support with conditional logic
- Watch expressions for variable monitoring
- Time-travel debugging with state snapshots
- Multi-transaction debugging sessions
- Integration with development frameworks (Hardhat, Foundry)
- Support for additional block explorers beyond Etherscan