/**
 * WebExtension interface — mechanism layer for client-side extensions.
 *
 * TypeScript equivalent of the Rust `TuiExtension` trait
 * (`ext/client/tui/drivers/display/src/render_backend.rs`).
 *
 * The editor dispatches generically over `WebExtension[]` via `kind()` matching.
 * Individual extensions own their state, JSON parsing, DOM rendering, and lifecycle.
 */
export interface WebExtension {
  /** Extension identifier (e.g., "cmdline", "whichkey"). */
  kind(): string;

  /** Whether the extension UI is currently visible. */
  isActive(): boolean;

  /** Update internal state from a server notification JSON payload. */
  applyNotification(data: string): void;

  /** Create or update DOM elements inside the given container. */
  render(container: HTMLElement): void;

  /** Remove or hide DOM elements. */
  hide(): void;

  /** Return parsed state for headless testing (no DOM needed). */
  getState(): Record<string, unknown> | null;

  /** Extension kinds this extension depends on (default: empty). */
  dependencies?(): string[];

  /** Called after all extensions are created and sorted. */
  init?(): void;

  /** Called during shutdown in reverse dependency order. */
  exit?(): void;

  /** Server extension kinds this extension expects (default: [this.kind()]). */
  serverKinds?(): string[];
}
