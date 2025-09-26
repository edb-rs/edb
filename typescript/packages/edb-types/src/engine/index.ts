/**
 * @fileoverview Engine-related type definitions
 * @description Types for EDB engine data structures and state
 */

import { EDB } from '../index';

export namespace Engine {
  // Source location information
  export interface SourceLocation {
    file: string;
    line: number;
    column: number;
    length?: number;
  }

  // Variable information
  export interface Variable {
    name: string;
    type: string;
    value: string;
    isStorage?: boolean;
    isMemory?: boolean;
    storageSlot?: EDB.HexString;
    memoryOffset?: number;
    scope?: VariableScope;
  }

  export type VariableScope = 'local' | 'parameter' | 'return' | 'global';

  // Stack frame information
  export interface StackFrame {
    depth: number;
    contractAddress: EDB.Address;
    functionName?: string;
    sourceLocation?: SourceLocation;
    variables: Variable[];
  }

  // Memory state
  export interface MemoryState {
    data: EDB.HexString;
    size: number;
  }

  // Storage state
  export interface StorageState {
    [slot: EDB.HexString]: EDB.HexString;
  }

  // EVM execution context
  export interface ExecutionContext {
    pc: number; // Program counter
    opcode: string;
    gas: EDB.GasAmount;
    gasUsed: EDB.GasAmount;
    depth: number;
    stack: EDB.HexString[];
    memory: MemoryState;
    storage: StorageState;
  }

  // Execution snapshot
  export interface Snapshot {
    id: string;
    frameId: string;
    stepIndex: number;
    sourceLocation?: SourceLocation;
    variables: Variable[];
    stack: StackFrame[];
    memory: MemoryState;
    storage: StorageState;
    context: ExecutionContext;
    timestamp: number;
  }

  // Call trace information
  export interface CallTrace {
    id: string;
    type: 'CALL' | 'STATICCALL' | 'DELEGATECALL' | 'CREATE' | 'CREATE2';
    from: EDB.Address;
    to: EDB.Address;
    value: EDB.WeiAmount;
    gas: EDB.GasAmount;
    gasUsed: EDB.GasAmount;
    input: EDB.Bytes;
    output: EDB.Bytes;
    error?: string;
    children: CallTrace[];
    depth: number;
    sourceLocation?: SourceLocation;
  }

  // Contract artifact information
  export interface ContractArtifact {
    address: EDB.Address;
    name?: string;
    source?: string;
    abi?: any[]; // ABI array
    bytecode: EDB.Bytes;
    sourceMap?: string;
    isVerified: boolean;
  }

  // Execution trace
  export interface ExecutionTrace {
    transactionHash: EDB.Hash;
    blockNumber: EDB.BlockNumber;
    snapshots: Snapshot[];
    callTrace: CallTrace;
    artifacts: Record<EDB.Address, ContractArtifact>;
    gasUsed: EDB.GasAmount;
    success: boolean;
    error?: string;
  }
}