/**
 * Core CLM module — re-exports all types, contracts, and implementations.
 *
 * @module core
 */

// Value types
export type {
  Version,
  ProbeResult,
  ClientModuleError,
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
  BufferId,
} from "./types.js";

export {
  formatVersion,
  probeSuccess,
  probeDefer,
  probeFailed,
  initFailed,
  exitFailed,
  notificationParse,
  otherError,
  rect,
  INSETS_ZERO,
  defaultStyle,
} from "./types.js";

// Contracts (interfaces)
export type {
  PlatformCapabilities,
  RenderSurface,
  ServerHandle,
  ClmThemeProvider,
  ClientModuleRegistry,
  ClientServiceRegistry,
  ModuleContext,
  ClientModule,
} from "./contracts.js";

// Implementations
export { BrowserPlatformAdapter } from "./platform-adapter.js";
export type { PlatformOverrides } from "./platform-adapter.js";
export { GrpcServerHandle } from "./server-handle.js";
export { WebClientServiceRegistry } from "./service-registry.js";
export { ThemeProviderAdapter, resolvedStyleToClmStyle } from "./theme-adapter.js";
export {
  WebExtensionAdapter,
  adaptExtension,
  adaptExtensions,
} from "./extension-adapter.js";
export { ClientModuleLoader } from "./loader.js";
export { ChromeCompositor } from "./compositor.js";
export type { ChromeRegion, CompositorLayout } from "./compositor.js";
export { DomChromeSurface, styleToCss } from "./dom-surface.js";
export { ChromeDispatcher } from "./chrome-dispatcher.js";
