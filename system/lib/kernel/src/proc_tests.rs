//! Selftests for the bounded process table and scheduler task lifecycle.

use {
    super::{
        EMPTY_PROCESS_HANDLE, EMPTY_PROCESS_RECORD, EMPTY_WAIT_RECORD, MAX_PROCESSES, MAX_WAITS,
        ProcessHandle, ProcessRecord, ProcessState, ROOTD_PID, SHELL_PID, begin_wait,
        block_pipe_read_process, block_process, block_user_resume_process, cancel_wait,
        dispatch_next_ready_process, exit_process, finish_wait, install_shell_session,
        replace_program_image, reset, run_process, sleep_process_until, snapshot, snapshot_waits,
        spawn_child as try_spawn_child,
        spawn_child_with_program_argv_env_mapped as try_spawn_child_with_program_argv_env_mapped,
        spawn_program as try_spawn_program, wait_record, wake_operator_blocked_process,
        wake_process, wake_sleepers_due, yield_process,
    },
    crate::{
        mm::{
            self, ADDRESS_SPACE_STACK_FLAGS, AddressSpaceState, DEFAULT_USER_STACK_BYTES,
            USER_STACK_TOP,
        },
        program,
        sched::{self, BlockReason, EMPTY_KERNEL_TASK_RECORD, KernelTaskState, MAX_KERNEL_TASKS},
        source_store::ExecutableSourceStore,
    },
    reovim_testrt::{self as testrt, arch_test},
};

fn process_state(pid: usize) -> ProcessState {
    process_record(pid).state
}

fn process_record(pid: usize) -> ProcessRecord {
    let mut records = [EMPTY_PROCESS_RECORD; MAX_PROCESSES];
    let count = snapshot(&mut records);
    let mut index = 0usize;
    while index < count {
        if records[index].pid == pid {
            return records[index];
        }
        index += 1;
    }
    EMPTY_PROCESS_RECORD
}

fn task_state(task_id: usize) -> KernelTaskState {
    task_record(task_id).state
}

fn task_record(task_id: usize) -> sched::KernelTaskRecord {
    let mut records = [EMPTY_KERNEL_TASK_RECORD; MAX_KERNEL_TASKS];
    let count = sched::snapshot_kernel_tasks(&mut records);
    let mut index = 0usize;
    while index < count {
        if records[index].task_id == task_id {
            return records[index];
        }
        index += 1;
    }
    EMPTY_KERNEL_TASK_RECORD
}

fn loaded_program(index: usize) -> program::LoadedProgram {
    crate::exec::load_bin_program(
        crate::bin_fixture::programs(),
        ExecutableSourceStore::program_only(crate::bin_fixture::program_sources()),
        crate::bin_fixture::programs()[index].path,
    )
    .expect("program loads from /bin catalog")
}

fn spawn_program(parent_pid: usize, program: program::LoadedProgram) -> ProcessHandle {
    try_spawn_program(parent_pid, program).expect("process slot available")
}

fn spawn_child(
    parent_pid: usize,
    parent_task_id: usize,
    path: &'static str,
    loader: &'static str,
    entry_name: &'static str,
) -> ProcessHandle {
    try_spawn_child(parent_pid, parent_task_id, path, loader, entry_name)
        .expect("child process slot available")
}

