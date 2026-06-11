//! `DomainRouter` SUBSET — `intern_named`/`name_of` + one handler row +
//! one projector row (§4.1 walking-skeleton subset note, #797).
//!
//! The full §4.1 `DomainRouter` struct (bands, tie-break, manifests, codec
//! maps, positional-arg dispatch) is the spec target. The skeleton grows only
//! what the first consumer (the text Domain, #797) demands. The seam shape is
//! observed when the second Domain arrives (Phase 4); no premature abstraction
//! is extracted here (rule of three).
//!
//! ## What is implemented
//!
//! - `intern_named` / `name_of` — name↔id bidirectional intern using
//!   `arch::ds::Map` and `arch::ds::Bytes`.
//! - One handler row `(DomainId, handler)` and one projector row
//!   `(DomainId, projector)`.
//!
//! The contract types (`DomainId`, `OnRawInputHandler`, `RenderProjector`) live
//! in `reovim-subsys-domain` (the Domain contract tier, DAG2). The kernel owns
//! only the registration mechanics; it re-exports the contract types at this
//! module's historical paths for callers that name `reovim_kernel::router::*`.
//!
//! ## What is NOT implemented (deferred)
//!
//! - DT10 priority bands, DT17 tie-break (no `Vec<HandlerEntry>`; one handler
//!   per `DomainId`).
//! - `DomainManifest` registration and unload.
//! - Codec maps (`position_codecs`, `cursor_codecs`).
//! - Positional-arg dispatch (`AL10`).
//!
//! ## Core/ext boundary
//!
//! `DomainRouter` is a kernel type. It does NOT depend on `ext/server/domain/*`.
//! The composition root wires the text Domain's `TextHandler` and
//! `TextProjector` into the router. The kernel routes opaque through these
//! contract traits (the same mechanism-vs-policy principle as the rest of the
//! kernel).

use reovim_arch::ds::{Bytes, Map};

// Contract types live in the Domain contract tier; re-exported here so callers
// that name `reovim_kernel::router::{DomainId, OnRawInputHandler, RenderProjector}`
// continue to compile.
pub use reovim_subsys_domain::{DomainId, OnRawInputHandler, RenderProjector};

// ── DomainRouter ──────────────────────────────────────────────────────────────

/// `DomainRouter` SUBSET (§4.1 §2, walking-skeleton, #797).
///
/// Provides `intern_named`/`name_of` over `arch::ds::Map`, one handler row,
/// and one projector row. The full §4.1 struct (bands, tie-break, manifests,
/// codec maps) is grown when the second Domain consumer arrives (Phase 4).
pub struct DomainRouter {
    /// Next allocation counter; starts at 1 (`DomainId` is `NonZeroU32`).
    next_id: u32,
    /// Name → `DomainId` forward map.
    ///
    /// Keys are stored as 32-byte fixed-length names (zero-padded). Domain
    /// names in the skeleton are short ASCII strings ("text"); the limit is
    /// adequate and avoids a separate heap string allocation per key.
    names: Map<NameKey, DomainId>,
    /// `DomainId` → name reverse map (stored as owned `Bytes` per id).
    ids: Map<u32, Bytes>,
    /// One handler per `DomainId` (walking-skeleton: no `Vec`/bands).
    handler: Option<(DomainId, &'static dyn OnRawInputHandler)>,
    /// One projector per `DomainId` (walking-skeleton: no `Vec`/bands).
    projector: Option<(DomainId, &'static dyn RenderProjector)>,
}

/// Fixed-width Domain name key (max 31 bytes + null terminator, zero-padded).
///
/// Short ASCII names like "text" fit without allocation. The 32-byte bound is
/// sufficient for all anticipated skeleton Domain names.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct NameKey([u8; 32]);

impl NameKey {
    /// Builds a `NameKey` from a `&str`. Panics if `name.len() > 31`.
    fn from_str(name: &str) -> Self {
        let bytes = name.as_bytes();
        assert!(bytes.len() <= 31, "domain name exceeds 31 bytes");
        let mut key = [0u8; 32];
        key[..bytes.len()].copy_from_slice(bytes);
        Self(key)
    }
}

impl DomainRouter {
    /// Creates an empty `DomainRouter`.
    ///
    /// ```rust
    /// use reovim_kernel::router::DomainRouter;
    ///
    /// let r = DomainRouter::new();
    /// assert!(r.name_of_id(1).is_none());
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {
            next_id: 1,
            names: Map::new(),
            ids: Map::new(),
            handler: None,
            projector: None,
        }
    }

    /// Interns a Domain name: idempotent, returns the same `DomainId` for the
    /// same name across calls (§4.1 §3 intern stability).
    ///
    /// # Errors
    ///
    /// Returns `Err("alloc")` if the backing map allocation fails.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime.
    /// use reovim_kernel::router::DomainRouter;
    ///
    /// let mut r = DomainRouter::new();
    /// let id1 = r.intern_named("text").unwrap();
    /// let id2 = r.intern_named("text").unwrap();
    /// assert_eq!(id1, id2, "intern stability: same name → same DomainId");
    /// ```
    pub fn intern_named(&mut self, name: &str) -> Result<DomainId, &'static str> {
        let key = NameKey::from_str(name);
        if let Some(&existing) = self.names.get(&key) {
            return Ok(existing);
        }
        // Allocate a new id.
        let raw = core::num::NonZeroU32::new(self.next_id).ok_or("alloc: DomainId overflow")?;
        self.next_id = self
            .next_id
            .checked_add(1)
            .ok_or("alloc: DomainId overflow")?;
        let id = DomainId::new(raw);

        // Store name → id.
        self.names.try_insert(key, id).map_err(|_| "alloc")?;
        // Store id → name bytes.
        let name_bytes = Bytes::try_from_slice(name.as_bytes()).map_err(|_| "alloc")?;
        self.ids
            .try_insert(id.as_u32(), name_bytes)
            .map_err(|_| "alloc")?;

        Ok(id)
    }

