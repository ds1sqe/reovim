//! Behavioral and layout tests for `uapi/terminal`.

use {
    reovim_uapi_syscall::{RawSyscall, SyscallArgs, SyscallError, SyscallNr, SyscallRet},
    reovim_uapi_terminal::{
        DIAGNOSTIC_OUTPUT, PRIMARY_INPUT, PRIMARY_OUTPUT, RawModeRequest, RawModeToken,
        SyscallTerminalControl, TerminalControl, TerminalError, TerminalId,
    },
};

#[test]
fn well_known_terminal_ids_are_logical_values() {
    assert_eq!(PRIMARY_INPUT.raw(), 0);
    assert_eq!(PRIMARY_OUTPUT.raw(), 1);
    assert_eq!(DIAGNOSTIC_OUTPUT.raw(), 2);
    assert_eq!(TerminalId::new(99).raw(), 99);
}

#[test]
fn raw_mode_request_names_a_logical_terminal() {
    assert_eq!(RawModeRequest::primary_input(), RawModeRequest::new(PRIMARY_INPUT));
    assert_eq!(RawModeRequest::new(PRIMARY_OUTPUT).terminal, PRIMARY_OUTPUT);
}

#[test]
fn raw_mode_token_is_opaque_and_round_trips() {
    let token = RawModeToken::new(0x55aa);
    assert_eq!(token.raw(), 0x55aa);
}

#[test]
fn absent_terminal_errors_are_distinguished_from_hard_refusals() {
    assert!(TerminalError::NotTerminal.is_absent_terminal());
    assert!(TerminalError::Unsupported.is_absent_terminal());
    assert!(!TerminalError::InvalidTarget.is_absent_terminal());
    assert!(!TerminalError::Busy.is_absent_terminal());
    assert!(!TerminalError::PermissionDenied.is_absent_terminal());
}

#[test]
fn transparent_ids_and_tokens_keep_integer_layout() {
    assert_eq!(size_of::<TerminalId>(), size_of::<u32>());
    assert_eq!(align_of::<TerminalId>(), align_of::<u32>());
    assert_eq!(size_of::<RawModeToken>(), size_of::<u64>());
    assert_eq!(align_of::<RawModeToken>(), align_of::<u64>());
}

fn enter_fixture(_: RawModeRequest) -> Result<RawModeToken, TerminalError> {
    Ok(RawModeToken::new(11))
}

fn restore_fixture(token: RawModeToken) -> Result<(), TerminalError> {
    assert_eq!(token.raw(), 11);
    Ok(())
}

fn restore_primary_fixture() {}

#[test]
fn terminal_control_dispatches_function_pointers() {
    let terminal = TerminalControl::new(enter_fixture, restore_fixture, restore_primary_fixture);
    let token = terminal
        .enter_raw_mode(RawModeRequest::primary_input())
        .expect("fixture enters raw mode");
    assert_eq!(token.raw(), 11);
    assert_eq!(terminal.restore_raw_mode(token), Ok(()));
    terminal.restore_primary_input_raw_mode();
}

#[test]
fn syscall_terminal_control_lowers_clear_to_raw_transport() {
    fn syscall(nr: SyscallNr, args: SyscallArgs) -> SyscallRet {
        assert_eq!(nr, SyscallNr::TERMINAL_CLEAR);
        assert_eq!(args.a0, PRIMARY_OUTPUT.raw() as usize);
        assert_eq!(args.a1, 0);
        assert_eq!(args.a2, 0);
        assert_eq!(args.a3, 0);
        assert_eq!(args.a4, 0);
        assert_eq!(args.a5, 0);
        SyscallRet::success(0)
    }

    let terminal = SyscallTerminalControl::new(RawSyscall::new(syscall));
    assert_eq!(terminal.clear_primary_output(), Ok(()));
}

#[test]
fn syscall_terminal_control_lowers_raw_mode_entry_to_raw_transport() {
    fn syscall(nr: SyscallNr, args: SyscallArgs) -> SyscallRet {
        assert_eq!(nr, SyscallNr::TERMINAL_RAW_ENTER);
        assert_eq!(args.a0, PRIMARY_INPUT.raw() as usize);
        assert_eq!(args.a1, 0);
        assert_eq!(args.a2, 0);
        assert_eq!(args.a3, 0);
        assert_eq!(args.a4, 0);
        assert_eq!(args.a5, 0);
        SyscallRet::success(0x33)
    }

    let terminal = SyscallTerminalControl::new(RawSyscall::new(syscall));
    assert_eq!(
        terminal.enter_raw_mode(RawModeRequest::primary_input()),
        Ok(RawModeToken::new(0x33)),
    );
}

#[test]
fn syscall_terminal_control_lowers_raw_mode_restore_to_raw_transport() {
    fn syscall(nr: SyscallNr, args: SyscallArgs) -> SyscallRet {
        assert_eq!(nr, SyscallNr::TERMINAL_RAW_RESTORE);
        assert_eq!(args.a0, 0x33);
        assert_eq!(args.a1, 0);
        assert_eq!(args.a2, 0);
        assert_eq!(args.a3, 0);
        assert_eq!(args.a4, 0);
        assert_eq!(args.a5, 0);
        SyscallRet::success(0)
    }

    let terminal = SyscallTerminalControl::new(RawSyscall::new(syscall));
    assert_eq!(terminal.restore_raw_mode(RawModeToken::new(0x33)), Ok(()));
}

#[test]
fn syscall_terminal_control_lowers_primary_restore_to_raw_transport() {
    fn syscall(nr: SyscallNr, args: SyscallArgs) -> SyscallRet {
        assert_eq!(nr, SyscallNr::TERMINAL_RAW_RESTORE_PRIMARY);
        assert_eq!(args, SyscallArgs::EMPTY);
        SyscallRet::success(0)
    }

    let terminal = SyscallTerminalControl::new(RawSyscall::new(syscall));
    assert_eq!(terminal.restore_primary_input_raw_mode(), Ok(()));
}

#[test]
fn syscall_terminal_control_maps_transport_errors() {
    fn syscall(_: SyscallNr, _: SyscallArgs) -> SyscallRet {
        SyscallRet::failure(SyscallError::INVALID_ARGUMENT)
    }

    let terminal = SyscallTerminalControl::new(RawSyscall::new(syscall));
    assert_eq!(terminal.clear(TerminalId::new(99)), Err(TerminalError::InvalidTarget),);
}