arch_test!(proc_program_task_lifecycle_moves_ready_running_blocked_exited, {
    reset();

    install_shell_session("/bin/sh", "linked-bin", "bin_sh");

    let shell = process_record(SHELL_PID);
    testrt::check_eq(shell.pid, SHELL_PID);
    testrt::check_eq(shell.parent_pid, ROOTD_PID);
    testrt::check_eq(shell.task_id, SHELL_PID);
    testrt::check_eq(shell.state, ProcessState::Running);
    testrt::check_eq(shell.program_path, "/bin/sh");
    testrt::check_eq(shell.loader, "linked-bin");
    testrt::check_eq(shell.entry_name, "bin_sh");
    testrt::check_eq(shell.argc, 1usize);
    testrt::check_eq(shell.argv0(), "/bin/sh");
    testrt::check_eq(shell.argv_was_truncated(0), false);
    testrt::check_eq(shell.address_space_id, SHELL_PID);
    testrt::check_eq(shell.image_generation, 1usize);
    let shell_space = mm::address_space(shell.address_space_id).expect("shell mm record retained");
    testrt::check_eq(shell_space.owner_pid, SHELL_PID);
    testrt::check_eq(shell_space.state, AddressSpaceState::Active);
    testrt::check_eq(shell_space.program_path, "/bin/sh");
    testrt::check_eq(shell_space.source_path, "/bin/sh");
    testrt::check_eq(shell_space.text_bytes, 0usize);
    testrt::check_eq(shell_space.text_checksum, 0u32);
    testrt::check_eq(shell_space.stack_bytes, DEFAULT_USER_STACK_BYTES);
    testrt::check_eq(shell_space.region_count, 1usize);
    testrt::check_eq(shell_space.text_start, 0usize);
    testrt::check_eq(shell_space.stack_start, USER_STACK_TOP - DEFAULT_USER_STACK_BYTES);
    testrt::check_eq(shell_space.stack_end, USER_STACK_TOP);
    testrt::check_eq(shell_space.stack_flags, ADDRESS_SPACE_STACK_FLAGS);

    let task = task_record(SHELL_PID);
    testrt::check_eq(task.process_id, SHELL_PID);
    testrt::check_eq(task.parent_task_id, ROOTD_PID);
    testrt::check_eq(task.state, KernelTaskState::Running);
    testrt::check_eq(task.entry, "/bin/sh");

    let scheduler = sched::snapshot_scheduler();
    testrt::check_eq(scheduler.current_task_id, SHELL_PID);

    reset();

    let handle = spawn_program(SHELL_PID, loaded_program(0));
    testrt::check_eq(handle.pid, 3usize);
    testrt::check_eq(handle.task_id, 3usize);
    testrt::check_eq(handle.program_path, "/bin/help");
    let record = process_record(handle.pid);
    testrt::check_eq(record.argc, 1usize);
    testrt::check_eq(record.argv0(), "help");
    testrt::check_eq(record.argv_was_truncated(0), false);
    testrt::check_eq(record.argv1(), "");
    testrt::check_eq(record.argv_was_truncated(1), false);
    testrt::check_eq(record.address_space_id, handle.pid);
    testrt::check_eq(record.image_generation, 1usize);
    let space = mm::address_space(record.address_space_id).expect("spawned mm record retained");
    testrt::check_eq(space.owner_pid, handle.pid);
    testrt::check_eq(space.image_generation, 1usize);
    testrt::check_eq(space.state, AddressSpaceState::Active);
    testrt::check_eq(space.program_path, "/bin/help");
    testrt::check_eq(space.loader, "linked-bin");
    testrt::check_eq(space.entry_name, "bin_help");
    testrt::check_eq(space.source_path, "/bin/help");
    testrt::check_eq(space.text_bytes, 0usize);
    testrt::check_eq(space.text_checksum, 0u32);
    testrt::check_eq(space.stack_bytes, DEFAULT_USER_STACK_BYTES);
    testrt::check_eq(space.region_count, 1usize);
    testrt::check_eq(space.stack_start, USER_STACK_TOP - DEFAULT_USER_STACK_BYTES);
    testrt::check_eq(space.stack_end, USER_STACK_TOP);
    testrt::check_eq(process_state(handle.pid), ProcessState::Ready);
    testrt::check_eq(task_state(handle.task_id), KernelTaskState::Ready);

    let running = run_process(handle.pid);
    testrt::check(running.is_some(), "process can transition to running");
    testrt::check_eq(process_state(handle.pid), ProcessState::Running);
    testrt::check_eq(task_state(handle.task_id), KernelTaskState::Running);

    let blocked = block_process(handle.pid);
    testrt::check(blocked.is_some(), "process can transition to blocked");
    testrt::check_eq(process_state(handle.pid), ProcessState::Blocked);
    testrt::check_eq(task_state(handle.task_id), KernelTaskState::Blocked);
    testrt::check_eq(process_record(handle.pid).block_reason, BlockReason::Operator);
    testrt::check_eq(task_record(handle.task_id).block_reason, BlockReason::Operator);

    exit_process(handle.pid, ProcessState::Exited, 0);
    testrt::check_eq(process_state(handle.pid), ProcessState::Exited);
    testrt::check_eq(task_state(handle.task_id), KernelTaskState::Exited);
    testrt::check_eq(process_record(handle.pid).block_reason, BlockReason::None);
    testrt::check_eq(task_record(handle.task_id).block_reason, BlockReason::None);
    let retired_space =
        mm::address_space(record.address_space_id).expect("terminal mm record retained");
    testrt::check_eq(retired_space.state, AddressSpaceState::Retired);
});

arch_test!(proc_spawn_fails_closed_when_all_program_slots_are_live, {
    reset();

    let mut handles = [EMPTY_PROCESS_HANDLE; MAX_PROCESSES];
    let mut index = 0usize;
    while index < MAX_PROCESSES - 2 {
        handles[index] =
            try_spawn_program(SHELL_PID, loaded_program(0)).expect("live process slot installs");
        index += 1;
    }

    testrt::check(
        try_spawn_program(SHELL_PID, loaded_program(0)).is_none(),
        "full live process table rejects another spawn",
    );

    let mut records = [EMPTY_PROCESS_RECORD; MAX_PROCESSES];
    let count = snapshot(&mut records);
    testrt::check_eq(count, MAX_PROCESSES);
    testrt::check_eq(process_state(handles[0].pid), ProcessState::Ready);
    testrt::check_eq(task_state(handles[0].task_id), KernelTaskState::Ready);
    testrt::check_eq(process_state(handles[MAX_PROCESSES - 3].pid), ProcessState::Ready);
    testrt::check_eq(task_state(handles[MAX_PROCESSES - 3].task_id), KernelTaskState::Ready);

    let reusable = handles[0];
    let reusable_record = process_record(reusable.pid);
    exit_process(reusable.pid, ProcessState::Exited, 0);
    let replacement =
        try_spawn_program(SHELL_PID, loaded_program(1)).expect("terminal slot can be reclaimed");
    testrt::check(replacement.pid > reusable.pid, "replacement gets a fresh pid");
    testrt::check_eq(process_state(replacement.pid), ProcessState::Ready);
    testrt::check_eq(task_state(replacement.task_id), KernelTaskState::Ready);
    let reusable_space =
        mm::address_space(reusable_record.address_space_id).expect("terminal mm record retained");
    testrt::check_eq(reusable_space.state, AddressSpaceState::Retired);
    testrt::check_eq(process_state(handles[1].pid), ProcessState::Ready);
    testrt::check_eq(task_state(handles[1].task_id), KernelTaskState::Ready);
});

