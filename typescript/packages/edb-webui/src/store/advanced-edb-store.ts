/**
 * @fileoverview Advanced EDB store with TUI-inspired state management
 * @description Two-tier architecture with intelligent caching and background processing
 */

import { create } from 'zustand';
import { ExecutionManager, TraceData, SnapshotInfo, Code } from './execution-manager';
import { RequestProcessor, RequestDeduplicator } from './request-processor';
import { EDB } from '@edb/types';

interface AdvancedEDBStore {
  // Connection state
  isConnected: boolean;
  isConnecting: boolean;
  connectionError: string | null;
  serverUrl: string;

  // Managers (TUI-inspired architecture)
  executionManager: ExecutionManager | null;
  requestProcessor: RequestProcessor | null;
  deduplicator: RequestDeduplicator | null;

  // UI state
  currentSnapshotId: number | null;
  selectedAddress: string | null;

  // Actions - Connection
  connect: (url?: string) => Promise<void>;
  disconnect: () => void;
  setServerUrl: (url: string) => void;
  testConnection: () => Promise<boolean>;

  // Actions - Data Access (Non-blocking, TUI-style)
  getSnapshotCount: () => number;
  getTraceData: () => TraceData | null;
  getSnapshotInfo: (snapshotId: number) => SnapshotInfo | null;
  getCode: (address: string) => Code | null;
  getStorage: (snapshotId: number, slot: string) => string | null;
  getNextCall: (snapshotId: number) => number | null;
  getPrevCall: (snapshotId: number) => number | null;

  // Actions - Navigation
  setCurrentSnapshot: (snapshotId: number) => void;
  navigateToNextCall: () => void;
  navigateToPrevCall: () => void;
  setSelectedAddress: (address: string) => void;

  // Actions - System
  clearCache: () => void;
  getCacheStats: () => any;
  getLoadingStatus: () => any;
  isAnyLoading: () => boolean;

  // Internal actions
  _initializeManagers: () => void;
  _startBackgroundProcessing: () => void;
  _stopBackgroundProcessing: () => void;
}

