/**
 * @fileoverview Utility functions for EDB client
 * @description Helper functions and utilities
 */

// Generate unique request ID
export function generateRequestId(): string {
  return `req_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
}

// Delay utility for reconnection backoff
export function delay(ms: number): Promise<void> {
  return new Promise(resolve => setTimeout(resolve, ms));
}

// Validate Ethereum address
export function isValidAddress(address: string): boolean {
  return /^0x[a-fA-F0-9]{40}$/.test(address);
}

// Validate transaction hash
export function isValidTxHash(hash: string): boolean {
  return /^0x[a-fA-F0-9]{64}$/.test(hash);
}

// Format gas amount for display
export function formatGas(gas: number): string {
  if (gas < 1000) return gas.toString();
  if (gas < 1000000) return `${(gas / 1000).toFixed(1)}K`;
  return `${(gas / 1000000).toFixed(1)}M`;
}

// Format Wei amount for display
export function formatWei(wei: string): string {
  const value = BigInt(wei);
  const eth = value / BigInt('1000000000000000000');
  const remainder = value % BigInt('1000000000000000000');

  if (eth > 0) {
    return `${eth}.${remainder.toString().padStart(18, '0').slice(0, 4)} ETH`;
  }

  const gwei = value / BigInt('1000000000');
  if (gwei > 0) {
    return `${gwei} Gwei`;
  }

  return `${value} Wei`;
}

// Format source location for display
export function formatSourceLocation(location: { file: string; line: number; column: number }): string {
  const filename = location.file.split('/').pop() || location.file;
  return `${filename}:${location.line}:${location.column}`;
}

// Truncate hex string for display
export function truncateHex(hex: string, length = 8): string {
  if (hex.length <= length + 2) return hex; // +2 for '0x'
  return `${hex.slice(0, length + 2)}...${hex.slice(-length / 2)}`;
}

// Debounce function
export function debounce<T extends (...args: any[]) => any>(
  func: T,
  wait: number
): (...args: Parameters<T>) => void {
  let timeout: NodeJS.Timeout;

  return (...args: Parameters<T>) => {
    clearTimeout(timeout);
    timeout = setTimeout(() => func.apply(null, args), wait);
  };
}

// Throttle function
export function throttle<T extends (...args: any[]) => any>(
  func: T,
  limit: number
): (...args: Parameters<T>) => void {
  let inThrottle: boolean;

  return (...args: Parameters<T>) => {
    if (!inThrottle) {
      func.apply(null, args);
      inThrottle = true;
      setTimeout(() => (inThrottle = false), limit);
    }
  };
}

// Deep clone utility
export function deepClone<T>(obj: T): T {
  if (obj === null || typeof obj !== 'object') return obj;
  if (obj instanceof Date) return new Date(obj.getTime()) as unknown as T;
  if (obj instanceof Array) return obj.map(item => deepClone(item)) as unknown as T;

  const cloned = {} as T;
  for (const key in obj) {
    if (Object.prototype.hasOwnProperty.call(obj, key)) {
      cloned[key] = deepClone(obj[key]);
    }
  }
  return cloned;
}

// Parse WebSocket URL
export function parseWebSocketUrl(url: string): { protocol: string; host: string; port: number; path: string } {
  const urlObj = new URL(url);

  return {
    protocol: urlObj.protocol,
    host: urlObj.hostname,
    port: urlObj.port ? parseInt(urlObj.port) : (urlObj.protocol === 'wss:' ? 443 : 80),
    path: urlObj.pathname + urlObj.search,
  };
}

// Retry utility with exponential backoff
export async function retry<T>(
  fn: () => Promise<T>,
  options: {
    maxAttempts: number;
    baseDelay: number;
    maxDelay: number;
    backoffFactor: number;
  }
): Promise<T> {
  let attempt = 1;
  let lastError: Error;

  while (attempt <= options.maxAttempts) {
    try {
      return await fn();
    } catch (error) {
      lastError = error as Error;

      if (attempt === options.maxAttempts) {
        throw lastError;
      }

      const delayMs = Math.min(
        options.baseDelay * Math.pow(options.backoffFactor, attempt - 1),
        options.maxDelay
      );

      await delay(delayMs);
      attempt++;
    }
  }

  throw lastError!;
}

// Type guard for checking if value is defined
export function isDefined<T>(value: T | null | undefined): value is T {
  return value !== null && value !== undefined;
}

// Type guard for checking if value is a string
export function isString(value: unknown): value is string {
  return typeof value === 'string';
}

// Type guard for checking if value is a number
export function isNumber(value: unknown): value is number {
  return typeof value === 'number' && !isNaN(value);
}

// Error handling utilities
export class EDBClientError extends Error {
  constructor(
    message: string,
    public code?: string,
    public details?: any
  ) {
    super(message);
    this.name = 'EDBClientError';
  }
}

export class EDBNetworkError extends EDBClientError {
  constructor(message: string, details?: any) {
    super(message, 'NETWORK_ERROR', details);
    this.name = 'EDBNetworkError';
  }
}

export class EDBTimeoutError extends EDBClientError {
  constructor(message: string = 'Request timeout') {
    super(message, 'TIMEOUT_ERROR');
    this.name = 'EDBTimeoutError';
  }
}

export class EDBValidationError extends EDBClientError {
  constructor(message: string, field?: string) {
    super(message, 'VALIDATION_ERROR', { field });
    this.name = 'EDBValidationError';
  }
}