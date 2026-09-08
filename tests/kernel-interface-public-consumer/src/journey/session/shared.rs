use alloc::rc::Rc;

use reovim_kernel_interface::buffer::{
    BufferHistoryNodeIdentity, BufferHistoryStamp, BufferStateStamp,
    BufferVersion,
};
use reovim_kernel_interface::resource::state::ResourceStateError;

use crate::first_edit::Family;

/// One fixture-owned state slot; accepted stamps/edges are supplied by cases.
pub struct Shared<Model: Family> {
    pub model: Rc<Model>,
    pub current: Option<Model::State>,
    pub initial: Option<Model::State>,
    pub edited: Option<Model::State>,
    pub stamp: BufferStateStamp,
}

impl<Model: Family> Shared<Model> {
    pub fn new(model: Model) -> Self {
        let fixture =
            crate::semantic::support::fixture_for_model(&model.binding())
                .expect("existing resource and semantic contracts");
        let observed = fixture.state.stamp;
        let stamp = BufferStateStamp::new(
            observed.source(),
            observed.buffer(),
            BufferVersion::INITIAL,
            BufferHistoryStamp::new(
                observed.history().lifecycle(),
                BufferHistoryNodeIdentity::new(900),
            ),
        );
        Self {
            model: Rc::new(model),
            current: None,
            initial: None,
            edited: None,
            stamp,
        }
    }

    pub const fn initial_stamp(&self) -> BufferStateStamp {
        self.stamp
    }

    /// Root prepares and clones outside authority, then installs only moves.
    pub fn install_initial(
        &mut self,
        expected: &BufferStateStamp,
        current: Model::State,
        retained: Model::State,
    ) -> Result<
        BufferStateStamp,
        (ResourceStateError, Model::State, Model::State),
    > {
        if self.current.is_some()
            || self.initial.is_some()
            || *expected != self.stamp
        {
            return Err((ResourceStateError::StateMismatch, current, retained));
        }
        self.initial = Some(retained);
        self.current = Some(current);
        Ok(self.stamp)
    }
}
