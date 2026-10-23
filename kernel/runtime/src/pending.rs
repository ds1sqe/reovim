//! Token to outcome. A Driver's own token value is scoped by its Module;
//! the caller sees a Kernel token. A token is the caller's that made
//! it: no other may ask its answer out or cancel it.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;

use reovim_kernel_interface::{
    Caller, Decl, Error, Handle, Hash, ModuleId, Out, Outcome, Rd, Token,
    check_reply,
};

pub enum State {
    Waiting,
    Done(Outcome),
    Cancelled,
}

pub struct Wait {
    /// The caller owed the answer: the one that ran the op. Its `Ask`
    /// collects the outcome and its `Cancel` drops the wait; any other
    /// caller naming this token is `Denied`.
    pub caller: Caller,
    /// The handle the wait is counted against, and for an `open` the
    /// handle the caller will be given; `None` for `lookup` and
    /// `children`, which name none.
    pub handle: Option<Handle>,
    /// This wait is the `open` that made `handle`: its `done` carries
    /// the Driver's cookie where every other carries the answer.
    pub opening: bool,
    /// The Container this wait is the listing of: its `done` is what
    /// fills that rd's cache, so the rd counts as listed when the rows
    /// land and not when the op was run.
    pub listing: Option<Rd>,
    /// The declaration and method whose output a call must answer.
    pub call: Option<(Hash, u16)>,
    pub state: State,
}

impl Wait {
    /// A wait that fills nothing of the Kernel's own in, which is what
    /// every op but an `open` and a `children` makes.
    const fn new(caller: Caller, handle: Option<Handle>) -> Self {
        Self {
            caller,
            handle,
            opening: false,
            listing: None,
            call: None,
            state: State::Waiting,
        }
    }
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
        caller: Caller,
        handle: Option<Handle>,
    ) -> Result<Token, Error> {
        self.begin(module, own, Wait::new(caller, handle))
    }

    /// Remember the output contract on a call's newly opened wait.
    pub fn calling(
        &mut self,
        token: Token,
        api: Hash,
        method: u16,
    ) -> Result<(), Error> {
        self.by_token.get_mut(&token).ok_or(Error::Invalid)?.call =
            Some((api, method));
        Ok(())
    }

    /// The same for the `open` that made `handle`, whose `done` the
    /// Kernel reads as a cookie.
    pub fn opening(
        &mut self,
        module: ModuleId,
        own: u64,
        caller: Caller,
        handle: Handle,
    ) -> Result<Token, Error> {
        let mut wait = Wait::new(caller, Some(handle));
        wait.opening = true;
        self.begin(module, own, wait)
    }

    /// The same for the `children` of `rd`, whose `done` is the
    /// listing that rd's cache then answers from.
    pub fn listing(
        &mut self,
        module: ModuleId,
        own: u64,
        caller: Caller,
        rd: Rd,
    ) -> Result<Token, Error> {
        let mut wait = Wait::new(caller, None);
        wait.listing = Some(rd);
        self.begin(module, own, wait)
    }

    fn begin(
        &mut self,
        module: ModuleId,
        own: u64,
        wait: Wait,
    ) -> Result<Token, Error> {
        if self.by_driver.contains_key(&(module, own)) {
            return Err(Error::Invalid);
        }
        let token = Token(self.next);
        self.next += 1;
        self.by_driver.insert((module, own), token);
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

    /// The Container an outstanding listing of `module` is of, and
    /// `None` for every other wait: what tells a `done` that fills a
    /// cache from one that does not.
    pub fn listed_of(&self, module: ModuleId, own: u64) -> Option<Rd> {
        let token = self.by_driver.get(&(module, own))?;
        self.by_token.get(token)?.listing
    }

    /// The Driver finished; returns the handle the wait counted against.
    pub fn done(
        &mut self,
        module: ModuleId,
        own: u64,
        outcome: Outcome,
        decls: &BTreeMap<Hash, Decl>,
    ) -> Result<Option<Handle>, Error> {
        let token =
            self.by_driver.remove(&(module, own)).ok_or(Error::Invalid)?;
        let wait = self.by_token.get_mut(&token).ok_or(Error::Invalid)?;
        if matches!(wait.state, State::Waiting) {
            wait.state = State::Done(checked(outcome, wait.call, decls));
        }
        Ok(wait.handle)
    }

    /// `Busy` while waiting; the outcome once, then the token is gone.
    /// `Denied` to any caller but the one the answer is owed to, whose
    /// bytes these are.
    pub fn ask(&mut self, caller: Caller, token: Token) -> Outcome {
        let wait = self.by_token.get(&token).ok_or(Error::Invalid)?;
        if wait.caller != caller {
            return Err(Error::Denied);
        }
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
    /// now never land. `Denied` to any caller but the one the wait is
    /// owed to: another caller may not drop this answer.
    pub fn cancel(
        &mut self,
        caller: Caller,
        token: Token,
    ) -> Result<Option<Handle>, Error> {
        let wait = self.by_token.get(&token).ok_or(Error::Invalid)?;
        if wait.caller != caller {
            return Err(Error::Denied);
        }
        Ok(self.stop(token))
    }

    /// The marking alone, for the Kernel's own reasons rather than a
    /// caller's `Cancel`: no caller to check, and an unknown or
    /// finished token is nothing to do.
    fn stop(&mut self, token: Token) -> Option<Handle> {
        let wait = self.by_token.get_mut(&token)?;
        if !matches!(wait.state, State::Waiting) {
            return None;
        }
        wait.state = State::Cancelled;
        if wait.opening { wait.handle } else { None }
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
            self.stop(token);
        }
    }

    /// The caller owed these answers is gone: no `Ask` will ever come
    /// for them, so the waits go rather than stand in the table for
    /// ever. Answers the handles of its `open`s that were still
    /// waiting, whose cookies will never land, as a `Cancel` of one
    /// does.
    pub fn caller_end(&mut self, caller: Caller) -> Vec<Handle> {
        let mine: BTreeSet<Token> = self
            .by_token
            .iter()
            .filter(|(_, wait)| wait.caller == caller)
            .map(|(token, _)| *token)
            .collect();
        self.by_driver.retain(|_, token| !mine.contains(token));
        mine.iter()
            .filter_map(|token| self.by_token.remove(token))
            .filter(|wait| wait.opening && matches!(wait.state, State::Waiting))
            .filter_map(|wait| wait.handle)
            .collect()
    }
}

/// A call can finish only with one value of its declared output type.
fn checked(
    outcome: Outcome,
    call: Option<(Hash, u16)>,
    decls: &BTreeMap<Hash, Decl>,
) -> Outcome {
    match (call, outcome) {
        (Some((api, method)), Ok(Out::Bytes(bytes))) => {
            check_reply(decls, api, method, &bytes).map(|()| Out::Bytes(bytes))
        }
        (Some(_), Ok(_)) => Err(Error::Invalid),
        (_, outcome) => outcome,
    }
}
