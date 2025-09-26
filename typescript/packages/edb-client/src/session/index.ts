/**
 * @fileoverview EDB debugging session management
 * @description High-level session management with state tracking
 */

import { EventEmitter } from 'eventemitter3';
import { EDB, Debug, Engine } from '@edb/types';
import { EDBClient } from '../client';

export interface EDBSessionEvents {
  stateChanged: (state: Debug.DebuggingState) => void;
  snapshotChanged: (snapshot: Engine.Snapshot) => void;
  breakpointHit: (breakpoint: Debug.Breakpoint, snapshot: Engine.Snapshot) => void;
  error: (error: Error) => void;
  ended: (reason: string) => void;
}

export class EDBSession extends EventEmitter<EDBSessionEvents> {
  private client: EDBClient;
  private sessionInfo: Debug.Session;
  private state: Debug.DebuggingState;
  private currentSnapshot: Engine.Snapshot | null = null;

  constructor(client: EDBClient, sessionInfo: Debug.Session) {
    super();
    this.client = client;
    this.sessionInfo = sessionInfo;
    this.state = {
      isActive: true,
      isPaused: false,
      executionDirection: 'forward',
      autoStep: false,
      stepDelay: 1000,
    };

    this.setupEventHandlers();
  }

  // Getters
  get id(): string {
    return this.sessionInfo.id;
  }

  get transactionHash(): EDB.Hash {
    return this.sessionInfo.transactionHash;
  }

  get status(): Debug.SessionStatus {
    return this.sessionInfo.status;
  }

  get debuggingState(): Debug.DebuggingState {
    return { ...this.state };
  }

  get currentSnapshotData(): Engine.Snapshot | null {
    return this.currentSnapshot;
  }

  get breakpoints(): Debug.Breakpoint[] {
    return this.sessionInfo.breakpoints;
  }

  get watchExpressions(): Debug.WatchExpression[] {
    return this.sessionInfo.watchExpressions;
  }

  // Navigation methods
  async stepNext(count = 1): Promise<Engine.Snapshot> {
    const result = await this.client.navigate(this.id, 'next', count);
    await this.updateCurrentSnapshot(result.currentSnapshot);
    return result.currentSnapshot;
  }

  async stepPrevious(count = 1): Promise<Engine.Snapshot> {
    const result = await this.client.navigate(this.id, 'previous', count);
    await this.updateCurrentSnapshot(result.currentSnapshot);
    return result.currentSnapshot;
  }

  async stepIn(): Promise<Engine.Snapshot> {
    const result = await this.client.navigate(this.id, 'stepIn');
    await this.updateCurrentSnapshot(result.currentSnapshot);
    return result.currentSnapshot;
  }

  async stepOut(): Promise<Engine.Snapshot> {
    const result = await this.client.navigate(this.id, 'stepOut');
    await this.updateCurrentSnapshot(result.currentSnapshot);
    return result.currentSnapshot;
  }

  // Snapshot operations
  async getSnapshot(snapshotId: string): Promise<Engine.Snapshot> {
    return await this.client.getSnapshot(this.id, snapshotId);
  }

  async listSnapshots(offset = 0, limit = 100): Promise<{ snapshots: Engine.Snapshot[]; total: number }> {
    return await this.client.listSnapshots(this.id, offset, limit);
  }

  async jumpToSnapshot(snapshotId: string): Promise<Engine.Snapshot> {
    const snapshot = await this.getSnapshot(snapshotId);
    await this.updateCurrentSnapshot(snapshot);
    return snapshot;
  }

  // Execution trace
  async getTrace(): Promise<Engine.ExecutionTrace> {
    return await this.client.getTrace(this.id);
  }

  // Expression evaluation
  async evaluateExpression(expression: string, snapshotId?: string): Promise<{ result: string; type: string; error?: string }> {
    const targetSnapshotId = snapshotId || this.currentSnapshot?.id;
    if (!targetSnapshotId) {
      throw new Error('No snapshot available for evaluation');
    }

    return await this.client.evaluateExpression(this.id, targetSnapshotId, expression);
  }

  // Breakpoint management
  async addBreakpoint(breakpoint: Omit<Debug.Breakpoint, 'id' | 'hitCount' | 'createdAt' | 'modifiedAt'>): Promise<Debug.Breakpoint> {
    const fullBreakpoint: Debug.Breakpoint = {
      ...breakpoint,
      id: `bp_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`,
      hitCount: 0,
      createdAt: Date.now(),
      modifiedAt: Date.now(),
    };

    const result = await this.client.setBreakpoint(this.id, fullBreakpoint);

    // Update local session info
    this.sessionInfo.breakpoints.push(result);

    return result;
  }

