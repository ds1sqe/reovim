//! Integration tests that build the `#![no_std] #![no_main]` arch fixtures,
//! exec the built binaries, and assert their exit codes and flushed LOG2
//! output (#785 Phase 4 acceptance).
//!
//! Bootstrap state 1 (1.2 §10): this is a std libtest integration binary —
//! the `no_std` test runner is Phase 5. It only orchestrates fixture builds
//! and process execs; the fixtures themselves fly the `no_std` flight code.
//!
//! Coverage note: the panic line's *exact bytes* vary per run (the LOG2
//! timestamp is the live monotonic clock), so the assertions PARSE the line
//! against the LOG2 grammar (9.5 §2) and match the level/emitter/address and
//! message fields — never exact bytes.
//!
//! # Cross-target env contract
//!
//! Two optional environment variables control cross-target builds and execution.
//! Both are unset by default; native host behavior is byte-for-byte unchanged.
//!
//! - **`ARCH_FIXTURE_TARGET`** (e.g. `aarch64-unknown-linux-gnu`): when set,
//!   every `cargo build --package <pkg>` call gains `--target $ARCH_FIXTURE_TARGET`.
//!   The artifact path is read from cargo's JSON `"executable"` field, which
//!   cargo sets to the target-subdir path automatically — the parser here
//!   is path-agnostic and requires no adjustment.
//!
//! - **`ARCH_FIXTURE_RUNNER`** (e.g. `qemu-aarch64` or
//!   `qemu-aarch64 -cpu cortex-a72`): when set, fixture binaries are executed
//!   as `$ARCH_FIXTURE_RUNNER <exe> <args...>` rather than `<exe> <args...>`.
//!   The value is whitespace-split so multi-word runners with flags work.
//!   `qemu-user` forwards the guest exit code as its own exit code, so no
//!   assertion changes are needed.
//! - **`REOVIM_OS_BOOTLINE`** (build time only): newline-separated commands fed to
//!   `reovim-os` via `option_env!` for deterministic shell-transcript tests.
//!
//! `ARCH_FIXTURE_TARGET` without `ARCH_FIXTURE_RUNNER` is valid only when the
//! host can execute the target binaries natively (e.g. same ISA, different
//! vendor tuple). `ARCH_FIXTURE_RUNNER` without `ARCH_FIXTURE_TARGET` is
//! harmless but pointless.
//!
//! Example cross-target invocation:
//! ```text
//! ARCH_FIXTURE_TARGET=aarch64-unknown-linux-gnu \
//! ARCH_FIXTURE_RUNNER=qemu-aarch64 \
//!     cargo test -p reovim-arch --test fixtures_exec
//! # OS shell transcript: build-time scripted input
//! REOVIM_OS_BOOTLINE='help\ndevice\ndmesg\n' \
//! ARCH_FIXTURE_TARGET=x86_64-unknown-none \
//! ARCH_FIXTURE_RUNNER=qemu-system-x86_64 \
//!     cargo test -p reovim-arch --test fixtures_exec os_shell_profile_transcript_on_x86_target_boots_and_prints_cli
//! ```
//!
//! # System-image mode (bare metal)
//!
//! A freestanding `ARCH_FIXTURE_TARGET` (suffix `-none`) selects the third
//! execution mode: the built ELF is repackaged into a bootable image and
//! booted under a system emulator instead of exec'd as a process. The runner
//! value stays a bare program name — the machine flags are owned by
//! `run_system_image`, not threaded through the environment — and dispatch on
//! the arch:
//!
//! ```text
//! # aarch64: raw kernel8.img on raspi4b, semihosting exit
//! ARCH_FIXTURE_TARGET=aarch64-unknown-none \
//! ARCH_FIXTURE_RUNNER=qemu-system-aarch64 \
//!     cargo test -p reovim-arch --test fixtures_exec
//!
//! # x86_64: Multiboot1 ELF32 on q35, isa-debug-exit exit
//! ARCH_FIXTURE_TARGET=x86_64-unknown-none \
//! ARCH_FIXTURE_RUNNER=qemu-system-x86_64 \
//!     cargo test -p reovim-arch --test fixtures_exec
//! ```
//!
//! Bare metal has no process ABI: no argv (so no file-sink fixtures), no
//! filesystem, no threads. Only the arch-selftest pilots run in this mode —
//! the image's exit code arrives through QEMU's exit channel (semihosting on
//! aarch64, `isa-debug-exit` on `x86_64`) and the LOG2 panic line through the
//! serial console on stdout. Every other test skips itself, loudly, when the
//! target is freestanding.

use std::{
    fs::File,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Mutex, OnceLock},
    thread,
    time::{Duration, Instant},
};

/// Wall-clock ceiling on a single system-image boot. A boot hang (triple-fault
/// reset loop, a wedged poll) must fail the test deterministically rather than
/// block CI; one minute is far above a healthy boot (~1 s) yet bounds the worst
/// case.
const BOOT_TIMEOUT: Duration = Duration::from_mins(1);

/// Sentinel exit code returned when a system-image boot exceeds [`BOOT_TIMEOUT`]
/// — distinct from the floor's success (0) and halt (70) codes so a hang is an
/// unambiguous test failure.
const BOOT_TIMEOUT_CODE: i32 = 124;

// ---------------------------------------------------------------------------
// Serialization guards for the selftest-runner tests
//
// `testrt_pilot_all_pass_exits_zero` and `testrt_pilot_one_failure_exits_nonzero`
// both build package `arch-selftest` with DIFFERENT feature sets (`[]` vs
// `["inject-failure"]`), and cargo writes both variants to the same artifact
// path. The guard must therefore span build AND exec: serializing only the
// builds would still let one test relink the binary between the other test's
// build and exec, swapping in the wrong feature variant. A per-test scratch
// copy (copy-under-lock, exec the private copy) would also work but adds
// filesystem churn for the same effect.
//
// The two `uapi-selftest` tests share the identical hazard over their own
// artifact path, so they take their own guard (separate lock: the two
// packages' artifacts do not collide with each other, only with their own
// feature variants).
//
// A poisoned lock is recovered deliberately: each guard protects a cargo
// artifact that the next build regenerates, not in-memory state, so a panic
// in one test (a failed assertion) leaves nothing corrupt behind.
// ---------------------------------------------------------------------------
fn selftest_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The `uapi-selftest` artifact guard — same shape and rationale as
/// [`selftest_lock`], over the uapi runner's artifact path.
fn uapi_selftest_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn os_image_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

// ---------------------------------------------------------------------------
// Env var helpers
// ---------------------------------------------------------------------------

/// Returns `$ARCH_FIXTURE_TARGET` if set and non-empty, otherwise `None`.
fn fixture_target() -> Option<String> {
    std::env::var("ARCH_FIXTURE_TARGET")
        .ok()
        .filter(|v| !v.is_empty())
}

/// Returns `$ARCH_FIXTURE_RUNNER` split on whitespace into tokens, or an empty
/// `Vec` when the variable is unset or empty (meaning: direct exec).
fn fixture_runner() -> Vec<String> {
    std::env::var("ARCH_FIXTURE_RUNNER")
        .ok()
        .filter(|v| !v.is_empty())
        .map(|v| v.split_whitespace().map(str::to_owned).collect())
        .unwrap_or_default()
}

/// True when the cross-target is freestanding (system-image mode): the
/// fixture is a bootable machine image, not an executable process.
fn target_is_none() -> bool {
    fixture_target().is_some_and(|t| t.ends_with("-none"))
}

/// Skips a test that has no realization in system-image mode (needs a
/// process ABI: argv file sinks, a filesystem, or a thread floor), with a
/// loud marker so a green run cannot be mistaken for bare-metal coverage.
/// Returns `true` when the caller should return immediately.
fn skip_in_system_image_mode(test: &str) -> bool {
    if target_is_none() {
        eprintln!("{test}: SKIPPED in system-image mode (needs a process ABI)");
        return true;
    }
    false
}

// ---------------------------------------------------------------------------
// Build helpers
// ---------------------------------------------------------------------------

/// Builds the fixture crate `pkg` (with its `runtime` feature) and returns the
/// path to the built executable, parsed from cargo's JSON build messages so no
/// target dir / profile is hardcoded.
///
/// The fixtures live in a nested workspace (`arch/tests/fixtures/`) excluded
/// from the parent, so the build is scoped by that manifest path — keeping the
/// `runtime` feature off the parent's `cargo test --workspace` graph (see the
/// nested workspace's `Cargo.toml` for the lang-item rationale).
///
/// When `ARCH_FIXTURE_TARGET` is set the build gains `--target <value>` and
/// cargo places the artifact under `target/<triple>/debug/`; the JSON
/// `"executable"` field already reflects the correct path so no extra handling
/// is needed.
fn build_fixture(pkg: &str) -> PathBuf {
    build_fixture_features(pkg, &[])
}

/// Builds the fixture crate `pkg` with the given cargo `features` enabled and
/// returns the built executable path. The features select bin variants (e.g.
/// the test-runner pilot's deliberately-failing test) without a second crate.
///
/// See [`build_fixture`] for the `ARCH_FIXTURE_TARGET` contract.
fn build_fixture_features(pkg: &str, features: &[&str]) -> PathBuf {
    let fixtures_dir = fixtures_workspace_dir();
    build_workspace_binary_with_env(&fixtures_dir, pkg, features, &[])
}

/// Builds `reovim-os` for a scripted root shell transcript and returns the
/// built executable. This uses the same JSON output parsing contract and target
/// env handling as `build_fixture`.
fn build_os_image(features: &[&str], bootline: Option<&str>, os_profile: Option<&str>) -> PathBuf {
    let apps_dir = apps_workspace_dir();
    let mut envs: Vec<(&str, &str)> = Vec::new();
    if let Some(script) = bootline {
        envs.push(("REOVIM_OS_BOOTLINE", script));
    }
    if let Some(profile) = os_profile {
        envs.push(("REOVIM_OS_PROFILE", profile));
    }
    build_workspace_binary_with_env(&apps_dir, "reovim-os", features, &envs)
}

/// Builds one package in the specified nested Cargo workspace and returns the
/// built executable path.
fn build_workspace_binary_with_env(
    workspace_dir: &Path,
    pkg: &str,
    features: &[&str],
    envs: &[(&str, &str)],
) -> PathBuf {
    let mut args = vec![
        "build".to_owned(),
        "--package".to_owned(),
        pkg.to_owned(),
        "--message-format=json-render-diagnostics".to_owned(),
    ];
    if let Some(target) = fixture_target() {
        args.push("--target".to_owned());
        args.push(target);
    }
    if !features.is_empty() {
        args.push("--features".to_owned());
        args.push(features.join(","));
    }
    let mut command = Command::new(env!("CARGO"));
    command
        .current_dir(workspace_dir)
        .args(&args)
        .stderr(Stdio::inherit());
    for (key, value) in envs {
        command.env(key, value);
    }
    let output = command.output().expect("cargo build runs");
    assert!(output.status.success(), "package {pkg} builds");

    // Each JSON line is a build message; the `compiler-artifact` for the bin
    // target carries `"executable":"<path>"`. Find the last one for `pkg`.
    let stdout = String::from_utf8(output.stdout).expect("cargo json is utf8");
    let exe = stdout
        .lines()
        .filter(|l| l.contains("\"compiler-artifact\""))
        .filter(|l| l.contains(pkg))
        .filter_map(extract_executable)
        .next_back()
        .unwrap_or_else(|| panic!("no executable artifact for {pkg}"));
    PathBuf::from(exe)
}

