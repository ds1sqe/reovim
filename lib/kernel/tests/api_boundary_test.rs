//! API boundary enforcement tests.
//!
//! Verifies that modules can ONLY import from `api::*`.
//! Kernel internals are `pub(crate)` and cannot be accessed externally.
//!
//! # Design Principle
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                      KERNEL                                 │
//! │  ┌───────────────────────────────────────────────────────┐  │
//! │  │  Internal (PRIVATE - pub(crate))                      │  │
//! │  │  sched/, mm/, ipc/, internal/                         │  │
//! │  └───────────────────────────────────────────────────────┘  │
//! │                          │                                  │
//! │  ┌───────────────────────▼───────────────────────────────┐  │
//! │  │  ████████████ pub mod api ████████████████████████    │  │
//! │  │  KernelContext   mode::*   module::*                  │  │
//! │  └───────────────────────────────────────────────────────┘  │
//! └─────────────────────────────────────────────────────────────┘
//!                            │
//!                            │ ONLY api::*
//!                            ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │  MODULES                                                    │
//! │  use reovim_kernel::api::{Mode, ModeId, CommandId, ...};    │
//! │  // Cannot import kernel internals - won't compile          │
//! └─────────────────────────────────────────────────────────────┘
//! ```

// This SHOULD compile - accessing the public API
use reovim_kernel::api::v1::*;

// These should NOT compile (uncomment to verify):
// use reovim_kernel::mm::*;       // ERROR: module `mm` is private
// use reovim_kernel::ipc::*;      // ERROR: module `ipc` is private
// use reovim_kernel::core::*;     // ERROR: module `core` is private
// use reovim_kernel::sched::*;    // ERROR: module `sched` is private
// use reovim_kernel::block::*;    // ERROR: module `block` is private
// use reovim_kernel::printk::*;   // ERROR: module `printk` is private

#[test]
fn api_boundary_enforced() {
    // Verify key types are accessible via api::v1
    let version: Version = API_VERSION;
    assert_eq!(version.major, 0);
}

#[test]
fn module_types_accessible() {
    // Module system types should be accessible
    let id = ModuleId::new("test-module");
    assert_eq!(id.as_str(), "test-module");
}

#[test]
fn mode_types_accessible() {
    // Mode identity types should be accessible via api::v1
    let module = ModuleId::new("test");
    let mode_id = ModeId::new(module.clone(), "normal");
    let cmd_id = CommandId::new(module, "test-cmd");

    assert_eq!(mode_id.name(), "normal");
    assert_eq!(cmd_id.name(), "test-cmd");
}

#[test]
fn mode_stack_accessible() {
    // ModeStack should be accessible via api::v1
    let module = ModuleId::new("test");
    let normal = ModeId::new(module.clone(), "normal");
    let insert = ModeId::new(module, "insert");

    let mut stack = ModeStack::new(normal.clone());
    assert_eq!(stack.current(), &normal);

    stack.push(insert.clone());
    assert_eq!(stack.current(), &insert);

    stack.pop();
    assert_eq!(stack.current(), &normal);
}

#[test]
fn register_bank_accessible() {
    // RegisterBank should be accessible via api::v1
    let bank = RegisterBank::new();
    // With RwLock wrapping, use directly or via kernel context
    assert!(bank.get().text.is_empty());
}

#[test]
fn mark_bank_accessible() {
    // MarkBank should be accessible via api::v1
    let mut bank = MarkBank::new();
    bank.set_local('a', Position::new(10, 5));
    assert_eq!(bank.get_local('a'), Some(Position::new(10, 5)));
}

#[test]
fn motion_engine_accessible() {
    // MotionEngine should be accessible via api::v1
    let engine = MotionEngine;
    _ = engine; // Verify type compiles
}

#[test]
fn text_object_engine_accessible() {
    // TextObjectEngine should be accessible via api::v1
    let engine = TextObjectEngine;
    _ = engine; // Verify type compiles
}

#[test]
fn undo_tree_accessible() {
    // UndoTree should be accessible via api::v1
    let tree = UndoTree::new();
    assert!(!tree.can_undo());
}

#[test]
fn window_id_accessible() {
    // WindowId (identity only) should be accessible via api::v1
    let id1 = WindowId::new();
    let id2 = WindowId::new();
    assert_ne!(id1, id2);
}

#[test]
fn buffer_types_accessible() {
    // Buffer types should be accessible via api::v1
    let id = BufferId::new();
    let buf = Buffer::from_string("Hello\nWorld");
    assert_eq!(buf.line_count(), 2);
    _ = id; // Verify type compiles
}

#[test]
fn event_bus_accessible() {
    // EventBus should be accessible via api::v1
    let _bus = EventBus::new();
    // Note: subscribe() requires handler and priority args
    // The type being accessible via api::v1 is what we're testing
}

#[test]
fn sync_primitives_accessible() {
    // Sync primitives from reovim-arch should be re-exported
    use std::sync::Arc;

    let value = Arc::new(RwLock::new(42));
    assert_eq!(*value.read(), 42);
}

// ============================================================================
// Mode trait tests
// ============================================================================

/// A test mode implementation to verify the Mode trait is accessible.
struct TestMode {
    id: ModeId,
}

impl Mode for TestMode {
    fn id(&self) -> ModeId {
        self.id.clone()
    }
}

#[test]
fn mode_trait_accessible() {
    let module = ModuleId::new("test");
    let mode = TestMode {
        id: ModeId::new(module, "test-mode"),
    };

    // Mode trait is accessible
    assert_eq!(mode.id().name(), "test-mode");

    // Mode can be used as trait object
    let mode_ref: &dyn Mode = &mode;
    assert_eq!(mode_ref.id().name(), "test-mode");
}
