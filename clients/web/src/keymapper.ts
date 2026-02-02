/**
 * Browser Keyboard Event to Vim Notation Mapper
 *
 * Converts browser KeyboardEvent to vim-style key notation.
 *
 * ## Browser Keyboard Policy
 *
 * Browsers reserve certain shortcuts (Ctrl+W, Ctrl+T, Ctrl+N) for security.
 * Web apps CANNOT capture these keys - this is intentional browser policy.
 *
 * ### Solution: Web-Only Leader Key
 *
 * Use `\` (backslash) as a leader key to access Ctrl combinations:
 *   - `\w` → `<C-w>` (window commands)
 *   - `\t` → `<C-t>` (tag jump)
 *   - `\n` → `<C-n>` (next completion)
 *   - etc.
 *
 * The server receives standard vim notation - it doesn't know about
 * the web leader key. TUI clients capture Ctrl+W natively.
 *
 * ## Other Limitations
 *
 * - Meta key (Cmd on Mac) behavior varies by browser
 * - Dead keys (accents) may not map correctly
 * - Some international keyboard layouts may have edge cases
 */

/** Special key mappings from KeyboardEvent.key to vim notation */
const SPECIAL_KEYS: Record<string, string> = {
  Escape: "Esc",
  Enter: "CR",
  Return: "CR",
  Tab: "Tab",
  Backspace: "BS",
  Delete: "Del",
  Insert: "Insert",
  Home: "Home",
  End: "End",
  PageUp: "PageUp",
  PageDown: "PageDown",
  ArrowUp: "Up",
  ArrowDown: "Down",
  ArrowLeft: "Left",
  ArrowRight: "Right",
  " ": "Space",
  "<": "lt",
  "\\": "Bslash",
  "|": "Bar",
  F1: "F1",
  F2: "F2",
  F3: "F3",
  F4: "F4",
  F5: "F5",
  F6: "F6",
  F7: "F7",
  F8: "F8",
  F9: "F9",
  F10: "F10",
  F11: "F11",
  F12: "F12",
};

/**
 * Convert a browser KeyboardEvent to vim notation.
 *
 * @param event - The keyboard event from the browser
 * @returns Vim notation string (e.g., "<C-w>", "<Esc>", "a")
 *
 * @example
 * // Regular keys
 * browserKeyToVim({ key: "a" }) // => "a"
 * browserKeyToVim({ key: "A", shiftKey: true }) // => "A"
 *
 * // Special keys
 * browserKeyToVim({ key: "Escape" }) // => "<Esc>"
 * browserKeyToVim({ key: "Enter" }) // => "<CR>"
 *
 * // Modifiers
 * browserKeyToVim({ key: "w", ctrlKey: true }) // => "<C-w>"
 * browserKeyToVim({ key: "a", altKey: true }) // => "<A-a>"
 * browserKeyToVim({ key: "a", ctrlKey: true, shiftKey: true }) // => "<C-S-a>"
 */
export function browserKeyToVim(event: KeyboardEvent): string | null {
  const { key, ctrlKey, altKey, metaKey, shiftKey } = event;

  // Ignore modifier-only key presses
  if (
    key === "Control" ||
    key === "Alt" ||
    key === "Shift" ||
    key === "Meta"
  ) {
    return null;
  }

  // Determine the base key
  let baseKey: string;
  const specialKey = SPECIAL_KEYS[key];

  if (specialKey) {
    baseKey = specialKey;
  } else if (key.length === 1) {
    // Single character key
    baseKey = key;
  } else {
    // Unknown key, skip
    console.warn(`Unknown key: ${key}`);
    return null;
  }

  // Build modifier prefix
  const modifiers: string[] = [];

  // Note: We treat Meta (Cmd on Mac) as Ctrl for vim compatibility
  if (ctrlKey || metaKey) {
    modifiers.push("C");
  }

  if (altKey) {
    modifiers.push("A");
  }

  // Only add Shift modifier for special keys or when combined with Ctrl/Alt
  // Regular shifted characters (A-Z, !, @, etc.) don't need <S-> prefix
  if (shiftKey && (specialKey || ctrlKey || altKey || metaKey)) {
    modifiers.push("S");
  }

  // Format the result
  if (modifiers.length > 0 || specialKey) {
    // Use angle bracket notation: <C-w>, <Esc>, <S-Tab>
    const modifierStr = modifiers.length > 0 ? modifiers.join("-") + "-" : "";
    return `<${modifierStr}${baseKey}>`;
  }

  // Plain character key
  return baseKey;
}

