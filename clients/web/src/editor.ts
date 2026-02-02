/**
 * Editor State and Rendering
 *
 * Manages editor state and renders to the DOM.
 * Uses WASM-based layout interpreter for multi-window support.
 */

import type { ReovimClient } from "./client.js";
import type { Notification } from "./gen/reovim/v2/notification_pb.js";
import type { WindowInfo } from "./gen/reovim/v2/notification_pb.js";

// WASM bindings and types
import {
  initWasm,
  isWasmReady,
  interpretLayout,
  protoToLogicalLayout,
  allWindows,
  type WindowTree,
  type LogicalLayout,
  type ScreenPosition,
} from "./wasm/index.js";

// Rendering layer
import { LayoutRenderer } from "./render/layout.js";
import { BufferRenderer, type SelectionRange } from "./render/buffer.js";

// Caching layer (Phase 11.1)
import { ViewportCache, BufferCache } from "./cache/index.js";

// Overlay rendering (Phase 11.1)
import { OverlayRenderer } from "./render/overlay.js";

// Capture handler (Phase 16)
import { CaptureHandler, type CaptureableState } from "./capture/index.js";

/** Position within the buffer */
interface Position {
  line: number;
  col: number;
}

/** Visual mode type */
type VisualMode = "char" | "line" | "block";

/** Per-window state */
interface WindowState {
  lines: string[];
  cursorLine: number;
  cursorCol: number;
  topLine: number;
  selection: SelectionRange | null;
}

interface EditorState {
  mode: string;
  modeDisplay: string;

  // Legacy single-window state (for backward compatibility)
  cursorLine: number;
  cursorCol: number;
  lines: string[];
  hasSelection: boolean;
  selectionAnchor: Position | null;
  selectionCursor: Position | null;
  visualMode: VisualMode | null;

  // Multi-window state (Phase 10.2)
  layout: WindowTree | null;
  focusedWindowId: number;
  windowStates: Map<number, WindowState>;

  // Flag to enable multi-window rendering
  useMultiWindow: boolean;
}

/**
 * Editor class manages state and DOM rendering.
 *
 * Supports both single-window (legacy) and multi-window (WASM) rendering modes.
 */
export class Editor {
  private client: ReovimClient;
  private state: EditorState;

  // Renderers
  private layoutRenderer: LayoutRenderer;
  private bufferRenderer: BufferRenderer;

  // Caches (Phase 11.1 - incremental updates)
  private viewportCache: ViewportCache;
  private bufferCache: BufferCache;

  // Overlay rendering (Phase 11.1)
  private overlayRenderer: OverlayRenderer;

  // Capture handler (Phase 16)
  private captureHandler: CaptureHandler;

  // DOM elements (legacy single-window)
  private modeElement: HTMLElement | null;
  private bufferElement: HTMLElement | null;
  private cursorElement: HTMLElement | null;
  private positionElement: HTMLElement | null;

  // DOM elements (multi-window)
  private editorElement: HTMLElement | null;

  constructor(client: ReovimClient) {
    this.client = client;
    this.state = {
      mode: "normal",
      modeDisplay: "NORMAL",
      cursorLine: 0,
      cursorCol: 0,
      lines: [""],
      hasSelection: false,
      selectionAnchor: null,
      selectionCursor: null,
      visualMode: null,
      // Multi-window state
      layout: null,
      focusedWindowId: 0,
      windowStates: new Map(),
      useMultiWindow: false,
    };

    // Initialize renderers
    this.layoutRenderer = new LayoutRenderer();
    this.bufferRenderer = new BufferRenderer();

    // Initialize caches (Phase 11.1)
    this.viewportCache = new ViewportCache();
    this.bufferCache = new BufferCache();

    // Initialize overlay renderer (Phase 11.1)
    this.overlayRenderer = new OverlayRenderer();

    // Initialize capture handler (Phase 16)
    this.captureHandler = new CaptureHandler({
      client: this.client,
      getState: () => this.getCaptureableState(),
    });

    // Cache DOM elements
    this.modeElement = document.getElementById("mode");
    this.bufferElement = document.getElementById("buffer");
    this.cursorElement = document.getElementById("cursor");
    this.positionElement = document.getElementById("position");
    this.editorElement = document.getElementById("editor");
  }

