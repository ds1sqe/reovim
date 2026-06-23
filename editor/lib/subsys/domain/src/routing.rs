//! Domain routing contract shapes (§4.1, #798).
//!
//! This module contains the in-process data contracts for manifests,
//! handler/projector identifiers, DT10 priority validation, and DT17
//! deterministic ordering metadata. EditorCore-owned maps and dispatch loops live
//! in `reovim-editor-core`; these types are the shared surface those maps store.

use reovim_lib_ds::{Bytes, Seq};

use crate::id::{CdylibId, DomainId};

/// Handler kind registered under a [`DomainId`] (§4.1 §5).
///
/// ```rust
/// use reovim_subsys_domain::routing::HandlerId;
///
/// assert_eq!(HandlerId::OnRawInput.name(), "on_raw_input");
/// assert_eq!(HandlerId::custom(7), HandlerId::Custom(7));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HandlerId {
    /// Raw input handler used by focus dispatch.
    OnRawInput,
    /// Attachment initialization handler.
    OnAttach,
    /// Attachment teardown handler.
    OnDetach,
    /// Persistence-save handler.
    OnPersistSave,
    /// Persistence-load handler.
    OnPersistLoad,
    /// Positional-argument handler (AL10).
    OnPositionalArg,
    /// Module-defined handler kind scoped by owner cdylib.
    Custom(u32),
}

impl HandlerId {
    /// Builds a module-defined handler id.
    ///
    /// ```rust
    /// use reovim_subsys_domain::routing::HandlerId;
    ///
    /// assert_eq!(HandlerId::custom(3), HandlerId::Custom(3));
    /// ```
    #[must_use]
    pub const fn custom(raw: u32) -> Self {
        Self::Custom(raw)
    }

    /// Stable diagnostic name for built-in handler kinds.
    ///
    /// ```rust
    /// use reovim_subsys_domain::routing::HandlerId;
    ///
    /// assert_eq!(HandlerId::OnDetach.name(), "on_detach");
    /// assert_eq!(HandlerId::Custom(9).name(), "custom");
    /// ```
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::OnRawInput => "on_raw_input",
            Self::OnAttach => "on_attach",
            Self::OnDetach => "on_detach",
            Self::OnPersistSave => "on_persist_save",
            Self::OnPersistLoad => "on_persist_load",
            Self::OnPositionalArg => "on_positional_arg",
            Self::Custom(_) => "custom",
        }
    }
}

/// Projector kind registered under a [`DomainId`] (§4.1 §5).
///
/// ```rust
/// use reovim_subsys_domain::routing::ProjectorId;
///
/// assert_eq!(ProjectorId::Render.name(), "render");
/// assert_eq!(ProjectorId::custom(4), ProjectorId::Custom(4));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProjectorId {
    /// Main buffer/window projection.
    Render,
    /// Status-line projection.
    StatusLine,
    /// Module-defined projector kind scoped by owner cdylib.
    Custom(u32),
}

impl ProjectorId {
    /// Builds a module-defined projector id.
    ///
    /// ```rust
    /// use reovim_subsys_domain::routing::ProjectorId;
    ///
    /// assert_eq!(ProjectorId::custom(8), ProjectorId::Custom(8));
    /// ```
    #[must_use]
    pub const fn custom(raw: u32) -> Self {
        Self::Custom(raw)
    }

    /// Stable diagnostic name for built-in projector kinds.
    ///
    /// ```rust
    /// use reovim_subsys_domain::routing::ProjectorId;
    ///
    /// assert_eq!(ProjectorId::StatusLine.name(), "status_line");
    /// assert_eq!(ProjectorId::Custom(1).name(), "custom");
    /// ```
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Render => "render",
            Self::StatusLine => "status_line",
            Self::Custom(_) => "custom",
        }
    }
}

