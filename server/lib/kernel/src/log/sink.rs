//! `FileSink` — the LOG7 file-backed DS12 subscriber (9.5 §9).
//!
//! `FileSink` is an optional DS12 subscriber that writes rendered LOG2 lines
//! to `<state-dir>/reovim.log`. It is **not** opened automatically during
//! boot; the caller attaches it after construction via
//! [`FileSink::open_and_subscribe`].
//!
//! ## Process-global state
//!
//! One file sink per process is the deliberate scope (LOG7). LOG7
//! specifies the single kernel file sink; multi-sink and rotation are deferred.
//! Sink state lives in a `static SINK: Mutex<SinkState>` so
//! the subscriber callback — a plain `fn(&DS12Event)` pointer — can reach it
//! without any raw-pointer indirection. The `const fn Mutex::new` constructor
//! makes the static safe.
//!
//! ## Open and replay (LOG8)
//!
//! On open, `FileSink` replays the ring head first so the log file contains
//! the full boot sequence including pre-sink events (LOG8 §9 replay). After
//! replay, every new DS12 event is written as a LOG2 line.
//!
//! ## Non-blocking failure (LOG7)
//!
//! On a write failure: the sink marks itself closed (so subsequent events are
//! ignored), releases the state lock, then emits one `log.sink.fail` event
//! via the stored `Shared<DS12EventBus>`. Re-entrant emission is safe: CC6
//! clone-then-invoke holds no lock across callbacks, and the now-closed sink
//! ignores the re-entrant event. The sink never panics and never blocks the
//! emitter.
//!
//! ## Early stderr gate (LOG8)
//!
//! `stderr_echo` writes a rendered line to fd 2 when the sink is not yet open
//! (`fd` is `None`) or when the headless flag is set. This is called from
//! `LogRing::push_event` — a one-way ring→sink gate.

use reovim_arch::{
    ds::Shared,
    sync::Mutex,
    sys::{AT_FDCWD, O_CLOEXEC, O_CREAT, O_WRONLY, close, openat, write},
};

use crate::{
    event_bus::{BootStageFields, DS12Event, DS12EventBus, SubscribeError},
    log::{
        render::{EmitterAddress, InstanceAddress, RenderInput, render_line},
        ring::kernel_subsystem_from_event,
    },
};

// ── O_APPEND ──────────────────────────────────────────────────────────────────
//
// `O_APPEND` (0o2000 / 1024 decimal) is not yet exported by `arch::sys`.
// Defined locally; matches Linux x86_64 and aarch64 (same value on both).
// When arch/sys/wrap.rs gains the export, replace this with the import.
const O_APPEND: usize = 0o2000;

// ── OBS1 event constant (9.5 family) ─────────────────────────────────────────

/// `log.sink.fail` — emitted when the file sink encounters a write failure.
///
/// Visible via the ring and any other DS12 subscriber; the emitter is never
/// blocked (LOG7 non-blocking contract).
///
/// ```rust
/// use reovim_kernel::log::sink::EVT_LOG_SINK_FAIL;
/// assert!(EVT_LOG_SINK_FAIL.starts_with("log."));
/// ```
pub const EVT_LOG_SINK_FAIL: &str = "log.sink.fail";

// ── SinkState ─────────────────────────────────────────────────────────────────

/// Interior state of the process-global file sink.
struct SinkState {
    /// Open file descriptor (`>= 0`), or `None` when not yet opened or after
    /// a fatal failure.
    fd: Option<i32>,
    /// When `true`, rendered lines continue to be written to fd 2 (stderr)
    /// after the sink opens (headless-server posture). Placeholder default
    /// `true`; TUI-suppression policy lands with the client phase (Phase E).
    headless: bool,
    /// Set on first write failure. Once closed the subscriber callback drops
    /// all events without attempting further writes (LOG7 non-blocking).
    closed: bool,
    /// Clone of the `Shared<DS12EventBus>` stored so the subscriber callback
    /// can emit `log.sink.fail` after releasing the state lock.
    /// `None` until `open_and_subscribe` is called.
    bus: Option<Shared<DS12EventBus>>,
}

