/**
 * CLM Contract Interfaces
 *
 * TypeScript equivalents of the Rust CLM traits from
 * `shared/clients/driver/src/traits/mod.rs`.
 *
 * These are interface-only definitions. Implementations live in
 * separate adapter files (`platform-adapter.ts`, `server-handle.ts`, etc.).
 *
 * @module core/contracts
 */

import type {
  Version,
  ProbeResult,
  ClientModuleState,
  ChromePosition,
  ColumnWidth,
  RenderingModel,
  ColorDepth,
  Rect,
  Insets,
  Style,
  OptionValue,
  OptionMetadata,
} from "./types.js";

// =============================================================================
// PlatformCapabilities
// =============================================================================

/**
 * Platform capability queries.
 *
 * Mirrors the Rust `PlatformCapabilities` trait. Modules use this to
 * adapt behavior to the runtime environment (browser, terminal, etc.).
 */
export interface PlatformCapabilities {
  /** How the client renders content. */
  renderingModel(): RenderingModel;

  /** Grid dimensions in cells (null if not grid-based). */
  gridSize(): { width: number; height: number } | null;

  /** Color depth supported by the display. */
  colorDepth(): ColorDepth;

  /** Pixel dimensions of the viewport (null if not available). */
  pixelSize(): { width: number; height: number } | null;

  /** Whether Unicode width calculation is reliable. */
  reliableUnicodeWidth(): boolean;

  /** Whether the system is in dark mode. */
  darkMode(): boolean;

  /** Whether smooth scrolling is available. */
  smoothScroll(): boolean;

  /** Whether pointer (mouse) events are available. */
  pointerEvents(): boolean;

  /** Whether touch input is available. */
  touchInput(): boolean;

  /** Whether haptic feedback is available. */
  haptic(): boolean;

  /** Safe area insets (for notched displays / PWAs). */
  safeArea(): Insets;

  /** Whether the client window currently has focus. */
  hasFocus(): boolean;

  /** Whether clipboard API is available. */
  clipboardAvailable(): boolean;

  /** Whether a screen reader is active. */
  screenReaderActive(): boolean;
}

// =============================================================================
// RenderSurface (placeholder for #651)
// =============================================================================

/**
 * Surface for rendering styled text.
 *
 * Mirrors the Rust `RenderSurface` trait. This is a placeholder interface
 * defined here for type-level use. Implementation is tracked in #651.
 */
export interface RenderSurface {
  /** Write styled text at a position. */
  writeStyled(x: number, y: number, text: string, style: Style): void;

  /** Apply a style to a rectangular region. */
  applyStyle(rect: Rect, style: Style): void;

  /** Overlay a background color on a region. */
  overlayBg(rect: Rect, color: string): void;

  /** Fill a region with a character and style. */
  fill(rect: Rect, ch: string, style: Style): void;

  /** Clear the entire surface. */
  clear(): void;

  /** Get the surface dimensions. */
  size(): { width: number; height: number };
}

// =============================================================================
// ServerHandle
// =============================================================================

/**
 * Handle for querying server state and executing commands.
 *
 * Mirrors the Rust `ServerHandle` trait. Modules use this to interact
 * with the server without knowing the transport layer.
 */
export interface ServerHandle {
  /** Get option values by name. Returns a map of name -> value. */
  getOptions(names: string[]): Promise<Map<string, OptionValue>>;

  /** Execute a command by name. */
  executeCommand(command: string): Promise<void>;

  /** List available commands (empty if server does not support it). */
  listCommands(): Promise<string[]>;

  /** Get metadata for an option (null if not available). */
  getOptionMetadata(name: string): Promise<OptionMetadata | null>;
}

// =============================================================================
// ClmThemeProvider
// =============================================================================

/**
 * Theme style provider for CLM modules.
 *
 * Mirrors the Rust `ThemeProvider` trait (5 methods). This is separate
 * from the existing web `ThemeProvider` at `theme/provider.ts` -- an
 * adapter bridges the two.
 */
export interface ClmThemeProvider {
  /** Get the style for a highlight group. */
  highlight(group: string): Style | null;

  /** Get the first matching style from a list of groups. */
  highlightWithFallback(groups: string[]): Style | null;

  /** Get the foreground (default text) style. */
  foreground(): Style;

  /** Get the background style. */
  background(): Style;

  /** Whether the current theme is dark. */
  isDark(): boolean;
}

// =============================================================================
// ClientModuleRegistry
// =============================================================================

/**
 * Read-only view of loaded modules for cross-module introspection.
 *
 * Mirrors the Rust `ClientModuleRegistry` trait.
 */
export interface ClientModuleRegistry {
  /** Whether a module with the given kind is running. */
  isRunning(kind: string): boolean;

  /** Get the lifecycle state of a module by kind. */
  moduleState(kind: string): ClientModuleState | null;