arch_test!(proc_pipe_read_block_reason_is_scheduler_visible, {
    reset();

    let handle = spawn_program(SHELL_PID, loaded_program(0));
    let running = run_process(handle.pid);
    testrt::check(running.is_some(), "process can run before pipe read block");

    let blocked = block_pipe_read_process(handle.pid);
    testrt::check(blocked.is_some(), "process can block on pipe input");
    testrt::check_eq(process_state(handle.pid), ProcessState::Blocked);
    testrt::check_eq(task_state(handle.task_id), KernelTaskState::Blocked);
    testrt::check_eq(process_record(handle.pid).block_reason, BlockReason::PipeRead);
    testrt::check_eq(task_record(handle.task_id).block_reason, BlockReason::PipeRead);

    let woken = wake_process(handle.pid);
    testrt::check(woken.is_some(), "pipe-read blocked process can wake");
    testrt::check_eq(process_record(handle.pid).block_reason, BlockReason::None);
    testrt::check_eq(task_record(handle.task_id).block_reason, BlockReason::None);
});

arch_test!(proc_lifecycle_rejects_blocked_and_terminal_resurrection, {
    reset();

    let handle = spawn_program(SHELL_PID, loaded_program(0));
    testrt::check_eq(run_process(handle.pid), Some(handle));
    testrt::check_eq(block_process(handle.pid), Some(handle));
    testrt::check_eq(process_state(handle.pid), ProcessState::Blocked);
    testrt::check_eq(task_state(handle.task_id), KernelTaskState::Blocked);
    testrt::check_eq(process_record(handle.pid).block_reason, BlockReason::Operator);
    testrt::check_eq(task_record(handle.task_id).block_reason, BlockReason::Operator);

    testrt::check_eq(run_process(handle.pid), None);
    testrt::check_eq(block_process(handle.pid), None);
    testrt::check_eq(block_pipe_read_process(handle.pid), None);
    testrt::check_eq(sleep_process_until(handle.pid, 9), None);
    testrt::check_eq(process_state(handle.pid), ProcessState::Blocked);
    testrt::check_eq(task_state(handle.task_id), KernelTaskState::Blocked);
    testrt::check_eq(process_record(handle.pid).block_reason, BlockReason::Operator);
    testrt::check_eq(task_record(handle.task_id).block_reason, BlockReason::Operator);
    testrt::check_eq(task_record(handle.task_id).wake_tick, 0usize);

    let adoption = exit_process(handle.pid, ProcessState::Failed, 7);
    testrt::check_eq(adoption.count, 0usize);
    testrt::check_eq(process_state(handle.pid), ProcessState::Failed);
    testrt::check_eq(task_state(handle.task_id), KernelTaskState::Failed);
    testrt::check_eq(process_record(handle.pid).exit_code, 7);
    testrt::check_eq(process_record(handle.pid).block_reason, BlockReason::None);
    testrt::check_eq(task_record(handle.task_id).block_reason, BlockReason::None);

    testrt::check_eq(run_process(handle.pid), None);
    testrt::check_eq(block_process(handle.pid), None);
    testrt::check_eq(sleep_process_until(handle.pid, 11), None);
    testrt::check_eq(wake_process(handle.pid), None);
    testrt::check_eq(process_state(handle.pid), ProcessState::Failed);
    testrt::check_eq(task_state(handle.task_id), KernelTaskState::Failed);
    testrt::check_eq(process_record(handle.pid).exit_code, 7);
});

arch_test!(proc_replace_image_requires_running_process, {
    reset();

    let handle = spawn_program(SHELL_PID, loaded_program(0));
    let mut argv = program::ProgramArgvBuffer::empty();
    testrt::check(argv.push("pwd").is_ok(), "replacement argv0 accepted");

    testrt::check_eq(
        replace_program_image(handle.pid, "/bin/pwd", "linked-bin", "bin_pwd", &argv, "pwd"),
        None,
    );
    testrt::check_eq(process_state(handle.pid), ProcessState::Ready);
    testrt::check_eq(process_record(handle.pid).program_path, "/bin/help");
    testrt::check_eq(task_state(handle.task_id), KernelTaskState::Ready);
    testrt::check_eq(task_record(handle.task_id).entry, "/bin/help");
    let original = process_record(handle.pid);
    let original_address_space = original.address_space_id;
    let original_generation = original.image_generation;

    testrt::check_eq(run_process(handle.pid), Some(handle));
    testrt::check_eq(
        replace_program_image(handle.pid, "/bin/pwd", "linked-bin", "bin_pwd", &argv, "pwd"),
        Some(ProcessHandle {
            pid: handle.pid,
            task_id: handle.task_id,
            program_path: "/bin/pwd",
            loader: "linked-bin",
            entry_name: "bin_pwd",
        }),
    );
    let replaced = process_record(handle.pid);
    testrt::check_eq(replaced.state, ProcessState::Running);
    testrt::check_eq(replaced.program_path, "/bin/pwd");
    testrt::check_eq(replaced.loader, "linked-bin");
    testrt::check_eq(replaced.entry_name, "bin_pwd");
    testrt::check_eq(replaced.argv0(), "pwd");
    testrt::check(
        replaced.address_space_id > original_address_space,
        "execve replacement gets fresh address space identity",
    );
    testrt::check_eq(replaced.image_generation, original_generation + 1);
    let old_space =
        mm::address_space(original_address_space).expect("old exec address space retained");
    testrt::check_eq(old_space.owner_pid, handle.pid);
    testrt::check_eq(old_space.state, AddressSpaceState::Retired);
    testrt::check_eq(old_space.program_path, "/bin/help");
    let new_space =
        mm::address_space(replaced.address_space_id).expect("replacement address space retained");
    testrt::check_eq(new_space.owner_pid, handle.pid);
    testrt::check_eq(new_space.state, AddressSpaceState::Active);
    testrt::check_eq(new_space.image_generation, replaced.image_generation);
    testrt::check_eq(new_space.program_path, "/bin/pwd");
    testrt::check_eq(new_space.source_path, "/bin/pwd");
    testrt::check_eq(new_space.stack_bytes, DEFAULT_USER_STACK_BYTES);
    testrt::check_eq(new_space.region_count, 1usize);
    testrt::check_eq(new_space.stack_start, USER_STACK_TOP - DEFAULT_USER_STACK_BYTES);
    testrt::check_eq(new_space.stack_end, USER_STACK_TOP);
    testrt::check_eq(task_record(handle.task_id).entry, "bin_pwd");

    exit_process(handle.pid, ProcessState::Exited, 0);
    let terminal_space =
        mm::address_space(replaced.address_space_id).expect("terminal replacement space retained");
    testrt::check_eq(terminal_space.state, AddressSpaceState::Retired);
    testrt::check_eq(
        replace_program_image(handle.pid, "/bin/cat", "linked-bin", "bin_cat", &argv, "cat"),
        None,
    );
    testrt::check_eq(process_state(handle.pid), ProcessState::Exited);
    let terminal = process_record(handle.pid);
    testrt::check_eq(terminal.program_path, "/bin/pwd");
    testrt::check_eq(terminal.address_space_id, replaced.address_space_id);
    testrt::check_eq(terminal.image_generation, replaced.image_generation);
    testrt::check_eq(task_state(handle.task_id), KernelTaskState::Exited);
    testrt::check_eq(task_record(handle.task_id).entry, "bin_pwd");
});

