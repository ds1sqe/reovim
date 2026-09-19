//! `reovim server --sys NAME=VALUE`: the kernel command line, writing
//! `/sys/NAME` before the first Module loads. Through real processes.

mod common;

use std::process::Stdio;

use common::{Server, place, serving};
use reovim_host::link;

#[test]
fn a_boot_parameter_is_set_before_the_first_module() {
    let sys = ["loglevel=debug", "limit/log_ring=256"];
    let server = Server::with("boot", &sys, false);
    // The level was already `debug` when the Modules loaded, so their
    // loading was traced; the ring was already 256, so it kept 256. Read
    // before anything else: at debug a run under `/bin` traces a couple
    // of dozen ops, and the ring keeps the boot for only a few of them.
    let said = server.out(&["dmesg", "-t"]);
    assert!(said.contains("load module=fs"), "{said}");
    assert!(said.lines().count() <= 256, "{said}");
    assert!(said.lines().any(|line| line.starts_with("op=")), "{said}");
    assert_eq!(server.out(&["cat", "/sys/loglevel"]), "debug");
    assert_eq!(server.out(&["cat", "/sys/limit/log_ring"]), "256");
}

#[test]
fn a_parameter_the_kernel_refuses_refuses_the_start() {
    let (dir, socket) = place("refused");
    let out = serving(&dir, &socket, &["limit/reentry=x"], false)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(!out.status.success(), "the server started anyway");
    assert_eq!(
        String::from_utf8_lossy(&out.stderr).trim_end(),
        "reovim: --sys limit/reentry=x: invalid"
    );
    // Nothing listens: the Kernel never reached its first Module.
    assert!(link::open(&socket).is_err());
    // A name that is no parameter is refused the same way.
    let out = serving(&dir, &socket, &["nothing=1"], false)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stderr).trim_end(),
        "reovim: --sys nothing=1: invalid"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
