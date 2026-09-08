use alloc::rc::Rc;

use reovim_kernel_interface::bound::CheckedByteCount;
use reovim_kernel_interface::cancellation::InstallationOutcome;
use reovim_kernel_interface::resource::state::{
    ResourcePreparationStamp, ResourcePreparedState, ResourceStateError,
    ResourceStateTarget, ResourceStateTransition,
};

use crate::{Buffer, BufferError, HistorySelection, Rejected, StagedHistory};

impl<State> Buffer<State> {
    /// Checks the exact selected reconstruction and allocates its handle.
    /// Selection and preparation remain outside mutable commit ownership.
    ///
    /// # Errors
    /// Returns mismatched state/selection/plan or capacity with the candidate
    /// retained. The borrowed selection remains owned by the caller.
    pub fn stage_history<'a>(
        &self,
        selection: &'a HistorySelection<State>,
        expected: &ResourcePreparationStamp,
        candidate: ResourcePreparedState<State>,
    ) -> Result<
        StagedHistory<'a, State>,
        Rejected<Rc<ResourcePreparedState<State>>>,
    > {
        let prepared = Rc::new(candidate);
        let checked = self.check_history(
            selection,
            expected,
            &prepared,
            InstallationOutcome::Current,
        );
        if let Err(cause) = checked {
            return Err(Rejected { cause, value: prepared });
        }
        Ok(StagedHistory { selection, prepared })
    }

    pub(super) fn check_history(
        &self,
        selection: &HistorySelection<State>,
        expected: &ResourcePreparationStamp,
        prepared: &ResourcePreparedState<State>,
        fence: InstallationOutcome,
    ) -> Result<CheckedByteCount, BufferError> {
        prepared
            .check_admission(
                self.config.model,
                *expected,
                ResourceStateTarget::Buffer(self.stamp),
                self.config.preparation,
                fence,
            )
            .map_err(BufferError::State)?;
        self.validate_selection(selection)?;
        let transition = ResourceStateTransition::Restore {
            plan: selection.plan,
            direction: selection.direction,
            edge: selection.edge,
        };
        if expected.transition != transition
            || expected.retained() != Some(selection.retained.stamp())
            || selection.plan.owner() != self.config.plan_owner
        {
            return Err(BufferError::State(
                ResourceStateError::PreparationMismatch,
            ));
        }
        let resident = self
            .retained_bytes
            .checked_add(prepared.usage().resident_bytes)
            .map_err(|_| BufferError::Capacity)?;
        if resident.value() > self.config.limits.resident_bytes.value() {
            return Err(BufferError::Capacity);
        }
        Ok(resident)
    }
}
