//! Retained service credentials select exact Caller custody across sockets.

use alloc::{rc::Rc, vec::Vec};
use reovim_kernel_interface::Error;
use reovim_kernel_runtime::{Binding, Runtime};
use reovim_service::protocol::{Endpoint, Frame, Reply};

struct Client {
    token: [u8; 16],
    state: State,
}

enum State {
    Active { binding: Rc<Binding>, endpoint: Endpoint },
    Closed,
}

/// Host credential routing; Runtime owns requests and binding retirement.
///
/// Composition supplies credentials and bindings after authorizing grants.
/// Socket loss retires neither. Retired credentials cannot be reassigned.
/// This owner handles no startup, provider selection or delivery receipt table.
pub struct Clients {
    entries: Vec<Client>,
    capacity: usize,
}

impl Clients {
    /// Reserves a finite credential budget, including retired credentials.
    /// This budget is separate from Runtime's reusable active Caller capacity.
    /// # Errors
    /// Zero/excessive capacity or allocation failure rejects before admission.
    pub fn new(capacity: usize) -> Result<Self, Error> {
        if capacity == 0 || capacity > 65_536 {
            return Err(Error::INVALID);
        }
        let mut entries = Vec::new();
        entries.try_reserve_exact(capacity).map_err(|_| Error::PRESSURE)?;
        Ok(Self { entries, capacity })
    }

    /// Associates one Host-issued credential with an exact granted binding.
    /// An identical association is idempotent, including during retirement.
    /// No request or provider executes here; opening policy stays in the Host.
    /// # Errors
    /// Invalid credentials, conflicting associations, retired credentials and
    /// exhausted credential capacity reject without changing the old Caller.
    pub fn admit(
        &mut self,
        token: [u8; 16],
        binding: &Rc<Binding>,
    ) -> Result<(), Error> {
        if token == [0; 16] {
            return Err(Error::INVALID);
        }
        if let Some(client) = self.entries.iter().find(|c| c.token == token) {
            return repeated(&client.state, binding);
        }
        if self.entries.iter().any(|client| uses(&client.state, binding)) {
            return Err(Error::CONFLICT);
        }
        if self.entries.len() == self.capacity {
            return Err(Error::PRESSURE);
        }
        self.entries.push(Client {
            token,
            state: State::Active {
                binding: binding.clone(),
                endpoint: Endpoint::new(token, binding.caller()),
            },
        });
        Ok(())
    }

    /// Routes one ordinary action under its original Caller authority.
    /// For example, a reconnected socket reads the original request's result.
    #[must_use]
    pub fn dispatch(&self, frame: Frame) -> Reply {
        let Some(client) =
            self.entries.iter().find(|c| c.token == frame.caller)
        else {
            return Reply::Error(Error::DENIED);
        };
        match &client.state {
            State::Active { endpoint, .. } => endpoint.dispatch(frame),
            State::Closed => Reply::Error(Error::CLOSED),
        }
    }

    /// Retires one binding, retaining its endpoint until custody has drained.
    /// Busy/Incomplete retain result, cancellation, reconciliation and release
    /// access. After completion, the credential remains a bounded tombstone.
    /// # Errors
    /// Unknown credentials reject. Runtime reports foreign ownership, live
    /// module use or pending cleanup; retry after driving the same Runtime.
    pub fn retire(
        &mut self,
        runtime: &mut Runtime,
        token: [u8; 16],
    ) -> Result<(), Error> {
        let client = self
            .entries
            .iter_mut()
            .find(|c| c.token == token)
            .ok_or(Error::DENIED)?;
        if let State::Active { binding, .. } = &client.state {
            runtime.retire_binding(binding)?;
        }
        client.state = State::Closed;
        Ok(())
    }
}

fn uses(state: &State, binding: &Rc<Binding>) -> bool {
    matches!(state, State::Active { binding: active, .. }
        if Rc::ptr_eq(active, binding))
}

fn repeated(state: &State, binding: &Rc<Binding>) -> Result<(), Error> {
    match state {
        State::Closed => Err(Error::CLOSED),
        State::Active { .. } if uses(state, binding) => Ok(()),
        State::Active { .. } => Err(Error::CONFLICT),
    }
}