arch_test!(proc_operator_wake_only_releases_operator_blocked_processes, {
    reset();

    let operator = spawn_program(SHELL_PID, loaded_program(0));
    let _ = run_process(operator.pid);
    let _ = block_process(operator.pid);
    testrt::check_eq(process_record(operator.pid).block_reason, BlockReason::Operator);
    testrt::check_eq(wake_operator_blocked_process(operator.pid), Some(operator));
    testrt::check_eq(process_state(operator.pid), ProcessState::Ready);

    let pipe_reader = spawn_program(SHELL_PID, loaded_program(1));
    let _ = run_process(pipe_reader.pid);
    let _ = block_pipe_read_process(pipe_reader.pid);
    testrt::check_eq(process_record(pipe_reader.pid).block_reason, BlockReason::PipeRead);
    testrt::check_eq(wake_operator_blocked_process(pipe_reader.pid), None);
    testrt::check_eq(process_state(pipe_reader.pid), ProcessState::Blocked);
    testrt::check_eq(process_record(pipe_reader.pid).block_reason, BlockReason::PipeRead);
    testrt::check_eq(wake_process(pipe_reader.pid), Some(pipe_reader));
    testrt::check_eq(process_state(pipe_reader.pid), ProcessState::Ready);

    let parent = spawn_program(SHELL_PID, loaded_program(2));
    let _ = run_process(parent.pid);
    let child =
        spawn_child(parent.pid, parent.task_id, "/bin/child", "selftest-loader", "child_main");
    let _ = begin_wait(parent.pid, child.pid).expect("parent can wait on child");
    testrt::check_eq(process_record(parent.pid).block_reason, BlockReason::WaitChild);
    testrt::check_eq(wake_operator_blocked_process(parent.pid), None);
    testrt::check_eq(process_state(parent.pid), ProcessState::Blocked);
    testrt::check_eq(process_record(parent.pid).block_reason, BlockReason::WaitChild);
});

arch_test!(proc_user_resume_block_rejects_generic_wake, {
    reset();

    let handle = spawn_program(SHELL_PID, loaded_program(0));
    let _ = run_process(handle.pid);
    testrt::check_eq(block_user_resume_process(handle.pid), Some(handle));
    testrt::check_eq(process_state(handle.pid), ProcessState::Blocked);
    testrt::check_eq(process_record(handle.pid).block_reason, BlockReason::UserResume);
    testrt::check_eq(task_state(handle.task_id), KernelTaskState::Blocked);
    testrt::check_eq(task_record(handle.task_id).block_reason, BlockReason::UserResume);

    testrt::check_eq(wake_operator_blocked_process(handle.pid), None);
    testrt::check_eq(wake_process(handle.pid), None);
    testrt::check_eq(process_state(handle.pid), ProcessState::Blocked);
    testrt::check_eq(process_record(handle.pid).block_reason, BlockReason::UserResume);
    testrt::check_eq(task_state(handle.task_id), KernelTaskState::Blocked);
    testrt::check_eq(task_record(handle.task_id).block_reason, BlockReason::UserResume);
});

