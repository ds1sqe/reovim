//! `DomainRouter` — Domain names plus ordered handler/projector rows (§4.1).
//!
//! The #797 walking skeleton carried only `intern_named`/`name_of`, one
//! `OnRawInput` handler, and one `Render` projector. Phase 4 grows that into
//! the row-vector substrate: handlers/projectors are keyed by
//! `(DomainId, HandlerId)` / `(DomainId, ProjectorId)`, carry DT10 priority
//! metadata, and remain deterministically ordered by DT17.
//!
//! ## What is implemented
//!
//! - `intern_named` / `name_of` — name↔id bidirectional intern using
//!   `arch::ds::Map` and `arch::ds::Bytes`.
//! - Handler rows keyed by `(DomainId, HandlerId)`.
//! - Projector rows keyed by `(DomainId, ProjectorId)`.
//! - DT10 priority validation and DT17 deterministic row order.
//!
//! The contract row shapes live in `reovim-subsys-domain` (DAG2). The kernel
//! owns only storage, registration mechanics, and dispatch lookup.
//!
//! ## Core/ext boundary
//!
//! `DomainRouter` is a kernel type. It does NOT depend on `ext/server/domain/*`.
//! The composition root wires concrete Domain trait objects into the router.
//! The kernel routes opaque through contract traits.

use reovim_arch::ds::{Bytes, Map, Seq};

// Contract types live in the Domain contract tier; re-exported here so callers
// that name `reovim_kernel::router::*` continue to compile.
pub use reovim_subsys_domain::{
    CdylibId, DomainApiVersion, DomainId, DomainManifest, HandlerEntry, HandlerId,
    OnRawInputHandler, PriorityBand, ProjectorEntry, ProjectorId, RawInputResult,
    RegistrationError, RegistrationPhase, RenderProjector, RowKind,
};

/// Default static owner used by the current in-process composition root.
const STATIC_OWNER: CdylibId = CdylibId::new(core::num::NonZeroU32::MIN);
/// Default owner order for the current static owner set.
const STATIC_OWNER_ORDER: u32 = 0;

/// In-process owner lifecycle record for Phase 4 static row simulation.
///
/// Phase 4 does not load or unload dynamic libraries. It still needs the same
/// row gates later lifecycle work will use, so the router tracks static owner
/// records with an init/active/failed phase and deterministic DT17 order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StaticOwnerRecord {
    /// Owner id shared by manifests and dispatch rows.
    pub cdylib_id: CdylibId,
    /// Canonical owner order used by DT17.
    pub owner_order: u32,
    /// Current init-only registration phase.
    pub phase: RegistrationPhase,
}

impl StaticOwnerRecord {
    /// Builds an owner record in the init phase.
    #[must_use]
    pub const fn init(cdylib_id: CdylibId, owner_order: u32) -> Self {
        Self {
            cdylib_id,
            owner_order,
            phase: RegistrationPhase::Init,
        }
    }
}

// ── DomainRouter ──────────────────────────────────────────────────────────────

/// `DomainRouter` (§4.1 §2, Phase 4 row substrate).
///
/// Provides `intern_named`/`name_of` over `arch::ds::Map`, plus ordered
/// handler/projector vectors keyed by `(DomainId, HandlerId)` and
/// `(DomainId, ProjectorId)`.
pub struct DomainRouter {
    /// Next allocation counter; starts at 1 (`DomainId` is `NonZeroU32`).
    next_id: u32,
    /// Name → `DomainId` forward map.
    names: Map<NameKey, DomainId>,
    /// `DomainId` → name reverse map (stored as owned `Bytes` per id).
    ids: Map<u32, Bytes>,
    /// Registered manifests by Domain id.
    manifests: Map<DomainId, DomainManifest>,
    /// Static owner lifecycle records by owner id.
    owners: Map<CdylibId, StaticOwnerRecord>,
    /// Handler rows keyed by `(DomainId, HandlerId)`.
    handlers: Map<(DomainId, HandlerId), Seq<RegisteredHandler>>,
    /// Projector rows keyed by `(DomainId, ProjectorId)`.
    projectors: Map<(DomainId, ProjectorId), Seq<RegisteredProjector>>,
}

/// Registered handler row plus callable.
///
/// ```rust,no_run
/// // no_run: requires a concrete static handler.
/// ```
pub struct RegisteredHandler {
    /// Contract row metadata.
    pub entry: HandlerEntry,
    callable: &'static dyn OnRawInputHandler,
}

