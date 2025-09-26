/**
 * @fileoverview Execution state management inspired by Rust TUI
 * @description Two-tier architecture with caching and background processing
 */

import { FetchCache, CacheStatus } from './cache';
import { EDB } from '@edb/types';

export interface SnapshotInfo {
  id: number;
  frame_id: any;
  next_id: number;
  prev_id: number;
  target_address: string;
  bytecode_address: string;
  detail: SnapshotInfoDetail;
}

export interface SnapshotInfoDetail {
  Opcode?: OpcodeSnapshotInfoDetail;
  Hook?: HookSnapshotInfoDetail;
}

export interface OpcodeSnapshotInfoDetail {
  id: number;
  frame_id: any;
  pc: number;
  opcode: number;
  memory: number[];
  stack: string[];
  calldata: number[];
  transient_storage: Record<string, string>;
}

export interface HookSnapshotInfoDetail {
  id: number;
  frame_id: any;
  locals: Record<string, any>;
  state_variables: Record<string, any>;
  path: string;
  offset: number;
  length: number;
}

export interface Code {
  address: string;
  bytecode: string;
  sourceMap?: string;
}

export interface StorageSlot {
  snapshotId: number;
  slot: string;
}

export interface TraceData {
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

/**
 * Request types for different data fetching operations
 */
export type ExecutionRequest =
  | { type: 'GetSnapshotInfo'; snapshotId: number }
  | { type: 'GetCode'; address: string }
  | { type: 'GetNextCall'; snapshotId: number }
  | { type: 'GetPrevCall'; snapshotId: number }
  | { type: 'GetStorage'; snapshotId: number; slot: string }
  | { type: 'GetStorageDiff'; snapshotId: number }
  | { type: 'GetTrace' }
  | { type: 'GetSnapshotCount' };

/**
 * Execution state with intelligent caching (mirrors Rust TUI's ExecutionState)
 */
export class ExecutionState {
  // Basic state
  snapshotCount: number = 0;

  // Cached data with intelligent fetching
  snapshotInfo = new FetchCache<number, SnapshotInfo>();
  code = new FetchCache<string, Code>(); // Cache by address
  nextCall = new FetchCache<number, number>(); // snapshotId -> next call snapshotId
  prevCall = new FetchCache<number, number>(); // snapshotId -> prev call snapshotId
  storage = new FetchCache<string, string>(); // "snapshotId:slot" -> value
  storageDiff = new FetchCache<number, Record<string, [string, string]>>(); // snapshotId -> diff map
  traceData: TraceData | null = null;

  /**
   * Update from another state (for synchronization)
   */
  update(other: ExecutionState): void {
    this.snapshotCount = other.snapshotCount;

    if (this.snapshotInfo.needsUpdate(other.snapshotInfo)) {
      this.snapshotInfo.update(other.snapshotInfo);
    }

    if (this.code.needsUpdate(other.code)) {
      this.code.update(other.code);
    }

    if (this.nextCall.needsUpdate(other.nextCall)) {
      this.nextCall.update(other.nextCall);
    }

    if (this.prevCall.needsUpdate(other.prevCall)) {
      this.prevCall.update(other.prevCall);
    }

    if (this.storage.needsUpdate(other.storage)) {
      this.storage.update(other.storage);
    }

    if (this.storageDiff.needsUpdate(other.storageDiff)) {
      this.storageDiff.update(other.storageDiff);
    }

    if (other.traceData && !this.traceData) {
      this.traceData = other.traceData;
    }
  }

  /**
   * Get cache statistics for debugging
   */
  getCacheStats() {
    return {
      snapshotInfo: this.snapshotInfo.getStats(),
      code: this.code.getStats(),
      nextCall: this.nextCall.getStats(),
      prevCall: this.prevCall.getStats(),
      storage: this.storage.getStats(),
      storageDiff: this.storageDiff.getStats(),
    };
  }
}

/**
 * Manager for execution state with non-blocking data access
 * Inspired by Rust TUI's ExecutionManager
 */
export class ExecutionManager {
  private state = new ExecutionState();
  private pendingRequests = new Set<string>();

  /**
   * Get snapshot count (immediate)
   */
  getSnapshotCount(): number {
    return this.state.snapshotCount;
  }

  /**
   * Get trace data (immediate)
   */
  getTraceData(): TraceData | null {
    return this.state.traceData;
  }

  /**
   * Get snapshot info (immediate if cached, triggers fetch if not)
   */
  getSnapshotInfo(snapshotId: number): SnapshotInfo | null {
    const cached = this.state.snapshotInfo.get(snapshotId);
    if (cached) return cached;

    // Check if already pending
    const status = this.state.snapshotInfo.getStatus(snapshotId);
    if (status === CacheStatus.NotRequested) {
      this.requestSnapshotInfo(snapshotId);
    }

    return null; // Will show loading state
  }

  /**
   * Get code for address (immediate if cached, triggers fetch if not)
   */
  getCode(address: string): Code | null {
    const cached = this.state.code.get(address);
    if (cached) return cached;

    // Check if already pending
    const status = this.state.code.getStatus(address);
    if (status === CacheStatus.NotRequested) {
      this.requestCode(address);
    }

    return null;
  }

