/**
 * CmdlineExtension — command-line popup rendering.
 *
 * Extracted from the hardcoded handler in `editor.ts` (lines 706-733, 988-1027).
 * Owns its own state, JSON parsing, and DOM lifecycle.
 *
 * JSON payload (from `server/modules/cmdline/src/bridge.rs`):
 * ```json
 * {"active": true, "prompt": ":", "input": "wq", "cursor": 2,
 *  "completions": ["write", "wq"], "completion_index": 0}
 * ```
 */

import { CMDLINE } from "./extension-kinds.js";
import type { WebExtension } from "./interface.js";

interface CmdlineState {
  active: boolean;
  prompt: string;
  input: string;
  cursor: number;
  completions: string[];
  completionIndex: number;
}

export class CmdlineExtension implements WebExtension {
  private state: CmdlineState = {
    active: false,
    prompt: ":",
    input: "",
    cursor: 0,
    completions: [],
    completionIndex: -1,
  };

  kind(): string {
    return CMDLINE;
  }

  isActive(): boolean {
    return this.state.active;
  }

  applyNotification(data: string): void {
    try {
      // Server sends snake_case JSON (completion_index), map to camelCase
      const parsed = JSON.parse(data) as Record<string, unknown>;
      this.state = {
        active: (parsed.active as boolean | undefined) ?? this.state.active,
        prompt: (parsed.prompt as string | undefined) ?? this.state.prompt,
        input: (parsed.input as string | undefined) ?? this.state.input,
        cursor: (parsed.cursor as number | undefined) ?? this.state.cursor,
        completions: (parsed.completions as string[] | undefined) ?? this.state.completions,
        completionIndex: (parsed.completion_index as number | undefined) ?? this.state.completionIndex,
      };
    } catch {
      // Invalid JSON — retain previous state
    }
  }

  render(container: HTMLElement): void {
    const el = container.querySelector("#commandline") as HTMLElement | null
      ?? document.getElementById("commandline");
    if (!el) return;

    el.innerHTML = "";

    const wrapper = document.createElement("div");
    wrapper.className = "cmdline-container";

    const prefixSpan = document.createElement("span");
    prefixSpan.className = "cmdline-prefix";
    prefixSpan.textContent = this.state.prompt;
    wrapper.appendChild(prefixSpan);

    const contentSpan = document.createElement("span");
    contentSpan.className = "cmdline-content";

    const beforeCursor = this.state.input.slice(0, this.state.cursor);
    const afterCursor = this.state.input.slice(this.state.cursor);

    const beforeSpan = document.createElement("span");
    beforeSpan.textContent = beforeCursor;
    contentSpan.appendChild(beforeSpan);

    const cursorSpan = document.createElement("span");
    cursorSpan.className = "cmdline-cursor";
    cursorSpan.textContent = afterCursor.charAt(0) || " ";
    contentSpan.appendChild(cursorSpan);

    if (afterCursor.length > 1) {
      const afterSpan = document.createElement("span");
      afterSpan.textContent = afterCursor.slice(1);
      contentSpan.appendChild(afterSpan);
    }

    wrapper.appendChild(contentSpan);
    el.appendChild(wrapper);

    // Render completions if present
    if (this.state.completions.length > 0) {
      const completionList = document.createElement("div");
      completionList.className = "cmdline-completions";

      for (let i = 0; i < this.state.completions.length; i++) {
        const item = document.createElement("div");
        item.className = "cmdline-completion";
        if (i === this.state.completionIndex) {
          item.classList.add("selected");
        }
        item.textContent = this.state.completions[i] ?? "";
        completionList.appendChild(item);
      }

      el.appendChild(completionList);
    }
  }

  hide(): void {
    const el = document.getElementById("commandline");
    if (el) {
      el.innerHTML = "";
    }
  }

  getState(): Record<string, unknown> | null {
    if (!this.state.active) return null;
    return {
      active: this.state.active,
      prompt: this.state.prompt,
      input: this.state.input,
      cursor: this.state.cursor,
      completions: this.state.completions,
      completionIndex: this.state.completionIndex,
    };
  }
}
