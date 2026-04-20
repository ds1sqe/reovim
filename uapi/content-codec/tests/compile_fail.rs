//! Compile-fail protections for Phase 3 architectural guarantees.

#[test]
fn no_raw_bytes_accessor_in_codec_session_state() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/inode_no_raw_bytes.rs");
}
