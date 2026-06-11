//! Minimal `Session` + `SessionState` for the walking-skeleton (§2.3, #797).
//!
//! A `Session` wraps a per-session `Mutex<SessionState>` (the turn-gate
//! serialising dispatch — CC14) and a `WindowId`. A single session, single
//! buffer, single-entry focus chain is the entire extent built here (§4.2
//! walking-skeleton subset note). Multi-session, multi-buffer, multi-window
//! support arrives with master Phase 4.
//!
//! ## CC14 dispatch shape
//!
//! Dispatch follows the CC14 pattern: snapshot under the state lock, drop the
//! lock, invoke the handler, re-lock to apply mutations. The lock is NEVER held
//! across the Domain handler invocation.
//!
//! ## `()` placeholders untouched
//!
//! The existing `Kernel` `()` placeholders (`config`, `lockfile`, `inventory`,
//! `correlation_alloc`, `force_overrides`) are NOT refilled here. The skeleton
//! grows NEW fields (`session`, `domain_router`) onto `Kernel`.

use reovim_arch::{ds::Bytes, sync::Mutex};

use crate::{
    projection::Projection,
    router::{DomainId, DomainRouter},
};

// `BufferId` and `WindowId` live in the Domain contract tier
// (`reovim-subsys-domain`) because they appear in the `RenderProjector` contract
// and in `Projection`. Re-exported here so callers that name
// `reovim_kernel::session::{BufferId, WindowId}` continue to compile.
pub use reovim_subsys_domain::id::{BufferId, WindowId};

// ── SessionId ────────────────────────────────────────────────────────────────

/// Opaque session identifier (walking-skeleton: one session, value = 1).
///
/// The numeric value is allocation-order-dependent across runs (CR9). For the
/// skeleton a single fixed session is used; the generalized map arrives with
/// Phase 4.
///
/// ```rust
/// use reovim_kernel::session::SessionId;
///
/// let id = SessionId::new(1);
/// assert_eq!(id.as_u32(), 1);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionId(u32);

impl SessionId {
    /// Constructs a `SessionId` from a raw `u32`.
    ///
    /// ```rust
    /// use reovim_kernel::session::SessionId;
    ///
    /// let id = SessionId::new(42);
    /// assert_eq!(id.as_u32(), 42);
    /// ```
    #[must_use]
    pub const fn new(v: u32) -> Self {
        Self(v)
    }

    /// Returns the raw `u32` value.
    ///
    /// ```rust
    /// use reovim_kernel::session::SessionId;
    ///
    /// assert_eq!(SessionId::new(7).as_u32(), 7);
    /// ```
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self.0
    }
}

// ── DomainAttachmentId ───────────────────────────────────────────────────────

/// Opaque Domain-attachment identifier (walking-skeleton: one root, value = 1).
///
/// ```rust
/// use reovim_kernel::session::DomainAttachmentId;
///
/// let id = DomainAttachmentId::new(1);
/// assert_eq!(id.as_u32(), 1);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DomainAttachmentId(u32);

impl DomainAttachmentId {
    /// Constructs a `DomainAttachmentId` from a raw `u32`.
    ///
    /// ```rust
    /// use reovim_kernel::session::DomainAttachmentId;
    ///
    /// let id = DomainAttachmentId::new(1);
    /// assert_eq!(id.as_u32(), 1);
    /// ```
    #[must_use]
    pub const fn new(v: u32) -> Self {
        Self(v)
    }

    /// Returns the raw `u32` value.
    ///
    /// ```rust
    /// use reovim_kernel::session::DomainAttachmentId;
    ///
    /// assert_eq!(DomainAttachmentId::new(9).as_u32(), 9);
    /// ```
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self.0
    }
}

// ── FocusEntry ───────────────────────────────────────────────────────────────

/// One entry in a `FocusChain` (§4.2 §3, walking-skeleton: `Resolved` only).
///
/// The walking skeleton realizes only `Resolved` entries. `Pending` entries
/// (DT12, `PendingAttachmentId`) are deferred to Phase 4.
///
/// ```rust
/// use reovim_kernel::session::{DomainAttachmentId, FocusEntry};
///
/// let entry = FocusEntry::Resolved(DomainAttachmentId::new(1));
/// match entry {
///     FocusEntry::Resolved(id) => assert_eq!(id.as_u32(), 1),
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusEntry {
    /// A resolved Domain attachment in the focus chain.
    Resolved(DomainAttachmentId),
}