fn fixtures_workspace_dir() -> PathBuf {
    // `CARGO_MANIFEST_DIR` is `arch/`; the nested fixture workspace lives at
    // `arch/tests/fixtures/`.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

fn apps_workspace_dir() -> PathBuf {
    // `CARGO_MANIFEST_DIR` is `arch/`; app composition crates live under `apps/`
    // one directory up.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("apps")
}

/// Extracts the `"executable":"..."` value from one cargo JSON message line,
/// or `None` when the field is absent or null. A minimal hand parser keeps the
/// test free of a JSON dependency (L9: zero third-party).
fn extract_executable(line: &str) -> Option<String> {
    let key = "\"executable\":";
    let start = line.find(key)? + key.len();
    let rest = line[start..].trim_start();
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(rest[..end].to_owned())
}

// ---------------------------------------------------------------------------
// Exec helpers
// ---------------------------------------------------------------------------

/// Runs `exe` with `args`, returns its exit code (the value the process exited
/// with; the test asserts the disposition codes).
///
/// When `ARCH_FIXTURE_RUNNER` is set the invocation becomes
/// `<runner_tokens...> <exe> <args...>`, allowing `qemu-aarch64` (or any other
/// user-mode emulator) to execute cross-compiled fixture binaries.  The runner
/// forwards the guest exit code as its own, so the caller's assertion logic is
/// identical in both paths.
fn run(exe: &Path, args: &[&str]) -> i32 {
    let runner = fixture_runner();
    let status = if runner.is_empty() {
        Command::new(exe)
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("fixture execs")
    } else {
        // Prepend the runner: `runner[0] runner[1..] exe args...`
        Command::new(&runner[0])
            .args(&runner[1..])
            .arg(exe)
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("fixture execs via runner")
    };
    status
        .code()
        .expect("fixture exited with a code, not a signal")
}

/// A unique scratch path under the OS temp dir for a fixture's flushed sink.
fn scratch_path(tag: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("reovim-arch-fixture-{tag}-{}.log", std::process::id()));
    p
}

// ---------------------------------------------------------------------------
// System-image mode (bare metal)
// ---------------------------------------------------------------------------

/// Locates the rustup-bundled `llvm-objcopy` (the `llvm-tools` component):
/// `<sysroot>/lib/rustlib/<host>/bin/llvm-objcopy`. The toolchain's own
/// objcopy keeps the harness L9-clean — no system binutils dependency.
fn llvm_objcopy() -> PathBuf {
    let out = Command::new("rustc")
        .args(["--print", "sysroot"])
        .output()
        .expect("rustc --print sysroot runs");
    let sysroot = String::from_utf8(out.stdout).expect("sysroot is utf8");
    let out = Command::new("rustc")
        .arg("-vV")
        .output()
        .expect("rustc -vV runs");
    let verbose = String::from_utf8(out.stdout).expect("rustc -vV is utf8");
    let host = verbose
        .lines()
        .find_map(|l| l.strip_prefix("host: "))
        .expect("rustc -vV reports a host triple");
    let mut p = PathBuf::from(sysroot.trim());
    p.extend(["lib", "rustlib", host, "bin", "llvm-objcopy"]);
    p
}

/// Converts the linked ELF into the raw image the aarch64 machine boots (the
/// firmware-style load: raw bytes at the link address, entered at the first
/// byte) and returns the image path, `<elf>.kernel8.img`.
fn objcopy_kernel_image(elf: &Path) -> PathBuf {
    let mut img = elf.as_os_str().to_owned();
    img.push(".kernel8.img");
    let img = PathBuf::from(img);
    let status = Command::new(llvm_objcopy())
        .arg("-O")
        .arg("binary")
        .arg(elf)
        .arg(&img)
        .status()
        .expect("llvm-objcopy runs");
    assert!(status.success(), "objcopy {} -> kernel8.img", elf.display());
    img
}

/// Repackages the 64-bit ELF as a 32-bit ELF for QEMU's Multiboot1 `-kernel`
/// loader, returning the image path, `<elf>.mb32.elf`.
///
/// Multiboot1 enters in 32-bit protected mode and QEMU's loader rejects an
/// `ELFCLASS64` image ("give a 32bit one"), even though the climb to long mode
/// happens inside the kernel. The instruction bytes are unchanged — only the
/// ELF container class is rewritten — so the `.code32` `_start` prologue still
/// loads correctly and climbs to 64-bit itself.
fn objcopy_multiboot_elf32(elf: &Path) -> PathBuf {
    let mut img = elf.as_os_str().to_owned();
    img.push(".mb32.elf");
    let img = PathBuf::from(img);
    let status = Command::new(llvm_objcopy())
        .arg("-O")
        .arg("elf32-i386")
        .arg(elf)
        .arg(&img)
        .status()
        .expect("llvm-objcopy runs");
    assert!(status.success(), "objcopy {} -> elf32", elf.display());
    img
}

/// Runs `cmd` with stdout captured to `serial_path`, enforcing [`BOOT_TIMEOUT`].
///
/// Returns the process exit code, or [`BOOT_TIMEOUT_CODE`] if the boot hung and
/// had to be killed. Stdout is redirected to a file rather than a pipe so a
/// chatty boot log (the full test runner) cannot deadlock against an unread
/// pipe buffer while we poll for the timeout.
fn run_with_timeout(mut cmd: Command, serial_path: &Path) -> i32 {
    let sink = File::create(serial_path).expect("create serial sink");
    let mut child = cmd
        .stdout(Stdio::from(sink))
        .stderr(Stdio::inherit())
        .spawn()
        .expect("system emulator spawns");
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().expect("try_wait on emulator") {
            return status
                .code()
                .expect("emulator exited with a code, not a signal");
        }
        if start.elapsed() >= BOOT_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            return BOOT_TIMEOUT_CODE;
        }
        thread::sleep(Duration::from_millis(25));
    }
}

