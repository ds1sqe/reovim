/**
 * CLM Value Types
 *
 * TypeScript equivalents of the Rust CLM types from
 * `clients/lib/driver/src/types/mod.rs`.
 *
 * These are pure value types with no behavior beyond factory functions.
 *
 * @module core/types
 */

// =============================================================================
// Version
// =============================================================================

/** Semantic version triple. */
export interface Version {
  readonly major: number;
  readonly minor: number;
  readonly patch: number;
}

/** Format a version as "major.minor.patch". */
export function formatVersion(v: Version): string {
  return `${v.major}.${v.minor}.${v.patch}`;
}

// =============================================================================
// ProbeResult
// =============================================================================

/**
 * Result of a module's `init()` probe.
 *
 * Discriminated union matching the Rust `ProbeResult` enum:
 * - `success`: Module initialized successfully.
 * - `defer`: Module wants to retry later (dependency not ready yet).
 * - `failed`: Module cannot initialize (permanent failure).
 */
export type ProbeResult =
  | { readonly status: "success" }
  | { readonly status: "defer"; readonly reason: string }
  | { readonly status: "failed"; readonly error: string };

/** Factory: successful probe. */
export function probeSuccess(): ProbeResult {
  return { status: "success" };
}

/** Factory: deferred probe. */
export function probeDefer(reason: string): ProbeResult {
  return { status: "defer", reason };
}

/** Factory: failed probe. */
export function probeFailed(error: string): ProbeResult {
  return { status: "failed", error };
}

// =============================================================================
// ClientModuleError
// =============================================================================

/**
 * Error type for client module operations.
 *
 * Mirrors the Rust `ClientModuleError` enum variants.
 */
export interface ClientModuleError {
  readonly message: string;
  readonly source?: Error;
}

/** Factory: init failed error. */
export function initFailed(message: string, source?: Error): ClientModuleError {
  return { message: `init failed: ${message}`, source };
}

/** Factory: exit failed error. */
export function exitFailed(message: string, source?: Error): ClientModuleError {
  return { message: `exit failed: ${message}`, source };
}

/** Factory: notification parse error. */
export function notificationParse(
  message: string,
  source?: Error,
): ClientModuleError {
  return { message: `notification parse: ${message}`, source };
}

/** Factory: other error. */
export function otherError(
  message: string,
  source?: Error,
): ClientModuleError {
  return { message, source };
}

// =============================================================================
// ClientModuleState
// =============================================================================

/**
 * Module lifecycle state.
 *
 * - `"loaded"` - Registered but not yet initialized.
 * - `"initializing"` - Currently in init().
 * - `"running"` - Successfully initialized.
 * - `{ failed: string }` - Init permanently failed.
 */
export type ClientModuleState =
  | "loaded"
  | "initializing"
  | "running"
  | { readonly failed: string };

// =============================================================================
// ChromePosition
// =============================================================================

/**
 * Position of a chrome panel relative to the editor viewport.
 *
 * Matches Rust `ChromePosition` enum.
 */
export type ChromePosition = "top" | "bottom" | "left" | "right" | "overlay";

// =============================================================================
// ColumnWidth
// =============================================================================

/**
 * Width hint for chrome columns (left/right panels).
 *
 * - `fixed`: Exact column count.
 * - `dynamic`: Preferred width, may shrink.
 */
export type ColumnWidth =
  | { readonly kind: "fixed"; readonly width: number }
  | { readonly kind: "dynamic"; readonly width: number };

// =============================================================================
// RenderingModel
// =============================================================================

/**
 * How the client renders content.
 *
 * - `CellGrid`: Terminal-style fixed cell grid (TUI).
 * - `Canvas`: Pixel-based canvas (Web).
 * - `NativeLayout`: Platform-native text layout.
 */
export type RenderingModel = "CellGrid" | "Canvas" | "NativeLayout";

// =============================================================================
// ColorDepth
// =============================================================================

/** Terminal/display color depth. */
export type ColorDepth = "Monochrome" | "Ansi16" | "Ansi256" | "TrueColor";

// =============================================================================
// Rect
// =============================================================================

/** Axis-aligned rectangle (x, y are top-left corner). */
export interface Rect {
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
}

/** Create a Rect. */
export function rect(
  x: number,
  y: number,
  width: number,
  height: number,
): Rect {
  return { x, y, width, height };
}

// =============================================================================
// Insets
// =============================================================================

/** Edge insets (padding/margin). */
export interface Insets {
  readonly top: number;
  readonly bottom: number;
  readonly left: number;
  readonly right: number;
}

/** Zero insets constant. */
export const INSETS_ZERO: Insets = Object.freeze({
  top: 0,
  bottom: 0,
  left: 0,
  right: 0,
});

// =============================================================================
// Style
// =============================================================================

/**
 * Inline style for styled text rendering.
 *
 * Colors are optional CSS color strings (hex, named, etc.).
 * Boolean attributes default to false when undefined.
 */
export interface Style {
  readonly fg?: string;
  readonly bg?: string;
  readonly bold?: boolean;
  readonly italic?: boolean;
  readonly underline?: boolean;
  readonly strikethrough?: boolean;
  readonly dim?: boolean;
  readonly reverse?: boolean;
}

/** Create a default (empty) style. */
export function defaultStyle(): Style {
  return {};
}

// =============================================================================
// OptionValue
// =============================================================================

/**
 * Editor option value (discriminated union).
 *
 * Matches Rust `OptionValue` enum.
 */
export type OptionValue =
  | { readonly kind: "bool"; readonly value: boolean }
  | { readonly kind: "integer"; readonly value: number }
  | { readonly kind: "string"; readonly value: string };

// =============================================================================
// OptionMetadata
// =============================================================================

/** Metadata describing an editor option. */
export interface OptionMetadata {
  readonly name: string;
  readonly description: string;
  readonly optionKind: "bool" | "integer" | "string";
  readonly defaultValue: OptionValue;
}

// =============================================================================
// BufferId
// =============================================================================

/** Buffer identifier (numeric). */
export type BufferId = number;