    /// Returns the name string bytes for a `DomainId`, if it was interned.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime.
    /// use reovim_kernel::router::DomainRouter;
    ///
    /// let mut r = DomainRouter::new();
    /// let id = r.intern_named("text").unwrap();
    /// let name = r.name_of(id).unwrap();
    /// assert_eq!(name, b"text");
    /// ```
    #[must_use]
    pub fn name_of(&self, id: DomainId) -> Option<&[u8]> {
        self.ids
            .get(&id.as_u32())
            .map(reovim_arch::ds::Bytes::as_slice)
    }

    /// Returns the name for a raw id value (used by tests and diagnostics).
    ///
    /// ```rust
    /// use reovim_kernel::router::DomainRouter;
    ///
    /// let r = DomainRouter::new();
    /// assert!(r.name_of_id(999).is_none());
    /// ```
    #[must_use]
    pub fn name_of_id(&self, raw: u32) -> Option<&[u8]> {
        self.ids.get(&raw).map(reovim_arch::ds::Bytes::as_slice)
    }

    /// Registers the `OnRawInput` handler for the given `DomainId`.
    ///
    /// Walking-skeleton: only one handler is stored; a second call overwrites
    /// the previous (rule of three: no Vec/bands before Phase 4).
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator + a concrete handler.
    /// ```
    pub fn register_handler(&mut self, id: DomainId, h: &'static dyn OnRawInputHandler) {
        self.handler = Some((id, h));
    }

    /// Registers the `Render` projector for the given `DomainId`.
    ///
    /// Walking-skeleton: only one projector is stored (rule of three).
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator + a concrete projector.
    /// ```
    pub fn register_projector(&mut self, id: DomainId, p: &'static dyn RenderProjector) {
        self.projector = Some((id, p));
    }

    /// Returns the registered `OnRawInput` handler for `id`, if any.
    ///
    /// ```rust
    /// use reovim_kernel::router::DomainRouter;
    ///
    /// let r = DomainRouter::new();
    /// use core::num::NonZeroU32;
    /// use reovim_kernel::router::DomainId;
    /// let id = DomainId::new(NonZeroU32::new(1).unwrap());
    /// assert!(r.handler(id).is_none());
    /// ```
    #[must_use]
    pub fn handler(&self, id: DomainId) -> Option<&dyn OnRawInputHandler> {
        self.handler
            .as_ref()
            .filter(|(hid, _)| *hid == id)
            .map(|(_, h)| *h)
    }

    /// Returns the registered `Render` projector for `id`, if any.
    ///
    /// ```rust
    /// use reovim_kernel::router::DomainRouter;
    ///
    /// let r = DomainRouter::new();
    /// use core::num::NonZeroU32;
    /// use reovim_kernel::router::DomainId;
    /// let id = DomainId::new(NonZeroU32::new(1).unwrap());
    /// assert!(r.projector(id).is_none());
    /// ```
    #[must_use]
    pub fn projector(&self, id: DomainId) -> Option<&dyn RenderProjector> {
        self.projector
            .as_ref()
            .filter(|(pid, _)| *pid == id)
            .map(|(_, p)| *p)
    }
}

impl Default for DomainRouter {
    fn default() -> Self {
        Self::new()
    }
}

// `DomainRouter` is auto-`Send + Sync`: its `Map` fields carry the arch-DS
// bounded impls (exclusive bucket ownership, Vec-analogy), and the handler/
// projector rows are `'static` fn-pointer/`Send + Sync` data. No manual
// impl — the bounds must come from the fields so a future non-`Send` field
// is caught by the compiler, not papered over.

// L12 layout: tests in sibling router_tests.rs, declared in lib.rs.
