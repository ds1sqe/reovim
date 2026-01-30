/**
 * Editor State and Rendering
 *
 * Manages editor state and renders to the DOM.
 */

import type { ReovimClient } from "./client.js";
import type { Notification } from "./gen/reovim/v2/notification_pb.js";

/** Position within the buffer */
interface Position {
  line: number;
  col: number;
}

/** Visual mode type */
type VisualMode = "char" | "line" | "block";

interface EditorState {
  mode: string;
  modeDisplay: string;
  cursorLine: number;
  cursorCol: number;
  lines: string[];
  // Selection state (Phase 9.1)
  hasSelection: boolean;
  selectionAnchor: Position | null;
  selectionCursor: Position | null;
  visualMode: VisualMode | null;
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
      // Selection state initialized to no selection
      hasSelection: false,
      selectionAnchor: null,
      selectionCursor: null,
      visualMode: null,
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

      case "selectionChanged": {
        const { hasSelection, selection, visualMode } = payload.value;
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
        } else {
          // Clear selection state
          this.state.selectionAnchor = null;
          this.state.selectionCursor = null;
          this.state.visualMode = null;
        }

        // Re-render buffer with selection highlighting
        this.renderBuffer();
        break;
      }

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
   * Render buffer content with optional selection highlighting.
   */
  private renderBuffer(): void {
    if (!this.bufferElement) return;

    // Clear existing content
    this.bufferElement.innerHTML = "";

    // Render each line
    this.state.lines.forEach((lineContent, lineIndex) => {
      const lineDiv = document.createElement("div");
      lineDiv.className = "line";

      // Line number
      const lineNumber = document.createElement("span");
      lineNumber.className = "line-number";
      lineNumber.textContent = String(lineIndex + 1);

      // Line content - render with selection if active
      const content = document.createElement("span");
      content.className = "line-content";

      if (this.state.hasSelection && lineContent.length > 0) {
        // Render with selection highlighting
        this.renderLineWithSelection(content, lineContent, lineIndex);
      } else if (
        this.state.hasSelection &&
        lineContent.length === 0 &&
        this.state.visualMode === "line" &&
        this.isLineSelected(lineIndex)
      ) {
        // Empty line in line-wise selection - show highlighted space
        content.innerHTML = '<span class="selected">\u00A0</span>';
      } else {
        // No selection or empty line - render normally
        content.textContent = lineContent || "\u00A0";
      }

      lineDiv.appendChild(lineNumber);
      lineDiv.appendChild(content);
      this.bufferElement!.appendChild(lineDiv);
    });
  }

  /**
   * Render a line with selection highlighting.
   *
   * Groups consecutive characters by selection state for efficiency,
   * creating spans only at selection boundaries.
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
        // Start new span when selection state changes
        currentSpan = document.createElement("span");
        if (isSelected) {
          currentSpan.className = "selected";
        }
        container.appendChild(currentSpan);
        currentSelected = isSelected;
      }

      currentSpan.textContent += char;
    }

    // Handle empty line (shouldn't happen since we check length > 0 above)
    if (text.length === 0) {
      container.textContent = "\u00A0";
    }
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

    this.positionElement.textContent = `${this.state.cursorLine + 1}:${this.state.cursorCol + 1}`;
  }

  /**
   * Get current mode.
   */
  getMode(): string {
    return this.state.mode;
  }

  /**
   * Check if a position is within the current selection.
   *
   * Handles all three visual modes (char, line, block) and both
   * forward and reverse selections.
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

    // Normalize positions: always work with start <= end
    // This handles both forward (anchor before cursor) and reverse selections
    const isForward =
      anchor.line < cursor.line ||
      (anchor.line === cursor.line && anchor.col <= cursor.col);
    const start = isForward ? anchor : cursor;
    const end = isForward ? cursor : anchor;

    switch (this.state.visualMode) {
      case "char": {
        // Character-wise: position must be between start and end (inclusive)
        // Out of line range?
        if (line < start.line || line > end.line) return false;

        // Single line selection (same start and end line)
        if (start.line === end.line) {
          return col >= start.col && col <= end.col;
        }

        // Multi-line: first line (from start.col to EOL)
        if (line === start.line) return col >= start.col;

        // Multi-line: last line (from BOL to end.col)
        if (line === end.line) return col <= end.col;

        // Multi-line: middle lines are fully selected
        return true;
      }

      case "line": {
        // Line-wise: entire lines between start and end are selected
        return line >= start.line && line <= end.line;
      }

      case "block": {
        // Block-wise: rectangular region defined by corners
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
   * Used for line-wise visual mode and empty line highlighting.
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
}