  /**
   * Initialize the editor, including WASM module.
   *
   * Call this before any other methods.
   */
  async init(): Promise<void> {
    try {
      // Initialize WASM module
      await initWasm();
      console.log("WASM module initialized");

      // Enable multi-window mode by default when WASM is ready
      this.state.useMultiWindow = true;
    } catch (error) {
      console.warn("WASM initialization failed, using single-window mode:", error);
      this.state.useMultiWindow = false;
    }

    // Set overlay container (Phase 11.1)
    if (this.editorElement) {
      this.overlayRenderer.setContainer(this.editorElement);
    }
  }

  /**
   * Refresh editor state from server.
   */
  async refresh(): Promise<void> {
    try {
      if (this.state.useMultiWindow && isWasmReady()) {
        await this.refreshMultiWindow();
      } else {
        await this.refreshSingleWindow();
      }
    } catch (error) {
      console.error("Failed to refresh editor state:", error);
    }
  }

  /**
   * Refresh for single-window mode (legacy).
   */
  private async refreshSingleWindow(): Promise<void> {
    const [modeResponse, cursorResponse, bufferResponse] = await Promise.all([
      this.client.state.getMode({}),
      this.client.state.getCursor({}),
      this.client.buffer.getRawContent({}),
    ]);

    this.state.mode = modeResponse.name.toLowerCase();
    this.state.modeDisplay = modeResponse.display;
    this.state.cursorLine = Number(cursorResponse.position?.line ?? 0);
    this.state.cursorCol = Number(cursorResponse.position?.column ?? 0);
    this.state.lines = bufferResponse.lines;

    this.renderSingleWindow();
  }

  /**
   * Refresh for multi-window mode (WASM-based).
   */
  private async refreshMultiWindow(): Promise<void> {
    const [modeResponse, layoutResponse] = await Promise.all([
      this.client.state.getMode({}),
      this.client.state.getLayout({}),
    ]);

    // Update mode
    this.state.mode = modeResponse.name.toLowerCase();
    this.state.modeDisplay = modeResponse.display;

    // Convert proto to logical layout and interpret
    if (layoutResponse.root) {
      const logical = protoToLogicalLayout(layoutResponse.root);
      if (logical) {
        this.updateLayout(logical, Number(layoutResponse.focusedWindowId));
      }
    }

    // Fetch buffer content for each window
    await this.refreshWindowBuffers();

    // Render
    this.renderMultiWindow();
  }

  /**
   * Update layout from a logical layout.
   */
  private updateLayout(logical: LogicalLayout, focusedId: number): void {
    if (!this.editorElement) return;

    const screen = this.layoutRenderer.getScreenSize(this.editorElement);
    this.state.layout = interpretLayout(logical, screen.width, screen.height);
    this.state.focusedWindowId = focusedId;
  }

  /**
   * Refresh buffer content for all windows.
   *
   * Phase 11.1: Updates both window states and buffer cache.
   */
  private async refreshWindowBuffers(): Promise<void> {
    if (!this.state.layout) return;

    const windows = allWindows(this.state.layout);

    await Promise.all(
      windows.map(async (window) => {
        try {
          const [bufferResponse, cursorResponse] = await Promise.all([
            this.client.buffer.getRawContent({ bufferId: BigInt(window.buffer_id) }),
            this.client.state.getCursor({ windowId: BigInt(window.id) }),
          ]);

          // Update buffer cache
          this.bufferCache.set(window.buffer_id, bufferResponse.lines, Date.now());

          const existing = this.state.windowStates.get(window.id) || {
            lines: [],
            cursorLine: 0,
            cursorCol: 0,
            topLine: 0,
            selection: null,
          };

          this.state.windowStates.set(window.id, {
            ...existing,
            lines: bufferResponse.lines,
            cursorLine: Number(cursorResponse.position?.line ?? 0),
            cursorCol: Number(cursorResponse.position?.column ?? 0),
          });
        } catch (error) {
          console.warn(`Failed to fetch content for window ${window.id}:`, error);
        }
      })
    );
  }

