# EDB TypeScript Workspace

TypeScript frontend packages for the EDB (Ethereum Debugger) ecosystem.

## Quick Start

```bash
# Install dependencies
pnpm install

# Build all packages
pnpm run build

# Start Web UI in development mode
cd packages/edb-webui
pnpm run dev
```

## Packages

- **`@edb/types`** - Shared TypeScript types for all frontends
- **`@edb/client`** - Unified RPC client library for EDB engine communication
- **`@edb/webui`** - React-based web debugging interface
- **`edb-vscode`** - VS Code extension for native debugging

## Development

```bash
# Build all packages
pnpm run build

# Watch mode for development
pnpm run dev

# Type checking
pnpm run type-check

# Linting
pnpm run lint

# Run tests
pnpm run test

# Clean build artifacts
pnpm run clean
```

## Web UI Development

```bash
cd packages/edb-webui

# Start dev server (http://localhost:3000)
pnpm run dev

# Build for production
pnpm run build

# Preview production build
pnpm run preview
```

## VS Code Extension

```bash
cd packages/edb-vscode

# Build extension
pnpm run build

# Package extension
pnpm run package

# Install locally for testing
code --install-extension edb-vscode-0.0.1.vsix
```

## Prerequisites

- Node.js 18+
- pnpm 9+
- Running EDB engine (Rust backend) on `ws://localhost:8545`