use alloc::{rc::Rc, vec::Vec};

use reovim_kernel_interface::buffer::{
    BufferHistoryNodeIdentity, BufferHistoryStamp, BufferStateStamp,
    BufferVersion,
};
use reovim_kernel_interface::cancellation::InstallationOutcome;
use reovim_kernel_interface::resource::state::{
    ResourcePreparationStamp, ResourcePreparedState, ResourceStateError,
    ResourceStateTarget, ResourceStateTransition,
};

use super::{Buffer, Retained};
use crate::{BufferConfig, BufferError, Rejected};

impl<State> Buffer<State> {
    /// Initializes an unpublished Buffer from exactly reserved native work.
    /// Runtime still owns the open reservation and its publication fence.
    /// All storage allocation occurs before this value becomes authoritative.
    ///
    /// # Errors
    /// Returns invalid configuration, capacity or native admission failure,
    /// retaining the unchanged candidate for outside-authority cleanup.
    pub fn new(
        config: &BufferConfig,
        expected: &ResourcePreparationStamp,
        candidate: ResourcePreparedState<State>,
        fence: InstallationOutcome,
    ) -> Result<Self, Rejected<Rc<ResourcePreparedState<State>>>> {
        let candidate = Rc::new(candidate);
        let capacity = match initial_check(config, expected, &candidate, fence)
        {
            Ok(capacity) => capacity,
            Err(cause) => return Err(Rejected { cause, value: candidate }),
        };
        let mut nodes = Vec::new();
        if nodes.try_reserve_exact(capacity).is_err() {
            return Err(Rejected {
                cause: BufferError::Capacity,
                value: candidate,
            });
        }
        nodes.resize_with(capacity, || None);
        // IDs are private allocations, never History positions or depths.
        let stamp = BufferStateStamp::new(
            config.source,
            config.incarnation,
            BufferVersion::INITIAL,
            BufferHistoryStamp::new(
                config.history,
                BufferHistoryNodeIdentity::new(0),
            ),
        );
        let retained_bytes = candidate.usage().resident_bytes;
        nodes[0] = Some(Retained { stamp, parent: None, prepared: candidate });
        Ok(Self {
            config: *config,
            nodes,
            current: 0,
            used: 1,
            stamp,
            next_node: 1,
            retained_bytes,
        })
    }
}

fn initial_check<State>(
    config: &BufferConfig,
    expected: &ResourcePreparationStamp,
    candidate: &ResourcePreparedState<State>,
    fence: InstallationOutcome,
) -> Result<usize, BufferError> {
    if config.limits.history_nodes.value() == 0
        || config.source.revision().schema() != config.model.schemas.revision()
    {
        return Err(BufferError::InvalidConfiguration);
    }
    if expected.transition != ResourceStateTransition::Initial {
        return Err(BufferError::State(
            ResourceStateError::PreparationMismatch,
        ));
    }
    candidate
        .check_admission(
            config.model,
            *expected,
            ResourceStateTarget::Initial(config.source),
            config.preparation,
            fence,
        )
        .map_err(BufferError::State)?;
    if candidate.usage().resident_bytes.value()
        > config.limits.resident_bytes.value()
    {
        return Err(BufferError::Capacity);
    }
    usize::try_from(config.limits.history_nodes.value())
        .map_err(|_| BufferError::Capacity)
}