  /**
   * Subscribe to server notifications for real-time updates.
   */
  async subscribeToNotifications(): Promise<void> {
    try {
      const stream = this.client.notification.subscribe({});

      for await (const notification of stream) {
        this.handleNotification(notification);
      }
    } catch (error) {
      console.error("Notification stream error:", error);
    }
  }

  /**
   * Handle incoming notification from server.
   */
  private handleNotification(notification: Notification): void {
    const { payload } = notification;

    switch (payload.case) {
      case "modeChanged": {
        const { name, display } = payload.value;
        this.state.mode = (name || "normal").toLowerCase();
        this.state.modeDisplay = display || "NORMAL";
        this.renderMode();
        break;
      }

      case "cursorMoved": {
        const { position, windowId } = payload.value;
        const line = Number(position?.line ?? 0n);
        const col = Number(position?.column ?? 0n);
        const winId = Number(windowId ?? 0n);

        // Update legacy state
        this.state.cursorLine = line;
        this.state.cursorCol = col;

        // Update per-window state
        if (this.state.useMultiWindow) {
          const windowState = this.state.windowStates.get(winId);
          if (windowState) {
            windowState.cursorLine = line;
            windowState.cursorCol = col;
          }
        }

        if (this.state.useMultiWindow) {
          this.renderMultiWindow();
        } else {
          this.renderCursor();
          this.renderPosition();
        }
        break;
      }

      case "bufferModified": {
        const { bufferId } = payload.value;
        const bufId = Number(bufferId ?? 0n);

        // Invalidate cache - next render will fetch fresh content
        this.bufferCache.invalidate(bufId);

        if (this.state.useMultiWindow) {
          // Find windows showing this buffer and refresh them
          this.refreshBufferForBuffer(bufId);
        } else {
          this.refreshBuffer();
        }
        break;
      }

      case "selectionChanged": {
        const { hasSelection, selection, visualMode, windowId } = payload.value;
        const winId = Number(windowId ?? 0n);

        // Update legacy state
        this.state.hasSelection = hasSelection ?? false;

        if (hasSelection && selection) {
          this.state.selectionAnchor = {
            line: Number(selection.start?.line ?? 0n),
            col: Number(selection.start?.column ?? 0n),
          };
          this.state.selectionCursor = {
            line: Number(selection.end?.line ?? 0n),
            col: Number(selection.end?.column ?? 0n),
          };
          this.state.visualMode = (visualMode as VisualMode) ?? null;

          // Update per-window state
          if (this.state.useMultiWindow) {
            const windowState = this.state.windowStates.get(winId);
            if (windowState) {
              windowState.selection = {
                anchor: {
                  x: Number(selection.start?.column ?? 0n),
                  y: Number(selection.start?.line ?? 0n),
                },
                cursor: {
                  x: Number(selection.end?.column ?? 0n),
                  y: Number(selection.end?.line ?? 0n),
                },
                mode: (visualMode as "char" | "line" | "block") ?? "char",
              };
            }
          }
        } else {
          this.state.selectionAnchor = null;
          this.state.selectionCursor = null;
          this.state.visualMode = null;

          // Clear per-window selection
          if (this.state.useMultiWindow) {
            const windowState = this.state.windowStates.get(winId);
            if (windowState) {
              windowState.selection = null;
            }
          }
        }

        if (this.state.useMultiWindow) {
          this.renderMultiWindow();
        } else {
          this.renderBuffer();
        }
        break;
      }

      case "layoutChanged": {
        const { focusedWindowId, windows } = payload.value;

        if (this.state.useMultiWindow) {
          // Phase 11.1: Only fetch layout tree, use cached buffer content
          this.refreshLayoutOnly(Number(focusedWindowId), windows);
        }
        break;
      }

      case "viewportUpdated": {
        // Phase 11.1: Incremental viewport updates without full re-fetch
        const update = payload.value;
        const viewportId = Number(update.viewportId ?? 0n);

        // Apply incremental update to viewport cache
        this.viewportCache.applyUpdate({
          viewport_id: viewportId,
          top_line: update.topLine !== undefined ? Number(update.topLine) : undefined,
          left_col: update.leftCol !== undefined ? Number(update.leftCol) : undefined,
          cursor_line: update.cursorLine !== undefined ? Number(update.cursorLine) : undefined,
          cursor_col: update.cursorCol !== undefined ? Number(update.cursorCol) : undefined,
        });

        // Update window state if in multi-window mode
        if (this.state.useMultiWindow) {
          const windowState = this.state.windowStates.get(viewportId);
          if (windowState) {
            if (update.topLine !== undefined) {
              windowState.topLine = Number(update.topLine);
            }
            if (update.cursorLine !== undefined) {
              windowState.cursorLine = Number(update.cursorLine);
            }
            if (update.cursorCol !== undefined) {
              windowState.cursorCol = Number(update.cursorCol);
            }
          }
          // Only re-render the affected viewport/window
          this.renderMultiWindow();
        }
        break;
      }

      case "captureRequest": {
        // Phase 16: Handle capture request from server (e.g., from CLI)
        this.captureHandler.handleCaptureRequest(payload.value);
        break;
      }

      default:
        break;
    }
  }

