//! `EditorCore` — the steady-state root (2.1 §3, boot-core subset).
//!
//! `EditorCore` is the shared in-process state root after the `EditorInit`→`EditorCore`
//! handoff. It can only be constructed by [`EditorInit::boot`] (LF13): there is no
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
//! | `session` | yes (#797 compatibility) | retained until server runtime moves to `sessions` |
//! | `sessions` | yes (#798 Phase 3 subset) | full layout operations |
//! | `domain_router` | yes (#797, subset) | full router (Phase 4) |
//! | `state` | yes (#798, Phase 3 subset) | full state hostapi |
//! | `config` | placeholder `()` | config service |
//! | `lockfile` | placeholder `()` | lockfile / library-root feature |
//! | `inventory` | placeholder `()` | module/driver discovery |
//! | `correlation_alloc` | placeholder `()` | correlation-ID allocator |
//! | `force_overrides` | placeholder `()` | config service |

use {
    reovim_lib_ds::{Bytes, RwLock, Shared},
    reovim_subsys_domain::{
        carrier::{CarrierStatus, CursorCarrier, PositionCarrier},
        id::{
            BufferId, CdylibId as StateCdylibId, ClientId, RegisterId, ServiceKey, ServiceLeaseId,
            StreamId, WindowId,
        },
        routing::RegistrationPhase,
        service::{ServiceAccessError, ServiceBorrow, ServiceDescriptorMeta, ServiceLease},
        state::{EditOrigin, RegisterScope, UndoGroupId, ViewSlotFlags, ViewSlotKey},
        stream::{BackpressureCounters, StreamControlClass, StreamControlOp, StreamScheme},
    },
    reovim_uapi::abi::{AbiVersion, error::LogLevel},
};

use crate::{
    BootClock,
    event_bus::{BootStageFields, DS12Event, DS12EventBus, EVT_CARRIER_VALIDATION_ERROR},
    log::ring::LogRing,
    router::{CdylibId, DomainApiVersion, DomainManifest, DomainRouter, HandlerId, ProjectorId},
    session::{DispatchRoutes, Session, SessionId, SessionTable},
    state::{ServiceCallToken, StateSubstrate, ViewSlotRecord},
};

const STATIC_OWNER: CdylibId = CdylibId::new(core::num::NonZeroU32::MIN);
const STATIC_OWNER_ORDER: u32 = 0;

// ── Current ABI version ──────────────────────────────────────────────────────

/// The ABI version this editor-core instance exposes to cdylibs (6.2 §1).
///
/// Major = 1, minor = 0, patch = 0 for v0.16 boot-core baseline.
///
/// ```rust
/// use reovim_editor_core::root::EDITOR_CORE_ABI_VERSION;
/// use reovim_uapi::abi::AbiVersion;
///
/// assert_eq!(EDITOR_CORE_ABI_VERSION.major, 1);
/// assert_eq!(EDITOR_CORE_ABI_VERSION.minor, 0);
/// ```
pub const EDITOR_CORE_ABI_VERSION: AbiVersion = AbiVersion {
    major: 1,
    minor: 0,
    patch: 0,
    pad: 0,
};

// ── EditorCoreAbi ────────────────────────────────────────────────────────────────

/// The editor core's ABI identity (the `abi` field of `EditorCore`).
///
/// Wraps the `AbiVersion` the editor core presents to cdylibs. Shared so the
/// loader can compare against loaded-cdylib vtable headers without taking
/// any other editor-core lock (CC2 — no central editor-core lock).
///
/// # Example
///
/// ```rust
/// use reovim_editor_core::root::{EditorCoreAbi, EDITOR_CORE_ABI_VERSION};
/// use reovim_uapi::abi::AbiVersion;
///
/// let abi = EditorCoreAbi::new(EDITOR_CORE_ABI_VERSION);
/// assert_eq!(abi.version().major, 1);
/// ```
#[derive(Debug)]
pub struct EditorCoreAbi {
    version: AbiVersion,
}