impl SinkState {
    /// Const-constructible initial state (sink closed, headless by default).
    const fn initial() -> Self {
        Self {
            fd: None,
            headless: true,
            closed: false,
            bus: None,
        }
    }
}

// ── Process-global sink state ─────────────────────────────────────────────────
//
// One sink per process (LOG7 scope). The `static` is reachable by
// the subscriber callback — a plain `fn(&DS12Event)` — without any raw-pointer
// indirection. `Mutex::new` is `const`, so the static initializer is sound.
//
// Tests that need a clean slate call `reset_for_test()` at the start of each
// test case (selftest-gated).

static SINK: Mutex<SinkState> = Mutex::new(SinkState::initial());

// ── FileSink ─────────────────────────────────────────────────────────────────

/// The LOG7 file-backed DS12 subscriber (9.5 §9).
///
/// Created via [`FileSink::new`] and attached to the bus with
/// [`FileSink::open_and_subscribe`], which replays the ring head (LOG8)
/// and registers the sink subscriber.
///
/// One `FileSink` per process is the deliberate scope: LOG7 specifies the
/// single kernel file sink; multi-sink / rotation is deferred.
///
/// # Example
///
/// ```rust,no_run
/// // no_run: requires the arch runtime — use the kernel-selftest bin.
/// use reovim_arch::ds::Shared;
/// use reovim_kernel::log::ring::LogRing;
/// use reovim_kernel::log::sink::FileSink;
/// use reovim_kernel::event_bus::DS12EventBus;
///
/// let ring = Shared::try_new(LogRing::try_new(1024 * 1024).unwrap()).unwrap();
/// let bus = Shared::try_new(DS12EventBus::new()).unwrap();
/// let sink = FileSink::new(b"/tmp/reovim-test.log\0");
/// sink.open_and_subscribe(&ring, &bus).expect("sink open");
/// ```
pub struct FileSink {
    /// NUL-terminated log path, owned by value so callers may build the path
    /// in a stack buffer (e.g. a per-process unique test path). Sized for
    /// `sockaddr_un`-class paths; the log path shares that scale.
    path: [u8; 128],
    /// Filled length of `path` (including the NUL).
    path_len: usize,
}

impl FileSink {
    /// Creates a `FileSink` targeting `path`.
    ///
    /// `path` MUST be a NUL-terminated byte slice (the `openat` contract).
    /// The sink is not opened yet; call [`open_and_subscribe`] to open and
    /// register.
    ///
    /// # Panics
    ///
    /// Panics when `path` exceeds the 128-byte internal buffer — a path that
    /// long would be refused by `openat` on a Unix socket-class limit anyway.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// // no_run: requires the arch runtime — use the kernel-selftest bin.
    /// use reovim_kernel::log::sink::FileSink;
    /// let sink = FileSink::new(b"/tmp/reovim.log\0");
    /// ```
    ///
    /// [`open_and_subscribe`]: FileSink::open_and_subscribe
    #[must_use]
    pub const fn new(path: &[u8]) -> Self {
        assert!(path.len() <= 128, "FileSink path exceeds the internal buffer");
        let mut buf = [0u8; 128];
        let mut i = 0;
        while i < path.len() {
            buf[i] = path[i];
            i += 1;
        }
        Self {
            path: buf,
            path_len: path.len(),
        }
    }

