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
//! │  │  KernelContext   traits::*   types::*   module::*     │  │
//! │  └───────────────────────────────────────────────────────┘  │
//! └─────────────────────────────────────────────────────────────┘
//!                            │
//!                            │ ONLY api::*
//!                            ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │  MODULES                                                    │
//! │  use reovim_kernel::api::{KernelContext, traits::*, ...};   │
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
fn kernel_context_accessible() {
    // KernelContext should be accessible via api::v1
    let ctx = KernelContext::default();
    assert!(ctx.buffers.count() == 0);
}

#[test]
fn module_types_accessible() {
    // Module system types should be accessible
    let id = ModuleId::new("test-module");
    assert_eq!(id.as_str(), "test-module");
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
fn undo_types_accessible() {
    // Undo types should be accessible via api::v1
    let tree = UndoTree::new();
    let err: UndoError = UndoError::NothingToUndo;
    assert!(!tree.can_undo());
    assert_eq!(err.to_string(), "nothing to undo");
}

#[test]
fn window_types_accessible() {
    // Window types should be accessible via api::v1
    let id = WindowId::new();
    let buffer_id = BufferId::new();
    let window = Window::new(id, buffer_id);
    assert_eq!(window.buffer_id, buffer_id);

    // Viewport is mechanism (what lines are visible)
    let viewport = Viewport::new(0, 50);
    assert_eq!(viewport.top_line, 0);
    assert_eq!(viewport.height, 50);
}

#[test]
fn policy_traits_accessible() {
    // Policy interface traits should be accessible via api::v1
    fn _accepts_operator<T: Operator>(_: T) {}
    fn _accepts_keymap<T: KeymapProvider>(_: T) {}
    fn _accepts_command<T: CommandHandler>(_: T) {}

    // Range type for operators
    let range = Range::new(Position::new(0, 0), Position::new(1, 5));
    assert!(!range.is_empty());

    // KeyEvent for keymap
    let key = KeyEvent::char('j');
    assert_eq!(key.code, KeyCode::Char('j'));
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