/// DT10 priority band name.
///
/// ```rust
/// use reovim_subsys_domain::routing::PriorityBand;
///
/// assert!(PriorityBand::Pre.contains(950));
/// assert_eq!(PriorityBand::Decor.default_priority(), 200);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PriorityBand {
    /// 900..=999: hooks before base behavior.
    Pre,
    /// 500..=899: default/base behavior.
    Base,
    /// 300..=499: adapters and shims.
    Adapt,
    /// 100..=299: decoration and view-only effects.
    Decor,
    /// 0..=99: observation and cleanup.
    Post,
}

impl PriorityBand {
    /// Inclusive numeric range for this band.
    ///
    /// ```rust
    /// use reovim_subsys_domain::routing::{PriorityBand, PriorityRange};
    ///
    /// assert_eq!(PriorityBand::Base.range(), PriorityRange { min: 500, max: 899 });
    /// ```
    #[must_use]
    pub const fn range(self) -> PriorityRange {
        match self {
            Self::Pre => PriorityRange { min: 900, max: 999 },
            Self::Base => PriorityRange { min: 500, max: 899 },
            Self::Adapt => PriorityRange { min: 300, max: 499 },
            Self::Decor => PriorityRange { min: 100, max: 299 },
            Self::Post => PriorityRange { min: 0, max: 99 },
        }
    }

    /// Default priority assigned when a manifest names only a band.
    ///
    /// ```rust
    /// use reovim_subsys_domain::routing::PriorityBand;
    ///
    /// assert_eq!(PriorityBand::Pre.default_priority(), 950);
    /// assert_eq!(PriorityBand::Post.default_priority(), 50);
    /// ```
    #[must_use]
    pub const fn default_priority(self) -> u32 {
        match self {
            Self::Pre => 950,
            Self::Base => 700,
            Self::Adapt => 400,
            Self::Decor => 200,
            Self::Post => 50,
        }
    }

    /// Returns whether `priority` is inside this band's DT10 range.
    ///
    /// ```rust
    /// use reovim_subsys_domain::routing::PriorityBand;
    ///
    /// assert!(PriorityBand::Adapt.contains(300));
    /// assert!(!PriorityBand::Decor.contains(950));
    /// ```
    #[must_use]
    pub const fn contains(self, priority: u32) -> bool {
        let range = self.range();
        priority >= range.min && priority <= range.max
    }
}

/// Inclusive DT10 priority range.
///
/// ```rust
/// use reovim_subsys_domain::routing::PriorityRange;
///
/// let range = PriorityRange { min: 1, max: 2 };
/// assert!(range.contains(2));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PriorityRange {
    /// Lowest accepted priority.
    pub min: u32,
    /// Highest accepted priority.
    pub max: u32,
}

impl PriorityRange {
    /// Returns whether `priority` lies inside the inclusive range.
    ///
    /// ```rust
    /// use reovim_subsys_domain::routing::PriorityRange;
    ///
    /// assert!(PriorityRange { min: 3, max: 5 }.contains(4));
    /// ```
    #[must_use]
    pub const fn contains(self, priority: u32) -> bool {
        priority >= self.min && priority <= self.max
    }
}

/// Priority plus the named band that validated it.
///
/// ```rust
/// use reovim_subsys_domain::routing::{BandedPriority, PriorityBand};
///
/// let p = BandedPriority::new(PriorityBand::Base, 700).unwrap();
/// assert_eq!(p.priority, 700);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BandedPriority {
    /// Manifest-declared band.
    pub band: PriorityBand,
    /// Explicit or default priority.
    pub priority: u32,
}

impl BandedPriority {
    /// Validates `priority` against `band`.
    ///
    /// # Errors
    ///
    /// Returns [`RegistrationError::PriorityOutOfBand`] when `priority` is
    /// outside the named band's DT10 range.
    ///
    /// ```rust
    /// use reovim_subsys_domain::routing::{BandedPriority, PriorityBand};
    ///
    /// assert!(BandedPriority::new(PriorityBand::Decor, 950).is_err());
    /// ```
    pub const fn new(band: PriorityBand, priority: u32) -> Result<Self, RegistrationError> {
        if band.contains(priority) {
            Ok(Self { band, priority })
        } else {
            Err(RegistrationError::PriorityOutOfBand { band, priority })
        }
    }