impl EditorCoreAbi {
    /// Creates a `EditorCoreAbi` with the given version.
    ///
    /// Called once in `EditorInit::boot` (stage 7, the handoff). After this point
    /// the ABI version is immutable — the `Shared<EditorCoreAbi>` is read-only.
    ///
    /// # Example
    ///
    /// ```rust
    /// use reovim_editor_core::root::{EditorCoreAbi, EDITOR_CORE_ABI_VERSION};
    ///
    /// let abi = EditorCoreAbi::new(EDITOR_CORE_ABI_VERSION);
    /// assert_eq!(abi.version().minor, 0);
    /// ```
    #[must_use]
    pub const fn new(version: AbiVersion) -> Self {
        Self { version }
    }

    /// The ABI version this editor core exposes.
    ///
    /// # Example
    ///
    /// ```rust
    /// use reovim_editor_core::root::{EditorCoreAbi, EDITOR_CORE_ABI_VERSION};
    ///
    /// let abi = EditorCoreAbi::new(EDITOR_CORE_ABI_VERSION);
    /// assert_eq!(abi.version(), EDITOR_CORE_ABI_VERSION);
    /// ```
    #[must_use]
    pub const fn version(&self) -> AbiVersion {
        self.version
    }
}

// `EditorCoreAbi` holds only `AbiVersion` — a `#[repr(C)]` plain-integer struct
// with no raw pointers and no interior mutability — so `Send + Sync` are
// derived automatically by the compiler's auto-trait rules. No manual unsafe
// impl needed.

// ── EditorCore ───────────────────────────────────────────────────────────────────

/// The steady-state editor-core root (2.1 §3, boot-core subset, LF13).
///
/// Shared via `Shared<EditorCore>` (the `lib_ds::Shared` no-std Arc analog).
/// Per-field locks (arch `RwLock`/`Mutex`) own concurrency; there is no central
/// `Mutex<EditorCore>` (CC1/CC2). The boot-core subset carries the realized fields;
/// all deferred fields are typed as `()` placeholders until their features land.
///
/// **No public constructor** — only `EditorInit::boot` can produce a `EditorCore` (LF13).
/// The `pub(crate)` `new` function is intentionally crate-private so the
/// compile-fail probe for LF13 sees no public path.
///
/// # Example
///
/// ```rust,no_run
/// // no_run: requires the arch runtime — use the editor-core-selftest bin.
/// use reovim_editor_core::{EditorInit, LauncherArgs};
///
/// let editor_core = EditorInit::new(LauncherArgs::default())
///     .boot()
///     .expect("boot succeeds");
/// assert_eq!(editor_core.abi.version().major, 1);
/// ```
pub struct EditorCore {
    // ── Realized fields ──────────────────────────────────────────────────────
    /// The ABI identity this editor core presents to cdylibs (6.2 §1).
    pub abi: Shared<EditorCoreAbi>,

    /// Boot-clock anchor transferred from `EditorInit` at the handoff (7.5 §4).
    /// Timestamps and ring content are continuous across the boundary.
    pub boot_anchor: BootClock,

    /// DS12 event bus — clone-then-invoke fan-out (CC6), OBS1 boot families.
    ///
    /// The log ring is wired as the always-present built-in subscriber (LOG1).
    /// The bus is `Shared<DS12EventBus>` so the log ring can hold a reference
    /// without borrowing `EditorCore`.
    pub event_bus: Shared<DS12EventBus>,

    /// The editor-core log ring — allocated in boot stage 0, before any event
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
    /// reference without borrowing `EditorCore`.
    pub session: Shared<Session>,

    /// Phase 3 session table: multiple live sessions with per-session locks.
    pub sessions: Shared<RwLock<SessionTable>>,

    /// `DomainRouter` SUBSET — `intern_named`/`name_of` + one handler row +
    /// one projector row (§4.1 subset note, #797).
    ///
    /// Wrapped in `RwLock` so the runtime can dispatch under read and the
    /// composition root can register under write. No interior mut inside
    /// `DomainRouter` itself.
    pub domain_router: Shared<RwLock<DomainRouter>>,

