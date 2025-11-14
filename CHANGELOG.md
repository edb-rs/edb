# Changelog

All notable changes to EDB (Ethereum Debugger) will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Support for calldata variables ([#33](https://github.com/edb-rs/edb/pull/33))
- Add new `r` and `R` commands in code panel (`run`/`runback` in terminal panels) to run forward/backward until the next breakpoint
- Add `edb server` which collectively spawns edb debug server. ([#46](https://github.com/edb-rs/edb/pull/46))
- Add `release.yml` for automatic release publishing to GitHub Releases

### Fixed
- Struct fields are no longer be incorrectly treated as variables ([#33](https://github.com/edb-rs/edb/pull/33))
- Gas limit relaxation is now correctly applied at callsites ([#39](https://github.com/edb-rs/edb/issues/39))

### Changed

- Update `install.sh` to download the latest release from GitHub Releases first, and fallback to building from source if no releases are found.

## [0.0.2] - 2024-10-11

### Added
- Add expression watcher ([#7](https://github.com/edb-rs/edb/issues/7))
- Partially support integration tests for edb ([#6](https://github.com/edb-rs/edb/issues/6))
- Add a popup window when errors occur in TUI
- Add mouse interaction support in TUI ([#16](https://github.com/edb-rs/edb/issues/16))
- Support conditional, unconditional, and data breakpoints ([#9](https://github.com/edb-rs/edb/issues/9))
- Tracking transient storage changes in source-level snapshots
- Runtime path-based filtering via EDB_ASSERT environment variable for assertion
- Introduced statement body analysis for more accurate source-level debugging ([#21](https://github.com/edb-rs/edb/pull/21))

### Changed
- Improved horizontal scrolling support in terminal panel vim mode
- Update dependencies to match foundry [0867fc1](https://github.com/foundry-rs/foundry/commit/0867fc1)
- Extend CI to Windows and MacOS
- Improve the cache mechanism to avoid redundant downloads ([#10](https://github.com/edb-rs/edb/issues/10))
- Speed up health check in rpc proxy ([#11](https://github.com/edb-rs/edb/pull/11))
- Remove Web UI code and dependencies ([#15](https://github.com/edb-rs/edb/pull/15))
- Add more tests for common and rpc-proxy crates, and well as more end-to-end tests for engine crate
- Optimized snapshot memory usage and processing speed by selectively storing calldata/memory/storage changes only when necessary and using persistent data structures for stack.
- Refactored analysis module with improved AST abstractions for better maintainability and extensibility ([#21](https://github.com/edb-rs/edb/pull/21))

## [0.0.1] - 2024-09-19

### Added
- **Initial release** of EDB - Ethereum Debugger
- **Source-level debugging** for Solidity smart contracts
- **Time-travel capabilities** with step-by-step execution navigation
- **Local variable inspection** with real-time value tracking
- **Custom expression evaluation** using Solidity syntax
- **Terminal User Interface (TUI)** with vim-style navigation
- **RPC proxy system** with intelligent caching and load balancing
- **Transaction replay** functionality for mainnet and testnet transactions
- **Multi-chain support** for EVM-compatible networks

#### Core Components
- `edb` - Main CLI binary for transaction debugging
- `edb-rpc-proxy` - Intelligent RPC proxy with caching
- `edb-tui` - Terminal-based debugger interface
- `edb-engine` - Core debugging and instrumentation engine
- `edb-common` - Shared utilities and types

#### Key Features
- **Bytecode instrumentation** for source-level debugging without relying on fragile source maps
- **Smart contract intelligence** with automatic ABI detection and decoding
- **Expression evaluator** supporting arbitrary Solidity expressions during debugging
- **Flexible navigation** with vim-style keybindings and time-travel controls
- **Performance optimization** through RPC caching and efficient state management

#### Debugging Capabilities
- Step through Solidity code line-by-line
- Inspect local variables, function parameters, and contract state
- Navigate function calls and returns naturally
- Jump to specific execution points
- Evaluate custom expressions against current execution state
- View opcodes and EVM state when needed

#### User Interface
- Full-featured terminal UI with syntax highlighting
- Multiple panel layout (code, variables, terminal, stack trace)
- Vim-style navigation with support for movement commands
- Real-time status updates and progress indicators
- Horizontal and vertical scrolling support

#### Technical Architecture
- Built on REVM for fast and accurate EVM simulation
- Modular crate structure for maintainability
- Comprehensive error handling and logging
- Extensible plugin architecture for future enhancements

### Dependencies
- Rust 1.89+ required
- REVM v27 for EVM simulation
- Ratatui for terminal user interface
- Tokio for async runtime
- Alloy for Ethereum type definitions

### Known Limitations
- Source code must be available and verified for full debugging capabilities
- Some advanced Solidity features may have limited support
- Performance may vary with complex contracts and long execution traces

---

## Release Notes

### Version 0.0.1
This is the initial public release of EDB, representing months of development and testing. While marked as 0.0.1, the debugger is functional and can handle real-world debugging scenarios for most Solidity contracts.

**Feedback Welcome!**
This is an early release and we're actively seeking feedback from the Ethereum development community. Please report issues, request features, and share your debugging experiences through GitHub Issues.

---

**Note**: Versions prior to 0.0.1 were internal development releases and are not documented in this changelog.
