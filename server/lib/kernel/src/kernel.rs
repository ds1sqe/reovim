//! `Kernel` — the steady-state root (2.1 §3, boot-core subset).
//!
//! `Kernel` is the shared in-process state root after the `Init`→`Kernel`
//! handoff. It can only be constructed by [`Init::boot`] (LF13): there is no
//! public constructor here.
//!
//! ## Boot-core subset (2.1 §3 note)
//!
//! The full §3 struct (sessions/buffers/windows maps, all registries,
//! correlation allocator) is the spec target this crate grows toward
//! monotonically. The boot-core realization carries the fields it does not yet
//! populate as explicit `()` placeholders so the struct shape mirrors the spec
//! and later features fill them in. The placeholders are not fabricated types
//! (rule of three — no registry type before its walking-skeleton consumer).
//!
//! | Field | Realized | Deferred to |
//! |---|---|---|
//! | `abi` | yes | — |
//! | `boot_anchor` | yes | — |
//! | `event_bus` | yes | — |
//! | `log_ring` | yes | — |
//! | `session` | yes (#797) | multi-session map (Phase 4) |
//! | `domain_router` | yes (#797, subset) | full router (Phase 4) |
//! | `config` | placeholder `()` | config service |
//! | `lockfile` | placeholder `()` | lockfile / library-root feature |
//! | `inventory` | placeholder `()` | module/driver discovery |
//! | `correlation_alloc` | placeholder `()` | correlation-ID allocator |
//! | `force_overrides` | placeholder `()` | config service |

use {
    reovim_arch::{ds::Shared, sync::RwLock},
    reovim_uapi_abi::AbiVersion,
};

use crate::{
    BootClock, event_bus::DS12EventBus, log::ring::LogRing, router::DomainRouter, session::Session,
};

// ── Current ABI version ──────────────────────────────────────────────────────

/// The ABI version this kernel instance exposes to cdylibs (6.2 §1).
///
/// Major = 1, minor = 0, patch = 0 for v0.16 boot-core baseline.
///
/// ```rust
/// use reovim_kernel::kernel::KERNEL_ABI_VERSION;
/// use reovim_uapi_abi::AbiVersion;
///
/// assert_eq!(KERNEL_ABI_VERSION.major, 1);
/// assert_eq!(KERNEL_ABI_VERSION.minor, 0);
/// ```
pub const KERNEL_ABI_VERSION: AbiVersion = AbiVersion {
    major: 1,
    minor: 0,
    patch: 0,
    pad: 0,
};

// ── KernelAbi ────────────────────────────────────────────────────────────────

/// The kernel's ABI identity (the `abi` field of `Kernel`).
///
/// Wraps the `AbiVersion` the kernel presents to cdylibs. Shared so the
/// loader can compare against loaded-cdylib vtable headers without taking
/// any other kernel lock (CC2 — no central kernel lock).
///
/// # Example
///
/// ```rust
/// use reovim_kernel::kernel::{KernelAbi, KERNEL_ABI_VERSION};
/// use reovim_uapi_abi::AbiVersion;
///
/// let abi = KernelAbi::new(KERNEL_ABI_VERSION);
/// assert_eq!(abi.version().major, 1);
/// ```
#[derive(Debug)]
pub struct KernelAbi {
    version: AbiVersion,
}

impl KernelAbi {
    /// Creates a `KernelAbi` with the given version.
    ///
    /// Called once in `Init::boot` (stage 7, the handoff). After this point
    /// the ABI version is immutable — the `Shared<KernelAbi>` is read-only.
    ///
    /// # Example
    ///
    /// ```rust
    /// use reovim_kernel::kernel::{KernelAbi, KERNEL_ABI_VERSION};
    ///
    /// let abi = KernelAbi::new(KERNEL_ABI_VERSION);
    /// assert_eq!(abi.version().minor, 0);
    /// ```
    #[must_use]
    pub const fn new(version: AbiVersion) -> Self {
        Self { version }
    }

    /// The ABI version this kernel exposes.
    ///
    /// # Example
    ///
    /// ```rust
    /// use reovim_kernel::kernel::{KernelAbi, KERNEL_ABI_VERSION};
    ///
    /// let abi = KernelAbi::new(KERNEL_ABI_VERSION);
    /// assert_eq!(abi.version(), KERNEL_ABI_VERSION);
    /// ```
    #[must_use]
    pub const fn version(&self) -> AbiVersion {
        self.version
    }
}