    /// Opens the log file, replays the ring head (LOG8), and registers the
    /// sink as a DS12 subscriber on `bus`.
    ///
    /// On success, every subsequent DS12 event is written as a LOG2 line to
    /// the file. A `Shared<DS12EventBus>` clone is stored so that write
    /// failures can emit `log.sink.fail` (LOG7).
    ///
    /// # Errors
    ///
    /// Returns [`SinkOpenError`] when `openat` fails or bus subscriber
    /// capacity is exhausted. On `openat` failure the `log.sink.fail` event
    /// is emitted via `bus` before this function returns (the caller still
    /// receives the `Err`; the event is advisory).
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// // no_run: requires the arch runtime — use the kernel-selftest bin.
    /// use reovim_arch::ds::Shared;
    /// use reovim_kernel::log::ring::LogRing;
    /// use reovim_kernel::log::sink::FileSink;
    /// use reovim_kernel::event_bus::DS12EventBus;
    ///
    /// let ring = Shared::try_new(LogRing::try_new(1024 * 1024).unwrap()).unwrap();
    /// let bus = Shared::try_new(DS12EventBus::new()).unwrap();
    /// let sink = FileSink::new(b"/tmp/reovim-test.log\0");
    /// let _ = sink.open_and_subscribe(&ring, &bus);
    /// ```
    pub fn open_and_subscribe(
        &self,
        ring: &Shared<crate::log::ring::LogRing>,
        bus: &Shared<DS12EventBus>,
    ) -> Result<(), SinkOpenError> {
        // ── Open the log file ─────────────────────────────────────────────────
        //
        // O_WRONLY | O_CREAT | O_APPEND | O_CLOEXEC
        // O_APPEND: sequential writes go to end even if another process has
        // the file open. O_TRUNC is intentionally absent: we replay the ring
        // head after open so the file contains the full boot history.
        // Mode 0o644: owner read/write, group/other read.
        let flags = O_WRONLY | O_CREAT | O_APPEND | O_CLOEXEC;
        let open_result = openat(AT_FDCWD, &self.path[..self.path_len], flags, 0o644);
        let fd = match open_result {
            // Kernel-ABI fds fit in i32; a larger value is a kernel-contract
            // violation we surface as EBADF rather than truncate.
            Ok(raw_fd) => {
                let Ok(fd) = i32::try_from(raw_fd) else {
                    emit_sink_fail(bus, reovim_arch::sys::EBADF.0);
                    return Err(SinkOpenError::Open(reovim_arch::sys::EBADF));
                };
                fd
            }
            Err(errno) => {
                // Emit log.sink.fail advisory event after returning the error.
                // We emit here (before return) so the event is visible even if
                // the caller ignores the returned error.
                emit_sink_fail(bus, errno.0);
                return Err(SinkOpenError::Open(errno));
            }
        };

        // ── Store fd and bus clone in global sink state ───────────────────────
        {
            let mut st = SINK.lock();
            st.fd = Some(fd);
            st.bus = Some(Shared::clone(bus));
            st.closed = false;
        }

        // ── Register the panic-flush fd (AB12, 9.5 §9.1) ─────────────────────
        //
        // `set_flush_fd` is write-once. We register it here (sink-open time)
        // rather than at boot because the fd does not exist until the file is
        // opened. The arch write-once contract prevents correction after the
        // fact, so we cannot register a placeholder at boot. The default posture
        // (no fd → panic writes to stderr) is safe until this point.
        //
        // `AlreadySet` means another sink previously registered a fd for this
        // process (multi-sink scenario, or re-open in tests). Silently accept it:
        // the first registered fd wins and is used by the panic handler.
        let _ = reovim_arch::panic::set_flush_fd(fd);

        // ── Replay ring head (LOG8) ───────────────────────────────────────────
        //
        // Write every current ring entry to the file so the file contains the
        // full boot sequence including events emitted before the sink opened.
        // Best-effort: a short write on replay does not abort boot.
        ring.for_each(|entry| {
            let _ = write_all_to_fd(fd, entry.line.as_slice());
        });

        // ── Subscribe to the bus ───────────────────────────────────────────────
        bus.subscribe(sink_subscriber_callback)
            .map_err(|e| match e {
                SubscribeError::Capacity | SubscribeError::Alloc => SinkOpenError::Subscribe,
            })?;

        Ok(())
    }

