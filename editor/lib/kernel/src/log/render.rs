//! LOG2 canonical line renderer (9.5 §2).
//!
//! Produces the `lib_ds::Bytes` that go into every ring entry and every
//! sink line. This is the **one** rendering (LOG1: logging is a rendering of
//! DS12; there is no second pipeline).
//!
//! ## Grammar (9.5 §2)
//!
//! ```text
//! line := "[" ts "]" SP emitter SP address [SP instance] ":" SP message "\n"
//! ts   := seconds "." micros
//!         ; seconds right-aligned min width 5, micros zero-padded width 6
//! ```
//!
//! ## Emitter forms
//!
//! - Kernel event (§3): bare subsystem name (`init`, `dispatch`, …). A kernel
//!   address never contains `/`.
//! - Cdylib event (§3): `kind/CdylibId.vtable_index` (e.g. `module/7.0`).
//!
//! ## Byte determinism
//!
//! Given the same timestamp, emitter, and event fields, `render_line` always
//! produces byte-identical output. No locale, no OS call, no RNG.
//!
//! ## Instance address (§4)
//!
//! The `instance` token is optional; it is rendered only when at least one
//! scoping field is present. See [`InstanceAddress`] for the exhaustive table.

use core::fmt::Write as _;

use reovim_lib_ds::{Bytes, BytesWriter};

use crate::event_bus::{DS12Event, EVT_BOOT_STAGE_FAIL, EVT_BOOT_STAGE_OK, EVT_BOOT_STAGE_START};

// ── Emitter address forms ────────────────────────────────────────────────────

/// The emitter address stamped on a LOG2 line (9.5 §3).
///
/// This is always derived by the kernel from dispatch context, never
/// caller-supplied (LOG3).
///
/// # Example
///
/// ```rust
/// use reovim_kernel::log::render::EmitterAddress;
///
/// let k = EmitterAddress::Kernel { subsystem: "init" };
/// let c = EmitterAddress::Cdylib { kind: "module", id: 7, vtable: 0 };
/// assert!(matches!(k, EmitterAddress::Kernel { .. }));
/// assert!(matches!(c, EmitterAddress::Cdylib { .. }));
/// ```
#[derive(Debug, Clone, Copy)]
pub enum EmitterAddress {
    /// Kernel-owned subsystem — bare name (`init`, `dispatch`, `config`, …).
    ///
    /// A kernel address never contains `/` (9.5 §3 disambiguation rule).
    Kernel {
        /// The kernel subsystem name (e.g. `"init"`, `"dispatch"`).
        subsystem: &'static str,
    },
    /// Cdylib emitter — `kind/CdylibId.vtable` form.
    Cdylib {
        /// The `ManifestKind` address string (e.g. `"module"`, `"driver"`).
        kind: &'static str,
        /// Interned `CdylibId` (non-zero `u32`, monotone per-process).
        id: u32,
        /// 0-based vtable index in manifest declaration order.
        vtable: u32,
    },
}

// ── Instance address (§4 exhaustive table) ──────────────────────────────────

/// Optional instance address scoping the event to runtime state (9.5 §4).
///
/// Derived from DS12 common scoping fields (9.4 §5). The table from the spec
/// is exactly the variant set here; the grammar is additive for future ABI
/// major versions.
///
/// # Example
///
/// ```rust
/// use reovim_kernel::log::render::InstanceAddress;
///
/// assert_eq!(InstanceAddress::None.is_present(), false);
/// assert_eq!(InstanceAddress::Session { session: "main" }.is_present(), true);
/// ```
#[derive(Debug, Clone, Copy)]
pub enum InstanceAddress {
    /// No scoping fields — the instance token is omitted (9.5 §4).
    None,
    /// Session only → `main`.
    Session {
        /// Session name.
        session: &'static str,
    },
    /// Session + buffer → `main/b3`.
    SessionBuffer {
        /// Session name.
        session: &'static str,
        /// 0-based buffer index.
        buffer: u32,
    },
    /// Session + buffer + window → `main/b3.w1`.
    SessionBufferWindow {
        /// Session name.
        session: &'static str,
        /// 0-based buffer index.
        buffer: u32,
        /// 0-based window index.
        window: u32,
    },
    /// Client only → `c2`.
    Client {
        /// Client ID.
        id: u32,
    },
    /// Stream only → `s17`.
    Stream {
        /// Stream ID.
        id: u32,
    },
}