  /**
   * Refresh layout only - uses cached buffer content when available.
   *
   * Phase 11.1: Efficient layout refresh that:
   * 1. Fetches layout tree from server
   * 2. Uses cached buffer content for existing windows
   * 3. Only fetches content for NEW windows (not in cache)
   */
  private async refreshLayoutOnly(
    focusedId: number,
    _windowInfos: WindowInfo[]
  ): Promise<void> {
    try {
      const layoutResponse = await this.client.state.getLayout({});
      if (!layoutResponse.root) return;

      const logical = protoToLogicalLayout(layoutResponse.root);
      if (!logical) return;

      this.updateLayout(logical, focusedId);

      // Get list of windows that need buffer content
      const windows = allWindows(this.state.layout!);
      const windowsNeedingContent: Array<{ windowId: number; bufferId: number }> = [];

      for (const window of windows) {
        // Check if we have cached content for this buffer
        if (!this.bufferCache.has(window.buffer_id)) {
          windowsNeedingContent.push({
            windowId: window.id,
            bufferId: window.buffer_id,
          });
        } else {
          // Use cached content
          const cachedLines = this.bufferCache.get(window.buffer_id);
          const existing = this.state.windowStates.get(window.id) || {
            lines: [],
            cursorLine: 0,
            cursorCol: 0,
            topLine: 0,
            selection: null,
          };
          this.state.windowStates.set(window.id, {
            ...existing,
            lines: cachedLines || [],
          });
        }
      }

      // Fetch content only for windows without cached data
      if (windowsNeedingContent.length > 0) {
        await Promise.all(
          windowsNeedingContent.map(async ({ windowId, bufferId }) => {
            try {
              const [bufferResponse, cursorResponse] = await Promise.all([
                this.client.buffer.getRawContent({ bufferId: BigInt(bufferId) }),
                this.client.state.getCursor({ windowId: BigInt(windowId) }),
              ]);

              // Cache the buffer content
              this.bufferCache.set(bufferId, bufferResponse.lines, 1);

              const existing = this.state.windowStates.get(windowId) || {
                lines: [],
                cursorLine: 0,
                cursorCol: 0,
                topLine: 0,
                selection: null,
              };

              this.state.windowStates.set(windowId, {
                ...existing,
                lines: bufferResponse.lines,
                cursorLine: Number(cursorResponse.position?.line ?? 0),
                cursorCol: Number(cursorResponse.position?.column ?? 0),
              });
            } catch (error) {
              console.warn(`Failed to fetch content for window ${windowId}:`, error);
            }
          })
        );
      }

      this.renderMultiWindow();
    } catch (error) {
      console.error("Failed to refresh layout:", error);
    }
  }

  /**
   * Refresh buffer content for windows showing a specific buffer.
   *
   * Phase 11.1: Fetches once and updates both cache and window states.
   */
  private async refreshBufferForBuffer(bufferId: number): Promise<void> {
    if (!this.state.layout) return;

    const windows = allWindows(this.state.layout);
    const affectedWindows = windows.filter((w) => w.buffer_id === bufferId);

    if (affectedWindows.length === 0) return;

    try {
      // Fetch buffer content once (not per-window)
      const bufferResponse = await this.client.buffer.getRawContent({
        bufferId: BigInt(bufferId),
      });

      // Update cache
      this.bufferCache.set(bufferId, bufferResponse.lines, Date.now());

      // Update all affected window states
      for (const window of affectedWindows) {
        const windowState = this.state.windowStates.get(window.id);
        if (windowState) {
          windowState.lines = bufferResponse.lines;
        }
      }
    } catch (error) {
      console.warn(`Failed to refresh buffer ${bufferId}:`, error);
    }

    this.renderMultiWindow();
  }