  /** Get the kinds of all loaded modules. */
  loadedKinds(): string[];

  /** Get the count of running modules. */
  runningCount(): number;
}

// =============================================================================
// ClientServiceRegistry
// =============================================================================

/**
 * Cross-module service registry.
 *
 * TypeScript equivalent of Rust `ClientServiceRegistry`. Uses string keys
 * instead of TypeId (TypeId is not available in TypeScript).
 */
export interface ClientServiceRegistry {
  /** Register a service by key. Overwrites any existing service. */
  register<T>(key: string, service: T): void;

  /** Get a service by key. Returns undefined if not registered. */
  get<T>(key: string): T | undefined;

  /** Check if a service is registered. */
  contains(key: string): boolean;

  /** Number of registered services. */
  readonly size: number;
}

// =============================================================================
// ModuleContext
// =============================================================================

/**
 * Context passed to modules during initialization and events.
 *
 * Mirrors the Rust `ModuleContext` struct.
 */
export interface ModuleContext {
  /** Platform capability queries. */
  readonly capabilities: PlatformCapabilities;

  /** Server interaction handle. */
  readonly server: ServerHandle;

  /** Theme style provider. */
  readonly theme: ClmThemeProvider;

  /** Cross-module service registry (optional for backward compat). */
  readonly services?: ClientServiceRegistry;

  /** Module registry for introspection (optional for backward compat). */
  readonly moduleRegistry?: ClientModuleRegistry;
}

// =============================================================================
// ClientModule
// =============================================================================

/**
 * Client-side module interface.
 *
 * Mirrors the Rust `ClientModule` trait. This is the primary interface
 * that all client modules must implement. Existing `WebExtension`
 * implementations are wrapped via `WebExtensionAdapter`.
 *
 * Method groups:
 * - **Identity**: id, kind, name, version
 * - **Lifecycle**: init, exit, onAllLoaded
 * - **Roles**: hasChrome, hasBufferContrib, hasAnnotations
 * - **Events** (all optional): notification, mode, cursor, buffer, options, etc.
 * - **Chrome** (all optional): position, size, priority, z-order, render
 * - **Dependencies**: dependencies, optionalDependencies, serverKinds
 * - **Test support**: getState
 */
export interface ClientModule {
  // ---- Identity ----

  /** Unique module identifier. */
  id(): string;

  /** Module kind (e.g., "cmdline", "whichkey"). */
  kind(): string;

  /** Human-readable module name. */
  name(): string;

  /** Module version. */
  version(): Version;

  // ---- Lifecycle ----

  /** Initialize the module. Returns a probe result. */
  init(ctx: ModuleContext): ProbeResult;

  /** Shutdown the module. */
  exit(): void;

  /** Called after all modules have been initialized. */
  onAllLoaded?(ctx: ModuleContext): void;

  // ---- Roles ----

  /** Whether this module provides chrome UI. */
  hasChrome(): boolean;

  /** Whether this module contributes to buffer rendering. */
  hasBufferContrib(): boolean;

  /** Whether this module provides annotations. */
  hasAnnotations(): boolean;

  // ---- Events (all optional) ----

  /** Handle a server notification for this module's kind. */
  onNotification?(data: string): void;

  /** Handle an option change. */
  onOptionChanged?(name: string, value: OptionValue): void;

  /** Handle a buffer content update. */
  onBufferUpdate?(bufferId: number, startLine: number, endLine: number): void;

  /** Handle cursor movement. */
  onCursorUpdate?(line: number, col: number): void;

  /** Handle buffer focus change. */
  onBufferFocus?(bufferId: number): void;

  /** Handle mode change. */
  onModeChange?(mode: string): void;

  /** Handle platform capabilities change. */
  onCapabilitiesChanged?(capabilities: PlatformCapabilities): void;

  /** Handle theme change. */
  onThemeChanged?(theme: ClmThemeProvider): void;

  /** Periodic tick (for animations, timeouts, etc.). */
  tick?(deltaMs: number): void;

  // ---- Chrome (all optional) ----

  /** Chrome panel position. */
  chromePosition?(): ChromePosition;

  /** Requested size for chrome panel. */
  chromeRequestedSize?(): ColumnWidth | number;

  /** Chrome rendering priority (lower = rendered first). */
  chromePriority?(): number;

  /** Chrome z-order for overlays (higher = on top). */
  chromeZOrder?(): number;

  /** Render chrome into a surface. */
  chromeRender?(surface: RenderSurface, rect: Rect): void;

  // ---- Dependencies ----

  /** Required module kinds that must be loaded before this module. */
  dependencies?(): string[];

  /** Optional module kinds (loaded if available, not required). */
  optionalDependencies?(): string[];

  /** Server extension kinds this module expects. */
  serverKinds?(): string[];

  // ---- Test support ----

  /** Return internal state for headless testing. */
  getState?(): Record<string, unknown> | null;
}