// ── SessionState ─────────────────────────────────────────────────────────────

/// Mutable per-session state, held behind a `Mutex` (CC14).
///
/// The walking-skeleton subset carries:
/// - one text buffer (`arch::ds::Bytes`),
/// - one `DomainId` (the registered text Domain),
/// - one `DomainAttachmentId` for the root attachment,
/// - a single-entry `FocusChain = [Resolved(root)]` (§4.2 subset note),
/// - the cursor byte offset within the buffer.
///
/// Multi-session, multi-buffer, and multi-window support arrives with Phase 4.
pub struct SessionState {
    /// The buffer bytes (DT1: bytes + identity, no interpretation inside).
    pub buffer: Bytes,
    /// Cursor position in bytes within `buffer`.
    pub cursor: usize,
    /// The Domain this session's root attachment belongs to.
    pub root_domain_id: DomainId,
    /// The root `DomainAttachment` for this session's single buffer (§4.2).
    pub root_attachment: DomainAttachmentId,
    /// Single-entry focus chain `[Resolved(root)]` (§4.2 subset note).
    pub focus_chain: [FocusEntry; 1],
    /// The `WindowId` for this session's single window.
    pub window_id: WindowId,
    /// The `BufferId` for this session's single buffer.
    pub buffer_id: BufferId,
}

impl SessionState {
    /// Creates a new `SessionState` with an empty buffer and a registered
    /// text Domain.
    ///
    /// The root attachment `struct_refs = focus_refs = 1` is implicit (§4.2
    /// walking-skeleton subset: the skeleton grows root-only, so the refcount
    /// starts at 1; multi-session allocation arrives with #778 Phase 4).
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime.
    /// use reovim_kernel::{
    ///     router::DomainId,
    ///     session::{BufferId, DomainAttachmentId, SessionState, WindowId},
    /// };
    /// use core::num::NonZeroU32;
    ///
    /// let domain_id = DomainId::new(NonZeroU32::new(1).unwrap());
    /// let state = SessionState::new(
    ///     domain_id,
    ///     DomainAttachmentId::new(1),
    ///     BufferId::new(1),
    ///     WindowId::new(1),
    /// );
    /// assert!(state.buffer.is_empty());
    /// assert_eq!(state.cursor, 0);
    /// ```
    #[must_use]
    pub const fn new(
        root_domain_id: DomainId,
        root_attachment: DomainAttachmentId,
        buffer_id: BufferId,
        window_id: WindowId,
    ) -> Self {
        Self {
            buffer: Bytes::new(),
            cursor: 0,
            root_domain_id,
            root_attachment,
            focus_chain: [FocusEntry::Resolved(root_attachment)],
            window_id,
            buffer_id,
        }
    }
}

// ── Session ───────────────────────────────────────────────────────────────────

/// A single editing session (walking-skeleton: one session per kernel).
///
/// The `Mutex<SessionState>` is the turn gate (CC14): dispatch snapshots state
/// under the lock, drops it, invokes the handler, then re-locks to apply
/// mutations. The lock is NEVER held across a Domain handler call.
pub struct Session {
    /// Per-session mutable state, serialised by `Mutex` (CC14).
    pub state: Mutex<SessionState>,
    /// This session's identifier.
    pub id: SessionId,
}

impl Session {
    /// Constructs a new `Session` with the given initial state.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator + futex runtime.
    /// use reovim_kernel::{
    ///     router::DomainId,
    ///     session::{BufferId, DomainAttachmentId, Session, SessionId, SessionState, WindowId},
    /// };
    /// use core::num::NonZeroU32;
    ///
    /// let domain_id = DomainId::new(NonZeroU32::new(1).unwrap());
    /// let state = SessionState::new(
    ///     domain_id,
    ///     DomainAttachmentId::new(1),
    ///     BufferId::new(1),
    ///     WindowId::new(1),
    /// );
    /// let session = Session::new(SessionId::new(1), state);
    /// assert_eq!(session.id.as_u32(), 1);
    /// ```
    #[must_use]
    pub const fn new(id: SessionId, state: SessionState) -> Self {
        Self {
            state: Mutex::new(state),
            id,
        }
    }