impl RegisteredHandler {
    /// Builds a registered handler row.
    ///
    /// ```rust,no_run
    /// // no_run: requires a concrete static handler.
    /// ```
    #[must_use]
    pub const fn new(entry: HandlerEntry, callable: &'static dyn OnRawInputHandler) -> Self {
        Self { entry, callable }
    }

    /// Returns the callable trait object.
    ///
    /// ```rust,no_run
    /// // no_run: requires a concrete static handler.
    /// ```
    #[must_use]
    pub const fn callable(&self) -> &'static dyn OnRawInputHandler {
        self.callable
    }
}

/// Registered projector row plus callable.
///
/// ```rust,no_run
/// // no_run: requires a concrete static projector.
/// ```
pub struct RegisteredProjector {
    /// Contract row metadata.
    pub entry: ProjectorEntry,
    callable: &'static dyn RenderProjector,
}

impl RegisteredProjector {
    /// Builds a registered projector row.
    ///
    /// ```rust,no_run
    /// // no_run: requires a concrete static projector.
    /// ```
    #[must_use]
    pub const fn new(entry: ProjectorEntry, callable: &'static dyn RenderProjector) -> Self {
        Self { entry, callable }
    }

    /// Returns the callable trait object.
    ///
    /// ```rust,no_run
    /// // no_run: requires a concrete static projector.
    /// ```
    #[must_use]
    pub const fn callable(&self) -> &'static dyn RenderProjector {
        self.callable
    }
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
    pub const fn new() -> Self {
        Self {
            next_id: 1,
            names: Map::new(),
            ids: Map::new(),
            manifests: Map::new(),
            owners: Map::new(),
            handlers: Map::new(),
            projectors: Map::new(),
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
    /// assert_eq!(id1, id2, "intern stability: same name -> same DomainId");
    /// ```
    pub fn intern_named(&mut self, name: &str) -> Result<DomainId, &'static str> {
        let key = NameKey::from_str(name);
        if let Some(&existing) = self.names.get(&key) {
            return Ok(existing);
        }
        let raw = core::num::NonZeroU32::new(self.next_id).ok_or("alloc: DomainId overflow")?;
        self.next_id = self
            .next_id
            .checked_add(1)
            .ok_or("alloc: DomainId overflow")?;
        let id = DomainId::new(raw);

        self.names.try_insert(key, id).map_err(|_| "alloc")?;
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

    /// Registers a manifest for `DomainId`.
    ///
    /// # Errors
    ///
    /// Returns `Err("alloc")` if the backing map allocation fails.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime for manifest fields.
    /// ```
    pub fn register_manifest(&mut self, manifest: DomainManifest) -> Result<(), &'static str> {
        let owner = self
            .owners
            .get(&manifest.owner_cdylib_id)
            .ok_or("router: unknown owner")?;
        owner
            .phase
            .validate_init_only(RowKind::Handler)
            .map_err(registration_error)?;
        if self.manifests.get(&manifest.id).is_some() {
            return Err("router: duplicate manifest");
        }
        self.manifests
            .try_insert(manifest.id, manifest)
            .map_err(|_| "alloc")?;
        Ok(())
    }

    /// Returns the registered manifest for `id`, if any.
    ///
    /// ```rust
    /// use core::num::NonZeroU32;
    /// use reovim_kernel::router::{DomainId, DomainRouter};
    ///
    /// let r = DomainRouter::new();
    /// let id = DomainId::new(NonZeroU32::new(1).unwrap());
    /// assert!(r.manifest(id).is_none());
    /// ```
    #[must_use]
    pub fn manifest(&self, id: DomainId) -> Option<&DomainManifest> {
        self.manifests.get(&id)
    }

    /// Registers or reopens a static owner record in the init phase.
    ///
    /// # Errors
    ///
    /// Returns an error when the same owner id is already present with a
    /// different DT17 owner order.
    pub fn register_static_owner(
        &mut self,
        owner: CdylibId,
        owner_order: u32,
    ) -> Result<(), &'static str> {
        if let Some(record) = self.owners.get_mut(&owner) {
            if record.owner_order != owner_order {
                return Err("router: owner order mismatch");
            }
            record.phase = RegistrationPhase::Init;
            return Ok(());
        }
        self.owners
            .try_insert(owner, StaticOwnerRecord::init(owner, owner_order))
            .map_err(|_| "alloc")?;
        Ok(())
    }

    /// Returns the static owner record, if any.
    #[must_use]
    pub fn owner_record(&self, owner: CdylibId) -> Option<StaticOwnerRecord> {
        self.owners.get(&owner).copied()
    }

    /// Moves a static owner from init to active, closing init-only row gates.
    ///
    /// # Errors
    ///
    /// Returns an error if `owner` has not been registered.
    pub fn activate_owner(&mut self, owner: CdylibId) -> Result<(), &'static str> {
        let record = self.owners.get_mut(&owner).ok_or("router: unknown owner")?;
        record.phase = RegistrationPhase::Active;
        Ok(())
    }

    /// Revokes every manifest/row owned by `owner` and marks the owner failed.
    ///
    /// This is the Phase 4 static unload simulation. It intentionally does not
    /// call into `dlclose` or any filesystem lifecycle.
    ///
    /// # Errors
    ///
    /// Returns an error if `owner` has not been registered.
    pub fn revoke_owner(&mut self, owner: CdylibId) -> Result<usize, &'static str> {
        if self.owners.get(&owner).is_none() {
            return Err("router: unknown owner");
        }
        let removed_rows = self.revoke_handler_rows(owner)? + self.revoke_projector_rows(owner)?;
        self.remove_owner_manifests(owner)?;
        if let Some(record) = self.owners.get_mut(&owner) {
            record.phase = RegistrationPhase::Failed;
        }
        Ok(removed_rows)
    }

    /// Registers the default `OnRawInput` handler for the given `DomainId`.
    ///
    /// Uses the current static owner row with `PriorityBand::Base` default
    /// priority. Full callers that need explicit owner/declaration metadata use
    /// [`register_handler_entry`](Self::register_handler_entry).
    ///
    /// # Errors
    ///
    /// Returns `Err("alloc")` if row storage cannot grow, or a static
    /// registration invariant fails.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator + a concrete handler.
    /// ```
    pub fn register_handler(
        &mut self,
        id: DomainId,
        h: &'static dyn OnRawInputHandler,
    ) -> Result<(), &'static str> {
        let key = (id, HandlerId::OnRawInput);
        let declaration_order = self.next_handler_declaration_order(key)?;
        let entry = HandlerEntry::new(
            id,
            HandlerId::OnRawInput,
            STATIC_OWNER,
            STATIC_OWNER_ORDER,
            declaration_order,
            PriorityBand::Base,
            PriorityBand::Base.default_priority(),
            false,
        )
        .map_err(|_| "router: invalid handler metadata")?;
        self.register_handler_entry(entry, h)
    }

    /// Registers a fully described handler row.
    ///
    /// Rows are kept sorted by DT17: priority descending, owner order
    /// ascending, declaration order ascending.
    ///
    /// # Errors
    ///
    /// Returns `Err("alloc")` if row storage cannot grow.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator + a concrete handler.
    /// ```
    pub fn register_handler_entry(
        &mut self,
        entry: HandlerEntry,
        h: &'static dyn OnRawInputHandler,
    ) -> Result<(), &'static str> {
        self.validate_handler_entry(entry)?;
        let key = (entry.domain_id, entry.handler_id);
        let rows = self.handler_rows_mut(key)?;
        rows.try_push(RegisteredHandler::new(entry, h))
            .map_err(|_| "alloc")?;
        sort_handler_rows(rows);
        Ok(())
    }

    /// Registers the default `Render` projector for the given `DomainId`.
    ///
    /// Uses the current static owner row with `PriorityBand::Base` default
    /// priority. Full callers that need explicit owner/declaration metadata use
    /// [`register_projector_entry`](Self::register_projector_entry).
    ///
    /// # Errors
    ///
    /// Returns `Err("alloc")` if row storage cannot grow, or a static
    /// registration invariant fails.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator + a concrete projector.
    /// ```
    pub fn register_projector(
        &mut self,
        id: DomainId,
        p: &'static dyn RenderProjector,
    ) -> Result<(), &'static str> {
        let key = (id, ProjectorId::Render);
        let declaration_order = self.next_projector_declaration_order(key)?;
        let entry = ProjectorEntry::new(
            id,
            ProjectorId::Render,
            STATIC_OWNER,
            STATIC_OWNER_ORDER,
            declaration_order,
            PriorityBand::Base,
            PriorityBand::Base.default_priority(),
            false,
        )
        .map_err(|_| "router: invalid projector metadata")?;
        self.register_projector_entry(entry, p)
    }

    /// Registers a fully described projector row.
    ///
    /// Rows are kept sorted by DT17: priority descending, owner order
    /// ascending, declaration order ascending.
    ///
    /// # Errors
    ///
    /// Returns `Err("alloc")` if row storage cannot grow.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator + a concrete projector.
    /// ```
    pub fn register_projector_entry(
        &mut self,
        entry: ProjectorEntry,
        p: &'static dyn RenderProjector,
    ) -> Result<(), &'static str> {
        self.validate_projector_entry(entry)?;
        let key = (entry.domain_id, entry.projector_id);
        let rows = self.projector_rows_mut(key)?;
        rows.try_push(RegisteredProjector::new(entry, p))
            .map_err(|_| "alloc")?;
        sort_projector_rows(rows);
        Ok(())
    }

    /// Returns the first registered `OnRawInput` handler for `id`, if any.
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
    pub fn handler(&self, id: DomainId) -> Option<&'static dyn OnRawInputHandler> {
        self.handler_for(id, HandlerId::OnRawInput)
    }

    /// Returns the first registered handler for `(id, handler_id)`, if any.
    ///
    /// ```rust
    /// use core::num::NonZeroU32;
    /// use reovim_kernel::router::{DomainId, DomainRouter, HandlerId};
    ///
    /// let r = DomainRouter::new();
    /// let id = DomainId::new(NonZeroU32::new(1).unwrap());
    /// assert!(r.handler_for(id, HandlerId::OnAttach).is_none());
    /// ```
    #[must_use]
    pub fn handler_for(
        &self,
        id: DomainId,
        handler_id: HandlerId,
    ) -> Option<&'static dyn OnRawInputHandler> {
        self.handler_rows(id, handler_id)
            .and_then(<[RegisteredHandler]>::first)
            .map(RegisteredHandler::callable)
    }

    /// Returns ordered registered handler rows for `(id, handler_id)`.
    ///
    /// ```rust
    /// use core::num::NonZeroU32;
    /// use reovim_kernel::router::{DomainId, DomainRouter, HandlerId};
    ///
    /// let r = DomainRouter::new();
    /// let id = DomainId::new(NonZeroU32::new(1).unwrap());
    /// assert!(r.handler_rows(id, HandlerId::OnRawInput).is_none());
    /// ```
    #[must_use]
    pub fn handler_rows(
        &self,
        id: DomainId,
        handler_id: HandlerId,
    ) -> Option<&[RegisteredHandler]> {
        self.handlers
            .get(&(id, handler_id))
            .map(reovim_arch::ds::Seq::as_slice)
    }

    /// Returns the first registered `Render` projector for `id`, if any.
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
    pub fn projector(&self, id: DomainId) -> Option<&'static dyn RenderProjector> {
        self.projector_for(id, ProjectorId::Render)
    }

    /// Returns the first registered projector for `(id, projector_id)`, if any.
    ///
    /// ```rust
    /// use core::num::NonZeroU32;
    /// use reovim_kernel::router::{DomainId, DomainRouter, ProjectorId};
    ///
    /// let r = DomainRouter::new();
    /// let id = DomainId::new(NonZeroU32::new(1).unwrap());
    /// assert!(r.projector_for(id, ProjectorId::StatusLine).is_none());
    /// ```
    #[must_use]
    pub fn projector_for(
        &self,
        id: DomainId,
        projector_id: ProjectorId,
    ) -> Option<&'static dyn RenderProjector> {
        self.projector_rows(id, projector_id)
            .and_then(<[RegisteredProjector]>::first)
            .map(RegisteredProjector::callable)
    }

    /// Returns ordered registered projector rows for `(id, projector_id)`.
    ///
    /// ```rust
    /// use core::num::NonZeroU32;
    /// use reovim_kernel::router::{DomainId, DomainRouter, ProjectorId};
    ///
    /// let r = DomainRouter::new();
    /// let id = DomainId::new(NonZeroU32::new(1).unwrap());
    /// assert!(r.projector_rows(id, ProjectorId::Render).is_none());
    /// ```
    #[must_use]
    pub fn projector_rows(
        &self,
        id: DomainId,
        projector_id: ProjectorId,
    ) -> Option<&[RegisteredProjector]> {
        self.projectors
            .get(&(id, projector_id))
            .map(reovim_arch::ds::Seq::as_slice)
    }

    fn handler_rows_mut(
        &mut self,
        key: (DomainId, HandlerId),
    ) -> Result<&mut Seq<RegisteredHandler>, &'static str> {
        if self.handlers.get(&key).is_none() {
            self.handlers
                .try_insert(key, Seq::new())
                .map_err(|_| "alloc")?;
        }
        self.handlers
            .get_mut(&key)
            .ok_or("router: handler row allocation missing")
    }

    fn projector_rows_mut(
        &mut self,
        key: (DomainId, ProjectorId),
    ) -> Result<&mut Seq<RegisteredProjector>, &'static str> {
        if self.projectors.get(&key).is_none() {
            self.projectors
                .try_insert(key, Seq::new())
                .map_err(|_| "alloc")?;
        }
        self.projectors
            .get_mut(&key)
            .ok_or("router: projector row allocation missing")
    }

    fn next_handler_declaration_order(
        &mut self,
        key: (DomainId, HandlerId),
    ) -> Result<u32, &'static str> {
        let rows = self.handler_rows_mut(key)?;
        u32::try_from(rows.len()).map_err(|_| "router: too many handler rows")
    }

    fn next_projector_declaration_order(
        &mut self,
        key: (DomainId, ProjectorId),
    ) -> Result<u32, &'static str> {
        let rows = self.projector_rows_mut(key)?;
        u32::try_from(rows.len()).map_err(|_| "router: too many projector rows")
    }

    fn validate_handler_entry(&self, entry: HandlerEntry) -> Result<(), &'static str> {
        let manifest = self
            .manifests
            .get(&entry.domain_id)
            .ok_or("router: missing manifest")?;
        let owner = self
            .owners
            .get(&entry.meta.owner_cdylib_id)
            .ok_or("router: unknown owner")?;
        owner
            .phase
            .validate_init_only(RowKind::Handler)
            .map_err(registration_error)?;
        validate_owner_metadata(owner, entry.meta.owner_order)?;
        let declared = manifest.declares_handler(entry.handler_id);
        if !declared {
            manifest
                .validate_owner(entry.meta.owner_cdylib_id)
                .map_err(registration_error)?;
        }
        validate_manifest_late_flag(declared, entry.meta.late_registered)
    }

    fn validate_projector_entry(&self, entry: ProjectorEntry) -> Result<(), &'static str> {
        let manifest = self
            .manifests
            .get(&entry.domain_id)
            .ok_or("router: missing manifest")?;
        let owner = self
            .owners
            .get(&entry.meta.owner_cdylib_id)
            .ok_or("router: unknown owner")?;
        owner
            .phase
            .validate_init_only(RowKind::Projector)
            .map_err(registration_error)?;
        validate_owner_metadata(owner, entry.meta.owner_order)?;
        let declared = manifest.declares_projector(entry.projector_id);
        if !declared {
            manifest
                .validate_owner(entry.meta.owner_cdylib_id)
                .map_err(registration_error)?;
        }
        validate_manifest_late_flag(declared, entry.meta.late_registered)
    }

    fn revoke_handler_rows(&mut self, owner: CdylibId) -> Result<usize, &'static str> {
        let mut keys = Seq::new();
        for (key, rows) in &self.handlers {
            if rows
                .iter()
                .any(|row| row.entry.meta.owner_cdylib_id == owner)
            {
                keys.try_push(*key).map_err(|_| "alloc")?;
            }
        }

        let mut removed = 0usize;
        for key in keys.as_slice() {
            let became_empty = self.handlers.get_mut(key).is_some_and(|rows| {
                removed += compact_handler_rows(rows, owner);
                rows.is_empty()
            });
            if became_empty {
                self.handlers.remove(key);
            }
        }
        Ok(removed)
    }

    fn revoke_projector_rows(&mut self, owner: CdylibId) -> Result<usize, &'static str> {
        let mut keys = Seq::new();
        for (key, rows) in &self.projectors {
            if rows
                .iter()
                .any(|row| row.entry.meta.owner_cdylib_id == owner)
            {
                keys.try_push(*key).map_err(|_| "alloc")?;
            }
        }

        let mut removed = 0usize;
        for key in keys.as_slice() {
            let became_empty = self.projectors.get_mut(key).is_some_and(|rows| {
                removed += compact_projector_rows(rows, owner);
                rows.is_empty()
            });
            if became_empty {
                self.projectors.remove(key);
            }
        }
        Ok(removed)
    }

    fn remove_owner_manifests(&mut self, owner: CdylibId) -> Result<(), &'static str> {
        let mut ids = Seq::new();
        for (id, manifest) in &self.manifests {
            if manifest.owner_cdylib_id == owner {
                ids.try_push(*id).map_err(|_| "alloc")?;
            }
        }
        for id in ids.as_slice() {
            self.manifests.remove(id);
        }
        Ok(())
    }
}

