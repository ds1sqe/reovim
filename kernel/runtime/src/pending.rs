//! Token to outcome. A Driver's own token value is scoped by its Module;
//! the caller sees a Kernel token.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use reovim_kernel_interface::{Error, Handle, ModuleId, Outcome, Token};

pub enum State {
    Waiting,
    Done(Outcome),
    Cancelled,
}

pub struct Wait {
    /// The handle the wait is counted against, and for an `open` the
    /// handle the caller will be given; `None` for `lookup` and
    /// `children`, which name none.
    pub handle: Option<Handle>,
    /// This wait is the `open` that made `handle`: its `done` carries
    /// the Driver's cookie where every other carries the answer.
    pub opening: bool,
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
        handle: Option<Handle>,
    ) -> Result<Token, Error> {
        self.begin(module, own, handle, false)
    }

    /// The same for the `open` that made `handle`, whose `done` the
    /// Kernel reads as a cookie.
    pub fn opening(
        &mut self,
        module: ModuleId,
        own: u64,
        handle: Handle,
    ) -> Result<Token, Error> {
        self.begin(module, own, Some(handle), true)
    }

    fn begin(
        &mut self,
        module: ModuleId,
        own: u64,
        handle: Option<Handle>,
        opening: bool,
    ) -> Result<Token, Error> {
        if self.by_driver.contains_key(&(module, own)) {
            return Err(Error::Invalid);
        }
        let token = Token(self.next);
        self.next += 1;
        self.by_driver.insert((module, own), token);
        let wait = Wait { handle, opening, state: State::Waiting };
        self.by_token.insert(token, wait);
        Ok(token)
    }

    /// The handle of an outstanding `open` of `module`, and `None` for
    /// every other wait: what tells a `done` carrying a cookie from a
    /// `done` carrying the caller's own answer.
    pub fn cookie_of(&self, module: ModuleId, own: u64) -> Option<Handle> {
        let token = self.by_driver.get(&(module, own))?;
        let wait = self.by_token.get(token)?;
        if wait.opening { wait.handle } else { None }
    }

    /// The Driver finished; returns the handle the wait counted against.
    pub fn done(
        &mut self,
        module: ModuleId,
        own: u64,
        outcome: Outcome,
    ) -> Result<Option<Handle>, Error> {
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

    /// Every outstanding token with its wait; the core writes them.
    pub fn all(&self) -> impl Iterator<Item = (Token, &Wait)> {
        self.by_token.iter().map(|(token, wait)| (*token, wait))
    }

    /// Mark a waiting token; a finished one is left alone. Answers the
    /// handle of an `open` that was still waiting, whose cookie will
    /// now never land.
    pub fn cancel(&mut self, token: Token) -> Result<Option<Handle>, Error> {
        let wait = self.by_token.get_mut(&token).ok_or(Error::Invalid)?;
        if !matches!(wait.state, State::Waiting) {
            return Ok(None);
        }
        wait.state = State::Cancelled;
        Ok(if wait.opening { wait.handle } else { None })
    }

    /// A Module is gone: nothing will finish its waits, so a caller's
    /// `Ask` answers `Cancelled` and not `Busy` for ever. A token
    /// already done keeps its outcome.
    pub fn ended(&mut self, module: ModuleId) {
        let of_it = |(m, _): &&(ModuleId, u64)| *m == module;
        let mine: Vec<(ModuleId, u64)> =
            self.by_driver.keys().filter(of_it).copied().collect();
        let gone: Vec<Token> =
            mine.iter().filter_map(|key| self.by_driver.remove(key)).collect();
        for token in gone {
            let _ = self.cancel(token);
        }
    }
}
