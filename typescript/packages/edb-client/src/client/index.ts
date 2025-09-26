/**
 * @fileoverview Main EDB client implementation
 * @description WebSocket and HTTP client for EDB engine communication
 */

import { EventEmitter } from 'eventemitter3';
import WebSocket from 'ws';
import { EDB, RPC, Engine, Debug } from '@edb/types';
import { EDBClientConfig, DEFAULT_CLIENT_CONFIG } from '../index';
import { EDBCache } from '../cache';
import { generateRequestId, delay } from '../utils';

export interface EDBClientEvents {
  connected: () => void;
  disconnected: (reason: string) => void;
  error: (error: Error) => void;
  snapshotUpdate: (event: RPC.SnapshotUpdateEvent) => void;
  breakpointHit: (event: RPC.BreakpointHitEvent) => void;
  sessionEnded: (event: RPC.SessionEndedEvent) => void;
}

export class EDBClient extends EventEmitter<EDBClientEvents> {
  private ws: WebSocket | null = null;
  private config: Required<EDBClientConfig>;
  private cache: EDBCache;
  private pendingRequests = new Map<string | number, {
    resolve: (value: any) => void;
    reject: (error: Error) => void;
    timeout: NodeJS.Timeout;
  }>();
  private reconnectAttempts = 0;
  private reconnectTimer: NodeJS.Timeout | null = null;
  private isReconnecting = false;

  constructor(config: EDBClientConfig) {
    super();
    this.config = { ...DEFAULT_CLIENT_CONFIG, ...config };
    this.cache = new EDBCache(this.config.cacheConfig);
  }

  // Connection management
  async connect(): Promise<void> {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      return;
    }