impl Default for DomainRouter {
    fn default() -> Self {
        Self::new()
    }
}

fn sort_handler_rows(rows: &mut Seq<RegisteredHandler>) {
    let slice = rows.as_mut_slice();
    let mut idx = slice.len().saturating_sub(1);
    while idx > 0 && handler_before(&slice[idx], &slice[idx - 1]) {
        slice.swap(idx, idx - 1);
        idx -= 1;
    }
}

fn sort_projector_rows(rows: &mut Seq<RegisteredProjector>) {
    let slice = rows.as_mut_slice();
    let mut idx = slice.len().saturating_sub(1);
    while idx > 0 && projector_before(&slice[idx], &slice[idx - 1]) {
        slice.swap(idx, idx - 1);
        idx -= 1;
    }
}

const fn handler_before(a: &RegisteredHandler, b: &RegisteredHandler) -> bool {
    a.entry.meta.sorts_before(b.entry.meta)
}

const fn projector_before(a: &RegisteredProjector, b: &RegisteredProjector) -> bool {
    a.entry.meta.sorts_before(b.entry.meta)
}

const fn validate_owner_metadata(
    owner: &StaticOwnerRecord,
    row_owner_order: u32,
) -> Result<(), &'static str> {
    if owner.owner_order == row_owner_order {
        Ok(())
    } else {
        Err("router: owner order mismatch")
    }
}

