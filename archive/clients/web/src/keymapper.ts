/**
 * Browser Keyboard Event to Vim Notation Mapper
 *
 * Converts browser KeyboardEvent to vim-style key notation.
 *
 * KNOWN LIMITATIONS:
 * - Ctrl+W, Ctrl+T, Ctrl+N may be intercepted by browser (cannot reliably capture)
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
 * Documented for user awareness.
 */
export const UNCAPTURABLE_KEYS = [
  "Ctrl+W (closes tab)",
  "Ctrl+T (new tab)",
  "Ctrl+N (new window)",
  "Ctrl+Shift+T (reopen tab)",
  "F11 (fullscreen)",
  "Alt+F4 (close window on Windows)",
];
