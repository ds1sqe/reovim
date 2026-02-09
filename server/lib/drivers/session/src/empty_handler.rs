//! Empty session handler mechanism.
//!
//! Provides the trait for handling empty sessions (no buffers).
//! Modules implement this trait to define policy for what happens
//! when a session starts with no buffers.
//!
//! # Linux Kernel Parallel
//!
//! Like a driver's `probe()` function that initializes a device,
//! `EmptySessionHandler::handle()` initializes an empty session.

use std::path::Path;

/// Action to take when session is empty.
///
/// Returned by [`EmptySessionHandler::handle()`] to indicate what
/// action the runner should take.
#[derive(Debug, Clone)]
pub enum EmptySessionAction {
    /// Create a buffer with the given name and content.
    ///
    /// - `name`: Buffer name (`None` = unnamed scratch buffer)
    /// - `content`: Initial content (empty string for blank buffer)
    CreateBuffer {
        /// Buffer name. `None` creates an unnamed scratch buffer.
        name: Option<String>,
        /// Initial buffer content.
        content: String,
    },

    /// Do nothing - defer to next handler.
    ///
    /// Return this to let a lower-priority handler decide.
    None,
}

/// Context provided to empty session handlers.
///
/// Contains read-only information about the session state.
/// Handlers cannot mutate state directly; they return an action
/// that the runner executes.
#[derive(Debug)]
pub struct EmptySessionContext<'a> {
    /// Session ID.
    pub session_id: u64,

    /// Command-line file arguments (files to open).
    /// If non-empty, handler should likely return `None`.
    pub file_args: &'a [String],

    /// Current working directory.
    pub cwd: &'a Path,
}

