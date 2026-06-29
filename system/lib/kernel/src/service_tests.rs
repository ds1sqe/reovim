use reovim_testrt::{self as testrt, arch_test};

use super::{EMPTY_SERVICE_RECORD, MAX_SERVICES, ServiceError, ServiceReason, ServiceState};

arch_test!(service_table_records_updates_and_marks_init_services, {
    super::reset();

    let record = super::request_service(3, 3, "shell", "/bin/sh").expect("service records");
    testrt::check_eq(record.seq, 1usize);
    testrt::check_eq(record.name, "shell");
    testrt::check_eq(record.target, "/bin/sh");
    testrt::check_eq(record.owner_pid, 3usize);
    testrt::check_eq(record.owner_task_id, 3usize);
    testrt::check_eq(record.state, ServiceState::Requested);
    testrt::check_eq(record.reason, ServiceReason::Requested);
    testrt::check_eq(super::service_target("shell"), Some("/bin/sh"));

    let started = super::mark_started("shell", 2, 2).expect("service can start");
    testrt::check_eq(started.state, ServiceState::Started);
    testrt::check_eq(started.reason, ServiceReason::Running);
    testrt::check_eq(started.service_pid, 2usize);
    testrt::check_eq(started.service_task_id, 2usize);

    let mut records = [EMPTY_SERVICE_RECORD; MAX_SERVICES];
    let count = super::snapshot(&mut records);
    testrt::check_eq(count, 1usize);
    testrt::check_eq(records[0].name, "shell");
    testrt::check_eq(records[0].state, ServiceState::Started);
    testrt::check_eq(records[0].reason, ServiceReason::Running);
    testrt::check_eq(records[0].service_pid, 2usize);
    testrt::check_eq(records[0].service_task_id, 2usize);

    let failed = super::mark_failed("shell", ServiceReason::StartError)
        .expect("service can fail after update");
    testrt::check_eq(failed.state, ServiceState::Failed);
    testrt::check_eq(failed.reason, ServiceReason::StartError);

    super::reset();
    testrt::check_eq(super::snapshot(&mut records), 0usize);
});

arch_test!(service_table_rejects_empty_requests, {
    super::reset();

    testrt::check_eq(super::request_service(3, 3, "", "/bin/sh"), Err(ServiceError::Empty));
    testrt::check_eq(super::request_service(3, 3, "shell", ""), Err(ServiceError::Empty));
});

arch_test!(service_table_marks_exited_by_service_pid, {
    super::reset();

    super::request_service(4, 4, "editor", "/payload/editor-smoke").expect("service records");
    super::mark_started("editor", 4, 4).expect("service starts");

    let exited = super::mark_exited_by_service_pid(4).expect("service can exit by service pid");
    testrt::check_eq(exited.name, "editor");
    testrt::check_eq(exited.target, "/payload/editor-smoke");
    testrt::check_eq(exited.state, ServiceState::Exited);
    testrt::check_eq(exited.reason, ServiceReason::ProcessExited);
    testrt::check_eq(exited.service_pid, 4usize);
    testrt::check_eq(exited.service_task_id, 4usize);

    testrt::check_eq(super::mark_exited_by_service_pid(99), None);
    testrt::check_eq(super::mark_exited_by_service_pid(0), None);
});

arch_test!(service_table_marks_failed_by_service_pid, {
    super::reset();

    super::request_service(4, 4, "editor", "/payload/editor-smoke").expect("service records");
    super::mark_started("editor", 4, 4).expect("service starts");

    let failed = super::mark_failed_by_service_pid(4, ServiceReason::ProcessKilled)
        .expect("service can fail by service pid");
    testrt::check_eq(failed.name, "editor");
    testrt::check_eq(failed.target, "/payload/editor-smoke");
    testrt::check_eq(failed.state, ServiceState::Failed);
    testrt::check_eq(failed.reason, ServiceReason::ProcessKilled);
    testrt::check_eq(failed.service_pid, 4usize);
    testrt::check_eq(failed.service_task_id, 4usize);

    testrt::check_eq(super::mark_failed_by_service_pid(99, ServiceReason::ProcessKilled), None);
    testrt::check_eq(super::mark_failed_by_service_pid(0, ServiceReason::ProcessKilled), None);
});

arch_test!(service_table_marks_operator_stopped_services, {
    super::reset();

    super::request_service(4, 4, "editor", "/payload/editor-smoke").expect("service records");
    super::mark_started("editor", 4, 4).expect("service starts");

    let stopped = super::mark_stopped("editor").expect("service can be stopped by name");
    testrt::check_eq(stopped.name, "editor");
    testrt::check_eq(stopped.target, "/payload/editor-smoke");
    testrt::check_eq(stopped.state, ServiceState::Stopped);
    testrt::check_eq(stopped.reason, ServiceReason::OperatorStop);
    testrt::check_eq(stopped.service_pid, 4usize);
    testrt::check_eq(stopped.service_task_id, 4usize);
    testrt::check_eq(super::service("editor"), Some(stopped));
    testrt::check_eq(super::mark_stopped("missing"), None);
});
