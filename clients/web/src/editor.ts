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

// Color palette for multi-client presence (Issue #474)
import { colorForClient, dimmedColorForClient } from "./palette.js";

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

/** Remote client state for multi-client awareness (Phase 14, #471) */
interface RemoteClient {
  clientId: bigint;
  displayName: string;
  cursorLine: number;
  cursorCol: number;
  bufferId: number;
  selection: { anchor: Position; cursor: Position; mode: VisualMode } | null;
}

/**
 * Editor class manages state and DOM rendering.
 *
 * Supports both single-window (legacy) and multi-window (WASM) rendering modes.
 */
export class Editor {
  private client: ReovimClient;
  /** This client's unique ID (Phase 11.2 - per-client state). */
  readonly myClientId: bigint;
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

  // Remote clients for multi-client awareness (Phase 14, #471)
  private remoteClients: Map<bigint, RemoteClient> = new Map();

  // DOM elements (legacy single-window)
  private modeElement: HTMLElement | null;
  private bufferElement: HTMLElement | null;
  private cursorElement: HTMLElement | null;
  private positionElement: HTMLElement | null;

  // DOM elements (multi-window)
  private editorElement: HTMLElement | null;

  /**
   * Create a new Editor instance.
   *
   * @param client - gRPC client for server communication
   * @param myClientId - This client's unique ID (Phase 11.2)
   */
  constructor(client: ReovimClient, myClientId: bigint) {
    this.client = client;
    this.myClientId = myClientId;
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
   *
   * Phase 11.2: Handles empty layout by creating a default local view.
   * When server returns no windows, client creates its own view of the buffer.
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
    let hasLayout = false;
    if (layoutResponse.root) {
      const logical = protoToLogicalLayout(layoutResponse.root);
      if (logical) {
        this.updateLayout(logical, Number(layoutResponse.focusedWindowId));
        hasLayout = true;
      }
    }

    // If no layout from server, create default local view (Phase 11.2)
    if (!hasLayout) {
      await this.createDefaultView();
      return;
    }

    // Fetch buffer content for each window
    await this.refreshWindowBuffers();

    // Render
    this.renderMultiWindow();
  }

  /**
   * Create a default local view when server has no windows.
   *
   * Phase 11.2: Client-side handling of empty layout.
   * Fetches active buffer and populates single-window state for rendering.
   */
  private async createDefaultView(): Promise<void> {
    try {
      // Fetch buffer content and cursor for single-window fallback
      const [cursorResponse, bufferResponse] = await Promise.all([
        this.client.state.getCursor({}),
        this.client.buffer.getRawContent({}),
      ]);

      // Populate legacy single-window state
      this.state.cursorLine = Number(cursorResponse.position?.line ?? 0);
      this.state.cursorCol = Number(cursorResponse.position?.column ?? 0);
      this.state.lines = bufferResponse.lines;

      // Clear multi-window state to trigger single-window fallback
      this.state.layout = null;
      this.state.focusedWindowId = 0;

      // Render in single-window mode
      this.renderSingleWindow();
    } catch (error) {
      console.error("Failed to create default view:", error);
    }
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
    console.log("[notifications] Starting subscription...");
    try {
      const stream = this.client.notification.subscribe({});
      console.log("[notifications] Stream established, waiting for notifications...");

      for await (const notification of stream) {
        console.log("[notifications] Received:", notification.eventType, notification.payload);
        this.handleNotification(notification);
      }
      console.log("[notifications] Stream ended normally");
    } catch (error) {
      console.error("[notifications] Stream error:", error);
    }
  }