    return new Promise((resolve, reject) => {
      try {
        this.ws = new WebSocket(this.config.url);

        const onOpen = () => {
          this.reconnectAttempts = 0;
          this.isReconnecting = false;
          this.emit('connected');
          resolve();
        };

        const onError = (error: Error) => {
          this.emit('error', error);
          if (!this.isReconnecting) {
            reject(error);
          }
        };

        const onClose = (code: number, reason: Buffer) => {
          this.emit('disconnected', reason.toString());
          this.handleDisconnection();
        };

        const onMessage = (data: Buffer) => {
          try {
            const message = JSON.parse(data.toString());
            this.handleMessage(message);
          } catch (error) {
            this.emit('error', new Error(`Failed to parse message: ${error}`));
          }
        };

        this.ws.once('open', onOpen);
        this.ws.once('error', onError);
        this.ws.on('close', onClose);
        this.ws.on('message', onMessage);

      } catch (error) {
        reject(error);
      }
    });
  }

  disconnect(): void {
    this.isReconnecting = false;
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }

    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }

    // Reject all pending requests
    for (const [, request] of this.pendingRequests) {
      clearTimeout(request.timeout);
      request.reject(new Error('Client disconnected'));
    }
    this.pendingRequests.clear();
  }

  get isConnected(): boolean {
    return this.ws !== null && this.ws.readyState === WebSocket.OPEN;
  }

  // Session management
  async createSession(transactionHash: EDB.Hash, blockNumber?: EDB.BlockNumber): Promise<Debug.Session> {
    const response = await this.sendRequest<RPC.CreateSessionResponse>({
      method: 'edb_createSession',
      params: { transactionHash, blockNumber }
    });
    return response.result!;
  }

  async getSessionInfo(sessionId: string): Promise<Debug.Session> {
    const cacheKey = `session:${sessionId}`;
    const cached = this.cache.get<Debug.Session>(cacheKey);
    if (cached) return cached;

    const response = await this.sendRequest<RPC.GetSessionInfoResponse>({
      method: 'edb_getSessionInfo',
      params: { sessionId }
    });

    const session = response.result!;
    this.cache.set(cacheKey, session);
    return session;
  }

  // Snapshot operations
  async getSnapshot(sessionId: string, snapshotId: string): Promise<Engine.Snapshot> {
    const cacheKey = `snapshot:${sessionId}:${snapshotId}`;
    const cached = this.cache.get<Engine.Snapshot>(cacheKey);
    if (cached) return cached;

    const response = await this.sendRequest<RPC.GetSnapshotResponse>({
      method: 'edb_getSnapshot',
      params: { sessionId, snapshotId }
    });

    const snapshot = response.result!;
    this.cache.set(cacheKey, snapshot);
    return snapshot;
  }

  async listSnapshots(
    sessionId: string,
    offset = 0,
    limit = 100
  ): Promise<{ snapshots: Engine.Snapshot[]; total: number }> {
    const response = await this.sendRequest<RPC.ListSnapshotsResponse>({
      method: 'edb_listSnapshots',
      params: { sessionId, offset, limit }
    });
    return response.result!;
  }

  // Navigation
  async navigate(
    sessionId: string,
    direction: 'next' | 'previous' | 'stepIn' | 'stepOut',
    count = 1
  ): Promise<{ currentSnapshot: Engine.Snapshot; canStepNext: boolean; canStepPrevious: boolean }> {
    const response = await this.sendRequest<RPC.NavigateResponse>({
      method: 'edb_navigate',
      params: { sessionId, direction, count }
    });
    return response.result!;
  }

  // Trace operations
  async getTrace(sessionId: string): Promise<Engine.ExecutionTrace> {
    const cacheKey = `trace:${sessionId}`;
    const cached = this.cache.get<Engine.ExecutionTrace>(cacheKey);
    if (cached) return cached;

    const response = await this.sendRequest<RPC.GetTraceResponse>({
      method: 'edb_getTrace',
      params: { sessionId }
    });

    const trace = response.result!;
    this.cache.set(cacheKey, trace, 600000); // Cache for 10 minutes
    return trace;
  }

  // Expression evaluation
  async evaluateExpression(
    sessionId: string,
    snapshotId: string,
    expression: string
  ): Promise<{ result: string; type: string; error?: string }> {
    const response = await this.sendRequest<RPC.EvaluateExpressionResponse>({
      method: 'edb_evaluateExpression',
      params: { sessionId, snapshotId, expression }
    });
    return response.result!;
  }

  // Breakpoint management
  async setBreakpoint(sessionId: string, breakpoint: Debug.Breakpoint): Promise<Debug.Breakpoint> {
    const response = await this.sendRequest<RPC.SetBreakpointResponse>({
      method: 'edb_setBreakpoint',
      params: { sessionId, breakpoint }
    });

    // Invalidate session cache since breakpoints changed
    this.cache.delete(`session:${sessionId}`);

    return response.result!;
  }

  async removeBreakpoint(sessionId: string, breakpointId: string): Promise<boolean> {
    const response = await this.sendRequest<RPC.RemoveBreakpointResponse>({
      method: 'edb_removeBreakpoint',
      params: { sessionId, breakpointId }
    });

    // Invalidate session cache since breakpoints changed
    this.cache.delete(`session:${sessionId}`);

    return response.result!;
  }

  // Watch expressions
  async addWatch(sessionId: string, expression: string, name?: string): Promise<Debug.WatchExpression> {
    const response = await this.sendRequest<RPC.AddWatchResponse>({
      method: 'edb_addWatch',
      params: { sessionId, expression, name }
    });
    return response.result!;
  }

  // Private methods
  private async sendRequest<T extends RPC.EDBResponse>(
    request: Omit<RPC.EDBRequest, 'jsonrpc' | 'id'>
  ): Promise<T> {
    if (!this.isConnected) {
      throw new Error('Client is not connected');
    }

    const id = generateRequestId();
    const fullRequest: RPC.JSONRPCRequest = {
      jsonrpc: '2.0',
      id,
      ...request
    };

    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        this.pendingRequests.delete(id);
        reject(new Error('Request timeout'));
      }, this.config.requestTimeout);

      this.pendingRequests.set(id, { resolve, reject, timeout });

      this.ws!.send(JSON.stringify(fullRequest), (error) => {
        if (error) {
          this.pendingRequests.delete(id);
          clearTimeout(timeout);
          reject(error);
        }
      });
    });
  }

  private handleMessage(message: any): void {
    // Handle RPC responses
    if (message.id !== undefined && this.pendingRequests.has(message.id)) {
      const request = this.pendingRequests.get(message.id)!;
      this.pendingRequests.delete(message.id);
      clearTimeout(request.timeout);

      if (message.error) {
        request.reject(new Error(`RPC Error ${message.error.code}: ${message.error.message}`));
      } else {
        request.resolve(message);
      }
      return;
    }

    // Handle WebSocket events
    if (message.type) {
      switch (message.type) {
        case 'snapshotUpdate':
          this.emit('snapshotUpdate', message);
          break;
        case 'breakpointHit':
          this.emit('breakpointHit', message);
          break;
        case 'sessionEnded':
          this.emit('sessionEnded', message);
          break;
        default:
          console.warn('Unknown event type:', message.type);
      }
    }
  }

  private handleDisconnection(): void {
    if (this.isReconnecting || this.reconnectAttempts >= this.config.maxReconnectAttempts) {
      return;
    }

    this.isReconnecting = true;
    this.reconnectAttempts++;

    this.reconnectTimer = setTimeout(async () => {
      try {
        await this.connect();
      } catch (error) {
        console.error('Reconnection failed:', error);
        this.handleDisconnection();
      }
    }, this.config.reconnectInterval);
  }
}