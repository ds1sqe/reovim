//! Tests for the panic/log bridge's pure mapping decisions.

use {
    super::panic::{
        map_set_result, panic_control, record_and_forward_panic_state_for_tests,
        reset_forwarded_state_record_hook_for_tests, set_disposition, set_flush_target,
        set_pre_exit_hook, set_ring_tail_provider, set_state_record_hook,
    },
    crate::dump,
    reovim_kabi_panic::SetError,
    reovim_testrt::{self as testrt, arch_test},
    reovim_uapi_panic::{Disposition, PanicConfigError, PanicControl, PanicRecord},
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

arch_test!(panic_bridge_records_state_for_dump_before_forwarding, {
    dump::reset_panic_record_for_tests();
    reset_forwarded_state_record_hook_for_tests();

    record_and_forward_panic_state_for_tests(PanicRecord {
        disposition: Disposition::Recover,
        rollback_failed: true,
    });

    let status = dump::status();
    testrt::check_eq(status.panic_state, "recorded");
    testrt::check_eq(status.panic_records, 1usize);
    let record = status.panic_record.expect("panic record captured");
    testrt::check_eq(record.disposition, Disposition::Recover);
    testrt::check_eq(record.rollback_failed, true);

    dump::reset_panic_record_for_tests();
});