impl InstanceAddress {
    /// Whether this instance address contributes a token to the rendered line.
    ///
    /// ```rust
    /// use reovim_kernel::log::render::InstanceAddress;
    ///
    /// assert!(!InstanceAddress::None.is_present());
    /// assert!(InstanceAddress::Client { id: 1 }.is_present());
    /// ```
    #[must_use]
    pub const fn is_present(&self) -> bool {
        !matches!(self, Self::None)
    }
}

// ── RenderInput ──────────────────────────────────────────────────────────────

/// All inputs to one `render_line` call.
///
/// Collecting them in a struct keeps the function signature manageable and
/// makes it easy to construct goldens in tests.
///
/// # Example
///
/// ```rust
/// use reovim_kernel::log::render::{EmitterAddress, InstanceAddress, RenderInput};
/// use reovim_uapi_abi::error::LogLevel;
///
/// let input = RenderInput {
///     ts_nanos: 2_413_000,
///     emitter_pkg: "kernel",
///     emitter_addr: EmitterAddress::Kernel { subsystem: "init" },
///     instance: InstanceAddress::None,
///     message: "boot.stage.ok stage=2 name=config",
///     level: LogLevel::Info,
/// };
/// assert_eq!(input.ts_nanos, 2_413_000);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct RenderInput<'a> {
    /// Nanoseconds since the boot anchor (drives `ts` in the LOG2 line).
    pub ts_nanos: u64,
    /// Package/emitter name printed after the timestamp (LOG2 annotated example).
    ///
    /// For kernel events this is `"kernel"`; for cdylib events it is the
    /// cdylib's package name from the manifest.
    pub emitter_pkg: &'a str,
    /// Where in the loaded topology the event came from (9.5 §3).
    pub emitter_addr: EmitterAddress,
    /// Optional runtime-state scope (9.5 §4).
    pub instance: InstanceAddress,
    /// The rendered event string (9.5 §5).
    ///
    /// Embedded `\n` characters are escaped as the literal two-character
    /// sequence `\n` before writing (9.5 §5, §2 "Embedded newlines … escaped
    /// as `\n`").
    pub message: &'a str,
    /// Log level (carried in the ring entry alongside the rendered bytes).
    pub level: reovim_uapi_abi::error::LogLevel,
}

// ── Render error ─────────────────────────────────────────────────────────────

/// Why [`render_line`] failed.
///
/// The only failure mode is an allocation failure in the backing `Bytes`.
///
/// # Example
///
/// ```rust
/// use reovim_kernel::log::render::RenderError;
///
/// let e = RenderError::Alloc;
/// assert!(matches!(e, RenderError::Alloc));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderError {
    /// The `Bytes` backing allocation was refused.
    Alloc,
}

// ── Core renderer ─────────────────────────────────────────────────────────────