    /// Phase 3 state substrate: byte buffers, registers, view slots, and
    /// service leases.
    pub state: Shared<RwLock<StateSubstrate>>,

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

pub(crate) struct EditorCoreParts {
    pub abi_shared: Shared<EditorCoreAbi>,
    pub boot_anchor: BootClock,
    pub event_bus: Shared<DS12EventBus>,
    pub log_ring: Shared<LogRing>,
    pub session: Shared<Session>,
    pub sessions: Shared<RwLock<SessionTable>>,
    pub domain_router: Shared<RwLock<DomainRouter>>,
    pub state: Shared<RwLock<StateSubstrate>>,
}

impl EditorCore {
    /// Constructs a `EditorCore` from the fields moved out of `EditorInit` at the
    /// handoff (stage 7).
    ///
    /// **Crate-private**: only `EditorInit::boot` calls this. External code has no
    /// `EditorCore` constructor — that is the LF13 invariant.
    ///
    /// # Example (crate-internal)
    ///
    /// ```rust,no_run
    /// // no_run: pub(crate) — only EditorInit::boot is the intended caller (LF13).
    /// ```
    pub(crate) fn new(parts: EditorCoreParts) -> Self {
        Self {
            abi: parts.abi_shared,
            boot_anchor: parts.boot_anchor,
            event_bus: parts.event_bus,
            log_ring: parts.log_ring,
            session: parts.session,
            sessions: parts.sessions,
            domain_router: parts.domain_router,
            state: parts.state,
            config: (),
            lockfile: (),
            inventory: (),
            correlation_alloc: (),
            force_overrides: (),
        }
    }
}

impl EditorCore {
    /// Registers the `OnRawInput` handler and `Render` projector for the named
    /// text Domain, returning the interned `DomainId`.
    ///
    /// Called by the composition root (or by editor-core-selftest tests) AFTER
    /// `EditorInit::boot` to wire the text Domain into the editor core. The editor core does not
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
        router.register_static_owner(STATIC_OWNER, STATIC_OWNER_ORDER)?;
        let mut manifest = DomainManifest::new(
            id,
            Bytes::try_from_slice(name.as_bytes()).map_err(|_| "alloc")?,
            STATIC_OWNER,
            DomainApiVersion::new(0, 16),
        );
        manifest
            .handler_kinds
            .try_push(HandlerId::OnRawInput)
            .map_err(|_| "alloc")?;
        manifest
            .projector_kinds
            .try_push(ProjectorId::Render)
            .map_err(|_| "alloc")?;
        router.register_manifest(manifest)?;
        router.register_handler(id, handler)?;
        router.register_projector(id, projector)?;
        router.activate_owner(STATIC_OWNER)?;
        Ok(id)
    }

    /// Replaces the session state with a real `SessionState` tied to the
    /// registered Domain.
    ///
    /// Called by the composition root (or tests) after `register_domain` to
    /// wire the session to the correct `DomainId`. Overwrites the placeholder
    /// installed by `EditorInit::boot`.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch runtime + registered domain.
    /// ```
    pub fn setup_session(&self, state: crate::session::SessionState) {
        let mut guard = self.session.state.lock();
        *guard = state;
    }

