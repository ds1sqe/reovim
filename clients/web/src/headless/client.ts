/**
 * Headless Web Client for Testing
 *
 * A Node.js-only version of the web client that maintains internal state
 * without DOM rendering. Mirrors the TUI headless pattern for consistency.
 *
 * # Architecture
 *
 * ```text
 * ┌─────────────────────────────────────────────────────────────┐
 * │  HeadlessWebClient (this module)                             │
 * │    - No DOM rendering                                        │
 * │    - Internal state management                               │
 * │    - Programmatic capture/sendKeys/waitFor                   │
 * ├─────────────────────────────────────────────────────────────┤
 * │  gRPC-Web (Connect RPC)                          TRANSPORT   │
 * │    - Service calls (input, state, buffer)                    │
 * │    - Streaming notifications                                 │
 * ├─────────────────────────────────────────────────────────────┤
 * │  Server (lib/server/)                            MECHANISM   │
 * │    - Raw buffer content                                      │
 * │    - Layout, mode, cursor state                              │
 * └─────────────────────────────────────────────────────────────┘
 * ```
 *
 * # Usage
 *
 * ```typescript
 * const client = await HeadlessWebClient.connect({
 *   address: "127.0.0.1:12521",
 *   width: 80,
 *   height: 24,
 * });
 *
 * await client.sendKeys("ihello<Esc>");
 * const frame = await client.waitFor((f) => f.includes("hello"), 2000);
 * console.log(frame);
 *
 * client.close();
 * ```
 */

import { createClient as createConnectClient } from "@connectrpc/connect";
import { createGrpcTransport } from "@connectrpc/connect-node";
import type { Notification } from "../gen/reovim/v2/notification_pb.js";
import { InputService } from "../gen/reovim/v2/input_connect.js";
import { StateService } from "../gen/reovim/v2/state_connect.js";
import { BufferService } from "../gen/reovim/v2/buffer_connect.js";
import { NotificationService } from "../gen/reovim/v2/notification_connect.js";
import { ServerService } from "../gen/reovim/v2/server_connect.js";
import {
  CaptureHandler,
  type CaptureFormat,
  type CaptureableState,
  type FrameCapture,
  type WindowState as CaptureWindowState,
} from "../capture/index.js";

/** gRPC client interface (same as browser client) */
interface ReovimClient {
  input: ReturnType<typeof createConnectClient<typeof InputService>>;
  state: ReturnType<typeof createConnectClient<typeof StateService>>;
  buffer: ReturnType<typeof createConnectClient<typeof BufferService>>;
  notification: ReturnType<typeof createConnectClient<typeof NotificationService>>;
  server: ReturnType<typeof createConnectClient<typeof ServerService>>;
}

/**
 * Create a Node.js gRPC client (uses HTTP/2 native transport).
 */
function createNodeClient(baseUrl: string): ReovimClient {
  const transport = createGrpcTransport({
    baseUrl,
    httpVersion: "2",
  });

  return {
    input: createConnectClient(InputService, transport),
    state: createConnectClient(StateService, transport),
    buffer: createConnectClient(BufferService, transport),
    notification: createConnectClient(NotificationService, transport),
    server: createConnectClient(ServerService, transport),
  };
}

/** Connection options for HeadlessWebClient */
export interface HeadlessClientOptions {
  /** Server address (host:port) */
  address: string;
  /** Viewport width in characters (default: 80) */
  width?: number;
  /** Viewport height in lines (default: 24) */
  height?: number;
}

/** Per-window state maintained by headless client */
interface WindowState {
  lines: string[];
  cursorLine: number;
  cursorCol: number;
  topLine: number;
}

/** Internal state maintained by headless client */
interface HeadlessState {
  mode: string;
  modeDisplay: string;
  isInsertMode: boolean;
  cursorLine: number;
  cursorCol: number;
  focusedWindowId: number;
  lines: string[];
  windowStates: Map<number, WindowState>;
  connected: boolean;
  width: number;
  height: number;
}

