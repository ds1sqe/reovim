/**
 * Web Client Service Registry
 *
 * Simple Map-based implementation of `ClientServiceRegistry`.
 * TypeScript version uses string keys (no TypeId equivalent).
 *
 * @module core/service-registry
 */

import type { ClientServiceRegistry } from "./contracts.js";

/**
 * Cross-module service registry for the web client.
 *
 * Modules register services by string key during init, then other
 * modules look them up at runtime.
 */
export class WebClientServiceRegistry implements ClientServiceRegistry {
  private services: Map<string, unknown> = new Map();

  register<T>(key: string, service: T): void {
    this.services.set(key, service);
  }

  get<T>(key: string): T | undefined {
    return this.services.get(key) as T | undefined;
  }

  contains(key: string): boolean {
    return this.services.has(key);
  }

  get size(): number {
    return this.services.size;
  }
}
