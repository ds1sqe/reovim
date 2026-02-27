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
mod tests {
    use {super::*, reovim_kernel::api::v1::YankType};

    fn content(text: &str) -> RegisterContent {
        RegisterContent::new(text.to_string(), YankType::Characterwise)
    }

    // ========================================================================
    // store_and_sync tests
    // ========================================================================

    #[test]
    fn test_history_registers_are_read_only() {
        let kernel = KernelContext::default();
        let mut registers = RegisterBank::new();
        assert!(!store_and_sync(&kernel, &mut registers, Register::History(0), &content("test")));
        assert!(!store_and_sync(&kernel, &mut registers, Register::History(5), &content("test")));
        assert!(!store_and_sync(
            &kernel,
            &mut registers,
            Register::History(255),
            &content("test")
        ));
    }

    #[test]
    fn test_peer_history_is_read_only() {
        let kernel = KernelContext::default();
        let mut registers = RegisterBank::new();
        assert!(!store_and_sync(
            &kernel,
            &mut registers,
            Register::PeerHistory {
                client: 1,
                index: 0
            },
            &content("test"),
        ));
    }

    #[test]
    fn test_session_registers_not_handled() {
        let kernel = KernelContext::default();
        let mut registers = RegisterBank::new();
        assert!(!store_and_sync(
            &kernel,
            &mut registers,
            Register::Session('A'),
            &content("test")
        ));
    }

    #[test]
    fn test_slot_registers_work() {
        let kernel = KernelContext::default();
        let mut registers = RegisterBank::new();
        assert!(store_and_sync(&kernel, &mut registers, Register::Slot('a'), &content("alpha")));
        assert_eq!(registers.get_named('a').map(|r| r.text.as_str()), Some("alpha"));
    }

    #[test]
    fn test_default_register_works() {
        let kernel = KernelContext::default();
        let mut registers = RegisterBank::new();
        assert!(store_and_sync(&kernel, &mut registers, Register::Default, &content("unnamed")));
        assert_eq!(registers.get().text, "unnamed");
    }

    #[test]
    fn test_multiple_slot_registers() {
        let kernel = KernelContext::default();
        let mut registers = RegisterBank::new();
        assert!(store_and_sync(&kernel, &mut registers, Register::Slot('a'), &content("alpha")));
        assert!(store_and_sync(&kernel, &mut registers, Register::Slot('b'), &content("bravo")));
        assert!(store_and_sync(&kernel, &mut registers, Register::Slot('z'), &content("zulu")));

        assert_eq!(registers.get_named('a').map(|r| r.text.as_str()), Some("alpha"));
        assert_eq!(registers.get_named('b').map(|r| r.text.as_str()), Some("bravo"));
        assert_eq!(registers.get_named('z').map(|r| r.text.as_str()), Some("zulu"));
    }

    #[test]
    fn test_default_register_overwrite() {
        let kernel = KernelContext::default();
        let mut registers = RegisterBank::new();
        assert!(store_and_sync(&kernel, &mut registers, Register::Default, &content("first")));
        assert_eq!(registers.get().text, "first");

        assert!(store_and_sync(&kernel, &mut registers, Register::Default, &content("second")));
        assert_eq!(registers.get().text, "second");
    }

    #[test]
    fn test_slot_register_overwrite() {
        let kernel = KernelContext::default();
        let mut registers = RegisterBank::new();
        assert!(store_and_sync(&kernel, &mut registers, Register::Slot('a'), &content("first")));
        assert!(store_and_sync(&kernel, &mut registers, Register::Slot('a'), &content("second")));
        assert_eq!(registers.get_named('a').map(|r| r.text.as_str()), Some("second"));
    }

    #[test]
    fn test_system_register_no_provider() {
        // Without a ClipboardProvider, System register sync fails silently
        let kernel = KernelContext::default();
        let mut registers = RegisterBank::new();
        assert!(store_and_sync(&kernel, &mut registers, Register::System, &content("clip")));
    }

    #[test]
    fn test_linewise_content_stored() {
        let kernel = KernelContext::default();
        let mut registers = RegisterBank::new();
        let linewise_content = RegisterContent::linewise("line\n".to_string());
        assert!(store_and_sync(&kernel, &mut registers, Register::Slot('a'), &linewise_content));
        let reg = registers.get_named('a').cloned();
        assert!(reg.is_some());
        assert_eq!(reg.unwrap().text, "line\n");
    }

    #[test]
    fn test_empty_text_stored() {
        let kernel = KernelContext::default();
        let mut registers = RegisterBank::new();
        assert!(store_and_sync(&kernel, &mut registers, Register::Default, &content("")));
        assert_eq!(registers.get().text, "");
    }

    #[test]
    fn test_all_lowercase_slot_registers() {
        let kernel = KernelContext::default();
        let mut registers = RegisterBank::new();
        for c in 'a'..='z' {
            assert!(
                store_and_sync(
                    &kernel,
                    &mut registers,
                    Register::Slot(c),
                    &content(&format!("reg-{c}"))
                ),
                "register '{c}' should accept writes"
            );
        }
        assert_eq!(registers.get_named('a').map(|r| r.text.as_str()), Some("reg-a"));
        assert_eq!(registers.get_named('m').map(|r| r.text.as_str()), Some("reg-m"));
        assert_eq!(registers.get_named('z').map(|r| r.text.as_str()), Some("reg-z"));
    }

