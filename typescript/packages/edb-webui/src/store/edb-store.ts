import { create } from 'zustand';
import { EDB } from '@edb/types';

interface TraceData {
  inner: Array<{
    id: number;
    bytecode: string;
    caller: string;
    target: string;
    input: string;
    depth: number;
    call_type: any;
    result: any;
    events: Array<any>;
    first_snapshot_id: number;
    parent_id: number | null;
  }>;
}

interface EDBStore {
  // Connection state
  isConnected: boolean;
  isConnecting: boolean;
  connectionError: string | null;
  serverUrl: string;

  // Current trace data
  traceData: TraceData | null;
  snapshotCount: number | null;
  isLoadingTrace: boolean;

  // Actions
  connect: (url?: string) => Promise<void>;
  disconnect: () => void;
  loadTrace: () => Promise<void>;
  loadSnapshotCount: () => Promise<void>;
  setServerUrl: (url: string) => void;
  testConnection: () => Promise<boolean>;
  callRpcMethod: (method: string, params?: any[]) => Promise<any>;
}

export const useEDBStore = create<EDBStore>((set, get) => ({
  // Initial state
  isConnected: false,
  isConnecting: false,
  connectionError: null,
  serverUrl: '/api',
  traceData: null,
  snapshotCount: null,
  isLoadingTrace: false,

  // Actions
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
      set({
        isConnected: true,
        isConnecting: false,
        connectionError: null,
        serverUrl: targetUrl
      });

      // Auto-load trace data
      const { loadTrace, loadSnapshotCount } = get();
      await Promise.all([loadTrace(), loadSnapshotCount()]);

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
    set({
      isConnected: false,
      isConnecting: false,
      connectionError: null,
      traceData: null,
      snapshotCount: null
    });
  },

  callRpcMethod: async (method: string, params: any[] = []) => {
    const { serverUrl, isConnected } = get();

    if (!isConnected) {
      throw new Error('Not connected to EDB server');
    }

    try {
      const response = await fetch(serverUrl, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          jsonrpc: '2.0',
          method,
          params,
          id: 1
        })
      });

      if (!response.ok) {
        throw new Error(`HTTP ${response.status}: ${response.statusText}`);
      }

      const result = await response.json();

      if (result.error) {
        throw new Error(result.error.message || 'RPC error');
      }

      return result.result;

    } catch (error) {
      console.error(`Failed to call ${method}:`, error);
      throw error;
    }
  },

  loadTrace: async () => {
    set({ isLoadingTrace: true });
    try {
      const traceData = await get().callRpcMethod('edb_getTrace');
      set({ traceData, isLoadingTrace: false });
    } catch (error) {
      console.error('Failed to load trace:', error);
      set({ isLoadingTrace: false });
    }
  },

  loadSnapshotCount: async () => {
    try {
      const snapshotCount = await get().callRpcMethod('edb_getSnapshotCount');
      set({ snapshotCount });
    } catch (error) {
      console.error('Failed to load snapshot count:', error);
    }
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
  }
}));