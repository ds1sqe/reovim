//! Phase 2 integration smoke (#797): kernel + text Domain dispatch.
//!
//! This module lives in the `kernel-selftest` bin (not in the kernel rlib)
//! because the text Domain is an ext crate (`ext/server/domain/text`) and the
//! kernel rlib must NOT depend on ext (core/ext boundary, `CLAUDE.md §Design
//! Rules`). The bin links `reovim-domain-text` and wires its trait objects into
//! the kernel here.
//!
//! ## What the smoke proves
//!
//! Boot → register text Domain → attach session → dispatch `RawInput` batch
//! through `OnRawInput` → `Render` → assert `Projection` bytes match fixture.
//! This exercises the full in-kernel dispatch loop before any wire is involved
//! (Phase 2 AC: integration smoke).

use reovim_arch::arch_test;
use reovim_domain_text::{TextHandler, TextProjector};
use reovim_kernel::{
    Init, LauncherArgs,
    session::{BufferId, DomainAttachmentId, SessionState, WindowId},
};

/// Static text Domain singletons. Zero-sized structs; no heap needed.
static TEXT_HANDLER: TextHandler = TextHandler;
static TEXT_PROJECTOR: TextProjector = TextProjector;

arch_test!(phase2_integration_smoke_insert_and_backspace, {
    // Boot the kernel.
    let kernel = Init::new(LauncherArgs::default())
        .boot()
        .expect("boot succeeds");

    // Register the text Domain.
    let domain_id = kernel
        .register_domain("text", &TEXT_HANDLER, &TEXT_PROJECTOR)
        .expect("register_domain succeeds");

    // Wire the session to the registered Domain.
    let state = SessionState::new(
        domain_id,
        DomainAttachmentId::new(1),
        BufferId::new(1),
        WindowId::new(1),
    );
    kernel.setup_session(state);

    // Insert "hi".
    let proj = kernel.dispatch_input(b"hi").expect("dispatch 'hi'");
    assert_eq!(proj.content.as_slice(), b"hi", "buffer must be 'hi' after insert");
    assert_eq!(proj.cursor_byte, 2);
    assert_eq!(proj.span.start, 0);
    assert_eq!(proj.span.end, 2);

    // Backspace deletes 'i'.
    let proj2 = kernel.dispatch_input(b"\x08").expect("dispatch backspace");
    assert_eq!(proj2.content.as_slice(), b"h", "backspace must delete last char");
    assert_eq!(proj2.cursor_byte, 1);
    assert_eq!(proj2.span.end, 1);
});

arch_test!(phase2_intern_stability, {
    // Intern stability: same name → same DomainId across two intern calls
    // (§4.1 §3).
    let kernel = Init::new(LauncherArgs::default())
        .boot()
        .expect("boot succeeds");

    let id1 = {
        let mut router = kernel.domain_router.write();
        router.intern_named("text").expect("first intern")
    };
    let id2 = {
        let mut router = kernel.domain_router.write();
        router.intern_named("text").expect("second intern")
    };
    assert_eq!(id1, id2, "intern_named is idempotent: same name → same DomainId");
});

arch_test!(phase2_projection_encode_decode_roundtrip_with_dispatch, {
    use reovim_kernel::projection::Projection;

    let kernel = Init::new(LauncherArgs::default())
        .boot()
        .expect("boot succeeds");

    let domain_id = kernel
        .register_domain("text", &TEXT_HANDLER, &TEXT_PROJECTOR)
        .expect("register domain");

    let state = SessionState::new(
        domain_id,
        DomainAttachmentId::new(1),
        BufferId::new(1),
        WindowId::new(1),
    );
    kernel.setup_session(state);

    // Type "abc" and encode the projection.
    let proj = kernel.dispatch_input(b"abc").expect("dispatch 'abc'");
    let encoded = proj.encode().expect("encode");
    let decoded = Projection::decode(encoded.as_slice()).expect("decode");
    assert_eq!(decoded.content.as_slice(), b"abc");
    assert_eq!(decoded.cursor_byte, 3);
    assert_eq!(decoded.span.end, 3);
});