/**
 * Check if a key event should be prevented from browser default handling.
 *
 * Returns true for keys that vim should handle instead of the browser.
 */
export function shouldPreventDefault(event: KeyboardEvent): boolean {
  const { key, ctrlKey, altKey, metaKey } = event;

  // Always prevent Escape (might close dialogs)
  if (key === "Escape") {
    return true;
  }

  // Prevent Ctrl+key combinations that vim uses
  // Note: Some like Ctrl+T, Ctrl+W may still be intercepted by browser
  if (ctrlKey || metaKey) {
    const vimCtrlKeys = [
      "a",
      "b",
      "c",
      "d",
      "e",
      "f",
      "g",
      "h",
      "i",
      "j",
      "k",
      "l",
      "m",
      "n",
      "o",
      "p",
      "q",
      "r",
      "s",
      "t",
      "u",
      "v",
      "w",
      "x",
      "y",
      "z",
      "[",
      "]",
      "\\",
    ];
    if (vimCtrlKeys.includes(key.toLowerCase())) {
      return true;
    }
  }

  // Prevent Alt+key (used for vim <A-> bindings)
  if (altKey) {
    return true;
  }

  // Prevent Tab (vim uses it)
  if (key === "Tab") {
    return true;
  }

  // Prevent Backspace in non-insert contexts (let vim handle it)
  if (key === "Backspace") {
    return true;
  }

  return false;
}

/**
 * List of keys that browsers typically intercept and cannot be captured.
 * Use the leader key (\) as an alternative. See WebKeymapper.
 */
export const UNCAPTURABLE_KEYS = [
  "Ctrl+W (closes tab) - use \\w instead",
  "Ctrl+T (new tab) - use \\t instead",
  "Ctrl+N (new window) - use \\n instead",
  "Ctrl+Shift+T (reopen tab)",
  "Ctrl+Shift+I (dev tools)",
  "F11 (fullscreen)",
  "Alt+F4 (close window on Windows)",
];

// ============================================================================
// Web-Only Leader Key System
// ============================================================================

/** Leader key state */
export type KeymapperState = "normal" | "leader_pending";

/** Default leader key (backslash) */
const LEADER_KEY = "\\";

/** Leader timeout in milliseconds */
const LEADER_TIMEOUT_MS = 1000;

/**
 * Mapping from leader+key to Ctrl combinations.
 *
 * These are browser-reserved keys that we translate via leader key.
 */
const LEADER_TO_CTRL: Record<string, string> = {
  // Window/tab management (browser reserves Ctrl+W/T/N)
  w: "<C-w>", // Window commands
  t: "<C-t>", // Tag jump
  n: "<C-n>", // Next completion / down

  // Navigation (browser may reserve some)
  p: "<C-p>", // Prev completion / up
  o: "<C-o>", // Jump back in jumplist
  i: "<C-i>", // Jump forward (same as Tab)
  "]": "<C-]>", // Jump to definition
  "[": "<C-[>", // Same as Escape

  // Scrolling
  d: "<C-d>", // Half page down
  u: "<C-u>", // Half page up
  f: "<C-f>", // Page forward (down)
  b: "<C-b>", // Page back (up)
  e: "<C-e>", // Scroll down one line
  y: "<C-y>", // Scroll up one line

  // Editing
  r: "<C-r>", // Redo / insert register
  a: "<C-a>", // Increment number
  x: "<C-x>", // Decrement number
  v: "<C-v>", // Visual block mode

  // Other
  g: "<C-g>", // File info
  l: "<C-l>", // Redraw screen
  z: "<C-z>", // Suspend (no-op in web)
};

/** Options for WebKeymapper constructor */
export interface WebKeymapperOptions {
  /** Called when leader key is pressed (to show indicator) */
  onLeaderStart?: () => void;
  /** Called when leader sequence completes or times out */
  onLeaderEnd?: () => void;
}

