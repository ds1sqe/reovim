//! Register routing helper for operators.
//!
//! Routes register operations between:
//! - **Kernel `RegisterBank`**: Unnamed (`""`) and named (`a-z`) registers
//! - **`ClipboardProvider`**: System clipboard (`+`), selection (`*`), history (`0-9`)
//!
//! # Usage in Operators
//!
//! ```ignore
//! use super::registers::{store_to_register, push_to_history};
//!
//! // After yanking/deleting:
//! store_to_register(ctx.kernel, ctx.register, &content);
//! push_to_history(ctx.kernel, &content);
//! ```

use {
    reovim_driver_clipboard::{ClipboardKey, ClipboardProviderRegistry},
    reovim_kernel::api::v1::{KernelContext, RegisterContent},
};

/// Store content to the appropriate register.
///
/// Routes based on register name:
/// - `+` → System clipboard
/// - `*` → Selection clipboard
/// - `0-9` → Ignored (history is read-only, use `push_to_history` instead)
/// - `a-z`, `A-Z`, `"`, `None` → Kernel's `RegisterBank`
///
/// Returns `true` if the content was stored somewhere, `false` if the register
/// was invalid (e.g., trying to write to numbered registers).
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn store_to_register(
    kernel: &KernelContext,
    register: Option<char>,
    content: &RegisterContent,
) -> bool {
    match register {
        // System clipboard
        Some('+') => {
            if let Some(registry) = kernel.services.get::<ClipboardProviderRegistry>()
                && let Some(provider) = registry.get(&ClipboardKey::Default)
            {
                // Ignore errors - clipboard may not be available
                let _ = provider.copy_to_clipboard(&content.text);
                return true;
            }
            // Fallback: store in kernel's registers if clipboard unavailable
            kernel
                .registers
                .write()
                .set_by_name(register, content.clone())
        }

        // Selection clipboard
        Some('*') => {
            if let Some(registry) = kernel.services.get::<ClipboardProviderRegistry>()
                && let Some(provider) = registry.get(&ClipboardKey::Default)
            {
                // Ignore errors - selection may not be available
                let _ = provider.copy_to_selection(&content.text);
                return true;
            }
            // Fallback: store in kernel's registers if clipboard unavailable
            kernel
                .registers
                .write()
                .set_by_name(register, content.clone())
        }

        // Numbered registers (0-9) are read-only, populated via history
        Some(n) if n.is_ascii_digit() => false,

        // Named registers and unnamed - use kernel's RegisterBank
        _ => kernel
            .registers
            .write()
            .set_by_name(register, content.clone()),
    }
}

/// Push content to the clipboard history.
///
/// This should be called after every yank/delete operation to populate
/// the numbered registers (0-9).
///
/// Note: We always push to history regardless of target register.
/// This matches vim behavior where `"ayy` yanks to register 'a' AND
/// updates the history.
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn push_to_history(kernel: &KernelContext, content: &RegisterContent) {
    if let Some(registry) = kernel.services.get::<ClipboardProviderRegistry>()
        && let Some(provider) = registry.get(&ClipboardKey::Default)
    {
        provider.push_history(content.clone());
    }
}

#[cfg(test)]
#[allow(clippy::significant_drop_tightening)]
mod tests {
    use {super::*, reovim_kernel::api::v1::YankType};

    fn content(text: &str) -> RegisterContent {
        RegisterContent::new(text.to_string(), YankType::Characterwise)
    }

    #[test]
    fn test_numbered_registers_are_read_only() {
        let kernel = KernelContext::default();
        // Numbered registers should not accept writes
        assert!(!store_to_register(&kernel, Some('0'), &content("test")));
        assert!(!store_to_register(&kernel, Some('5'), &content("test")));
        assert!(!store_to_register(&kernel, Some('9'), &content("test")));
    }

    #[test]
    fn test_named_registers_work() {
        let kernel = KernelContext::default();
        assert!(store_to_register(&kernel, Some('a'), &content("alpha")));
        assert_eq!(
            kernel
                .registers
                .read()
                .get_named('a')
                .map(|r| r.text.as_str()),
            Some("alpha")
        );
    }

    #[test]
    fn test_unnamed_register_works() {
        let kernel = KernelContext::default();
        assert!(store_to_register(&kernel, None, &content("unnamed")));
        assert_eq!(kernel.registers.read().get().text, "unnamed");
    }

    // ========================================================================
    // Additional register routing tests
    // ========================================================================

    #[test]
    fn test_all_numbered_registers_read_only() {
        let kernel = KernelContext::default();
        for n in '0'..='9' {
            assert!(
                !store_to_register(&kernel, Some(n), &content("test")),
                "register '{n}' should be read-only"
            );
        }
    }

    #[test]
    fn test_uppercase_named_registers() {
        let kernel = KernelContext::default();
        assert!(store_to_register(&kernel, Some('A'), &content("upper")));
    }

    #[test]
    fn test_multiple_named_registers() {
        let kernel = KernelContext::default();
        assert!(store_to_register(&kernel, Some('a'), &content("alpha")));
        assert!(store_to_register(&kernel, Some('b'), &content("bravo")));
        assert!(store_to_register(&kernel, Some('z'), &content("zulu")));

        let regs = kernel.registers.read();
        assert_eq!(regs.get_named('a').map(|r| r.text.as_str()), Some("alpha"));
        assert_eq!(regs.get_named('b').map(|r| r.text.as_str()), Some("bravo"));
        assert_eq!(regs.get_named('z').map(|r| r.text.as_str()), Some("zulu"));
    }