  /**
   * Handle incoming notification from server.
   */
  private handleNotification(notification: Notification): void {
    const { payload } = notification;

    switch (payload.case) {
      case "modeChanged": {
        const { name, display, clientId } = payload.value;
        const notifClientId = clientId ?? 0n;

        // Phase 14 (#471): Filter by client_id for multi-client mode isolation
        const isLocalMode = notifClientId === this.myClientId;

        console.log(
          "[modeChanged] mode:",
          name,
          "display:",
          display,
          "client:",
          notifClientId,
          "local:",
          isLocalMode
        );

        if (isLocalMode) {
          // Local mode update
          this.state.mode = (name || "normal").toLowerCase();
          this.state.modeDisplay = display || "NORMAL";
          this.renderMode();
        } else {
          // Remote mode update - update tracking map
          const remote = this.remoteClients.get(notifClientId);
          if (remote) {
            remote.mode = display || "NORMAL";
          }
        }
        break;
      }

      case "cursorMoved": {
        const { position, windowId, clientId } = payload.value;
        const line = Number(position?.line ?? 0n);
        const col = Number(position?.column ?? 0n);
        const winId = Number(windowId ?? 0n);
        const notifClientId = clientId ?? 0n;

        // Phase 14 (#471): Filter by client_id for multi-client cursor isolation
        const isLocalCursor = notifClientId === this.myClientId;

        if (isLocalCursor) {
          // Local cursor update
          // Update legacy state
          this.state.cursorLine = line;
          this.state.cursorCol = col;

          // Update per-window state (only if we have a layout)
          if (this.state.useMultiWindow && this.state.layout) {
            const windowState = this.state.windowStates.get(winId);
            if (windowState) {
              windowState.cursorLine = line;
              windowState.cursorCol = col;
            }
            this.renderMultiWindow();
          } else {
            // Single-window fallback
            this.renderCursor();
            this.renderPosition();
          }
        } else {
          // Remote cursor update - update tracking map and re-render
          const remote = this.remoteClients.get(notifClientId);
          if (remote) {
            remote.cursorLine = line;
            remote.cursorCol = col;
          }
          this.renderRemoteCursors();
        }
        break;
      }

      case "bufferModified": {
        const { bufferId } = payload.value;
        const bufId = Number(bufferId ?? 0n);
        console.log("[bufferModified] Buffer", bufId, "modified, useMultiWindow:", this.state.useMultiWindow, "hasLayout:", !!this.state.layout);

        // Invalidate cache - next render will fetch fresh content
        this.bufferCache.invalidate(bufId);

        // Use multi-window refresh only if we have a layout, otherwise fall back to single-window
        if (this.state.useMultiWindow && this.state.layout) {
          // Find windows showing this buffer and refresh them
          this.refreshBufferForBuffer(bufId).catch(e => console.error("[bufferModified] refresh error:", e));
        } else {
          // Single-window fallback (also used when layout is null)
          this.refreshBuffer().catch(e => console.error("[bufferModified] refresh error:", e));
        }
        break;
      }

      case "selectionChanged": {
        const { hasSelection, selection, visualMode, windowId, clientId } =
          payload.value;
        const winId = Number(windowId ?? 0n);
        const notifClientId = clientId ?? 0n;

        // Phase 14 (#471): Filter by client_id for multi-client selection isolation
        const isLocalSelection = notifClientId === this.myClientId;

        if (isLocalSelection) {
          // Local selection update
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

            // Update per-window state (only if we have a layout)
            if (this.state.useMultiWindow && this.state.layout) {
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

            // Clear per-window selection (only if we have a layout)
            if (this.state.useMultiWindow && this.state.layout) {
              const windowState = this.state.windowStates.get(winId);
              if (windowState) {
                windowState.selection = null;
              }
            }
          }

          // Render with fallback to single-window if no layout
          if (this.state.useMultiWindow && this.state.layout) {
            this.renderMultiWindow();
          } else {
            this.renderBuffer();
          }
        } else {
          // Remote selection update - track in remoteClients map
          const remote = this.remoteClients.get(notifClientId);
          if (remote) {
            if (hasSelection && selection) {
              remote.selection = {
                anchor: {
                  line: Number(selection.start?.line ?? 0n),
                  col: Number(selection.start?.column ?? 0n),
                },
                cursor: {
                  line: Number(selection.end?.line ?? 0n),
                  col: Number(selection.end?.column ?? 0n),
                },
                mode: (visualMode as VisualMode) ?? "char",
              };
            } else {
              remote.selection = null;
            }
          }
          // TODO: Render remote selections (future enhancement)
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

        // Update window state if in multi-window mode with layout
        if (this.state.useMultiWindow && this.state.layout) {
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
        // Note: No single-window fallback needed here - viewport updates are
        // only meaningful in multi-window mode
        break;
      }

      case "captureRequest": {
        // Phase 16: Handle capture request from server (e.g., from CLI)
        this.captureHandler.handleCaptureRequest(payload.value);
        break;
      }

      case "presenceJoined": {
        // Phase 14 (#471): Track new remote client
        const { client } = payload.value;
        if (client && BigInt(client.clientId) !== this.myClientId) {
          this.remoteClients.set(BigInt(client.clientId), {
            clientId: BigInt(client.clientId),
            displayName: client.displayName ?? "unknown",
            cursorLine: 0,
            cursorCol: 0,
            bufferId: Number(client.bufferId ?? 0n),
            selection: null,
          });
          this.renderRemoteCursors();
        }
        break;
      }

      case "presenceUpdated": {
        // Phase 14 (#471): Update remote client state
        const { client } = payload.value;
        if (client && BigInt(client.clientId) !== this.myClientId) {
          const remote = this.remoteClients.get(BigInt(client.clientId));
          if (remote) {
            remote.bufferId = Number(client.bufferId ?? remote.bufferId);
          }
          this.renderRemoteCursors();
        }
        break;
      }

      case "presenceLeft": {
        // Phase 14 (#471): Remove departed client
        const { clientId } = payload.value;
        if (clientId !== undefined) {
          this.remoteClients.delete(BigInt(clientId));
          this.renderRemoteCursors();
        }
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
   *
   * Phase 11.2: Handles empty layout by creating default view.
   */
  private async refreshLayoutOnly(
    focusedId: number,
    _windowInfos: WindowInfo[]
  ): Promise<void> {
    try {
      const layoutResponse = await this.client.state.getLayout({});

      // Phase 11.2: Handle empty layout from server
      if (!layoutResponse.root) {
        await this.createDefaultView();
        return;
      }

      const logical = protoToLogicalLayout(layoutResponse.root);
      if (!logical) {
        await this.createDefaultView();
        return;
      }

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
    console.log("[refreshBufferForBuffer] bufferId:", bufferId, "hasLayout:", !!this.state.layout);
    if (!this.state.layout) {
      console.log("[refreshBufferForBuffer] No layout, returning early");
      return;
    }

    const windows = allWindows(this.state.layout);
    const affectedWindows = windows.filter((w) => w.buffer_id === bufferId);
    console.log("[refreshBufferForBuffer] windows:", windows.length, "affected:", affectedWindows.length);

    if (affectedWindows.length === 0) {
      console.log("[refreshBufferForBuffer] No affected windows, returning early");
      return;
    }

    try {
      // Fetch buffer content once (not per-window)
      const bufferResponse = await this.client.buffer.getRawContent({
        bufferId: BigInt(bufferId),
      });
      console.log("[refreshBufferForBuffer] Got", bufferResponse.lines.length, "lines");

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
      console.warn(`[refreshBufferForBuffer] Failed to refresh buffer ${bufferId}:`, error);
    }

    console.log("[refreshBufferForBuffer] Rendering...");
    this.renderMultiWindow();
  }

  /**
   * Refresh only buffer content (legacy single-window).
   */
  private async refreshBuffer(): Promise<void> {
    console.log("[refreshBuffer] Fetching buffer content...");
    try {
      const response = await this.client.buffer.getRawContent({});
      console.log("[refreshBuffer] Got", response.lines.length, "lines, rendering...");
      this.state.lines = response.lines;
      this.renderBuffer();
      console.log("[refreshBuffer] Render complete");
    } catch (error) {
      console.error("[refreshBuffer] Failed:", error);
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
   *
   * Phase 14 (#471): Show role and client_id in mode indicator.
   * Format: "MODE [Role#ClientId]" e.g., "NORMAL [Owner#1]"
   */
  private renderMode(): void {
    if (!this.modeElement) return;

    // Phase 14 (#471): Include role and client_id in statusline
    // Web client is always Owner (Follow/Share not implemented yet)
    const role = "Owner";
    const clientIdStr = `#${this.myClientId}`;
    const modeText = `${this.state.modeDisplay} [${role}${clientIdStr}]`;

    this.modeElement.textContent = modeText;
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
   * Render remote clients' cursors (Phase 14, #471).
   *
   * Displays thin colored bars at each remote client's cursor position
   * so local user can see where collaborators are working.
   */
  private renderRemoteCursors(): void {
    // Clear existing remote cursor and selection elements
    document.querySelectorAll(".remote-cursor").forEach((el) => el.remove());
    document.querySelectorAll(".remote-selection").forEach((el) => el.remove());

    const container = this.editorElement ?? this.bufferElement;
    if (!container) return;

    const charWidth = 8.4;
    const lineHeight = 21;
    const lineNumberWidth = 48;
    const padding = 8;

    // Determine current buffer ID
    const currentBufferId = this.state.focusedWindowId
      ? this.state.windowStates.get(this.state.focusedWindowId)
        ? 0 // TODO: Track buffer ID per window
        : 0
      : 0;

    for (const [clientId, remote] of this.remoteClients) {
      // Only show cursors for clients viewing the same buffer
      // TODO: Proper buffer ID tracking for multi-buffer comparison
      if (remote.bufferId !== currentBufferId && currentBufferId !== 0) continue;

      // Get per-client color from CBF-8 palette (Issue #474)
      const cursorColor = colorForClient(clientId);

      // Render remote selection if present (Issue #474)
      if (remote.selection) {
        this.renderRemoteSelection(container, clientId, remote, charWidth, lineHeight, lineNumberWidth, padding);
      }

      // Render remote cursor
      const el = document.createElement("div");
      el.className = "remote-cursor";
      el.style.cssText = `
        position: absolute;
        left: ${padding + lineNumberWidth + remote.cursorCol * charWidth}px;
        top: ${padding + remote.cursorLine * lineHeight}px;
        width: 2px;
        height: ${lineHeight}px;
        background: ${cursorColor};
        opacity: 0.85;
        pointer-events: none;
        z-index: 10;
        box-shadow: 0 0 2px rgba(0, 0, 0, 0.3);
      `;
      // Add tooltip with client name
      el.title = remote.displayName;
      container.appendChild(el);
    }
  }

  /**
   * Render a remote client's selection (Issue #474).
   *
   * Called from renderRemoteCursors for each remote client with an active selection.
   */
  private renderRemoteSelection(
    container: HTMLElement,
    clientId: bigint,
    remote: RemoteClient,
    charWidth: number,
    lineHeight: number,
    lineNumberWidth: number,
    padding: number
  ): void {
    const selection = remote.selection;
    if (!selection) return;

    const selectionColor = dimmedColorForClient(clientId);

    // Use anchor and cursor from the selection (normalize to start/end)
    const anchorLine = selection.anchor.line;
    const anchorCol = selection.anchor.col;
    const cursorLine = selection.cursor.line;
    const cursorCol = selection.cursor.col;

    // Calculate selection range
    const startLine = Math.min(anchorLine, cursorLine);
    const endLine = Math.max(anchorLine, cursorLine);

    for (let line = startLine; line <= endLine; line++) {
      // Calculate column range for this line based on selection mode
      let startCol: number;
      let endCol: number;

      if (selection.mode === "line") {
        // Line mode: entire line (use a large width)
        startCol = 0;
        endCol = 200; // Large enough for most lines
      } else if (selection.mode === "block") {
        // Block mode: same columns on every line
        startCol = Math.min(anchorCol, cursorCol);
        endCol = Math.max(anchorCol, cursorCol) + 1;
      } else {
        // Char mode: depends on line position
        if (line === startLine && line === endLine) {
          // Single line selection
          startCol = Math.min(anchorCol, cursorCol);
          endCol = Math.max(anchorCol, cursorCol) + 1;
        } else if (line === startLine) {
          // First line: from anchor/cursor to end of line
          startCol = anchorLine < cursorLine ? anchorCol : cursorCol;
          endCol = 200; // Large width for rest of line
        } else if (line === endLine) {
          // Last line: from start of line to anchor/cursor
          startCol = 0;
          endCol = (anchorLine < cursorLine ? cursorCol : anchorCol) + 1;
        } else {
          // Middle lines: entire line
          startCol = 0;
          endCol = 200;
        }
      }

      const el = document.createElement("div");
      el.className = "remote-selection";
      el.style.cssText = `
        position: absolute;
        left: ${padding + lineNumberWidth + startCol * charWidth}px;
        top: ${padding + line * lineHeight}px;
        width: ${(endCol - startCol) * charWidth}px;
        height: ${lineHeight}px;
        background: ${selectionColor};
        pointer-events: none;
        z-index: 5;
        border-radius: 2px;
      `;
      container.appendChild(el);
    }
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
