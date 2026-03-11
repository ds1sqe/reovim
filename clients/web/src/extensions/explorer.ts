/**
 * ExplorerExtension — file explorer sidebar rendering.
 *
 * Mirrors `clients/tui/extensions/explorer/src/lib.rs`.
 * Displays a sidebar tree view on the left side with file navigation,
 * expand/collapse, and input prompts for file operations.
 *
 * JSON payload (from `server/modules/explorer/src/bridge.rs`):
 * ```json
 * {"active": true, "rootName": "project", "cursorIndex": 0,
 *  "scrollOffset": 0, "width": 30, "inputMode": "none",
 *  "inputBuffer": "", "showHidden": false,
 *  "nodes": [{"name": "src", "depth": 0, "isDir": true,
 *             "isExpanded": true, "isHidden": false, "isLast": false,
 *             "verticalLines": [], "isSymlink": false, "size": 0}]}
 * ```
 */

import { EXPLORER } from "./extension-kinds.js";
import type { WebExtension } from "./interface.js";

interface NodeData {
  name: string;
  depth: number;
  isDir: boolean;
  isExpanded: boolean;
  isHidden: boolean;
  isLast: boolean;
  verticalLines: boolean[];
  isSymlink: boolean;
  size: number;
}

interface ExplorerState {
  active: boolean;
  rootName: string;
  cursorIndex: number;
  scrollOffset: number;
  width: number;
  inputMode: string;
  inputBuffer: string;
  showHidden: boolean;
  message?: string;
  nodes: NodeData[];
}

function defaultState(): ExplorerState {
  return {
    active: false,
    rootName: "",
    cursorIndex: 0,
    scrollOffset: 0,
    width: 30,
    inputMode: "none",
    inputBuffer: "",
    showHidden: false,
    nodes: [],
  };
}

/** Map inputMode to prompt label (matches TUI lib.rs:125-131). */
function inputLabel(mode: string): string {
  switch (mode) {
    case "createFile":
      return "New file: ";
    case "createDir":
      return "New dir: ";
    case "rename":
      return "Rename: ";
    case "confirmDelete":
      return "Delete? (y/n): ";
    default:
      return "";
  }
}

/**
 * Build tree prefix from verticalLines and isLast (matches TUI render.rs:157-181).
 *
 * - depth 0: no prefix
 * - For each ancestor: verticalLines[i] ? "│ " : "  "
 * - Final connector: isLast ? "└─" : "├─"
 */
function buildPrefix(verticalLines: boolean[], isLast: boolean, depth: number): string {
  if (depth === 0) return "";

  let result = "";
  for (let i = 0; i < depth - 1; i++) {
    result += verticalLines[i] ? "\u2502 " : "  ";
  }
  result += isLast ? "\u2514\u2500" : "\u251c\u2500";
  return result;
}

export class ExplorerExtension implements WebExtension {
  private state: ExplorerState = defaultState();
  private sidebarElement: HTMLElement | null = null;

  kind(): string {
    return EXPLORER;
  }

  isActive(): boolean {
    return this.state.active;
  }

  applyNotification(data: string): void {
    try {
      const parsed = JSON.parse(data) as Record<string, unknown>;
      const active = parsed.active as boolean | undefined;

      if (!active) {
        this.state = defaultState();
        return;
      }

      this.state.active = true;
      this.state.rootName = (parsed.rootName as string) ?? "";
      this.state.cursorIndex = (parsed.cursorIndex as number) ?? 0;
      this.state.scrollOffset = (parsed.scrollOffset as number) ?? 0;
      this.state.width = (parsed.width as number) ?? 30;
      this.state.inputMode = (parsed.inputMode as string) ?? "none";
      this.state.inputBuffer = (parsed.inputBuffer as string) ?? "";
      this.state.showHidden = (parsed.showHidden as boolean) ?? false;
      this.state.message =
        typeof parsed.message === "string" ? parsed.message : undefined;

      // Delta snapshot optimisation: the server omits the "nodes" key when
      // only metadata (cursor, scroll, inputMode) changed. In that case we
      // keep the existing nodes array — only replace when a full snapshot
      // with "nodes" is received (see bridge.rs snapshot_generation logic).
      if (Array.isArray(parsed.nodes)) {
        this.state.nodes = parsed.nodes as NodeData[];
      }
    } catch {
      // Invalid JSON — retain previous state
    }
  }