    /// Sets the headless flag (default `true`).
    ///
    /// When `headless = true`, rendered lines continue to go to stderr after
    /// the sink opens (server posture). When `false`, stderr output stops after
    /// open (TUI posture). TUI-suppression policy lands with the client
    /// platform work (#753).
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// // no_run: requires the arch runtime — use the kernel-selftest bin.
    /// use reovim_kernel::log::sink::FileSink;
    /// let sink = FileSink::new(b"/tmp/reovim.log\0");
    /// sink.set_headless(false);
    /// ```
    pub fn set_headless(&self, headless: bool) {
        SINK.lock().headless = headless;
    }
}

impl Drop for FileSink {
    fn drop(&mut self) {
        // Close the fd if still open. Ignore close errors — we are in Drop
        // and cannot propagate (best-effort teardown).
        let maybe_fd = SINK.lock().fd.take();
        if let Some(fd) = maybe_fd {
            let _ = close(fd);
        }
    }
}

// ── Subscriber callback ───────────────────────────────────────────────────────

/// The DS12 subscriber callback for the file sink.
///
/// Reads the process-global `SINK` state, renders the event as a LOG2 line,
/// and writes to the file fd and/or stderr. On a write failure: marks the sink
/// closed (LOG7 non-blocking), drops the lock, then emits one `log.sink.fail`
/// event via the stored bus clone. Re-entrant emission is safe: CC6
/// clone-then-invoke holds no lock across callbacks, and the now-closed sink
/// ignores the re-entrant event.
fn sink_subscriber_callback(event: &DS12Event) {
    // Render first (before acquiring the lock) so lock hold-time is minimal.
    let subsystem = kernel_subsystem_from_event(event.event);
    let input = RenderInput {
        ts_nanos: event.ts_nanos,
        emitter_pkg: "kernel",
        emitter_addr: EmitterAddress::Kernel { subsystem },
        instance: InstanceAddress::None,
        message: event.event,
        level: event.level,
    };
    let Ok(line) = render_line(&input) else {
        return; // Render failed (OOM); drop silently.
    };
    let bytes = line.as_slice();

    // Acquire the sink lock only to read fd/flags and potentially mark closed.
    let maybe_bus = {
        let mut st = SINK.lock();

        // Sink not yet open or already closed — nothing to write.
        if st.closed {
            return;
        }
        let Some(fd) = st.fd else {
            return;
        };

        if write_all_to_fd(fd, bytes) {
            None // write succeeded — no failure to report
        } else {
            // Mark closed first (stops future writes), then extract the bus
            // clone so we can emit outside the lock.
            st.closed = true;
            let _ = close(fd);
            st.fd = None;
            st.bus.take() // Some(Shared<DS12EventBus>) if available
        }
    };
    // Lock is released here; safe to emit without re-entrancy deadlock.

    if let Some(bus) = maybe_bus {
        emit_sink_fail(&bus, 0);
    }
}

// ── Early stderr gate (LOG8) ──────────────────────────────────────────────────

/// Writes a rendered LOG2 line to fd 2 (stderr) when appropriate (LOG8).
///
/// Called from `LogRing::push_event` after each append (one-way ring→sink
/// gate). Writes to fd 2 when:
/// - the sink has not yet been opened (`fd` is `None`), OR
/// - the headless flag is `true` (server posture, stderr mirrors log file).
///
/// Does nothing when the sink is open and headless is `false` (TUI posture).
///
/// ```rust,no_run
/// // no_run: requires the arch runtime — use the kernel-selftest bin.
/// use reovim_kernel::log::sink::stderr_echo;
/// stderr_echo(b"[    0.000001] kernel boot: test\n");
/// ```
pub fn stderr_echo(line: &[u8]) {
    let st = SINK.lock();
    // Write to stderr when sink is not open yet (early boot) or headless.
    if st.fd.is_none() || st.headless {
        drop(st); // release lock before the write syscall
        let _ = write_all_to_fd(2, line);
    }
}

// ── Sink failure event ────────────────────────────────────────────────────────

