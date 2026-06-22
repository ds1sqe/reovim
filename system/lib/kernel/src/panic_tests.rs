//! Tests for the panic/log bridge's pure mapping decisions.

use {
    super::panic::{
        map_set_result, panic_control, set_disposition, set_flush_target, set_pre_exit_hook,
        set_ring_tail_provider, set_state_record_hook,
    },
    reovim_kabi_panic::SetError,
    reovim_testrt::{self as testrt, arch_test},
    reovim_uapi_panic::{PanicConfigError, PanicControl},
};

arch_test!(panic_control_points_at_system_kernel_bridge_fns, {
    let control = panic_control();
    testrt::check_eq(
        control.set_disposition_fn as *const () as usize,
        set_disposition as *const () as usize,
    );
    testrt::check_eq(
        control.set_ring_tail_provider_fn as *const () as usize,
        set_ring_tail_provider as *const () as usize,
    );
    testrt::check_eq(
        control.set_state_record_hook_fn as *const () as usize,
        set_state_record_hook as *const () as usize,
    );
    testrt::check_eq(
        control.set_flush_target_fn as *const () as usize,
        set_flush_target as *const () as usize,
    );
    testrt::check_eq(
        control.set_pre_exit_hook_fn as *const () as usize,
        set_pre_exit_hook as *const () as usize,
    );
});

arch_test!(panic_bridge_maps_write_once_refusal_to_up_face_error, {
    testrt::check_eq(map_set_result(Ok(())), Ok(()));
    testrt::check_eq(
        map_set_result(Err(SetError::AlreadySet)),
        Err(PanicConfigError::AlreadyConfigured),
    );
});

arch_test!(panic_control_type_remains_copyable_up_face_table, {
    let control: PanicControl = panic_control();
    let copied = control;
    testrt::check_eq(
        copied.set_pre_exit_hook_fn as *const () as usize,
        control.set_pre_exit_hook_fn as *const () as usize,
    );
});
