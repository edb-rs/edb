/**
 * @fileoverview Main exports for EDB TypeScript types
 * @description Central type definitions for the EDB frontend ecosystem
 */

// Re-export all type modules
export * from './rpc';
export * from './engine';
export * from './debug';
export * from './ui';

// Core namespace for all EDB types
export namespace EDB {
  // Common utility types
  export type HexString = `0x${string}`;
  export type Address = HexString;
  export type Hash = HexString;
  export type Bytes = HexString;

  // Ethereum primitive types
  export type BlockNumber = number;
  export type ChainId = number;
  export type GasAmount = number;
  export type WeiAmount = string; // Use string to handle large numbers
}

// Version information
export const VERSION = '0.0.1';
export const SUPPORTED_RPC_VERSION = '2.0';