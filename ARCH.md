# EDB Architecture Documentation

## Overview

EDB (Ethereum Debugger) is a sophisticated time-travel debugger for Ethereum transactions that provides source-level debugging capabilities through bytecode instrumentation and comprehensive state snapshots. The system achieves this by replaying transactions in a controlled environment with instrumented contracts that capture execution state at strategic points.

## System Architecture

```
┌──────────────────────────────────────────────────────────────┐
│                         User Interface                       │
├─────────────────────────┬────────────────────────────────────┤
│      Terminal UI        │           Web UI                   │
│       (TUI Crate)       │        (WebUI Crate)              │
└───────────┬─────────────┴──────────────┬────────────────────┘
            │                            │
            │      JSON-RPC API          │
            └────────────┬───────────────┘
                         │
┌────────────────────────▼─────────────────────────────────────┐
│                    Engine Module                             │
│  ┌─────────────────────────────────────────────────────┐    │
│  │                 Core Engine (core.rs)                │    │
│  ├─────────────────────────────────────────────────────┤    │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────────────┐  │    │
│  │  │  Source  │  │ Analysis │  │  Instrumentation │  │    │
│  │  │ Download │  │  (AST)   │  │   & Compilation  │  │    │
│  │  └──────────┘  └──────────┘  └──────────────────┘  │    │
│  ├─────────────────────────────────────────────────────┤    │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────────────┐  │    │
│  │  │  Tweak   │  │ Inspector│  │    Snapshots     │  │    │
│  │  │ Bytecode │  │  (REVM)  │  │  (Dual-Layer)    │  │    │
│  │  └──────────┘  └──────────┘  └──────────────────┘  │    │
│  ├─────────────────────────────────────────────────────┤    │
│  │              RPC Server (server.rs)                  │    │
│  └─────────────────────────────────────────────────────┘    │
└────────────────────┬─────────────────────────────────────────┘
                     │
┌────────────────────▼─────────────────────────────────────────┐
│                    Common Module                             │
│  ┌─────────────────────────────────────────────────────┐    │
│  │   • Forking & Transaction Replay (forking.rs)        │    │
│  │   • EVM Context Management (context.rs)              │    │
│  │   • Caching Infrastructure (cache.rs)                │    │
│  │   • Hardfork Specification (spec_id.rs)              │    │
│  │   • Shared Types (types/)                            │    │
│  └─────────────────────────────────────────────────────┘    │
└────────────────────┬─────────────────────────────────────────┘
                     │
┌────────────────────▼─────────────────────────────────────────┐
│                 RPC Proxy Module (Optional)                  │
│  ┌─────────────────────────────────────────────────────┐    │
│  │   • Multi-Provider Load Balancing                    │    │
│  │   • Immutable Response Caching                       │    │
│  │   • Health Monitoring & Metrics                      │    │
│  └─────────────────────────────────────────────────────┘    │
└────────────────────┬─────────────────────────────────────────┘
                     │
┌────────────────────▼─────────────────────────────────────────┐
│              Ethereum Network (via RPC)                      │
└───────────────────────────────────────────────────────────────┘
```

## Core Components

### 1. EDB Binary (`crates/edb`)

The main orchestrator that coordinates the entire debugging workflow:

- **CLI Interface**: Command-line argument parsing and validation
- **Proxy Management**: Ensures RPC proxy is running for optimal performance
- **Workflow Orchestration**: Coordinates forking, engine preparation, and UI launch
- **UI Selection**: Launches appropriate interface (TUI or Web) based on user preference

**Key Files:**
- `main.rs`: Entry point and CLI handling
- `proxy.rs`: RPC proxy lifecycle management
- `cmd/replay.rs`: Transaction replay command implementation
- `cmd/debug.rs`: Foundry test debugging (future)

### 2. Common Module (`crates/common`)

Shared utilities and core functionality used across all components:

#### Forking & Replay (`forking.rs`)
- **Real REVM Execution**: Uses `transact_commit()` for accurate state changes
- **Block History Replay**: Replays all preceding transactions in a block
- **Progress Tracking**: Visual progress bars for long replay operations
- **Fork Result**: Returns prepared context, environment, and fork information

#### Context Management (`context.rs`)
- **EdbContext**: Wrapper around REVM Context with debugging extensions
- **EdbDB**: Database wrapper for state management
- **Helper Methods**: Simplified EVM building and transaction execution

#### Caching (`cache.rs`)
- **Multi-Level Caching**: Etherscan, RPC, and compilation caches
- **TTL Support**: Different expiration times for different data types
- **Persistent Storage**: File-based caching across sessions