/// Handler for empty session state.
///
/// Modules implement this trait to define what happens when a session
/// has no buffers. The runner calls handlers in priority order until
/// one returns an action other than [`EmptySessionAction::None`].
///
/// # Priority Convention
///
/// - 0-50: Core handlers (system-level)
/// - 100: Default module priority
/// - 200+: Late/fallback handlers
///
/// # Example
///
/// ```ignore
/// use reovim_driver_session::{
///     EmptySessionHandler, EmptySessionContext, EmptySessionAction
/// };
///
/// pub struct ScratchBufferHandler;
///
/// impl EmptySessionHandler for ScratchBufferHandler {
///     fn handle(&self, _ctx: &EmptySessionContext) -> EmptySessionAction {
///         EmptySessionAction::CreateBuffer {
///             name: None,
///             content: String::new(),
///         }
///     }
///
///     fn priority(&self) -> u32 { 100 }
///
///     fn id(&self) -> &'static str { "defaults:scratch-buffer" }
///
///     fn description(&self) -> &'static str {
///         "Create empty scratch buffer on startup"
///     }
/// }
/// ```
pub trait EmptySessionHandler: Send + Sync + 'static {
    /// Handle empty session state.
    ///
    /// Called when session has no buffers. Return an action describing
    /// what should happen. Return [`EmptySessionAction::None`] to defer
    /// to next handler.
    fn handle(&self, ctx: &EmptySessionContext) -> EmptySessionAction;

    /// Handler priority (lower = called first).
    ///
    /// Default is 100 (standard module priority).
    fn priority(&self) -> u32 {
        100
    }

    /// Unique handler identifier.
    ///
    /// Format: `"module-name:handler-name"` (e.g., `"scratch-buffer:handler"`).
    fn id(&self) -> &'static str;

    /// Human-readable description for debugging/introspection.
    fn description(&self) -> &'static str;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify the trait is object-safe (can use `dyn EmptySessionHandler`).
    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn trait_is_object_safe() {
        struct TestHandler;

        impl EmptySessionHandler for TestHandler {
            fn handle(&self, _ctx: &EmptySessionContext) -> EmptySessionAction {
                EmptySessionAction::None
            }
            fn id(&self) -> &'static str {
                "test:handler"
            }
            fn description(&self) -> &'static str {
                "Test handler"
            }
        }

        // This compiles if trait is object-safe
        let _: Box<dyn EmptySessionHandler> = Box::new(TestHandler);
    }

    #[test]
    fn action_variants_constructible() {
        let create = EmptySessionAction::CreateBuffer {
            name: Some("test".to_string()),
            content: "hello".to_string(),
        };
        assert!(matches!(create, EmptySessionAction::CreateBuffer { .. }));

        let none = EmptySessionAction::None;
        assert!(matches!(none, EmptySessionAction::None));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn default_priority_is_100() {
        struct TestHandler;

        impl EmptySessionHandler for TestHandler {
            fn handle(&self, _ctx: &EmptySessionContext) -> EmptySessionAction {
                EmptySessionAction::None
            }
            fn id(&self) -> &'static str {
                "test:handler"
            }
            fn description(&self) -> &'static str {
                "Test handler"
            }
        }

        let handler = TestHandler;
        assert_eq!(handler.priority(), 100);
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn custom_priority() {
        struct HighPriorityHandler;

        impl EmptySessionHandler for HighPriorityHandler {
            fn handle(&self, _ctx: &EmptySessionContext) -> EmptySessionAction {
                EmptySessionAction::None
            }
            fn priority(&self) -> u32 {
                10
            }
            fn id(&self) -> &'static str {
                "test:high-priority"
            }
            fn description(&self) -> &'static str {
                "High priority handler"
            }
        }

        let handler = HighPriorityHandler;
        assert_eq!(handler.priority(), 10);
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn handler_creates_buffer_with_content() {
        struct ContentHandler;

        impl EmptySessionHandler for ContentHandler {
            fn handle(&self, _ctx: &EmptySessionContext) -> EmptySessionAction {
                EmptySessionAction::CreateBuffer {
                    name: Some("welcome.txt".to_string()),
                    content: "Welcome to reovim!".to_string(),
                }
            }
            fn id(&self) -> &'static str {
                "test:content"
            }
            fn description(&self) -> &'static str {
                "Content handler"
            }
        }

        let handler = ContentHandler;
        let ctx = EmptySessionContext {
            session_id: 1,
            file_args: &[],
            cwd: Path::new("/tmp"),
        };
        let action = handler.handle(&ctx);
        if let EmptySessionAction::CreateBuffer { name, content } = action {
            assert_eq!(name, Some("welcome.txt".to_string()));
            assert_eq!(content, "Welcome to reovim!");
        } else {
            panic!("expected CreateBuffer");
        }
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn handler_creates_unnamed_buffer() {
        struct ScratchHandler;

        impl EmptySessionHandler for ScratchHandler {
            fn handle(&self, _ctx: &EmptySessionContext) -> EmptySessionAction {
                EmptySessionAction::CreateBuffer {
                    name: None,
                    content: String::new(),
                }
            }
            fn id(&self) -> &'static str {
                "test:scratch"
            }
            fn description(&self) -> &'static str {
                "Scratch buffer handler"
            }
        }

        let handler = ScratchHandler;
        let ctx = EmptySessionContext {
            session_id: 1,
            file_args: &[],
            cwd: Path::new("/tmp"),
        };
        let action = handler.handle(&ctx);
        if let EmptySessionAction::CreateBuffer { name, content } = action {
            assert!(name.is_none());
            assert!(content.is_empty());
        } else {
            panic!("expected CreateBuffer");
        }
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn handler_with_file_args_returns_none() {
        struct ConditionalHandler;

        impl EmptySessionHandler for ConditionalHandler {
            fn handle(&self, ctx: &EmptySessionContext) -> EmptySessionAction {
                if ctx.file_args.is_empty() {
                    EmptySessionAction::CreateBuffer {
                        name: None,
                        content: String::new(),
                    }
                } else {
                    EmptySessionAction::None
                }
            }
            fn id(&self) -> &'static str {
                "test:conditional"
            }
            fn description(&self) -> &'static str {
                "Conditional handler"
            }
        }

        let handler = ConditionalHandler;

        // With file args, should return None
        let files = vec!["file.txt".to_string()];
        let ctx = EmptySessionContext {
            session_id: 1,
            file_args: &files,
            cwd: Path::new("/tmp"),
        };
        assert!(matches!(handler.handle(&ctx), EmptySessionAction::None));

        // Without file args, should create buffer
        let ctx = EmptySessionContext {
            session_id: 1,
            file_args: &[],
            cwd: Path::new("/tmp"),
        };
        assert!(matches!(handler.handle(&ctx), EmptySessionAction::CreateBuffer { .. }));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn handler_id_and_description() {
        struct TestHandler;

        impl EmptySessionHandler for TestHandler {
            fn handle(&self, _ctx: &EmptySessionContext) -> EmptySessionAction {
                EmptySessionAction::None
            }
            fn id(&self) -> &'static str {
                "test-module:my-handler"
            }
            fn description(&self) -> &'static str {
                "A test handler for unit tests"
            }
        }

        let handler = TestHandler;
        assert_eq!(handler.id(), "test-module:my-handler");
        assert_eq!(handler.description(), "A test handler for unit tests");
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn empty_session_context_debug() {
        let ctx = EmptySessionContext {
            session_id: 42,
            file_args: &[],
            cwd: Path::new("/home/user"),
        };
        let debug = format!("{ctx:?}");
        assert!(debug.contains("EmptySessionContext"));
        assert!(debug.contains("42"));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn empty_session_action_debug() {
        let action = EmptySessionAction::None;
        let debug = format!("{action:?}");
        assert!(debug.contains("None"));

        let action = EmptySessionAction::CreateBuffer {
            name: Some("test".to_string()),
            content: "hello".to_string(),
        };
        let debug = format!("{action:?}");
        assert!(debug.contains("CreateBuffer"));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn empty_session_action_clone() {
        let action = EmptySessionAction::CreateBuffer {
            name: Some("test".to_string()),
            content: "content".to_string(),
        };
        #[allow(clippy::redundant_clone)]
        let cloned = action.clone();
        if let EmptySessionAction::CreateBuffer { name, content } = cloned {
            assert_eq!(name, Some("test".to_string()));
            assert_eq!(content, "content");
        } else {
            panic!("expected CreateBuffer");
        }
    }

    #[test]
    fn empty_session_action_clone_none() {
        let action = EmptySessionAction::None;
        #[allow(clippy::redundant_clone)]
        let cloned = action.clone();
        assert!(matches!(cloned, EmptySessionAction::None));
    }

    #[test]
    fn empty_session_context_fields() {
        let files = vec!["a.rs".to_string(), "b.rs".to_string()];
        let ctx = EmptySessionContext {
            session_id: 99,
            file_args: &files,
            cwd: Path::new("/home/user/project"),
        };
        assert_eq!(ctx.session_id, 99);
        assert_eq!(ctx.file_args.len(), 2);
        assert_eq!(ctx.file_args[0], "a.rs");
        assert_eq!(ctx.cwd, Path::new("/home/user/project"));
    }

    #[test]
    fn empty_session_context_empty_file_args() {
        let ctx = EmptySessionContext {
            session_id: 1,
            file_args: &[],
            cwd: Path::new("/"),
        };
        assert!(ctx.file_args.is_empty());
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn handler_multiple_on_same_context() {
        struct FirstHandler;
        impl EmptySessionHandler for FirstHandler {
            fn handle(&self, _ctx: &EmptySessionContext) -> EmptySessionAction {
                EmptySessionAction::None
            }
            fn priority(&self) -> u32 {
                50
            }
            fn id(&self) -> &'static str {
                "test:first"
            }
            fn description(&self) -> &'static str {
                "First handler"
            }
        }

        struct SecondHandler;
        impl EmptySessionHandler for SecondHandler {
            fn handle(&self, _ctx: &EmptySessionContext) -> EmptySessionAction {
                EmptySessionAction::CreateBuffer {
                    name: None,
                    content: String::new(),
                }
            }
            fn priority(&self) -> u32 {
                100
            }
            fn id(&self) -> &'static str {
                "test:second"
            }
            fn description(&self) -> &'static str {
                "Second handler"
            }
        }

        let ctx = EmptySessionContext {
            session_id: 1,
            file_args: &[],
            cwd: Path::new("/tmp"),
        };

        let first = FirstHandler;
        let second = SecondHandler;

        // First returns None, second returns CreateBuffer
        assert!(matches!(first.handle(&ctx), EmptySessionAction::None));
        assert!(matches!(second.handle(&ctx), EmptySessionAction::CreateBuffer { .. }));

        // Priority ordering
        assert!(first.priority() < second.priority());
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn handler_with_zero_priority() {
        struct ZeroPriority;
        impl EmptySessionHandler for ZeroPriority {
            fn handle(&self, _ctx: &EmptySessionContext) -> EmptySessionAction {
                EmptySessionAction::None
            }
            fn priority(&self) -> u32 {
                0
            }
            fn id(&self) -> &'static str {
                "test:zero"
            }
            fn description(&self) -> &'static str {
                "Zero priority"
            }
        }

        let handler = ZeroPriority;
        assert_eq!(handler.priority(), 0);
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn create_buffer_with_none_name_and_content() {
        let action = EmptySessionAction::CreateBuffer {
            name: None,
            content: "some content here".to_string(),
        };
        if let EmptySessionAction::CreateBuffer { name, content } = action {
            assert!(name.is_none());
            assert!(!content.is_empty());
        } else {
            panic!("expected CreateBuffer");
        }
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn handler_send_sync_bounds() {
        fn assert_send_sync<T: Send + Sync + 'static>() {}

        struct StaticHandler;
        impl EmptySessionHandler for StaticHandler {
            fn handle(&self, _ctx: &EmptySessionContext) -> EmptySessionAction {
                EmptySessionAction::None
            }
            fn id(&self) -> &'static str {
                "test:static"
            }
            fn description(&self) -> &'static str {
                "Static handler"
            }
        }

        assert_send_sync::<StaticHandler>();
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn handler_uses_cwd_from_context() {
        struct CwdHandler;
        impl EmptySessionHandler for CwdHandler {
            fn handle(&self, ctx: &EmptySessionContext) -> EmptySessionAction {
                EmptySessionAction::CreateBuffer {
                    name: Some(ctx.cwd.display().to_string()),
                    content: String::new(),
                }
            }
            fn id(&self) -> &'static str {
                "test:cwd"
            }
            fn description(&self) -> &'static str {
                "CWD handler"
            }
        }

        let handler = CwdHandler;
        let ctx = EmptySessionContext {
            session_id: 1,
            file_args: &[],
            cwd: Path::new("/home/user/project"),
        };
        let action = handler.handle(&ctx);
        if let EmptySessionAction::CreateBuffer { name, .. } = action {
            assert_eq!(name, Some("/home/user/project".to_string()));
        } else {
            panic!("expected CreateBuffer");
        }
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn handler_uses_session_id_from_context() {
        struct SessionIdHandler;
        impl EmptySessionHandler for SessionIdHandler {
            fn handle(&self, ctx: &EmptySessionContext) -> EmptySessionAction {
                EmptySessionAction::CreateBuffer {
                    name: Some(format!("session-{}", ctx.session_id)),
                    content: String::new(),
                }
            }
            fn id(&self) -> &'static str {
                "test:session-id"
            }
            fn description(&self) -> &'static str {
                "Session ID handler"
            }
        }

        let handler = SessionIdHandler;
        let ctx = EmptySessionContext {
            session_id: 42,
            file_args: &[],
            cwd: Path::new("/tmp"),
        };
        let action = handler.handle(&ctx);
        if let EmptySessionAction::CreateBuffer { name, .. } = action {
            assert_eq!(name, Some("session-42".to_string()));
        } else {
            panic!("expected CreateBuffer");
        }
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn empty_session_context_debug_with_file_args() {
        let files = vec!["file1.rs".to_string(), "file2.rs".to_string()];
        let ctx = EmptySessionContext {
            session_id: 10,
            file_args: &files,
            cwd: Path::new("/tmp"),
        };
        let debug = format!("{ctx:?}");
        assert!(debug.contains("10"));
        assert!(debug.contains("file1.rs"));
        assert!(debug.contains("file2.rs"));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn handler_with_max_priority() {
        struct MaxPriorityHandler;
        impl EmptySessionHandler for MaxPriorityHandler {
            fn handle(&self, _ctx: &EmptySessionContext) -> EmptySessionAction {
                EmptySessionAction::None
            }
            fn priority(&self) -> u32 {
                u32::MAX
            }
            fn id(&self) -> &'static str {
                "test:max-priority"
            }
            fn description(&self) -> &'static str {
                "Max priority handler"
            }
        }

        let handler = MaxPriorityHandler;
        assert_eq!(handler.priority(), u32::MAX);
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn create_buffer_action_debug_with_none_name() {
        let action = EmptySessionAction::CreateBuffer {
            name: None,
            content: String::new(),
        };
        let debug = format!("{action:?}");
        assert!(debug.contains("CreateBuffer"));
        assert!(debug.contains("None"));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn handler_description_is_not_empty() {
        struct DescribedHandler;
        impl EmptySessionHandler for DescribedHandler {
            fn handle(&self, _ctx: &EmptySessionContext) -> EmptySessionAction {
                EmptySessionAction::None
            }
            fn id(&self) -> &'static str {
                "test:described"
            }
            fn description(&self) -> &'static str {
                "A handler with a description"
            }
        }

        let handler = DescribedHandler;
        assert!(!handler.description().is_empty());
        assert!(handler.description().contains("description"));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn handler_id_follows_naming_convention() {
        struct WellNamedHandler;
        impl EmptySessionHandler for WellNamedHandler {
            fn handle(&self, _ctx: &EmptySessionContext) -> EmptySessionAction {
                EmptySessionAction::None
            }
            fn id(&self) -> &'static str {
                "module:handler-name"
            }
            fn description(&self) -> &'static str {
                "Well named handler"
            }
        }

        let handler = WellNamedHandler;
        assert!(handler.id().contains(':'));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn clone_create_buffer_with_long_content() {
        let action = EmptySessionAction::CreateBuffer {
            name: Some("long-content".to_string()),
            content: "a".repeat(10000),
        };
        #[allow(clippy::redundant_clone)]
        let cloned = action.clone();
        if let EmptySessionAction::CreateBuffer { name, content } = cloned {
            assert_eq!(name, Some("long-content".to_string()));
            assert_eq!(content.len(), 10000);
        } else {
            panic!("expected CreateBuffer");
        }
    }
}