/// Boots the fixture ELF as a machine image and returns the guest exit code
/// plus the captured serial output, dispatching on the freestanding arch.
///
/// The harness owns the machine flags (no flag soup in the environment), and
/// `ARCH_FIXTURE_RUNNER` supplies the emulator program (`qemu-system-aarch64`
/// or `qemu-system-x86_64`):
///
/// - **aarch64** (`-M raspi4b -semihosting`): the raw `kernel8.img` is loaded
///   at its link address; the floor's semihosting `SYS_EXIT` carries the guest
///   exit code out as the emulator's own, verbatim.
/// - **`x86_64`** (`-M q35 -device isa-debug-exit`): the Multiboot1 ELF32 is
///   loaded; the floor signals exit by writing to the `isa-debug-exit` port,
///   which makes QEMU exit with `(code << 1) | 1`. That transform is inverted
///   here so the guest's logical code (0 success, 70 halt) reaches the
///   assertions unchanged, identical to the aarch64 path.
///
/// Both paths bind `-serial stdio` to the console the floor writes to and run
/// headless under [`run_with_timeout`].
fn run_system_image(exe: &Path) -> (i32, String) {
    let runner = fixture_runner();
    assert!(
        !runner.is_empty(),
        "system-image mode needs ARCH_FIXTURE_RUNNER (a system emulator)",
    );
    let target = fixture_target().expect("system-image mode sets ARCH_FIXTURE_TARGET");

    let mut cmd = Command::new(&runner[0]);
    cmd.args(&runner[1..]);
    let is_x86 = target.starts_with("x86_64");
    if is_x86 {
        let img = objcopy_multiboot_elf32(exe);
        cmd.args([
            "-M",
            "q35",
            "-display",
            "none",
            "-serial",
            "stdio",
            "-device",
            "isa-debug-exit,iobase=0xf4,iosize=0x04",
        ])
        .arg("-kernel")
        .arg(&img);
    } else {
        let img = objcopy_kernel_image(exe);
        // QEMU's `raspi4b` machine generates no device tree (it has no FDT), so
        // `x0` would be zero at `_start` and there would be nothing to
        // enumerate. Supply the real Raspberry Pi 4 firmware DTB via `-dtb` so
        // the boot path captures a genuine device tree. This blob is GPL-2.0
        // third-party data used ONLY as a runtime input — never compiled into
        // any binary; see tests/fixtures/dtb/PROVENANCE.md.
        let dtb = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/dtb/bcm2711-rpi-4-b.dtb");
        cmd.args([
            "-M",
            "raspi4b",
            "-display",
            "none",
            "-serial",
            "stdio",
            "-semihosting",
        ])
        .arg("-dtb")
        .arg(dtb)
        .arg("-kernel")
        .arg(&img);
    }

    let mut serial_path = exe.as_os_str().to_owned();
    serial_path.push(".serial.log");
    let serial_path = PathBuf::from(serial_path);
    let raw = run_with_timeout(cmd, &serial_path);
    let serial = std::fs::read_to_string(&serial_path).unwrap_or_default();
    let _ = std::fs::remove_file(&serial_path);

    // isa-debug-exit reports `(code << 1) | 1`; invert it to the guest's
    // logical code. A timed-out boot keeps its sentinel (it is not an exit the
    // floor produced) so the assertions see an unambiguous failure.
    let code = if is_x86 && raw != BOOT_TIMEOUT_CODE {
        (raw - 1) >> 1
    } else {
        raw
    };
    (code, serial)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn smoke_fixture_boots_allocates_threads_exits_zero() {
    // The charter smoke: boot via `_start`, allocate a DS, spawn + join a
    // thread, exit 0. This is the phase's end-to-end proof.
    if skip_in_system_image_mode("smoke_fixture_boots_allocates_threads_exits_zero") {
        return;
    }
    let exe = build_fixture("arch-fixture-smoke");
    assert_eq!(run(&exe, &[]), 0, "smoke fixture boots and exits 0");
}

#[test]
fn net_smoke_uds_roundtrip_exits_zero() {
    // Integration smoke (#797 Phase 1 AC): bind a UDS listener, spawn an accept
    // thread, connect from the main thread, echo a 16-byte frame-sized buffer
    // round-trip, exit 0 — proving the blocking thread-per-connection carrier
    // primitive composes end-to-end on the real arch floor.
    if skip_in_system_image_mode("net_smoke_uds_roundtrip_exits_zero") {
        return; // UDS is kernel-ABI surface; no freestanding realization.
    }
    let exe = build_fixture("arch-fixture-net-smoke");
    assert_eq!(run(&exe, &[]), 0, "net-smoke round-trip exits 0");
}

#[test]
fn testrt_pilot_all_pass_exits_zero() {
    // The no_std runner charter smoke (pass side): the full arch test suite
    // runs on the in-repo runner and exits 0 when every test passes.
    //
    // Serialized against `testrt_pilot_one_failure_exits_nonzero` via
    // `selftest_lock()` — see the guard's comment for the shared-artifact
    // hazard. The guard spans build AND exec.
    let _guard = selftest_lock();
    let exe = build_fixture("arch-selftest");
    if target_is_none() {
        let (code, serial) = run_system_image(&exe);
        assert_eq!(code, 0, "all-pass bare-metal image exits 0; serial: {serial:?}");
        return;
    }
    assert_eq!(run(&exe, &[]), 0, "all-pass test run exits 0");
}

#[test]
fn uapi_selftest_all_pass_exits_zero() {
    // uapi selftest charter smoke (pass side, #786 Phase 5): the full uapi
    // test suite — layout goldens, codec round-trips + 57-byte Hello frame
    // golden, CF5 cross-check, macro vtable smoke — all run on the no_std
    // runner and exit 0 when every test passes.
    if skip_in_system_image_mode("uapi_selftest_all_pass_exits_zero") {
        return;
    }
    // Serialized against the inject-failure variant via `uapi_selftest_lock()`
    // — the guard spans build AND exec (see the guards' comment).
    let _guard = uapi_selftest_lock();
    let exe = build_fixture("uapi-selftest");
    assert_eq!(run(&exe, &[]), 0, "uapi all-pass test run exits 0");
}

#[test]
fn uapi_selftest_one_failure_exits_nonzero() {
    // uapi selftest charter smoke (fail side): the inject-failure variant
    // adds a deliberately-failing test; the arch panic handler exits the
    // halt disposition code (70) — the runner's fail-fast contract.
    if skip_in_system_image_mode("uapi_selftest_one_failure_exits_nonzero") {
        return;
    }
    // Serialized against the all-pass variant via `uapi_selftest_lock()` —
    // the guard spans build AND exec (see the guards' comment).
    let _guard = uapi_selftest_lock();
    let exe = build_fixture_features("uapi-selftest", &["inject-failure"]);
    assert_eq!(run(&exe, &[]), 70, "uapi inject-failure exits the halt code");
}

#[test]
fn testrt_pilot_one_failure_exits_nonzero() {
    // The no_std runner charter smoke (fail side): a deliberately-failing test
    // (the `inject-failure` variant) panics, so the arch panic handler exits
    // the halt disposition code (70) — the runner's fail-fast contract.
    //
    // Serialized against `testrt_pilot_all_pass_exits_zero` via
    // `selftest_lock()` — see the guard's comment for the shared-artifact
    // hazard. The guard spans build AND exec.
    let _guard = selftest_lock();
    let exe = build_fixture_features("arch-selftest", &["inject-failure"]);
    if target_is_none() {
        // Bare metal has no file sink, so the LOG2 panic line arrives on the
        // serial console instead — the disposition smoke for this mode.
        let (code, serial) = run_system_image(&exe);
        assert_eq!(code, 70, "failing bare-metal image exits the halt code; serial: {serial:?}");
        assert_log2_kernel_panic(&serial, "deliberately_fails");
        return;
    }
    assert_eq!(run(&exe, &[]), 70, "a failing test exits the halt code");
}

#[test]
fn bootcore_boots_real_kernel_exits_zero() {
    // The first x86 root-daemon proof: the bootcore payload enters the system
    // kernel shell path on the freestanding floor and exits 0 through
    // isa-debug-exit.
    //
    // x86-only: the aarch64 bootcore parks (`wfe`) to persist its framebuffer
    // for a manual screendump, so it never exits — running it here would time
    // out. aarch64's kernel-boot proof stays the manual screendump.
    let Some(target) = fixture_target() else {
        return; // hosted mode: no bare-metal kernel-boot image
    };
    if !target.starts_with("x86_64") {
        return;
    }
    let exe = build_fixture("arch-bootcore");
    let (code, serial) = run_system_image(&exe);
    assert_eq!(code, 0, "bootcore boots the system kernel and exits 0; serial: {serial:?}");
    assert!(
        serial.contains("system kernel shell ready"),
        "bootcore reached root-daemon boot path; serial: {serial:?}",
    );
    assert!(
        serial.contains("reovim-os> "),
        "bootcore printed shell prompt for shell-only profile; serial: {serial:?}",
    );
}

#[test]
fn os_shell_profile_transcript_on_x86_target_boots_and_prints_cli() {
    // The first proof from the official distribution root (`apps/os`): the
    // kernel-only shell handles scripted input and prints a deterministic root
    // shell transcript before exiting.
    let Some(target) = fixture_target() else {
        return;
    };
    if !target.starts_with("x86_64") {
        return;
    }

    let _guard = os_image_lock();
    let bootline = concat!(
        "help clear\n",
        "help screentest\n",
        "help input\n",
        "help status\n",
        "help proof\n",
        "help probe\n",
        "help dmesg\n",
        "help dump\n",
        "help sched\n",
        "help proc\n",
        "probe xhci-start\n",
        "probe xhci-enable-slot\n",
        "probe xhci-address-device\n",
        "probe xhci-get-device-descriptor\n",
        "probe xhci-set-address\n",
        "probe xhci-read-device-descriptor\n",
        "probe xhci-read-config-descriptor-header\n",
        "probe xhci-read-config-descriptor\n",
        "probe xhci-set-configuration\n",
        "probe xhci-configure-endpoint\n",
        "probe xhci-set-hid-protocol\n",
        "probe xhci-read-keyboard-report\n",
        "proof\n",
        "cat /boot/proof\n",
        "help\n",
        "help clear\n",
        "help screentest\n",
        "help input\n",
        "help proof\n",
        "help pwd\n",
        "help ls\n",
        "help cd\n",
        "help cat\n",
        "help read\n",
        "help mount\n",
        "help device\n",
        "help dmesg\n",
        "help dump\n",
        "help sched\n",
        "help proc\n",
        "help status\n",
        "help probe\n",
        "help launch\n",
        "help reovim\n",
        "help halt\n",
        "cat /boot/help\n",
        "clear\n",
        "screentest\n",
        "pwd\n",
        "ls /\n",
        "ls /bin\n",
        "cat /bin/help\n",
        "cat /bin/clear\n",
        "cat /bin/screentest\n",
        "cat /bin/pwd\n",
        "cat /bin/ls\n",
        "cat /bin/cd\n",
        "cat /bin/cat\n",
        "cat /bin/read\n",
        "cat /bin/mount\n",
        "cat /bin/device\n",
        "cat /bin/input\n",
        "cat /bin/status\n",
        "cat /bin/proof\n",
        "cat /bin/dmesg\n",
        "cat /bin/dump\n",
        "cat /bin/sched\n",
        "cat /bin/proc\n",
        "cat /bin/probe\n",
        "cat /bin/launch\n",
        "cat /bin/reovim\n",
        "cat /bin/halt\n",
        "ls /proc\n",
        "cat /proc/execs\n",
        "cat /proc/media\n",
        "cat /proc/pending\n",
        "cat /proc/self\n",
        "cat /proc/processes\n",
        "cat /proc/tasks\n",
        "cat /proc/scheduler\n",
        "cat /proc/sources\n",
        "sched tick\n",
        "sched yield\n",
        "proc\n",
        "proc self\n",
        "proc media\n",
        "proc sources\n",
        "proc exec pwd\n",
        "proc exec cat /boot/profile\n",
        "proc spawn pwd\n",
        "pwd\n",
        "read\n",
        "scripted tty line\n",
        "cat /proc/syscalls\n",
        "cat /proc/waits\n",
        "ls /dump\n",
        "cat /dump/status\n",
        "cat /dump/snapshot\n",
        "ls /boot\n",
        "ls /dev\n",
        "ls /log\n",
        "mount\n",
        "cat /boot/mounts\n",
        "device\n",
        "cat /boot/memory\n",
        "cat /boot/devices\n",
        "cd /dev\n",
        "pwd\n",
        "ls\n",
        "cat uart0\n",
        "cd /\n",
        "cat /boot/image\n",
        "status\n",
        "cat /boot/status\n",
        "input\n",
        "cat /boot/input\n",
        "pwd | input\n",
        "pwd | cat\n",
        "probe help\n",
        "cat /boot/probes\n",
        "probe pcie\n",
        "probe usb-keyboard\n",
        "cat /boot/profile\n",
        "launch\n",
        "reovim\n",
        "dmesg --stats\n",
        "dump status\n",
        "dump snapshot\n",
        "dump sync\n",
        "cat /log/stats\n",
        "cat /log/events\n",
        "proc\n",
        "dmesg\n",
        "cat /log/dmesg\n",
        "halt\n",
    );
    let exe = build_os_image(&[], Some(bootline), None);
    let (code, serial) = run_system_image(&exe);
    assert_eq!(code, 0, "reovim-os shell profile exits 0; serial: {serial:?}");
    assert!(
        serial.contains("system kernel shell ready"),
        "shell-only os profile reached root-daemon shell path; serial: {serial:?}",
    );
    assert!(
        serial.contains("reovim-os> reovim root shell"),
        "shell prompt + help output appears; serial: {serial:?}",
    );
    assert!(
        serial.contains("reovim root shell"),
        "help output appears in transcript; serial: {serial:?}",
    );
    assert!(
        serial.matches("reovim root shell").count() >= 2,
        "help catalog appears through command and VFS; serial: {serial:?}",
    );
    assert!(
        serial.contains(
            "/bin programs: help, clear, screentest, pwd, ls, cd, cat, read, mount, input, status, proof, device, dmesg, dump, sched, proc, probe, launch, reovim, halt"
        ),
        "help program vocabulary appears; serial: {serial:?}",
    );
    assert!(
        serial.contains("namespace: /bin") && serial.contains("usage: help [program]"),
        "help usage appears; serial: {serial:?}",
    );
    assert!(
        serial.matches("usage: help [program]").count() >= 2,
        "VFS help pseudo-file prints help usage; serial: {serial:?}",
    );
    assert!(
        serial.contains("details:\n  help [program] - show program help")
            && serial
                .contains("  sched [status|tick|yield] - inspect scheduler state, tick, or yield current task")
            && serial.contains(
                "  proc [processes|execs|media|pending|self|sources|tasks|waits|syscalls|scheduler|exec PROGRAM [ARG...]|spawn PROGRAM [ARG...]|block PROGRAM [ARG...]|wait PID|wake PID|kill PID|install-bin NAME ok|error|install-payload NAME ready|failed|install-bin-media NAME|install-payload-media NAME] - inspect process state"
            )
            && serial.contains("  cat [path...] - print stdin or kernel VFS pseudo files")
            && serial.contains("  read - read one TTY line")
            && serial.contains("  halt - request root daemon shutdown"),
        "VFS help pseudo-file prints detailed program catalog; serial: {serial:?}",
    );
    assert!(
        serial.contains("clear - clear framebuffer console and terminal"),
        "clear help appears; serial: {serial:?}",
    );
    assert!(
        serial.contains("screentest - print renderer diagnostics"),
        "screentest help appears; serial: {serial:?}",
    );
    assert!(
        serial.contains("required rows: el: clean, el1: clean-left, el2: clean-all"),
        "screentest help names required erase rows; serial: {serial:?}",
    );
    assert!(
        serial.contains("input - print live console input diagnostics"),
        "input help appears; serial: {serial:?}",
    );
    assert!(
        serial.contains("read - read one TTY line"),
        "read help appears; serial: {serial:?}",
    );
    assert!(
        serial.contains("status - print boot, input, and manual_next summary"),
        "status help appears; serial: {serial:?}",
    );
    assert!(
        serial.contains("proof - print physical input proof checklist"),
        "proof help appears; serial: {serial:?}",
    );
    assert!(
        serial.contains(
            "probe target - run lower hardware probe; try `probe help` or `cat /boot/probes`"
        ),
        "probe help appears; serial: {serial:?}",
    );
    assert!(
        serial.contains("dmesg [--stats] - print retained kernel log or ring stats"),
        "dmesg help appears; serial: {serial:?}",
    );
    assert!(
        serial.contains("dump [status|snapshot|sync] - inspect or flush kernel dump state"),
        "dump help appears; serial: {serial:?}",
    );
    assert!(
        serial.contains(
            "sched [status|tick|yield] - inspect scheduler state, tick, or yield current task"
        ),
        "sched help appears; serial: {serial:?}",
    );
    assert!(
        serial.contains(
            "proc [processes|execs|media|pending|self|sources|tasks|waits|syscalls|scheduler|exec PROGRAM [ARG...]|spawn PROGRAM [ARG...]|block PROGRAM [ARG...]|wait PID|wake PID|kill PID|install-bin NAME ok|error|install-payload NAME ready|failed|install-bin-media NAME|install-payload-media NAME] - inspect process state"
        ),
        "proc help appears; serial: {serial:?}",
    );
    assert!(
        serial.contains("device - print boot memory and device inventory"),
        "device help appears; serial: {serial:?}",
    );
    assert!(
        serial.contains("launch [payload] - list or run registered payloads"),
        "launch help appears; serial: {serial:?}",
    );
    assert!(
        serial.contains("reovim - run the default reovim payload alias"),
        "reovim help appears; serial: {serial:?}",
    );
    assert!(
        serial.contains("halt - request root daemon shutdown"),
        "halt help appears; serial: {serial:?}",
    );
    assert!(
        serial.contains("usb_keyboard_pending_bytes=0"),
        "input diagnostics appear in transcript; serial: {serial:?}",
    );
    assert!(
        serial.contains("usb_keyboard_probe=disabled"),
        "input probe state appears in transcript; serial: {serial:?}",
    );
    assert!(
        serial.contains("program_stdin_read_bytes=2"),
        "pipeline routes producer stdout into /bin/input stdin; serial: {serial:?}",
    );
    assert!(
        serial.contains("package=reovim-os"),
        "boot image diagnostics appear in transcript; serial: {serial:?}",
    );
    assert!(
        serial.contains("target=x86_64-unknown-none"),
        "boot image target appears in transcript; serial: {serial:?}",
    );
    assert!(
        serial.contains("bootline=present"),
        "boot image bootline state appears in transcript; serial: {serial:?}",
    );
    assert!(
        serial.contains("manual_next=probe-help"),
        "boot status gives manual next step for unsupported USB target; serial: {serial:?}",
    );
    assert!(
        serial.contains("source_state=unavailable"),
        "boot status includes selected source readiness; serial: {serial:?}",
    );
    assert!(
        serial.contains("usb_keyboard_poll_interval_ms=0"),
        "boot status includes USB poll interval; serial: {serial:?}",
    );
    assert!(
        serial.contains("usb_keyboard_last_poll=not-polled"),
        "boot status includes USB last-poll diagnostics; serial: {serial:?}",
    );
    assert!(
        serial.contains("probe targets:"),
        "probe target help appears in transcript; serial: {serial:?}",
    );
    let proof_commands = "proof:\n/bin programs:\n  proof\n  cat /boot/proof\n  help\n  help clear\n  help screentest\n  help input\n  help proof\n  help pwd\n  help ls\n  help cd\n  help cat\n  help read\n  help mount\n  help device\n  help dmesg\n  help dump\n  help sched\n  help proc\n  help status\n  help probe\n  help launch\n  help reovim\n  help halt\n  cat /boot/help";
    assert!(
        serial.contains(proof_commands),
        "proof checklist appears in transcript; serial: {serial:?}",
    );
    assert!(
        serial.contains(
            "  help\n  help clear\n  help screentest\n  help input\n  help proof\n  help pwd\n  help ls\n  help cd\n  help cat\n  help read\n  help mount\n  help device\n  help dmesg\n  help dump\n  help sched\n  help proc\n  help status\n  help probe\n  help launch\n  help reovim\n  help halt\n  cat /boot/help\n  clear"
        ),
        "proof checklist includes targeted help and VFS help; serial: {serial:?}",
    );
    assert!(
        serial.contains("  pwd\n  ls /\n  ls /boot"),
        "proof checklist includes root namespace commands; serial: {serial:?}",
    );
    assert!(
        serial.contains("  ls /dev\n  ls /log\n  mount\n  cat /boot/mounts"),
        "proof checklist includes VFS namespace and log listing; serial: {serial:?}",
    );
    assert!(
        serial.contains("  device\n  cat /boot/memory\n  cat /boot/devices"),
        "proof checklist includes boot inventory commands; serial: {serial:?}",
    );
    assert!(
        serial.contains("  cd /dev\n  pwd\n  ls\n  cat uart0\n  cd /"),
        "proof checklist includes relative device access commands; serial: {serial:?}",
    );
    assert!(
        serial.contains("expected:\n  package=reovim-os"),
        "proof checklist image facts appear; serial: {serial:?}",
    );
    assert!(
        serial.contains("  bootline=absent"),
        "proof checklist bootline fact appears; serial: {serial:?}",
    );
    assert!(
        serial.contains("  profile=shell-only\n  launch=disabled"),
        "proof checklist profile facts appear; serial: {serial:?}",
    );
    assert!(
        serial.contains("  input_mode=live"),
        "proof checklist profile input mode appears; serial: {serial:?}",
    );
    assert!(
        serial.contains("  usb_keyboard_last_poll=report-ready"),
        "proof checklist names the lower report-ready fact; serial: {serial:?}",
    );
    assert!(
        serial.contains(
            "  dmesg contains input.usb_keyboard=ready source=usb-keyboard+uart-fallback last_poll=report-ready",
        ),
        "proof checklist names the retained USB readiness log; serial: {serial:?}",
    );
    assert!(
        serial.contains("  manual_next=type-shell-command"),
        "proof checklist names successful manual next step; serial: {serial:?}",
    );
    assert!(
        serial.contains(
            "  help clear\n  help screentest\n  help input\n  help proof\n  help pwd\n  help ls\n  help cd\n  help cat\n  help read\n  help mount\n  help device\n  help dmesg\n  help dump\n  help sched\n  help proc\n  help status\n  help probe\n  help launch\n  help reovim\n  help halt\n  cat /boot/help"
        ),
        "proof checklist includes targeted help commands; serial: {serial:?}",
    );
    assert!(
        serial.contains("probe targets include pcie"),
        "proof checklist names read-only PCIe probe; serial: {serial:?}",
    );
    assert!(
        serial.contains("probe targets include xhci-read-keyboard-report"),
        "proof checklist names keyboard report probe; serial: {serial:?}",
    );
    assert!(
        serial.contains("detailed help catalog available through /boot/help"),
        "proof checklist names detailed VFS help catalog expectation; serial: {serial:?}",
    );
    assert!(
        serial.contains("screentest includes erase-line mode diagnostics"),
        "proof checklist names screentest erase-mode expectation; serial: {serial:?}",
    );
    assert!(
        serial.contains("kernel log stats available through /log/stats"),
        "proof checklist names log stats expectation; serial: {serial:?}",
    );
    assert!(
        serial.contains("kernel structured events available through /log/events"),
        "proof checklist names log events expectation; serial: {serial:?}",
    );
    assert!(
        serial.contains("dump status available through /bin/dump"),
        "proof checklist names dump status expectation; serial: {serial:?}",
    );
    assert!(
        serial.contains("dump sync fails closed until persistent storage is available"),
        "proof checklist names dump sync fail-closed expectation; serial: {serial:?}",
    );
    assert!(
        serial.contains("kernel log dropped_bytes=0"),
        "proof checklist names zero dropped log bytes expectation; serial: {serial:?}",
    );
    assert!(
        serial.contains("retained dmesg has no [klog] dropped_bytes marker"),
        "proof checklist names retained dmesg no-wrap marker expectation; serial: {serial:?}",
    );
    assert!(
        serial.contains("probe catalog available through /boot/probes"),
        "proof checklist names VFS probe catalog expectation; serial: {serial:?}",
    );
    assert!(
        serial.contains("  probe help\n  cat /boot/probes\n  probe pcie\n  probe usb-keyboard"),
        "proof checklist includes VFS probe catalog and read-only PCIe probe; serial: {serial:?}",
    );
    assert!(
        serial.contains("  launch\n  reovim"),
        "proof checklist includes shell-only payload checks; serial: {serial:?}",
    );
    assert!(
        serial.contains(
            "  dmesg --stats\n  dump status\n  dump snapshot\n  dump sync\n  cat /log/stats\n  cat /log/events\n  proc\n  dmesg\n  cat /log/dmesg"
        ),
        "proof checklist includes dump, log stats, process, and dmesg commands; serial: {serial:?}",
    );
    assert!(
        serial.contains("terminal:\n  halt"),
        "proof checklist names terminal halt step; serial: {serial:?}",
    );
    assert!(
        serial.contains("launch/reovim disabled in shell-only profile"),
        "proof checklist names shell-only payload-disabled expectation; serial: {serial:?}",
    );
    assert!(
        serial.contains("halt typed last prints halt: ok and stops root daemon"),
        "proof checklist names terminal halt expectation; serial: {serial:?}",
    );
    assert!(
        serial.contains("xhci-read-keyboard-report (alias: usb-keyboard-read-report)"),
        "probe target help includes keyboard report checkpoint; serial: {serial:?}",
    );
    assert!(
        serial.contains("\x1b[2J\x1b[H"),
        "clear command emits terminal clear/home escapes; serial: {serial:?}",
    );
    assert!(
        serial.contains("screen test:\n"),
        "screentest command runs in booted OS transcript; serial: {serial:?}",
    );
    assert!(
        serial.contains("target: framebuffer/serial tty renderer subset"),
        "screentest prints renderer target; serial: {serial:?}",
    );
    assert!(
        serial.contains("idx-fg:"),
        "screentest prints indexed color row; serial: {serial:?}",
    );
    assert!(
        serial.contains("rgb-bg:"),
        "screentest prints truecolor background row; serial: {serial:?}",
    );
    assert!(serial.contains("attrs:"), "screentest prints attribute row; serial: {serial:?}",);
    assert!(
        serial.contains("el: clean"),
        "screentest prints erase-to-end row; serial: {serial:?}",
    );
    assert!(
        serial.contains("el1: clean-left"),
        "screentest prints erase-to-start row; serial: {serial:?}",
    );
    assert!(
        serial.contains("el2: clean-all"),
        "screentest prints erase-all row; serial: {serial:?}",
    );
    assert!(
        serial.contains("kernel on / type rootfs (ro,pseudo)"),
        "mount table appears in transcript; serial: {serial:?}",
    );
    assert!(
        serial.contains("klog on /log type logfs (ro,pseudo)"),
        "mount table includes klog namespace; serial: {serial:?}",
    );
    assert!(
        serial.contains("reovim-os> dmesg\nevents\nstats"),
        "ls /log prints log namespace files; serial: {serial:?}",
    );
    assert!(
        serial.contains("capacity_bytes=65536"),
        "log stats report klog ring capacity; serial: {serial:?}",
    );
    assert!(
        serial.contains("boot_id=1")
            && serial.contains("session_id=1")
            && serial.contains("identity_source=rootd-volatile"),
        "log stats report boot/session identity; serial: {serial:?}",
    );
    assert!(
        serial.contains("retained_events="),
        "log stats report structured event count; serial: {serial:?}",
    );
    assert!(
        serial.contains("reovim-os> events:\n- seq=")
            && serial.contains("boot=1")
            && serial.contains("session=1")
            && serial.contains("source=rootd")
            && serial.contains("component=boot")
            && serial.contains("component=proc")
            && serial.contains("kind=program-exit")
            && serial.contains("pid=")
            && serial.contains("task="),
        "cat /log/events prints structured kernel event rows; serial: {serial:?}",
    );
    assert!(
        serial.contains("retained_bytes="),
        "log stats report retained klog bytes; serial: {serial:?}",
    );
    assert!(
        serial.contains("dropped_bytes=0"),
        "log stats report dropped klog bytes; serial: {serial:?}",
    );
    assert!(
        serial.contains("probe pcie"),
        "probe command is included in scripted input; serial: {serial:?}",
    );
    assert!(
        serial.contains("state=unsupported-on-this-target"),
        "x86 OS profile reports pcie probe unsupported; serial: {serial:?}",
    );
    assert!(
        serial.contains("probe usb-keyboard:\nstate=unsupported-on-this-target"),
        "x86 OS profile reports usb-keyboard probe unsupported; serial: {serial:?}",
    );
    assert!(
        serial.contains("probe xhci-start:\nstate=unsupported-on-this-target"),
        "x86 OS profile reports xhci-start probe unsupported; serial: {serial:?}",
    );
    assert!(
        serial.contains("probe xhci-enable-slot:\nstate=unsupported-on-this-target"),
        "x86 OS profile reports xhci-enable-slot probe unsupported; serial: {serial:?}",
    );
    assert!(
        serial.contains("probe xhci-address-device:\nstate=unsupported-on-this-target"),
        "x86 OS profile reports xhci-address-device probe unsupported; serial: {serial:?}",
    );
    assert!(
        serial.contains("probe xhci-get-device-descriptor:\nstate=unsupported-on-this-target"),
        "x86 OS profile reports xhci-get-device-descriptor probe unsupported; serial: {serial:?}",
    );
    assert!(
        serial.contains("probe xhci-set-address:\nstate=unsupported-on-this-target"),
        "x86 OS profile reports xhci-set-address probe unsupported; serial: {serial:?}",
    );
    assert!(
        serial.contains("probe xhci-read-device-descriptor:\nstate=unsupported-on-this-target"),
        "x86 OS profile reports xhci-read-device-descriptor probe unsupported; serial: {serial:?}",
    );
    assert!(
        serial.contains(
            "probe xhci-read-config-descriptor-header:\nstate=unsupported-on-this-target"
        ),
        "x86 OS profile reports xhci-read-config-descriptor-header probe unsupported; serial: {serial:?}",
    );
    assert!(
        serial.contains("probe xhci-read-config-descriptor:\nstate=unsupported-on-this-target"),
        "x86 OS profile reports xhci-read-config-descriptor probe unsupported; serial: {serial:?}",
    );
    assert!(
        serial.contains("probe xhci-set-configuration:\nstate=unsupported-on-this-target"),
        "x86 OS profile reports xhci-set-configuration probe unsupported; serial: {serial:?}",
    );
    assert!(
        serial.contains("probe xhci-configure-endpoint:\nstate=unsupported-on-this-target"),
        "x86 OS profile reports xhci-configure-endpoint probe unsupported; serial: {serial:?}",
    );
    assert!(
        serial.contains("probe xhci-set-hid-protocol:\nstate=unsupported-on-this-target"),
        "x86 OS profile reports xhci-set-hid-protocol probe unsupported; serial: {serial:?}",
    );
    assert!(
        serial.contains("probe xhci-read-keyboard-report:\nstate=unsupported-on-this-target"),
        "x86 OS profile reports xhci-read-keyboard-report probe unsupported; serial: {serial:?}",
    );
    assert!(
        serial.contains("reovim-os> launch disabled for this profile"),
        "shell-only OS profile refuses generic payload launch; serial: {serial:?}",
    );
    assert!(
        serial.contains("reovim-os> reovim disabled for this profile"),
        "shell-only OS profile refuses reovim alias launch; serial: {serial:?}",
    );
    assert!(serial.contains("reovim-os> /\n"), "pwd prints root cwd; serial: {serial:?}",);
    assert!(
        serial.contains("reovim-os> bin\nboot\ndev\ndump\nlog\nproc"),
        "ls / prints root namespace; serial: {serial:?}",
    );
    assert!(
        serial.contains("reovim-os> help\nclear\nscreentest")
            && serial.contains("probe\nlaunch\nreovim\nhalt\n"),
        "ls /bin prints image program namespace; serial: {serial:?}",
    );
    assert!(
        serial.contains(
            "program=help\npath=/bin/help\nsummary=show /bin program help\ntype=bin\nloader=source-image\nentry_fn=bin_help"
        ),
        "cat /bin/help prints program descriptor; serial: {serial:?}",
    );
    assert!(
        serial.contains(
            "program=clear\npath=/bin/clear\nsummary=clear framebuffer console and terminal\ntype=bin\nloader=source-image\nentry_fn=bin_clear"
        ),
        "cat /bin/clear prints source-image program descriptor; serial: {serial:?}",
    );
    assert!(
        serial.contains(
            "program=screentest\npath=/bin/screentest\nsummary=print renderer diagnostics\ntype=bin\nloader=source-image\nentry_fn=bin_screentest"
        ),
        "cat /bin/screentest prints source-image program descriptor; serial: {serial:?}",
    );
    assert!(
        serial.contains(
            "program=pwd\npath=/bin/pwd\nsummary=print current kernel VFS directory\ntype=bin\nloader=source-image\nentry_fn=bin_pwd"
        ),
        "cat /bin/pwd prints source-image program descriptor; serial: {serial:?}",
    );
    assert!(
        serial.contains(
            "program=ls\npath=/bin/ls\nsummary=list a kernel VFS directory\ntype=bin\nloader=source-image\nentry_fn=bin_ls"
        ),
        "cat /bin/ls prints source-image program descriptor; serial: {serial:?}",
    );
    assert!(
        serial.contains(
            "program=cd\npath=/bin/cd\nsummary=change current kernel VFS directory\ntype=bin\nloader=source-image\nentry_fn=bin_cd"
        ),
        "cat /bin/cd prints source-image program descriptor; serial: {serial:?}",
    );
    assert!(
        serial.contains(
            "program=cat\npath=/bin/cat\nsummary=print stdin or kernel VFS pseudo files\ntype=bin\nloader=source-image\nentry_fn=bin_cat"
        ),
        "cat /bin/cat prints source-image program descriptor; serial: {serial:?}",
    );
    assert!(
        serial.contains(
            "program=read\npath=/bin/read\nsummary=read one TTY line\ntype=bin\nloader=source-image\nentry_fn=bin_read"
        ),
        "cat /bin/read prints source-image program descriptor; serial: {serial:?}",
    );
    assert!(
        serial.contains("reovim-os> scripted tty line\n"),
        "/bin/read consumes the next scripted TTY line; serial: {serial:?}",
    );
    assert!(
        serial.contains(
            "program=mount\npath=/bin/mount\nsummary=print kernel VFS mount table\ntype=bin\nloader=source-image\nentry_fn=bin_mount"
        ),
        "cat /bin/mount prints source-image program descriptor; serial: {serial:?}",
    );
    assert!(
        serial.contains(
            "program=device\npath=/bin/device\nsummary=print boot memory and device inventory\ntype=bin\nloader=source-image\nentry_fn=bin_device"
        ),
        "cat /bin/device prints source-image program descriptor; serial: {serial:?}",
    );
    assert!(
        serial.contains(
            "program=input\npath=/bin/input\nsummary=print live console input diagnostics\ntype=bin\nloader=source-image\nentry_fn=bin_input"
        ),
        "cat /bin/input prints source-image program descriptor; serial: {serial:?}",
    );
    assert!(
        serial.contains(
            "program=status\npath=/bin/status\nsummary=print boot, input, and manual_next summary\ntype=bin\nloader=source-image\nentry_fn=bin_status"
        ),
        "cat /bin/status prints source-image program descriptor; serial: {serial:?}",
    );
    assert!(
        serial.contains(
            "program=proof\npath=/bin/proof\nsummary=print physical input proof checklist\ntype=bin\nloader=source-image\nentry_fn=bin_proof"
        ),
        "cat /bin/proof prints source-image program descriptor; serial: {serial:?}",
    );
    assert!(
        serial.contains(
            "program=dmesg\npath=/bin/dmesg\nsummary=print retained kernel log or ring stats\ntype=bin\nloader=source-image\nentry_fn=bin_dmesg"
        ),
        "cat /bin/dmesg prints source-image program descriptor; serial: {serial:?}",
    );
    assert!(
        serial.contains(
            "program=dump\npath=/bin/dump\nsummary=inspect or flush kernel dump state\ntype=bin\nloader=source-image\nentry_fn=bin_dump"
        ),
        "cat /bin/dump prints source-image program descriptor; serial: {serial:?}",
    );
    assert!(
        serial.contains(
            "program=sched\npath=/bin/sched\nsummary=inspect scheduler state, tick, or yield\ntype=bin\nloader=source-image\nentry_fn=bin_sched"
        ),
        "cat /bin/sched prints source-image program descriptor; serial: {serial:?}",
    );
    assert!(
        serial.contains(
            "program=proc\npath=/bin/proc\nsummary=inspect process state\ntype=bin\nloader=source-image\nentry_fn=bin_proc"
        ),
        "cat /bin/proc prints source-image program descriptor; serial: {serial:?}",
    );
    assert!(
        serial.contains(
            "program=probe\npath=/bin/probe\nsummary=run a lower hardware probe\ntype=bin\nloader=source-image\nentry_fn=bin_probe"
        ),
        "cat /bin/probe prints source-image program descriptor; serial: {serial:?}",
    );
    assert!(
        serial.contains(
            "program=launch\npath=/bin/launch\nsummary=list or run registered payloads\ntype=bin\nloader=source-image\nentry_fn=bin_launch"
        ),
        "cat /bin/launch prints source-image program descriptor; serial: {serial:?}",
    );
    assert!(
        serial.contains(
            "program=reovim\npath=/bin/reovim\nsummary=run the default reovim payload alias\ntype=bin\nloader=source-image\nentry_fn=bin_reovim"
        ),
        "cat /bin/reovim prints source-image program descriptor; serial: {serial:?}",
    );
    assert!(
        serial.contains(
            "program=halt\npath=/bin/halt\nsummary=request root daemon shutdown\ntype=bin\nloader=source-image\nentry_fn=bin_halt"
        ),
        "cat /bin/halt prints source-image program descriptor; serial: {serial:?}",
    );
    assert!(
        serial.contains(
            "reovim-os> execs\nmedia\npending\nprocesses\nself\nscheduler\nsources\nsyscalls\ntasks\nwaits\n"
        ),
        "ls /proc prints process namespace; serial: {serial:?}",
    );
    assert!(
        serial.contains("reovim-os> execs:\n")
            && serial.contains("argv0=cat status=ok reason=loaded path=/bin/cat")
            && serial.contains("path=/bin/cat source=/bin/cat")
            && serial.contains("loader=source-image entry_fn=bin_cat")
            && serial.contains("truncated=false kind=bin"),
        "cat /proc/execs prints executable admission rows; serial: {serial:?}",
    );
    assert!(
        serial.contains("reovim-os> pending:\n"),
        "cat /proc/pending prints pending executable table; serial: {serial:?}",
    );
    assert!(
        serial.contains("source-media:\n")
            && serial.contains("storage=none\nstorage_capacity_bytes=0\nroot_bytes=0")
            && serial.contains("status=unavailable\nreason=source-media-unavailable"),
        "source-media table reports unavailable media in default x86 boot; serial: {serial:?}",
    );
    assert!(
        serial.contains("sources:\n")
            && serial.contains("namespace=bin path=/bin/help loader=source-image bytes=")
            && serial.contains("namespace=bin path=/bin/proc loader=source-image bytes="),
        "source-store table exposes /bin source artifacts; serial: {serial:?}",
    );
    assert!(
        serial.contains("reovim-os> sources:\n")
            && serial.contains("namespace=bin path=/bin/cat loader=source-image bytes="),
        "/bin/proc sources prints executable source artifacts; serial: {serial:?}",
    );
    assert!(
        serial.contains("reovim-os> self:\n")
            && serial.contains("state=running path=/bin/cat")
            && serial.contains("loader=source-image entry_fn=bin_cat"),
        "cat /proc/self prints current /bin/cat process; serial: {serial:?}",
    );
    assert!(
        serial.contains("processes:\n- pid=1 ppid=0 task=1 state=running path=rootd")
            && serial.contains("path=rootd exit=0 loader=kernel entry_fn=rootd_main")
            && serial.contains("state=running path=/bin/cat")
            && serial.contains("loader=source-image entry_fn=bin_cat"),
        "cat /proc/processes prints root and current program process; serial: {serial:?}",
    );
    assert!(
        serial.contains("tasks:\n- task=1 pid=1 parent_task=0 state=running entry=rootd")
            && serial.contains("state=running entry=/bin/cat"),
        "cat /proc/tasks prints root and current program task; serial: {serial:?}",
    );
    assert!(
        serial.contains("scheduler:\ncurrent_task=")
            && serial.contains("ready_queue_len=0")
            && serial.contains("dispatch_count=")
            && serial.contains("yield_count=")
            && serial.contains("tick_count="),
        "cat /proc/scheduler prints scheduler state; serial: {serial:?}",
    );
    assert!(
        serial.contains("reovim-os> sched tick:\nticked=true\nstatus=ok\ntask=")
            && serial.contains("task_ticks=1")
            && serial.contains("scheduler:\n")
            && serial.contains("tick_count=1"),
        "/bin/sched records an explicit scheduler tick; serial: {serial:?}",
    );
    assert!(
        serial.contains("reovim-os> sched yield:\nyielded=false\nstatus=no-peer\nselected_pid=")
            && serial.contains("selected_task=")
            && serial.contains("scheduler:\n")
            && serial.contains("yield_count=1"),
        "/bin/sched reports no peer task when yielding from the shell transcript; serial: {serial:?}",
    );
    assert!(
        serial.contains("reovim-os> processes:\n")
            && serial.contains("state=running path=/bin/proc")
            && serial.contains("loader=source-image entry_fn=bin_proc"),
        "/bin/proc prints process state through typed syscalls; serial: {serial:?}",
    );
    assert!(
        serial.contains("reovim-os> self:\n")
            && serial.contains("state=running path=/bin/proc")
            && serial.contains("loader=source-image entry_fn=bin_proc"),
        "/bin/proc self prints current process through typed syscall; serial: {serial:?}",
    );
    assert!(
        serial.contains("shell: proc exec pwd\n")
            && serial.contains("wait.start parent_pid=")
            && serial.contains("exec.path=/bin/pwd")
            && serial.contains("loader=source-image entry_fn=bin_pwd"),
        "/bin/proc exec spawns and waits for /bin/pwd child; serial: {serial:?}",
    );
    assert!(
        serial.contains("shell: proc exec cat /boot/profile\n")
            && serial.contains("exec.path=/bin/cat")
            && serial.contains("profile=shell-only")
            && serial.contains("launch=disabled"),
        "/bin/proc exec forwards argv to /bin/cat child; serial: {serial:?}",
    );
    assert!(
        serial.contains("shell: proc spawn pwd\n")
            && serial.contains("proc spawn:\n")
            && serial.contains("state=ready\n")
            && serial.contains("shell: pwd\n"),
        "/bin/proc spawn leaves a ready /bin child before later scheduler dispatch; serial: {serial:?}",
    );
    assert!(
        serial.contains("syscalls:\n")
            && serial.contains("op=exec-load status=ok")
            && serial.contains("op=exec-spawn status=ok")
            && serial.contains("op=scheduler-dispatch status=ok")
            && serial.contains("op=vfs-normalize status=ok")
            && serial.contains("op=vfs-lookup status=ok")
            && serial.contains("op=source-media-snapshot status=ok")
            && serial.contains("op=source-media-read status=unavailable")
            && serial.contains("op=snapshot-source-store status=ok")
            && serial.contains("op=snapshot-syscalls status=ok")
            && serial.contains("op=tty-read-line status=ok")
            && serial.contains("loader=source-image entry_fn=bin_proc"),
        "cat /proc/syscalls prints typed syscall dispatch records; serial: {serial:?}",
    );
    assert!(
        serial.contains("waits:\n") && serial.contains("child_state=exited exit=0 completed=true"),
        "cat /proc/waits prints completed child wait namespace; serial: {serial:?}",
    );
    assert!(
        serial.contains("reovim-os> snapshot\nstatus\n"),
        "ls /dump prints dump namespace; serial: {serial:?}",
    );
    assert!(
        serial.contains("format=reovim-dump-v1")
            && serial.contains("boot_id=1")
            && serial.contains("session_id=1")
            && serial.contains("identity_source=rootd-volatile")
            && serial.contains("boot_memory_ranges=")
            && serial.contains("device_records=")
            && serial.contains("pending_exec_records=")
            && serial.contains("proof_state=operator-required")
            && serial.contains("panic_state=none")
            && serial.contains("persistent=available\nstorage=qemu-diagnostic-dump0")
            && serial.contains("storage_capacity_bytes=65536")
            && serial.contains("reovim-dump-v1\n")
            && serial.contains("checksum=")
            && serial.contains("boot:\n")
            && serial.contains("devices:\n")
            && serial.contains("proof:\nstate=operator-required")
            && serial.contains("panic:\nstate=none")
            && serial.contains("events:\n")
            && serial.contains("pending:\n")
            && serial.contains("syscalls:\n"),
        "cat /dump/status and /dump/snapshot print in-memory dump identity and forensic sections; serial: {serial:?}",
    );
    assert!(
        serial.contains("reovim-os> reovim-os> /dev"),
        "cd /dev updates cwd; serial: {serial:?}",
    );
    assert!(
        serial.contains("profile=shell-only"),
        "cat /boot/profile prints profile pseudo file; serial: {serial:?}",
    );
    assert!(
        serial.contains("shell: cat /boot/help"),
        "cat /log/dmesg prints VFS help command audit; serial: {serial:?}",
    );
    assert!(
        serial.contains("exec.path=/bin/pwd") && serial.contains("exec.path=/bin/cat"),
        "cat /log/dmesg prints program exec audit paths; serial: {serial:?}",
    );
    assert!(
        serial.contains(
            "event seq=1 component=boot severity=info kind=boot-start boot=1 session=1 source=rootd"
        ) && serial.contains(
            "component=proc severity=info kind=program-exit boot=1 session=1 source=process"
        ),
        "cat /log/dmesg prints structured boot and process events; serial: {serial:?}",
    );
    assert!(
        serial.contains("input=bootline-script"),
        "boot profile names scripted input harness; serial: {serial:?}",
    );
    assert!(
        serial.contains("usb_keyboard=unavailable"),
        "boot profile names missing USB keyboard provider; serial: {serial:?}",
    );
    assert!(
        serial.contains("rootd: boot report"),
        "cat /log/dmesg prints kernel boot log; serial: {serial:?}",
    );
    assert!(
        serial.contains("shell: pwd"),
        "cat /log/dmesg prints shell command log entries; serial: {serial:?}",
    );
    assert!(
        serial.contains("shell: clear")
            && serial.contains("exec.path=/bin/clear")
            && serial.contains("loader=source-image entry_fn=bin_clear"),
        "cat /log/dmesg prints source-image clear command audit; serial: {serial:?}",
    );
    assert!(
        serial.contains("shell: screentest")
            && serial.contains("exec.path=/bin/screentest")
            && serial.contains("loader=source-image entry_fn=bin_screentest"),
        "cat /log/dmesg prints source-image screentest command audit; serial: {serial:?}",
    );
    assert!(
        serial.contains("shell: mount")
            && serial.contains("exec.path=/bin/mount")
            && serial.contains("loader=source-image entry_fn=bin_mount"),
        "cat /log/dmesg prints source-image mount command audit; serial: {serial:?}",
    );
    assert!(
        serial.contains("shell: cat /boot/memory"),
        "cat /log/dmesg prints VFS memory command audit; serial: {serial:?}",
    );
    assert!(
        serial.contains("shell: cat /boot/devices"),
        "cat /log/dmesg prints VFS devices command audit; serial: {serial:?}",
    );
    assert!(
        serial.contains("shell: device")
            && serial.contains("exec.path=/bin/device")
            && serial.contains("loader=source-image entry_fn=bin_device"),
        "cat /log/dmesg prints source-image device command audit; serial: {serial:?}",
    );
    assert!(
        serial.contains("shell: input")
            && serial.contains("exec.path=/bin/input")
            && serial.contains("loader=source-image entry_fn=bin_input"),
        "cat /log/dmesg prints source-image input command audit; serial: {serial:?}",
    );
    assert!(
        serial.contains("shell: ls /log"),
        "cat /log/dmesg prints log namespace listing audit; serial: {serial:?}",
    );
    assert!(
        serial.contains("shell: cat uart0"),
        "cat /log/dmesg prints relative device command audit; serial: {serial:?}",
    );
    assert!(
        serial.contains("shell.status=ok"),
        "cat /log/dmesg prints shell command status entries; serial: {serial:?}",
    );
    assert!(
        serial.contains("shell: launch")
            && serial.contains("exec.path=/bin/launch")
            && serial.contains("shell: reovim")
            && serial.contains("exec.path=/bin/reovim")
            && serial.contains("shell.status=error"),
        "cat /log/dmesg prints shell-only payload-disabled audit entries; serial: {serial:?}",
    );
    assert!(
        serial.contains("shell: dmesg --stats")
            && serial.contains("exec.path=/bin/dmesg")
            && serial.contains("loader=source-image entry_fn=bin_dmesg"),
        "cat /log/dmesg prints dmesg stats command audit; serial: {serial:?}",
    );
    assert!(
        serial.contains("shell: dump status")
            && serial.contains("exec.path=/bin/dump")
            && serial.contains("loader=source-image entry_fn=bin_dump")
            && serial.contains("shell: dump sync")
            && serial.contains("status=written")
            && serial.contains("verified=true")
            && serial.contains("reason=written-readback-ok"),
        "cat /log/dmesg prints dump program audit and checked sync result; serial: {serial:?}",
    );
    assert!(
        serial.contains("shell: cat /log/stats"),
        "cat /log/dmesg prints VFS log stats command audit; serial: {serial:?}",
    );
    assert!(
        serial.contains("shell: cat /log/events"),
        "cat /log/dmesg prints VFS log events command audit; serial: {serial:?}",
    );
    for command in [
        "shell: help\n",
        "shell: help clear\n",
        "shell: help screentest\n",
        "shell: help input\n",
        "shell: help proof\n",
        "shell: help pwd\n",
        "shell: help ls\n",
        "shell: help cd\n",
        "shell: help cat\n",
        "shell: help read\n",
        "shell: help mount\n",
        "shell: help device\n",
        "shell: help dmesg\n",
        "shell: help dump\n",
        "shell: help sched\n",
        "shell: help proc\n",
        "shell: help status\n",
        "shell: help probe\n",
        "shell: help launch\n",
        "shell: help reovim\n",
        "shell: help halt\n",
    ] {
        assert!(
            serial.contains(command),
            "cat /log/dmesg prints targeted help audit {command:?}; serial: {serial:?}",
        );
    }
    assert!(
        serial.contains("shell: sched tick\n") && serial.contains("op=scheduler-tick status=ok"),
        "cat /log/dmesg prints /bin/sched tick audit; serial: {serial:?}",
    );
    assert!(
        serial.contains("shell: sched yield\n") && serial.contains("exec.path=/bin/sched"),
        "cat /log/dmesg prints /bin/sched yield audit; serial: {serial:?}",
    );
    assert!(
        serial.contains("shell: pwd | cat\n") && serial.contains("exec.path=/bin/cat"),
        "cat /log/dmesg prints /bin pipe audit; serial: {serial:?}",
    );
    assert!(
        serial.contains("shell: read\n")
            && serial.contains("exec.path=/bin/read")
            && serial.contains("loader=source-image entry_fn=bin_read"),
        "cat /log/dmesg prints /bin/read audit; serial: {serial:?}",
    );
    assert!(
        serial.contains("shell: proc\n") && serial.contains("exec.path=/bin/proc"),
        "cat /log/dmesg prints /bin/proc audit; serial: {serial:?}",
    );
    assert!(
        serial.contains("shell: proc exec pwd\n") && serial.contains("exec.path=/bin/pwd"),
        "cat /log/dmesg prints program child exec audit; serial: {serial:?}",
    );
    assert!(
        serial.contains("shell: proc exec cat /boot/profile\n")
            && serial.contains("exec.path=/bin/cat"),
        "cat /log/dmesg prints argv child exec audit; serial: {serial:?}",
    );
    assert!(
        serial.contains("shell: proc spawn pwd\n") && serial.contains("exec.path=/bin/pwd"),
        "cat /log/dmesg prints spawned /bin child audit; serial: {serial:?}",
    );
    assert!(
        serial.contains("exec.path=/bin/help"),
        "cat /log/dmesg prints /bin/help exec audit; serial: {serial:?}",
    );
    assert!(
        serial.contains("boot_info:"),
        "device command prints boot info; serial: {serial:?}",
    );
    assert!(
        serial.contains("dmesg:\nrootd: boot start"),
        "dmesg prints kernel log source; serial: {serial:?}",
    );
    let final_dmesg = serial
        .rsplit_once("dmesg:\nrootd: boot start")
        .map_or("", |(_, tail)| tail);
    assert!(
        !final_dmesg.contains("probe pcie:\nprobe pcie:")
            && !final_dmesg
                .contains("state=unsupported-on-this-target\nstate=unsupported-on-this-target"),
        "final dmesg does not duplicate probe fragments; serial: {serial:?}",
    );
    assert!(
        final_dmesg.contains("probe pcie:\nstate=unsupported-on-this-target"),
        "final dmesg preserves provider probe output; serial: {serial:?}",
    );
    assert!(
        final_dmesg.contains("probe xhci-read-keyboard-report:\nstate=unsupported-on-this-target"),
        "final dmesg preserves keyboard report probe output; serial: {serial:?}",
    );
    assert!(
        final_dmesg.contains("shell: cat /boot/probes")
            && final_dmesg.contains("probe targets:\n  pcie")
            && final_dmesg.contains("xhci-read-keyboard-report (alias: usb-keyboard-read-report)")
            && final_dmesg.contains("exec.path=/bin/cat"),
        "final dmesg preserves VFS probe catalog audit; serial: {serial:?}",
    );
    assert!(
        final_dmesg.contains("shell: launch")
            && final_dmesg.contains("exec.path=/bin/launch")
            && final_dmesg.contains("shell: reovim")
            && final_dmesg.contains("exec.path=/bin/reovim")
            && final_dmesg.contains("shell: dmesg"),
        "final log read preserves shell-only payload-disabled audit before dmesg; serial: {serial:?}",
    );
    assert!(
        final_dmesg.contains("shell: dmesg --stats")
            && final_dmesg.contains("exec.path=/bin/dmesg")
            && final_dmesg.contains("shell: dump status")
            && final_dmesg.contains("shell: dump snapshot")
            && final_dmesg.contains("shell: dump sync")
            && final_dmesg.contains("exec.path=/bin/dump")
            && final_dmesg.contains("loader=source-image entry_fn=bin_dump")
            && final_dmesg.contains("shell: cat /log/stats")
            && final_dmesg.contains("shell: cat /log/events")
            && final_dmesg.contains("exec.path=/bin/cat")
            && final_dmesg.contains("shell: dmesg")
            && final_dmesg.contains("shell: cat /log/dmesg"),
        "final log read exposes dump/log stats and dmesg status before its own command audit; serial: {serial:?}",
    );
    assert!(
        serial.contains("reovim-os> halt: ok\n"),
        "terminal halt command runs after final log read; serial: {serial:?}",
    );
    assert!(
        !serial.contains("halt: ok\nreovim-os>"),
        "terminal halt does not print another shell prompt; serial: {serial:?}",
    );
}

#[test]
fn os_proc_block_wake_transcript_on_x86_target_runs_woken_child() {
    let Some(target) = fixture_target() else {
        return;
    };
    if !target.starts_with("x86_64") {
        return;
    }

    let _guard = os_image_lock();
    let bootline = concat!(
        "proc block pwd\n",
        "proc wake 4\n",
        "pwd\n",
        "cat /proc/syscalls\n",
        "dmesg\n",
        "halt\n"
    );
    let exe = build_os_image(&[], Some(bootline), None);
    let (code, serial) = run_system_image(&exe);
    assert_eq!(code, 0, "proc block/wake transcript exits 0; serial: {serial:?}");
    assert!(
        serial.contains("reovim-os> proc block:\npid=4\npath=/bin/pwd\nstate=blocked\n"),
        "/bin/proc block leaves child blocked; serial: {serial:?}",
    );
    assert!(
        serial.contains("reovim-os> proc wake:\npid=4\npath=/bin/pwd\nstate=ready\n"),
        "/bin/proc wake makes blocked child ready; serial: {serial:?}",
    );
    assert!(
        serial.contains("op=process-block status=ok")
            && serial.contains("op=process-wake status=ok"),
        "process block/wake syscall records are retained; serial: {serial:?}",
    );
    assert!(
        serial.contains("reovim-os> /\n/\n"),
        "woken child /bin/pwd runs before later shell /bin/pwd; serial: {serial:?}",
    );
    assert!(
        serial.contains("exec.path=/bin/pwd pid=4 task=4 status=ok")
            && serial.contains("exec.path=/bin/pwd pid=6 task=6 status=ok"),
        "both woken and later /bin/pwd executions are audited; serial: {serial:?}",
    );
}

#[test]
fn os_proc_wait_transcript_on_x86_target_waits_for_spawned_child() {
    let Some(target) = fixture_target() else {
        return;
    };
    if !target.starts_with("x86_64") {
        return;
    }

    let _guard = os_image_lock();
    let bootline = concat!(
        "proc spawn pwd\n",
        "proc wait 4\n",
        "cat /proc/waits\n",
        "cat /proc/syscalls\n",
        "dmesg\n",
        "halt\n"
    );
    let exe = build_os_image(&[], Some(bootline), None);
    let (code, serial) = run_system_image(&exe);
    assert_eq!(code, 0, "proc wait transcript exits 0; serial: {serial:?}");
    assert!(
        serial.contains("reovim-os> proc spawn:\npid=4\npath=/bin/pwd\nstate=ready\n"),
        "/bin/proc spawn leaves child ready for retained wait; serial: {serial:?}",
    );
    assert!(
        serial.contains("reovim-os> /\nproc wait:\npid=4\nstate=exited\nexit=0\ncompleted=true\n"),
        "/bin/proc wait observes the retained child completion; serial: {serial:?}",
    );
    assert!(
        serial.contains("- parent_pid=5 child_pid=4 child_state=exited exit=0 completed=true\n"),
        "/proc/waits retains the completed wait; serial: {serial:?}",
    );
    assert!(
        serial.contains("path=/bin/proc op=wait-begin status=ok")
            && serial.contains("path=/bin/proc op=wait-end status=ok"),
        "wait begin/end syscall records are retained; serial: {serial:?}",
    );
    assert!(
        serial.contains("wait.start parent_pid=5 parent_task=5 child_pid=4 child_task=4\n")
            && serial.contains("wait.end parent_pid=5 child_pid=4 child_state=exited exit=0\n"),
        "wait lifecycle is present in dmesg; serial: {serial:?}",
    );
    assert!(
        serial.contains("exec.path=/bin/pwd pid=4 task=4 status=ok")
            && serial.contains("exec.path=/bin/proc pid=5 task=5 status=ok"),
        "spawned child and wait program executions are audited; serial: {serial:?}",
    );
}

#[test]
fn os_proc_kill_transcript_on_x86_target_terminates_blocked_child() {
    let Some(target) = fixture_target() else {
        return;
    };
    if !target.starts_with("x86_64") {
        return;
    }

    let _guard = os_image_lock();
    let bootline = concat!(
        "proc block pwd\n",
        "proc pending\n",
        "proc kill 4\n",
        "proc pending\n",
        "pwd\n",
        "cat /proc/syscalls\n",
        "dmesg\n",
        "halt\n"
    );
    let exe = build_os_image(&[], Some(bootline), None);
    let (code, serial) = run_system_image(&exe);
    assert_eq!(code, 0, "proc kill transcript exits 0; serial: {serial:?}");
    assert!(
        serial.contains("reovim-os> proc block:\npid=4\npath=/bin/pwd\nstate=blocked\n"),
        "/bin/proc block leaves child blocked; serial: {serial:?}",
    );
    assert!(
        serial.contains("reovim-os> proc kill:\npid=4\npath=/bin/pwd\nstate=failed\n"),
        "/bin/proc kill fails the blocked child; serial: {serial:?}",
    );
    assert!(
        serial.contains(
            "reovim-os> pending:\n- pid=4 ppid=1 task=4 path=/bin/pwd loader=source-image entry_fn=bin_pwd kind=bin stdin_bytes=0\n"
        ),
        "/bin/proc pending exposes the blocked child before kill; serial: {serial:?}",
    );
    assert!(
        serial.contains("reovim-os> pending:\nreovim-os> /\n"),
        "/bin/proc pending is empty after kill discards child image; serial: {serial:?}",
    );
    assert!(
        serial.contains("op=process-block status=ok")
            && serial.contains("op=process-kill status=ok")
            && serial.contains("op=process-exit status=error"),
        "process block/kill syscall records are retained; serial: {serial:?}",
    );
    assert!(
        serial.contains("reovim-os> /\nreovim-os> syscalls:")
            && !serial.contains("reovim-os> /\n/\n"),
        "killed child does not run before later shell /bin/pwd; serial: {serial:?}",
    );
    assert!(
        serial.contains("exec.path=/bin/pwd pid=4 task=4 status=error")
            && serial.contains("exec.path=/bin/pwd pid=8 task=8 status=ok"),
        "killed and later /bin/pwd executions are audited; serial: {serial:?}",
    );
}

#[test]
fn os_launch_profile_transcript_on_x86_target_launches_editor_smoke() {
    // The phase-4 proof from the official launch profile: a scripted root shell
    // launches image-linked and installed-source payloads and returns
    // `payload.ready`.
    let Some(target) = fixture_target() else {
        return;
    };
    if !target.starts_with("x86_64") {
        return;
    }

    let _guard = os_image_lock();
    let exe = build_os_image(
        &["launch-profile"],
        Some(
            "launch editor-smoke\nlaunch server-smoke\ncat /proc/sources\ncat /proc/execs\ncat /proc/processes\ncat /proc/tasks\ncat /proc/waits\ncat /proc/syscalls\ncat /log/dmesg\nhelp\n",
        ),
        Some("launch"),
    );
    let (code, serial) = run_system_image(&exe);
    assert_eq!(
        code, 0,
        "reovim-os launch profile exits 0 after editor smoke path; serial: {serial:?}"
    );
    assert!(
        serial.contains("launch editor-smoke: payload.ready"),
        "editor-smoke launch command succeeded; serial: {serial:?}",
    );
    assert!(
        serial.contains("launch server-smoke: payload.ready"),
        "server-smoke launch uses installed x86 source overlay; serial: {serial:?}",
    );
    assert!(
        serial.contains("namespace=payload path=/payload/server-smoke loader=source-image bytes=")
            && serial.contains("origin=installed"),
        "source-store table exposes installed server-smoke source; serial: {serial:?}",
    );
    assert!(
        serial.contains("argv0=editor-smoke status=ok reason=loaded path=/payload/editor-smoke")
            && serial.contains("path=/payload/editor-smoke source=/payload/editor-smoke")
            && serial.contains("entry_fn=payload_editor_smoke truncated=false kind=payload"),
        "editor-smoke payload admission is retained in /proc/execs; serial: {serial:?}",
    );
    assert!(
        serial.contains("argv0=server-smoke status=ok reason=loaded path=/payload/server-smoke")
            && serial.contains("entry_fn=payload_server_smoke truncated=false kind=payload"),
        "server-smoke installed payload admission is retained in /proc/execs; serial: {serial:?}",
    );
    assert!(
        serial.contains("path=/payload/editor-smoke exit=0"),
        "editor-smoke payload process is retained in /proc/processes; serial: {serial:?}",
    );
    assert!(
        serial.contains("entry=/payload/editor-smoke"),
        "editor-smoke payload task is retained in /proc/tasks; serial: {serial:?}",
    );
    assert!(
        serial.contains("child_state=exited exit=0 completed=true"),
        "editor-smoke payload wait is retained in /proc/waits; serial: {serial:?}",
    );
    assert!(
        serial.contains(
            "path=/payload/editor-smoke op=payload-run status=ok loader=source-image entry_fn=payload_editor_smoke"
        ),
        "editor-smoke payload execution is visible in /proc/syscalls; serial: {serial:?}",
    );
    assert!(
        serial.contains("payload.start path=/payload/editor-smoke")
            && serial.contains("payload.exit path=/payload/editor-smoke")
            && serial.contains("wait.start parent_pid=")
            && serial.contains("wait.end parent_pid="),
        "retained dmesg includes payload lifecycle rows; serial: {serial:?}",
    );
    assert!(
        serial.contains(
            "payload.start path=/payload/editor-smoke loader=source-image entry_fn=payload_editor_smoke"
        ),
        "editor-smoke payload is loaded as a source-image payload; serial: {serial:?}",
    );
    assert!(
        serial.contains("reovim root shell"),
        "help output still available after payload launch; serial: {serial:?}",
    );
}

#[test]
fn panic_halt_fixture_flushes_log2_and_exits_70() {
    if skip_in_system_image_mode("panic_halt_fixture_flushes_log2_and_exits_70") {
        return;
    }
    let exe = build_fixture("arch-fixture-panic-halt");
    let sink = scratch_path("halt");
    let code = run(&exe, &[sink.to_str().unwrap()]);
    let line = read_and_remove(&sink);
    assert_eq!(code, 70, "default disposition halts with EX_SOFTWARE");
    assert_log2_kernel_panic(&line, "halt-path fixture panic");
    assert!(!line.contains("rollback=failed"), "non-cleanup panic has no rollback marker");
}

#[test]
fn panic_recover_fixture_fires_hook_and_exits_75() {
    if skip_in_system_image_mode("panic_recover_fixture_fires_hook_and_exits_75") {
        return;
    }
    let exe = build_fixture("arch-fixture-panic-recover");
    let sink = scratch_path("recover");
    let code = run(&exe, &[sink.to_str().unwrap()]);
    let out = read_and_remove(&sink);
    assert_eq!(code, 75, "recover disposition exits EX_TEMPFAIL");
    assert!(
        out.contains("state-hook: disposition=recover rollback=ok"),
        "state-record hook fired with the expected PanicRecord: {out:?}",
    );
    assert_log2_kernel_panic(&out, "recover-path fixture panic");
}

#[test]
fn panic_ab13_fixture_marks_rollback_failed() {
    if skip_in_system_image_mode("panic_ab13_fixture_marks_rollback_failed") {
        return;
    }
    let exe = build_fixture("arch-fixture-panic-ab13");
    let sink = scratch_path("ab13");
    let code = run(&exe, &[sink.to_str().unwrap()]);
    let line = read_and_remove(&sink);
    assert_eq!(code, 70, "cleanup panic with no disposition halts");
    assert!(line.contains("rollback=failed"), "AB13 marker in flushed line: {line:?}");
    assert_log2_kernel_panic(&line, "ab13 cleanup-context panic");
}

/// Reads the flushed sink to a string and removes it.
fn read_and_remove(path: &Path) -> String {
    let s = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("read sink {}: {e}", path.display()));
    let _ = std::fs::remove_file(path);
    s
}