#### Hardfork Specification (`spec_id.rs`)
- **Mainnet Mapping**: Block numbers to Ethereum hardfork SpecIds
- **All Hardforks**: From Frontier to Cancun and beyond
- **Efficient Lookup**: BTreeMap-based implementation

### 3. Engine Module (`crates/engine`)

The heart of EDB's debugging capabilities:

#### Core Engine (`core.rs`)
- **Preparation Workflow**: Orchestrates the entire debugging preparation
- **Six-Step Process**:
  1. Replay & trace collection
  2. Source code download
  3. Opcode snapshot collection
  4. Source code analysis
  5. Code instrumentation & compilation
  6. Hook snapshot collection

#### Analysis Module (`analysis/`)
- **AST Analysis**: Parses Solidity AST to identify execution steps
- **Variable Scope Tracking**: Manages variable visibility across scopes
- **Step Partitioning**: Breaks code into debuggable execution steps
- **Visibility Analysis**: Identifies private functions/variables needing modification

**Key Components:**
- `analyzer.rs`: Main AST walker and analysis logic
- `variable.rs`: Variable scope and lifetime management
- `step.rs`: Execution step identification
- `hook.rs`: Hook placement strategies

#### Instrumentation (`instrumentation/`)
- **Source Modification**: Inserts debugging hooks at strategic points
- **Hook Types**:
  - `BeforeStep`: Before executing a statement
  - `VariableInScope`: When variable becomes visible
  - `VariableOutOfScope`: When variable leaves scope
  - `VariableUpdate`: When variable value changes
- **Compilation**: Uses Foundry's solc wrapper for recompilation

#### Inspector Module (`inspector/`)
- **Call Tracer**: Captures all contract calls and interactions
- **Opcode Inspector**: Fine-grained instruction-level snapshots
- **Hook Inspector**: Captures state at instrumentation points
- **Tweak Inspector**: Manages bytecode replacement

#### Bytecode Tweaking (`tweak.rs`)
- **Creation TX Discovery**: Finds contract creation transactions
- **Init Code Extraction**: Gets original deployment bytecode
- **Code Replacement**: Swaps original with instrumented bytecode
- **Constructor Handling**: Preserves constructor arguments

#### Snapshot Management (`snapshot.rs`)
- **Dual-Layer System**: Merges opcode and hook snapshots
- **Unified Interface**: Single snapshot list for debugging
- **Frame Tracking**: Maintains call stack relationships
- **State Capture**: Complete EVM state at each point

#### RPC Server (`rpc/`)
- **JSON-RPC API**: Standard debugging protocol
- **Methods**:
  - `edb_getTrace`: Get execution trace
  - `edb_getSnapshot`: Get state at specific point
  - `edb_navigate`: Move through execution
- **Thread-Safe**: Uses Arc for shared state access

### 4. RPC Proxy Module (`crates/rpc-proxy`)

Intelligent caching proxy for improved performance:

#### Features
- **Multi-Provider Support**: Load balancing across multiple RPC endpoints
- **Smart Caching**: Caches immutable blockchain data
- **Health Monitoring**: Automatic failover for unreliable providers
- **Metrics Collection**: Performance and usage statistics
- **Process Management**: Heartbeat-based lifecycle management

#### Architecture
- **Registry**: Manages provider pool and selection
- **Cache**: In-memory and persistent caching layers
- **Proxy Server**: Intercepts and routes RPC calls
- **TUI**: Optional monitoring interface

### 5. TUI Module (`crates/tui`)

Rich terminal-based debugging interface:

#### Panels
- **Code Panel**: Syntax-highlighted source with execution position
- **Trace Panel**: Interactive call hierarchy navigation
- **Terminal Panel**: Command input and output
- **Display Panel**: Variable and state inspection

#### Features
- **Syntax Highlighting**: Solidity and opcode highlighting
- **Theme Support**: Customizable color schemes
- **Keyboard Navigation**: Vi-like keybindings
- **Resource Management**: Efficient terminal rendering

### 6. WebUI Module (`crates/webui`)

Browser-based debugging interface (in development):

- **Axum Server**: Modern async web framework
- **REST API**: HTTP interface to engine
- **WebSocket**: Real-time debugging updates
- **React Frontend**: (Planned) Interactive web application

## Debugging Workflow

### Phase 1: Transaction Preparation

1. **Parse Arguments**: Extract transaction hash and options
2. **Start RPC Proxy**: Ensure caching proxy is running
3. **Fork Chain**: Create isolated fork at target block
4. **Replay History**: Execute preceding transactions

### Phase 2: Analysis & Instrumentation

