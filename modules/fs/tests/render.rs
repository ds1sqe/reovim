//! Committed host declarations track the public APIs.

#[test]
fn committed_renderings_match() {
    let rendered = reovim_module_fs::fs::text("fs");
    assert_eq!(include_str!("../fs.fs.fipe"), rendered);
}