    /// Builds the band's default priority.
    ///
    /// ```rust
    /// use reovim_subsys_domain::routing::{BandedPriority, PriorityBand};
    ///
    /// let p = BandedPriority::default_for(PriorityBand::Post);
    /// assert_eq!(p.priority, 50);
    /// ```
    #[must_use]
    pub const fn default_for(band: PriorityBand) -> Self {
        Self {
            band,
            priority: band.default_priority(),
        }
    }
}

/// Participant registration phase for init-only row gates (§4.1 §4.1).
///
/// ```rust
/// use reovim_subsys_domain::routing::{RegistrationPhase, RowKind};
///
/// assert!(RegistrationPhase::Init.validate_init_only(RowKind::Handler).is_ok());
/// assert!(RegistrationPhase::Active.validate_init_only(RowKind::Handler).is_err());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RegistrationPhase {
    /// Participant init is still running; v0.16 row registration is allowed.
    Init,
    /// Participant is active; v0.16 init-only rows are closed.
    Active,
    /// Participant failed; no row registration is accepted.
    Failed,
}

impl RegistrationPhase {
    /// Validates that an init-only row may be registered in this phase.
    ///
    /// # Errors
    ///
    /// Returns [`RegistrationError::PostInitRegistration`] unless the phase is
    /// [`RegistrationPhase::Init`].
    ///
    /// ```rust
    /// use reovim_subsys_domain::routing::{RegistrationPhase, RowKind};
    ///
    /// assert!(RegistrationPhase::Failed.validate_init_only(RowKind::CursorCodec).is_err());
    /// ```
    pub const fn validate_init_only(self, row_kind: RowKind) -> Result<(), RegistrationError> {
        match self {
            Self::Init => Ok(()),
            Self::Active | Self::Failed => {
                Err(RegistrationError::PostInitRegistration { row_kind })
            }
        }
    }
}

/// Row kind used by registration diagnostics.
///
/// ```rust
/// use reovim_subsys_domain::routing::RowKind;
///
/// assert_eq!(RowKind::Projector.name(), "projector");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RowKind {
    /// Handler row.
    Handler,
    /// Projector row.
    Projector,
    /// Position codec row.
    PositionCodec,
    /// Cursor codec row.
    CursorCodec,
}

impl RowKind {
    /// Stable diagnostic name for this row kind.
    ///
    /// ```rust
    /// use reovim_subsys_domain::routing::RowKind;
    ///
    /// assert_eq!(RowKind::PositionCodec.name(), "position_codec");
    /// ```
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Handler => "handler",
            Self::Projector => "projector",
            Self::PositionCodec => "position_codec",
            Self::CursorCodec => "cursor_codec",
        }
    }
}

/// Registration failure for manifest or row validation.
///
/// ```rust
/// use reovim_subsys_domain::routing::{PriorityBand, RegistrationError};
///
/// let err = RegistrationError::PriorityOutOfBand {
///     band: PriorityBand::Decor,
///     priority: 950,
/// };
/// assert!(matches!(err, RegistrationError::PriorityOutOfBand { .. }));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistrationError {
    /// Priority was outside the named DT10 band.
    PriorityOutOfBand {
        /// Manifest-declared band.
        band: PriorityBand,
        /// Rejected priority.
        priority: u32,
    },
    /// Row registration was attempted after the init window closed.
    PostInitRegistration {
        /// Kind of row that was rejected.
        row_kind: RowKind,
    },
    /// A cdylib attempted to register a row for a Domain it does not own.
    OwnerMismatch {
        /// Manifest owner.
        expected: CdylibId,
        /// Row owner.
        actual: CdylibId,
    },
}

