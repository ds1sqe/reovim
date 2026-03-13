/**
 * LandingExtension — startup landing screen for the web client.
 *
 * Displays a centered overlay with ASCII art, version info, and quick-action
 * hints when the editor starts with no file argument. Dismissed on first
 * keypress via a one-time keydown listener.
 *
 * This is a purely client-side extension with no server-side module.
 * `applyNotification` is a no-op.
 */

import type { WebExtension } from "./interface.js";

declare const __APP_VERSION__: string;

const ASCII_LOGO = [
  "  ____                  _            ",
  " |  _ \\ ___  _____   _(_)_ __ ___   ",
  " | |_) / _ \\/ _ \\ \\ / / | '_ ` _ \\  ",
  " |  _ <  __/ (_) \\ V /| | | | | | | ",
  " |_| \\_\\___|\\___/ \\_/ |_|_| |_| |_| ",
];

const ACTIONS = [
  { key: "e", desc: "New file" },
  { key: ":e <file>", desc: "Open file" },
  { key: ":q", desc: "Quit" },
  { key: "?", desc: "Help" },
];

const FOOTER = "Press any key to start";

export class LandingExtension implements WebExtension {
  private dismissed: boolean = false;
  private overlayElement: HTMLElement | null = null;
  private keydownHandler: ((e: KeyboardEvent) => void) | null = null;

  kind(): string {
    return "landing";
  }

  isActive(): boolean {
    return !this.dismissed;
  }

  /** No-op: purely client-driven, no server notifications. */
  applyNotification(_data: string): void {}

  render(container: HTMLElement): void {
    if (this.dismissed) return;

    // Remove existing overlay if re-rendering.
    if (this.overlayElement) {
      this.overlayElement.remove();
    }

    const overlay = document.createElement("div");
    overlay.className = "landing-overlay overlay";

    // Logo.
    const logoPre = document.createElement("pre");
    logoPre.className = "landing-logo";
    logoPre.textContent = ASCII_LOGO.join("\n");
    overlay.appendChild(logoPre);

    // Version.
    const versionDiv = document.createElement("div");
    versionDiv.className = "landing-version";
    versionDiv.textContent = `reovim v${__APP_VERSION__}`;
    overlay.appendChild(versionDiv);

    // Actions.
    const actionsDiv = document.createElement("div");
    actionsDiv.className = "landing-actions";
    for (const action of ACTIONS) {
      const row = document.createElement("div");
      row.className = "landing-action";

      const keySpan = document.createElement("span");
      keySpan.className = "landing-key";
      keySpan.textContent = action.key;
      row.appendChild(keySpan);

      const descSpan = document.createElement("span");
      descSpan.className = "landing-desc";
      descSpan.textContent = action.desc;
      row.appendChild(descSpan);

      actionsDiv.appendChild(row);
    }
    overlay.appendChild(actionsDiv);

    // Footer.
    const footerDiv = document.createElement("div");
    footerDiv.className = "landing-footer";
    footerDiv.textContent = FOOTER;
    overlay.appendChild(footerDiv);

    container.appendChild(overlay);
    this.overlayElement = overlay;

    // Dismiss on first keypress.
    if (!this.keydownHandler) {
      this.keydownHandler = () => {
        this.dismiss();
      };
      document.addEventListener("keydown", this.keydownHandler, { once: true });
    }
  }

  hide(): void {
    if (this.overlayElement) {
      this.overlayElement.remove();
      this.overlayElement = null;
    }
  }

  getState(): Record<string, unknown> | null {
    return { active: !this.dismissed };
  }

  private dismiss(): void {
    this.dismissed = true;
    this.hide();
    if (this.keydownHandler) {
      document.removeEventListener("keydown", this.keydownHandler);
      this.keydownHandler = null;
    }
  }
}
