use reovim_kernel_interface::buffer::{
    BufferHistoryDirection, BufferHistoryStamp, BufferStateStamp, BufferVersion,
};
use reovim_kernel_interface::cancellation::InstallationOutcome;

use crate::{
    Buffer, BufferCommit, BufferError, Installation, Rejected, StagedHistory,
    StateLease,
};

impl<State> Buffer<State> {
    /// Revalidates and installs the model's actual Restore materialization.
    /// Canonical retained node data is never replaced by reconstruction.
    ///
    /// # Errors
    /// Returns stale/changed selection, capacity, identity exhaustion or fence
    /// with candidate ownership intact. Release errors/retired data after the
    /// Buffer guard; no native destructor or allocation runs at this cut.
    ///
    /// # Panics
    /// Panics if a validated private parent slot has become corrupted.
    pub fn commit_history<'a>(
        &mut self,
        staged: StagedHistory<'a, State>,
        fence: InstallationOutcome,
    ) -> Result<Installation<State>, Rejected<StagedHistory<'a, State>>> {
        let selection = staged.selection;
        let checked = self.check_history(
            selection,
            &staged.prepared.stamp(),
            &staged.prepared,
            fence,
        );
        let resident = match checked {
            Ok(resident) => resident,
            Err(cause) => return Err(Rejected { cause, value: staged }),
        };
        let Some(version) = self.stamp.version().value().checked_add(1) else {
            return Err(Rejected {
                cause: BufferError::IdentityExhausted,
                value: staged,
            });
        };
        let result = BufferStateStamp::new(
            self.stamp.source(),
            self.stamp.buffer(),
            BufferVersion::new(version),
            BufferHistoryStamp::new(
                self.stamp.history().lifecycle(),
                selection.retained.stamp().history().node(),
            ),
        );
        let commit = BufferCommit {
            prior: self.stamp,
            result,
            edge: selection.edge,
            plan: selection.plan,
        };
        let parent_index = match selection.direction {
            BufferHistoryDirection::Undo => selection.retained_index,
            BufferHistoryDirection::Redo => self.current,
        };
        let parent = self.nodes[parent_index]
            .as_mut()
            .expect("validated parent remains retained");
        // Atomic cut: only scalar preference/state updates and owned moves.
        parent.preferred_child = Some(selection.edge.child());
        let retired = StateLease {
            model: self.config.model,
            stamp: self.stamp,
            prepared: core::mem::replace(
                &mut self.current_state,
                staged.prepared,
            ),
        };
        self.current = selection.retained_index;
        self.stamp = result;
        self.resident_bytes = resident;
        Ok(Installation { commit, retired })
    }
}
