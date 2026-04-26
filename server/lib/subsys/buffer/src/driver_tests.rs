//! Compile-time shape tests for `BufferDriver`.

use super::*;

// ── Object-safety assertion ──────────────────────────────────────────────────

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn buffer_driver_is_object_safe() {
    fn accepts_ref(_: &dyn BufferDriver) {}
    fn accepts_box(_: Box<dyn BufferDriver>) {}
    let _: fn(&dyn BufferDriver) = accepts_ref;
    let _: fn(Box<dyn BufferDriver>) = accepts_box;
}

// ── Send + Sync assertion ────────────────────────────────────────────────────

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn buffer_driver_trait_object_is_send_sync() {
    fn assert<T: Send + Sync + ?Sized>() {}
    assert::<dyn BufferDriver>();
}
