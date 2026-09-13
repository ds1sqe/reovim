#![no_std]
#![forbid(unsafe_code)]
//! Resource admission, registered providers and independently driven execution.

extern crate alloc;

mod access;
mod binding;
pub use binding::Binding;
mod catalog;
mod closing;
mod contributions;
mod driver;
mod execution;
mod history;
mod modules;
mod namespace;
mod opening;
mod owned;
mod publication;
mod read_views;
mod registration;
mod request;

pub mod diagnostics;

use alloc::{
    boxed::Box,
    collections::VecDeque,
    rc::{Rc, Weak},
    vec::Vec,
};
use core::{cell::RefCell, task::Context};
mod describe;
mod sequence;
mod sessions;
mod snapshots;
mod teardown;
mod watch;
use reovim_kernel_interface::buffer::ModelProvider;
use reovim_kernel_interface::provider::{Permissions, ResourceProvider};
use reovim_kernel_interface::{Connection, Error, ResourceId};
use sequence::Sequence;

use diagnostics::{Event, Stage};
use namespace::{Entry, Kind, Scope};
use request::Record;

/// Finite reservations; pressure rejects before admitting another effect.
#[derive(Clone, Copy)]
pub struct Limits {
    pub resources: usize,
    pub requests: usize,
    pub callers: usize,
    pub message_bytes: usize,
    pub path_bytes: usize,
    pub history_nodes: usize,
    pub state_bytes: u64,
    pub events: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            resources: 256,
            requests: 64,
            callers: 16,
            message_bytes: 1024 * 1024,
            path_bytes: 1024,
            history_nodes: 256,
            state_bytes: 32 * 1024 * 1024,
            events: 256,
        }
    }
}

pub use reovim_kernel_interface::driver::Progress;

struct Editing {
    state: history::History,
    model: Rc<RefCell<ModelProvider>>,
    resource: u64,
    history: u64,
    owner: u64,
    feed: Rc<RefCell<watch::Feed>>,
    backing: Option<opening::Association>,
}

struct Shared {
    identity: u64,
    limits: Limits,
    ids: Sequence,
    ready: bool,
    closing: bool,
    entries: Vec<Entry>,
    scopes: Vec<Weak<RefCell<Scope>>>,
    requests: Vec<Rc<RefCell<Record>>>,
    events: VecDeque<Event>,
    event_sequence: Sequence,
    lost_events: u64,
    active: Option<u64>,
}

/// Composition owns this driver. Ordinary consumers receive only Connection.
pub struct Runtime {
    shared: Rc<RefCell<Shared>>,
    cursor: usize,
    modules: Vec<modules::Instance>,
}

impl Runtime {
    /// # Errors
    /// Rejects unusable bounds or unavailable reservation storage.
    pub fn new(identity: u64, limits: Limits) -> Result<Self, Error> {
        if identity == 0
            || limits.resources < 3
            || limits.requests == 0
            || limits.callers == 0
            || limits.message_bytes == 0
            || limits.path_bytes == 0
            || limits.history_nodes == 0
            || limits.events == 0
        {
            return Err(Error::INVALID);
        }
        let mut entries = reserve(limits.resources)?;
        entries.push(Entry::directory(1, 1, ""));
        entries.push(Entry::directory(2, 1, "requests"));
        entries.push(Entry::directory(3, 1, "buffers"));
        let mut events = VecDeque::new();
        events.try_reserve_exact(limits.events).map_err(|_| Error::PRESSURE)?;
        Ok(Self {
            shared: Rc::new(RefCell::new(Shared {
                identity,
                limits,
                ids: Sequence::after(3),
                ready: false,
                closing: false,
                entries,
                scopes: reserve(limits.callers)?,
                requests: reserve(limits.requests)?,
                events,
                event_sequence: Sequence::default(),
                lost_events: 0,
                active: None,
            })),
            cursor: 0,
            modules: Vec::new(),
        })
    }