// `KernelAbi` holds only `AbiVersion` — a `#[repr(C)]` plain-integer struct
// with no raw pointers and no interior mutability — so `Send + Sync` are
// derived automatically by the compiler's auto-trait rules. No manual unsafe
// impl needed.

// ── Kernel ───────────────────────────────────────────────────────────────────

/// The steady-state kernel root (2.1 §3, boot-core subset, LF13).
///
/// Shared via `Shared<Kernel>` (the `arch::ds::Shared` no-std Arc analog).
/// Per-field locks (arch `RwLock`/`Mutex`) own concurrency; there is no central
/// `Mutex<Kernel>` (CC1/CC2). The boot-core subset carries the realized fields;
/// all deferred fields are typed as `()` placeholders until their features land.
///
/// **No public constructor** — only `Init::boot` can produce a `Kernel` (LF13).
/// The `pub(crate)` `new` function is intentionally crate-private so the
/// compile-fail probe for LF13 sees no public path.
///
/// # Example
///
/// ```rust,no_run
/// // no_run: requires the arch runtime — use the kernel-selftest bin.
/// use reovim_kernel::{Init, LauncherArgs};
///
/// let kernel = Init::new(LauncherArgs::default())
///     .boot()
///     .expect("boot succeeds");
/// assert_eq!(kernel.abi.version().major, 1);
/// ```
pub struct Kernel {
    // ── Realized fields ──────────────────────────────────────────────────────
    /// The ABI identity this kernel presents to cdylibs (6.2 §1).
    pub abi: Shared<KernelAbi>,

    /// Boot-clock anchor transferred from `Init` at the handoff (7.5 §4).
    /// Timestamps and ring content are continuous across the boundary.
    pub boot_anchor: BootClock,

    /// DS12 event bus — clone-then-invoke fan-out (CC6), OBS1 boot families.
    ///
    /// The log ring is wired as the always-present built-in subscriber (LOG1).
    /// The bus is `Shared<DS12EventBus>` so the log ring can hold a reference
    /// without borrowing `Kernel`.
    pub event_bus: Shared<DS12EventBus>,

    /// The kernel log ring — allocated in boot stage 0, before any event
    /// (LOG6). A second `Shared<LogRing>` clone lives in `DS12EventBus::builtin_ring`
    /// (the bus's built-in LOG1 slot). Keeping both clones alive ensures the
    /// refcount stays ≥2 for the process lifetime. The file sink attaches
    /// on-demand via `FileSink::open_and_subscribe`.
    pub log_ring: Shared<LogRing>,

    /// The single session for the walking skeleton (#797).
    ///
    /// Full multi-session map (`HashMap<SessionId, Session>`) is the spec
    /// target (§2.1 §3); it is grown when the second session consumer arrives
    /// (Phase 4). The `Shared<Session>` allows the server runtime to hold a
    /// reference without borrowing `Kernel`.
    pub session: Shared<Session>,

    /// `DomainRouter` SUBSET — `intern_named`/`name_of` + one handler row +
    /// one projector row (§4.1 subset note, #797).
    ///
    /// Wrapped in `RwLock` so the runtime can dispatch under read and the
    /// composition root can register under write. No interior mut inside
    /// `DomainRouter` itself.
    pub domain_router: Shared<RwLock<DomainRouter>>,

    // ── Deferred placeholders (filled by their respective features) ──────────
    // Named as `()` rather than fabricated registry types: rule of three —
    // no type is extracted before its walking-skeleton consumer demands it.
    // Each placeholder is a single `()` so the struct compiles and the field
    // positions are reserved.
    /// Effective config (config service). Placeholder.
    pub config: (),

    /// File-system lockfile (lockfile / library-root feature). Placeholder.
    pub lockfile: (),

    /// Loaded-cdylib inventory (module/driver discovery). Placeholder.
    pub inventory: (),

    /// Correlation-ID allocator (correlation-ID allocator feature). Placeholder.
    pub correlation_alloc: (),

    /// Force-override config layer (config service). Placeholder.
    pub force_overrides: (),
}