/// Emits a `log.sink.fail` event on `bus` with `errno` as the error code.
///
/// Used by both the open-failure path and the write-failure path. The event
/// level is Error (6.3 §2.4). `BootStageFields` is reused here: stage is 0
/// (not applicable) and `error_code` carries the errno. This is the minimum
/// generalization needed; a dedicated `SinkFailFields` type is deferred until
/// a second structured-field consumer exists (rule of three).
fn emit_sink_fail(bus: &DS12EventBus, errno: i32) {
    bus.emit(&DS12Event {
        ts_nanos: 0, // no BootClock available at callback time; 0 is the sentinel
        level: reovim_uapi_abi::error::LogLevel::Error,
        event: EVT_LOG_SINK_FAIL,
        fields: BootStageFields {
            stage: 0,
            error_code: Some(errno),
        },
    });
}

// ── Write helper ──────────────────────────────────────────────────────────────

/// Writes all of `buf` to `fd`, looping over short writes.
///
/// Returns `true` if all bytes were written, `false` on the first error or
/// zero-byte stall. Best-effort, matching LOG7's non-blocking contract.
/// Never panics.
#[must_use]
fn write_all_to_fd(fd: i32, buf: &[u8]) -> bool {
    let mut off = 0;
    while off < buf.len() {
        match write(fd, &buf[off..]) {
            Ok(0) | Err(_) => return false,
            Ok(n) => off += n,
        }
    }
    true
}

// ── Test helpers (selftest-gated) ─────────────────────────────────────────────

/// Resets the process-global sink to the initial state.
///
/// Must be called at the start of every sink test to ensure tests do not
/// share state. If a previous test left an fd open it is closed here.
///
/// Only available under the `selftest` feature (enforced at compile time to
/// prevent accidental production use).
///
/// ```rust,no_run
/// // no_run: selftest-gated — not available in production builds.
/// ```
#[cfg(feature = "selftest")]
pub fn reset_for_test() {
    let mut st = SINK.lock();
    if let Some(fd) = st.fd.take() {
        let _ = close(fd);
    }
    st.headless = true;
    st.closed = false;
    st.bus = None;
}

/// Injects a pre-opened (but immediately invalidated) fd into the sink state
/// for write-failure testing, and registers the sink subscriber callback on
/// `bus`.
///
/// Callers open a file, store its fd in the sink via this function, then close
/// the fd externally so the next write fails. The test can then assert that
/// the sink transitions to the closed state and emits `log.sink.fail`.
///
/// Registering the subscriber on `bus` is necessary because `inject_fd_for_test`
/// bypasses the normal `open_and_subscribe` path that would call `bus.subscribe`.
///
/// Only available under the `selftest` feature.
///
/// ```rust,no_run
/// // no_run: selftest-gated — not available in production builds.
/// ```
#[cfg(feature = "selftest")]
pub fn inject_fd_for_test(fd: i32, bus: Shared<DS12EventBus>) {
    {
        let mut st = SINK.lock();
        st.fd = Some(fd);
        st.closed = false;
        st.bus = Some(Shared::clone(&bus));
    }
    // Register the subscriber callback so writes through the bus reach the sink.
    // Ignore errors (capacity/alloc): the test is single-subscriber and the bus
    // starts fresh.
    let _ = bus.subscribe(sink_subscriber_callback);
}

// ── SinkOpenError ─────────────────────────────────────────────────────────────

/// Why [`FileSink::open_and_subscribe`] failed.
///
/// # Example
///
/// ```rust
/// use reovim_kernel::log::sink::SinkOpenError;
///
/// let e = SinkOpenError::Subscribe;
/// assert!(matches!(e, SinkOpenError::Subscribe));
/// ```
#[derive(Debug, Clone, Copy)]
pub enum SinkOpenError {
    /// The `openat` syscall failed.
    Open(reovim_arch::sys::Errno),
    /// The bus subscriber capacity was exhausted.
    Subscribe,
}

// L12 layout: tests in sibling sink_tests.rs, declared in log/mod.rs.
