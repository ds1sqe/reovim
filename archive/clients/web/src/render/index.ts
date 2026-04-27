// Rendering layer exports.
//
// This module provides DOM rendering utilities for the web client,
// converting WASM-computed layouts into styled HTML elements.

export { LayoutRenderer, type LayoutRendererOptions } from "./layout.js";
export {
  BufferRenderer,
  type BufferRendererOptions,
  type SelectionRange,
  createSelectionRange,
} from "./buffer.js";