/**
 * Headless Web Client for programmatic testing.
 *
 * Maintains internal editor state and provides a programmatic API
 * for sending keys, capturing frames, and waiting for conditions.
 */
export class HeadlessWebClient {
  private client: ReovimClient;
  private state: HeadlessState;
  private captureHandler: CaptureHandler;
  private notificationAbort: AbortController | null = null;

  private constructor(client: ReovimClient, options: HeadlessClientOptions) {
    this.client = client;
    this.state = {
      mode: "normal",
      modeDisplay: "NORMAL",
      isInsertMode: false,
      cursorLine: 0,
      cursorCol: 0,
      focusedWindowId: 0,
      lines: [""],
      windowStates: new Map(),
      connected: true,
      width: options.width ?? 80,
      height: options.height ?? 24,
    };

    this.captureHandler = new CaptureHandler({
      client: this.client,
      getState: () => this.getCaptureableState(),
    });
  }

  /**
   * Connect to a server and create a headless client.
   *
   * Uses native gRPC (HTTP/2) transport for Node.js environment.
   *
   * @param options - Connection options
   * @returns Connected headless client
   */
  static async connect(options: HeadlessClientOptions): Promise<HeadlessWebClient> {
    const baseUrl = `http://${options.address}`;
    const client = createNodeClient(baseUrl);

    const headless = new HeadlessWebClient(client, options);
    await headless.start();

    return headless;
  }

  /**
   * Start the client: fetch initial state and begin notification subscription.
   */
  private async start(): Promise<void> {
    // Fetch initial state from server
    const [modeResp, cursorResp, bufferResp] = await Promise.all([
      this.client.state.getMode({}),
      this.client.state.getCursor({}),
      this.client.buffer.getRawContent({}),
    ]);

    this.state.mode = (modeResp.name || "normal").toLowerCase();
    this.state.modeDisplay = modeResp.display || "NORMAL";
    this.state.isInsertMode = modeResp.isInsert || false;
    this.state.cursorLine = Number(cursorResp.position?.line ?? 0n);
    this.state.cursorCol = Number(cursorResp.position?.column ?? 0n);
    this.state.lines = bufferResp.lines.length > 0 ? bufferResp.lines : [""];

    // Start notification subscription
    this.startNotificationLoop();
  }

  /**
   * Start listening for notifications from the server.
   */
  private startNotificationLoop(): void {
    this.notificationAbort = new AbortController();

    // Fire-and-forget async loop
    void (async () => {
      try {
        const stream = this.client.notification.subscribe(
          {},
          { signal: this.notificationAbort!.signal },
        );

        for await (const notification of stream) {
          await this.handleNotification(notification);
        }
      } catch (error) {
        // Check if this is an abort error (expected during close)
        if (this.notificationAbort?.signal.aborted) {
          return;
        }
        console.error("Notification stream error:", error);
        this.state.connected = false;
      }
    })();
  }

  /**
   * Handle incoming notification from server.
   */
  private async handleNotification(notification: Notification): Promise<void> {
    const { payload } = notification;

    switch (payload.case) {
      case "modeChanged": {
        const { name, display, isInsert } = payload.value;
        this.state.mode = (name || "normal").toLowerCase();
        this.state.modeDisplay = display || "NORMAL";
        this.state.isInsertMode = isInsert || false;
        break;
      }

      case "cursorMoved": {
        const { position, windowId } = payload.value;
        const line = Number(position?.line ?? 0n);
        const col = Number(position?.column ?? 0n);
        const winId = Number(windowId ?? 0n);

        // Update main cursor state
        this.state.cursorLine = line;
        this.state.cursorCol = col;

        // Update per-window state if exists
        const windowState = this.state.windowStates.get(winId);
        if (windowState) {
          windowState.cursorLine = line;
          windowState.cursorCol = col;
        }
        break;
      }

      case "bufferModified": {
        // Re-fetch buffer content
        const { bufferId } = payload.value;
        try {
          const bufferResp = await this.client.buffer.getRawContent({
            bufferId: bufferId,
          });
          this.state.lines = bufferResp.lines.length > 0 ? bufferResp.lines : [""];
        } catch (error) {
          console.warn("Failed to fetch buffer content:", error);
        }
        break;
      }

      case "captureRequest": {
        // Handle capture request from CLI
        await this.captureHandler.handleCaptureRequest(payload.value);
        break;
      }

      default:
        // Ignore other notification types
        break;
    }
  }

