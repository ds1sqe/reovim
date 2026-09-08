//! Selected client-local meanings, not a Kernel coordinate or renderer enum.

use core::fmt;

use reovim_kernel_interface::buffer::BufferStateStamp;
use reovim_kernel_interface::operation::ContractSelection;
use reovim_kernel_interface::resource::state::ResourceStateModelBinding;
use reovim_kernel_interface::semantic::stamp::{
    CoordinateSpaceStamp, ProjectionStamp, RepresentationPolicyStamp,
};
use reovim_resource_local::{Key, profile};

mod policy;
mod translation;

/// Bounded, disjoint native record identities; never two text endpoints.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeySet {
    alpha: bool,
    omega: bool,
}

impl KeySet {
    /// Selects one named record.
    #[must_use]
    pub const fn one(key: Key) -> Self {
        Self {
            alpha: matches!(key, Key::Alpha),
            omega: matches!(key, Key::Omega),
        }
    }

    /// Selects both disjoint named records.
    #[must_use]
    pub const fn both() -> Self {
        Self { alpha: true, omega: true }
    }

    /// Tests semantic set membership without interpreting an ordinal offset.
    #[must_use]
    pub const fn contains(self, key: Key) -> bool {
        match key {
            Key::Alpha => self.alpha,
            Key::Omega => self.omega,
        }
    }

    const fn is_subset_of(self, other: Self) -> bool {
        (!self.alpha || other.alpha) && (!self.omega || other.omega)
    }
}

/// Local response to a matched accepted change and semantic snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResponsePolicy {
    /// Revalidate explicit correspondence before advancing the selection.
    Follow,
    /// Keep the original qualified value; expose stale correspondence.
    Keep,
}

/// Visible correspondence state of a retained local selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectionStatus {
    /// Its complete binding matches the currently observed snapshot.
    Current,
    /// Explicit revalidation is required before a selection-derived edit.
    Stale,
    /// The selected meaning has no representation in the observed snapshot.
    Unrepresentable,
}

/// Typed local policy failure, independent of Kernel editing acceptance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectionError {
    /// No local selection exists.
    Missing,
    /// Qualified source, model, coordinate or Projection is no longer current.
    Stale,
    /// The selected meaning has no target correspondence.
    Unrepresentable,
    /// No selected translator or native family supports this schema.
    Unsupported,
    /// Omitted meaning requires explicit consent.
    PartialNeedsConsent,
    /// The requested action names a part outside the local selection.
    OutsideSelection,
    /// The encoded snapshot does not satisfy the selected client schema.
    InvalidSnapshot,
}

impl fmt::Display for SelectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "local selection: {self:?}")
    }
}
impl core::error::Error for SelectionError {}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Binding {
    source: BufferStateStamp,
    model: ResourceStateModelBinding,
    representation: RepresentationPolicyStamp,
    coordinate: CoordinateSpaceStamp,
    projection: ProjectionStamp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Value {
    Keys(KeySet),
    HeadingLabel,
}

/// Immutable client-owned selection with exact schema and observation binding.
/// Constructors resolve only selected concrete policies, not universal casts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QualifiedSelection {
    schema: ContractSelection,
    binding: Binding,
    value: Value,
}

impl QualifiedSelection {
    /// Returns the explicit local selection schema.
    #[must_use]
    pub const fn schema(&self) -> ContractSelection {
        self.schema
    }

    /// Returns the exact historical/current observation qualifying this value.
    #[must_use]
    pub const fn source(&self) -> BufferStateStamp {
        self.binding.source
    }

    /// Returns the observed Projection, including its generation.
    #[must_use]
    pub const fn projection(&self) -> ProjectionStamp {
        self.binding.projection
    }

    /// Returns the selected coordinate-space incarnation.
    #[must_use]
    pub const fn coordinate(&self) -> CoordinateSpaceStamp {
        self.binding.coordinate
    }

    /// Returns selected native identities for a keyed selection.
    #[must_use]
    pub const fn keys(&self) -> Option<KeySet> {
        match self.value {
            Value::Keys(keys) => Some(keys),
            Value::HeadingLabel => None,
        }
    }
}

/// Semantic classification of a selected local translation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TranslationStatus {
    /// Every selected part has an exact correspondence.
    Exact,
    /// Some selected meaning would be omitted.
    Partial,
    /// No selected part has a target correspondence.
    Unrepresentable,
    /// This schema pair has no selected translator.
    Unsupported,
    /// The source correspondence is no longer current.
    Stale,
}

/// Explicit choice required before accepting a partial local translation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PartialAcceptance {
    /// Preserve the original selection and report omitted meaning.
    Reject,
    /// Accept the specifically reported omissions.
    Accept,
}

/// Bounded translation evidence; it owns no authoritative editing state.
pub struct SelectionTranslation {
    status: TranslationStatus,
    original: Option<QualifiedSelection>,
    selected: Option<QualifiedSelection>,
    omitted: Option<KeySet>,
}

impl SelectionTranslation {
    /// Returns whether correspondence is exact, partial or unavailable.
    #[must_use]
    pub const fn status(&self) -> TranslationStatus {
        self.status
    }

    /// Borrows the proposed translated value without changing client policy.
    #[must_use]
    pub const fn selected(&self) -> Option<&QualifiedSelection> {
        self.selected.as_ref()
    }

    /// Returns precisely the named parts that would be omitted.
    #[must_use]
    pub const fn omitted(&self) -> Option<KeySet> {
        self.omitted
    }
}

/// Schema for a multipart selection of native keys.
#[must_use]
pub fn keyed_schema() -> ContractSelection {
    profile::schema(0x9000)
}

/// Distinct local schema that can express Alpha but not Omega.
#[must_use]
pub fn alpha_schema() -> ContractSelection {
    profile::schema(0x9001)
}

/// Schema for the selected heading-label meaning, without a screen position.
#[must_use]
pub fn heading_schema() -> ContractSelection {
    profile::schema(0x9002)
}
