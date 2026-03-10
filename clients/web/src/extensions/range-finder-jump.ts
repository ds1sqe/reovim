/**
 * RangeFinderJumpExtension -- jump label rendering for web client.
 *
 * Mirrors `clients/tui/extensions/range-finder/src/jump.rs`.
 * Parses JumpBridge JSON and renders label overlays at match positions.
 *
 * JSON payload (from `server/modules/range-finder/src/jump/bridge.rs`):
 * ```json
 * {"active": true, "matches": [{"line": 0, "col": 5, "label": "s"}]}
 * ```
 */

import type { WebExtension } from "./interface.js";

interface JumpLabel {
  line: number;
  col: number;
  label: string;
}

interface JumpState {
  active: boolean;
  matches: JumpLabel[];
}

export class RangeFinderJumpExtension implements WebExtension {
  private state: JumpState = { active: false, matches: [] };
  private containerElement: HTMLElement | null = null;

  kind(): string {
    return "range-finder-jump";
  }

  isActive(): boolean {
    return this.state.active;
  }

  applyNotification(data: string): void {
    try {
      const parsed = JSON.parse(data) as Record<string, unknown>;
      const active = parsed.active as boolean | undefined;
      this.state.active = active === true;
      this.state.matches = [];

      if (!this.state.active) return;

      const matches = parsed.matches as Array<Record<string, unknown>> | undefined;
      if (!Array.isArray(matches)) return;

      for (const m of matches) {
        const line = m.line as number | undefined;
        const col = m.col as number | undefined;
        const label = m.label as string | undefined;
        if (line === undefined || col === undefined || label === undefined) continue;
        this.state.matches.push({ line, col, label });
      }
    } catch {
      // Invalid JSON -- retain previous state
    }
  }

  render(container: HTMLElement): void {
    if (this.containerElement) {
      this.containerElement.remove();
      this.containerElement = null;
    }

    if (!this.state.active || this.state.matches.length === 0) return;

    const wrapper = document.createElement("div");
    wrapper.className = "range-finder-jump-overlay overlay";

    for (const match of this.state.matches) {
      const labelEl = document.createElement("span");
      labelEl.className = "jump-label";
      labelEl.textContent = match.label;
      labelEl.dataset.line = String(match.line);
      labelEl.dataset.col = String(match.col);
      wrapper.appendChild(labelEl);
    }

    container.appendChild(wrapper);
    this.containerElement = wrapper;
  }

  hide(): void {
    if (this.containerElement) {
      this.containerElement.remove();
      this.containerElement = null;
    }
  }

  getState(): Record<string, unknown> | null {
    if (!this.state.active) return null;
    return {
      active: this.state.active,
      matches: this.state.matches,
    };
  }
}
