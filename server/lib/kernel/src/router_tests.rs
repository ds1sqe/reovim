//! Tests for `router.rs` — `DomainRouter` SUBSET coverage.
//!
//! Registered under the `selftest` feature; runs on the arch no_std selftest
//! runner via the `kernel-selftest` fixture bin.

use reovim_arch::arch_test;

use crate::router::{DomainId, DomainRouter};

arch_test!(router_new_is_empty, {
    let r = DomainRouter::new();
    assert!(r.name_of_id(1).is_none(), "new router has no names");
});

arch_test!(intern_named_first_call_returns_id, {
    let mut r = DomainRouter::new();
    let id = r.intern_named("text").expect("intern_named succeeds");
    assert_eq!(id.as_u32(), 1, "first interned id is 1");
});

arch_test!(intern_named_idempotent_same_name, {
    // §4.1 §3 intern stability: same name → same DomainId.
    let mut r = DomainRouter::new();
    let id1 = r.intern_named("text").expect("first intern");
    let id2 = r.intern_named("text").expect("second intern");
    assert_eq!(id1, id2, "intern_named is idempotent for the same name");
});

arch_test!(intern_named_different_names_get_different_ids, {
    let mut r = DomainRouter::new();
    let id_text = r.intern_named("text").expect("intern text");
    let id_elf = r.intern_named("elf").expect("intern elf");
    assert_ne!(id_text, id_elf, "different names → different ids");
});

arch_test!(name_of_returns_bytes_for_interned_id, {
    let mut r = DomainRouter::new();
    let id = r.intern_named("text").expect("intern");
    let name = r.name_of(id).expect("name_of returns Some for interned id");
    assert_eq!(name, b"text");
});

arch_test!(name_of_unknown_id_returns_none, {
    use core::num::NonZeroU32;
    let r = DomainRouter::new();
    let fake_id = DomainId::new(NonZeroU32::new(99).unwrap());
    assert!(r.name_of(fake_id).is_none(), "unknown id → None");
});

arch_test!(handler_returns_none_when_not_registered, {
    use core::num::NonZeroU32;
    let r = DomainRouter::new();
    let id = DomainId::new(NonZeroU32::new(1).unwrap());
    assert!(r.handler(id).is_none(), "no handler before registration");
});

arch_test!(projector_returns_none_when_not_registered, {
    use core::num::NonZeroU32;
    let r = DomainRouter::new();
    let id = DomainId::new(NonZeroU32::new(1).unwrap());
    assert!(r.projector(id).is_none(), "no projector before registration");
});

// ── Default impl coverage (router.rs line 241-243) ───────────────────────────
//
// `impl Default for DomainRouter` delegates to `Self::new()`. Calling it
// directly exercises lines 241-243 which are otherwise unreachable by the
// `new()` tests (the compiler may or may not inline them into the same
// region, but the coverage tool tracks the explicit `default()` call site).

arch_test!(domain_router_default_equals_new, {
    // `Default::default()` must produce an equivalent empty router.
    let r: DomainRouter = Default::default();
    assert!(r.name_of_id(1).is_none(), "default DomainRouter is empty");
});
