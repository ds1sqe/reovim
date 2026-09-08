//! Root coordinator fixture; native state remains in the shared state owner.

use alloc::{rc::Rc, vec::Vec};
use core::cell::RefCell;

use reovim_kernel_interface::interaction::root::{
    RootClientHandle, RootFailure, RootLifecycleRequest, RootLimits,
    RootOpenCompletion, RootOpenRequest,
};
use reovim_kernel_interface::interaction::session::InteractionClientStamp;
use reovim_kernel_interface::lifecycle::{
    Generation, LifecycleIdentity, LifecycleState,
};
use reovim_kernel_interface::resource::access::ResourceObservedState;

use super::session::{Session, Shared};
use crate::first_edit::{Family, native::support::count};

mod faults;
mod open;
mod port;
pub mod resource;

pub use faults::exercise_root_faults;

struct Authorization {
    authorization: LifecycleIdentity,
    client: InteractionClientStamp,
}

struct Client<Model: Family> {
    handle: RootClientHandle,
    endpoint: Session<Model>,
    opened: bool,
    closing: bool,
}

struct Open {
    request: RootOpenRequest,
    completion: Option<RootOpenCompletion>,
    cancelled: bool,
}

pub struct Root<Model: Family> {
    owner: LifecycleIdentity,
    state: LifecycleState,
    limits: RootLimits,
    shared: Rc<RefCell<Shared<Model>>>,
    authorizations: Vec<Authorization>,
    clients: Vec<Client<Model>>,
    opens: Vec<Open>,
    pub resource: resource::Resource,
}

impl<Model: Family> Root<Model> {
    pub fn new(
        shared: Rc<RefCell<Shared<Model>>>,
        observation: ResourceObservedState,
    ) -> Self {
        Self {
            owner: LifecycleIdentity::new(8100, 8101, Generation::new(1)),
            state: LifecycleState::Constructed,
            limits: RootLimits {
                clients: count(2),
                opens: count(4),
                work: count(16),
            },
            shared,
            authorizations: Vec::new(),
            clients: Vec::new(),
            opens: Vec::new(),
            resource: resource::Resource::new(observation),
        }
    }

    /// Privileged fixture assembly issues this record, not the Client port.
    pub fn authorization_for(
        &mut self,
        client: InteractionClientStamp,
    ) -> LifecycleIdentity {
        let identity = LifecycleIdentity::new(
            8200,
            client.client().value(),
            Generation::new(1),
        );
        self.authorizations
            .push(Authorization { authorization: identity, client });
        identity
    }

    pub fn budget(&self) -> RootLifecycleRequest {
        RootLifecycleRequest {
            root: self.owner,
            work: count(8),
            deadline:
                reovim_kernel_interface::cancellation::DeadlineState::Pending,
        }
    }

    fn check_budget(
        &self,
        request: RootLifecycleRequest,
    ) -> Result<(), RootFailure> {
        if request.root != self.owner {
            return Err(RootFailure::Stale);
        }
        self.check_work(request.work.value())
    }

    const fn check_work(&self, work: u64) -> Result<(), RootFailure> {
        if work == 0 || work > self.limits.work.value() {
            return Err(RootFailure::Pressure);
        }
        Ok(())
    }

    const fn require_ready(&self) -> Result<(), RootFailure> {
        match self.state {
            LifecycleState::Ready => Ok(()),
            LifecycleState::Closing | LifecycleState::IncompleteShutdown => {
                Err(RootFailure::Closing)
            }
            LifecycleState::Closed => Err(RootFailure::Closed),
            _ => Err(RootFailure::NotReady),
        }
    }

    fn client_index(
        &self,
        handle: &RootClientHandle,
    ) -> Result<usize, RootFailure> {
        if handle.root != self.owner {
            return Err(RootFailure::Stale);
        }
        self.clients
            .iter()
            .position(|entry| entry.handle == *handle)
            .ok_or(RootFailure::Denied)
    }

    fn active_client(
        &self,
        handle: &RootClientHandle,
    ) -> Result<usize, RootFailure> {
        self.require_ready()?;
        let index = self.client_index(handle)?;
        if self.clients[index].closing {
            return Err(RootFailure::Closing);
        }
        Ok(index)
    }
}