1. **Collect Traces**: Identify all touched contracts
2. **Download Sources**: Fetch from Etherscan with caching
3. **Analyze AST**: Parse code structure and identify steps
4. **Insert Hooks**: Add debugging instrumentation
5. **Recompile**: Generate instrumented bytecode

### Phase 3: Execution & Snapshots

1. **Tweak Bytecode**: Replace original with instrumented
2. **Collect Opcode Snapshots**: Fine-grained state capture
3. **Collect Hook Snapshots**: Source-level breakpoints
4. **Merge Snapshots**: Create unified debugging timeline

### Phase 4: Interactive Debugging

1. **Start RPC Server**: Launch debugging API
2. **Launch UI**: Start TUI or Web interface
3. **Connect to Engine**: Establish RPC connection
4. **Debug Session**: Step through execution interactively

## Key Design Decisions

### 1. Dual-Layer Snapshot System

**Rationale**: Combines benefits of both approaches
- **Opcode Snapshots**: Complete coverage, fine granularity
- **Hook Snapshots**: Source-level accuracy, semantic meaning
- **Intelligent Merging**: Prioritizes hook snapshots when available

### 2. REVM Integration

**Benefits**:
- **Accuracy**: Real EVM execution, not simulation
- **Performance**: Optimized Rust implementation
- **Flexibility**: Custom inspectors and modifications
- **Compatibility**: Supports all Ethereum hardforks

### 3. Modular Architecture

**Advantages**:
- **Separation of Concerns**: Each crate has clear responsibility
- **Reusability**: Components can be used independently
- **Testability**: Isolated units are easier to test
- **Maintainability**: Changes are localized

### 4. Caching Strategy

**Levels**:
1. **RPC Proxy**: Network-level caching
2. **Etherscan Cache**: Source code caching
3. **Compilation Cache**: Artifact caching
4. **State Cache**: Snapshot caching

### 5. Source-Level Debugging

**Why Source, Not Bytecode**:
- **Developer Friendly**: Work with familiar Solidity
- **Semantic Understanding**: Preserve high-level concepts
- **Variable Names**: Keep original identifiers
- **Control Flow**: Understand program logic

## Performance Optimizations

### Caching
- **Immutable Data**: Blocks, transactions, receipts cached forever
- **TTL-Based**: Different expiration for different data types
- **Memory + Disk**: Hot data in memory, cold on disk

### Parallelization
- **Async I/O**: Non-blocking network operations
- **Concurrent Downloads**: Parallel source fetching
- **Batch Processing**: Group related operations

### Lazy Evaluation
- **On-Demand Snapshots**: Only capture when needed
- **Incremental Compilation**: Recompile only changed contracts
- **Selective Instrumentation**: Instrument only relevant code

## Security Considerations

### Isolation
- **Forked Environment**: No interaction with live network
- **Sandboxed Execution**: Controlled EVM environment
- **Read-Only by Default**: Explicit permission for state changes

### API Security
- **Local Only**: RPC server binds to localhost
- **Authentication**: (Future) Token-based access control
- **Rate Limiting**: (Future) Prevent resource exhaustion

### Data Protection
- **API Key Management**: Secure storage of credentials
- **Cache Encryption**: (Future) Encrypted cache storage
- **Audit Logging**: (Future) Track all debugging operations

## Future Enhancements

### Near Term
- **Complete Web UI**: Full-featured browser interface
- **Foundry Integration**: Direct test debugging support
- **Breakpoint Conditions**: Conditional breakpoints
- **Watch Expressions**: Monitor specific variables

### Medium Term
- **Multi-Chain Support**: Polygon, Arbitrum, Optimism
- **Collaborative Debugging**: Share debugging sessions
- **Execution Recording**: Save and replay sessions
- **Advanced Analysis**: Gas profiling, security checks

### Long Term
- **AI Assistant**: Intelligent debugging suggestions
- **Formal Verification**: Integration with verification tools
- **Custom Inspectors**: Plugin system for extensions
- **Cloud Debugging**: Remote debugging infrastructure

## Testing Strategy

### Unit Tests
- **Pure Functions**: Type conversions, utilities
- **Isolated Components**: Individual module testing
- **Mock Dependencies**: Simulated external services

### Integration Tests
- **Real Transactions**: Mainnet transaction replay
- **End-to-End**: Complete debugging workflow
- **Performance**: Benchmark critical paths

### Test Coverage Areas
- **Forking**: Different block heights and transactions
- **Analysis**: Various Solidity constructs
- **Instrumentation**: Edge cases in code modification
- **Snapshots**: State capture accuracy

---

*This architecture documentation was crafted with Claude with Love ❤️*