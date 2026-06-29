//! Selftests for the image `/bin` program loader boundary.

use {
    super::{
        LoadedProgram, MAX_PROGRAM_ARG_BYTES, MAX_PROGRAM_ARGS, ProgramArgvBuffer,
        ProgramArgvBuildError, ProgramDescriptor, ProgramImageKind, ProgramStatus, load_argv0,
    },
    crate::{
        source_store::ExecutableSourceStore,
        syscall::{ProgramStdio, ProgramStream},
    },
    reovim_testrt::{self as testrt, arch_test},
};

fn programs() -> &'static [ProgramDescriptor] {
    crate::bin_fixture::programs()
}

fn source_store() -> ExecutableSourceStore {
    ExecutableSourceStore::program_only(crate::bin_fixture::program_sources())
}

fn load(name: &str) -> LoadedProgram {
    load_argv0(programs(), source_store(), name)
        .expect("source store admits program")
        .expect("program loads by argv0")
}

arch_test!(program_loader_resolves_bin_name_and_absolute_path, {
    let by_name = load("help");
    testrt::check_eq(by_name.catalog_index, 0usize);
    testrt::check_eq(by_name.descriptor.name, "help");
    testrt::check_eq(by_name.descriptor.path, "/bin/help");
    testrt::check_eq(by_name.descriptor.entry_name, "bin_help");
    testrt::check_eq(by_name.descriptor.image_kind(), ProgramImageKind::LinkedBin);
    testrt::check_eq(by_name.image_kind, ProgramImageKind::LinkedBin);
    testrt::check_eq(by_name.image_kind, by_name.descriptor.image_kind());
    testrt::check_eq(by_name.image_kind.as_str(), "linked-bin");
    testrt::check_eq(by_name.source_path, "/bin/help");
    testrt::check_eq(by_name.source_bytes(), b"");

    let init = load("init");
    testrt::check_eq(init.descriptor.name, "init");
    testrt::check_eq(init.descriptor.path, "/bin/init");
    testrt::check_eq(init.descriptor.entry_name, "bin_init");
    testrt::check_eq(init.descriptor.image_kind(), ProgramImageKind::LinkedBin);
    testrt::check_eq(init.image_kind, ProgramImageKind::LinkedBin);
    testrt::check_eq(init.image_kind, init.descriptor.image_kind());
    testrt::check_eq(init.image_kind.as_str(), "linked-bin");
    testrt::check_eq(init.source_path, "/bin/init");
    testrt::check_eq(init.source_bytes(), b"");
    let sh = load("sh");
    testrt::check_eq(sh.descriptor.name, "sh");
    testrt::check_eq(sh.descriptor.path, "/bin/sh");
    testrt::check_eq(sh.descriptor.entry_name, "bin_sh");
    testrt::check_eq(sh.descriptor.image_kind(), ProgramImageKind::LinkedBin);
    testrt::check_eq(sh.image_kind, ProgramImageKind::LinkedBin);
    testrt::check_eq(sh.image_kind, sh.descriptor.image_kind());
    testrt::check_eq(sh.image_kind.as_str(), "linked-bin");
    testrt::check_eq(sh.source_path, "/bin/sh");
    testrt::check_eq(sh.source_bytes(), b"");
    let pwd = load("pwd");
    testrt::check_eq(pwd.descriptor.name, "pwd");
    testrt::check_eq(pwd.descriptor.path, "/bin/pwd");
    testrt::check_eq(pwd.descriptor.entry_name, "bin_pwd");
    testrt::check_eq(pwd.descriptor.image_kind(), ProgramImageKind::LinkedBin);
    testrt::check_eq(pwd.image_kind, ProgramImageKind::LinkedBin);
    testrt::check_eq(pwd.image_kind, pwd.descriptor.image_kind());
    testrt::check_eq(pwd.image_kind.as_str(), "linked-bin");
    testrt::check_eq(pwd.source_path, "/bin/pwd");
    testrt::check_eq(pwd.source_bytes(), b"");
    let ls = load("ls");
    testrt::check_eq(ls.descriptor.name, "ls");
    testrt::check_eq(ls.descriptor.path, "/bin/ls");
    testrt::check_eq(ls.descriptor.entry_name, "bin_ls");
    testrt::check_eq(ls.descriptor.image_kind(), ProgramImageKind::LinkedBin);
    testrt::check_eq(ls.image_kind, ProgramImageKind::LinkedBin);
    testrt::check_eq(ls.image_kind, ls.descriptor.image_kind());
    testrt::check_eq(ls.image_kind.as_str(), "linked-bin");
    testrt::check_eq(ls.source_path, "/bin/ls");
    testrt::check_eq(ls.source_bytes(), b"");
    let cd = load("cd");
    testrt::check_eq(cd.descriptor.name, "cd");
    testrt::check_eq(cd.descriptor.path, "/bin/cd");
    testrt::check_eq(cd.descriptor.entry_name, "bin_cd");
    testrt::check_eq(cd.descriptor.image_kind(), ProgramImageKind::LinkedBin);
    testrt::check_eq(cd.image_kind, ProgramImageKind::LinkedBin);
    testrt::check_eq(cd.image_kind, cd.descriptor.image_kind());
    testrt::check_eq(cd.image_kind.as_str(), "linked-bin");
    testrt::check_eq(cd.source_path, "/bin/cd");
    testrt::check_eq(cd.source_bytes(), b"");
    let cat = load("cat");
    testrt::check_eq(cat.descriptor.name, "cat");
    testrt::check_eq(cat.descriptor.path, "/bin/cat");
    testrt::check_eq(cat.descriptor.entry_name, "bin_cat");
    testrt::check_eq(cat.descriptor.image_kind(), ProgramImageKind::LinkedBin);
    testrt::check_eq(cat.image_kind, ProgramImageKind::LinkedBin);
    testrt::check_eq(cat.image_kind, cat.descriptor.image_kind());
    testrt::check_eq(cat.image_kind.as_str(), "linked-bin");
    testrt::check_eq(cat.source_path, "/bin/cat");
    testrt::check_eq(cat.source_bytes(), b"");
    let read = load("read");
    testrt::check_eq(read.descriptor.name, "read");
    testrt::check_eq(read.descriptor.path, "/bin/read");
    testrt::check_eq(read.descriptor.entry_name, "bin_read");
    testrt::check_eq(read.descriptor.image_kind(), ProgramImageKind::LinkedBin);
    testrt::check_eq(read.image_kind, ProgramImageKind::LinkedBin);
    testrt::check_eq(read.image_kind, read.descriptor.image_kind());
    testrt::check_eq(read.image_kind.as_str(), "linked-bin");
    testrt::check_eq(read.source_path, "/bin/read");
    testrt::check_eq(read.source_bytes(), b"");
    let clear = load("clear");
    testrt::check_eq(clear.descriptor.name, "clear");
    testrt::check_eq(clear.descriptor.path, "/bin/clear");
    testrt::check_eq(clear.descriptor.entry_name, "bin_clear");
    testrt::check_eq(clear.descriptor.image_kind(), ProgramImageKind::LinkedBin);
    testrt::check_eq(clear.image_kind, ProgramImageKind::LinkedBin);
    testrt::check_eq(clear.image_kind, clear.descriptor.image_kind());
    testrt::check_eq(clear.image_kind.as_str(), "linked-bin");
    testrt::check_eq(clear.source_path, "/bin/clear");
    testrt::check_eq(clear.source_bytes(), b"");

    let screentest = load("screentest");
    testrt::check_eq(screentest.descriptor.name, "screentest");
    testrt::check_eq(screentest.descriptor.path, "/bin/screentest");
    testrt::check_eq(screentest.descriptor.entry_name, "bin_screentest");
    testrt::check_eq(screentest.descriptor.image_kind(), ProgramImageKind::LinkedBin);
    testrt::check_eq(screentest.image_kind, ProgramImageKind::LinkedBin);
    testrt::check_eq(screentest.image_kind, screentest.descriptor.image_kind());
    testrt::check_eq(screentest.image_kind.as_str(), "linked-bin");
    testrt::check_eq(screentest.source_path, "/bin/screentest");
    testrt::check_eq(screentest.source_bytes(), b"");

    let halt = load("halt");
    testrt::check_eq(halt.descriptor.name, "halt");
    testrt::check_eq(halt.descriptor.path, "/bin/halt");
    testrt::check_eq(halt.descriptor.entry_name, "bin_halt");
    testrt::check_eq(halt.descriptor.image_kind(), ProgramImageKind::LinkedBin);
    testrt::check_eq(halt.image_kind, ProgramImageKind::LinkedBin);
    testrt::check_eq(halt.image_kind, halt.descriptor.image_kind());
    testrt::check_eq(halt.image_kind.as_str(), "linked-bin");
    testrt::check_eq(halt.source_path, "/bin/halt");
    testrt::check_eq(halt.source_bytes(), b"");
    let ps = load("ps");
    testrt::check_eq(ps.descriptor.name, "ps");
    testrt::check_eq(ps.descriptor.path, "/bin/ps");
    testrt::check_eq(ps.descriptor.entry_name, "bin_ps");
    testrt::check_eq(ps.descriptor.image_kind(), ProgramImageKind::LinkedBin);
    testrt::check_eq(ps.image_kind, ProgramImageKind::LinkedBin);
    testrt::check_eq(ps.image_kind, ps.descriptor.image_kind());
    testrt::check_eq(ps.image_kind.as_str(), "linked-bin");
    testrt::check_eq(ps.source_path, "/bin/ps");
    testrt::check_eq(ps.source_bytes(), b"");
    let kill = load("kill");
    testrt::check_eq(kill.descriptor.name, "kill");
    testrt::check_eq(kill.descriptor.path, "/bin/kill");
    testrt::check_eq(kill.descriptor.entry_name, "bin_kill");
    testrt::check_eq(kill.descriptor.image_kind(), ProgramImageKind::LinkedBin);
    testrt::check_eq(kill.image_kind, ProgramImageKind::LinkedBin);
    testrt::check_eq(kill.image_kind, kill.descriptor.image_kind());
    testrt::check_eq(kill.image_kind.as_str(), "linked-bin");
    testrt::check_eq(kill.source_path, "/bin/kill");
    testrt::check_eq(kill.source_bytes(), b"");
    let wake = load("wake");
    testrt::check_eq(wake.descriptor.name, "wake");
    testrt::check_eq(wake.descriptor.path, "/bin/wake");
    testrt::check_eq(wake.descriptor.entry_name, "bin_wake");
    testrt::check_eq(wake.descriptor.image_kind(), ProgramImageKind::LinkedBin);
    testrt::check_eq(wake.image_kind, ProgramImageKind::LinkedBin);
    testrt::check_eq(wake.image_kind, wake.descriptor.image_kind());
    testrt::check_eq(wake.image_kind.as_str(), "linked-bin");
    testrt::check_eq(wake.source_path, "/bin/wake");
    testrt::check_eq(wake.source_bytes(), b"");
    let block = load("block");
    testrt::check_eq(block.descriptor.name, "block");
    testrt::check_eq(block.descriptor.path, "/bin/block");
    testrt::check_eq(block.descriptor.entry_name, "bin_block");
    testrt::check_eq(block.descriptor.image_kind(), ProgramImageKind::LinkedBin);
    testrt::check_eq(block.image_kind, ProgramImageKind::LinkedBin);
    testrt::check_eq(block.image_kind, block.descriptor.image_kind());
    testrt::check_eq(block.image_kind.as_str(), "linked-bin");
    testrt::check_eq(block.source_path, "/bin/block");
    testrt::check_eq(block.source_bytes(), b"");
    let spawn = load("spawn");
    testrt::check_eq(spawn.descriptor.name, "spawn");
    testrt::check_eq(spawn.descriptor.path, "/bin/spawn");
    testrt::check_eq(spawn.descriptor.entry_name, "bin_spawn");
    testrt::check_eq(spawn.descriptor.image_kind(), ProgramImageKind::LinkedBin);
    testrt::check_eq(spawn.image_kind, ProgramImageKind::LinkedBin);
    testrt::check_eq(spawn.image_kind, spawn.descriptor.image_kind());
    testrt::check_eq(spawn.image_kind.as_str(), "linked-bin");
    testrt::check_eq(spawn.source_path, "/bin/spawn");
    testrt::check_eq(spawn.source_bytes(), b"");
    let sleep = load("sleep");
    testrt::check_eq(sleep.descriptor.name, "sleep");
    testrt::check_eq(sleep.descriptor.path, "/bin/sleep");
    testrt::check_eq(sleep.descriptor.entry_name, "bin_sleep");
    testrt::check_eq(sleep.descriptor.image_kind(), ProgramImageKind::LinkedBin);
    testrt::check_eq(sleep.image_kind, ProgramImageKind::LinkedBin);
    testrt::check_eq(sleep.image_kind, sleep.descriptor.image_kind());
    testrt::check_eq(sleep.image_kind.as_str(), "linked-bin");
    testrt::check_eq(sleep.source_path, "/bin/sleep");
    testrt::check_eq(sleep.source_bytes(), b"");
    let wait = load("wait");
    testrt::check_eq(wait.descriptor.name, "wait");
    testrt::check_eq(wait.descriptor.path, "/bin/wait");
    testrt::check_eq(wait.descriptor.entry_name, "bin_wait");
    testrt::check_eq(wait.descriptor.image_kind(), ProgramImageKind::LinkedBin);
    testrt::check_eq(wait.image_kind, ProgramImageKind::LinkedBin);
    testrt::check_eq(wait.image_kind, wait.descriptor.image_kind());
    testrt::check_eq(wait.image_kind.as_str(), "linked-bin");
    testrt::check_eq(wait.source_path, "/bin/wait");
    testrt::check_eq(wait.source_bytes(), b"");
    let wait_ticks = load("wait-ticks");
    testrt::check_eq(wait_ticks.descriptor.name, "wait-ticks");
    testrt::check_eq(wait_ticks.descriptor.path, "/bin/wait-ticks");
    testrt::check_eq(wait_ticks.descriptor.entry_name, "bin_wait_ticks");
    testrt::check_eq(wait_ticks.descriptor.image_kind(), ProgramImageKind::LinkedBin);
    testrt::check_eq(wait_ticks.image_kind, ProgramImageKind::LinkedBin);
    testrt::check_eq(wait_ticks.image_kind, wait_ticks.descriptor.image_kind());
    testrt::check_eq(wait_ticks.image_kind.as_str(), "linked-bin");
    testrt::check_eq(wait_ticks.source_path, "/bin/wait-ticks");
    testrt::check_eq(wait_ticks.source_bytes(), b"");
    let service_stop = load("service-stop");
    testrt::check_eq(service_stop.descriptor.name, "service-stop");
    testrt::check_eq(service_stop.descriptor.path, "/bin/service-stop");
    testrt::check_eq(service_stop.descriptor.entry_name, "bin_service_stop");
    testrt::check_eq(service_stop.descriptor.image_kind(), ProgramImageKind::LinkedBin);
    testrt::check_eq(service_stop.image_kind, ProgramImageKind::LinkedBin);
    testrt::check_eq(service_stop.image_kind, service_stop.descriptor.image_kind());
    testrt::check_eq(service_stop.image_kind.as_str(), "linked-bin");
    testrt::check_eq(service_stop.source_path, "/bin/service-stop");
    testrt::check_eq(service_stop.source_bytes(), b"");
    let service_start = load("service-start");
    testrt::check_eq(service_start.descriptor.name, "service-start");
    testrt::check_eq(service_start.descriptor.path, "/bin/service-start");
    testrt::check_eq(service_start.descriptor.entry_name, "bin_service_start");
    testrt::check_eq(service_start.descriptor.image_kind(), ProgramImageKind::LinkedBin);
    testrt::check_eq(service_start.image_kind, ProgramImageKind::LinkedBin);
    testrt::check_eq(service_start.image_kind, service_start.descriptor.image_kind());
    testrt::check_eq(service_start.image_kind.as_str(), "linked-bin");
    testrt::check_eq(service_start.source_path, "/bin/service-start");
    testrt::check_eq(service_start.source_bytes(), b"");
    let service_restart = load("service-restart");
    testrt::check_eq(service_restart.descriptor.name, "service-restart");
    testrt::check_eq(service_restart.descriptor.path, "/bin/service-restart");
    testrt::check_eq(service_restart.descriptor.entry_name, "bin_service_restart");
    testrt::check_eq(service_restart.descriptor.image_kind(), ProgramImageKind::LinkedBin);
    testrt::check_eq(service_restart.image_kind, ProgramImageKind::LinkedBin);
    testrt::check_eq(service_restart.image_kind, service_restart.descriptor.image_kind());
    testrt::check_eq(service_restart.image_kind.as_str(), "linked-bin");
    testrt::check_eq(service_restart.source_path, "/bin/service-restart");
    testrt::check_eq(service_restart.source_bytes(), b"");
    let hello = load("hello");
    testrt::check_eq(hello.descriptor.name, "hello");
    testrt::check_eq(hello.descriptor.path, "/bin/hello");
    testrt::check_eq(hello.descriptor.entry_name, "bin_hello");
    testrt::check_eq(hello.descriptor.image_kind(), ProgramImageKind::LinkedBin);
    testrt::check_eq(hello.image_kind, ProgramImageKind::LinkedBin);
    testrt::check_eq(hello.image_kind, hello.descriptor.image_kind());
    testrt::check_eq(hello.image_kind.as_str(), "linked-bin");
    testrt::check_eq(hello.source_path, "/bin/hello");
    testrt::check_eq(hello.source_bytes(), b"");
    let mount = load("mount");
    testrt::check_eq(mount.descriptor.name, "mount");
    testrt::check_eq(mount.descriptor.path, "/bin/mount");
    testrt::check_eq(mount.descriptor.entry_name, "bin_mount");
    testrt::check_eq(mount.descriptor.image_kind(), ProgramImageKind::LinkedBin);
    testrt::check_eq(mount.image_kind, ProgramImageKind::LinkedBin);
    testrt::check_eq(mount.image_kind, mount.descriptor.image_kind());
    testrt::check_eq(mount.image_kind.as_str(), "linked-bin");
    testrt::check_eq(mount.source_path, "/bin/mount");
    testrt::check_eq(mount.source_bytes(), b"");
    let device = load("device");
    testrt::check_eq(device.descriptor.name, "device");
    testrt::check_eq(device.descriptor.path, "/bin/device");
    testrt::check_eq(device.descriptor.entry_name, "bin_device");
    testrt::check_eq(device.descriptor.image_kind(), ProgramImageKind::LinkedBin);
    testrt::check_eq(device.image_kind, ProgramImageKind::LinkedBin);
    testrt::check_eq(device.image_kind, device.descriptor.image_kind());
    testrt::check_eq(device.image_kind.as_str(), "linked-bin");
    testrt::check_eq(device.source_path, "/bin/device");
    testrt::check_eq(device.source_bytes(), b"");
    let input = load("input");
    testrt::check_eq(input.descriptor.name, "input");
    testrt::check_eq(input.descriptor.path, "/bin/input");
    testrt::check_eq(input.descriptor.entry_name, "bin_input");
    testrt::check_eq(input.descriptor.image_kind(), ProgramImageKind::LinkedBin);
    testrt::check_eq(input.image_kind, ProgramImageKind::LinkedBin);
    testrt::check_eq(input.image_kind, input.descriptor.image_kind());
    testrt::check_eq(input.image_kind.as_str(), "linked-bin");
    testrt::check_eq(input.source_path, "/bin/input");
    testrt::check_eq(input.source_bytes(), b"");
    let status = load("status");
    testrt::check_eq(status.descriptor.name, "status");
    testrt::check_eq(status.descriptor.path, "/bin/status");
    testrt::check_eq(status.descriptor.entry_name, "bin_status");
    testrt::check_eq(status.descriptor.image_kind(), ProgramImageKind::LinkedBin);
    testrt::check_eq(status.image_kind, ProgramImageKind::LinkedBin);
    testrt::check_eq(status.image_kind, status.descriptor.image_kind());
    testrt::check_eq(status.image_kind.as_str(), "linked-bin");
    testrt::check_eq(status.source_path, "/bin/status");
    testrt::check_eq(status.source_bytes(), b"");
    let proof = load("proof");
    testrt::check_eq(proof.descriptor.name, "proof");
    testrt::check_eq(proof.descriptor.path, "/bin/proof");
    testrt::check_eq(proof.descriptor.entry_name, "bin_proof");
    testrt::check_eq(proof.descriptor.image_kind(), ProgramImageKind::LinkedBin);
    testrt::check_eq(proof.image_kind, ProgramImageKind::LinkedBin);
    testrt::check_eq(proof.image_kind, proof.descriptor.image_kind());
    testrt::check_eq(proof.image_kind.as_str(), "linked-bin");
    testrt::check_eq(proof.source_path, "/bin/proof");
    testrt::check_eq(proof.source_bytes(), b"");
    let dmesg = load("dmesg");
    testrt::check_eq(dmesg.descriptor.name, "dmesg");
    testrt::check_eq(dmesg.descriptor.path, "/bin/dmesg");
    testrt::check_eq(dmesg.descriptor.entry_name, "bin_dmesg");
    testrt::check_eq(dmesg.descriptor.image_kind(), ProgramImageKind::LinkedBin);
    testrt::check_eq(dmesg.image_kind, ProgramImageKind::LinkedBin);
    testrt::check_eq(dmesg.image_kind, dmesg.descriptor.image_kind());
    testrt::check_eq(dmesg.image_kind.as_str(), "linked-bin");
    testrt::check_eq(dmesg.source_path, "/bin/dmesg");
    testrt::check_eq(dmesg.source_bytes(), b"");
    let dump = load("dump");
    testrt::check_eq(dump.descriptor.name, "dump");
    testrt::check_eq(dump.descriptor.path, "/bin/dump");
    testrt::check_eq(dump.descriptor.entry_name, "bin_dump");
    testrt::check_eq(dump.descriptor.image_kind(), ProgramImageKind::LinkedBin);
    testrt::check_eq(dump.image_kind, ProgramImageKind::LinkedBin);
    testrt::check_eq(dump.image_kind, dump.descriptor.image_kind());
    testrt::check_eq(dump.image_kind.as_str(), "linked-bin");
    testrt::check_eq(dump.source_path, "/bin/dump");
    testrt::check_eq(dump.source_bytes(), b"");
    let sched = load("sched");
    testrt::check_eq(sched.descriptor.name, "sched");
    testrt::check_eq(sched.descriptor.path, "/bin/sched");
    testrt::check_eq(sched.descriptor.entry_name, "bin_sched");
    testrt::check_eq(sched.descriptor.image_kind(), ProgramImageKind::LinkedBin);
    testrt::check_eq(sched.image_kind, ProgramImageKind::LinkedBin);
    testrt::check_eq(sched.image_kind, sched.descriptor.image_kind());
    testrt::check_eq(sched.image_kind.as_str(), "linked-bin");
    testrt::check_eq(sched.source_path, "/bin/sched");
    testrt::check_eq(sched.source_bytes(), b"");
    let proc = load("proc");
    testrt::check_eq(proc.descriptor.name, "proc");
    testrt::check_eq(proc.descriptor.path, "/bin/proc");
    testrt::check_eq(proc.descriptor.entry_name, "bin_proc");
    testrt::check_eq(proc.descriptor.image_kind(), ProgramImageKind::LinkedBin);
    testrt::check_eq(proc.image_kind, ProgramImageKind::LinkedBin);
    testrt::check_eq(proc.image_kind, proc.descriptor.image_kind());
    testrt::check_eq(proc.image_kind.as_str(), "linked-bin");
    testrt::check_eq(proc.source_path, "/bin/proc");
    testrt::check_eq(proc.source_bytes(), b"");
    let exec = load("exec");
    testrt::check_eq(exec.descriptor.name, "exec");
    testrt::check_eq(exec.descriptor.path, "/bin/exec");
    testrt::check_eq(exec.descriptor.entry_name, "bin_exec");
    testrt::check_eq(exec.descriptor.image_kind(), ProgramImageKind::LinkedBin);
    testrt::check_eq(exec.image_kind, ProgramImageKind::LinkedBin);
    testrt::check_eq(exec.image_kind, exec.descriptor.image_kind());
    testrt::check_eq(exec.image_kind.as_str(), "linked-bin");
    testrt::check_eq(exec.source_path, "/bin/exec");
    testrt::check_eq(exec.source_bytes(), b"");
    let probe = load("probe");
    testrt::check_eq(probe.descriptor.name, "probe");
    testrt::check_eq(probe.descriptor.path, "/bin/probe");
    testrt::check_eq(probe.descriptor.entry_name, "bin_probe");
    testrt::check_eq(probe.descriptor.image_kind(), ProgramImageKind::LinkedBin);
    testrt::check_eq(probe.image_kind, ProgramImageKind::LinkedBin);
    testrt::check_eq(probe.image_kind, probe.descriptor.image_kind());
    testrt::check_eq(probe.image_kind.as_str(), "linked-bin");
    testrt::check_eq(probe.source_path, "/bin/probe");
    testrt::check_eq(probe.source_bytes(), b"");
    let launch = load("launch");
    testrt::check_eq(launch.descriptor.name, "launch");
    testrt::check_eq(launch.descriptor.path, "/bin/launch");
    testrt::check_eq(launch.descriptor.entry_name, "bin_launch");
    testrt::check_eq(launch.descriptor.image_kind(), ProgramImageKind::LinkedBin);
    testrt::check_eq(launch.image_kind, ProgramImageKind::LinkedBin);
    testrt::check_eq(launch.image_kind, launch.descriptor.image_kind());
    testrt::check_eq(launch.image_kind.as_str(), "linked-bin");
    testrt::check_eq(launch.source_path, "/bin/launch");
    testrt::check_eq(launch.source_bytes(), b"");
    let reovim = load("reovim");
    testrt::check_eq(reovim.descriptor.name, "reovim");
    testrt::check_eq(reovim.descriptor.path, "/bin/reovim");
    testrt::check_eq(reovim.descriptor.entry_name, "bin_reovim");
    testrt::check_eq(reovim.descriptor.image_kind(), ProgramImageKind::LinkedBin);
    testrt::check_eq(reovim.image_kind, ProgramImageKind::LinkedBin);
    testrt::check_eq(reovim.image_kind, reovim.descriptor.image_kind());
    testrt::check_eq(reovim.image_kind.as_str(), "linked-bin");
    testrt::check_eq(reovim.source_path, "/bin/reovim");
    testrt::check_eq(reovim.source_bytes(), b"");

    let by_path = load_argv0(programs(), source_store(), "/bin/help")
        .expect("source store admits program")
        .expect("help loads by absolute path");
    testrt::check_eq(by_path.catalog_index, by_name.catalog_index);
    testrt::check_eq(by_path.descriptor.path, by_name.descriptor.path);

    testrt::check(
        load_argv0(programs(), source_store(), "missing")
            .expect("source lookup succeeds for missing argv0")
            .is_none(),
        "unknown basename does not load",
    );
    testrt::check(
        load_argv0(programs(), source_store(), "/boot/help")
            .expect("source lookup succeeds for non-/bin argv0")
            .is_none(),
        "non-/bin path does not load as a program",
    );
});