arch_test!(proc_scheduler_snapshot_correlates_tasks_with_processes, {
    reset();

    let first = spawn_program(SHELL_PID, loaded_program(0));
    let second = spawn_program(SHELL_PID, loaded_program(1));

    let snapshot = sched::snapshot_scheduler();
    testrt::check_eq(snapshot.current_task_id, SHELL_PID);
    testrt::check_eq(snapshot.current_process_id, SHELL_PID);
    testrt::check_eq(snapshot.ready_len, 2usize);
    testrt::check_eq(snapshot.next_ready_task_id(), first.task_id);
    testrt::check_eq(snapshot.next_ready_process_id(), first.pid);
    testrt::check_eq(snapshot.ready_queue[0], first.task_id);
    testrt::check_eq(snapshot.ready_process_queue[0], first.pid);
    testrt::check_eq(snapshot.ready_queue[1], second.task_id);
    testrt::check_eq(snapshot.ready_process_queue[1], second.pid);

    let dispatched = dispatch_next_ready_process().expect("first ready process dispatches");
    testrt::check_eq(dispatched.pid, first.pid);
    testrt::check_eq(dispatched.task_id, first.task_id);

    let snapshot = sched::snapshot_scheduler();
    testrt::check_eq(snapshot.current_task_id, first.task_id);
    testrt::check_eq(snapshot.current_process_id, first.pid);
    testrt::check_eq(snapshot.ready_len, 1usize);
    testrt::check_eq(snapshot.next_ready_task_id(), second.task_id);
    testrt::check_eq(snapshot.next_ready_process_id(), second.pid);
    testrt::check_eq(snapshot.ready_process_queue[0], second.pid);
});

arch_test!(proc_child_with_argv_retains_bounded_arguments, {
    reset();

    let mut argv = program::ProgramArgvBuffer::empty();
    testrt::check(argv.push("cat").is_ok(), "argv0 accepted");
    testrt::check(argv.push("/boot/profile").is_ok(), "argv1 accepted");
    testrt::check(argv.push("/boot/status").is_ok(), "argv2 accepted");
    let mut env = program::ProgramEnvBuffer::empty();
    testrt::check(env.push("REOVIM_MEDIA_PATH", "/boot/status").is_ok(), "env entry accepted");

    let handle = try_spawn_child_with_program_argv_env_mapped(
        SHELL_PID,
        SHELL_PID,
        "/bin/cat",
        "source-image",
        "bin_cat",
        mm::AddressSpaceImage::metadata_only("/bin/cat"),
        &argv,
        &env,
        "cat",
        crate::exec_artifact::ExecArtifactBodyFormat::None,
        crate::exec_artifact::ExecArtifactBodyInnerFormat::None,
    )
    .expect("process slot available");
    let record = process_record(handle.pid);
    testrt::check_eq(record.pid, handle.pid);
    testrt::check_eq(record.parent_pid, SHELL_PID);
    testrt::check_eq(record.task_id, handle.task_id);
    testrt::check_eq(record.program_path, "/bin/cat");
    testrt::check_eq(record.loader, "source-image");
    testrt::check_eq(record.entry_name, "bin_cat");
    testrt::check_eq(record.argc, 3usize);
    testrt::check_eq(record.argv0(), "cat");
    testrt::check_eq(record.argv_was_truncated(0), false);
    testrt::check_eq(record.argv1(), "/boot/profile");
    testrt::check_eq(record.argv_was_truncated(1), false);
    testrt::check_eq(record.argv(2), "/boot/status");
    testrt::check_eq(record.argv_was_truncated(2), false);
    testrt::check_eq(record.argv(3), "");
    testrt::check_eq(record.argv_was_truncated(3), false);
    testrt::check_eq(record.envc, 1usize);
    testrt::check_eq(record.env_name(0), "REOVIM_MEDIA_PATH");
    testrt::check_eq(record.env_value(0), "/boot/status");
    testrt::check_eq(record.env_was_truncated(0), false);
    testrt::check_eq(record.env_name(1), "");
    testrt::check_eq(record.env_value(1), "");
    testrt::check_eq(record.env_was_truncated(1), false);
    testrt::check_eq(task_state(handle.task_id), KernelTaskState::Ready);
});

arch_test!(proc_yield_rejects_non_running_process, {
    reset();

    let handle = spawn_program(SHELL_PID, loaded_program(0));
    let yielded = yield_process(handle.pid);
    testrt::check_eq(yielded, None);
    testrt::check_eq(process_state(handle.pid), ProcessState::Ready);
    testrt::check_eq(task_state(handle.task_id), KernelTaskState::Ready);

    let snapshot = sched::snapshot_scheduler();
    testrt::check_eq(snapshot.current_task_id, SHELL_PID);
    testrt::check_eq(snapshot.ready_len, 1usize);
    testrt::check_eq(snapshot.next_ready_task_id(), handle.task_id);
});

arch_test!(proc_wait_blocks_parent_until_child_exit_then_resumes_parent, {
    reset();

    let parent = spawn_program(SHELL_PID, loaded_program(0));
    let _ = run_process(parent.pid);
    let child =
        spawn_child(parent.pid, parent.task_id, "/payload/test", "selftest-loader", "payload_test");

    let wait = begin_wait(parent.pid, child.pid);
    testrt::check(wait.is_some(), "parent can begin waiting for child");
    let wait = wait.expect("wait record");
    testrt::check_eq(wait.child_state, ProcessState::Ready);
    testrt::check_eq(wait.exit_code, 0);
    testrt::check_eq(wait.completed, false);
    testrt::check_eq(process_state(parent.pid), ProcessState::Blocked);
    testrt::check_eq(task_state(parent.task_id), KernelTaskState::Blocked);
    testrt::check_eq(process_record(parent.pid).block_reason, BlockReason::WaitChild);
    testrt::check_eq(task_record(parent.task_id).block_reason, BlockReason::WaitChild);

    let _ = run_process(child.pid);
    testrt::check_eq(process_state(child.pid), ProcessState::Running);
    exit_process(child.pid, ProcessState::Exited, 0);

    let wait = finish_wait(parent.pid, child.pid).expect("wait completes");
    testrt::check(wait.completed, "wait is marked complete");
    testrt::check_eq(wait.parent_pid, parent.pid);
    testrt::check_eq(wait.child_pid, child.pid);
    testrt::check_eq(wait.child_state, ProcessState::Exited);
    testrt::check_eq(wait.exit_code, 0);
    testrt::check_eq(process_state(child.pid), ProcessState::Reaped);
    testrt::check_eq(task_state(child.task_id), KernelTaskState::Reaped);
    testrt::check_eq(process_state(parent.pid), ProcessState::Running);
    testrt::check_eq(task_state(parent.task_id), KernelTaskState::Running);
    testrt::check_eq(process_record(parent.pid).block_reason, BlockReason::None);
    testrt::check_eq(task_record(parent.task_id).block_reason, BlockReason::None);
    testrt::check(
        finish_wait(parent.pid, child.pid).is_none(),
        "reaped child cannot be waited twice",
    );

    let mut waits = [EMPTY_WAIT_RECORD; MAX_WAITS];
    let count = snapshot_waits(&mut waits);
    testrt::check_eq(count, 1usize);
    testrt::check_eq(waits[0], wait);
});

