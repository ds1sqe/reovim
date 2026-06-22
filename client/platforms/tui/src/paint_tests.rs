//! Tests for `paint.rs` raw-mode entry (L12.1 sibling, compiled under
//! `selftest`).
//!
//! The raw-mode/restore round trip on a real tty needs a terminal the selftest
//! runner does not have, so the covered path is the up-face absent-terminal
//! fallback: a terminal control table reports [`TerminalError::NotTerminal`],
//! and `run` continues without a raw guard. Provider terminal mechanics are
//! tested below the bridge; TUI only names the up-face vocabulary here.

use {
    crate::paint::enter_raw_mode_guard,
    reovim_arch::{arch_test, testrt},
    reovim_uapi::terminal::{
        PRIMARY_INPUT, RawModeRequest, RawModeToken, TerminalControl, TerminalError,
    },
};

fn enter_not_terminal(request: RawModeRequest) -> Result<RawModeToken, TerminalError> {
    testrt::check_eq(request.terminal.raw(), PRIMARY_INPUT.raw());
    Err(TerminalError::NotTerminal)
}

fn restore_noop(_: RawModeToken) -> Result<(), TerminalError> {
    Ok(())
}

fn restore_primary_noop() {}

fn not_terminal_control() -> TerminalControl {
    TerminalControl::new(enter_not_terminal, restore_noop, restore_primary_noop)
}

arch_test!(paint_not_terminal_fallback_uses_no_raw_guard, {
    let guard = enter_raw_mode_guard(not_terminal_control());
    testrt::check(guard.is_none(), "NotTerminal takes the no-raw-guard fallback arm");
});

arch_test!(paint_names_up_face_raw_mode_request_vocabulary, {
    let request = RawModeRequest::primary_input();
    testrt::check_eq(request.terminal.raw(), PRIMARY_INPUT.raw());
    testrt::check(
        TerminalError::NotTerminal.is_absent_terminal(),
        "not-a-terminal maps to headless/pipe fallback vocabulary",
    );
});
