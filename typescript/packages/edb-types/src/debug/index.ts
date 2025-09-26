/**
 * @fileoverview Debug session and breakpoint types
 * @description Types for debugging sessions, breakpoints, and watch expressions
 */

import { EDB } from '../index';
import { Engine } from '../engine';

export namespace Debug {
  // Debug session
  export interface Session {
    id: string;
    transactionHash: EDB.Hash;
    blockNumber: EDB.BlockNumber;
    chainId: EDB.ChainId;
    currentSnapshotId?: string;
    breakpoints: Breakpoint[];
    watchExpressions: WatchExpression[];
    status: SessionStatus;
    createdAt: number;
    lastActiveAt: number;
  }

  export type SessionStatus = 'initializing' | 'ready' | 'running' | 'paused' | 'ended' | 'error';

  // Breakpoint types
  export interface Breakpoint {
    id: string;
    type: BreakpointType;
    enabled: boolean;
    condition?: string;
    hitCount: number;
    // Location-based breakpoint
    sourceLocation?: Engine.SourceLocation;
    // Address-based breakpoint
    address?: EDB.Address;
    // Function-based breakpoint
    functionName?: string;
    // Opcode-based breakpoint
    opcode?: string;
    // Event-based breakpoint
    eventSignature?: string;
    // Created/modified timestamps
    createdAt: number;
    modifiedAt: number;
  }

  export type BreakpointType =
    | 'line'        // Break at specific source line
    | 'function'    // Break at function entry
    | 'address'     // Break at specific address
    | 'opcode'      // Break at specific opcode
    | 'event'       // Break on event emission
    | 'exception'   // Break on exception/revert
    | 'storage'     // Break on storage access
    | 'call';       // Break on external call

  // Watch expressions
  export interface WatchExpression {
    id: string;
    expression: string;
    name?: string;
    enabled: boolean;
    lastValue?: string;
    lastType?: string;
    lastError?: string;
    evaluationCount: number;
    createdAt: number;
    modifiedAt: number;
  }

  // Debugging state
  export interface DebuggingState {
    isActive: boolean;
    isPaused: boolean;
    currentSnapshot?: Engine.Snapshot;
    executionDirection: 'forward' | 'backward';
    autoStep: boolean;
    stepDelay: number; // milliseconds
  }

  // Call stack information for debugging
  export interface CallStackFrame {
    id: string;
    contractAddress: EDB.Address;
    contractName?: string;
    functionName?: string;
    functionSelector?: EDB.HexString;
    sourceLocation?: Engine.SourceLocation;
    depth: number;
    isInternal: boolean;
    gas: EDB.GasAmount;
    gasUsed: EDB.GasAmount;
  }

  // Variable inspection
  export interface VariableInspection {
    variable: Engine.Variable;
    path: string; // Dot notation path like "myStruct.field[0]"
    isExpandable: boolean;
    children?: VariableInspection[];
  }

  // Source code information
  export interface SourceFile {
    path: string;
    content: string;
    language: 'solidity' | 'vyper' | 'yul';
    compiled: boolean;
    verified: boolean;
    sourceMap?: string;
  }

  // Execution statistics
  export interface ExecutionStats {
    totalGasUsed: EDB.GasAmount;
    totalSteps: number;
    totalCalls: number;
    totalStorage: number;
    executionTime: number; // milliseconds
    contractsCalled: EDB.Address[];
  }

  // Debug configuration
  export interface DebugConfig {
    maxSnapshots: number;
    enableStorage: boolean;
    enableMemory: boolean;
    enableStack: boolean;
    stepLimit: number;
    gasLimit: EDB.GasAmount;
    autoBreakOnRevert: boolean;
    autoBreakOnCall: boolean;
  }

  // Error types
  export interface DebugError {
    code: string;
    message: string;
    details?: any;
    snapshot?: Engine.Snapshot;
    timestamp: number;
  }

  // Session events
  export type SessionEvent = {
    type: 'created' | 'started' | 'paused' | 'resumed' | 'stepped' | 'ended' | 'error';
    sessionId: string;
    data?: any;
    timestamp: number;
  };
}