arch_test!(proc_begin_wait_snapshots_blocked_child_state, {
    reset();

    let parent = spawn_program(SHELL_PID, loaded_program(0));
    let _ = run_process(parent.pid);
    let child =
        spawn_child(parent.pid, parent.task_id, "/bin/child", "selftest-loader", "child_main");
    let _ = block_process(child.pid);

    let wait = begin_wait(parent.pid, child.pid).expect("parent can begin waiting for child");
    testrt::check_eq(wait.parent_pid, parent.pid);
    testrt::check_eq(wait.child_pid, child.pid);
    testrt::check_eq(wait.child_state, ProcessState::Blocked);
    testrt::check_eq(wait.exit_code, 0);
    testrt::check_eq(wait.completed, false);
    testrt::check_eq(process_state(parent.pid), ProcessState::Blocked);
    testrt::check_eq(task_state(parent.task_id), KernelTaskState::Blocked);

    let mut waits = [EMPTY_WAIT_RECORD; MAX_WAITS];
    let count = snapshot_waits(&mut waits);
    testrt::check_eq(count, 1usize);
    testrt::check_eq(waits[0], wait);
});

arch_test!(proc_begin_wait_preserves_active_wait_on_child_pid_slot_collision, {
    reset();

    let parent_a = spawn_program(SHELL_PID, loaded_program(0));
    let _ = run_process(parent_a.pid);
    let child_a =
        spawn_child(parent_a.pid, parent_a.task_id, "/bin/child-a", "selftest-loader", "child_a");
    let wait_a = begin_wait(parent_a.pid, child_a.pid).expect("first wait starts");
    let target_slot = child_a.pid % MAX_WAITS;

    let mut parent_b = EMPTY_PROCESS_HANDLE;
    let mut child_b = EMPTY_PROCESS_HANDLE;
    let mut found_collision = false;
    let mut attempts = 0usize;
    while attempts < MAX_WAITS + 2 && !found_collision {
        parent_b = spawn_program(SHELL_PID, loaded_program(1));
        let _ = run_process(parent_b.pid);
        child_b = spawn_child(
            parent_b.pid,
            parent_b.task_id,
            "/bin/child-b",
            "selftest-loader",
            "child_b",
        );
        if child_b.pid % MAX_WAITS == target_slot {
            found_collision = true;
        } else {
            exit_process(child_b.pid, ProcessState::Exited, 0);
            exit_process(parent_b.pid, ProcessState::Exited, 0);
        }
        attempts += 1;
    }
    testrt::check(found_collision, "test generated a child pid slot collision");

    let wait_b = begin_wait(parent_b.pid, child_b.pid).expect("colliding wait starts");
    testrt::check_eq(wait_record(parent_a.pid, child_a.pid), Some(wait_a));
    testrt::check_eq(wait_record(parent_b.pid, child_b.pid), Some(wait_b));
    testrt::check_eq(process_record(parent_a.pid).block_reason, BlockReason::WaitChild);
    testrt::check_eq(process_record(parent_b.pid).block_reason, BlockReason::WaitChild);

    let mut waits = [EMPTY_WAIT_RECORD; MAX_WAITS];
    let count = snapshot_waits(&mut waits);
    testrt::check_eq(count, 2usize);
});

arch_test!(proc_begin_wait_missing_child_does_not_block_parent, {
    reset();

    let parent = spawn_program(SHELL_PID, loaded_program(0));
    let _ = run_process(parent.pid);

    testrt::check_eq(begin_wait(parent.pid, 999usize), None);
    testrt::check_eq(process_state(parent.pid), ProcessState::Running);
    testrt::check_eq(task_state(parent.task_id), KernelTaskState::Running);
    testrt::check_eq(process_record(parent.pid).block_reason, BlockReason::None);
    testrt::check_eq(task_record(parent.task_id).block_reason, BlockReason::None);

    let mut waits = [EMPTY_WAIT_RECORD; MAX_WAITS];
    testrt::check_eq(snapshot_waits(&mut waits), 0usize);
});

