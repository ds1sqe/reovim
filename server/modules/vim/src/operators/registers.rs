//! Register and clipboard helpers for operators (#515).
//!
//! Provides helpers for storing content to registers with optional clipboard sync.
//!
//! # Architecture (#515 Phase 4)
//!
//! Register storage and clipboard I/O are fully decoupled:
//! - **`RegisterBank`** handles per-client register storage (a-z, "", +, *)
//! - **`ClipboardApi`** handles OS clipboard I/O (copy/paste)
//! - **These helpers** coordinate both when needed
//!
//! # Usage in Operators
//!
//! ```ignore
//! use super::registers;
//!
//! // After yanking/deleting:
//! registers::store_and_sync(ctx.kernel, ctx.registers, ctx.register, &content);
//! registers::push_to_history(ctx.clipboard_history, &content);
//! ```

use {
    reovim_driver_clipboard::{ClipboardKey, ClipboardProviderRegistry},
    reovim_kernel::api::v1::{HistoryRing, KernelContext, Register, RegisterBank, RegisterContent},
};

/// Convert a vim register prefix character to a kernel `Register`.
///
/// Handles the reovim convention:
/// - `a`-`z` → per-client `Slot`
/// - `A`-`Z` → session-shared `Session`
/// - `+`/`*` → system clipboard
/// - `0`-`9` → history ring (read-only)
/// - `"`       → default register (sentinel; callers usually map `None` → `Default`)
///
/// Returns `None` for unrecognised characters.
#[must_use]
pub const fn char_to_register(c: char) -> Option<Register> {
    match c {
        '"' => Some(Register::Default),
        'a'..='z' => Some(Register::Slot(c)),
        'A'..='Z' => Some(Register::Session(c)),
        '+' | '*' => Some(Register::System),
        '0'..='9' => Some(Register::History((c as u8) - b'0')),
        _ => None,
    }
}

/// Convert an `Option<char>` register to a `Register`, defaulting to `Register::Default`.
///
/// This is the primary bridge between driver-level `Option<char>` register
/// addressing and kernel-level `Register` enum addressing.
#[must_use]
pub fn option_char_to_register(register: Option<char>) -> Register {
    register
        .and_then(char_to_register)
        .unwrap_or(Register::Default)
}

/// Store content to the register and sync to OS clipboard if the register is `System`.
///
/// This is the "raw" version for operators that hold separate `&KernelContext` and
/// `&mut RegisterBank` borrows (e.g., via `OperatorContext`).
///
/// - `Register::Default` / `Register::Slot` → stored in per-client `RegisterBank`
/// - `Register::System` → synced to OS clipboard via `ClipboardProvider`
/// - `Register::History` / `Register::PeerHistory` → read-only, returns `false`
/// - `Register::Session` → not handled here (requires session-level access)
///
/// Returns `true` if the content was stored, `false` if the register was
/// read-only or session-scoped.
pub fn store_and_sync(
    kernel: &KernelContext,
    registers: &mut RegisterBank,
    register: Register,
    content: &RegisterContent,
) -> bool {
    match register {
        // Read-only registers and session-scoped (require session-level access)
        Register::History(_) | Register::PeerHistory { .. } | Register::Session(_) => false,

        // System clipboard: sync to OS clipboard
        Register::System => {
            sync_to_clipboard(kernel, &content.text);
            true
        }

        // Bank registers (Default, Slot): store in per-client RegisterBank
        _ => registers.set_register(&register, content.clone()),
    }
}

/// Sync content to OS clipboard.
///
/// Called when storing to `Register::System`. Clipboard errors are
/// silently ignored (graceful degradation).
#[cfg_attr(coverage_nightly, coverage(off))]
fn sync_to_clipboard(kernel: &KernelContext, text: &str) {
    if let Some(registry) = kernel.services.get::<ClipboardProviderRegistry>()
        && let Some(provider) = registry.get(&ClipboardKey::Default)
    {
        let _ = provider.copy_to_clipboard(text);
    }
}

/// Push content to the per-client clipboard history (#515).
///
/// This should be called after every yank/delete operation to populate
/// the numbered registers (0-9).
///
/// Note: We always push to history regardless of target register.
/// This matches vim behavior where `"ayy` yanks to register 'a' AND
/// updates the history.
pub fn push_to_history(clipboard_history: &mut HistoryRing, content: &RegisterContent) {
    clipboard_history.push(content.clone());
}

#[cfg(test)]
#[allow(clippy::significant_drop_tightening)]
#[path = "tests/registers.rs"]
mod tests;
