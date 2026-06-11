//! DEV2 E2E exec harness (#797 Phase 5, bootstrap-state-1).
//!
//! Boots the composed `reovim` launcher (apps/reovim) headlessly, drives a
//! scripted byte sequence through a **pipe** (not a PTY — DEV5 determinism
//! pin), captures the composed ANSI frame bytes from stdout, and asserts
//! them against committed goldens.
//!
//! ## What this test proves
//!
//! 1. The composed binary boots: kernel + text Domain + UDS listener + TUI
//!    client all compose end-to-end into one working process.
//! 2. DEV3 frame golden: the captured initial frame bytes match the committed
//!    golden byte-for-byte; a stale golden fails with a named-path diff.
//! 3. DEV4 buffer-byte golden: the buffer content visible in the post-input
//!    frame matches the committed buffer dump, proving the screen is right
//!    AND the bytes are right.
//! 4. DEV5 reproducibility: the test runs the same scenario twice and asserts
//!    the two captures are byte-identical; any wall-clock / hostname / path
//!    leak in the frame would produce a mismatch.
//!
//! ## Pipe-fed stdin (DEV5 determinism)
//!
//! The launched process reads from a pipe, not a real terminal. The TUI
//! client's `RawMode::enter(0)` returns `ENOTTY` on a pipe fd and the runtime
//! continues without raw mode (the DEV2-compatible path documented in
//! `paint.rs`). Input bytes are injected by writing to the pipe write-end from
//! the orchestrating process.
//!
//! ## Golden files
//!
//! Goldens live beside this file under `goldens/`:
//!
//! | File | Contents |
//! |---|---|
//! | `goldens/initial-frame.ansi` | ANSI bytes for the initial empty-buffer frame |
//! | `goldens/after-x-frame.ansi` | ANSI bytes after inserting `x` |
//! | `goldens/after-x-buffer.bin` | Raw buffer bytes after inserting `x` |
//!
//! **Blessing**: re-derive the golden from the formula in the comments below;
//! update on intentional output changes only — never to silence a red test.
//!
//! ## Socket-path TID uniqueness
//!
//! Every test invocation uses `reovim_arch::testrt::unique_path`-style TID-
//! unique socket paths so parallel test runs never collide. The socket is
//! passed as `argv[1]` to the launched binary and removed after each run.

use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

// ---------------------------------------------------------------------------
// Golden-file helpers
// ---------------------------------------------------------------------------

/// Returns the `tests/goldens/` directory beside this integration test file.
fn goldens_dir() -> PathBuf {
    // `CARGO_MANIFEST_DIR` for an integration test resolves to the *crate*
    // root (tools/testing/), not to the file's parent directory.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("goldens")
}

/// Reads the committed golden at `name` from the goldens directory.
///
/// Panics with the path on read failure so the error message names the file.
fn read_golden(name: &str) -> Vec<u8> {
    let path = goldens_dir().join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("read golden {}: {e}", path.display()))
}

// ---------------------------------------------------------------------------
// Build helper
// ---------------------------------------------------------------------------