/// Shared metadata carried by handler and projector dispatch rows.
///
/// ```rust
/// use core::num::NonZeroU32;
/// use reovim_subsys_domain::{id::CdylibId, routing::{DispatchEntryMeta, PriorityBand}};
///
/// let owner = CdylibId::new(NonZeroU32::new(1).unwrap());
/// let meta = DispatchEntryMeta::new(owner, 0, 2, PriorityBand::Base, 700, false).unwrap();
/// assert_eq!(meta.priority.priority, 700);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DispatchEntryMeta {
    /// Owner cdylib for row lifetime/revocation.
    pub owner_cdylib_id: CdylibId,
    /// Canonical owner order for DT17 across-owner tie-breaks.
    pub owner_order: u32,
    /// Manifest declaration order within `owner_cdylib_id`.
    pub declaration_order: u32,
    /// Validated DT10 band/priority.
    pub priority: BandedPriority,
    /// Whether this same-owner row was registered during init but not declared
    /// in the manifest.
    pub late_registered: bool,
}

impl DispatchEntryMeta {
    /// Builds row metadata after validating the DT10 band/priority pair.
    ///
    /// # Errors
    ///
    /// Returns [`RegistrationError::PriorityOutOfBand`] when `priority` falls
    /// outside `band`.
    ///
    /// ```rust
    /// use core::num::NonZeroU32;
    /// use reovim_subsys_domain::{id::CdylibId, routing::{DispatchEntryMeta, PriorityBand}};
    ///
    /// let owner = CdylibId::new(NonZeroU32::new(2).unwrap());
    /// assert!(DispatchEntryMeta::new(owner, 0, 0, PriorityBand::Decor, 950, false).is_err());
    /// ```
    pub fn new(
        owner_cdylib_id: CdylibId,
        owner_order: u32,
        declaration_order: u32,
        band: PriorityBand,
        priority: u32,
        late_registered: bool,
    ) -> Result<Self, RegistrationError> {
        Ok(Self {
            owner_cdylib_id,
            owner_order,
            declaration_order,
            priority: BandedPriority::new(band, priority)?,
            late_registered,
        })
    }

    /// Builds row metadata using the band's default priority.
    ///
    /// ```rust
    /// use core::num::NonZeroU32;
    /// use reovim_subsys_domain::{id::CdylibId, routing::{DispatchEntryMeta, PriorityBand}};
    ///
    /// let owner = CdylibId::new(NonZeroU32::new(3).unwrap());
    /// let meta = DispatchEntryMeta::default_priority(owner, 1, 2, PriorityBand::Pre, true);
    /// assert_eq!(meta.priority.priority, 950);
    /// ```
    #[must_use]
    pub const fn default_priority(
        owner_cdylib_id: CdylibId,
        owner_order: u32,
        declaration_order: u32,
        band: PriorityBand,
        late_registered: bool,
    ) -> Self {
        Self {
            owner_cdylib_id,
            owner_order,
            declaration_order,
            priority: BandedPriority::default_for(band),
            late_registered,
        }
    }

    /// Returns true when `self` dispatches before `other` by DT17.
    ///
    /// ```rust
    /// use core::num::NonZeroU32;
    /// use reovim_subsys_domain::{id::CdylibId, routing::{DispatchEntryMeta, PriorityBand}};
    ///
    /// let owner = CdylibId::new(NonZeroU32::new(1).unwrap());
    /// let high = DispatchEntryMeta::new(owner, 0, 0, PriorityBand::Pre, 950, false).unwrap();
    /// let low = DispatchEntryMeta::new(owner, 0, 0, PriorityBand::Base, 700, false).unwrap();
    /// assert!(high.sorts_before(low));
    /// ```
    #[must_use]
    pub const fn sorts_before(self, other: Self) -> bool {
        if self.priority.priority != other.priority.priority {
            return self.priority.priority > other.priority.priority;
        }
        if self.owner_order != other.owner_order {
            return self.owner_order < other.owner_order;
        }
        self.declaration_order < other.declaration_order
    }
}