  /**
   * Get state in format suitable for capture.
   */
  private getCaptureableState(): CaptureableState {
    // Convert internal WindowState to CaptureWindowState
    const captureWindowStates = new Map<number, CaptureWindowState>();
    for (const [id, ws] of this.state.windowStates) {
      captureWindowStates.set(id, {
        lines: ws.lines,
        cursorLine: ws.cursorLine,
        cursorCol: ws.cursorCol,
        topLine: ws.topLine,
      });
    }

    return {
      mode: this.state.mode,
      modeDisplay: this.state.modeDisplay,
      cursorLine: this.state.cursorLine,
      cursorCol: this.state.cursorCol,
      lines: this.state.lines,
      width: this.state.width,
      height: this.state.height,
      focusedWindowId: this.state.focusedWindowId,
      windowStates: captureWindowStates,
    };
  }

  // ========== Public API ==========

  /**
   * Send keys to the server.
   *
   * @param keys - Key sequence (e.g., "ihello<Esc>")
   * @returns True if server accepted the keys
   */
  async sendKeys(keys: string): Promise<boolean> {
    const response = await this.client.input.sendKeys({ keys });
    return response.ok;
  }

  /**
   * Capture current frame.
   *
   * @param format - Capture format (default: "plain_text")
   * @returns Frame content and metadata
   */
  capture(format: CaptureFormat = "plain_text"): FrameCapture {
    return this.captureHandler.capture(format);
  }

  /**
   * Wait for a condition to be true, polling the frame content.
   *
   * @param predicate - Function that returns true when condition is met
   * @param timeoutMs - Maximum time to wait (default: 2000ms)
   * @returns Frame content when predicate returns true
   * @throws Error if timeout is reached
   */
  async waitFor(
    predicate: (frame: string) => boolean,
    timeoutMs: number = 2000,
  ): Promise<string> {
    const pollInterval = 10;
    const start = Date.now();

    while (Date.now() - start < timeoutMs) {
      const frame = this.capture().content;
      if (predicate(frame)) {
        return frame;
      }
      await new Promise((resolve) => setTimeout(resolve, pollInterval));
    }

    // Include last frame in error for debugging
    const lastFrame = this.capture().content;
    throw new Error(
      `waitFor timed out after ${timeoutMs}ms. Last frame:\n${lastFrame}`,
    );
  }

  /**
   * Get current mode.
   */
  getMode(): string {
    return this.state.mode;
  }

  /**
   * Get current mode display string.
   */
  getModeDisplay(): string {
    return this.state.modeDisplay;
  }

  /**
   * Get current cursor position.
   */
  getCursor(): { line: number; col: number } {
    return {
      line: this.state.cursorLine,
      col: this.state.cursorCol,
    };
  }

  /**
   * Get buffer content as array of lines.
   */
  getLines(): string[] {
    return [...this.state.lines];
  }

  /**
   * Get buffer content as single string.
   */
  getBuffer(): string {
    return this.state.lines.join("\n");
  }

  /**
   * Check if client is connected.
   */
  isConnected(): boolean {
    return this.state.connected;
  }

  /**
   * Close the client connection.
   */
  close(): void {
    this.notificationAbort?.abort();
    this.state.connected = false;
  }
}
