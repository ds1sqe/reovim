//! Register routing helper for operators.
//!
//! Routes register operations between:
//! - **Kernel RegisterBank**: Unnamed (`""`) and named (`a-z`) registers
//! - **ClipboardProvider**: System clipboard (`+`), selection (`*`), history (`0-9`)
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
/// - `a-z`, `A-Z`, `"`, `None` → Kernel's RegisterBank
///
/// Returns `true` if the content was stored somewhere, `false` if the register
/// was invalid (e.g., trying to write to numbered registers).
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
pub fn push_to_history(kernel: &KernelContext, content: &RegisterContent) {
    if let Some(registry) = kernel.services.get::<ClipboardProviderRegistry>()
        && let Some(provider) = registry.get(&ClipboardKey::Default)
    {
        provider.push_history(content.clone());
    }
}

#[cfg(test)]
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
}