    /// # Errors
    /// A closing Runtime cannot restart.
    pub fn start(&mut self) -> Result<(), Error> {
        let mut shared = self.shared.borrow_mut();
        if shared.closing {
            return Err(Error::CLOSED);
        }
        shared.ready = true;
        Ok(())
    }

    /// Resolves an owner-namespace identity for composition grants.
    /// Caller code uses Connection lookup and receives its own scoped IDs.
    /// # Errors
    /// Rejects invalid or unavailable paths without executing providers.
    pub fn resource(&self, path: &str) -> Result<ResourceId, Error> {
        self.shared.borrow().path(1, path).map(ResourceId)
    }

    /// Installs a namespace Resource for selected providers and shared aliases.
    /// # Errors
    /// Rejects duplicate/invalid paths, closing state or exhausted capacity.
    pub fn register_namespace(
        &mut self,
        path: &str,
    ) -> Result<ResourceId, Error> {
        let mut shared = self.shared.borrow_mut();
        shared.check_registration(path)?;
        Ok(ResourceId(shared.insert(path, Kind::Directory)))
    }

    /// Register a live provider; failure returns its unchanged ownership.
    /// # Errors
    /// Rejects duplicate names, closing state or exhausted capacity.
    pub fn register(
        &mut self,
        path: &str,
        provider: ResourceProvider,
    ) -> Result<ResourceId, (Error, ResourceProvider)> {
        if provider.is_owned() {
            return Err((Error::INVALID, provider));
        }
        let validation =
            self.shared.borrow().check_registration(path).and_then(|()| {
                registration::resource(&provider, self.shared.borrow().limits)
            });
        if let Err(error) = validation {
            return Err((error, provider));
        }
        let provider = Rc::new(RefCell::new(provider));
        let key =
            self.shared.borrow_mut().insert(path, Kind::Provider(provider));
        Ok(ResourceId(key))
    }

    /// # Errors
    /// Rejects incompatible names or capacity before taking model ownership.
    pub fn register_model(
        &mut self,
        path: &str,
        provider: ModelProvider,
    ) -> Result<ResourceId, (Error, Box<ModelProvider>)> {
        let validation =
            self.shared.borrow().check_registration(path).and_then(|()| {
                registration::model(&provider, self.shared.borrow().limits)
            });
        if let Err(error) = validation {
            return Err((error, Box::new(provider)));
        }
        let provider = Rc::new(RefCell::new(provider));
        let key = self.shared.borrow_mut().insert(path, Kind::Model(provider));
        Ok(ResourceId(key))
    }

    /// # Errors
    /// Busy leaves the live provider intact while work or model state needs it.
    pub fn unregister(
        &mut self,
        registration: ResourceId,
    ) -> Result<(), Error> {
        self.unregister_contributions(&[registration])
    }

    /// Composition grants exact subjects/subtrees, independently per caller.
    /// # Errors
    /// Rejects unknown grants, closing state or caller capacity exhaustion.
    pub fn connect(
        &mut self,
        grants: &[(ResourceId, Permissions)],
    ) -> Result<Connection, Error> {
        Ok(self.bind(grants)?.connection())
    }

    /// Runs one bounded work turn; caller Future polling never enters here.
    /// # Errors
    /// Reports unavailable startup or an exhausted internal identity space.
    pub fn advance(
        &mut self,
        context: &mut Context<'_>,
    ) -> Result<Progress, Error> {
        if !self.shared.borrow().ready && !self.shared.borrow().closing {
            return Err(Error::NOT_READY);
        }
        owned::reclaim(&self.shared);
        read_views::reclaim(&self.shared);
        let module = self.advance_modules(context);
        if !self.shared.borrow().ready {
            return Ok(module);
        }
        let request = self.advance_one(context)?;
        Ok(if module == Progress::More { Progress::More } else { request })
    }

