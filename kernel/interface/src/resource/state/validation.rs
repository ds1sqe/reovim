//! Structural admission before native evaluation; no authoritative mutation.

use crate::buffer::plan::BufferPlanDescriptor;
use crate::buffer::{
    BufferHistoryDirection, BufferHistoryEdge, BufferStateStamp,
};
use crate::cancellation::{CancellationState, DeadlineState, FenceCause};
use crate::resource::ResourceSnapshotStamp;
use crate::resource::access::ResourceObservedState;

use super::{
    ResourcePreparationStamp, ResourceStateError, ResourceStateInput,
    ResourceStateLimits, ResourceStateModelBinding, ResourceStateTarget,
    ResourceStateTransition, ResourceWorkingState,
};

pub(super) fn check_cancellation(
    cancellation: CancellationState,
    deadline: DeadlineState,
) -> Result<(), ResourceStateError> {
    if let CancellationState::Requested(_) = cancellation {
        return Err(ResourceStateError::Fenced(FenceCause::Cancellation));
    }
    if deadline == DeadlineState::Reached {
        return Err(ResourceStateError::Fenced(FenceCause::Deadline));
    }
    Ok(())
}

pub(super) fn validate_input<State>(
    model: &ResourceStateModelBinding,
    input: &ResourceStateInput<'_, State>,
    limits: ResourceStateLimits,
) -> Result<ResourcePreparationStamp, ResourceStateError> {
    match input {
        ResourceStateInput::Initial(observation) => {
            check_observations(
                model,
                core::slice::from_ref(*observation),
                limits,
            )?;
            Ok(ResourcePreparationStamp {
                target: ResourceStateTarget::Initial(observation.snapshot()),
                transition: ResourceStateTransition::Initial,
                retained: None,
            })
        }
        ResourceStateInput::Edit {
            observed,
            descriptor,
            plan,
            observations,
        } => {
            check_view(model, observed)?;
            descriptor.validate_plan(plan).map_err(ResourceStateError::Plan)?;
            check_observations(model, observations, limits)?;
            check_edit_observations(&observed.stamp, observations)?;
            if plan.target() != observed.stamp {
                return Err(ResourceStateError::StateMismatch);
            }
            Ok(ResourcePreparationStamp {
                target: ResourceStateTarget::Buffer(observed.stamp),
                transition: ResourceStateTransition::Edit(plan.stamp()),
                retained: None,
            })
        }
        ResourceStateInput::Restore {
            current,
            retained,
            plan,
            direction,
            edge,
        } => {
            check_view(model, current)?;
            check_view(model, retained)?;
            check_history(&current.stamp, &retained.stamp)?;
            check_edge(edge, *direction, &current.stamp, &retained.stamp)?;
            Ok(ResourcePreparationStamp {
                target: ResourceStateTarget::Buffer(current.stamp),
                transition: ResourceStateTransition::Restore {
                    plan: *plan,
                    direction: *direction,
                    edge: *edge,
                },
                retained: Some(retained.stamp),
            })
        }
    }
}

fn check_edge(
    edge: &BufferHistoryEdge,
    direction: BufferHistoryDirection,
    current: &BufferStateStamp,
    retained: &BufferStateStamp,
) -> Result<(), ResourceStateError> {
    if edge.from(direction) != current.history().node()
        || edge.to(direction) != retained.history().node()
    {
        return Err(ResourceStateError::StateMismatch);
    }
    Ok(())
}

pub(super) fn check_provider_plan<State>(
    input: &ResourceStateInput<'_, State>,
    selected: &BufferPlanDescriptor,
) -> Result<(), ResourceStateError> {
    let matches = match input {
        ResourceStateInput::Initial(_) => true,
        ResourceStateInput::Edit { descriptor, .. } => *descriptor == selected,
        ResourceStateInput::Restore { plan, .. } => {
            plan.owner() == selected.owner()
        }
    };
    if !matches {
        return Err(ResourceStateError::PlanDescriptorMismatch);
    }
    Ok(())
}

fn check_observations(
    model: &ResourceStateModelBinding,
    observations: &[ResourceObservedState],
    limits: ResourceStateLimits,
) -> Result<(), ResourceStateError> {
    let count = u64::try_from(observations.len())
        .map_err(|_| ResourceStateError::Pressure)?;
    if count > limits.observation_count.value() {
        return Err(ResourceStateError::Pressure);
    }
    let mut bytes = 0_u64;
    for observation in observations {
        bytes = bytes
            .checked_add(observation.payload().size().value())
            .ok_or(ResourceStateError::Pressure)?;
        if bytes > limits.observation_bytes.value() {
            return Err(ResourceStateError::Pressure);
        }
        check_snapshot(model, observation.snapshot())?;
        if observation.payload().stamp() != model.payload {
            return Err(ResourceStateError::PayloadSchemaMismatch);
        }
    }
    Ok(())
}

fn check_view<State>(
    model: &ResourceStateModelBinding,
    view: &ResourceWorkingState<'_, State>,
) -> Result<(), ResourceStateError> {
    if *model != view.model {
        return Err(ResourceStateError::ModelMismatch);
    }
    check_snapshot(model, view.stamp.source())
}

fn check_snapshot(
    model: &ResourceStateModelBinding,
    snapshot: ResourceSnapshotStamp,
) -> Result<(), ResourceStateError> {
    if model.schemas.revision() != snapshot.revision().schema() {
        return Err(ResourceStateError::RevisionSchemaMismatch);
    }
    Ok(())
}

fn check_edit_observations(
    state: &BufferStateStamp,
    observations: &[ResourceObservedState],
) -> Result<(), ResourceStateError> {
    if observations.iter().any(|item| item.snapshot() != state.source()) {
        return Err(ResourceStateError::StateMismatch);
    }
    Ok(())
}

fn check_history(
    current: &BufferStateStamp,
    retained: &BufferStateStamp,
) -> Result<(), ResourceStateError> {
    if current.buffer() != retained.buffer()
        || current.source().access() != retained.source().access()
        || current.history().lifecycle() != retained.history().lifecycle()
    {
        return Err(ResourceStateError::StateMismatch);
    }
    Ok(())
}