/// Parses a kernel-emitter LOG2 line out of `out` and asserts its fields
/// against the 9.5 §2 grammar: `[ts] emitter SP address ":" SP message`,
/// ts = `seconds.micros` (micros width 6), kernel address has no `/`.
/// Asserts the emitter (`kernel`), the `panic` subsystem address, and the
/// message substring — not exact bytes (the timestamp varies per run).
fn assert_log2_kernel_panic(out: &str, msg_substr: &str) {
    let line = out
        .lines()
        .find(|l| l.contains("kernel panic:"))
        .unwrap_or_else(|| panic!("a panic line in {out:?}"));
    // Hosted suites split runner progress (stdout) from the panic line
    // (stderr), so the line starts at the timestamp. On a shared serial
    // console (system-image mode) both multiplex onto one stream and the
    // LOG2 line lands mid-line after `test <name> ... `. Parse from the
    // timestamp bracket that opens the LOG2 grammar, wherever it sits.
    let panic_idx = line
        .find("kernel panic:")
        .expect("present by the find above");
    let open = line[..panic_idx].rfind('[').expect("ts opens with '['");
    let line = &line[open..];
    let close = line.find(']').expect("ts closes with ']'");
    let ts = &line[1..close];
    let (secs, micros) = ts.split_once('.').expect("ts is seconds.micros");
    assert!(secs.trim().parse::<u64>().is_ok(), "ts seconds numeric: {secs:?}");
    assert_eq!(micros.len(), 6, "micros zero-padded width 6");
    assert!(micros.parse::<u32>().is_ok(), "ts micros numeric: {micros:?}");
    let rest = line[close + 1..].trim_start();
    let (emitter, after) = rest.split_once(' ').expect("emitter token");
    assert_eq!(emitter, "kernel", "kernel-emitter form");
    let (address, message) = after.split_once(": ").expect("address ':' message");
    assert_eq!(address, "panic", "kernel subsystem address");
    assert!(!address.contains('/'), "kernel address has no '/'");
    assert!(message.contains(msg_substr), "message carries the panic text: {message:?}");
}