  async removeBreakpoint(breakpointId: string): Promise<boolean> {
    const result = await this.client.removeBreakpoint(this.id, breakpointId);

    if (result) {
      // Update local session info
      this.sessionInfo.breakpoints = this.sessionInfo.breakpoints.filter(bp => bp.id !== breakpointId);
    }

    return result;
  }

  async toggleBreakpoint(breakpointId: string): Promise<Debug.Breakpoint> {
    const breakpoint = this.sessionInfo.breakpoints.find(bp => bp.id === breakpointId);
    if (!breakpoint) {
      throw new Error(`Breakpoint ${breakpointId} not found`);
    }

    const updated = {
      ...breakpoint,
      enabled: !breakpoint.enabled,
      modifiedAt: Date.now(),
    };

    return await this.client.setBreakpoint(this.id, updated);
  }

  // Watch expressions
  async addWatchExpression(expression: string, name?: string): Promise<Debug.WatchExpression> {
    const watch = await this.client.addWatch(this.id, expression, name);

    // Update local session info
    this.sessionInfo.watchExpressions.push(watch);

    return watch;
  }

  async evaluateWatchExpressions(snapshotId?: string): Promise<Debug.WatchExpression[]> {
    const targetSnapshotId = snapshotId || this.currentSnapshot?.id;
    if (!targetSnapshotId) {
      return this.sessionInfo.watchExpressions;
    }

    const results = await Promise.allSettled(
      this.sessionInfo.watchExpressions.map(async (watch) => {
        if (!watch.enabled) return watch;

        try {
          const result = await this.evaluateExpression(watch.expression, targetSnapshotId);
          return {
            ...watch,
            lastValue: result.result,
            lastType: result.type,
            lastError: result.error,
            evaluationCount: watch.evaluationCount + 1,
          };
        } catch (error) {
          return {
            ...watch,
            lastError: error instanceof Error ? error.message : String(error),
            evaluationCount: watch.evaluationCount + 1,
          };
        }
      })
    );

    return results.map((result, index) =>
      result.status === 'fulfilled' ? result.value : this.sessionInfo.watchExpressions[index]
    );
  }

  // State management
  pause(): void {
    this.state.isPaused = true;
    this.state.autoStep = false;
    this.emitStateChanged();
  }

  resume(): void {
    this.state.isPaused = false;
    this.emitStateChanged();
  }

  setAutoStep(enabled: boolean, delay = 1000): void {
    this.state.autoStep = enabled;
    this.state.stepDelay = delay;
    if (enabled) {
      this.state.isPaused = false;
    }
    this.emitStateChanged();
  }

  setExecutionDirection(direction: 'forward' | 'backward'): void {
    this.state.executionDirection = direction;
    this.emitStateChanged();
  }

  // Session refresh
  async refresh(): Promise<Debug.Session> {
    this.sessionInfo = await this.client.getSessionInfo(this.id);
    return this.sessionInfo;
  }

  // Cleanup
  destroy(): void {
    this.state.isActive = false;
    this.removeAllListeners();
  }

  // Private methods
  private setupEventHandlers(): void {
    this.client.on('snapshotUpdate', (event) => {
      if (event.data.sessionId === this.id) {
        this.updateCurrentSnapshot(event.data.snapshot);
      }
    });

    this.client.on('breakpointHit', (event) => {
      if (event.data.sessionId === this.id) {
        this.pause();
        this.updateCurrentSnapshot(event.data.snapshot);
        this.emit('breakpointHit', event.data.breakpoint, event.data.snapshot);
      }
    });

    this.client.on('sessionEnded', (event) => {
      if (event.data.sessionId === this.id) {
        this.state.isActive = false;
        this.emit('ended', event.data.reason);
      }
    });

    this.client.on('error', (error) => {
      this.emit('error', error);
    });
  }

  private async updateCurrentSnapshot(snapshot: Engine.Snapshot): Promise<void> {
    this.currentSnapshot = snapshot;
    this.state.currentSnapshot = snapshot;
    this.sessionInfo.currentSnapshotId = snapshot.id;
    this.emit('snapshotChanged', snapshot);
  }

  private emitStateChanged(): void {
    this.emit('stateChanged', { ...this.state });
  }
}