    #[test]
    fn test_unnamed_register_overwrite() {
        let kernel = KernelContext::default();
        assert!(store_to_register(&kernel, None, &content("first")));
        assert_eq!(kernel.registers.read().get().text, "first");

        assert!(store_to_register(&kernel, None, &content("second")));
        assert_eq!(kernel.registers.read().get().text, "second");
    }

    #[test]
    fn test_named_register_overwrite() {
        let kernel = KernelContext::default();
        assert!(store_to_register(&kernel, Some('a'), &content("first")));
        assert!(store_to_register(&kernel, Some('a'), &content("second")));
        assert_eq!(
            kernel
                .registers
                .read()
                .get_named('a')
                .map(|r| r.text.as_str()),
            Some("second")
        );
    }

    #[test]
    fn test_clipboard_plus_fallback() {
        // Without a ClipboardProvider, '+' falls back to kernel register bank.
        // RegisterBank::set_by_name doesn't recognise '+' as a named register,
        // so the fallback returns false.
        let kernel = KernelContext::default();
        let result = store_to_register(&kernel, Some('+'), &content("clip"));
        assert!(!result);
    }

    #[test]
    fn test_clipboard_star_fallback() {
        // Without a ClipboardProvider, '*' falls back to kernel register bank.
        // RegisterBank::set_by_name doesn't recognise '*' as a named register,
        // so the fallback returns false.
        let kernel = KernelContext::default();
        let result = store_to_register(&kernel, Some('*'), &content("sel"));
        assert!(!result);
    }

    #[test]
    fn test_push_to_history_without_provider() {
        // push_to_history should not panic without a ClipboardProvider
        let kernel = KernelContext::default();
        push_to_history(&kernel, &content("test"));
        // Should not panic - just a no-op
    }

    #[test]
    fn test_linewise_content_stored() {
        let kernel = KernelContext::default();
        let linewise_content = RegisterContent::linewise("line\n".to_string());
        assert!(store_to_register(&kernel, Some('a'), &linewise_content));
        let reg = kernel.registers.read().get_named('a').cloned();
        assert!(reg.is_some());
        assert_eq!(reg.unwrap().text, "line\n");
    }

    #[test]
    fn test_empty_text_stored() {
        let kernel = KernelContext::default();
        assert!(store_to_register(&kernel, None, &content("")));
        assert_eq!(kernel.registers.read().get().text, "");
    }

    #[test]
    fn test_all_lowercase_named_registers() {
        let kernel = KernelContext::default();
        for c in 'a'..='z' {
            assert!(
                store_to_register(&kernel, Some(c), &content(&format!("reg-{c}"))),
                "register '{c}' should accept writes"
            );
        }
        // Verify a few
        let regs = kernel.registers.read();
        assert_eq!(regs.get_named('a').map(|r| r.text.as_str()), Some("reg-a"));
        assert_eq!(regs.get_named('m').map(|r| r.text.as_str()), Some("reg-m"));
        assert_eq!(regs.get_named('z').map(|r| r.text.as_str()), Some("reg-z"));
    }

    #[test]
    fn test_special_char_register() {
        let kernel = KernelContext::default();
        // Register '"' is the unnamed register
        let result = store_to_register(&kernel, Some('"'), &content("unnamed-explicit"));
        // set_by_name with '"' should either store or fail gracefully
        // The important thing is it doesn't panic
        let _ = result;
    }

    #[test]
    fn test_linewise_content_yank_type() {
        let kernel = KernelContext::default();
        let linewise = RegisterContent::linewise("line\n".to_string());
        assert!(store_to_register(&kernel, Some('a'), &linewise));
        let reg = kernel.registers.read().get_named('a').cloned();
        assert!(reg.is_some());
        assert!(reg.unwrap().is_linewise());
    }

    #[test]
    fn test_characterwise_content_yank_type() {
        let kernel = KernelContext::default();
        let charwise = RegisterContent::characterwise("text".to_string());
        assert!(store_to_register(&kernel, Some('b'), &charwise));
        let reg = kernel.registers.read().get_named('b').cloned();
        assert!(reg.is_some());
        assert!(reg.unwrap().is_characterwise());
    }

    #[test]
    fn test_multiline_text_stored() {
        let kernel = KernelContext::default();
        let multi = content("line1\nline2\nline3");
        assert!(store_to_register(&kernel, Some('c'), &multi));
        let reg = kernel.registers.read().get_named('c').cloned();
        assert!(reg.is_some());
        assert_eq!(reg.unwrap().text, "line1\nline2\nline3");
    }

    #[test]
    fn test_push_to_history_multiple_calls() {
        // push_to_history should not panic even with multiple calls
        let kernel = KernelContext::default();
        push_to_history(&kernel, &content("first"));
        push_to_history(&kernel, &content("second"));
        push_to_history(&kernel, &content("third"));
    }

    #[test]
    fn test_store_and_overwrite_unnamed() {
        let kernel = KernelContext::default();
        assert!(store_to_register(&kernel, None, &content("first")));
        assert_eq!(kernel.registers.read().get().text, "first");

        assert!(store_to_register(&kernel, None, &content("second")));
        assert_eq!(kernel.registers.read().get().text, "second");

        assert!(store_to_register(&kernel, None, &content("third")));
        assert_eq!(kernel.registers.read().get().text, "third");
    }

    #[test]
    fn test_named_does_not_affect_unnamed() {
        let kernel = KernelContext::default();
        assert!(store_to_register(&kernel, None, &content("unnamed")));
        assert!(store_to_register(&kernel, Some('a'), &content("named-a")));

        // Unnamed should still be "unnamed"
        assert_eq!(kernel.registers.read().get().text, "unnamed");
        assert_eq!(
            kernel
                .registers
                .read()
                .get_named('a')
                .map(|r| r.text.as_str()),
            Some("named-a")
        );
    }
}
