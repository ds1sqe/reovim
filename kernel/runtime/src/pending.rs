//! Token to outcome. A Driver's own token value is scoped by its Module;
//! the caller sees a Kernel token.

use alloc::collections::BTreeMap;

use reovim_kernel_interface::{Error, Handle, ModuleId, Outcome, Token};

pub enum State {
    Waiting,
    Done(Outcome),
    Cancelled,
}

pub struct Wait {
    pub handle: Handle,
    pub state: State,
}

pub struct Pending {
    by_token: BTreeMap<Token, Wait>,
    by_driver: BTreeMap<(ModuleId, u64), Token>,
    next: u64,
}

impl Pending {
    pub const fn new() -> Self {
        Self { by_token: BTreeMap::new(), by_driver: BTreeMap::new(), next: 1 }
    }

    /// A slot answered `Pending(own)`; `Invalid` if `own` is outstanding.
    pub fn open(
        &mut self,
        module: ModuleId,
        own: u64,
        handle: Handle,
    ) -> Result<Token, Error> {
        if self.by_driver.contains_key(&(module, own)) {
            return Err(Error::Invalid);
        }
        let token = Token(self.next);
        self.next += 1;
        self.by_driver.insert((module, own), token);
        self.by_token.insert(token, Wait { handle, state: State::Waiting });
        Ok(token)
    }

    /// The Driver finished; returns the handle the wait counted against.
    pub fn done(
        &mut self,
        module: ModuleId,
        own: u64,
        outcome: Outcome,
    ) -> Result<Handle, Error> {
        let token =
            self.by_driver.remove(&(module, own)).ok_or(Error::Invalid)?;
        let wait = self.by_token.get_mut(&token).ok_or(Error::Invalid)?;
        if matches!(wait.state, State::Waiting) {
            wait.state = State::Done(outcome);
        }
        Ok(wait.handle)
    }

    /// `Busy` while waiting; the outcome once, then the token is gone.
    pub fn ask(&mut self, token: Token) -> Outcome {
        let wait = self.by_token.get(&token).ok_or(Error::Invalid)?;
        let outcome = match &wait.state {
            State::Waiting => return Err(Error::Busy),
            State::Done(outcome) => outcome.clone(),
            State::Cancelled => Err(Error::Cancelled),
        };
        self.by_token.remove(&token);
        outcome
    }

    /// Mark a waiting token; a finished one is left alone.
    pub fn cancel(&mut self, token: Token) -> Result<(), Error> {
        let wait = self.by_token.get_mut(&token).ok_or(Error::Invalid)?;
        if matches!(wait.state, State::Waiting) {
            wait.state = State::Cancelled;
        }
        Ok(())
    }
}
