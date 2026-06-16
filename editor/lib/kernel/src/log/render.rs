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

// L12 layout: tests in sibling render_tests.rs, declared in log/mod.rs.