  /**
   * Refresh only buffer content (legacy single-window).
   */
  private async refreshBuffer(): Promise<void> {
    try {
      const response = await this.client.buffer.getRawContent({});
      this.state.lines = response.lines;
      this.renderBuffer();
    } catch (error) {
      console.error("Failed to refresh buffer:", error);
    }
  }

  // ============ Rendering Methods ============

  /**
   * Render in multi-window mode using WASM layout.
   */
  private renderMultiWindow(): void {
    if (!this.editorElement || !this.state.layout) {
      // Fall back to single-window rendering
      this.renderSingleWindow();
      return;
    }

    // Render layout structure
    this.layoutRenderer.render(
      this.state.layout,
      this.editorElement,
      this.state.focusedWindowId
    );

    // Render buffer content for each window
    for (const window of allWindows(this.state.layout)) {
      const windowEl = this.layoutRenderer.getWindowElement(window.id);
      if (!windowEl) continue;

      const windowState = this.state.windowStates.get(window.id);
      if (!windowState) continue;

      const cursor: ScreenPosition = {
        x: windowState.cursorCol,
        y: windowState.cursorLine,
      };

      this.bufferRenderer.render(
        windowState.lines,
        windowEl,
        windowState.selection,
        cursor,
        windowState.topLine
      );
    }

    // Also render mode indicator
    this.renderMode();
    this.renderPosition();
  }

  /**
   * Render in single-window mode (legacy).
   */
  private renderSingleWindow(): void {
    this.renderMode();
    this.renderBuffer();
    this.renderCursor();
    this.renderPosition();
  }

  /**
   * Render mode indicator.
   */
  private renderMode(): void {
    if (!this.modeElement) return;

    this.modeElement.textContent = this.state.modeDisplay;
    this.modeElement.className = this.state.mode;
  }

  /**
   * Render buffer content with optional selection highlighting.
   */
  private renderBuffer(): void {
    if (!this.bufferElement) return;

    this.bufferElement.innerHTML = "";

    this.state.lines.forEach((lineContent, lineIndex) => {
      const lineDiv = document.createElement("div");
      lineDiv.className = "line";

      const lineNumber = document.createElement("span");
      lineNumber.className = "line-number";
      lineNumber.textContent = String(lineIndex + 1);

      const content = document.createElement("span");
      content.className = "line-content";

      if (this.state.hasSelection && lineContent.length > 0) {
        this.renderLineWithSelection(content, lineContent, lineIndex);
      } else if (
        this.state.hasSelection &&
        lineContent.length === 0 &&
        this.state.visualMode === "line" &&
        this.isLineSelected(lineIndex)
      ) {
        content.innerHTML = '<span class="selected">\u00A0</span>';
      } else {
        content.textContent = lineContent || "\u00A0";
      }

      lineDiv.appendChild(lineNumber);
      lineDiv.appendChild(content);
      this.bufferElement!.appendChild(lineDiv);
    });
  }

  /**
   * Render a line with selection highlighting.
   */
  private renderLineWithSelection(
    container: HTMLElement,
    text: string,
    lineIndex: number
  ): void {
    let currentSpan: HTMLSpanElement | null = null;
    let currentSelected = false;

    for (let col = 0; col < text.length; col++) {
      const char = text[col];
      const isSelected = this.isPositionSelected(lineIndex, col);

      if (currentSpan === null || isSelected !== currentSelected) {
        currentSpan = document.createElement("span");
        if (isSelected) {
          currentSpan.className = "selected";
        }
        container.appendChild(currentSpan);
        currentSelected = isSelected;
      }

      currentSpan.textContent += char;
    }

    if (text.length === 0) {
      container.textContent = "\u00A0";
    }
  }

