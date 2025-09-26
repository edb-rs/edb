/**
 * @fileoverview JSON-RPC type definitions
 * @description Types for EDB JSON-RPC API communication
 */

import { EDB } from '../index';
import { Engine } from '../engine';
import { Debug } from '../debug';

export namespace RPC {
  // JSON-RPC 2.0 base types
  export interface JSONRPCRequest {
    jsonrpc: '2.0';
    id: string | number;
    method: string;
    params?: any;
  }

  export interface JSONRPCResponse<T = any> {
    jsonrpc: '2.0';
    id: string | number;
    result?: T;
    error?: JSONRPCError;
  }

  export interface JSONRPCError {
    code: number;
    message: string;
    data?: any;
  }

  // WebSocket event types
  export interface WebSocketEvent<T = any> {
    type: string;
    data: T;
    timestamp: number;
  }

  // EDB-specific RPC methods

  // Session management
  export interface CreateSessionRequest extends JSONRPCRequest {
    method: 'edb_createSession';
    params: {
      transactionHash: EDB.Hash;
      blockNumber?: EDB.BlockNumber;
    };
  }

  export interface CreateSessionResponse extends JSONRPCResponse<Debug.Session> {}

  // Snapshot operations
  export interface GetSnapshotRequest extends JSONRPCRequest {
    method: 'edb_getSnapshot';
    params: {
      sessionId: string;
      snapshotId: string;
    };
  }

  export interface GetSnapshotResponse extends JSONRPCResponse<Engine.Snapshot> {}

  export interface ListSnapshotsRequest extends JSONRPCRequest {
    method: 'edb_listSnapshots';
    params: {
      sessionId: string;
      offset?: number;
      limit?: number;
    };
  }

  export interface ListSnapshotsResponse extends JSONRPCResponse<{
    snapshots: Engine.Snapshot[];
    total: number;
  }> {}

  // Navigation
  export interface NavigateRequest extends JSONRPCRequest {
    method: 'edb_navigate';
    params: {
      sessionId: string;
      direction: 'next' | 'previous' | 'stepIn' | 'stepOut';
      count?: number;
    };
  }

  export interface NavigateResponse extends JSONRPCResponse<{
    currentSnapshot: Engine.Snapshot;
    canStepNext: boolean;
    canStepPrevious: boolean;
  }> {}

  // Trace operations
  export interface GetTraceRequest extends JSONRPCRequest {
    method: 'edb_getTrace';
    params: {
      sessionId: string;
    };
  }

  export interface GetTraceResponse extends JSONRPCResponse<Engine.ExecutionTrace> {}

  // Expression evaluation
  export interface EvaluateExpressionRequest extends JSONRPCRequest {
    method: 'edb_evaluateExpression';
    params: {
      sessionId: string;
      snapshotId: string;
      expression: string;
    };
  }

  export interface EvaluateExpressionResponse extends JSONRPCResponse<{
    result: string;
    type: string;
    error?: string;
  }> {}

  // Breakpoint management
  export interface SetBreakpointRequest extends JSONRPCRequest {
    method: 'edb_setBreakpoint';
    params: {
      sessionId: string;
      breakpoint: Debug.Breakpoint;
    };
  }

  export interface SetBreakpointResponse extends JSONRPCResponse<Debug.Breakpoint> {}

  export interface RemoveBreakpointRequest extends JSONRPCRequest {
    method: 'edb_removeBreakpoint';
    params: {
      sessionId: string;
      breakpointId: string;
    };
  }

  export interface RemoveBreakpointResponse extends JSONRPCResponse<boolean> {}

  // Variable watching
  export interface AddWatchRequest extends JSONRPCRequest {
    method: 'edb_addWatch';
    params: {
      sessionId: string;
      expression: string;
      name?: string;
    };
  }

  export interface AddWatchResponse extends JSONRPCResponse<Debug.WatchExpression> {}

  // Session info
  export interface GetSessionInfoRequest extends JSONRPCRequest {
    method: 'edb_getSessionInfo';
    params: {
      sessionId: string;
    };
  }

  export interface GetSessionInfoResponse extends JSONRPCResponse<Debug.Session> {}

  // WebSocket event types
  export type SnapshotUpdateEvent = WebSocketEvent<{
    sessionId: string;
    snapshot: Engine.Snapshot;
  }>;

  export type BreakpointHitEvent = WebSocketEvent<{
    sessionId: string;
    breakpoint: Debug.Breakpoint;
    snapshot: Engine.Snapshot;
  }>;

  export type SessionEndedEvent = WebSocketEvent<{
    sessionId: string;
    reason: string;
  }>;

  // Union types for easier handling
  export type EDBRequest =
    | CreateSessionRequest
    | GetSnapshotRequest
    | ListSnapshotsRequest
    | NavigateRequest
    | GetTraceRequest
    | EvaluateExpressionRequest
    | SetBreakpointRequest
    | RemoveBreakpointRequest
    | AddWatchRequest
    | GetSessionInfoRequest;

  export type EDBResponse =
    | CreateSessionResponse
    | GetSnapshotResponse
    | ListSnapshotsResponse
    | NavigateResponse
    | GetTraceResponse
    | EvaluateExpressionResponse
    | SetBreakpointResponse
    | RemoveBreakpointResponse
    | AddWatchResponse
    | GetSessionInfoResponse;

  export type EDBEvent =
    | SnapshotUpdateEvent
    | BreakpointHitEvent
    | SessionEndedEvent;
}