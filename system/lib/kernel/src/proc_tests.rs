//! Selftests for the bounded process table and scheduler task lifecycle.

use {
    super::{
        EMPTY_PROCESS_RECORD, EMPTY_WAIT_RECORD, MAX_PROCESSES, MAX_WAITS, ProcessRecord,
        ProcessState, ROOTD_PID, SHELL_PID, begin_wait, block_process, cancel_wait,
        dispatch_next_ready_process, exit_process, finish_wait, install_shell_session, reset,
        run_process, snapshot, snapshot_waits, spawn_child, spawn_program, wake_process,
        yield_process,
    },
    crate::{
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

arch_test!(proc_program_task_lifecycle_moves_ready_running_blocked_exited, {
    reset();

    install_shell_session("/bin/sh", "source-image", "bin_sh");

    let shell = process_record(SHELL_PID);
    testrt::check_eq(shell.pid, SHELL_PID);
    testrt::check_eq(shell.parent_pid, ROOTD_PID);
    testrt::check_eq(shell.task_id, SHELL_PID);
    testrt::check_eq(shell.state, ProcessState::Running);
    testrt::check_eq(shell.program_path, "/bin/sh");
    testrt::check_eq(shell.loader, "source-image");
    testrt::check_eq(shell.entry_name, "bin_sh");

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
    testrt::check_eq(process_state(parent.pid), ProcessState::Running);
    testrt::check_eq(task_state(parent.task_id), KernelTaskState::Running);
    testrt::check_eq(process_record(parent.pid).block_reason, BlockReason::None);
    testrt::check_eq(task_record(parent.task_id).block_reason, BlockReason::None);

    let mut waits = [EMPTY_WAIT_RECORD; MAX_WAITS];
    let count = snapshot_waits(&mut waits);
    testrt::check_eq(count, 1usize);
    testrt::check_eq(waits[0], wait);
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

arch_test!(proc_exit_reparents_live_children_to_rootd, {
    reset();

    let parent = spawn_program(SHELL_PID, loaded_program(0));
    let _ = run_process(parent.pid);
    let child =
        spawn_child(parent.pid, parent.task_id, "/bin/child", "selftest-loader", "child_main");
    testrt::check_eq(process_record(child.pid).parent_pid, parent.pid);
    testrt::check_eq(task_record(child.task_id).parent_task_id, parent.task_id);

    exit_process(parent.pid, ProcessState::Exited, 0);

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
