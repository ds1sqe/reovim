//! Coverage for the web stub. `run()` always errors with
//! `Unsupported`; `WebArgs` derives clap's `Args` so the default
//! listen address is the only runtime behavior worth asserting.

use super::*;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "reovim-web-test")]
struct TestCli {
    #[command(flatten)]
    web: WebArgs,
}

#[test]
fn run_returns_unsupported_with_ssr_hint() {
    let cli = TestCli::parse_from(["reovim-web-test"]);
    let err = run(cli.web).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::Unsupported);
    assert!(
        err.to_string().contains("SSR"),
        "error message should cite the SSR deferral: {err}"
    );
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
