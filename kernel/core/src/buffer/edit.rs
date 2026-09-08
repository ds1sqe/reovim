use alloc::rc::Rc;

use reovim_kernel_interface::bound::CheckedByteCount;
use reovim_kernel_interface::buffer::plan::BufferPlanStamp;
use reovim_kernel_interface::buffer::{
    BufferHistoryEdge, BufferHistoryNodeIdentity, BufferHistoryStamp,
    BufferStateStamp, BufferVersion,
};
use reovim_kernel_interface::cancellation::InstallationOutcome;
use reovim_kernel_interface::resource::state::{
    ResourcePreparationStamp, ResourcePreparedState, ResourceStateError,
    ResourceStateTarget, ResourceStateTransition,
};

use super::{Buffer, BufferCommit, Retained};
use crate::{BufferError, Rejected, StagedEdit};

impl<State> Buffer<State> {
    /// Checks a completed edit and allocates its retained immutable handle.
    /// This runs before acquiring mutable installation authority. A failure
    /// returns the original candidate's immutable ownership; staging changes
    /// no authoritative state.
    ///
    /// # Errors
    /// Returns exact preparation/currentness or capacity failure.
    pub fn stage_edit(
        &self,
        expected: &ResourcePreparationStamp,
        candidate: ResourcePreparedState<State>,
    ) -> Result<StagedEdit<State>, Rejected<Rc<ResourcePreparedState<State>>>>
    {
        let candidate = Rc::new(candidate);
        let checked =
            self.check_edit(expected, &candidate, InstallationOutcome::Current);
        if let Err(cause) = checked {
            return Err(Rejected { cause, value: candidate });
        }
        Ok(StagedEdit { prepared: candidate })
    }

    /// Rechecks and atomically installs one previously prepared native edit.
    ///
    /// The caller supplies the live Runtime fence, not accepted versions or
    /// nodes. No allocation, native callback or native destructor occurs here.
    /// Every old state remains retained, so this edit retires no native state.
    ///
    /// # Errors
    /// Returns a failed fence, changed observation/plan or exhausted bound.
    /// The returned staged candidate must be dropped outside authority.
    pub fn commit_edit(
        &mut self,
        staged: StagedEdit<State>,
        fence: InstallationOutcome,
    ) -> Result<BufferCommit, Rejected<StagedEdit<State>>> {
        // Staging validated the expected request against this immutable
        // stamp; there is no mutable duplicate that can diverge afterward.
        let expected = staged.prepared.stamp();
        let checked = self.check_edit(&expected, &staged.prepared, fence);
        let (plan, retained_bytes) = match checked {
            Ok(checked) => checked,
            Err(cause) => return Err(Rejected { cause, value: staged }),
        };
        let (commit, next_node) = match self.next_commit(plan) {
            Ok(next) => next,
            Err(cause) => return Err(Rejected { cause, value: staged }),
        };
        let node = Retained {
            stamp: commit.result,
            parent: Some(commit.edge.parent()),
            prepared: staged.prepared,
        };
        // Atomic cut: the slot is already allocated and proved empty.
        // Only moves/scalars follow; every prior node stays owned unchanged.
        self.nodes[self.used] = Some(node);
        self.current = self.used;
        self.used += 1;
        self.stamp = commit.result;
        self.next_node = next_node;
        self.retained_bytes = retained_bytes;
        Ok(commit)
    }

    fn check_edit(
        &self,
        expected: &ResourcePreparationStamp,
        candidate: &ResourcePreparedState<State>,
        fence: InstallationOutcome,
    ) -> Result<(BufferPlanStamp, CheckedByteCount), BufferError> {
        candidate
            .check_admission(
                self.config.model,
                *expected,
                ResourceStateTarget::Buffer(self.stamp),
                self.config.preparation,
                fence,
            )
            .map_err(BufferError::State)?;
        let ResourceStateTransition::Edit(plan) = expected.transition else {
            return Err(BufferError::State(
                ResourceStateError::PreparationMismatch,
            ));
        };
        if plan.owner() != self.config.plan_owner {
            return Err(BufferError::State(
                ResourceStateError::PlanDescriptorMismatch,
            ));
        }
        if self.used >= self.nodes.len() || self.nodes[self.used].is_some() {
            return Err(BufferError::Capacity);
        }
        let retained = self
            .retained_bytes
            .checked_add(candidate.usage().resident_bytes)
            .map_err(|_| BufferError::Capacity)?;
        if retained.value() > self.config.limits.resident_bytes.value() {
            return Err(BufferError::Capacity);
        }
        Ok((plan, retained))
    }

    fn next_commit(
        &self,
        plan: BufferPlanStamp,
    ) -> Result<(BufferCommit, u128), BufferError> {
        let version = self
            .stamp
            .version()
            .value()
            .checked_add(1)
            .ok_or(BufferError::IdentityExhausted)?;
        let next_node = self
            .next_node
            .checked_add(1)
            .ok_or(BufferError::IdentityExhausted)?;
        let node = BufferHistoryNodeIdentity::new(self.next_node);
        let edge = BufferHistoryEdge::new(self.stamp.history().node(), node)
            .map_err(|_| BufferError::IdentityExhausted)?;
        let result = BufferStateStamp::new(
            self.stamp.source(),
            self.stamp.buffer(),
            BufferVersion::new(version),
            BufferHistoryStamp::new(self.stamp.history().lifecycle(), node),
        );
        Ok((BufferCommit { prior: self.stamp, result, edge, plan }, next_node))
    }
}
