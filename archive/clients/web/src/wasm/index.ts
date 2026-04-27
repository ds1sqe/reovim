// WASM module exports.
//
// This is the main entry point for the WASM bridge layer.
// Re-exports types, bindings, helpers, and conversion utilities.

// Core WASM bindings and types
export {
  // Initialization
  initWasm,
  isWasmReady,

  // Layout functions
  interpretLayout,
  layoutSingle,
  layoutHsplit,
  layoutVsplit,
  layoutIsLeaf,
  layoutWindowCount,

  // Geometry functions
  rectContains,

  // Direction functions
  directionOpposite,
  directionIsHorizontal,
  directionIsVertical,

  // Split direction functions
  splitIsHorizontal,
  splitIsVertical,
  splitOpposite,
} from "./bindings.js";

// Re-export all types
export type {
  ScreenPosition,
  Size,
  Rect,
  Direction,
  SplitDirection,
  LogicalLayout,
  WindowTree,
  Window,
  LogicalOverlay,
  RenderedOverlay,
  OverlayState,
  OverlayStack,
  ViewportState,
  ViewportUpdate,
  ClientPresence,
  SyncMode,
  PanelState,
  SemanticOrigin,
  ExtensionCategory,
  Interaction,
  InteractionResult,
  LayoutSyncMode,
  OverlaySyncMode,
  PresenceTracker,
} from "./bindings.js";

// Tree traversal helpers
export {
  // Type guards
  isLeaf,
  isSplit,
  isTabs,

  // Traversal utilities
  allWindows,
  findWindow,
  findWindowByViewport,
  findWindowByBuffer,
  focusedWindow,
  windowCount,
  getBounds,
  windowAtPosition,
  mapWindows,
} from "./helpers.js";

// Proto conversion utilities
export {
  protoToLogicalLayout,
  layoutNotificationToLogical,
  isProtoLeaf,
  isProtoSplit,
  getProtoBufferId,
  getProtoWindowId,
  getAllProtoWindowIds,
  countProtoWindows,
} from "./convert.js";
