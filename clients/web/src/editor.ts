/**
 * Editor State and Rendering
 *
 * Manages editor state and renders to the DOM.
 */

import type { ReovimClient } from "./client.js";
import type { Notification } from "./gen/reovim/v2/notification_pb.js";

interface EditorState {
  mode: string;
  modeDisplay: string;
  cursorLine: number;
  cursorCol: number;
  lines: string[];
}

/**
 * Editor class manages state and DOM rendering.
 */
export class Editor {
  private client: ReovimClient;
  private state: EditorState;

  // DOM elements
  private modeElement: HTMLElement | null;
  private bufferElement: HTMLElement | null;
  private cursorElement: HTMLElement | null;
  private positionElement: HTMLElement | null;

  constructor(client: ReovimClient) {
    this.client = client;
    this.state = {
      mode: "normal",
      modeDisplay: "NORMAL",
      cursorLine: 0,
      cursorCol: 0,
      lines: [""],
    };

    // Cache DOM elements
    this.modeElement = document.getElementById("mode");
    this.bufferElement = document.getElementById("buffer");
    this.cursorElement = document.getElementById("cursor");
    this.positionElement = document.getElementById("position");
  }

  /**
   * Refresh editor state from server.
   */
  async refresh(): Promise<void> {
    try {
      // Fetch current state in parallel
      const [modeResponse, cursorResponse, bufferResponse] = await Promise.all([
        this.client.state.getMode({}),
        this.client.state.getCursor({}),
        this.client.buffer.getRawContent({}),
      ]);

      // Update state
      this.state.mode = modeResponse.name.toLowerCase();
      this.state.modeDisplay = modeResponse.display;
      this.state.cursorLine = Number(cursorResponse.position?.line ?? 0);
      this.state.cursorCol = Number(cursorResponse.position?.column ?? 0);
      this.state.lines = bufferResponse.lines;

      // Render updates
      this.render();
    } catch (error) {
      console.error("Failed to refresh editor state:", error);
    }
  }

  /**
   * Subscribe to server notifications for real-time updates.
   */
  async subscribeToNotifications(): Promise<void> {
    try {
      // Subscribe to all event types
      const stream = this.client.notification.subscribe({});

      for await (const notification of stream) {
        this.handleNotification(notification);
      }
    } catch (error) {
      console.error("Notification stream error:", error);
      // TODO: Implement reconnection logic
    }
  }

  /**
   * Handle incoming notification from server.
   *
   * Uses discriminated union pattern: notification.payload.case identifies the type.
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
        const { position } = payload.value;
        this.state.cursorLine = Number(position?.line ?? 0n);
        this.state.cursorCol = Number(position?.column ?? 0n);
        this.renderCursor();
        this.renderPosition();
        break;
      }

      case "bufferModified":
        // Refetch buffer content on modification
        this.refreshBuffer();
        break;

      default:
        // Ignore other notification types for now
        break;
    }
  }

  /**
   * Refresh only buffer content.
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

  /**
   * Render all editor components.
   */
  private render(): void {
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
   * Render buffer content.
   */
  private renderBuffer(): void {
    if (!this.bufferElement) return;

    // Clear existing content
    this.bufferElement.innerHTML = "";

    // Render each line
    this.state.lines.forEach((lineContent, index) => {
      const lineDiv = document.createElement("div");
      lineDiv.className = "line";

      // Line number
      const lineNumber = document.createElement("span");
      lineNumber.className = "line-number";
      lineNumber.textContent = String(index + 1);

      // Line content
      const content = document.createElement("span");
      content.className = "line-content";
      // Use a non-breaking space for empty lines to maintain height
      content.textContent = lineContent || "\u00A0";

      lineDiv.appendChild(lineNumber);
      lineDiv.appendChild(content);
      this.bufferElement!.appendChild(lineDiv);
    });
  }

  /**
   * Render cursor position.
   */
  private renderCursor(): void {
    if (!this.cursorElement || !this.bufferElement) return;

    // Calculate cursor position in pixels
    // This is approximate - a more accurate implementation would measure
    // actual character widths
    const charWidth = 8.4; // Approximate monospace character width at 14px
    const lineHeight = 21; // line-height: 1.5 * 14px

    const lineNumberWidth = 48; // 3rem min-width for line numbers
    const padding = 8; // 0.5rem padding

    const x = padding + lineNumberWidth + this.state.cursorCol * charWidth;
    const y = padding + this.state.cursorLine * lineHeight;

    this.cursorElement.style.left = `${x}px`;
    this.cursorElement.style.top = `${y}px`;

    // Cursor style based on mode
    if (this.state.mode === "insert") {
      this.cursorElement.classList.add("insert");
    } else {
      this.cursorElement.classList.remove("insert");
    }
  }

  /**
   * Render cursor position indicator.
   */
  private renderPosition(): void {
    if (!this.positionElement) return;

    this.positionElement.textContent = `${this.state.cursorLine + 1}:${this.state.cursorCol + 1}`;
  }

  /**
   * Get current mode.
   */
  getMode(): string {
    return this.state.mode;
  }
}
