//! Tests for the terminal/raw-mode bridge's pure mapping decisions.

use {
    super::terminal::{
        DIAGNOSTIC_OUTPUT_FD, PRIMARY_INPUT_FD, PRIMARY_OUTPUT_FD, map_terminal_error, terminal_fd,
        token_to_fd,
    },
    reovim_kabi_platform::{ENOTTY, Errno},
    reovim_testrt::{self as testrt, arch_test},
    reovim_uapi_terminal::{
        DIAGNOSTIC_OUTPUT, PRIMARY_INPUT, PRIMARY_OUTPUT, RawModeToken, TerminalError, TerminalId,
    },
};

arch_test!(terminal_well_known_ids_map_to_standard_stream_fds, {
    testrt::check_eq(terminal_fd(PRIMARY_INPUT), Ok(PRIMARY_INPUT_FD));
    testrt::check_eq(terminal_fd(PRIMARY_OUTPUT), Ok(PRIMARY_OUTPUT_FD));
    testrt::check_eq(terminal_fd(DIAGNOSTIC_OUTPUT), Ok(DIAGNOSTIC_OUTPUT_FD));
});

arch_test!(terminal_unknown_id_is_invalid_target, {
    testrt::check_eq(terminal_fd(TerminalId::new(99)), Err(TerminalError::InvalidTarget));
});

arch_test!(terminal_error_mapping_keeps_enotty_as_absent_terminal, {
    testrt::check_eq(map_terminal_error(ENOTTY), TerminalError::NotTerminal);
    testrt::check_eq(map_terminal_error(Errno::from_code(9)), TerminalError::PermissionDenied);
});

arch_test!(terminal_token_restore_key_rejects_out_of_range_values, {
    testrt::check_eq(token_to_fd(RawModeToken::new(2)), Ok(2));
    testrt::check_eq(
        token_to_fd(RawModeToken::new(2_147_483_648)),
        Err(TerminalError::InvalidTarget),
    );
});