impl Kernel {
    /// Constructs a `Kernel` from the fields moved out of `Init` at the
    /// handoff (stage 7).
    ///
    /// **Crate-private**: only `Init::boot` calls this. External code has no
    /// `Kernel` constructor — that is the LF13 invariant.
    ///
    /// # Example (crate-internal)
    ///
    /// ```rust,no_run
    /// // no_run: pub(crate) — only Init::boot is the intended caller (LF13).
    /// ```
    pub(crate) const fn new(
        abi: Shared<KernelAbi>,
        boot_anchor: BootClock,
        event_bus: Shared<DS12EventBus>,
        log_ring: Shared<LogRing>,
        session: Shared<Session>,
        domain_router: Shared<RwLock<DomainRouter>>,
    ) -> Self {
        Self {
            abi,
            boot_anchor,
            event_bus,
            log_ring,
            session,
            domain_router,
            config: (),
            lockfile: (),
            inventory: (),
            correlation_alloc: (),
            force_overrides: (),
        }
    }
}

impl Kernel {
    /// Registers the `OnRawInput` handler and `Render` projector for the named
    /// text Domain, returning the interned `DomainId`.
    ///
    /// Called by the composition root (or by kernel-selftest tests) AFTER
    /// `Init::boot` to wire the text Domain into the kernel. The kernel does not
    /// depend on `ext/server/domain/text` (core/ext boundary); the caller
    /// provides the `'static` trait objects.
    ///
    /// # Errors
    ///
    /// Returns `Err("alloc")` if `DomainRouter::intern_named` fails.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch runtime + a concrete handler/projector.
    /// ```
    pub fn register_domain(
        &self,
        name: &str,
        handler: &'static dyn crate::router::OnRawInputHandler,
        projector: &'static dyn crate::router::RenderProjector,
    ) -> Result<crate::router::DomainId, &'static str> {
        let mut router = self.domain_router.write();
        let id = router.intern_named(name)?;
        router.register_handler(id, handler);
        router.register_projector(id, projector);
        Ok(id)
    }

    /// Replaces the session state with a real `SessionState` tied to the
    /// registered Domain.
    ///
    /// Called by the composition root (or tests) after `register_domain` to
    /// wire the session to the correct `DomainId`. Overwrites the placeholder
    /// installed by `Init::boot`.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch runtime + registered domain.
    /// ```
    pub fn setup_session(&self, state: crate::session::SessionState) {
        let mut guard = self.session.state.lock();
        *guard = state;
    }

    /// Dispatches a raw-input byte sequence through the session's focus chain,
    /// returning the resulting `Projection`.
    ///
    /// Follows the CC14 shape: snapshot under state lock, drop lock, invoke
    /// handler, re-lock to apply, snapshot for projector, invoke projector.
    ///
    /// # Errors
    ///
    /// Returns `Err(&'static str)` if the router has no handler/projector for
    /// the session's domain, or if an allocation fails.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch runtime + registered domain + session.
    /// ```
    pub fn dispatch_input(
        &self,
        input: &[u8],
    ) -> Result<crate::projection::Projection, &'static str> {
        let router = self.domain_router.read();
        self.session.dispatch_input(input, &router)
    }
}

// `Kernel` field Send+Sync analysis:
// - `Shared<KernelAbi>`: plain-integer data → auto Send+Sync.
// - `BootClock`: `Copy` integer-only data → auto Send+Sync.
// - `Shared<DS12EventBus>`: holds `RwLock<Seq<fn>>` + `Option<Shared<LogRing>>` →
//   auto Send+Sync.
// - `Shared<LogRing>`: `Mutex<Ring<LogEntry>>` + `usize` → auto Send+Sync.
// - `Shared<Session>`: `Session` holds `Mutex<SessionState>` + `SessionId` (Copy).
//   `SessionState` contains `Bytes` (owns raw allocation; `Bytes: Send` because
//   it is single-owner) + primitive fields. `Mutex<T>` requires `T: Send`
//   (not `T: Sync`). `Bytes` is `Send`. So `Mutex<SessionState>: Send + Sync`.
//   `Session: Send + Sync`. `Shared<Session>: Send + Sync`.
// - `Shared<RwLock<DomainRouter>>`: `DomainRouter` holds `Map<K, V>` where
//   values are `DomainId`/`Bytes` (both `Send`). `handler`/`projector` are
//   `&'static dyn … + Send + Sync`. `RwLock<DomainRouter>: Send + Sync` (same
//   analysis as `Mutex` — `DomainRouter: Send`). `Shared<RwLock<DomainRouter>>:
//   Send + Sync`.
// - `()` placeholders: trivially Send+Sync.
//
// The compiler derives Send+Sync automatically; no manual `unsafe impl` needed.

// L12 layout: tests in sibling kernel_tests.rs, declared in lib.rs.
