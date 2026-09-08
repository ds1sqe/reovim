use alloc::rc::Rc;
use core::cell::RefCell;

use reovim_kernel_interface::buffer::BufferStateStamp;
use reovim_kernel_interface::interaction::history::{
    InteractionHistoryOutcome, InteractionHistoryRequest,
};
use reovim_kernel_interface::interaction::intent::InteractionIntent;
use reovim_kernel_interface::interaction::notification::{
    BufferChangeNotification, BufferChangeSubscription,
};
use reovim_kernel_interface::interaction::outcome::InteractionOutcome;
use reovim_kernel_interface::interaction::session::InteractionClientStamp;
use reovim_kernel_interface::interaction::update::{
    RepresentationUpdate, RepresentationUpdateIdentityStamp,
};

use crate::first_edit::Family;
use crate::interaction::support::{self, InteractionFixture};

mod changes;
mod completion;
mod history;
mod input;
mod port;
mod shared;

pub use completion::next_stamp;
pub use shared::Shared;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AttachmentState {
    Unnegotiated,
    Negotiated,
    Attached,
}

/// One endpoint's bounded admissions/delivery, separate from shared state.
pub struct Session<Model: Family> {
    pub fixture: InteractionFixture,
    pub shared: Rc<RefCell<Shared<Model>>>,
    pub attachment_state: AttachmentState,
    pub closed: bool,
    pub pending: Option<InteractionIntent>,
    pub completed: Option<(InteractionIntent, InteractionOutcome)>,
    pub outcome: Option<InteractionOutcome>,
    pub history_pending: Option<InteractionHistoryRequest>,
    pub history_completed: Option<InteractionHistoryOutcome>,
    pub history_older: Option<InteractionHistoryOutcome>,
    pub history_outcome: Option<InteractionHistoryOutcome>,
    pub update: Option<RepresentationUpdate>,
    pub delivered: Option<RepresentationUpdateIdentityStamp>,
    pub cancelled: bool,
    pub subscription: Option<BufferChangeSubscription>,
    pub subscription_closed: bool,
    pub notice: Option<BufferChangeNotification>,
    pub notice_gap: Option<BufferStateStamp>,
}

impl<Model: Family> Session<Model> {
    pub fn new(
        shared: Rc<RefCell<Shared<Model>>>,
        client: InteractionClientStamp,
    ) -> Self {
        let binding = shared.borrow().model.binding();
        let semantic = crate::semantic::support::fixture_for_model(&binding)
            .expect("selected native semantic profile");
        let mut fixture =
            support::fixture_for_client(semantic, client, client.lifecycle())
                .expect("independent endpoint identities");
        input::reserve_history(&mut fixture);
        Self {
            fixture,
            shared,
            attachment_state: AttachmentState::Unnegotiated,
            closed: false,
            pending: None,
            completed: None,
            outcome: None,
            history_pending: None,
            history_completed: None,
            history_older: None,
            history_outcome: None,
            update: None,
            delivered: None,
            cancelled: false,
            subscription: None,
            subscription_closed: false,
            notice: None,
            notice_gap: None,
        }
    }

    pub fn ready(&self) -> bool {
        !self.closed && self.shared.borrow().current.is_some()
    }

    pub fn current_stamp(&self) -> BufferStateStamp {
        self.shared.borrow().stamp
    }

    pub const fn is_negotiated(&self) -> bool {
        matches!(
            self.attachment_state,
            AttachmentState::Negotiated | AttachmentState::Attached
        )
    }

    pub const fn is_attached(&self) -> bool {
        matches!(self.attachment_state, AttachmentState::Attached)
    }
}
