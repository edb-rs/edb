# EDB Development Roadmap

## Overview

This document outlines the development roadmap for EDB (Ethereum Debugger), organized into milestones focusing on core functionality, performance, and user experience improvements.

## Milestones

### 🎯 Milestone 1: Enhanced Debugging Capabilities
**Goal**: Expand core debugging features to support more complex scenarios

- [ ] **Complex Type Variable Watcher**
  - Support user-defined types (structs, enums, mappings)
  - Implement alternative collection methods for non-ABI-encodable types
  - Add nested type inspection capabilities

- [ ] **Conditional Breakpoints**
  - Implement expression-based breakpoint conditions
  - Support state-dependent breakpoints
  - Add breakpoint management UI

- [ ] **Custom Expression Evaluation**
  - Enable custom watcher expressions
  - Support runtime expression evaluation
  - Add expression history and favorites

### 🤖 Milestone 2: Intelligence & Decoding
**Goal**: Enhanced contract understanding and automatic decoding capabilities

- [ ] **Smart Contract Intelligence**
  - Automatic address labeling from online resources
  - Integration with contract verification services
  - Known contract pattern detection

- [ ] **Selector Database Integration**
  - Function signature decoding without source code
  - Event signature recognition
  - Integration with 4byte directory and similar services

### 🚀 Milestone 3: Performance & Architecture
**Goal**: Optimize memory usage and improve snapshot mechanisms

- [ ] **Memory Optimization**
  - Implement trait objects for variable/state inheritance
  - Optimize accessible variables storage per step
  - Reduce redundant state snapshot storage

- [ ] **Improved Snapshot System**
  - Replace `call` opcode approach with bytecode pattern injection
  - Implement custom inspector for pattern-based snapshots
  - Remove function mutability requirements

- [ ] **Contract Creation Handling**
  - Fix snapshot issues when replayed transaction creates new contracts
  - Support debugging of constructor execution
  - Track contract deployment context and initialization
  - Handle CREATE and CREATE2 opcodes properly during replay

### 🔧 Milestone 4: Tool Integration
**Goal**: Seamless integration with popular Ethereum development tools

- [ ] **Foundry Test Debugging**
  - Full support for debugging Foundry test cases
  - Integration with forge test runner
  - Test failure root cause analysis

- [ ] **Hardhat Test Support**
  - Support for Hardhat test debugging
  - Integration with Hardhat network
  - Compatibility with Hardhat plugins

### 🌐 Milestone 5: Multi-Chain & L2 Support
**Goal**: Extend debugging capabilities beyond Ethereum mainnet with L2-specific features

- [ ] **Chain Support Expansion**
  - Support for major EVM chains (Polygon, BSC, Arbitrum, Optimism)
  - Chain-specific configuration handling
  - Multi-chain RPC endpoint management

- [ ] **L2-Specific Features**
  - Optimism/Arbitrum sequencer awareness and debugging
  - L1<->L2 message tracking and cross-layer debugging
  - L2 gas optimization analysis
  - Rollup-specific transaction lifecycle visualization

### 🧠 Milestone 6: AI & Automation
**Goal**: Leverage AI for advanced debugging assistance

- [ ] **Agent MCP Support**
  - Implement Model Context Protocol for AI assistance
  - Enable automated debugging suggestions
  - Support for debugging pattern recognition

### 📚 Milestone 7: Advanced Features
**Goal**: Provide advanced debugging capabilities for complex scenarios

- [ ] **Debugger-Friendly Decompilation**
  - Solidity decompilation for contracts without source
  - Readable bytecode representation
  - Source mapping reconstruction

### 🔄 Milestone 8: State Manipulation & What-If Analysis
**Goal**: Enable users to modify state and explore alternative execution paths

- [ ] **User State Modification**
  - Interactive state editing during debugging
  - Modify storage variables, balances, and contract code
  - Fork from any execution point with modified state

- [ ] **What-If Scenario Testing**
  - Save and load state snapshots
  - Compare execution paths with different initial states
  - Automated scenario generation for edge cases
  - State diff visualization between scenarios

### 🌟 Milestone 9: Enhanced Web UI
**Goal**: Develop a modern, feature-rich web interface

- [ ] **Modern Web UI Development**
  - React/Vue-based responsive interface
  - Real-time debugging with WebSocket connections
  - Drag-and-drop transaction analysis
  - Multi-tab support for parallel debugging sessions

- [ ] **Advanced UI Features**
  - Interactive call graph visualization
  - Gas usage heatmap
  - Storage and memory layout visualizers
  - Collaborative debugging with session sharing

### ✅ Milestone 10: Comprehensive Testing
**Goal**: Ensure reliability through extensive testing

- [ ] **Test Coverage Expansion**
  - Unit tests for all core modules (target: >80% coverage)
  - Integration tests for complete debugging workflows
  - End-to-end tests for UI interactions

- [ ] **Test Infrastructure**
  - Automated test suite for mainnet fork testing
  - Regression test suite for known issues
  - Performance benchmarking suite
  - Cross-chain compatibility testing

## Priority Levels

### High Priority
1. Complex type variable watcher
2. Memory optimization
3. Foundry test debugging support
4. Comprehensive test coverage

### Medium Priority
1. Improved snapshot system
2. Multi-chain and L2 support
3. Conditional breakpoints
4. Modern Web UI development
5. Smart contract intelligence

### Future Enhancements
1. State manipulation and what-if analysis
2. Agent MCP support
3. Debugger-friendly decompilation
4. Collaborative debugging features

## Contributing

Contributions are welcome! Please refer to the main README for development setup and contribution guidelines.

## Notes

- Features are subject to change based on user feedback and technical constraints
- Each milestone should be completed with comprehensive testing and documentation
- Performance benchmarks should be established before and after optimization work
