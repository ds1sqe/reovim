/**
 * NotificationExtension — toast notification rendering.
 *
 * Mirrors `clients/tui/extensions/notification/src/lib.rs`.
 * Displays stacked toast notifications with auto-dismiss and progress bars.
 *
 * JSON payload (from `server/modules/notification/src/bridge.rs`):
 * ```json
 * {"active": true, "entries": [
 *   {"id": 0, "level": "info", "title": "Hello", "body": "", "progress": {"percent": 35, "detail": "3/10"}}
 * ]}
 * ```
 */

import { NOTIFICATION } from "./extension-kinds.js";
import type { WebExtension } from "./interface.js";

/** Default auto-dismiss timeout in milliseconds. */
const DEFAULT_TIMEOUT_MS = 4000;

/** Maximum visible toasts. */
const MAX_VISIBLE = 5;

interface ToastEntry {
  id: number;
  level: string;
  title: string;
  body: string;
  progress: { percent: number; detail: string } | null;
  displayedAt: number;
  isProgress: boolean;
}

export class NotificationExtension implements WebExtension {
  private toasts: ToastEntry[] = [];
  private readonly timeoutMs: number;
  private containerElement: HTMLElement | null = null;
  private timerId: ReturnType<typeof setInterval> | null = null;

  constructor(timeoutMs: number = DEFAULT_TIMEOUT_MS) {
    this.timeoutMs = timeoutMs;
  }

  kind(): string {
    return NOTIFICATION;
  }

  isActive(): boolean {
    return this.toasts.length > 0;
  }

  applyNotification(data: string): void {
    try {
      const parsed = JSON.parse(data) as Record<string, unknown>;
      const entries = parsed.entries as Array<Record<string, unknown>> | undefined;
      if (!Array.isArray(entries)) return;

      const serverIds: number[] = [];

      for (const entry of entries) {
        const id = entry.id as number | undefined;
        if (id === undefined || id === null) continue;
        serverIds.push(id);

        // Update progress on existing toast
        const existing = this.toasts.find((t) => t.id === id);
        if (existing) {
          if (entry.progress != null) {
            const p = entry.progress as Record<string, unknown>;
            existing.progress = {
              percent: Math.min((p.percent as number) ?? 0, 100),
              detail: (p.detail as string) ?? "",
            };
          }
          continue;
        }

        // New toast
        const progress = entry.progress != null
          ? {
              percent: Math.min(((entry.progress as Record<string, unknown>).percent as number) ?? 0, 100),
              detail: ((entry.progress as Record<string, unknown>).detail as string) ?? "",
            }
          : null;

        this.toasts.push({
          id,
          level: (entry.level as string) ?? "info",
          title: (entry.title as string) ?? "",
          body: (entry.body as string) ?? "",
          progress,
          displayedAt: Date.now(),
          isProgress: progress !== null,
        });
      }

      // Remove toasts whose IDs are no longer in server state
      this.toasts = this.toasts.filter((t) => serverIds.includes(t.id));

      // Start tick timer if we have toasts
      this.ensureTickTimer();
    } catch {
      // Invalid JSON — retain previous state
    }
  }

  render(container: HTMLElement): void {
    // Remove existing notification container
    if (this.containerElement) {
      this.containerElement.remove();
      this.containerElement = null;
    }

    if (this.toasts.length === 0) return;

    const wrapper = document.createElement("div");
    wrapper.className = "notification-container overlay";

    // Show newest first, capped at MAX_VISIBLE
    const visible = [...this.toasts].reverse().slice(0, MAX_VISIBLE);

    for (const toast of visible) {
      const toastEl = document.createElement("div");
      toastEl.className = `notification-toast notification-${toast.level}`;
      toastEl.dataset.id = String(toast.id);

      // Icon + Title
      const header = document.createElement("div");
      header.className = "notification-header";

      const icon = document.createElement("span");
      icon.className = "notification-icon";
      icon.textContent = this.levelIcon(toast.level);
      header.appendChild(icon);

      const title = document.createElement("span");
      title.className = "notification-title";
      title.textContent = toast.title;
      header.appendChild(title);

      toastEl.appendChild(header);

      // Body
      if (toast.body) {
        const body = document.createElement("div");
        body.className = "notification-body";
        body.textContent = toast.body;
        toastEl.appendChild(body);
      }

      // Progress bar
      if (toast.progress) {
        const progressEl = document.createElement("div");
        progressEl.className = "notification-progress";

        const label = document.createElement("span");
        label.className = "notification-progress-label";
        label.textContent = `${toast.progress.percent}%`;
        progressEl.appendChild(label);

        const barContainer = document.createElement("div");
        barContainer.className = "notification-progress-bar";

        const barFill = document.createElement("div");
        barFill.className = "notification-progress-fill";
        barFill.style.width = `${toast.progress.percent}%`;
        barContainer.appendChild(barFill);

        progressEl.appendChild(barContainer);

        if (toast.progress.detail) {
          const detail = document.createElement("span");
          detail.className = "notification-progress-detail";
          detail.textContent = toast.progress.detail;
          progressEl.appendChild(detail);
        }

        toastEl.appendChild(progressEl);
      }

      wrapper.appendChild(toastEl);
    }

    container.appendChild(wrapper);
    this.containerElement = wrapper;
  }

  hide(): void {
    if (this.containerElement) {
      this.containerElement.remove();
      this.containerElement = null;
    }
    this.clearTickTimer();
  }

  getState(): Record<string, unknown> | null {
    if (this.toasts.length === 0) return null;
    return {
      toasts: this.toasts.map((t) => ({
        id: t.id,
        level: t.level,
        title: t.title,
        body: t.body,
        progress: t.progress,
      })),
    };
  }

  private levelIcon(level: string): string {
    switch (level) {
      case "success": return "+";
      case "warning": return "!";
      case "error": return "x";
      default: return "i";
    }
  }

  private ensureTickTimer(): void {
    if (this.timerId !== null) return;
    if (this.toasts.length === 0) return;

    this.timerId = setInterval(() => {
      const now = Date.now();
      const before = this.toasts.length;
      this.toasts = this.toasts.filter((t) => {
        if (t.isProgress) return true;
        return now - t.displayedAt < this.timeoutMs;
      });

      if (this.toasts.length !== before) {
        // Re-render if container exists
        if (this.containerElement?.parentElement) {
          this.render(this.containerElement.parentElement);
        }
      }

      if (this.toasts.length === 0) {
        this.clearTickTimer();
        if (this.containerElement) {
          this.containerElement.remove();
          this.containerElement = null;
        }
      }
    }, 500);
  }

  private clearTickTimer(): void {
    if (this.timerId !== null) {
      clearInterval(this.timerId);
      this.timerId = null;
    }
  }
}
