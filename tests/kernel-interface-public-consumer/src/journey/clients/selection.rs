use alloc::collections::BTreeSet;

use reovim_kernel_interface::buffer::BufferStateStamp;
use reovim_kernel_interface::interaction::descriptor;
use reovim_kernel_interface::operation::ContractSelection;
use reovim_kernel_interface::semantic::stamp::{
    CoordinateSpaceStamp, RepresentationPolicyStamp,
};

use super::{Failure, Key, KeyedState};
use crate::first_edit::native::support;

/// A concrete disjoint keyed selection, not a pair of text endpoints.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeySelection {
    pub schema: ContractSelection,
    pub coordinate: CoordinateSpaceStamp,
    pub representation: RepresentationPolicyStamp,
    pub source: BufferStateStamp,
    pub keys: BTreeSet<Key>,
}

/// A distinct local schema can express Alpha, but cannot express Omega.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlphaSelection {
    pub schema: ContractSelection,
    pub coordinate: CoordinateSpaceStamp,
    pub representation: RepresentationPolicyStamp,
    pub source: BufferStateStamp,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Translation {
    Exact(AlphaSelection),
    Partial { selected: AlphaSelection, omitted: BTreeSet<Key> },
    Unrepresentable,
}

impl KeySelection {
    pub fn disjoint(
        attachment: &descriptor::RepresentationAttachmentDescriptor,
        source: &BufferStateStamp,
    ) -> Self {
        Self {
            schema: support::selection(9300),
            coordinate: attachment.coordinate(),
            representation: attachment.representation(),
            source: *source,
            keys: [Key::Alpha, Key::Omega].into(),
        }
    }

    pub fn check_current(
        &self,
        source: &BufferStateStamp,
    ) -> Result<(), Failure> {
        if self.schema != support::selection(9300) {
            return Err(Failure::WrongSchema);
        }
        if !self.source.same_state(*source) {
            return Err(Failure::StaleState);
        }
        Ok(())
    }

    /// Stable native key identity is this schema's explicit correspondence.
    pub fn follow(
        &mut self,
        before: &BufferStateStamp,
        source: &BufferStateStamp,
        values: &KeyedState,
    ) -> Result<(), Failure> {
        self.check_current(before)?;
        if !self.keys.iter().all(|key| values.contains_key(key)) {
            return Err(Failure::Unrepresentable);
        }
        self.source = *source;
        Ok(())
    }

    /// Local declared mapping; no universal cross-schema conversion exists.
    pub fn to_alpha(
        &self,
        source: &BufferStateStamp,
    ) -> Result<Translation, Failure> {
        self.check_current(source)?;
        if !self.keys.contains(&Key::Alpha) {
            return Ok(Translation::Unrepresentable);
        }
        let selected = AlphaSelection {
            schema: alpha_schema(),
            coordinate: self.coordinate,
            representation: self.representation,
            source: self.source,
        };
        let omitted = self
            .keys
            .difference(&[Key::Alpha].into())
            .copied()
            .collect::<BTreeSet<_>>();
        if omitted.is_empty() {
            Ok(Translation::Exact(selected))
        } else {
            Ok(Translation::Partial { selected, omitted })
        }
    }
}

pub fn alpha_schema() -> ContractSelection {
    support::selection(9301)
}