  /**
   * Get storage value (immediate if cached, triggers fetch if not)
   */
  getStorage(snapshotId: number, slot: string): string | null {
    const key = `${snapshotId}:${slot}`;
    const cached = this.state.storage.get(key);
    if (cached) return cached;

    // Check if already pending
    const status = this.state.storage.getStatus(key);
    if (status === CacheStatus.NotRequested) {
      this.requestStorage(snapshotId, slot);
    }

    return null;
  }

  /**
   * Get next call snapshot ID (immediate if cached, triggers fetch if not)
   */
  getNextCall(snapshotId: number): number | null {
    const cached = this.state.nextCall.get(snapshotId);
    if (cached !== null) return cached;

    const status = this.state.nextCall.getStatus(snapshotId);
    if (status === CacheStatus.NotRequested) {
      this.requestNextCall(snapshotId);
    }

    return null;
  }

  /**
   * Get previous call snapshot ID (immediate if cached, triggers fetch if not)
   */
  getPrevCall(snapshotId: number): number | null {
    const cached = this.state.prevCall.get(snapshotId);
    if (cached !== null) return cached;

    const status = this.state.prevCall.getStatus(snapshotId);
    if (status === CacheStatus.NotRequested) {
      this.requestPrevCall(snapshotId);
    }

    return null;
  }

  /**
   * Check if any data is currently loading
   */
  isLoading(): boolean {
    return this.pendingRequests.size > 0;
  }

  /**
   * Get loading status for specific data types
   */
  getLoadingStatus() {
    return {
      snapshotInfo: this.state.snapshotInfo.getStats().pending > 0,
      code: this.state.code.getStats().pending > 0,
      storage: this.state.storage.getStats().pending > 0,
      navigation: this.state.nextCall.getStats().pending > 0 || this.state.prevCall.getStats().pending > 0,
      hasAnyLoading: this.isLoading()
    };
  }

  /**
   * Get all pending requests for batch processing
   */
  getPendingRequests(): ExecutionRequest[] {
    const requests: ExecutionRequest[] = [];

    // Collect all pending requests from caches
    for (const snapshotId of this.state.snapshotInfo.getPendingKeys()) {
      requests.push({ type: 'GetSnapshotInfo', snapshotId });
    }

    for (const address of this.state.code.getPendingKeys()) {
      requests.push({ type: 'GetCode', address });
    }

    for (const snapshotId of this.state.nextCall.getPendingKeys()) {
      requests.push({ type: 'GetNextCall', snapshotId });
    }

    for (const snapshotId of this.state.prevCall.getPendingKeys()) {
      requests.push({ type: 'GetPrevCall', snapshotId });
    }

    for (const key of this.state.storage.getPendingKeys()) {
      const [snapshotId, slot] = key.split(':');
      requests.push({ type: 'GetStorage', snapshotId: parseInt(snapshotId), slot });
    }

    for (const snapshotId of this.state.storageDiff.getPendingKeys()) {
      requests.push({ type: 'GetStorageDiff', snapshotId });
    }

    return requests;
  }

  /**
   * Update cached data (called from background processor)
   */
  updateCachedData(request: ExecutionRequest, data: any, error?: string): void {
    if (error) {
      this.setCacheError(request, error);
      return;
    }

    switch (request.type) {
      case 'GetSnapshotInfo':
        this.state.snapshotInfo.set(request.snapshotId, data);
        break;
      case 'GetCode':
        this.state.code.set(request.address, data);
        break;
      case 'GetNextCall':
        this.state.nextCall.set(request.snapshotId, data);
        break;
      case 'GetPrevCall':
        this.state.prevCall.set(request.snapshotId, data);
        break;
      case 'GetStorage':
        this.state.storage.set(`${request.snapshotId}:${request.slot}`, data);
        break;
      case 'GetStorageDiff':
        this.state.storageDiff.set(request.snapshotId, data);
        break;
      case 'GetTrace':
        this.state.traceData = data;
        break;
      case 'GetSnapshotCount':
        this.state.snapshotCount = data;
        break;
    }
  }

  /**
   * Set cache error for a request
   */
  private setCacheError(request: ExecutionRequest, error: string): void {
    switch (request.type) {
      case 'GetSnapshotInfo':
        this.state.snapshotInfo.setError(request.snapshotId, error);
        break;
      case 'GetCode':
        this.state.code.setError(request.address, error);
        break;
      case 'GetStorage':
        this.state.storage.setError(`${request.snapshotId}:${request.slot}`, error);
        break;
      // Add other error cases as needed
    }
  }

  // Private request methods (mark as pending)
  private requestSnapshotInfo(snapshotId: number): void {
    this.state.snapshotInfo.setPending(snapshotId);
  }

  private requestCode(address: string): void {
    this.state.code.setPending(address);
  }

  private requestStorage(snapshotId: number, slot: string): void {
    const key = `${snapshotId}:${slot}`;
    this.state.storage.setPending(key);
  }

  private requestNextCall(snapshotId: number): void {
    this.state.nextCall.setPending(snapshotId);
  }

  private requestPrevCall(snapshotId: number): void {
    this.state.prevCall.setPending(snapshotId);
  }

  /**
   * Clear all cached data
   */
  clearCache(): void {
    this.state.snapshotInfo.clear();
    this.state.code.clear();
    this.state.nextCall.clear();
    this.state.prevCall.clear();
    this.state.storage.clear();
    this.state.storageDiff.clear();
    this.state.traceData = null;
    this.pendingRequests.clear();
  }

  /**
   * Get cache statistics for debugging
   */
  getCacheStats() {
    return this.state.getCacheStats();
  }
}