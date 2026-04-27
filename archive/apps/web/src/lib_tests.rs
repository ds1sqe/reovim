//! Coverage for the `reovim-app-web` wrapper. `run()` is a thin
//! delegation to the platform crate, so the lib-level tests verify
//! only the argument-parsing surface that `apps/web` owns. End-to-end
//! HTTP coverage lives in the platform crate's `server_tests.rs`.

use super::*;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "reovim-web-test")]
struct TestCli {
    #[command(flatten)]
    web: WebArgs,
}

#[test]
fn web_args_parses_default_listen_address() {
    let cli = TestCli::parse_from(["reovim-web-test"]);
    assert_eq!(cli.web.listen.to_string(), "127.0.0.1:7870");
}

#[test]
fn web_args_parses_explicit_listen_flag() {
    let cli = TestCli::parse_from(["reovim-web-test", "--listen", "0.0.0.0:9090"]);
    assert_eq!(cli.web.listen.port(), 9090);
    assert!(cli.web.listen.ip().is_unspecified());
}