/// Renders a DS12 event into a LOG2 canonical line.
///
/// The line is byte-deterministic: same inputs → same bytes, every call.
/// The trailing `\n` is always present. Embedded newlines in `message` are
/// escaped as the two-character sequence `\n`.
///
/// ## Timestamp format
///
/// `seconds` is right-aligned with a minimum width of 5 (space-padded);
/// `micros` is zero-padded to exactly 6 digits. From spec §2 worked example:
///
/// ```text
/// [    0.002413] kernel init: boot.stage.ok stage=2 name=config
/// ```
///
/// `2413 µs` → seconds = `0`, micros = `002413`.
///
/// # Errors
///
/// Returns [`RenderError::Alloc`] if the backing [`Bytes`] cannot grow.
///
/// # Example
///
/// ```no_run
/// use reovim_kernel::log::render::{EmitterAddress, InstanceAddress, RenderInput, render_line};
/// use reovim_uapi_abi::error::LogLevel;
///
/// let input = RenderInput {
///     ts_nanos: 2_413_000,   // 2413 µs
///     emitter_pkg: "kernel",
///     emitter_addr: EmitterAddress::Kernel { subsystem: "init" },
///     instance: InstanceAddress::None,
///     message: "boot.stage.ok stage=2 name=config",
///     level: LogLevel::Info,
/// };
/// let bytes = render_line(&input).expect("render must succeed");
/// let s = core::str::from_utf8(bytes.as_slice()).expect("UTF-8");
/// assert!(s.starts_with("[    0.002413]"), "timestamp format: {s:?}");
/// assert!(s.ends_with('\n'));
/// ```
pub fn render_line(input: &RenderInput<'_>) -> Result<Bytes, RenderError> {
    let mut buf = Bytes::new();
    let mut w = BytesWriter::new(&mut buf);

    // ── Timestamp ────────────────────────────────────────────────────────────
    //
    // ts = seconds "." micros
    // seconds: right-aligned, min width 5, space-padded.
    // micros:  zero-padded, width 6.
    //
    // Spec example: "    0.002413" (4 spaces + "0" + "." + "002413")
    let total_micros = input.ts_nanos / 1_000;
    let seconds = total_micros / 1_000_000;
    let micros = total_micros % 1_000_000;

    write!(w, "[{seconds:>5}.{micros:06}]").map_err(|_| RenderError::Alloc)?;

    // ── Emitter package name ──────────────────────────────────────────────────
    write!(w, " {}", input.emitter_pkg).map_err(|_| RenderError::Alloc)?;

    // ── Emitter address ───────────────────────────────────────────────────────
    match input.emitter_addr {
        EmitterAddress::Kernel { subsystem } => {
            write!(w, " {subsystem}").map_err(|_| RenderError::Alloc)?;
        }
        EmitterAddress::Cdylib { kind, id, vtable } => {
            write!(w, " {kind}/{id}.{vtable}").map_err(|_| RenderError::Alloc)?;
        }
    }

    // ── Optional instance address ─────────────────────────────────────────────
    match input.instance {
        InstanceAddress::None => {}
        InstanceAddress::Session { session } => {
            write!(w, " {session}").map_err(|_| RenderError::Alloc)?;
        }
        InstanceAddress::SessionBuffer { session, buffer } => {
            write!(w, " {session}/b{buffer}").map_err(|_| RenderError::Alloc)?;
        }
        InstanceAddress::SessionBufferWindow {
            session,
            buffer,
            window,
        } => {
            write!(w, " {session}/b{buffer}.w{window}").map_err(|_| RenderError::Alloc)?;
        }
        InstanceAddress::Client { id } => {
            write!(w, " c{id}").map_err(|_| RenderError::Alloc)?;
        }
        InstanceAddress::Stream { id } => {
            write!(w, " s{id}").map_err(|_| RenderError::Alloc)?;
        }
    }

    // ── Separator ─────────────────────────────────────────────────────────────
    w.write_str(": ").map_err(|_| RenderError::Alloc)?;

    // ── Message with \n escaping ──────────────────────────────────────────────
    //
    // Embedded newlines in `message` are rendered as the two-character
    // literal sequence `\n` (9.5 §2, §5).
    write_escaped(input.message, &mut w).map_err(|_| RenderError::Alloc)?;

    // ── Trailing newline ──────────────────────────────────────────────────────
    w.write_char('\n').map_err(|_| RenderError::Alloc)?;

    Ok(buf)
}

/// Writes `s` to `w`, escaping embedded `\n` as the literal two-character
/// sequence `\n` (9.5 §2).
fn write_escaped(s: &str, w: &mut BytesWriter<'_>) -> core::fmt::Result {
    // Walk through the string; for each segment before a `\n` write the
    // segment then write the escape. After the last segment write the tail.
    let mut rest = s;
    loop {
        // Find the next embedded newline.
        if let Some(pos) = rest.bytes().position(|b| b == b'\n') {
            // Write the segment before the newline.
            w.write_str(&rest[..pos])?;
            // Write the two-character escape sequence.
            w.write_str("\\n")?;
            rest = &rest[pos + 1..];
        } else {
            // No more newlines — write the remainder and stop.
            w.write_str(rest)?;
            break;
        }
    }
    Ok(())
}

// ── Boot-stage message enrichment (2.2 stage map) ────────────────────────────

/// The last boot stage `run_boot_stages` emits (stages 1..=7); the denominator
/// in a rendered "stage N/7" message.
const LAST_BOOT_STAGE: u8 = 7;

/// Human-readable name for boot stage `n` (2.2 stage map). An out-of-range
/// stage renders as `"stage"` so the message stays well-formed.
pub(crate) const fn boot_stage_name(n: u8) -> &'static str {
    match n {
        0 => "init",
        1 => "host config",
        2 => "shell config",
        3 => "library root",
        4 => "module load",
        5 => "driver load",
        6 => "runtime",
        7 => "handoff",
        _ => "stage",
    }
}

