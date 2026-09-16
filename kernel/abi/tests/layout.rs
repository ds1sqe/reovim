//! reovim.h and the Rust mirror agree on every layout, as the system
//! C compiler sees it.

use std::mem::{offset_of, size_of};
use std::process::Command;

use reovim_kernel_abi::{RvBytes, RvCalls, RvDriver, RvModule, RvReply};

const PROBE: &str = r#"
#include <stdio.h>
#include "reovim.h"
int main(void) {
    printf("%zu %zu %zu %zu %zu %zu %zu %zu %zu %zu\n",
        sizeof(rv_bytes), sizeof(rv_reply), sizeof(rv_calls),
        sizeof(rv_driver), sizeof(rv_module), offsetof(rv_reply, value),
        offsetof(rv_calls, submit), offsetof(rv_driver, close),
        offsetof(rv_module, start), offsetof(rv_module, driver_count));
    return 0;
}
"#;

#[test]
fn the_header_and_the_mirror_agree() {
    let dir =
        std::env::temp_dir().join(format!("reovim-abi-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("layout.c");
    let binary = dir.join("layout");
    std::fs::write(&source, PROBE).unwrap();
    let include = concat!(env!("CARGO_MANIFEST_DIR"), "/include");
    let built = Command::new("cc")
        .args(["-I", include, "-o"])
        .arg(&binary)
        .arg(&source)
        .status()
        .expect("cc");
    assert!(built.success());
    let out = Command::new(&binary).output().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let c = String::from_utf8(out.stdout).unwrap();
    let rust = format!(
        "{} {} {} {} {} {} {} {} {} {}\n",
        size_of::<RvBytes>(),
        size_of::<RvReply>(),
        size_of::<RvCalls>(),
        size_of::<RvDriver>(),
        size_of::<RvModule>(),
        offset_of!(RvReply, value),
        offset_of!(RvCalls, submit),
        offset_of!(RvDriver, close),
        offset_of!(RvModule, start),
        offset_of!(RvModule, driver_count),
    );
    assert_eq!(c, rust);
}
