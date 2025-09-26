/**
 * @fileoverview Main exports for EDB client library
 * @description Unified RPC client for communicating with EDB engine
 */

// Core client exports
export { EDBClient } from './client';
export { EDBSession } from './session';

// Cache exports
export { EDBCache } from './cache';

// Utility exports
export * from './utils';

// Re-export commonly used types
export type {
  EDB,
  RPC,
  Engine,
  Debug
} from '@edb/types';

// Client configuration
export interface EDBClientConfig {
  url: string;
  reconnectInterval?: number;
  maxReconnectAttempts?: number;
  requestTimeout?: number;
  enableCache?: boolean;
  cacheConfig?: {
    maxSize: number;
    ttl: number;
  };
}

// Default configuration
export const DEFAULT_CLIENT_CONFIG: Required<EDBClientConfig> = {
  url: 'ws://localhost:8545',
  reconnectInterval: 5000,
  maxReconnectAttempts: 5,
  requestTimeout: 30000,
  enableCache: true,
  cacheConfig: {
    maxSize: 1000,
    ttl: 300000, // 5 minutes
  },
};