/// Handler dispatch row identity plus shared row metadata.
///
/// ```rust
/// use core::num::NonZeroU32;
/// use reovim_subsys_domain::{
///     id::{CdylibId, DomainId},
///     routing::{HandlerEntry, HandlerId, PriorityBand},
/// };
///
/// let owner = CdylibId::new(NonZeroU32::new(1).unwrap());
/// let domain = DomainId::new(NonZeroU32::new(2).unwrap());
/// let row = HandlerEntry::new(domain, HandlerId::OnRawInput, owner, 0, 0, PriorityBand::Base, 700, false).unwrap();
/// assert_eq!(row.handler_id, HandlerId::OnRawInput);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HandlerEntry {
    /// Domain under which this handler is registered.
    pub domain_id: DomainId,
    /// Handler kind.
    pub handler_id: HandlerId,
    /// Shared dispatch metadata.
    pub meta: DispatchEntryMeta,
}

impl HandlerEntry {
    /// Builds a handler row with validated DT10 metadata.
    ///
    /// # Errors
    ///
    /// Returns [`RegistrationError::PriorityOutOfBand`] when `priority` falls
    /// outside `band`.
    #[expect(
        clippy::too_many_arguments,
        reason = "contract row mirrors manifest columns"
    )]
    pub fn new(
        domain_id: DomainId,
        handler_id: HandlerId,
        owner_cdylib_id: CdylibId,
        owner_order: u32,
        declaration_order: u32,
        band: PriorityBand,
        priority: u32,
        late_registered: bool,
    ) -> Result<Self, RegistrationError> {
        Ok(Self {
            domain_id,
            handler_id,
            meta: DispatchEntryMeta::new(
                owner_cdylib_id,
                owner_order,
                declaration_order,
                band,
                priority,
                late_registered,
            )?,
        })
    }
}

/// Projector dispatch row identity plus shared row metadata.
///
/// ```rust
/// use core::num::NonZeroU32;
/// use reovim_subsys_domain::{
///     id::{CdylibId, DomainId},
///     routing::{PriorityBand, ProjectorEntry, ProjectorId},
/// };
///
/// let owner = CdylibId::new(NonZeroU32::new(1).unwrap());
/// let domain = DomainId::new(NonZeroU32::new(2).unwrap());
/// let row = ProjectorEntry::new(domain, ProjectorId::Render, owner, 0, 0, PriorityBand::Base, 700, false).unwrap();
/// assert_eq!(row.projector_id, ProjectorId::Render);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectorEntry {
    /// Domain under which this projector is registered.
    pub domain_id: DomainId,
    /// Projector kind.
    pub projector_id: ProjectorId,
    /// Shared dispatch metadata.
    pub meta: DispatchEntryMeta,
}

impl ProjectorEntry {
    /// Builds a projector row with validated DT10 metadata.
    ///
    /// # Errors
    ///
    /// Returns [`RegistrationError::PriorityOutOfBand`] when `priority` falls
    /// outside `band`.
    #[expect(
        clippy::too_many_arguments,
        reason = "contract row mirrors manifest columns"
    )]
    pub fn new(
        domain_id: DomainId,
        projector_id: ProjectorId,
        owner_cdylib_id: CdylibId,
        owner_order: u32,
        declaration_order: u32,
        band: PriorityBand,
        priority: u32,
        late_registered: bool,
    ) -> Result<Self, RegistrationError> {
        Ok(Self {
            domain_id,
            projector_id,
            meta: DispatchEntryMeta::new(
                owner_cdylib_id,
                owner_order,
                declaration_order,
                band,
                priority,
                late_registered,
            )?,
        })
    }
}

