/**
 * @fileoverview Background request processor for non-blocking RPC calls
 * @description Inspired by Rust TUI's background task processing
 */

import { ExecutionRequest, ExecutionManager } from './execution-manager';

export interface RpcCall {
  method: string;
  params: any[];
}

/**
 * Maps execution requests to RPC calls
 */
export class RequestProcessor {
  private serverUrl: string;
  private isProcessing = false;
  private processingInterval: number | null = null;

  constructor(serverUrl: string) {
    this.serverUrl = serverUrl;
  }

  /**
   * Start background processing (similar to Rust TUI's spawned tasks)
   */
  startProcessing(executionManager: ExecutionManager, intervalMs: number = 200): void {
    if (this.processingInterval) {
      this.stopProcessing();
    }

    this.processingInterval = window.setInterval(async () => {
      if (this.isProcessing) return;

      this.isProcessing = true;
      try {
        await this.processPendingRequests(executionManager);
      } catch (error) {
        console.error('Error processing pending requests:', error);
      } finally {
        this.isProcessing = false;
      }
    }, intervalMs);
  }

  /**
   * Stop background processing
   */
  stopProcessing(): void {
    if (this.processingInterval) {
      window.clearInterval(this.processingInterval);
      this.processingInterval = null;
    }
    this.isProcessing = false;
  }

  /**
   * Process all pending requests in batch
   */
  private async processPendingRequests(executionManager: ExecutionManager): Promise<void> {
    const pendingRequests = executionManager.getPendingRequests();
    if (pendingRequests.length === 0) return;

    console.log(`Processing ${pendingRequests.length} pending requests:`, pendingRequests.map(r => r.type));

    // Group requests by type for potential batching
    const requestGroups = this.groupRequests(pendingRequests);

    // Process each group
    for (const [requestType, requests] of requestGroups.entries()) {
      try {
        await this.processRequestGroup(requestType, requests, executionManager);
      } catch (error) {
        console.error(`Error processing ${requestType} requests:`, error);
        // Mark all requests in this group as errored
        for (const request of requests) {
          executionManager.updateCachedData(request, null, error instanceof Error ? error.message : 'Unknown error');
        }
      }
    }
  }

  /**
   * Group requests by type for batch processing
   */
  private groupRequests(requests: ExecutionRequest[]): Map<string, ExecutionRequest[]> {
    const groups = new Map<string, ExecutionRequest[]>();

    for (const request of requests) {
      const key = request.type;
      if (!groups.has(key)) {
        groups.set(key, []);
      }
      groups.get(key)!.push(request);
    }

    return groups;
  }

  /**
   * Process a group of requests of the same type
   */
  private async processRequestGroup(
    requestType: string,
    requests: ExecutionRequest[],
    executionManager: ExecutionManager
  ): Promise<void> {
    // For now, process requests individually
    // TODO: Implement true batching for methods that support it
    const promises = requests.map(request => this.processIndividualRequest(request, executionManager));
    await Promise.allSettled(promises);
  }

  /**
   * Process a single request
   */
  private async processIndividualRequest(
    request: ExecutionRequest,
    executionManager: ExecutionManager
  ): Promise<void> {
    try {
      const rpcCall = this.mapRequestToRpc(request);
      const result = await this.makeRpcCall(rpcCall);
      executionManager.updateCachedData(request, result);
    } catch (error) {
      console.error(`Error processing request ${request.type}:`, error);
      executionManager.updateCachedData(request, null, error instanceof Error ? error.message : 'Unknown error');
    }
  }

  /**
   * Map execution request to RPC call
   */
  private mapRequestToRpc(request: ExecutionRequest): RpcCall {
    switch (request.type) {
      case 'GetTrace':
        return { method: 'edb_getTrace', params: [] };

      case 'GetSnapshotCount':
        return { method: 'edb_getSnapshotCount', params: [] };

      case 'GetSnapshotInfo':
        return { method: 'edb_getSnapshotInfo', params: [request.snapshotId] };

      case 'GetCode':
        return { method: 'edb_getCode', params: [request.snapshotId] };

      case 'GetNextCall':
        return { method: 'edb_getNextCall', params: [request.snapshotId] };

      case 'GetPrevCall':
        return { method: 'edb_getPrevCall', params: [request.snapshotId] };

      case 'GetStorage':
        return { method: 'edb_getStorage', params: [request.snapshotId, request.slot] };

      case 'GetStorageDiff':
        return { method: 'edb_getStorageDiff', params: [request.snapshotId] };

      default:
        throw new Error(`Unknown request type: ${(request as any).type}`);
    }
  }

  /**
   * Make RPC call to server
   */
  private async makeRpcCall(rpcCall: RpcCall): Promise<any> {
    const response = await fetch(this.serverUrl, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({
        jsonrpc: '2.0',
        method: rpcCall.method,
        params: rpcCall.params,
        id: Date.now() // Simple ID generation
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
  }

  /**
   * Get processing status
   */
  getStatus() {
    return {
      isProcessing: this.isProcessing,
      hasInterval: this.processingInterval !== null
    };
  }
}

/**
 * Request deduplicator to avoid duplicate requests
 */
export class RequestDeduplicator {
  private activeRequests = new Set<string>();

  /**
   * Check if request is already active
   */
  isActive(request: ExecutionRequest): boolean {
    const key = this.getRequestKey(request);
    return this.activeRequests.has(key);
  }

  /**
   * Mark request as active
   */
  markActive(request: ExecutionRequest): void {
    const key = this.getRequestKey(request);
    this.activeRequests.add(key);
  }

  /**
   * Mark request as completed
   */
  markCompleted(request: ExecutionRequest): void {
    const key = this.getRequestKey(request);
    this.activeRequests.delete(key);
  }

  /**
   * Generate unique key for request
   */
  private getRequestKey(request: ExecutionRequest): string {
    switch (request.type) {
      case 'GetTrace':
      case 'GetSnapshotCount':
        return request.type;

      case 'GetSnapshotInfo':
      case 'GetNextCall':
      case 'GetPrevCall':
      case 'GetStorageDiff':
        return `${request.type}:${request.snapshotId}`;

      case 'GetCode':
        return `${request.type}:${request.snapshotId}`;

      case 'GetStorage':
        return `${request.type}:${request.snapshotId}:${request.slot}`;

      default:
        return JSON.stringify(request);
    }
  }

  /**
   * Clear all active requests
   */
  clear(): void {
    this.activeRequests.clear();
  }

  /**
   * Get active request count
   */
  getActiveCount(): number {
    return this.activeRequests.size;
  }
}