    /// Fences admission. Unobserved results remain recoverable until adopted.
    /// # Errors
    /// Busy means requests or results still have owners to settle.
    pub fn close(&mut self) -> Result<(), Error> {
        self.shared.borrow_mut().closing = true;
        owned::close_all(&self.shared)?;
        read_views::reclaim(&self.shared);
        owned::reclaim(&self.shared);
        self.close_modules()?;
        close_providers(self)?;
        if !self.shared.borrow().requests.is_empty() {
            return Err(Error::BUSY);
        }
        let editing = detach_editing(&mut self.shared.borrow_mut())?;
        drop(editing);
        if !self.close_modules()? {
            return Err(Error::BUSY);
        }
        close_providers(self)?;
        if self.shared.borrow().entries.iter().any(|entry| {
            matches!(entry.kind, Kind::Provider(_) | Kind::Model(_))
        }) {
            return Err(Error::BUSY);
        }
        let entries = core::mem::take(&mut self.shared.borrow_mut().entries);
        drop(entries);
        Ok(())
    }

    /// Passive copied diagnostics; each reader retains its own sequence.
    #[must_use]
    pub fn events(&self, after: u64) -> Vec<Event> {
        self.shared
            .borrow()
            .events
            .iter()
            .filter(|event| event.sequence > after)
            .copied()
            .collect()
    }

    #[must_use]
    pub fn lost_events(&self) -> u64 {
        self.shared.borrow().lost_events
    }
}

impl Shared {
    fn id(&mut self) -> Result<u64, Error> {
        self.ids.take().ok_or(Error::PRESSURE)
    }

    fn emit(
        &mut self,
        request: u64,
        scope: u64,
        subject: u64,
        stage: Stage,
        parent: Option<u64>,
    ) {
        let Some(sequence) = self.event_sequence.take() else {
            self.lost_events = self.lost_events.saturating_add(1);
            return;
        };
        if self.events.len() == self.limits.events {
            self.events.pop_front();
            self.lost_events = self.lost_events.saturating_add(1);
        }
        self.events.push_back(Event {
            sequence,
            request,
            scope,
            subject,
            stage,
            parent,
        });
    }
}

fn reserve<T>(capacity: usize) -> Result<Vec<T>, Error> {
    let mut values = Vec::new();
    values.try_reserve_exact(capacity).map_err(|_| Error::PRESSURE)?;
    Ok(values)
}

fn unload(kind: &Kind) -> Result<(), Error> {
    match kind {
        Kind::Provider(provider) => provider.borrow_mut().unload(),
        Kind::Model(model) => model.borrow_mut().unload(),
        _ => Err(Error::UNSUPPORTED),
    }
}

fn detach_editing(shared: &mut Shared) -> Result<Vec<Entry>, Error> {
    let mut retired = Vec::new();
    retired
        .try_reserve_exact(shared.entries.len())
        .map_err(|_| Error::PRESSURE)?;
    let mut index = 0;
    while index < shared.entries.len() {
        if matches!(
            shared.entries[index].kind,
            Kind::Buffer(_) | Kind::History(_) | Kind::Session(_)
        ) {
            retired.push(shared.entries.remove(index));
        } else {
            index += 1;
        }
    }
    Ok(retired)
}

fn close_providers(runtime: &mut Runtime) -> Result<(), Error> {
    let providers: Vec<_> = runtime
        .shared
        .borrow()
        .entries
        .iter()
        .filter(|entry| {
            matches!(entry.kind, Kind::Provider(_) | Kind::Model(_))
        })
        .map(|entry| ResourceId(entry.key))
        .collect();
    for provider in providers {
        if runtime.module_contribution(provider) {
            continue;
        }
        match runtime.unregister(provider) {
            Ok(()) => (),
            Err(error) if error == Error::BUSY => (),
            Err(error) => return Err(error),
        }
    }
    Ok(())
}