/**
 * Stateful keyboard mapper with leader key support.
 *
 * Handles browser-reserved keys by translating leader sequences:
 *   `\w` → `<C-w>`
 *   `\t` → `<C-t>`
 *   etc.
 *
 * The server receives standard vim notation and doesn't know
 * about the leader key - this is a web-only translation layer.
 *
 * @example
 * ```typescript
 * const keymapper = new WebKeymapper({
 *   onLeaderStart: () => showIndicator('\\'),
 *   onLeaderEnd: () => hideIndicator(),
 * });
 *
 * document.addEventListener('keydown', (event) => {
 *   const vimKey = keymapper.handleKeyEvent(event);
 *   if (vimKey) {
 *     sendToServer(vimKey);
 *   }
 * });
 * ```
 */
export class WebKeymapper {
  private state: KeymapperState = "normal";
  private leaderTimeout: number | null = null;
  private onLeaderStart?: () => void;
  private onLeaderEnd?: () => void;

  constructor(options?: WebKeymapperOptions) {
    this.onLeaderStart = options?.onLeaderStart;
    this.onLeaderEnd = options?.onLeaderEnd;
  }

  /**
   * Handle a keyboard event and return vim notation.
   *
   * @param event - Browser keyboard event
   * @returns Vim notation string, or null if no key should be sent
   *          (e.g., leader pending, modifier-only press, cancelled)
   */
  handleKeyEvent(event: KeyboardEvent): string | null {
    const { key } = event;

    // Ignore modifier-only presses
    if (["Control", "Alt", "Shift", "Meta"].includes(key)) {
      return null;
    }

    // Handle based on current state
    if (this.state === "normal") {
      return this.handleNormalState(event);
    }

    // Leader pending state
    return this.handleLeaderPendingState(event);
  }

  /**
   * Handle key in normal state.
   */
  private handleNormalState(event: KeyboardEvent): string | null {
    const { key, ctrlKey, altKey, metaKey } = event;

    // Check for leader key (\ without modifiers)
    if (key === LEADER_KEY && !ctrlKey && !altKey && !metaKey) {
      // Prevent browser default (\ might do something)
      event.preventDefault();

      this.state = "leader_pending";
      this.startTimeout();
      this.onLeaderStart?.();
      return null; // Wait for next key
    }

    // Normal key handling with preventDefault
    if (shouldPreventDefault(event)) {
      event.preventDefault();
    }

    return browserKeyToVim(event);
  }

  /**
   * Handle key in leader pending state.
   */
  private handleLeaderPendingState(event: KeyboardEvent): string | null {
    const { key } = event;

    // Always prevent default in leader state
    event.preventDefault();

    // Clear timeout and reset state
    this.clearTimeout();
    this.state = "normal";
    this.onLeaderEnd?.();

    // Escape cancels leader
    if (key === "Escape") {
      return null;
    }

    // Check leader mappings (case-insensitive for letters)
    const lookupKey = key.length === 1 ? key.toLowerCase() : key;
    const mapped = LEADER_TO_CTRL[lookupKey];

    if (mapped) {
      return mapped;
    }

    // Unknown leader combo - send the key after leader as-is
    // This allows \ followed by unmapped keys to work normally
    return browserKeyToVim(event);
  }

  /**
   * Start the leader timeout.
   * If timeout expires, leader is cancelled.
   */
  private startTimeout(): void {
    // Use globalThis for Node.js test compatibility
    this.leaderTimeout = globalThis.setTimeout(() => {
      this.state = "normal";
      this.leaderTimeout = null;
      this.onLeaderEnd?.();
    }, LEADER_TIMEOUT_MS) as unknown as number;
  }

  /**
   * Clear the leader timeout.
   */
  private clearTimeout(): void {
    if (this.leaderTimeout !== null) {
      globalThis.clearTimeout(this.leaderTimeout);
      this.leaderTimeout = null;
    }
  }

  /**
   * Get current keymapper state.
   */
  getState(): KeymapperState {
    return this.state;
  }

  /**
   * Check if leader key is currently pending.
   */
  isLeaderPending(): boolean {
    return this.state === "leader_pending";
  }

  /**
   * Reset state (useful for testing or error recovery).
   */
  reset(): void {
    this.clearTimeout();
    this.state = "normal";
  }
}
