//! Scratch buffer handler for reovim.
//!
//! Provides [`ScratchBufferHandler`] which creates an empty buffer
//! when a session starts with no buffers.
//!
//! # Linux Kernel Parallel
//!
//! Like a driver's `probe()` function that initializes hardware,
//! this handler initializes the editor with a usable state when
//! no files are specified.
//!
//! # Architecture
//!
//! This module follows the mechanism/policy separation:
//! - **Mechanism**: `EmptySessionHandler` trait (in `reovim-driver-session`)
//! - **Policy**: `ScratchBufferHandler` (this module) decides to create a buffer

use std::sync::Arc;

use {
    reovim_driver_session::{
        EmptySessionAction, EmptySessionContext, EmptySessionHandler, SessionHandlerKey,
        SessionHandlerRegistry,
    },
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
};

/// Handler that creates an empty scratch buffer.
///
/// When a session starts with no buffers and no file arguments,
/// this handler creates an unnamed scratch buffer so the user
/// has something to edit immediately.
///
/// Returns [`EmptySessionAction::None`] if files were specified
/// on the command line, allowing the file opener to handle those.
///
/// # Priority
///
/// Uses the default priority (100), allowing core handlers (0-50)
/// to take precedence if needed.
pub struct ScratchBufferHandler;

impl EmptySessionHandler for ScratchBufferHandler {
    fn handle(&self, ctx: &EmptySessionContext) -> EmptySessionAction {
        // If files were specified on command line, don't create scratch buffer
        // The file opener will handle opening those files
        if !ctx.file_args.is_empty() {
            return EmptySessionAction::None;
        }

        // Create an empty scratch buffer
        EmptySessionAction::CreateBuffer {
            name: None, // Unnamed scratch buffer
            content: String::new(),
        }
    }

    fn priority(&self) -> u32 {
        100 // Default module priority
    }

    fn id(&self) -> &'static str {
        "scratch-buffer:handler"
    }

    fn description(&self) -> &'static str {
        "Create empty scratch buffer on startup"
    }
}

// ============================================================================
// Module trait implementation
// ============================================================================

/// Scratch buffer module instance.
///
/// Provides the `ScratchBufferHandler` for empty session handling.
pub struct ScratchBufferModule;

