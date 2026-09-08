use crate::buffer::plan::{BufferPlanDescriptor, BufferPlanStamp};
use crate::buffer::{
    BufferHistoryDirection, BufferHistoryEdge, BufferStateStamp,
};
use crate::cancellation::{
    CancellationState, DeadlineState, InstallationOutcome,
};
use crate::resource::ResourceSnapshotStamp;
use crate::resource::access::ResourceObservedState;

use super::validation::{
    check_cancellation, check_provider_plan, validate_input,
};
use super::{
    ResourcePreparedState, ResourceStateError, ResourceStateLimits,
    ResourceStateModel, ResourceStateModelBinding, ResourceStateRejection,
    ResourceWorkingState,
};

/// Native inputs owned or retained by Runtime, never by commit-time callbacks.
pub enum ResourceStateInput<'a, State> {
    /// Initial data before a Buffer exists. No accepted edit is fabricated.
    Initial(&'a ResourceObservedState),
    /// A completely lowered plan over immutable current working state.
    Edit {
        /// Exact immutable state against which native preconditions run.
        observed: &'a ResourceWorkingState<'a, State>,
        /// Selected structural plan contract, validated before decoding.
        descriptor: &'a BufferPlanDescriptor,
        /// Resource-native operations, origin and preconditions.
        plan: &'a crate::buffer::plan::BufferTransactionPlan,
        /// Bounded observations for nonresident native data, if needed.
        observations: &'a [ResourceObservedState],
    },
    /// Reconstructs the exact History state selected by Core, not the model.
    Restore {
        /// Current state targeted by this new ordered transition.
        current: &'a ResourceWorkingState<'a, State>,
        /// Retained historical data, possibly under an older source snapshot.
        retained: &'a ResourceWorkingState<'a, State>,
        /// New operation identity and authorities; not the historical plan.
        plan: BufferPlanStamp,
        /// Core-selected direction. Eligibility is rechecked at installation.
        direction: BufferHistoryDirection,
        /// Exact retained tree edge selected by Core before preparation.
        edge: BufferHistoryEdge,
    },
}

/// Current authority against which preparation is admitted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceStateTarget {
    /// Reserved initial Resource observation, not a ready Buffer.
    Initial(ResourceSnapshotStamp),
    /// Exact current Buffer version and History relation.
    Buffer(BufferStateStamp),
}

/// Structural preparation identity; Core owns History selection and ordering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceStateTransition {
    /// Initial state preparation does not create an undo step.
    Initial,
    /// One lowered native edit under the complete plan identity.
    Edit(BufferPlanStamp),
    /// Exact selected reconstruction, not permission to choose another step.
    Restore {
        /// New request/transaction authorities.
        plan: BufferPlanStamp,
        /// Selected active History edge.
        direction: BufferHistoryDirection,
        /// Exact parent/child relation; node IDs have no ordinal meaning.
        edge: BufferHistoryEdge,
    },
}

/// Exact expected native preparation, retained by Runtime/Core before work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourcePreparationStamp {
    /// Current state or initial source reservation.
    pub target: ResourceStateTarget,
    /// Selected operation and History evidence.
    pub transition: ResourceStateTransition,
    pub(super) retained: Option<BufferStateStamp>,
}

impl ResourcePreparationStamp {
    /// Returns exact retained evidence only for a restore preparation.
    #[must_use]
    pub const fn retained(&self) -> Option<BufferStateStamp> {
        self.retained
    }
}

/// Structurally validated native work. Holds only immutable borrowed inputs.
pub struct ResourceStatePreparation<'a, State> {
    model: ResourceStateModelBinding,
    stamp: ResourcePreparationStamp,
    input: ResourceStateInput<'a, State>,
    limits: ResourceStateLimits,
}

impl<'a, State> ResourceStatePreparation<'a, State> {
    /// Validates model/schema, state and plan before invoking native code.
    ///
    /// # Errors
    ///
    /// Returns a structural mismatch or observed cancellation/deadline fence.
    /// Borrowed input ownership always remains with the caller.
    pub fn new(
        model: ResourceStateModelBinding,
        input: ResourceStateInput<'a, State>,
        limits: ResourceStateLimits,
        cancellation: CancellationState,
        deadline: DeadlineState,
    ) -> Result<Self, ResourceStateError> {
        check_cancellation(cancellation, deadline)?;
        let stamp = validate_input(&model, &input, limits)?;
        Ok(Self { model, stamp, input, limits })
    }

    /// Returns native inputs after structural admission.
    #[must_use]
    pub const fn input(&self) -> &ResourceStateInput<'a, State> {
        &self.input
    }

    /// Returns the exact work identity to retain for final revalidation.
    #[must_use]
    pub const fn stamp(&self) -> ResourcePreparationStamp {
        self.stamp
    }

    /// Returns the native work and retained-memory reservation.
    #[must_use]
    pub const fn limits(&self) -> ResourceStateLimits {
        self.limits
    }

    /// Checks provider selection, invokes native policy and binds its result.
    ///
    /// This entire operation runs outside Buffer commit ownership. Final
    /// currentness/fences must still be checked at authoritative installation.
    ///
    /// # Errors
    ///
    /// Returns typed rejection retaining any output already produced.
    pub fn prepare_with<Model>(
        &self,
        model: &Model,
    ) -> Result<ResourcePreparedState<State>, ResourceStateRejection<State>>
    where
        Model: ResourceStateModel<State = State> + ?Sized,
    {
        if model.binding() != self.model {
            return Err(ResourceStateRejection::empty(
                ResourceStateError::ModelMismatch,
            ));
        }
        check_provider_plan(&self.input, model.plan_descriptor())
            .map_err(ResourceStateRejection::empty)?;
        let output =
            model.prepare(self).map_err(ResourceStateRejection::empty)?;
        let candidate =
            ResourcePreparedState::new(&self.model, &self.stamp, output);
        let check = check_provider_plan(&self.input, model.plan_descriptor())
            .and_then(|()| {
                candidate.check_admission(
                    model.binding(),
                    self.stamp,
                    self.stamp.target,
                    self.limits,
                    InstallationOutcome::Current,
                )
            });
        match check {
            Ok(()) => Ok(candidate),
            Err(cause) => Err(ResourceStateRejection {
                cause,
                output: Some(candidate.into_output()),
            }),
        }
    }
}