/// A [`core::fmt::Write`] sink over a fixed `&mut [u8]`, for building a short
/// message without allocation. A write that would overflow the buffer fails,
/// so the message is rejected whole rather than clipped mid-write.
///
/// Shared with the `diagnostics` module (the boot banner + health probes), which
/// builds its metric lines the same no-alloc way.
pub(crate) struct FixedWriter<'a> {
    buf: &'a mut [u8],
    len: usize,
}

impl<'a> FixedWriter<'a> {
    /// Wraps `buf` with the write cursor at the start.
    pub(crate) const fn new(buf: &'a mut [u8]) -> Self {
        Self { buf, len: 0 }
    }

    /// Consumes the writer and returns the written prefix as a `&str`.
    ///
    /// Every successful [`write_str`](core::fmt::Write::write_str) placed a whole
    /// `&str` into the buffer, so the prefix is always valid UTF-8; an impossible
    /// decode failure yields an empty string rather than panicking.
    pub(crate) fn finish(self) -> &'a str {
        core::str::from_utf8(&self.buf[..self.len]).unwrap_or("")
    }
}

impl core::fmt::Write for FixedWriter<'_> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let end = self.len.checked_add(s.len()).ok_or(core::fmt::Error)?;
        let dst = self.buf.get_mut(self.len..end).ok_or(core::fmt::Error)?;
        dst.copy_from_slice(s.as_bytes());
        self.len = end;
        Ok(())
    }
}

/// Builds a human-readable boot-stage message ("stage 2/7 ok: shell config")
/// into `buf` and returns it as a `&str`. The verb comes from the OBS1 event
/// id, the name from [`boot_stage_name`].
///
/// Returns `None` when `event` is not a boot-stage event — the caller then
/// renders the bare event id — or when the formatted message would not fit
/// `buf` (it falls back to the bare id the same way).
pub(crate) fn boot_stage_message<'a>(buf: &'a mut [u8], event: &str, stage: u8) -> Option<&'a str> {
    let verb = match event {
        EVT_BOOT_STAGE_START => "starting",
        EVT_BOOT_STAGE_OK => "ok",
        EVT_BOOT_STAGE_FAIL => "failed",
        _ => return None,
    };
    let len = {
        let mut w = FixedWriter {
            buf: &mut *buf,
            len: 0,
        };
        write!(w, "stage {stage}/{LAST_BOOT_STAGE} {verb}: {}", boot_stage_name(stage)).ok()?;
        w.len
    };
    core::str::from_utf8(&buf[..len]).ok()
}

/// Maps a dotted OBS1 event name to its kernel subsystem label (9.5 §3).
///
/// The first dotted segment is the family; a static label is returned for the
/// families the boot-core crate emits. Unknown families fall back to `kernel`.
fn kernel_subsystem_from_event(event: &str) -> &'static str {
    if event.starts_with("boot.") {
        "boot"
    } else if event.starts_with("log.") {
        "log"
    } else if event.starts_with("health.") {
        "health"
    } else {
        "kernel"
    }
}

/// Renders a kernel DS12 `event` into its canonical LOG2 line bytes.
///
/// This is the ONE rendering both the ring (`LogRing::push_event`) and the file
/// sink use, so the two can never diverge (LOG1: one renderer, one mechanism).
/// Boot-stage events get the human-readable message from [`boot_stage_message`];
/// `health.*` events (the boot banner + probes) carry their already-formatted
/// metric after the family prefix, so the prefix is stripped for the message
/// while still selecting the `health` subsystem; every other event renders the
/// bare OBS1 event id.
///
/// # Errors
///
/// Returns [`RenderError::Alloc`] if the backing [`Bytes`] cannot grow.
pub(crate) fn render_event(event: &DS12Event) -> Result<Bytes, RenderError> {
    // The buffer holds the formatted boot-stage message for the duration of the
    // `render_line` call below; non-boot events use the bare event id, minus the
    // `health.` family prefix for health events (their metric is the remainder).
    let mut stage_buf = [0u8; 48];
    let message = boot_stage_message(&mut stage_buf, event.event, event.fields.stage)
        .or_else(|| event.event.strip_prefix("health."))
        .unwrap_or(event.event);
    let input = RenderInput {
        ts_nanos: event.ts_nanos,
        emitter_pkg: "kernel",
        emitter_addr: EmitterAddress::Kernel {
            subsystem: kernel_subsystem_from_event(event.event),
        },
        instance: InstanceAddress::None,
        message,
        level: event.level,
    };
    render_line(&input)
}

// L12 layout: tests in sibling render_tests.rs, declared in log/mod.rs.
