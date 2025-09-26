/**
 * @fileoverview Advanced caching system inspired by Rust TUI
 * @description Implements FetchCache and intelligent state management
 */

export enum CacheStatus {
  NotRequested = 'not_requested',
  Pending = 'pending',
  Cached = 'cached',
  Error = 'error'
}

export interface CacheEntry<T> {
  data: T | null;
  status: CacheStatus;
  timestamp: number;
  error?: string;
}

/**
 * Generic cache for storing fetched data with status tracking
 * Inspired by Rust TUI's FetchCache<K, V>
 */
export class FetchCache<K, V> {
  private cache = new Map<string, CacheEntry<V>>();
  private ttl: number; // Time to live in milliseconds

  constructor(ttl: number = 5 * 60 * 1000) { // 5 minutes default TTL
    this.ttl = ttl;
  }

  /**
   * Get cache key as string (handles complex keys)
   */
  private getCacheKey(key: K): string {
    if (typeof key === 'string' || typeof key === 'number') {
      return key.toString();
    }
    return JSON.stringify(key);
  }

  /**
   * Check if entry exists and is valid
   */
  has(key: K): boolean {
    const cacheKey = this.getCacheKey(key);
    const entry = this.cache.get(cacheKey);

    if (!entry) return false;

    // Check TTL
    if (Date.now() - entry.timestamp > this.ttl) {
      this.cache.delete(cacheKey);
      return false;
    }

    return entry.status === CacheStatus.Cached;
  }

  /**
   * Get cached data if available
   */
  get(key: K): V | null {
    const cacheKey = this.getCacheKey(key);
    const entry = this.cache.get(cacheKey);

    if (!entry || entry.status !== CacheStatus.Cached) {
      return null;
    }

    // Check TTL
    if (Date.now() - entry.timestamp > this.ttl) {
      this.cache.delete(cacheKey);
      return null;
    }

    return entry.data;
  }

  /**
   * Get cache status for a key
   */
  getStatus(key: K): CacheStatus {
    const cacheKey = this.getCacheKey(key);
    const entry = this.cache.get(cacheKey);

    if (!entry) return CacheStatus.NotRequested;

    // Check TTL
    if (Date.now() - entry.timestamp > this.ttl) {
      this.cache.delete(cacheKey);
      return CacheStatus.NotRequested;
    }

    return entry.status;
  }

  /**
   * Mark a key as pending (being fetched)
   */
  setPending(key: K): void {
    const cacheKey = this.getCacheKey(key);
    this.cache.set(cacheKey, {
      data: null,
      status: CacheStatus.Pending,
      timestamp: Date.now()
    });
  }

  /**
   * Set cached data for a key
   */
  set(key: K, data: V): void {
    const cacheKey = this.getCacheKey(key);
    this.cache.set(cacheKey, {
      data,
      status: CacheStatus.Cached,
      timestamp: Date.now()
    });
  }

  /**
   * Set error for a key
   */
  setError(key: K, error: string): void {
    const cacheKey = this.getCacheKey(key);
    this.cache.set(cacheKey, {
      data: null,
      status: CacheStatus.Error,
      timestamp: Date.now(),
      error
    });
  }

  /**
   * Clear specific key
   */
  delete(key: K): void {
    const cacheKey = this.getCacheKey(key);
    this.cache.delete(cacheKey);
  }

  /**
   * Clear all cached data
   */
  clear(): void {
    this.cache.clear();
  }

  /**
   * Get all pending keys (for batch processing)
   */
  getPendingKeys(): K[] {
    const pendingKeys: K[] = [];

    for (const [keyStr, entry] of this.cache.entries()) {
      if (entry.status === CacheStatus.Pending) {
        try {
          // Try to parse back to original key type
          const key = JSON.parse(keyStr) as K;
          pendingKeys.push(key);
        } catch {
          // If it's a simple string/number, use as is
          pendingKeys.push(keyStr as K);
        }
      }
    }

    return pendingKeys;
  }

  /**
   * Get cache statistics
   */
  getStats() {
    let cached = 0, pending = 0, errors = 0;

    for (const entry of this.cache.values()) {
      switch (entry.status) {
        case CacheStatus.Cached: cached++; break;
        case CacheStatus.Pending: pending++; break;
        case CacheStatus.Error: errors++; break;
      }
    }

    return {
      total: this.cache.size,
      cached,
      pending,
      errors
    };
  }

  /**
   * Check if cache needs update from another cache (for state sync)
   */
  needsUpdate(other: FetchCache<K, V>): boolean {
    // Simple version: check if other has more cached items
    const thisStats = this.getStats();
    const otherStats = other.getStats();
    return otherStats.cached > thisStats.cached;
  }

  /**
   * Update from another cache (for state sync)
   */
  update(other: FetchCache<K, V>): void {
    // Merge cached items from other cache
    for (const [key, entry] of other.cache.entries()) {
      if (entry.status === CacheStatus.Cached) {
        this.cache.set(key, { ...entry });
      }
    }
  }
}