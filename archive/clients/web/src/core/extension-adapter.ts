/**
 * WebExtension Adapter
 *
 * Wraps existing `WebExtension` implementations into the `ClientModule`
 * interface, preserving all existing behavior. This enables incremental
 * migration -- existing extensions keep working without changes.
 *
 * @module core/extension-adapter
 */

import type { ClientModule, ModuleContext } from "./contracts.js";
import type { Version, ProbeResult } from "./types.js";
import type { WebExtension } from "../extensions/interface.js";

/**
 * Adapter wrapping a `WebExtension` into a `ClientModule`.
 *
 * Provides defaults for all new CLM methods that `WebExtension`
 * does not have (roles, chrome metadata, events, etc.).
 */
export class WebExtensionAdapter implements ClientModule {
  private ext: WebExtension;

  constructor(ext: WebExtension) {
    this.ext = ext;
  }

  // ---- Identity ----

  id(): string {
    return this.ext.kind();
  }

  kind(): string {
    return this.ext.kind();
  }

  name(): string {
    return this.ext.kind();
  }

  version(): Version {
    return { major: 0, minor: 1, patch: 0 };
  }

  // ---- Lifecycle ----

  init(_ctx: ModuleContext): ProbeResult {
    this.ext.init?.();
    return { status: "success" };
  }

  exit(): void {
    this.ext.exit?.();
  }

  // ---- Roles ----

  hasChrome(): boolean {
    return true; // All existing web extensions render chrome
  }

  hasBufferContrib(): boolean {
    return false;
  }

  hasAnnotations(): boolean {
    return false;
  }

  // ---- Events ----

  onNotification(data: string): void {
    this.ext.applyNotification(data);
  }

  // ---- Chrome ----

  chromePosition(): "overlay" {
    return "overlay";
  }

  // ---- Dependencies ----

  dependencies(): string[] {
    return this.ext.dependencies?.() ?? [];
  }

  serverKinds(): string[] {
    return this.ext.serverKinds?.() ?? [this.ext.kind()];
  }

  // ---- Test support ----

  getState(): Record<string, unknown> | null {
    return this.ext.getState();
  }

  /**
   * Access the wrapped WebExtension for DOM rendering.
   *
   * This is used by the editor to continue delegating DOM operations
   * (render/hide/isActive) to the original extension.
   */
  get wrappedExtension(): WebExtension {
    return this.ext;
  }
}

/** Wrap a single WebExtension into a ClientModule. */
export function adaptExtension(ext: WebExtension): WebExtensionAdapter {
  return new WebExtensionAdapter(ext);
}

/** Wrap an array of WebExtensions into ClientModules. */
export function adaptExtensions(exts: WebExtension[]): WebExtensionAdapter[] {
  return exts.map((ext) => new WebExtensionAdapter(ext));
}