impl ScratchBufferModule {
    /// Create a new scratch buffer module.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for ScratchBufferModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for ScratchBufferModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("scratch-buffer")
    }

    fn name(&self) -> &'static str {
        "Scratch Buffer"
    }

    fn version(&self) -> Version {
        Version::new(0, 9, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // Register empty session handler with typed key (Epic #417)
        let handler_registry = ctx.services.get_or_create::<SessionHandlerRegistry>();
        handler_registry.register(SessionHandlerKey::Empty, Arc::new(ScratchBufferHandler));

        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

// Generate FFI entry points for dynamic loading (only when building standalone cdylib)
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(ScratchBufferModule);

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn test_creates_buffer_when_no_files() {
        let handler = ScratchBufferHandler;
        let ctx = EmptySessionContext {
            session_id: 1,
            file_args: &[],
            cwd: Path::new("/home/user"),
        };

        let action = handler.handle(&ctx);
        assert!(matches!(action, EmptySessionAction::CreateBuffer { name: None, .. }));
    }

    #[test]
    fn test_buffer_content_is_empty() {
        let handler = ScratchBufferHandler;
        let ctx = EmptySessionContext {
            session_id: 1,
            file_args: &[],
            cwd: Path::new("/home/user"),
        };

        let action = handler.handle(&ctx);
        if let EmptySessionAction::CreateBuffer { content, .. } = action {
            assert!(content.is_empty());
        } else {
            panic!("Expected CreateBuffer action");
        }
    }

    #[test]
    fn test_defers_when_files_specified() {
        let handler = ScratchBufferHandler;
        let files = vec!["file.txt".to_string()];
        let ctx = EmptySessionContext {
            session_id: 1,
            file_args: &files,
            cwd: Path::new("/home/user"),
        };

        let action = handler.handle(&ctx);
        assert!(matches!(action, EmptySessionAction::None));
    }

    #[test]
    fn test_defers_when_multiple_files_specified() {
        let handler = ScratchBufferHandler;
        let files = vec!["a.txt".to_string(), "b.txt".to_string()];
        let ctx = EmptySessionContext {
            session_id: 1,
            file_args: &files,
            cwd: Path::new("/home/user"),
        };

        let action = handler.handle(&ctx);
        assert!(matches!(action, EmptySessionAction::None));
    }

    #[test]
    fn test_default_priority_is_100() {
        let handler = ScratchBufferHandler;
        assert_eq!(handler.priority(), 100);
    }

    #[test]
    fn test_handler_id() {
        let handler = ScratchBufferHandler;
        assert_eq!(handler.id(), "scratch-buffer:handler");
    }

    #[test]
    fn test_handler_description() {
        let handler = ScratchBufferHandler;
        assert_eq!(handler.description(), "Create empty scratch buffer on startup");
    }

    #[test]
    fn test_module_id() {
        let module = ScratchBufferModule::new();
        assert_eq!(module.id().as_str(), "scratch-buffer");
    }

    #[test]
    fn test_module_name() {
        let module = ScratchBufferModule::new();
        assert_eq!(module.name(), "Scratch Buffer");
    }

    #[test]
    fn test_module_version() {
        let module = ScratchBufferModule::new();
        let version = module.version();
        assert_eq!(version.major, 0);
        assert_eq!(version.minor, 9);
        assert_eq!(version.patch, 0);
    }

    #[test]
    fn test_module_default() {
        let from_new = ScratchBufferModule::new();
        let from_default = ScratchBufferModule;
        assert_eq!(from_new.id(), from_default.id());
    }

    #[test]
    fn test_exit_succeeds() {
        let mut module = ScratchBufferModule::new();
        assert!(module.exit().is_ok());
    }

    #[test]
    fn test_buffer_name_is_none() {
        let handler = ScratchBufferHandler;
        let ctx = EmptySessionContext {
            session_id: 42,
            file_args: &[],
            cwd: Path::new("/tmp"),
        };

        let action = handler.handle(&ctx);
        if let EmptySessionAction::CreateBuffer { name, .. } = action {
            assert!(name.is_none());
        } else {
            panic!("Expected CreateBuffer action");
        }
    }

    #[test]
    fn test_different_session_ids_produce_same_action() {
        let handler = ScratchBufferHandler;

        let ctx1 = EmptySessionContext {
            session_id: 1,
            file_args: &[],
            cwd: Path::new("/home/user"),
        };
        let ctx2 = EmptySessionContext {
            session_id: 999,
            file_args: &[],
            cwd: Path::new("/other"),
        };

        let action1 = handler.handle(&ctx1);
        let action2 = handler.handle(&ctx2);

        assert!(matches!(action1, EmptySessionAction::CreateBuffer { .. }));
        assert!(matches!(action2, EmptySessionAction::CreateBuffer { .. }));
    }

    #[test]
    fn test_dependencies_default_empty() {
        let module = ScratchBufferModule::new();
        assert!(module.dependencies().is_empty());
    }

    #[test]
    fn test_init_registers_session_handler() {
        use {
            reovim_kernel::api::v1::{KernelContext, ModuleContext, ServiceRegistry},
            std::{path::PathBuf, sync::Arc},
        };

        let kernel = KernelContext::default();
        let services = Arc::new(ServiceRegistry::new());
        let ctx = ModuleContext::new(
            kernel,
            services.clone(),
            PathBuf::from("/tmp/test-data"),
            PathBuf::from("/tmp/test-cache"),
        );

        let mut module = ScratchBufferModule::new();
        let result = module.init(&ctx);
        assert_eq!(result, ProbeResult::Success);

        // Verify that SessionHandlerRegistry was created in services
        let registry = services.get::<SessionHandlerRegistry>();
        assert!(registry.is_some(), "SessionHandlerRegistry should be registered in services");
    }
}