    #[test]
    fn test_linewise_content_yank_type() {
        let kernel = KernelContext::default();
        let mut registers = RegisterBank::new();
        let linewise = RegisterContent::linewise("line\n".to_string());
        assert!(store_and_sync(&kernel, &mut registers, Register::Slot('a'), &linewise));
        let reg = registers.get_named('a').cloned();
        assert!(reg.is_some());
        assert!(reg.unwrap().is_linewise());
    }

    #[test]
    fn test_characterwise_content_yank_type() {
        let kernel = KernelContext::default();
        let mut registers = RegisterBank::new();
        let charwise = RegisterContent::characterwise("text".to_string());
        assert!(store_and_sync(&kernel, &mut registers, Register::Slot('b'), &charwise));
        let reg = registers.get_named('b').cloned();
        assert!(reg.is_some());
        assert!(reg.unwrap().is_characterwise());
    }

    #[test]
    fn test_multiline_text_stored() {
        let kernel = KernelContext::default();
        let mut registers = RegisterBank::new();
        let multi = content("line1\nline2\nline3");
        assert!(store_and_sync(&kernel, &mut registers, Register::Slot('c'), &multi));
        let reg = registers.get_named('c').cloned();
        assert!(reg.is_some());
        assert_eq!(reg.unwrap().text, "line1\nline2\nline3");
    }

    #[test]
    fn test_store_and_overwrite_default() {
        let kernel = KernelContext::default();
        let mut registers = RegisterBank::new();
        assert!(store_and_sync(&kernel, &mut registers, Register::Default, &content("first")));
        assert_eq!(registers.get().text, "first");

        assert!(store_and_sync(&kernel, &mut registers, Register::Default, &content("second")));
        assert_eq!(registers.get().text, "second");

        assert!(store_and_sync(&kernel, &mut registers, Register::Default, &content("third")));
        assert_eq!(registers.get().text, "third");
    }

    #[test]
    fn test_slot_does_not_affect_default() {
        let kernel = KernelContext::default();
        let mut registers = RegisterBank::new();
        assert!(store_and_sync(&kernel, &mut registers, Register::Default, &content("unnamed")));
        assert!(store_and_sync(
            &kernel,
            &mut registers,
            Register::Slot('a'),
            &content("named-a")
        ));

        assert_eq!(registers.get().text, "unnamed");
        assert_eq!(registers.get_named('a').map(|r| r.text.as_str()), Some("named-a"));
    }

    // ========================================================================
    // push_to_history tests
    // ========================================================================

    #[test]
    fn test_push_to_history() {
        let mut clipboard_history = HistoryRing::new();
        push_to_history(&mut clipboard_history, &content("test"));
        assert_eq!(clipboard_history.len(), 1);
    }

    #[test]
    fn test_push_to_history_multiple_calls() {
        let mut clipboard_history = HistoryRing::new();
        push_to_history(&mut clipboard_history, &content("first"));
        push_to_history(&mut clipboard_history, &content("second"));
        push_to_history(&mut clipboard_history, &content("third"));
        assert_eq!(clipboard_history.len(), 3);
    }

    // ========================================================================
    // sync_to_clipboard tests
    // ========================================================================

    #[test]
    fn test_sync_to_clipboard_no_provider() {
        let kernel = KernelContext::default();
        // Should not panic when no ClipboardProvider is registered
        sync_to_clipboard(&kernel, "test");
    }

    // ========================================================================
    // sync_to_clipboard with provider tests
    // ========================================================================

    /// Mock clipboard that records calls.
    struct MockClipboardProvider {
        clipboard: std::sync::Mutex<Option<String>>,
        selection: std::sync::Mutex<Option<String>>,
    }

    impl MockClipboardProvider {
        fn new() -> Self {
            Self {
                clipboard: std::sync::Mutex::new(None),
                selection: std::sync::Mutex::new(None),
            }
        }
    }

    impl reovim_driver_clipboard::ClipboardProvider for MockClipboardProvider {
        fn clipboard_available(&self) -> bool {
            true
        }
        fn copy_to_clipboard(
            &self,
            text: &str,
        ) -> Result<(), reovim_driver_clipboard::ClipboardError> {
            *self.clipboard.lock().unwrap() = Some(text.to_string());
            Ok(())
        }
        fn paste_from_clipboard(
            &self,
        ) -> Result<Option<String>, reovim_driver_clipboard::ClipboardError> {
            Ok(self.clipboard.lock().unwrap().clone())
        }
        fn selection_available(&self) -> bool {
            true
        }
        fn copy_to_selection(
            &self,
            text: &str,
        ) -> Result<(), reovim_driver_clipboard::ClipboardError> {
            *self.selection.lock().unwrap() = Some(text.to_string());
            Ok(())
        }
        fn paste_from_selection(
            &self,
        ) -> Result<Option<String>, reovim_driver_clipboard::ClipboardError> {
            Ok(self.selection.lock().unwrap().clone())
        }
    }

