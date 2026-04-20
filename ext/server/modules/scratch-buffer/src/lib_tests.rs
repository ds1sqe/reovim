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
#[cfg_attr(coverage_nightly, coverage(off))]
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
    fn create_default<T: Default>() -> T {
        T::default()
    }
    let from_default: ScratchBufferModule = create_default();
    let from_new = ScratchBufferModule::new();
    assert_eq!(from_new.id(), from_default.id());
}

#[test]
fn test_exit_succeeds() {
    let mut module = ScratchBufferModule::new();
    assert!(module.exit().is_ok());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
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