const fn validate_manifest_late_flag(
    declared: bool,
    late_registered: bool,
) -> Result<(), &'static str> {
    match (declared, late_registered) {
        (true, false) | (false, true) => Ok(()),
        (true, true) => Err("router: declared row marked late"),
        (false, false) => Err("router: undeclared row missing late flag"),
    }
}

const fn registration_error(err: RegistrationError) -> &'static str {
    match err {
        RegistrationError::PriorityOutOfBand { .. } => "router: priority out of band",
        RegistrationError::PostInitRegistration { .. } => "router: post-init registration",
        RegistrationError::OwnerMismatch { .. } => "router: owner mismatch",
    }
}

fn compact_handler_rows(rows: &mut Seq<RegisteredHandler>, owner: CdylibId) -> usize {
    let len = rows.len();
    let mut write = 0usize;
    {
        let slice = rows.as_mut_slice();
        let mut read = 0usize;
        while read < len {
            if slice[read].entry.meta.owner_cdylib_id != owner {
                if write != read {
                    slice.swap(write, read);
                }
                write += 1;
            }
            read += 1;
        }
    }
    while rows.len() > write {
        let _ = rows.pop();
    }
    len - write
}

fn compact_projector_rows(rows: &mut Seq<RegisteredProjector>, owner: CdylibId) -> usize {
    let len = rows.len();
    let mut write = 0usize;
    {
        let slice = rows.as_mut_slice();
        let mut read = 0usize;
        while read < len {
            if slice[read].entry.meta.owner_cdylib_id != owner {
                if write != read {
                    slice.swap(write, read);
                }
                write += 1;
            }
            read += 1;
        }
    }
    while rows.len() > write {
        let _ = rows.pop();
    }
    len - write
}

// `DomainRouter` is auto-`Send + Sync`: its `Map`/`Seq` fields carry the
// arch-DS bounded impls (exclusive bucket ownership, Vec-analogy), and the
// handler/projector rows hold `'static` trait objects with `Send + Sync`.
// No manual impl — the bounds must come from the fields so a future non-`Send`
// field is caught by the compiler, not papered over.

// L12 layout: tests in sibling router_tests.rs, declared in lib.rs.