  // Renders into document.body (not _container) because the sidebar is a
  // fixed-position panel alongside the editor, not an overlay inside #app.
  render(_container: HTMLElement): void {
    if (this.sidebarElement) {
      this.sidebarElement.remove();
      this.sidebarElement = null;
    }

    if (!this.state.active) {
      this.resetAppMargin();
      return;
    }

    const sidebar = document.createElement("div");
    sidebar.className = "explorer-sidebar";
    sidebar.style.width = `${this.state.width}ch`;

    // Header
    const header = document.createElement("div");
    header.className = "explorer-header";
    header.textContent = this.state.rootName;
    sidebar.appendChild(header);

    // Tree area
    const tree = document.createElement("div");
    tree.className = "explorer-tree";

    const { nodes, scrollOffset, cursorIndex } = this.state;
    // Scroll windowing: only render the visible slice of nodes.
    // Estimate visible rows from sidebar height / line-height (1.4em at
    // ~14px ≈ 20px per row). Falls back to 50 rows when DOM is not yet
    // laid out.
    const lineHeightPx = 20;
    const sidebarHeight = sidebar.clientHeight || 600;
    const headerHeight = 24;
    const visibleRows = Math.ceil((sidebarHeight - headerHeight) / lineHeightPx) || 50;
    const end = Math.min(scrollOffset + visibleRows, nodes.length);
    for (let i = scrollOffset; i < end; i++) {
      const node = nodes[i]!;
      const nodeEl = document.createElement("div");
      nodeEl.className = "explorer-node";

      if (i === cursorIndex) {
        nodeEl.classList.add("cursor");
      }
      if (node.isDir) {
        nodeEl.classList.add("dir");
      } else if (node.isSymlink) {
        nodeEl.classList.add("symlink");
      }
      if (node.isHidden) {
        nodeEl.classList.add("hidden-file");
      }

      // Prefix
      const prefix = buildPrefix(node.verticalLines, node.isLast, node.depth);
      if (prefix) {
        const prefixEl = document.createElement("span");
        prefixEl.className = "explorer-prefix";
        prefixEl.textContent = prefix;
        nodeEl.appendChild(prefixEl);
      }

      // Icon
      const iconEl = document.createElement("span");
      iconEl.className = "explorer-icon";
      if (node.isDir) {
        iconEl.textContent = node.isExpanded ? "\u25be " : "\u25b8 ";
      } else if (node.isSymlink) {
        iconEl.textContent = "@ ";
      } else {
        iconEl.textContent = "  ";
      }
      nodeEl.appendChild(iconEl);

      // Name
      const nameEl = document.createElement("span");
      nameEl.className = "explorer-name";
      nameEl.textContent = node.name;
      nodeEl.appendChild(nameEl);

      tree.appendChild(nodeEl);
    }

    sidebar.appendChild(tree);

    // Input prompt (when in input mode)
    if (this.state.inputMode !== "none") {
      const input = document.createElement("div");
      input.className = "explorer-input";

      const label = document.createElement("span");
      label.className = "explorer-input-label";
      label.textContent = inputLabel(this.state.inputMode);
      input.appendChild(label);

      const buffer = document.createElement("span");
      buffer.className = "explorer-input-buffer";
      buffer.textContent = this.state.inputBuffer;
      input.appendChild(buffer);

      const cursorEl = document.createElement("span");
      cursorEl.className = "explorer-input-cursor";
      cursorEl.textContent = "\u2588";
      input.appendChild(cursorEl);

      sidebar.appendChild(input);
    }

    // Status message
    if (this.state.message) {
      const msg = document.createElement("div");
      msg.className = "explorer-message";
      msg.textContent = this.state.message;
      sidebar.appendChild(msg);
    }

    // Render into document.body so sidebar floats above the app layout
    document.body.appendChild(sidebar);
    this.sidebarElement = sidebar;

    // Push #app content to the right to make room for the sidebar
    const app = document.getElementById("app");
    if (app) {
      app.style.marginLeft = `${this.state.width}ch`;
    }
  }

  hide(): void {
    if (this.sidebarElement) {
      this.sidebarElement.remove();
      this.sidebarElement = null;
    }
    this.resetAppMargin();
  }

  private resetAppMargin(): void {
    const app = document.getElementById("app");
    if (app) {
      app.style.marginLeft = "";
    }
  }

  getState(): Record<string, unknown> | null {
    if (!this.state.active) return null;
    return {
      active: this.state.active,
      rootName: this.state.rootName,
      cursorIndex: this.state.cursorIndex,
      scrollOffset: this.state.scrollOffset,
      width: this.state.width,
      inputMode: this.state.inputMode,
      inputBuffer: this.state.inputBuffer,
      showHidden: this.state.showHidden,
      message: this.state.message ?? null,
      nodeCount: this.state.nodes.length,
    };
  }
}
