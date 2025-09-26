/**
 * @fileoverview Client-side caching implementation
 * @description LRU cache with TTL support for RPC responses
 */

export interface CacheEntry<T> {
  value: T;
  expires: number;
  accessed: number;
}

export interface CacheConfig {
  maxSize: number;
  ttl: number; // Time to live in milliseconds
}

export class EDBCache {
  private cache = new Map<string, CacheEntry<any>>();
  private accessOrder: string[] = [];
  private config: CacheConfig;

  constructor(config: CacheConfig) {
    this.config = config;
  }

  get<T>(key: string): T | null {
    const entry = this.cache.get(key);

    if (!entry) {
      return null;
    }

    // Check if expired
    if (Date.now() > entry.expires) {
      this.delete(key);
      return null;
    }

    // Update access time and order
    entry.accessed = Date.now();
    this.updateAccessOrder(key);

    return entry.value;
  }

  set<T>(key: string, value: T, ttl?: number): void {
    const expires = Date.now() + (ttl || this.config.ttl);
    const entry: CacheEntry<T> = {
      value,
      expires,
      accessed: Date.now(),
    };

    // If cache is full, remove least recently used
    if (this.cache.size >= this.config.maxSize && !this.cache.has(key)) {
      this.evictLRU();
    }

    this.cache.set(key, entry);
    this.updateAccessOrder(key);
  }

  delete(key: string): boolean {
    const deleted = this.cache.delete(key);
    if (deleted) {
      const index = this.accessOrder.indexOf(key);
      if (index > -1) {
        this.accessOrder.splice(index, 1);
      }
    }
    return deleted;
  }

  has(key: string): boolean {
    const entry = this.cache.get(key);
    if (!entry) return false;

    // Check if expired
    if (Date.now() > entry.expires) {
      this.delete(key);
      return false;
    }

    return true;
  }

  clear(): void {
    this.cache.clear();
    this.accessOrder = [];
  }

  size(): number {
    return this.cache.size;
  }

  // Get cache statistics
  getStats(): {
    size: number;
    maxSize: number;
    hitRate: number;
    entries: Array<{
      key: string;
      size: number;
      expires: number;
      accessed: number;
    }>;
  } {
    const entries = Array.from(this.cache.entries()).map(([key, entry]) => ({
      key,
      size: JSON.stringify(entry.value).length,
      expires: entry.expires,
      accessed: entry.accessed,
    }));

    return {
      size: this.cache.size,
      maxSize: this.config.maxSize,
      hitRate: 0, // TODO: Implement hit rate tracking
      entries,
    };
  }

  // Clean up expired entries
  cleanup(): number {
    const now = Date.now();
    let cleaned = 0;

    for (const [key, entry] of this.cache.entries()) {
      if (now > entry.expires) {
        this.delete(key);
        cleaned++;
      }
    }

    return cleaned;
  }

  // Update TTL for existing entry
  touch(key: string, newTtl?: number): boolean {
    const entry = this.cache.get(key);
    if (!entry) return false;

    entry.expires = Date.now() + (newTtl || this.config.ttl);
    entry.accessed = Date.now();
    this.updateAccessOrder(key);

    return true;
  }

  // Get keys by pattern
  keys(pattern?: RegExp): string[] {
    const keys = Array.from(this.cache.keys());

    if (!pattern) {
      return keys;
    }

    return keys.filter(key => pattern.test(key));
  }

  // Private methods
  private updateAccessOrder(key: string): void {
    // Remove from current position
    const index = this.accessOrder.indexOf(key);
    if (index > -1) {
      this.accessOrder.splice(index, 1);
    }

    // Add to end (most recently used)
    this.accessOrder.push(key);
  }

  private evictLRU(): void {
    if (this.accessOrder.length === 0) return;

    // Remove least recently used (first in access order)
    const lruKey = this.accessOrder[0];
    this.delete(lruKey);
  }
}