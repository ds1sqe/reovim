//! Dispatch-table tests for the launcher. Exercise the pure
//! argv-to-`Dispatch` mapping without actually spawning any child
//! process — `CommandSpawn` is the test seam.

use super::*;

#[test]
fn dispatch_server_forwards_args_verbatim() {
    let cmd = Cmd::Server { args: vec!["--grpc".into(), "12540".into()] };
    let d = dispatch_for(&cmd);
    assert_eq!(d.bin, "reovim-server");
    assert!(d.leading_args.is_empty());
    assert_eq!(d.forwarded_args, &["--grpc".to_owned(), "12540".to_owned()]);
}

#[test]
fn dispatch_tui_uses_reovim_tui_bin() {
    let cmd = Cmd::Tui { args: vec![] };
    let d = dispatch_for(&cmd);
    assert_eq!(d.bin, "reovim-tui");
}

#[test]
fn dispatch_cli_uses_reovim_cli_bin() {
    let cmd = Cmd::Cli { args: vec!["ping".into()] };
    let d = dispatch_for(&cmd);
    assert_eq!(d.bin, "reovim-cli");
    assert_eq!(d.forwarded_args, &["ping".to_owned()]);
}

#[test]
fn dispatch_module_prefixes_reovim_server_with_module_leading_arg() {
    // `reovim module X` routes through `reovim-server module X`,
    // avoiding a separate module bin.
    let cmd = Cmd::Module { args: vec!["list".into()] };
    let d = dispatch_for(&cmd);
    assert_eq!(d.bin, "reovim-server");
    assert_eq!(d.leading_args, &["module"]);
    assert_eq!(d.forwarded_args, &["list".to_owned()]);
}

#[test]
fn dispatch_web_uses_reovim_web_bin() {
    let cmd = Cmd::Web { args: vec![] };
    let d = dispatch_for(&cmd);
    assert_eq!(d.bin, "reovim-web");
}

/// In-memory `CommandSpawn` impl for exit-code propagation tests.
struct RecordingSpawner {
    exit_code: i32,
    seen_bin: std::cell::RefCell<Option<String>>,
    seen_args: std::cell::RefCell<Vec<String>>,
}

impl CommandSpawn for RecordingSpawner {
    fn spawn(&self, dispatch: &Dispatch<'_>) -> std::io::Result<ExitStatus> {
        *self.seen_bin.borrow_mut() = Some(dispatch.bin.to_owned());
        let mut captured: Vec<String> =
            dispatch.leading_args.iter().map(ToString::to_string).collect();
        captured.extend(dispatch.forwarded_args.iter().cloned());
        *self.seen_args.borrow_mut() = captured;
        Ok(exit_status_with_code(self.exit_code))
    }
}

#[cfg(unix)]
fn exit_status_with_code(code: i32) -> ExitStatus {
    use std::os::unix::process::ExitStatusExt;
    ExitStatus::from_raw(code << 8)
}

#[cfg(not(unix))]
#[cfg_attr(coverage_nightly, coverage(off))]
fn exit_status_with_code(_code: i32) -> ExitStatus {
    // Stable std constructs ExitStatus from raw on unix only; the
    // non-unix path of this helper is exercised through the
    // cross-platform CI end-to-end job, and the unit test below skips
    // on non-unix.
    panic!("exit_status_with_code is unix-only");
}

#[cfg(unix)]
#[test]
fn recording_spawner_captures_server_bin_name_and_args() {
    let spawner = RecordingSpawner {
        exit_code: 0,
        seen_bin: std::cell::RefCell::new(None),
        seen_args: std::cell::RefCell::new(Vec::new()),
    };
    let cmd = Cmd::Server { args: vec!["--grpc".into(), "12540".into()] };
    let dispatch = dispatch_for(&cmd);
    let status = spawner.spawn(&dispatch).expect("spawn ok");
    assert_eq!(status.code(), Some(0));
    assert_eq!(spawner.seen_bin.borrow().as_deref(), Some("reovim-server"));
    assert_eq!(
        &*spawner.seen_args.borrow(),
        &["--grpc".to_owned(), "12540".to_owned()]
    );
}

#[cfg(unix)]
#[test]
fn recording_spawner_captures_module_dispatch_prefix() {
    let spawner = RecordingSpawner {
        exit_code: 0,
        seen_bin: std::cell::RefCell::new(None),
        seen_args: std::cell::RefCell::new(Vec::new()),
    };
    let cmd = Cmd::Module { args: vec!["list".into()] };
    let dispatch = dispatch_for(&cmd);
    let _ = spawner.spawn(&dispatch).expect("spawn ok");
    assert_eq!(
        &*spawner.seen_args.borrow(),
        &["module".to_owned(), "list".to_owned()]
    );
}