/// Builds the `reovim` launcher from the nested `apps/` workspace and returns
/// the executable path parsed from cargo's JSON build messages.
///
/// Uses `env!("CARGO")` so the build uses the same toolchain that is running
/// the test, identical to the pattern in `arch/tests/fixtures_exec.rs` and
/// `server/lib/server/tests/fixtures_exec.rs`.
fn build_reovim() -> PathBuf {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent() // tools/
        .and_then(Path::parent) // workspace root
        .expect("workspace root reachable from tools/testing/");
    let apps_manifest = workspace_root.join("apps").join("Cargo.toml");

    let output = Command::new(env!("CARGO"))
        .args([
            "build",
            "--manifest-path",
            apps_manifest.to_str().expect("apps manifest path is UTF-8"),
            "--package",
            "reovim",
            "--message-format=json-render-diagnostics",
        ])
        .output()
        .expect("cargo build runs");
    assert!(
        output.status.success(),
        "apps/reovim build failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("cargo json is UTF-8");
    let exe = stdout
        .lines()
        .filter(|l| l.contains("\"compiler-artifact\""))
        .filter(|l| l.contains("\"reovim\""))
        .filter_map(extract_executable)
        .next_back()
        .expect("cargo JSON names the reovim executable");
    PathBuf::from(exe)
}

/// Extracts the `"executable":"..."` value from one cargo JSON message line.
/// Returns `None` when the field is absent or null. A minimal hand-parser
/// keeps the test free of a JSON dependency.
fn extract_executable(line: &str) -> Option<String> {
    let key = "\"executable\":";
    let start = line.find(key)? + key.len();
    let rest = line[start..].trim_start();
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(rest[..end].to_owned())
}

// ---------------------------------------------------------------------------
// Scenario runner
// ---------------------------------------------------------------------------

/// Constructs a TID-unique socket path for the E2E test.
///
/// Uses `std::process::id()` (pid) for uniqueness within a single test run.
/// The path fits within Linux's 107-byte UDS limit.
fn unique_socket_path(tag: &str) -> String {
    // PID-unique within this test process; two parallel test invocations each
    // have their own PID and thus their own socket. The string is shorter than
    // the 107-byte UNIX_PATH_MAX.
    format!("/tmp/reovim-e2e-{tag}-{}.sock", std::process::id())
}

/// Runs the E2E scenario once:
///
/// 1. Launch `reovim_exe` with `socket_path` as `argv[1]`, pipe stdin, capture
///    stdout.
/// 2. Write the scripted input bytes to stdin, then close the pipe write-end so
///    the process sees EOF and exits cleanly.
/// 3. Wait for the process to exit; assert exit code 0.
/// 4. Return the captured stdout bytes.
///
/// The `reovim` process:
/// - Boots kernel + text Domain + listener (on the given socket).
/// - Connects TUI client, paints the initial frame to stdout.
/// - Reads `input_bytes` from stdin, sends `SendInput`, receives the resulting
///   Projection notify, paints the post-input frame to stdout.
/// - On stdin EOF returns from `run_loop` and exits 0.
fn run_scenario(reovim_exe: &Path, socket_path: &str, input_bytes: &[u8]) -> Vec<u8> {
    // Remove any stale socket from a previous test (the binary also unlinks on
    // start, but being explicit here avoids a bind race on a slow CI host).
    let _ = std::fs::remove_file(socket_path);

    let mut child = Command::new(reovim_exe)
        .arg(socket_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null()) // suppress server/terminal noise from the test output
        .spawn()
        .expect("reovim spawns");

    // Write the scripted input then close stdin so the process sees EOF after
    // processing the input.
    {
        let stdin = child.stdin.take().expect("stdin pipe open");
        // A short sleep is NOT used here: the process handles input only after
        // connecting to the server and painting the initial frame. To avoid a
        // race between "initial frame painted" and "write to stdin", we write
        // input and close the pipe, then drain stdout. The `read_to_end` below
        // blocks until the process closes stdout (i.e., exits), which happens
        // after all output is written. If the process reads stdin before it
        // has connected, `is_forwardable` will skip the byte anyway (no connection
        // established yet); in practice the two goroutines (listener + client)
        // race and the client always connects first (it blocks in connect, which
        // resolves as soon as the listener finishes bind — no sleep needed per
        // runtime_smoke.rs §4 comment).
        //
        // Use a helper thread to write + close so we can simultaneously drain
        // stdout (avoiding a deadlock when the child's stdout buffer is full).
        let input_owned = input_bytes.to_vec();
        std::thread::spawn(move || {
            let mut pipe = stdin;
            // Ignore write errors — the process may have exited by the time we
            // write (e.g. if it panicked during connect).
            let _ = pipe.write_all(&input_owned);
            // Drop of `pipe` closes the write-end of the pipe, sending EOF
            // to the child.
        });
    }

    // Drain stdout. Blocks until the child closes its stdout (i.e., exits).
    let mut stdout = child.stdout.take().expect("stdout pipe open");
    let mut captured = Vec::new();
    stdout
        .read_to_end(&mut captured)
        .expect("read child stdout");

    let status = child.wait().expect("wait for reovim");
    assert_eq!(status.code(), Some(0), "reovim E2E scenario must exit 0 (socket={socket_path})");

    // Clean up the socket left by the binary.
    let _ = std::fs::remove_file(socket_path);

    captured
}

// ---------------------------------------------------------------------------
// Frame-content extraction (DEV4)
// ---------------------------------------------------------------------------

/// Extracts the buffer-content bytes from a captured ANSI frame.
///
/// The frame format produced by `compose_ansi_frame` is:
///
/// ```text
/// ESC[2J ESC[H <content> ESC[1;<col>H
/// ```
///
/// where `ESC` = `\x1b`. This function strips the leading `\x1b[2J\x1b[H`
/// and trailing `\x1b[1;<digits>H` and returns the middle slice.
///
/// Returns `None` when the frame does not match the expected prefix / suffix
/// (unexpected format or truncation).
fn extract_frame_content(frame: &[u8]) -> Option<&[u8]> {
    let prefix: &[u8] = b"\x1b[2J\x1b[H";
    let start = frame.strip_prefix(prefix)?;
    // Find the cursor-reposition suffix: ESC[1;NNH where NN is one or more digits.
    // Walk backwards from the end.
    let suffix_start = find_cursor_reposition_suffix_start(start)?;
    Some(&start[..suffix_start])
}

/// Finds the byte offset at which the trailing `\x1b[1;<digits>H` starts
/// within `after_prefix`.
fn find_cursor_reposition_suffix_start(after_prefix: &[u8]) -> Option<usize> {
    // The suffix is `ESC [ 1 ; <digits> H` — at minimum 7 bytes: `\x1b[1;1H`.
    if after_prefix.len() < 7 {
        // Entire slice is the cursor-reposition sequence (empty content).
        // Check that the slice IS `\x1b[1;<digits>H`.
        return if after_prefix.starts_with(b"\x1b[1;") && after_prefix.ends_with(b"H") {
            Some(0)
        } else {
            None
        };
    }
    // Scan backwards for `\x1b[1;`.
    let marker = b"\x1b[1;";
    for i in (0..=after_prefix.len().saturating_sub(marker.len())).rev() {
        if after_prefix[i..].starts_with(marker) && after_prefix.ends_with(b"H") {
            // Verify everything from `i+4` to `len-1` is ASCII digits.
            let digits = &after_prefix[i + marker.len()..after_prefix.len() - 1];
            if !digits.is_empty() && digits.iter().all(u8::is_ascii_digit) {
                return Some(i);
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// DEV3/DEV4/DEV5 tests
// ---------------------------------------------------------------------------

// Build once; share across all test functions. `std::sync::OnceLock` is the
// safe single-init cell.
fn reovim_exe() -> &'static PathBuf {
    static EXE: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    EXE.get_or_init(build_reovim)
}

/// DEV2 smoke: boot the composed launcher, drive `x` via pipe, assert exit 0.
/// DEV3: initial frame == golden. DEV4: buffer content after `x` == golden.
#[test]
fn dev2_dev3_dev4_e2e_smoke_exits_zero_and_frames_match_goldens() {
    let exe = reovim_exe();
    let socket = unique_socket_path("smoke");

    let initial_golden = read_golden("initial-frame.ansi");
    let after_x_golden = read_golden("after-x-frame.ansi");
    let buffer_golden = read_golden("after-x-buffer.bin");

    // Run the scenario: pipe `x` then EOF.
    let captured = run_scenario(exe, &socket, b"x");

    // The captured output is the two frames concatenated:
    //   [initial frame][after-x frame]
    // Split by the initial frame length.
    let initial_len = initial_golden.len();
    assert!(
        captured.len() >= initial_len,
        "captured stdout must be at least as long as the initial frame golden \
         ({initial_len} bytes); got {} bytes — \
         initial_golden path: {}/initial-frame.ansi",
        captured.len(),
        goldens_dir().display(),
    );
    let captured_initial = &captured[..initial_len];
    let captured_after = &captured[initial_len..];

    // DEV3 assertion — initial frame.
    assert_eq!(
        captured_initial,
        initial_golden.as_slice(),
        "DEV3 initial-frame golden mismatch — golden path: {}/initial-frame.ansi\n\
         expected: {:?}\n  actual: {:?}",
        goldens_dir().display(),
        initial_golden,
        captured_initial,
    );

    // DEV3 assertion — after-x frame.
    assert_eq!(
        captured_after,
        after_x_golden.as_slice(),
        "DEV3 after-x-frame golden mismatch — golden path: {}/after-x-frame.ansi\n\
         expected: {:?}\n  actual: {:?}",
        goldens_dir().display(),
        after_x_golden,
        captured_after,
    );

    // DEV4 assertion — extract buffer content from the after-x frame.
    let extracted = extract_frame_content(captured_after).unwrap_or_else(|| {
        panic!("DEV4: could not extract content bytes from after-x frame: {captured_after:?}")
    });
    assert_eq!(
        extracted,
        buffer_golden.as_slice(),
        "DEV4 buffer-byte golden mismatch — golden path: {}/after-x-buffer.bin\n\
         expected: {:?}\n  actual: {:?}",
        goldens_dir().display(),
        buffer_golden,
        extracted,
    );
}

/// DEV5 reproducibility: run the same scenario twice and assert byte-identical
/// captures. Any wall-clock time, hostname, username, or absolute path leaking
/// into the ANSI frame would produce a mismatch here.
#[test]
fn dev5_two_runs_are_byte_identical() {
    let exe = reovim_exe();

    let socket_a = unique_socket_path("dev5a");
    let socket_b = unique_socket_path("dev5b");

    let run_a = run_scenario(exe, &socket_a, b"x");
    let run_b = run_scenario(exe, &socket_b, b"x");

    assert_eq!(
        run_a,
        run_b,
        "DEV5: two runs of the same E2E scenario must produce byte-identical captures;\
         \n  run A ({} bytes): {:?}\
         \n  run B ({} bytes): {:?}",
        run_a.len(),
        run_a,
        run_b.len(),
        run_b,
    );
}

/// DEV5 leakage check: the captured frame must NOT contain an absolute path
/// (a byte sequence starting with `/` followed by a path-like character,
/// indicative of a `/tmp/...` socket or `$HOME` leak in the rendered output).
///
/// The ANSI frames produced by `compose_ansi_frame` carry only raw buffer
/// bytes (the text content typed by the user), ANSI escape sequences, and
/// cursor-positioning codes. None of these include absolute paths.
#[test]
fn dev5_frame_contains_no_absolute_path_leak() {
    let exe = reovim_exe();
    let socket = unique_socket_path("dev5leak");

    let captured = run_scenario(exe, &socket, b"x");

    // An absolute path would be a `/` byte followed by an alphanumeric
    // character (e.g. `/tmp/`, `/home/`). The ANSI frames should not contain
    // any such sequence.
    for window in captured.windows(2) {
        if let [slash, next] = window {
            assert!(
                !(*slash == b'/' && next.is_ascii_alphabetic()),
                "DEV5 leakage: absolute-path-like sequence found in captured frame: {:?}",
                &captured,
            );
        }
    }
}
