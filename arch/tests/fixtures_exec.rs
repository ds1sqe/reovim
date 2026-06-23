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
    let exe = build_os_image(
        &[],
        Some(
            "help\nhelp clear\nhelp screentest\nhelp input\nhelp status\nhelp proof\nhelp probe\nclear\nscreentest\npwd\nls /\ncd /dev\npwd\nls\ncat /boot/profile\ncat /boot/image\ninput\ncat /boot/input\nstatus\ncat /boot/status\nproof\ncat /boot/proof\nmount\ncat /boot/mounts\ncat /log/dmesg\ndevice\nprobe help\nprobe pcie\nprobe usb-keyboard\nprobe xhci-start\nprobe xhci-enable-slot\nprobe xhci-address-device\nprobe xhci-get-device-descriptor\nprobe xhci-set-address\nprobe xhci-read-device-descriptor\nprobe xhci-read-config-descriptor-header\nprobe xhci-read-config-descriptor\nprobe xhci-set-configuration\nprobe xhci-configure-endpoint\nprobe xhci-set-hid-protocol\nprobe xhci-read-keyboard-report\nlaunch\nreovim\ndmesg\ncat /log/dmesg\n",
        ),
        None,
    );
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
        serial.contains(
            "commands: help, clear, screentest, pwd, ls, cd, cat, mount, input, status, proof, device, dmesg, probe, launch, reovim, halt"
        ),
        "help vocabulary appears; serial: {serial:?}",
    );
    assert!(
        serial.contains("usage: help [command]"),
        "help usage appears; serial: {serial:?}",
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
        serial.contains("input - print live console input diagnostics"),
        "input help appears; serial: {serial:?}",
    );
    assert!(
        serial.contains("status - print boot and input summary"),
        "status help appears; serial: {serial:?}",
    );
    assert!(
        serial.contains("proof - print physical input proof checklist"),
        "proof help appears; serial: {serial:?}",
    );
    assert!(
        serial.contains("probe target - run lower hardware probe; try `probe help`"),
        "probe help appears; serial: {serial:?}",
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
        serial.contains("probe targets:"),
        "probe target help appears in transcript; serial: {serial:?}",
    );
    assert!(
        serial.contains("proof:\ncommands:\n  help\n  clear\n  screentest"),
        "proof checklist appears in transcript; serial: {serial:?}",
    );
    assert!(
        serial.contains("  pwd\n  ls /\n  ls /boot"),
        "proof checklist includes root namespace commands; serial: {serial:?}",
    );
    assert!(
        serial.contains("  ls /dev\n  mount\n  cat /boot/mounts"),
        "proof checklist includes VFS namespace commands; serial: {serial:?}",
    );
    assert!(
        serial.contains("  device\n  cat /boot/memory\n  cat /boot/devices"),
        "proof checklist includes boot inventory commands; serial: {serial:?}",
    );
    assert!(
        serial.contains("  cd /dev\n  pwd\n  cat uart0\n  cd /"),
        "proof checklist includes relative device access commands; serial: {serial:?}",
    );
    assert!(
        serial.contains("expected:\n  bootline=absent"),
        "proof checklist expected facts appear; serial: {serial:?}",
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
        serial.contains("  probe help\n  probe pcie\n  probe usb-keyboard"),
        "proof checklist includes read-only PCIe probe; serial: {serial:?}",
    );
    assert!(
        serial.contains("  launch\n  reovim"),
        "proof checklist includes shell-only payload checks; serial: {serial:?}",
    );
    assert!(
        serial.contains("launch/reovim disabled in shell-only profile"),
        "proof checklist names shell-only payload-disabled expectation; serial: {serial:?}",
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
        serial.contains("kernel on / type rootfs (ro,pseudo)"),
        "mount table appears in transcript; serial: {serial:?}",
    );
    assert!(
        serial.contains("klog on /log type logfs (ro,pseudo)"),
        "mount table includes klog namespace; serial: {serial:?}",
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
        serial.contains("reovim-os> boot\ndev\nlog"),
        "ls / prints root namespace; serial: {serial:?}",
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
        serial.contains("shell: clear"),
        "cat /log/dmesg prints clear command audit; serial: {serial:?}",
    );
    assert!(
        serial.contains("shell: screentest"),
        "cat /log/dmesg prints screentest command audit; serial: {serial:?}",
    );
    assert!(
        serial.contains("shell.status=ok"),
        "cat /log/dmesg prints shell command status entries; serial: {serial:?}",
    );
    assert!(
        serial.contains("shell: launch\nshell.status=error\nshell: reovim\nshell.status=error"),
        "cat /log/dmesg prints shell-only payload-disabled audit entries; serial: {serial:?}",
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
        final_dmesg.contains("probe pcie:\nstate=unsupported-on-this-target"),
        "final dmesg preserves provider probe output; serial: {serial:?}",
    );
    assert!(
        final_dmesg.contains("probe xhci-read-keyboard-report:\nstate=unsupported-on-this-target"),
        "final dmesg preserves keyboard report probe output; serial: {serial:?}",
    );
    assert!(
        final_dmesg.contains(
            "shell: launch\nshell.status=error\nshell: reovim\nshell.status=error\nshell: dmesg"
        ),
        "final log read preserves shell-only payload-disabled audit before dmesg; serial: {serial:?}",
    );
    assert!(
        final_dmesg.contains("shell: dmesg\nshell.status=ok\nshell: cat /log/dmesg"),
        "final log read exposes dmesg status before its own command audit; serial: {serial:?}",
    );
}

#[test]
fn os_launch_profile_transcript_on_x86_target_launches_editor_smoke() {
    // The phase-4 proof from the official launch profile: a scripted root shell
    // launches the editor-core smoke payload and returns `payload.ready`.
    let Some(target) = fixture_target() else {
        return;
    };
    if !target.starts_with("x86_64") {
        return;
    }

    let _guard = os_image_lock();
    let exe =
        build_os_image(&["launch-profile"], Some("launch editor-smoke\nhelp\n"), Some("launch"));
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