    fn kernel_with_clipboard() -> (KernelContext, std::sync::Arc<MockClipboardProvider>) {
        let kernel = KernelContext::default();
        let registry = kernel.services.get_or_create::<ClipboardProviderRegistry>();
        let provider = std::sync::Arc::new(MockClipboardProvider::new());
        registry.register(ClipboardKey::Default, provider.clone());
        (kernel, provider)
    }

    #[test]
    fn test_sync_to_clipboard_with_provider() {
        let (kernel, provider) = kernel_with_clipboard();
        sync_to_clipboard(&kernel, "clipboard-text");
        assert_eq!(*provider.clipboard.lock().unwrap(), Some("clipboard-text".to_string()));
    }

    #[test]
    fn test_store_and_sync_system_with_provider() {
        let (kernel, provider) = kernel_with_clipboard();
        let mut registers = RegisterBank::new();
        assert!(store_and_sync(
            &kernel,
            &mut registers,
            Register::System,
            &content("clip-store")
        ));
        assert_eq!(*provider.clipboard.lock().unwrap(), Some("clip-store".to_string()));
    }

    #[test]
    fn test_all_history_registers_read_only() {
        let kernel = KernelContext::default();
        let mut registers = RegisterBank::new();
        for i in 0..=9_u8 {
            assert!(
                !store_and_sync(&kernel, &mut registers, Register::History(i), &content("test")),
                "history register {i} should be read-only"
            );
        }
    }

    // ========================================================================
    // char_to_register tests
    // ========================================================================

    #[test]
    fn test_char_to_register_default() {
        assert_eq!(char_to_register('"'), Some(Register::Default));
    }

    #[test]
    fn test_char_to_register_lowercase_slots() {
        assert_eq!(char_to_register('a'), Some(Register::Slot('a')));
        assert_eq!(char_to_register('m'), Some(Register::Slot('m')));
        assert_eq!(char_to_register('z'), Some(Register::Slot('z')));
    }

    #[test]
    fn test_char_to_register_uppercase_session() {
        assert_eq!(char_to_register('A'), Some(Register::Session('A')));
        assert_eq!(char_to_register('M'), Some(Register::Session('M')));
        assert_eq!(char_to_register('Z'), Some(Register::Session('Z')));
    }

    #[test]
    fn test_char_to_register_system_clipboard() {
        assert_eq!(char_to_register('+'), Some(Register::System));
        assert_eq!(char_to_register('*'), Some(Register::System));
    }

    #[test]
    fn test_char_to_register_history_digits() {
        for d in '0'..='9' {
            let expected_idx = (d as u8) - b'0';
            assert_eq!(
                char_to_register(d),
                Some(Register::History(expected_idx)),
                "digit '{d}' should map to History({expected_idx})"
            );
        }
    }

    #[test]
    fn test_char_to_register_unknown() {
        assert_eq!(char_to_register('!'), None);
        assert_eq!(char_to_register('@'), None);
        assert_eq!(char_to_register(' '), None);
        assert_eq!(char_to_register('\n'), None);
    }

    #[test]
    fn test_char_to_register_all_lowercase() {
        for c in 'a'..='z' {
            assert!(
                matches!(char_to_register(c), Some(Register::Slot(ch)) if ch == c),
                "'{c}' should be Slot('{c}')"
            );
        }
    }

    #[test]
    fn test_char_to_register_all_uppercase() {
        for c in 'A'..='Z' {
            assert!(
                matches!(char_to_register(c), Some(Register::Session(ch)) if ch == c),
                "'{c}' should be Session('{c}')"
            );
        }
    }

    // ========================================================================
    // option_char_to_register tests
    // ========================================================================

    #[test]
    fn test_option_char_to_register_none() {
        assert_eq!(option_char_to_register(None), Register::Default);
    }

    #[test]
    fn test_option_char_to_register_some_slot() {
        assert_eq!(option_char_to_register(Some('a')), Register::Slot('a'));
    }

    #[test]
    fn test_option_char_to_register_some_system() {
        assert_eq!(option_char_to_register(Some('+')), Register::System);
    }

    #[test]
    fn test_option_char_to_register_some_unknown() {
        // Unknown chars default to Register::Default
        assert_eq!(option_char_to_register(Some('!')), Register::Default);
    }

    #[test]
    fn test_option_char_to_register_some_session() {
        assert_eq!(option_char_to_register(Some('A')), Register::Session('A'));
    }

    #[test]
    fn test_option_char_to_register_some_history() {
        assert_eq!(option_char_to_register(Some('0')), Register::History(0));
        assert_eq!(option_char_to_register(Some('9')), Register::History(9));
    }

    #[test]
    fn test_option_char_to_register_some_default() {
        assert_eq!(option_char_to_register(Some('"')), Register::Default);
    }
}
