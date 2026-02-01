/**
 * Style group registry for module-provided defaults.
 *
 * Modules can register their own style groups at runtime.
 * Used as tier 3 in the 4-tier lookup order.
 *
 * @module theme/registry
 */

import type { ResolvedStyle } from './types';

/**
 * Registry for module-provided style defaults.
 *
 * Matches the TUI Rust implementation:
 * ```rust
 * pub struct StyleGroupRegistry {
 *     groups: RwLock<HashMap<&'static str, Style>>,
 * }
 * ```
 *
 * @example
 * ```typescript
 * const registry = new StyleGroupRegistry();
 *
 * // Register custom groups
 * registry.register('rainbow.bracket.1', { fg: '#e06c75', ... });
 * registry.register('rainbow.bracket.2', { fg: '#98c379', ... });
 *
 * // Batch registration
 * registry.registerBatch([
 *   ['my.custom.1', style1],
 *   ['my.custom.2', style2],
 * ]);
 *
 * // Retrieve
 * const style = registry.get('rainbow.bracket.1');
 * ```
 */
export class StyleGroupRegistry {
  private groups: Map<string, ResolvedStyle>;

  constructor() {
    this.groups = new Map();
  }

  /**
   * Register a style for a highlight group.
   *
   * @param group - Group name (e.g., "rainbow.bracket.1")
   * @param style - Resolved style to register
   */
  register(group: string, style: ResolvedStyle): void {
    this.groups.set(group, style);
  }

  /**
   * Register multiple styles at once.
   *
   * @param registrations - Array of [group, style] tuples
   */
  registerBatch(registrations: [string, ResolvedStyle][]): void {
    for (const [group, style] of registrations) {
      this.groups.set(group, style);
    }
  }

  /**
   * Get a style by group name.
   *
   * @param group - Group name to look up
   * @returns Resolved style or null if not registered
   */
  get(group: string): ResolvedStyle | null {
    return this.groups.get(group) ?? null;
  }

  /**
   * Check if a group is registered.
   *
   * @param group - Group name to check
   */
  has(group: string): boolean {
    return this.groups.has(group);
  }

  /**
   * Remove a registered group.
   *
   * @param group - Group name to remove
   * @returns true if the group was removed
   */
  remove(group: string): boolean {
    return this.groups.delete(group);
  }

  /**
   * Clear all registered groups.
   */
  clear(): void {
    this.groups.clear();
  }

  /**
   * Get all registered group names.
   */
  registeredGroups(): string[] {
    return Array.from(this.groups.keys());
  }

  /**
   * Get the number of registered groups.
   */
  get size(): number {
    return this.groups.size;
  }
}
