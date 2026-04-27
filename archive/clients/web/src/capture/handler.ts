/**
 * Capture Handler for Web Client
 *
 * Handles `captureRequest` notifications from the server and submits
 * frame captures via the StateService.SubmitCaptureResponse RPC.
 *
 * This handler is shared by both:
 * - Editor (browser): Captures DOM-rendered state
 * - HeadlessWebClient (Node.js): Captures internal virtual state
 *
 * Following mechanism vs policy: This is mechanism (how to capture),
 * the caller decides what state to capture (policy).
 */

import type { ReovimClient } from "../client.js";
import type { CaptureRequestPayload } from "../gen/reovim/v2/notification_pb.js";

/** Capture format types (from proto) */
export type CaptureFormat = "plain_text" | "raw_ansi" | "cell_grid";

/** Per-window state for capture */
export interface WindowState {
  lines: string[];
  cursorLine: number;
  cursorCol: number;
  topLine: number;
}

/** State required for frame capture */
export interface CaptureableState {
  mode: string;
  modeDisplay: string;
  cursorLine: number;
  cursorCol: number;
  lines: string[];
  width: number;
  height: number;
  focusedWindowId: number;
  windowStates: Map<number, WindowState>;
}

/** Frame capture result with metadata */
export interface FrameCapture {
  content: string;
  metadata: FrameMetadata;
}

/** Metadata about the captured frame */
export interface FrameMetadata {
  timestamp: string;
  mode: string;
  cursorLine: number;
  cursorCol: number;
  width: number;
  height: number;
  windowCount: number;
}

/** Configuration for CaptureHandler */
export interface CaptureHandlerConfig {
  /** gRPC client for submitting capture responses */
  client: ReovimClient;
  /** Function to get current captureable state */
  getState: () => CaptureableState;
  /** Optional callback when capture is requested */
  onCapture?: (requestId: bigint, format: string) => void;
}

/**
 * Handles capture requests from the server.
 *
 * When the server sends a `captureRequest` notification, this handler:
 * 1. Gets the current state from the configured `getState` function
 * 2. Formats the state as text (matching TUI capture format)
 * 3. Submits the response via `StateService.SubmitCaptureResponse`
 */
export class CaptureHandler {
  private client: ReovimClient;
  private getState: () => CaptureableState;
  private onCapture?: (requestId: bigint, format: string) => void;

  constructor(config: CaptureHandlerConfig) {
    this.client = config.client;
    this.getState = config.getState;
    this.onCapture = config.onCapture;
  }

  /**
   * Handle a capture request notification from the server.
   *
   * This is called from the notification handler when a `captureRequest`
   * payload is received.
   */
  async handleCaptureRequest(payload: CaptureRequestPayload): Promise<void> {
    const requestId = payload.requestId;
    const format = (payload.format || "plain_text") as CaptureFormat;

    // Notify listener if configured
    this.onCapture?.(requestId, format);

    // Get current state and capture
    const state = this.getState();
    const content = this.captureFrame(state, format);

    // Submit response to server
    try {
      await this.client.state.submitCaptureResponse({
        requestId,
        width: BigInt(state.width),
        height: BigInt(state.height),
        format,
        content,
      });
    } catch (error) {
      console.error("Failed to submit capture response:", error);
    }
  }

  /**
   * Capture frame content in the specified format.
   *
   * The format matches TUI's frame capture output for consistency,
   * enabling shared frame assertion helpers.
   */
  captureFrame(state: CaptureableState, format: CaptureFormat): string {
    const lines: string[] = [];
    const now = new Date().toISOString();

    // Header section (matches TUI format)
    lines.push("=== FRAME CAPTURE ===");
    lines.push(`timestamp: ${now}`);
    lines.push(`screen_size: ${state.width}x${state.height}`);
    lines.push(`mode: ${state.modeDisplay}`);
    lines.push(`cursor: line=${state.cursorLine}, col=${state.cursorCol}`);
    lines.push(`windows: ${state.windowStates.size || 1}`);
    lines.push("");

    // Content section
    lines.push(
      `=== SCREEN CONTENT (${state.width} cols x ${state.height} rows) ===`,
    );

    // Render buffer content with line numbers
    const contentLines = state.lines.length > 0 ? state.lines : [""];
    const visibleHeight = Math.min(state.height, contentLines.length + 1);

    for (let y = 0; y < visibleHeight; y++) {
      const lineNum = y + 1;
      if (y < contentLines.length) {
        const lineContent = contentLines[y];
        if (format === "plain_text") {
          lines.push(`[${lineNum}] ${lineContent}`);
        } else {
          // For raw_ansi or cell_grid, still output as plain for now
          // Full ANSI support would require DOM color extraction
          lines.push(`[${lineNum}] ${lineContent}`);
        }
      } else {
        // Empty line marker (like vim's ~ for empty lines)
        lines.push(`[${lineNum}] ~`);
      }
    }

    // Fill remaining height with empty markers
    for (let y = visibleHeight; y < state.height; y++) {
      lines.push(`[${y + 1}] ~`);
    }

    lines.push("=== END FRAME ===");

    return lines.join("\n");
  }

  /**
   * Capture current frame and return with metadata.
   *
   * This is a convenience method for programmatic capture (used by
   * HeadlessWebClient) without needing a server request.
   */
  capture(format: CaptureFormat = "plain_text"): FrameCapture {
    const state = this.getState();
    const content = this.captureFrame(state, format);

    return {
      content,
      metadata: {
        timestamp: new Date().toISOString(),
        mode: state.modeDisplay,
        cursorLine: state.cursorLine,
        cursorCol: state.cursorCol,
        width: state.width,
        height: state.height,
        windowCount: state.windowStates.size || 1,
      },
    };
  }
}