export const useAdvancedEDBStore = create<AdvancedEDBStore>((set, get) => ({
  // Initial state
  isConnected: false,
  isConnecting: false,
  connectionError: null,
  serverUrl: '/api',
  executionManager: null,
  requestProcessor: null,
  deduplicator: null,
  currentSnapshotId: null,
  selectedAddress: null,

  // Connection actions
  connect: async (url?: string) => {
    const targetUrl = url || get().serverUrl;

    set({ isConnecting: true, connectionError: null });

    try {
      // Test connection with health endpoint
      const response = await fetch(`${targetUrl}/health`);
      if (!response.ok) {
        throw new Error(`Server responded with ${response.status}`);
      }

      console.log('EDB server health check passed');

      // Initialize managers
      get()._initializeManagers();

      set({
        isConnected: true,
        isConnecting: false,
        connectionError: null,
        serverUrl: targetUrl
      });

      // Start background processing
      get()._startBackgroundProcessing();

      // Initialize with basic data
      await get()._loadInitialData();

    } catch (error) {
      console.error('Failed to connect to EDB:', error);
      set({
        connectionError: error instanceof Error ? error.message : 'Connection failed',
        isConnecting: false,
        isConnected: false
      });
    }
  },

  disconnect: () => {
    get()._stopBackgroundProcessing();
    set({
      isConnected: false,
      isConnecting: false,
      connectionError: null,
      executionManager: null,
      requestProcessor: null,
      deduplicator: null,
      currentSnapshotId: null,
      selectedAddress: null
    });
  },

  setServerUrl: (url: string) => {
    set({ serverUrl: url });
  },

  testConnection: async () => {
    const { serverUrl } = get();
    try {
      const response = await fetch(`${serverUrl}/health`);
      return response.ok;
    } catch {
      return false;
    }
  },

  // Non-blocking data access (TUI-style)
  getSnapshotCount: () => {
    const { executionManager } = get();
    return executionManager?.getSnapshotCount() || 0;
  },

  getTraceData: () => {
    const { executionManager } = get();
    return executionManager?.getTraceData() || null;
  },

  getSnapshotInfo: (snapshotId: number) => {
    const { executionManager } = get();
    return executionManager?.getSnapshotInfo(snapshotId) || null;
  },

  getCode: (address: string) => {
    const { executionManager } = get();
    return executionManager?.getCode(address) || null;
  },

  getStorage: (snapshotId: number, slot: string) => {
    const { executionManager } = get();
    return executionManager?.getStorage(snapshotId, slot) || null;
  },

  getNextCall: (snapshotId: number) => {
    const { executionManager } = get();
    return executionManager?.getNextCall(snapshotId) || null;
  },

  getPrevCall: (snapshotId: number) => {
    const { executionManager } = get();
    return executionManager?.getPrevCall(snapshotId) || null;
  },

  // Navigation actions
  setCurrentSnapshot: (snapshotId: number) => {
    set({ currentSnapshotId: snapshotId });
  },

  navigateToNextCall: () => {
    const { currentSnapshotId, getNextCall } = get();
    if (currentSnapshotId !== null) {
      const nextCallId = getNextCall(currentSnapshotId);
      if (nextCallId !== null) {
        set({ currentSnapshotId: nextCallId });
      }
    }
  },

  navigateToPrevCall: () => {
    const { currentSnapshotId, getPrevCall } = get();
    if (currentSnapshotId !== null) {
      const prevCallId = getPrevCall(currentSnapshotId);
      if (prevCallId !== null) {
        set({ currentSnapshotId: prevCallId });
      }
    }
  },

  setSelectedAddress: (address: string) => {
    set({ selectedAddress: address });
  },

  // System actions
  clearCache: () => {
    const { executionManager } = get();
    executionManager?.clearCache();
  },

  getCacheStats: () => {
    const { executionManager } = get();
    return executionManager?.getCacheStats() || {};
  },

  getLoadingStatus: () => {
    const { executionManager } = get();
    return executionManager?.getLoadingStatus() || {};
  },

  isAnyLoading: () => {
    const { executionManager } = get();
    return executionManager?.isLoading() || false;
  },

  // Internal actions
  _initializeManagers: () => {
    const { serverUrl } = get();
    set({
      executionManager: new ExecutionManager(),
      requestProcessor: new RequestProcessor(serverUrl),
      deduplicator: new RequestDeduplicator()
    });
  },

  _startBackgroundProcessing: () => {
    const { requestProcessor, executionManager } = get();
    if (requestProcessor && executionManager) {
      requestProcessor.startProcessing(executionManager, 200); // 200ms interval like Rust TUI
    }
  },

  _stopBackgroundProcessing: () => {
    const { requestProcessor } = get();
    requestProcessor?.stopProcessing();
  },

  // Load initial data (trace and snapshot count)
  _loadInitialData: async () => {
    const { executionManager, serverUrl } = get();
    if (!executionManager) return;

    try {
      // Make initial RPC calls for essential data
      const traceResponse = await fetch(serverUrl, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          jsonrpc: '2.0',
          method: 'edb_getTrace',
          params: [],
          id: 1
        })
      });

      const countResponse = await fetch(serverUrl, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          jsonrpc: '2.0',
          method: 'edb_getSnapshotCount',
          params: [],
          id: 2
        })
      });

      const [traceResult, countResult] = await Promise.all([
        traceResponse.json(),
        countResponse.json()
      ]);

      // Update execution manager with initial data
      if (traceResult.result) {
        executionManager.updateCachedData({ type: 'GetTrace' }, traceResult.result);
      }

      if (countResult.result) {
        executionManager.updateCachedData({ type: 'GetSnapshotCount' }, countResult.result);

        // Set initial snapshot to first snapshot
        if (countResult.result > 0) {
          set({ currentSnapshotId: 0 });
        }
      }

    } catch (error) {
      console.error('Failed to load initial data:', error);
    }
  }
}));

// Add a re-export for easy migration
export { useAdvancedEDBStore as useEDBStore };