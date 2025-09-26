/**
 * @fileoverview VS Code Extension for EDB (Ethereum Debugger)
 * @description Provides debugging capabilities for Ethereum smart contracts
 */

import * as vscode from 'vscode';
import { EDBClient } from '@edb/client';
import { EDB } from '@edb/types';

let client: EDBClient;

export function activate(context: vscode.ExtensionContext) {
  console.log('EDB extension is now active');

  // Initialize EDB client
  client = new EDBClient({
    url: 'ws://localhost:8080/ws',
  });

  // Register commands
  const debugTransactionCommand = vscode.commands.registerCommand(
    'edb.debugTransaction',
    debugTransaction
  );

  const connectCommand = vscode.commands.registerCommand(
    'edb.connect',
    connectToEDB
  );

  const disconnectCommand = vscode.commands.registerCommand(
    'edb.disconnect',
    disconnectFromEDB
  );

  // Register debug configuration provider
  const configProvider = vscode.debug.registerDebugConfigurationProvider(
    'edb',
    new EDBConfigurationProvider()
  );

  // Register debug adapter descriptor factory
  const descriptorFactory = vscode.debug.registerDebugAdapterDescriptorFactory(
    'edb',
    new EDBAdapterDescriptorFactory()
  );

  context.subscriptions.push(
    debugTransactionCommand,
    connectCommand,
    disconnectCommand,
    configProvider,
    descriptorFactory
  );
}

export function deactivate() {
  if (client) {
    client.disconnect();
  }
}

async function connectToEDB() {
  const url = await vscode.window.showInputBox({
    prompt: 'Enter EDB server URL',
    placeHolder: 'ws://localhost:8080/ws',
    value: 'ws://localhost:8080/ws'
  });

  if (!url) return;

  try {
    client = new EDBClient({
      url: url,
    });

    await client.connect();
    vscode.window.showInformationMessage('Connected to EDB server');
  } catch (error) {
    vscode.window.showErrorMessage(`Failed to connect to EDB: ${error}`);
  }
}

async function disconnectFromEDB() {
  if (client) {
    client.disconnect();
    vscode.window.showInformationMessage('Disconnected from EDB server');
  }
}

async function debugTransaction() {
  if (!client) {
    const shouldConnect = await vscode.window.showInformationMessage(
      'Not connected to EDB server. Connect now?',
      'Connect', 'Cancel'
    );
    if (shouldConnect === 'Connect') {
      await connectToEDB();
    } else {
      return;
    }
  }

  const transactionHash = await vscode.window.showInputBox({
    prompt: 'Enter transaction hash to debug',
    placeHolder: '0x...',
    validateInput: (value) => {
      if (!value) return 'Transaction hash is required';
      if (!/^0x[a-fA-F0-9]{64}$/.test(value)) {
        return 'Invalid transaction hash format';
      }
      return null;
    }
  });

  if (!transactionHash) return;

  try {
    const session = await client.createSession(transactionHash as EDB.Hash);

    // Start debug session with VS Code
    await vscode.debug.startDebugging(undefined, {
      type: 'edb',
      name: `Debug Transaction: ${transactionHash.slice(0, 10)}...`,
      request: 'launch',
      transactionHash: transactionHash,
      sessionId: session.id
    });

  } catch (error) {
    vscode.window.showErrorMessage(`Failed to start debugging: ${error}`);
  }
}

class EDBConfigurationProvider implements vscode.DebugConfigurationProvider {
  resolveDebugConfiguration(
    folder: vscode.WorkspaceFolder | undefined,
    config: vscode.DebugConfiguration,
    token?: vscode.CancellationToken
  ): vscode.ProviderResult<vscode.DebugConfiguration> {
    // Return null to use default configuration
    return config;
  }
}

class EDBAdapterDescriptorFactory implements vscode.DebugAdapterDescriptorFactory {
  createDebugAdapterDescriptor(
    session: vscode.DebugSession,
    executable: vscode.DebugAdapterExecutable | undefined
  ): vscode.ProviderResult<vscode.DebugAdapterDescriptor> {
    // Return null to use default debug adapter
    return null;
  }
}