    /// Creates a new session-table entry rooted at `root_domain_id`.
    ///
    /// The returned session has fresh client, buffer, and window ids and a
    /// matching empty buffer record in [`StateSubstrate`].
    ///
    /// # Errors
    ///
    /// Returns an allocation or state-substrate error if either table cannot
    /// grow.
    pub fn create_session(
        &self,
        root_domain_id: crate::router::DomainId,
    ) -> Result<SessionId, &'static str> {
        let session = {
            let mut sessions = self.sessions.write();
            sessions.create_session(root_domain_id)?
        };
        let buffer_id = {
            let guard = session.state.lock();
            guard.buffer_id
        };
        let mut state = self.state.write();
        state.ensure_buffer(buffer_id)?;
        Ok(session.id)
    }

    /// Returns a shared session-table handle.
    #[must_use]
    pub fn session_by_id(&self, id: SessionId) -> Option<Shared<Session>> {
        let sessions = self.sessions.read();
        sessions.get(id)
    }

    fn static_owner_phase(
        &self,
        owner_cdylib_id: StateCdylibId,
    ) -> Result<RegistrationPhase, &'static str> {
        let router = self.domain_router.read();
        router
            .owner_record(owner_cdylib_id)
            .map(|record| record.phase)
            .ok_or("owner: missing static owner")
    }

    fn emit_carrier_validation_error(&self, source_id: u32) {
        let error_code = i32::try_from(source_id).unwrap_or(i32::MAX);
        let event = DS12Event {
            ts_nanos: self.boot_anchor.elapsed_nanos(),
            level: LogLevel::Warn,
            event: EVT_CARRIER_VALIDATION_ERROR,
            fields: BootStageFields {
                stage: 0,
                error_code: Some(error_code),
            },
        };
        self.event_bus.emit(&event);
    }

    /// Registers a position-carrier codec row during static owner init.
    ///
    /// # Errors
    ///
    /// Returns an owner-phase, limit, duplicate-row, or allocation error.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch runtime and a static owner init window.
    /// ```
    pub fn hostapi_position_codec_register(
        &self,
        domain_id: crate::router::DomainId,
        inner_id: u16,
        owner_cdylib_id: StateCdylibId,
        owner_generation: u64,
        max_content_bytes: usize,
    ) -> Result<(), &'static str> {
        if self.static_owner_phase(owner_cdylib_id)? != RegistrationPhase::Init {
            return Err("codec: registration outside init");
        }
        let mut state = self.state.write();
        state.register_position_codec(
            domain_id,
            inner_id,
            owner_cdylib_id,
            owner_generation,
            max_content_bytes,
        )
    }

    /// Registers a cursor-carrier codec row during static owner init.
    ///
    /// # Errors
    ///
    /// Returns an owner-phase, limit, duplicate-row, or allocation error.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch runtime and a static owner init window.
    /// ```
    pub fn hostapi_cursor_codec_register(
        &self,
        domain_id: crate::router::DomainId,
        inner_id: u16,
        owner_cdylib_id: StateCdylibId,
        owner_generation: u64,
        max_content_bytes: usize,
    ) -> Result<(), &'static str> {
        if self.static_owner_phase(owner_cdylib_id)? != RegistrationPhase::Init {
            return Err("codec: registration outside init");
        }
        let mut state = self.state.write();
        state.register_cursor_codec(
            domain_id,
            inner_id,
            owner_cdylib_id,
            owner_generation,
            max_content_bytes,
        )
    }

    /// Simulates codec-owner revocation; matching carriers validate as opaque.
    ///
    /// # Errors
    ///
    /// Returns an allocation error if the temporary row list cannot grow.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch runtime and registered codec rows.
    /// ```
    pub fn hostapi_codec_revoke_owner(
        &self,
        owner_cdylib_id: StateCdylibId,
        next_generation: u64,
    ) -> Result<usize, &'static str> {
        let mut state = self.state.write();
        state.revoke_codec_owner(owner_cdylib_id, next_generation)
    }

    /// Validates a position carrier at an editor-core ingress boundary.
    ///
    /// # Errors
    ///
    /// Returns `Err("carrier: invalid")` for malformed or over-cap carriers,
    /// and allocation errors from validation-event rate bookkeeping.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch runtime.
    /// ```
    pub fn hostapi_validate_position_carrier(
        &self,
        session_id: SessionId,
        source_id: u32,
        now_ms: u64,
        carrier: &PositionCarrier,
    ) -> Result<CarrierStatus, &'static str> {
        let report = {
            let mut state = self.state.write();
            state.validate_position_carrier(session_id, source_id, now_ms, carrier)?
        };
        if report.emit_ds12 {
            self.emit_carrier_validation_error(source_id);
        }
        if report.status == CarrierStatus::Invalid {
            Err("carrier: invalid")
        } else {
            Ok(report.status)
        }
    }

    /// Validates a cursor carrier at an editor-core ingress boundary.
    ///
    /// # Errors
    ///
    /// Returns `Err("carrier: invalid")` for malformed or over-cap carriers,
    /// and allocation errors from validation-event rate bookkeeping.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch runtime.
    /// ```
    pub fn hostapi_validate_cursor_carrier(
        &self,
        session_id: SessionId,
        source_id: u32,
        now_ms: u64,
        carrier: &CursorCarrier,
    ) -> Result<CarrierStatus, &'static str> {
        let report = {
            let mut state = self.state.write();
            state.validate_cursor_carrier(session_id, source_id, now_ms, carrier)?
        };
        if report.emit_ds12 {
            self.emit_carrier_validation_error(source_id);
        }
        if report.status == CarrierStatus::Invalid {
            Err("carrier: invalid")
        } else {
            Ok(report.status)
        }
    }

    /// Registers a stream scheme during static owner init.
    ///
    /// # Errors
    ///
    /// Returns an owner-phase, duplicate-name, limit, or allocation error.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch runtime and a static owner init window.
    /// ```
    pub fn hostapi_stream_scheme_register(&self, scheme: StreamScheme) -> Result<(), &'static str> {
        let phase = self.static_owner_phase(scheme.owner_cdylib_id)?;
        let mut state = self.state.write();
        state.register_stream_scheme(scheme, phase)
    }

    /// Opens a stream handle for a registered scheme name.
    ///
    /// # Errors
    ///
    /// Returns missing-scheme, limit, id-overflow, or allocation errors.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch runtime and registered stream scheme.
    /// ```
    pub fn hostapi_stream_open(
        &self,
        scheme_name: &[u8],
        session_id: Option<SessionId>,
        buffer_id: Option<BufferId>,
    ) -> Result<StreamId, &'static str> {
        let mut state = self.state.write();
        state.open_stream(scheme_name, session_id, buffer_id)
    }

    /// Adds a per-buffer subscription to a stream handle.
    ///
    /// # Errors
    ///
    /// Returns missing-stream, limit, or allocation errors.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch runtime and an open stream.
    /// ```
    pub fn hostapi_stream_subscribe(
        &self,
        stream_id: StreamId,
        session_id: SessionId,
        buffer_id: BufferId,
        max_buffered_bytes: usize,
    ) -> Result<bool, &'static str> {
        let mut state = self.state.write();
        state.stream_subscribe(stream_id, session_id, buffer_id, max_buffered_bytes)
    }

    /// Emits bytes into a stream's editor-core-owned subscription queues.
    ///
    /// # Errors
    ///
    /// Returns stale/closed, backpressure, missing-stream, or allocation errors.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch runtime and an open stream.
    /// ```
    pub fn hostapi_stream_emit(
        &self,
        stream_id: StreamId,
        bytes: &[u8],
        now_ms: u64,
    ) -> Result<BackpressureCounters, &'static str> {
        let mut state = self.state.write();
        state.stream_emit(stream_id, bytes, now_ms)
    }

    /// Drains one stream subscription queue.
    ///
    /// # Errors
    ///
    /// Returns missing-stream errors.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch runtime and an open stream.
    /// ```
    pub fn hostapi_stream_drain_subscription(
        &self,
        stream_id: StreamId,
        buffer_id: BufferId,
        now_ms: u64,
    ) -> Result<Option<Bytes>, &'static str> {
        let mut state = self.state.write();
        state.stream_drain_subscription(stream_id, buffer_id, now_ms)
    }

    /// Drains one stream subscription through the external byte-edit path.
    ///
    /// # Errors
    ///
    /// Returns missing-stream, byte-range, cap, or allocation errors.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch runtime and an open stream subscription.
    /// ```
    pub fn hostapi_stream_drain_to_buffer(
        &self,
        stream_id: StreamId,
        buffer_id: BufferId,
        now_ms: u64,
    ) -> Result<bool, &'static str> {
        let mut state = self.state.write();
        state.stream_drain_to_buffer(stream_id, buffer_id, now_ms)
    }

    /// Marks a stream stale after its underlying source ends.
    ///
    /// # Errors
    ///
    /// Returns a missing-stream error.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch runtime and an open stream.
    /// ```
    pub fn hostapi_stream_mark_stale(&self, stream_id: StreamId) -> Result<(), &'static str> {
        let mut state = self.state.write();
        state.stream_mark_stale(stream_id)
    }

    /// Classifies and validates a stream control op.
    ///
    /// # Errors
    ///
    /// Returns missing-stream, invalid-op, or reserved-op errors.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch runtime and an open stream.
    /// ```
    pub fn hostapi_stream_control_class(
        &self,
        stream_id: StreamId,
        op: StreamControlOp,
    ) -> Result<StreamControlClass, &'static str> {
        let state = self.state.read();
        state.stream_control_class(stream_id, op)
    }

    /// Closes a stream handle and clears its subscription queues.
    ///
    /// # Errors
    ///
    /// Returns a missing-stream error.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch runtime and an open stream.
    /// ```
    pub fn hostapi_stream_close(&self, stream_id: StreamId) -> Result<(), &'static str> {
        let mut state = self.state.write();
        state.stream_close(stream_id)
    }

    /// Simulates stream owner revocation over static owners.
    ///
    /// # Errors
    ///
    /// Returns allocation errors from temporary bookkeeping.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch runtime and registered stream schemes.
    /// ```
    pub fn hostapi_stream_revoke_owner(
        &self,
        owner_cdylib_id: StateCdylibId,
    ) -> Result<usize, &'static str> {
        let mut state = self.state.write();
        state.revoke_stream_owner(owner_cdylib_id)
    }

    /// HostApi-shaped register set operation.
    ///
    /// # Errors
    ///
    /// Returns validation, limit, or allocation errors from the state
    /// substrate.
    pub fn hostapi_register_set(
        &self,
        scope: RegisterScope,
        client_id: ClientId,
        session_id: SessionId,
        id: RegisterId,
        carrier: &CursorCarrier,
    ) -> Result<(), &'static str> {
        let mut state = self.state.write();
        state.set_register(scope, client_id, session_id, id, carrier)
    }

    /// HostApi-shaped register lookup with `client -> session -> system`
    /// fallback.
    ///
    /// # Errors
    ///
    /// Returns `Err("alloc")` if cloning the stored carrier fails.
    pub fn hostapi_register_get(
        &self,
        client_id: ClientId,
        session_id: SessionId,
        id: RegisterId,
    ) -> Result<Option<CursorCarrier>, &'static str> {
        let state = self.state.read();
        state.lookup_register_cloned(client_id, session_id, id)
    }

    /// HostApi-shaped register clear operation.
    #[must_use]
    pub fn hostapi_register_clear(
        &self,
        scope: RegisterScope,
        client_id: ClientId,
        session_id: SessionId,
        id: RegisterId,
    ) -> bool {
        let mut state = self.state.write();
        state.clear_register(scope, client_id, session_id, id)
    }

    /// HostApi-shaped view-slot write.
    ///
    /// # Errors
    ///
    /// Returns limit or allocation errors from the view-slot registry.
    pub fn hostapi_view_slot_put(
        &self,
        key: ViewSlotKey,
        owner_cdylib_id: StateCdylibId,
        flags: ViewSlotFlags,
        bytes: &[u8],
    ) -> Result<(), &'static str> {
        let mut state = self.state.write();
        state.put_view_slot(key, owner_cdylib_id, flags, bytes)
    }

    /// HostApi-shaped view-slot read by exact key.
    ///
    /// # Errors
    ///
    /// Returns `Err("alloc")` if cloning the stored slot bytes fails.
    pub fn hostapi_view_slot_get(
        &self,
        key: ViewSlotKey,
    ) -> Result<Option<ViewSlotRecord>, &'static str> {
        let state = self.state.read();
        state.view_slot_cloned(key)
    }

    /// Drops all view slots for one owner.
    ///
    /// # Errors
    ///
    /// Returns `Err("alloc")` if the temporary drop list cannot grow.
    pub fn hostapi_view_slots_drop_owner(
        &self,
        owner_cdylib_id: StateCdylibId,
    ) -> Result<usize, &'static str> {
        let mut state = self.state.write();
        state.drop_owner_view_slots(owner_cdylib_id)
    }

    /// Drops all view slots for one window tuple.
    ///
    /// # Errors
    ///
    /// Returns `Err("alloc")` if the temporary drop list cannot grow.
    pub fn hostapi_view_slots_drop_window(
        &self,
        client_id: ClientId,
        buffer_id: BufferId,
        window_id: WindowId,
    ) -> Result<usize, &'static str> {
        let mut state = self.state.write();
        state.drop_window_view_slots(client_id, buffer_id, window_id)
    }

    /// Drops all view slots associated with `buffer_id`.
    ///
    /// # Errors
    ///
    /// Returns `Err("alloc")` if the temporary drop list cannot grow.
    pub fn hostapi_view_slots_drop_buffer(
        &self,
        buffer_id: BufferId,
    ) -> Result<usize, &'static str> {
        let mut state = self.state.write();
        state.drop_buffer_view_slots(buffer_id)
    }

    /// HostApi-shaped byte-buffer edit.
    ///
    /// # Errors
    ///
    /// Returns range, cap, or allocation errors from the buffer record.
    pub fn hostapi_buffer_apply_edit(
        &self,
        buffer_id: BufferId,
        origin: EditOrigin,
        start: usize,
        end: usize,
        new_bytes: &[u8],
        timestamp_ms: u64,
    ) -> Result<(), &'static str> {
        let mut state = self.state.write();
        state.apply_buffer_edit(buffer_id, origin, start, end, new_bytes, timestamp_ms)
    }

    /// HostApi-shaped undo group open.
    ///
    /// # Errors
    ///
    /// Returns an error if a group is already open, the origin is not recorded,
    /// or allocation fails.
    pub fn hostapi_undo_group_open(
        &self,
        buffer_id: BufferId,
        origin: EditOrigin,
        timestamp_ms: u64,
    ) -> Result<UndoGroupId, &'static str> {
        let mut state = self.state.write();
        state.open_buffer_undo_group(buffer_id, origin, timestamp_ms)
    }

    /// HostApi-shaped undo group close.
    ///
    /// # Errors
    ///
    /// Returns an error if no group is open or buffer creation fails.
    pub fn hostapi_undo_group_close(&self, buffer_id: BufferId) -> Result<(), &'static str> {
        let mut state = self.state.write();
        state.close_buffer_undo_group(buffer_id)
    }

    /// HostApi-shaped undo operation.
    ///
    /// # Errors
    ///
    /// Returns an error if a group is open or allocation fails.
    pub fn hostapi_undo(&self, buffer_id: BufferId) -> Result<bool, &'static str> {
        let mut state = self.state.write();
        state.undo_buffer(buffer_id)
    }

    /// HostApi-shaped redo operation.
    ///
    /// # Errors
    ///
    /// Returns an error if replaying a redo group fails or allocation fails.
    pub fn hostapi_redo(&self, buffer_id: BufferId) -> Result<bool, &'static str> {
        let mut state = self.state.write();
        state.redo_buffer(buffer_id)
    }

    /// Reads cloned bytes from an editor-core-owned buffer record.
    ///
    /// # Errors
    ///
    /// Returns `Err("alloc")` if cloning buffer bytes fails.
    pub fn hostapi_buffer_bytes(&self, buffer_id: BufferId) -> Result<Option<Bytes>, &'static str> {
        let state = self.state.read();
        state.buffer_bytes(buffer_id)
    }

    /// Registers a service row.
    ///
    /// # Errors
    ///
    /// Returns duplicate-key or allocation errors from the service registry.
    pub fn hostapi_service_register(
        &self,
        meta: ServiceDescriptorMeta,
        owner_generation: u64,
    ) -> Result<(), &'static str> {
        let mut state = self.state.write();
        state.service_registry.register(meta, owner_generation)
    }

    /// Performs a borrow-only service lookup.
    ///
    /// # Errors
    ///
    /// Returns [`ServiceAccessError::NotFound`] when no visible row exists.
    pub fn hostapi_service_borrow(
        &self,
        key: ServiceKey,
    ) -> Result<ServiceBorrow, ServiceAccessError> {
        let state = self.state.read();
        state.service_registry.borrow(key)
    }

    /// Retains a service lease.
    ///
    /// # Errors
    ///
    /// Returns lookup or allocation errors from the service registry.
    pub fn hostapi_service_lease(
        &self,
        key: ServiceKey,
        acquired_at_ms: u64,
    ) -> Result<ServiceLease, ServiceAccessError> {
        let mut state = self.state.write();
        state.service_registry.lease(key, acquired_at_ms)
    }

    /// Releases a retained service lease.
    #[must_use]
    pub fn hostapi_service_lease_release(&self, id: ServiceLeaseId) -> bool {
        let mut state = self.state.write();
        state.service_registry.release_lease(id)
    }

    /// Validates a service lease against generation and timeout.
    ///
    /// # Errors
    ///
    /// Returns `Busy` on timeout and `Stale` for revoked or mismatched rows.
    pub fn hostapi_service_lease_validate(
        &self,
        id: ServiceLeaseId,
        now_ms: u64,
    ) -> Result<(), ServiceAccessError> {
        let state = self.state.read();
        state.service_registry.validate_lease(id, now_ms)
    }

    /// Begins a service call after `SEND_SAFE`/`SYNC_SAFE` checks.
    ///
    /// # Errors
    ///
    /// Returns thread-safety, concurrency, lookup, or stale-generation errors.
    pub fn hostapi_service_begin_call(
        &self,
        key: ServiceKey,
        caller_thread_id: i32,
    ) -> Result<ServiceCallToken, ServiceAccessError> {
        let mut state = self.state.write();
        state.service_registry.begin_call(key, caller_thread_id)
    }

    /// Ends a service call token.
    #[must_use]
    pub fn hostapi_service_end_call(&self, token: ServiceCallToken) -> bool {
        let mut state = self.state.write();
        state.service_registry.end_call(token)
    }

    /// Simulates owner-scoped service revocation over static owners.
    ///
    /// # Errors
    ///
    /// Returns `Err("alloc")` if the temporary owner-key list cannot grow.
    pub fn hostapi_service_revoke_owner(
        &self,
        owner_cdylib_id: StateCdylibId,
        next_generation: u64,
    ) -> Result<usize, &'static str> {
        let mut state = self.state.write();
        state
            .service_registry
            .revoke_owner(owner_cdylib_id, next_generation)
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
        let snapshot = self.session.prepare_dispatch(input)?;
        let routes = {
            let router = self.domain_router.read();
            DispatchRoutes::from_router(&snapshot, &router)
        };
        self.session.dispatch_prepared(&snapshot, input, &routes)
    }

    /// Dispatches input through a session-table entry.
    ///
    /// The sessions-map and router locks are used only for snapshots and are
    /// dropped before handler/projector invocation (CC14).
    ///
    /// # Errors
    ///
    /// Returns a missing-session error or any dispatch/route/projection error.
    pub fn dispatch_input_for_session(
        &self,
        session_id: SessionId,
        input: &[u8],
    ) -> Result<crate::projection::Projection, &'static str> {
        let session = self
            .session_by_id(session_id)
            .ok_or("session-table: missing session")?;
        let snapshot = session.prepare_dispatch(input)?;
        let routes = {
            let router = self.domain_router.read();
            DispatchRoutes::from_router(&snapshot, &router)
        };
        session.dispatch_prepared(&snapshot, input, &routes)
    }
}

