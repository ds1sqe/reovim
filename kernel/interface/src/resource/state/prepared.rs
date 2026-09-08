use core::fmt;

use crate::cancellation::InstallationOutcome;

use super::{
    ResourcePreparationStamp, ResourceStateError, ResourceStateLimits,
    ResourceStateModelBinding, ResourceStateOutput, ResourceStateTarget,
    ResourceStateUsage,
};

/// Fully evaluated immutable candidate, still without mutation authority.
///
/// Metadata comes from the validated request, not provider-supplied stamps.
/// No native state is decoded or destroyed during scalar admission checks.
pub struct ResourcePreparedState<State> {
    model: ResourceStateModelBinding,
    stamp: ResourcePreparationStamp,
    output: ResourceStateOutput<State>,
}

impl<State> ResourcePreparedState<State> {
    pub(super) const fn new(
        model: &ResourceStateModelBinding,
        stamp: &ResourcePreparationStamp,
        output: ResourceStateOutput<State>,
    ) -> Self {
        Self { model: *model, stamp: *stamp, output }
    }

    /// Returns the exact work identity bound before native evaluation.
    #[must_use]
    pub const fn stamp(&self) -> ResourcePreparationStamp {
        self.stamp
    }

    /// Borrows prepared immutable data without installing it.
    #[must_use]
    pub const fn state(&self) -> &State {
        &self.output.state
    }

    /// Returns the provider's conservative native memory and work charge.
    /// Core uses it to reserve aggregate retention before installation.
    #[must_use]
    pub const fn usage(&self) -> ResourceStateUsage {
        self.output.usage
    }

    /// Checks scalar installation conditions without callbacks or loss of
    /// candidate ownership.
    ///
    /// Core supplies its selected request and reads `current`, model binding,
    /// reservations and fences under the SAME serialization used to install.
    /// It also checks grants, History eligibility and aggregate reservations.
    /// A check performed before acquiring that authority is not permission
    /// to commit. Initial admission additionally requires a live unpublished
    /// open reservation; another successful open cannot reuse it.
    ///
    /// # Errors
    ///
    /// Returns a mismatch, exhausted bound or fence; the caller retains the
    /// entire candidate and releases rejected/retired data outside authority.
    pub fn check_admission(
        &self,
        model: ResourceStateModelBinding,
        expected: ResourcePreparationStamp,
        current: ResourceStateTarget,
        limits: ResourceStateLimits,
        installation: InstallationOutcome,
    ) -> Result<(), ResourceStateError> {
        if let InstallationOutcome::Fenced(cause) = installation {
            return Err(ResourceStateError::Fenced(cause));
        }
        if self.model != model {
            return Err(ResourceStateError::ModelMismatch);
        }
        if self.stamp.target != current {
            return Err(ResourceStateError::StateMismatch);
        }
        if self.stamp != expected {
            return Err(ResourceStateError::PreparationMismatch);
        }
        if !limits.admits(self.output.usage) {
            return Err(ResourceStateError::Pressure);
        }
        Ok(())
    }

    /// Transfers data ownership without invoking a native destructor.
    ///
    /// This is not admission. Core installs only after all final checks and
    /// returns displaced data for retirement outside its commit authority.
    #[must_use]
    pub fn into_output(self) -> ResourceStateOutput<State> {
        self.output
    }
}

impl<State> fmt::Debug for ResourcePreparedState<State> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResourcePreparedState")
            .field("model", &self.model)
            .field("stamp", &self.stamp)
            .field("output", &self.output)
            .finish()
    }
}

/// Preparation failure retaining output when the provider already produced it.
pub struct ResourceStateRejection<State> {
    /// Typed reason why no candidate may be admitted.
    pub cause: ResourceStateError,
    /// Ownership of already-produced data, released outside authority.
    pub output: Option<ResourceStateOutput<State>>,
}

impl<State> ResourceStateRejection<State> {
    pub(super) const fn empty(cause: ResourceStateError) -> Self {
        Self { cause, output: None }
    }
}

impl<State> fmt::Debug for ResourceStateRejection<State> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResourceStateRejection")
            .field("cause", &self.cause)
            .field("output", &self.output)
            .finish()
    }
}
