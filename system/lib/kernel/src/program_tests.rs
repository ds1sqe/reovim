//! Selftests for the image `/bin` program loader boundary.

use {
    super::{
        LoadedProgram, MAX_PROGRAM_ARG_BYTES, MAX_PROGRAM_ARGS, ProgramArgvBuffer,
        ProgramArgvBuildError, ProgramDescriptor, ProgramImageKind, load_argv0,
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

fn check_source_image(name: &str, path: &str, expected: &[u8]) {
    let loaded = load(name);
    testrt::check_eq(loaded.descriptor.path, path);
    testrt::check_eq(loaded.image_kind, ProgramImageKind::SourceImage);
    testrt::check_eq(loaded.image_kind, loaded.descriptor.image_kind());
    testrt::check_eq(loaded.image_kind.as_str(), "source-image");
    testrt::check_eq(loaded.source_path, path);
    testrt::check_eq(loaded.source_bytes(), expected);
}

fn count_lines_starting_with(bytes: &[u8], prefix: &[u8]) -> usize {
    let mut count = 0usize;
    let mut start = 0usize;
    let mut index = 0usize;

    while index <= bytes.len() {
        if index == bytes.len() || bytes[index] == b'\n' {
            if bytes[start..index].starts_with(prefix) {
                count += 1;
            }
            start = index + 1;
        }
        index += 1;
    }

    count
}

arch_test!(program_loader_resolves_bin_name_and_absolute_path, {
    let by_name = load("help");
    testrt::check_eq(by_name.catalog_index, 0usize);
    testrt::check_eq(by_name.descriptor.name, "help");
    testrt::check_eq(by_name.descriptor.path, "/bin/help");
    testrt::check_eq(by_name.descriptor.image_kind(), ProgramImageKind::SourceImage);
    testrt::check_eq(by_name.image_kind, ProgramImageKind::SourceImage);
    testrt::check_eq(by_name.image_kind, by_name.descriptor.image_kind());
    testrt::check_eq(by_name.image_kind.as_str(), "source-image");
    testrt::check_eq(by_name.source_path, "/bin/help");
    testrt::check_eq(
        by_name.source_bytes(),
        b"reovim-source-v1\nreject-argc-greater 2 help: too many arguments\nwrite-help-arg1-or-catalog\n",
    );

    check_source_image(
        "pwd",
        "/bin/pwd",
        b"reovim-source-v1\nreject-argc-greater 1 pwd: too many arguments\nwrite-cwd-line\n",
    );
    check_source_image(
        "ls",
        "/bin/ls",
        b"reovim-source-v1\nreject-argc-greater 2 ls: too many arguments\nwrite-vfs-listing-arg1-or-cwd\n",
    );
    check_source_image(
        "cd",
        "/bin/cd",
        b"reovim-source-v1\nreject-argc-greater 2 cd: too many arguments\nset-cwd-arg1-or-root\n",
    );
    check_source_image(
        "cat",
        "/bin/cat",
        b"reovim-source-v1\nwrite-stdin-or-vfs-files-argv-tail\n",
    );
    check_source_image(
        "read",
        "/bin/read",
        b"reovim-source-v1\nreject-argc-greater 1 read: too many arguments\nwrite-tty-line\n",
    );
    check_source_image(
        "clear",
        "/bin/clear",
        b"reovim-source-v1\nreject-argc-greater 1 clear: too many arguments\nclear-console\nwrite-stdout-hex 1b5b324a1b5b48\n",
    );

    let screentest = load("screentest");
    testrt::check_eq(screentest.descriptor.path, "/bin/screentest");
    testrt::check_eq(screentest.image_kind, ProgramImageKind::SourceImage);
    testrt::check_eq(screentest.source_path, "/bin/screentest");
    let screentest_bytes = screentest.source_bytes();
    testrt::check(
        screentest_bytes.starts_with(
            b"reovim-source-v1\nreject-argc-greater 1 screentest: too many arguments\n",
        ),
        "/bin/screentest source bytes have header and argc guard",
    );
    testrt::check_eq(count_lines_starting_with(screentest_bytes, b"write-stdout-hex "), 19usize);
    testrt::check(
        screentest_bytes.ends_with(b"write-stdout-hex 2020646f6e650a\n"),
        "/bin/screentest source bytes end with done line",
    );

    check_source_image(
        "halt",
        "/bin/halt",
        b"reovim-source-v1\nreject-argc-greater 1 halt: too many arguments\nwrite-stdout-hex 68616c743a206f6b0a\nexit-status halt\n",
    );
    check_source_image(
        "mount",
        "/bin/mount",
        b"reovim-source-v1\nreject-argc-greater 1 mount: too many arguments\nwrite-mount-table\n",
    );
    check_source_image(
        "device",
        "/bin/device",
        b"reovim-source-v1\nreject-argc-greater 1 device: too many arguments\nwrite-boot-info-summary\nwrite-device-inventory\n",
    );
    check_source_image(
        "input",
        "/bin/input",
        b"reovim-source-v1\nreject-argc-greater 1 input: too many arguments\nwrite-boot-input\n",
    );
    check_source_image(
        "status",
        "/bin/status",
        b"reovim-source-v1\nreject-argc-greater 1 status: too many arguments\nwrite-boot-status\n",
    );
    check_source_image(
        "proof",
        "/bin/proof",
        b"reovim-source-v1\nreject-argc-greater 1 proof: too many arguments\nwrite-boot-proof\n",
    );
    check_source_image(
        "dmesg",
        "/bin/dmesg",
        b"reovim-source-v1\nreject-argc-greater 2 dmesg: too many arguments\ndispatch-arg1 dmesg: unknown option\ndefault\nwrite-kernel-log-view\ncase --stats\nwrite-kernel-log-stats\nend-dispatch-arg1\n",
    );
    check_source_image(
        "dump",
        "/bin/dump",
        b"reovim-source-v1\nreject-argc-greater 2 dump: too many arguments\ndispatch-arg1 dump: unknown subcommand\ndefault\nwrite-dump-status\ncase status\nwrite-dump-status\ncase snapshot\nwrite-dump-snapshot\ncase sync\nwrite-dump-sync\nexit-status error\nend-dispatch-arg1\n",
    );
    check_source_image(
        "sched",
        "/bin/sched",
        b"reovim-source-v1\nreject-argc-greater 2 sched: too many arguments\ndispatch-arg1 sched: unknown subcommand\ndefault\nwrite-scheduler-state\ncase status\nwrite-scheduler-state\ncase tick\nwrite-scheduler-tick\nwrite-scheduler-state\ncase yield\nwrite-scheduler-yield\nwrite-scheduler-state\nend-dispatch-arg1\n",
    );
    check_source_image(
        "proc",
        "/bin/proc",
        b"reovim-source-v1\nreject-argc-greater-unless-arg1 2 exec,spawn,block,wait,wake,kill,install-bin,install-payload,install-bin-media,install-payload-media proc: too many arguments\ndispatch-arg1 proc: unknown subcommand\ndefault\nwrite-process-table\ncase processes\nwrite-process-table\ncase execs\nwrite-exec-load-table\ncase pending\nwrite-pending-exec-table\ncase self\nwrite-process-self\ncase sources\nwrite-source-store-table\ncase tasks\nwrite-task-table\ncase waits\nwrite-wait-table\ncase syscalls\nwrite-syscall-table\ncase scheduler\nwrite-scheduler-state\ncase exec\nproc-exec-argv-tail\ncase spawn\nproc-spawn-argv-tail\ncase block\nproc-block-argv-tail\ncase wait\nproc-wait-pid-arg2\ncase wake\nproc-wake-pid-arg2\ncase kill\nproc-kill-pid-arg2\ncase install-bin\nproc-install-bin-arg2-arg3\ncase install-payload\nproc-install-payload-arg2-arg3\ncase install-bin-media\nproc-install-bin-media-arg2\ncase install-payload-media\nproc-install-payload-media-arg2\nend-dispatch-arg1\n",
    );
    check_source_image(
        "probe",
        "/bin/probe",
        b"reovim-source-v1\nreject-argc-greater 2 probe: too many arguments\nrun-provider-probe-arg1\n",
    );
    check_source_image(
        "launch",
        "/bin/launch",
        b"reovim-source-v1\nrequire-launch-enabled launch disabled for this profile\nlaunch-payload-arg1-or-list\n",
    );
    check_source_image(
        "reovim",
        "/bin/reovim",
        b"reovim-source-v1\nreject-argc-greater 1 reovim: too many arguments\nrequire-launch-enabled reovim disabled for this profile\nwrite-stdout-hex 72656f76696d3a20\nlaunch-payload-name reovim\n",
    );

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

arch_test!(program_catalog_exposes_only_bin_program_entries, {
    let mut index = 0usize;
    while index < programs().len() {
        let descriptor = &programs()[index];
        testrt::check(
            descriptor.path.starts_with("/bin/"),
            "program descriptor path stays under /bin",
        );
        testrt::check(descriptor.entry_name.starts_with("bin_"), "program entry name uses bin_*");
        testrt::check_eq(descriptor.image_kind(), ProgramImageKind::SourceImage);
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