    /// Dispatches a raw-input byte sequence through the router's `OnRawInput`
    /// handler, then calls the `Render` projector to produce a `Projection`.
    ///
    /// Follows the CC14 dispatch shape:
    /// 1. Lock state, snapshot what the handler needs (buffer + cursor), unlock.
    /// 2. Invoke the handler (lock not held — CC14).
    /// 3. Re-lock and apply the handler's mutation.
    /// 4. Lock state, snapshot what the projector needs, unlock.
    /// 5. Invoke the projector (lock not held — CC14).
    ///
    /// # Errors
    ///
    /// Returns `Err(&'static str)` if the router has no handler or projector
    /// registered for the session's root Domain.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator + futex runtime and a registered domain.
    /// ```
    pub fn dispatch_input(
        &self,
        input: &[u8],
        router: &DomainRouter,
    ) -> Result<Projection, &'static str> {
        // ── Step 1: snapshot under lock ──────────────────────────────────────
        let (domain_id, buffer_snapshot, cursor_snapshot, buffer_id, window_id) = {
            let guard = self.state.lock();
            (
                guard.root_domain_id,
                // Clone the buffer bytes for the handler snapshot.
                // Safety: the allocation here is bounded and deterministic.
                {
                    let mut snap = Bytes::new();
                    // Extend from the current buffer; propagate alloc failure as dispatch error.
                    snap.try_extend_from_slice(guard.buffer.as_slice())
                        .map_err(|_| "alloc: buffer snapshot failed")?;
                    snap
                },
                guard.cursor,
                guard.buffer_id,
                guard.window_id,
            )
        };
        // Lock released here — CC14 invariant: not held across handler call.

        // ── Step 2: invoke OnRawInput handler (no lock held) ─────────────────
        let handler = router
            .handler(domain_id)
            .ok_or("dispatch: no OnRawInput handler for domain")?;
        let (new_buffer, new_cursor) =
            handler.on_raw_input(buffer_snapshot, cursor_snapshot, input);

        // ── Step 3: re-lock and apply mutations ──────────────────────────────
        {
            let mut guard = self.state.lock();
            // Replace buffer bytes with the handler's output.
            let buf = &mut guard.buffer;
            // Truncate to zero and extend with new content.
            // `Bytes` does not have a `clear`; we truncate via pop loop.
            // Use the arch `Bytes::truncate` pattern by building a fresh `Bytes`.
            let mut new_buf = Bytes::new();
            new_buf
                .try_extend_from_slice(new_buffer.as_slice())
                .map_err(|_| "alloc: apply buffer failed")?;
            *buf = new_buf;
            guard.cursor = new_cursor;
        }

        // ── Step 4: snapshot for projector (under lock) ──────────────────────
        let (proj_buffer, proj_cursor) = {
            let guard = self.state.lock();
            let mut snap = Bytes::new();
            snap.try_extend_from_slice(guard.buffer.as_slice())
                .map_err(|_| "alloc: projector snapshot failed")?;
            (snap, guard.cursor)
        };

        // ── Step 5: invoke Render projector (no lock held) ───────────────────
        let projector = router
            .projector(domain_id)
            .ok_or("dispatch: no Render projector for domain")?;
        let projection = projector.render(proj_buffer, proj_cursor, buffer_id, window_id)?;

        Ok(projection)
    }
}

// `Session` contains `Mutex<SessionState>` which is `Send + Sync` because:
// - `SessionState` holds `Bytes` (owns a raw allocation, `Send` but not `Sync`;
//   `Mutex<T>` requires only `T: Send` — `Bytes` is `Send`).
// - `Mutex` uses an `AtomicU32` lock word (futex), thread-safe by construction.
// - `SessionId` is `Copy` plain-integer data.
// The compiler auto-derives `Send + Sync` from the field types.

// L12 layout: tests in sibling session_tests.rs, declared in lib.rs.