arch_test!(proc_cancel_wait_resumes_parent_without_completing_wait, {
    reset();

    let parent = spawn_program(SHELL_PID, loaded_program(0));
    let _ = run_process(parent.pid);
    let child =
        spawn_child(parent.pid, parent.task_id, "/bin/child", "selftest-loader", "child_main");

    let wait = begin_wait(parent.pid, child.pid);
    testrt::check(wait.is_some(), "parent can begin waiting for child");
    testrt::check_eq(process_state(parent.pid), ProcessState::Blocked);
    testrt::check_eq(task_state(parent.task_id), KernelTaskState::Blocked);
    testrt::check_eq(process_record(parent.pid).block_reason, BlockReason::WaitChild);
    testrt::check_eq(task_record(parent.task_id).block_reason, BlockReason::WaitChild);

    let canceled = cancel_wait(parent.pid, child.pid).expect("wait can be canceled");
    testrt::check_eq(canceled.parent_pid, parent.pid);
    testrt::check_eq(canceled.child_pid, child.pid);
    testrt::check_eq(canceled.completed, false);
    testrt::check_eq(process_state(parent.pid), ProcessState::Running);
    testrt::check_eq(task_state(parent.task_id), KernelTaskState::Running);
    testrt::check_eq(process_record(parent.pid).block_reason, BlockReason::None);
    testrt::check_eq(task_record(parent.task_id).block_reason, BlockReason::None);

    let mut waits = [EMPTY_WAIT_RECORD; MAX_WAITS];
    let count = snapshot_waits(&mut waits);
    testrt::check_eq(count, 0usize);
});

arch_test!(proc_exit_cancels_incomplete_waits_owned_by_parent, {
    reset();

    let parent = spawn_program(SHELL_PID, loaded_program(0));
    let _ = run_process(parent.pid);
    let child =
        spawn_child(parent.pid, parent.task_id, "/bin/child", "selftest-loader", "child_main");

    let wait = begin_wait(parent.pid, child.pid).expect("parent can begin waiting for child");
    testrt::check_eq(wait.parent_pid, parent.pid);
    testrt::check_eq(wait.child_pid, child.pid);
    testrt::check_eq(process_state(parent.pid), ProcessState::Blocked);
    testrt::check_eq(process_record(parent.pid).block_reason, BlockReason::WaitChild);

    let adoption = exit_process(parent.pid, ProcessState::Failed, 1);
    testrt::check_eq(adoption.count, 1usize);
    testrt::check_eq(adoption.children[0].pid, child.pid);
    testrt::check_eq(process_state(parent.pid), ProcessState::Failed);
    testrt::check_eq(process_record(parent.pid).block_reason, BlockReason::None);
    testrt::check_eq(task_state(parent.task_id), KernelTaskState::Failed);

    let child_record = process_record(child.pid);
    testrt::check_eq(child_record.parent_pid, ROOTD_PID);
    testrt::check_eq(child_record.state, ProcessState::Ready);
    testrt::check_eq(task_record(child.task_id).parent_task_id, ROOTD_PID);

    let mut waits = [EMPTY_WAIT_RECORD; MAX_WAITS];
    let count = snapshot_waits(&mut waits);
    testrt::check_eq(count, 0usize);
});

arch_test!(proc_exit_reparents_live_children_to_rootd, {
    reset();

    let parent = spawn_program(SHELL_PID, loaded_program(0));
    let _ = run_process(parent.pid);
    let child =
        spawn_child(parent.pid, parent.task_id, "/bin/child", "selftest-loader", "child_main");
    testrt::check_eq(process_record(child.pid).parent_pid, parent.pid);
    testrt::check_eq(task_record(child.task_id).parent_task_id, parent.task_id);

    let adoption = exit_process(parent.pid, ProcessState::Exited, 0);
    testrt::check_eq(adoption.count, 1usize);
    testrt::check_eq(adoption.children[0], child);

    let child_record = process_record(child.pid);
    testrt::check_eq(child_record.parent_pid, ROOTD_PID);
    testrt::check_eq(child_record.state, ProcessState::Ready);
    let child_task = task_record(child.task_id);
    testrt::check_eq(child_task.parent_task_id, ROOTD_PID);
    testrt::check_eq(child_task.state, KernelTaskState::Ready);
});

