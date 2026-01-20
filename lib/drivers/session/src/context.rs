//! Session context for command and resolver execution.
//!
//! This module provides [`SessionContext`], which gives commands and resolvers
//! full access to per-client session state. This is the primary interface
//! for commands to interact with the session.
//!
//! # Design
//!
//! - **Session**: Full per-client state access
//! - **Extensions**: Module-provided policy state via `ext::<T>()`
//! - **Windows**: Window layout and active window
//! - **Mode**: Current mode stack
//!
//! # Example
//!
//! ```ignore
//! use reovim_driver_session::SessionContext;
//!
//! fn execute_command(ctx: &mut SessionContext) {
//!     // Access vim-specific state via extension
//!     let vim = ctx.ext_mut::<VimSessionState>();
//!     vim.pending_count = Some(5);
//!
//!     // Access current mode
//!     let mode = ctx.current_mode();
//!
//!     // Access windows
//!     if let Some(window) = ctx.session.windows.active() {
//!         println!("Active window: {:?}", window.id);
//!     }
//! }
//! ```

use {
    crate::{extension::SessionExtension, types::Session},
    reovim_kernel::api::v1::ModeId,
};

/// Context passed to commands, resolvers, and mode lifecycle hooks.
///
/// Provides full access to session state, including:
/// - The session itself (windows, mode stack, pending keys)
/// - Module extensions via type-safe `ext::<T>()` methods
///
/// # Lifetime
///
/// The context borrows the session mutably for the duration of command
/// execution. This ensures exclusive access and prevents data races.
///
/// # Thread Safety
///
/// `SessionContext` is not `Send` or `Sync` by design - it's meant for
/// single-threaded command execution within a session.
#[derive(Debug)]
pub struct SessionContext<'a> {
    /// Full session access.
    pub session: &'a mut Session,
}

impl<'a> SessionContext<'a> {
    /// Create a new session context.
    #[must_use]
    pub const fn new(session: &'a mut Session) -> Self {
        Self { session }
    }

    /// Get extension by type (immutable).
    ///
    /// Returns `None` if the extension hasn't been inserted yet.
    ///
    /// # Example
    ///
    /// ```ignore
    /// if let Some(vim) = ctx.ext::<VimSessionState>() {
    ///     println!("Pending count: {:?}", vim.pending_count);
    /// }
    /// ```
    #[must_use]
    pub fn ext<T: SessionExtension>(&self) -> Option<&T> {
        self.session.extensions.get::<T>()
    }

    /// Get extension by type (mutable), creating if needed.
    ///
    /// If the extension doesn't exist, it will be created using `T::create()`.
    /// This is the primary way modules access their per-session state.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let vim = ctx.ext_mut::<VimSessionState>();
    /// vim.pending_operator = Some(operators::DELETE);
    /// ```
    pub fn ext_mut<T: SessionExtension>(&mut self) -> &mut T {
        self.session.extensions.get_or_insert::<T>()
    }

    /// Get the current mode.
    ///
    /// Returns the mode at the top of the mode stack.
    #[must_use]
    pub fn current_mode(&self) -> &ModeId {
        self.session.mode_stack.current()
    }

    /// Get the home mode.
    ///
    /// The home mode is the bottom of the mode stack and cannot be popped.
    #[must_use]
    pub fn home_mode(&self) -> &ModeId {
        self.session.mode_stack.home()
    }

    /// Get the mode stack depth.
    ///
    /// Returns the number of modes on the stack (minimum 1 for home mode).
    #[must_use]
    pub const fn mode_depth(&self) -> usize {
        self.session.mode_stack.depth()
    }

    /// Check if a mode is on the stack.
    ///
    /// Returns `true` if the specified mode is anywhere in the mode stack.
    #[must_use]
    pub fn is_mode_active(&self, mode_id: &ModeId) -> bool {
        self.session.mode_stack.contains(mode_id)
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::types::SessionId, reovim_kernel::api::v1::ModuleId};

    fn test_mode() -> ModeId {
        ModeId::new(ModuleId::new("test"), "normal")
    }

    #[test]
    fn test_session_context_new() {
        let mut session = Session::new(SessionId::new(1), test_mode());
        let ctx = SessionContext::new(&mut session);

        assert_eq!(ctx.session.id.as_usize(), 1);
    }

    #[test]
    fn test_session_context_current_mode() {
        let mode = test_mode();
        let mut session = Session::new(SessionId::new(1), mode.clone());
        let ctx = SessionContext::new(&mut session);

        assert_eq!(ctx.current_mode(), &mode);
    }

    #[test]
    fn test_session_context_home_mode() {
        let mode = test_mode();
        let mut session = Session::new(SessionId::new(1), mode.clone());
        let ctx = SessionContext::new(&mut session);

        assert_eq!(ctx.home_mode(), &mode);
    }

    #[test]
    fn test_session_context_mode_depth() {
        let mode = test_mode();
        let mut session = Session::new(SessionId::new(1), mode);
        let ctx = SessionContext::new(&mut session);

        assert_eq!(ctx.mode_depth(), 1);
    }

    // Test extension for testing
    #[derive(Debug, Default)]
    struct TestExtension {
        value: i32,
    }

    impl SessionExtension for TestExtension {
        fn create() -> Self {
            Self { value: 42 }
        }
    }

    #[test]
    fn test_session_context_ext_none() {
        let mode = test_mode();
        let mut session = Session::new(SessionId::new(1), mode);
        let ctx = SessionContext::new(&mut session);

        assert!(ctx.ext::<TestExtension>().is_none());
    }

    #[test]
    fn test_session_context_ext_mut_creates() {
        let mode = test_mode();
        let mut session = Session::new(SessionId::new(1), mode);
        let mut ctx = SessionContext::new(&mut session);

        let ext = ctx.ext_mut::<TestExtension>();
        assert_eq!(ext.value, 42); // Default from create()
    }

    #[test]
    fn test_session_context_ext_after_ext_mut() {
        let mode = test_mode();
        let mut session = Session::new(SessionId::new(1), mode);
        {
            let mut ctx = SessionContext::new(&mut session);
            ctx.ext_mut::<TestExtension>().value = 100;
        }
        {
            let ctx = SessionContext::new(&mut session);
            assert_eq!(ctx.ext::<TestExtension>().unwrap().value, 100);
        }
    }

    #[test]
    fn test_session_context_is_mode_active() {
        let mode = test_mode();
        // Use different discriminant for the other mode
        let other_mode = ModeId::with_discriminant(ModuleId::new("test"), "insert", 1);
        let mut session = Session::new(SessionId::new(1), mode.clone());
        let ctx = SessionContext::new(&mut session);

        assert!(ctx.is_mode_active(&mode));
        assert!(!ctx.is_mode_active(&other_mode));
    }
}