arch_test!(program_argv_buffer_retains_owned_arguments, {
    let mut argv = ProgramArgvBuffer::empty();
    argv.push("cat").expect("argv0 fits");
    argv.push("/boot/profile").expect("arg fits");

    testrt::check_eq(argv.argc(), 2usize);
    testrt::check_eq(argv.argv0(), Some("cat"));
    testrt::check_eq(argv.arg(1), Some("/boot/profile"));

    let borrowed = argv.borrowed();
    testrt::check_eq(borrowed.argc(), 2usize);
    testrt::check_eq(borrowed.arg(0), Some("cat"));
    testrt::check_eq(borrowed.arg(1), Some("/boot/profile"));
});

arch_test!(program_argv_buffer_rejects_overflow, {
    let mut argv = ProgramArgvBuffer::empty();
    let mut index = 0usize;
    while index < MAX_PROGRAM_ARGS {
        argv.push("x").expect("arg slot fits");
        index += 1;
    }
    testrt::check_eq(argv.push("overflow").err(), Some(ProgramArgvBuildError::TooManyArgs));

    let long = [b'a'; MAX_PROGRAM_ARG_BYTES + 1];
    // SAFETY: the fixture is ASCII-only.
    let long = unsafe { core::str::from_utf8_unchecked(&long) };
    let mut argv = ProgramArgvBuffer::empty();
    testrt::check_eq(argv.push(long).err(), Some(ProgramArgvBuildError::ArgTooLong));
});

