use alloc::{boxed::Box, vec::Vec};

use reovim_domain_basic::heading;
use reovim_kernel_interface::semantic::{
    descriptor::RepresentationPolicyDescriptor,
    policy::SemanticPolicyFailure,
    representation::RepresentationPolicyPort,
    snapshot::{
        RepresentationSnapshotDescriptor, RepresentationSnapshotLimits,
        RepresentationSnapshotPayload, RepresentationSnapshotPort,
        RepresentationSnapshotRequest,
    },
};
use reovim_resource_local::{ByteState, Key, KeyedState, profile};

use crate::{HeadingRepresentation, KeyedRepresentation};

/// Read-only encoded heading snapshot producer, independent of any Client view.
pub struct HeadingSnapshot {
    descriptor: RepresentationSnapshotDescriptor,
}

impl HeadingSnapshot {
    /// Selects the heading snapshot schema for this Representation generation.
    #[must_use]
    pub fn new(representation: &HeadingRepresentation) -> Self {
        Self { descriptor: descriptor(representation.descriptor(), 0x6008) }
    }
}

impl RepresentationSnapshotPort for HeadingSnapshot {
    type State = ByteState;

    fn descriptor(&self) -> &RepresentationSnapshotDescriptor {
        &self.descriptor
    }

    fn snapshot(
        &self,
        request: &RepresentationSnapshotRequest<'_, ByteState>,
    ) -> Result<RepresentationSnapshotPayload, SemanticPolicyFailure> {
        let current = heading(request.observed().state)?;
        Ok(payload(
            &self.descriptor,
            Box::new([b'H', current.level, current.label]),
        ))
    }
}

/// Read-only sparse-key snapshot producer; keys are not text coordinates.
pub struct KeyedSnapshot {
    descriptor: RepresentationSnapshotDescriptor,
}

impl KeyedSnapshot {
    /// Selects the keyed snapshot schema for this Representation generation.
    #[must_use]
    pub fn new(representation: &KeyedRepresentation) -> Self {
        Self { descriptor: descriptor(representation.descriptor(), 0x7008) }
    }
}

impl RepresentationSnapshotPort for KeyedSnapshot {
    type State = KeyedState;

    fn descriptor(&self) -> &RepresentationSnapshotDescriptor {
        &self.descriptor
    }

    fn snapshot(
        &self,
        request: &RepresentationSnapshotRequest<'_, KeyedState>,
    ) -> Result<RepresentationSnapshotPayload, SemanticPolicyFailure> {
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(5)
            .map_err(|_| SemanticPolicyFailure::Pressure)?;
        bytes.push(b'K');
        for key in [Key::Alpha, Key::Omega] {
            append_key(&mut bytes, request.observed().state, key);
        }
        Ok(payload(&self.descriptor, bytes.into_boxed_slice()))
    }
}

fn append_key(bytes: &mut Vec<u8>, state: &KeyedState, key: Key) {
    if let Some(value) = state.get(key) {
        bytes.extend_from_slice(&[key.tag(), u8::from(value)]);
    }
}

fn descriptor(
    representation: &RepresentationPolicyDescriptor,
    output: u128,
) -> RepresentationSnapshotDescriptor {
    RepresentationSnapshotDescriptor::new(
        representation,
        profile::schema(output),
        &reovim_domain_basic::profile::call(
            representation.stamp().lifecycle().instance(),
        ),
        RepresentationSnapshotLimits {
            items: profile::count(1),
            depth: profile::count(0),
        },
    )
}

fn payload(
    descriptor: &RepresentationSnapshotDescriptor,
    bytes: Box<[u8]>,
) -> RepresentationSnapshotPayload {
    RepresentationSnapshotPayload {
        schema: descriptor.output_schema(),
        // The encoding represents one complete snapshot object, even empty K.
        items: profile::count(1),
        depth: profile::count(0),
        bytes,
    }
}