arch_test!(proc_scheduler_ready_queue_dispatches_and_yields_fifo, {
    reset();

    let first = spawn_program(SHELL_PID, loaded_program(0));
    let second = spawn_program(SHELL_PID, loaded_program(1));
    let snapshot = sched::snapshot_scheduler();
    testrt::check_eq(snapshot.current_task_id, 2usize);
    testrt::check_eq(snapshot.ready_len, 2usize);
    testrt::check_eq(snapshot.ready_queue[0], first.task_id);
    testrt::check_eq(snapshot.ready_queue[1], second.task_id);
    testrt::check_eq(snapshot.tick_count, 0usize);
    testrt::check_eq(task_record(first.task_id).run_count, 0usize);
    testrt::check_eq(task_record(first.task_id).runtime_ticks, 0usize);

    let running = run_process(first.pid);
    testrt::check_eq(running, Some(first));
    let snapshot = sched::snapshot_scheduler();
    testrt::check_eq(snapshot.current_task_id, first.task_id);
    testrt::check_eq(snapshot.ready_len, 1usize);
    testrt::check_eq(snapshot.next_ready_task_id(), second.task_id);
    testrt::check_eq(snapshot.dispatch_count, 1usize);
    testrt::check_eq(task_record(first.task_id).run_count, 1usize);

    let ticked = sched::tick_kernel_task(first.task_id).expect("running task accepts tick");
    testrt::check_eq(ticked.runtime_ticks, 1usize);
    let snapshot = sched::snapshot_scheduler();
    testrt::check_eq(snapshot.tick_count, 1usize);

    let scheduled = yield_process(first.pid);
    testrt::check_eq(scheduled, Some(second));
    testrt::check_eq(process_state(first.pid), ProcessState::Ready);
    testrt::check_eq(task_state(first.task_id), KernelTaskState::Ready);
    testrt::check_eq(process_state(second.pid), ProcessState::Running);
    testrt::check_eq(task_state(second.task_id), KernelTaskState::Running);

    let snapshot = sched::snapshot_scheduler();
    testrt::check_eq(snapshot.current_task_id, second.task_id);
    testrt::check_eq(snapshot.ready_len, 1usize);
    testrt::check_eq(snapshot.next_ready_task_id(), first.task_id);
    testrt::check_eq(snapshot.yield_count, 1usize);
    testrt::check_eq(snapshot.dispatch_count, 2usize);
    testrt::check_eq(snapshot.tick_count, 1usize);
    testrt::check_eq(task_record(first.task_id).runtime_ticks, 1usize);
    testrt::check_eq(task_record(second.task_id).run_count, 1usize);

    let blocked = block_process(second.pid);
    testrt::check_eq(blocked, Some(second));
    testrt::check_eq(process_state(second.pid), ProcessState::Blocked);
    testrt::check_eq(task_state(second.task_id), KernelTaskState::Blocked);
    testrt::check_eq(process_record(second.pid).block_reason, BlockReason::Operator);
    testrt::check_eq(task_record(second.task_id).block_reason, BlockReason::Operator);

    let woken = wake_process(second.pid);
    testrt::check_eq(woken, Some(second));
    testrt::check_eq(process_state(second.pid), ProcessState::Ready);
    testrt::check_eq(task_state(second.task_id), KernelTaskState::Ready);
    testrt::check_eq(process_record(second.pid).block_reason, BlockReason::None);
    testrt::check_eq(task_record(second.task_id).block_reason, BlockReason::None);

    let snapshot = sched::snapshot_scheduler();
    testrt::check_eq(snapshot.ready_len, 2usize);
    testrt::check_eq(snapshot.ready_queue[0], first.task_id);
    testrt::check_eq(snapshot.ready_queue[1], second.task_id);
});

arch_test!(proc_sleep_wakes_when_tick_deadline_is_due, {
    reset();

    let child = spawn_program(SHELL_PID, loaded_program(0));
    let sleeping = sleep_process_until(child.pid, 2);
    testrt::check_eq(sleeping, Some(child));
    testrt::check_eq(process_state(child.pid), ProcessState::Blocked);
    testrt::check_eq(task_state(child.task_id), KernelTaskState::Blocked);
    testrt::check_eq(process_record(child.pid).block_reason, BlockReason::Sleep);
    testrt::check_eq(task_record(child.task_id).block_reason, BlockReason::Sleep);
    testrt::check_eq(task_record(child.task_id).wake_tick, 2usize);

    let root_shell_task = 2usize;
    testrt::check(sched::tick_kernel_task(root_shell_task).is_some(), "root shell tick succeeds");
    let snapshot = sched::snapshot_scheduler();
    testrt::check_eq(snapshot.tick_count, 1usize);
    let mut woken = [EMPTY_PROCESS_HANDLE; MAX_PROCESSES];
    testrt::check_eq(wake_sleepers_due(snapshot.tick_count, &mut woken), 0usize);
    testrt::check_eq(process_state(child.pid), ProcessState::Blocked);

    testrt::check(
        sched::tick_kernel_task(root_shell_task).is_some(),
        "second root shell tick succeeds",
    );
    let snapshot = sched::snapshot_scheduler();
    testrt::check_eq(snapshot.tick_count, 2usize);
    let count = wake_sleepers_due(snapshot.tick_count, &mut woken);
    testrt::check_eq(count, 1usize);
    testrt::check_eq(woken[0], child);
    testrt::check_eq(process_state(child.pid), ProcessState::Ready);
    testrt::check_eq(task_state(child.task_id), KernelTaskState::Ready);
    testrt::check_eq(task_record(child.task_id).wake_tick, 0usize);

    let snapshot = sched::snapshot_scheduler();
    testrt::check_eq(snapshot.ready_len, 1usize);
    testrt::check_eq(snapshot.next_ready_task_id(), child.task_id);
});

arch_test!(proc_dispatch_next_ready_process_follows_scheduler_fifo, {
    reset();

    let first = spawn_program(SHELL_PID, loaded_program(0));
    let second = spawn_program(SHELL_PID, loaded_program(1));

    let dispatched = dispatch_next_ready_process();
    testrt::check_eq(dispatched, Some(first));
    testrt::check_eq(process_state(first.pid), ProcessState::Running);
    testrt::check_eq(task_state(first.task_id), KernelTaskState::Running);
    testrt::check_eq(process_state(second.pid), ProcessState::Ready);
    testrt::check_eq(task_state(second.task_id), KernelTaskState::Ready);

    let snapshot = sched::snapshot_scheduler();
    testrt::check_eq(snapshot.current_task_id, first.task_id);
    testrt::check_eq(snapshot.ready_len, 1usize);
    testrt::check_eq(snapshot.next_ready_task_id(), second.task_id);

    exit_process(first.pid, ProcessState::Exited, 0);
    let dispatched = dispatch_next_ready_process();
    testrt::check_eq(dispatched, Some(second));
    testrt::check_eq(process_state(second.pid), ProcessState::Running);
    testrt::check_eq(task_state(second.task_id), KernelTaskState::Running);
});