/// Domain API version advertised by a manifest.
///
/// ```rust
/// use reovim_subsys_domain::routing::DomainApiVersion;
///
/// assert_eq!(DomainApiVersion::new(0, 16).minor, 16);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DomainApiVersion {
    /// Major API version.
    pub major: u16,
    /// Minor API version.
    pub minor: u16,
}

impl DomainApiVersion {
    /// Builds an API version.
    ///
    /// ```rust
    /// use reovim_subsys_domain::routing::DomainApiVersion;
    ///
    /// assert_eq!(DomainApiVersion::new(1, 2).major, 1);
    /// ```
    #[must_use]
    pub const fn new(major: u16, minor: u16) -> Self {
        Self { major, minor }
    }
}

/// Manifest data published during participant init (§4.1 §4).
///
/// ```rust,no_run
/// // no_run: requires arch allocator runtime for the manifest name.
/// ```
pub struct DomainManifest {
    /// Domain id allocated by the router.
    pub id: DomainId,
    /// Stable Domain name bytes.
    pub name: Bytes,
    /// Owning cdylib id.
    pub owner_cdylib_id: CdylibId,
    /// Contract API version.
    pub api_version: DomainApiVersion,
    /// Manifest-declared handler kinds.
    pub handler_kinds: Seq<HandlerId>,
    /// Manifest-declared projector kinds.
    pub projector_kinds: Seq<ProjectorId>,
    /// Manifest-declared position codec inner ids.
    pub position_codecs: Seq<u16>,
    /// Manifest-declared cursor codec inner ids.
    pub cursor_codecs: Seq<u16>,
    /// Whether this manifest declares a positional-argument handler.
    pub has_positional_args: bool,
}

impl DomainManifest {
    /// Builds an empty manifest. Declaration sequences can be filled by the
    /// manifest loader before registration.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime for Bytes::try_from_slice.
    /// ```
    #[must_use]
    pub const fn new(
        id: DomainId,
        name: Bytes,
        owner_cdylib_id: CdylibId,
        api_version: DomainApiVersion,
    ) -> Self {
        Self {
            id,
            name,
            owner_cdylib_id,
            api_version,
            handler_kinds: Seq::new(),
            projector_kinds: Seq::new(),
            position_codecs: Seq::new(),
            cursor_codecs: Seq::new(),
            has_positional_args: false,
        }
    }

    /// Returns whether `handler_id` was declared by the manifest.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime.
    /// ```
    #[must_use]
    pub fn declares_handler(&self, handler_id: HandlerId) -> bool {
        self.handler_kinds.iter().any(|&id| id == handler_id)
    }

    /// Returns whether `projector_id` was declared by the manifest.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime.
    /// ```
    #[must_use]
    pub fn declares_projector(&self, projector_id: ProjectorId) -> bool {
        self.projector_kinds.iter().any(|&id| id == projector_id)
    }

    /// Returns whether `inner_id` was declared for position carriers.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime.
    /// ```
    #[must_use]
    pub fn declares_position_codec(&self, inner_id: u16) -> bool {
        self.position_codecs.iter().any(|&id| id == inner_id)
    }

    /// Returns whether `inner_id` was declared for cursor carriers.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime.
    /// ```
    #[must_use]
    pub fn declares_cursor_codec(&self, inner_id: u16) -> bool {
        self.cursor_codecs.iter().any(|&id| id == inner_id)
    }

    /// Validates that `row_owner` is the manifest owner.
    ///
    /// # Errors
    ///
    /// Returns [`RegistrationError::OwnerMismatch`] when `row_owner` differs
    /// from [`DomainManifest::owner_cdylib_id`].
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime.
    /// ```
    pub fn validate_owner(&self, row_owner: CdylibId) -> Result<(), RegistrationError> {
        if self.owner_cdylib_id == row_owner {
            Ok(())
        } else {
            Err(RegistrationError::OwnerMismatch {
                expected: self.owner_cdylib_id,
                actual: row_owner,
            })
        }
    }
}

// L12 layout: tests in sibling routing_tests.rs, declared in lib.rs.