// `EditorCore` field Send+Sync analysis:
// - `Shared<EditorCoreAbi>`: plain-integer data → auto Send+Sync.
// - `BootClock`: `Copy` integer-only data → auto Send+Sync.
// - `Shared<DS12EventBus>`: holds `RwLock<Seq<fn>>` + `Option<Shared<LogRing>>` →
//   auto Send+Sync.
// - `Shared<LogRing>`: `Mutex<Ring<LogEntry>>` + `usize` → auto Send+Sync.
// - `Shared<Session>`: `Session` holds `Mutex<SessionState>` + `SessionId` (Copy).
//   `SessionState` contains `Bytes` (owns raw allocation; `Bytes: Send` because
//   it is single-owner) + primitive fields. `Mutex<T>` requires `T: Send`
//   (not `T: Sync`). `Bytes` is `Send`. So `Mutex<SessionState>: Send + Sync`.
//   `Session: Send + Sync`. `Shared<Session>: Send + Sync`.
// - `Shared<RwLock<SessionTable>>`: the map owns `Shared<Session>` handles;
//   table mutations are behind an `RwLock`, and `Session` is Send+Sync.
// - `Shared<RwLock<DomainRouter>>`: `DomainRouter` holds `Map<K, V>` where
//   values are `DomainId`/`Bytes` (both `Send`). `handler`/`projector` are
//   `&'static dyn … + Send + Sync`. `RwLock<DomainRouter>: Send + Sync` (same
//   analysis as `Mutex` — `DomainRouter: Send`). `Shared<RwLock<DomainRouter>>:
//   Send + Sync`.
// - `Shared<RwLock<StateSubstrate>>`: state maps own `Bytes`, carrier records,
//   and plain id keys; service metadata is owned bytes + integer flags. The
//   lock provides interior synchronization and the underlying records are Send.
// - `()` placeholders: trivially Send+Sync.
//
// The compiler derives Send+Sync automatically; no manual `unsafe impl` needed.

// L12 layout: tests in sibling root_tests.rs, declared in lib.rs.