  /**
   * Render cursor position.
   */
  private renderCursor(): void {
    if (!this.cursorElement || !this.bufferElement) return;

    const charWidth = 8.4;
    const lineHeight = 21;
    const lineNumberWidth = 48;
    const padding = 8;

    const x = padding + lineNumberWidth + this.state.cursorCol * charWidth;
    const y = padding + this.state.cursorLine * lineHeight;

    this.cursorElement.style.left = `${x}px`;
    this.cursorElement.style.top = `${y}px`;

    if (this.state.mode === "insert") {
      this.cursorElement.classList.add("insert");
      this.cursorElement.classList.remove("visual");
    } else if (this.state.mode.startsWith("visual")) {
      this.cursorElement.classList.add("visual");
      this.cursorElement.classList.remove("insert");
    } else {
      this.cursorElement.classList.remove("insert", "visual");
    }
  }

  /**
   * Render cursor position indicator.
   */
  private renderPosition(): void {
    if (!this.positionElement) return;

    // In multi-window mode, show focused window position
    if (this.state.useMultiWindow && this.state.focusedWindowId) {
      const windowState = this.state.windowStates.get(this.state.focusedWindowId);
      if (windowState) {
        this.positionElement.textContent = `${windowState.cursorLine + 1}:${windowState.cursorCol + 1}`;
        return;
      }
    }

    this.positionElement.textContent = `${this.state.cursorLine + 1}:${this.state.cursorCol + 1}`;
  }

  /**
   * Get current mode.
   */
  getMode(): string {
    return this.state.mode;
  }

  /**
   * Check if multi-window mode is enabled.
   */
  isMultiWindowEnabled(): boolean {
    return this.state.useMultiWindow && isWasmReady();
  }

  /**
   * Check if a position is within the current selection.
   */
  private isPositionSelected(line: number, col: number): boolean {
    if (
      !this.state.hasSelection ||
      !this.state.selectionAnchor ||
      !this.state.selectionCursor
    ) {
      return false;
    }

    const anchor = this.state.selectionAnchor;
    const cursor = this.state.selectionCursor;

    const isForward =
      anchor.line < cursor.line ||
      (anchor.line === cursor.line && anchor.col <= cursor.col);
    const start = isForward ? anchor : cursor;
    const end = isForward ? cursor : anchor;

    switch (this.state.visualMode) {
      case "char": {
        if (line < start.line || line > end.line) return false;

        if (start.line === end.line) {
          return col >= start.col && col <= end.col;
        }

        if (line === start.line) return col >= start.col;
        if (line === end.line) return col <= end.col;

        return true;
      }

      case "line": {
        return line >= start.line && line <= end.line;
      }

      case "block": {
        const minCol = Math.min(anchor.col, cursor.col);
        const maxCol = Math.max(anchor.col, cursor.col);
        return (
          line >= start.line &&
          line <= end.line &&
          col >= minCol &&
          col <= maxCol
        );
      }

      default:
        return false;
    }
  }

  /**
   * Check if an entire line is within the selection range.
   */
  private isLineSelected(line: number): boolean {
    if (
      !this.state.hasSelection ||
      !this.state.selectionAnchor ||
      !this.state.selectionCursor
    ) {
      return false;
    }
    const startLine = Math.min(
      this.state.selectionAnchor.line,
      this.state.selectionCursor.line
    );
    const endLine = Math.max(
      this.state.selectionAnchor.line,
      this.state.selectionCursor.line
    );
    return line >= startLine && line <= endLine;
  }

  /**
   * Get current state in a format suitable for frame capture.
   *
   * Phase 16: Used by CaptureHandler to generate frame captures.
   */
  private getCaptureableState(): CaptureableState {
    // Get viewport dimensions from editor element or use defaults
    const width = this.editorElement?.clientWidth
      ? Math.floor(this.editorElement.clientWidth / 8) // Approximate char width
      : 80;
    const height = this.editorElement?.clientHeight
      ? Math.floor(this.editorElement.clientHeight / 16) // Approximate line height
      : 24;

    return {
      mode: this.state.mode,
      modeDisplay: this.state.modeDisplay,
      cursorLine: this.state.cursorLine,
      cursorCol: this.state.cursorCol,
      lines: this.state.lines,
      width,
      height,
      focusedWindowId: this.state.focusedWindowId,
      windowStates: this.state.windowStates,
    };
  }
}