arch_test!(program_status_exit_code_reports_numeric_success_and_failure, {
    testrt::check_eq(ProgramStatus::ExitCode(0).as_bytes(), b"exit-code");
    testrt::check_eq(ProgramStatus::ExitCode(0).exit_code(), 0);
    testrt::check(ProgramStatus::ExitCode(0).is_success(), "zero exit code is success");
    testrt::check_eq(ProgramStatus::ExitCode(7).exit_code(), 7);
    testrt::check(!ProgramStatus::ExitCode(7).is_success(), "nonzero exit code is failure");
    testrt::check_eq(ProgramStatus::Replaced.as_bytes(), b"replaced");
    testrt::check_eq(ProgramStatus::Replaced.exit_code(), 0);
    testrt::check(ProgramStatus::Replaced.is_success(), "replaced image is success");
    testrt::check_eq(ProgramStatus::Blocked.as_bytes(), b"blocked");
    testrt::check_eq(ProgramStatus::Blocked.exit_code(), 0);
    testrt::check(!ProgramStatus::Blocked.is_success(), "blocked image is not complete");
});

arch_test!(program_catalog_exposes_only_bin_program_entries, {
    let mut index = 0usize;
    let mut linked_bins = 0usize;
    while index < programs().len() {
        let descriptor = &programs()[index];
        testrt::check(
            descriptor.path.starts_with("/bin/"),
            "program descriptor path stays under /bin",
        );
        testrt::check(descriptor.entry_name.starts_with("bin_"), "program entry name uses bin_*");
        if descriptor.image_kind() == ProgramImageKind::LinkedBin {
            linked_bins += 1;
        } else {
            testrt::check_eq(descriptor.image_kind(), ProgramImageKind::SourceImage);
        }
        testrt::check_eq(
            load_argv0(programs(), source_store(), descriptor.name)
                .expect("source store admits program")
                .expect("program loads by /bin basename")
                .image_kind,
            descriptor.image_kind(),
        );
        testrt::check_eq(
            load_argv0(programs(), source_store(), descriptor.name)
                .expect("source store admits program")
                .expect("program loads by /bin basename")
                .descriptor
                .path,
            descriptor.path,
        );
        testrt::check_eq(
            load_argv0(programs(), source_store(), descriptor.name)
                .expect("source store admits program")
                .expect("program loads by /bin basename")
                .source_path,
            descriptor.path,
        );

        index += 1;
    }
    testrt::check_eq(linked_bins, 51usize);
});

arch_test!(program_stdio_uses_standard_reovim_descriptors, {
    let stdio = ProgramStdio::standard();

    testrt::check_eq(stdio.stdin.fd, 0usize);
    testrt::check_eq(stdio.stdin.stream, ProgramStream::Stdin);
    testrt::check_eq(stdio.stdin.name(), "stdin");
    testrt::check(stdio.stdin.readable(), "stdin is readable");
    testrt::check(!stdio.stdin.writable(), "stdin is not writable");

    testrt::check_eq(stdio.stdout.fd, 1usize);
    testrt::check_eq(stdio.stdout.stream, ProgramStream::Stdout);
    testrt::check_eq(stdio.stdout.name(), "stdout");
    testrt::check(!stdio.stdout.readable(), "stdout is not readable");
    testrt::check(stdio.stdout.writable(), "stdout is writable");

    testrt::check_eq(stdio.stderr.fd, 2usize);
    testrt::check_eq(stdio.stderr.stream, ProgramStream::Stderr);
    testrt::check_eq(stdio.stderr.name(), "stderr");
    testrt::check(!stdio.stderr.readable(), "stderr is not readable");
    testrt::check(stdio.stderr.writable(), "stderr is writable");
});
