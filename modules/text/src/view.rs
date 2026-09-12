#![allow(clippy::future_not_send)]
use crate::{
    Factory,
    snapshot::{self, Snapshot},
    watch::{self, Watch},
};
use alloc::{
    boxed::Box,
    rc::{Rc, Weak},
    vec::Vec,
};
use core::cell::{Cell, RefCell};
use reovim_kernel_interface::{
    Connection, Contract, Error, Message, RequestError, ResourceId,
    buffer::{self, Watching},
    provider::{Permissions, ResourceOps, ResourceProvider, Surface},
};

pub struct View {
    pub factory: Rc<Factory>,
    pub parent: ResourceId,
    pub buffer: ResourceId,
    pub writable: bool,
    pub kernel: RefCell<Option<Connection>>,
    pub closed: Cell<bool>,
    pub active: Cell<usize>,
    pub snapshots: RefCell<Vec<Weak<Snapshot>>>,
    pub watches: RefCell<Vec<Weak<Watch>>>,
    pub pending: RefCell<Vec<Watching>>,
    pub index_hint: RefCell<Option<(buffer::BufferState, crate::index::Index)>>,
}

struct Active(Rc<View>);
impl Drop for Active {
    fn drop(&mut self) {
        self.0.active.set(self.0.active.get() - 1);
    }
}

impl View {
    fn admit(self: &Rc<Self>) -> Result<Active, RequestError> {
        if self.closed.get() {
            return Err(Error::CLOSED.into());
        }
        if self.active.get() == 32 {
            return Err(Error::PRESSURE.into());
        }
        self.active.set(self.active.get() + 1);
        Ok(Active(self.clone()))
    }

    fn kernel(&self) -> Result<Connection, Error> {
        if self.closed.get() {
            return Err(Error::CLOSED);
        }
        self.kernel.borrow().clone().ok_or(Error::CLOSED)
    }

    fn reserve(&self) -> Result<(), Error> {
        let mut snapshots = self.snapshots.borrow_mut();
        snapshots
            .retain(|weak| weak.upgrade().is_some_and(|s| !s.closed.get()));
        let mut watches = self.watches.borrow_mut();
        watches.retain(|weak| weak.upgrade().is_some_and(|w| !w.closed.get()));
        if snapshots.len() + self.active.get() > 32
            || watches.len() + self.active.get() > 32
        {
            return Err(Error::PRESSURE);
        }
        snapshots
            .try_reserve(self.active.get())
            .map_err(|_| Error::PRESSURE)?;
        watches.try_reserve(self.active.get()).map_err(|_| Error::PRESSURE)?;
        self.pending
            .borrow_mut()
            .try_reserve(self.active.get())
            .map_err(|_| Error::PRESSURE)
    }

    async fn observe(
        &self,
        subject: ResourceId,
        input: Message,
    ) -> Result<Message, RequestError> {
        if !input.data.is_empty() {
            return Err(Error::INVALID.into());
        }
        self.reserve()?;
        let kernel = self.kernel()?;
        let watched =
            buffer::watching(&buffer::observe(&kernel, self.buffer).await?)?;
        self.pending.borrow_mut().push(watched);
        let result = self.prepare(&kernel, subject, watched, input).await;
        if result.is_err() {
            release(&kernel, watched)?;
        }
        self.pending
            .borrow_mut()
            .retain(|p| p.snapshot.id != watched.snapshot.id);
        result
    }

    async fn prepare(
        &self,
        kernel: &Connection,
        subject: ResourceId,
        watched: Watching,
        input: Message,
    ) -> Result<Message, RequestError> {
        let value = kernel
            .read(
                watched.snapshot.id,
                &Message::new(Contract::new("source.bytes", 1), &[]),
            )
            .await?;
        let length = buffer::integer(&value.data)?;
        if self.closed.get() {
            return Err(Error::CLOSED.into());
        }
        let snapshot = Rc::new(Snapshot {
            kernel: kernel.capture(watched.snapshot.id, Permissions::READ)?,
            source: watched.snapshot,
            length,
            id: Cell::new(ResourceId(0)),
            closed: Cell::new(false),
            active: Cell::new(0),
            index: RefCell::new(
                self.index_hint
                    .borrow()
                    .as_ref()
                    .filter(|(state, _)| *state == watched.snapshot.state)
                    .and_then(|(_, index)| index.copy())
                    .unwrap_or_default(),
            ),
        });
        let watch = Rc::new(Watch {
            kernel: kernel.capture(watched.watch, Permissions::READ)?,
            source: watched.watch,
            closed: Cell::new(false),
            ended: Cell::new(false),
        });
        self.publish(subject, input, watched, &snapshot, &watch)
    }

    fn publish(
        &self,
        subject: ResourceId,
        input: Message,
        watched: Watching,
        snapshot: &Rc<Snapshot>,
        watch: &Rc<Watch>,
    ) -> Result<Message, RequestError> {
        let id = self.factory.publish(
            self.parent,
            "snapshot",
            snapshot::provider(snapshot.clone()),
        )?;
        snapshot.id.set(id);
        let output = self.factory.publish(
            self.parent,
            "watch",
            watch::provider(watch.clone()),
        );
        let stream = match output {
            Ok(id) => id,
            Err(error) => {
                self.factory.kernel.close(id).into_inner()?;
                return Err(error.into());
            }
        };
        self.snapshots.borrow_mut().push(Rc::downgrade(snapshot));
        self.watches.borrow_mut().push(Rc::downgrade(watch));
        let data = [
            watched.snapshot.state.revision.to_le_bytes(),
            watched.snapshot.node.to_le_bytes(),
            snapshot.length.to_le_bytes(),
        ]
        .concat();
        Ok(Message {
            contract: input.contract,
            data: data.into(),
            resources: Box::new([
                subject,
                subject,
                self.buffer,
                watched.history,
                id,
                stream,
            ]),
        })
    }

    async fn replace(&self, input: Message) -> Result<Message, RequestError> {
        if !self.writable {
            return Err(Error::DENIED.into());
        }
        let kernel = self.kernel()?;
        let snapshot = self
            .snapshots
            .borrow()
            .iter()
            .filter_map(Weak::upgrade)
            .find(|s| s.id.get() == input.resources[0] && !s.closed.get())
            .ok_or(Error::STALE)?;
        let _snapshot_use = snapshot.acquire()?;
        if input.data.len() < 16 {
            return Err(Error::INVALID.into());
        }
        let start = buffer::integer(&input.data[..8])?;
        let end = buffer::integer(&input.data[8..16])?;
        let text = core::str::from_utf8(&input.data[16..])
            .map_err(|_| Error::INVALID)?;
        let range = snapshot.range(start..end).await?;
        if self.closed.get() {
            return Err(Error::CLOSED.into());
        }
        let accepted = reovim_module_source::api::splice(
            &kernel,
            &snapshot.source,
            range.clone(),
            text.as_bytes(),
        )
        .await?;
        let index = snapshot.index.borrow().splice(
            start..end,
            range,
            text.chars().count() as u64,
            text.len() as u64,
        );
        *self.index_hint.borrow_mut() =
            index.map(|index| (accepted.state, index));
        let mut data = [
            accepted.state.revision.to_le_bytes(),
            accepted.node.to_le_bytes(),
        ]
        .concat();
        data.extend_from_slice(&accepted.value.data);
        Ok(Message {
            contract: input.contract,
            data: data.into(),
            resources: Box::new([accepted.state.buffer, accepted.history]),
        })
    }

    fn close(&self) -> Result<(), Error> {
        self.closed.set(true);
        if self.active.get() != 0 {
            return Err(Error::BUSY);
        }
        let kernel = self.kernel.borrow().clone();
        for pending in self.pending.borrow().iter() {
            release(kernel.as_ref().ok_or(Error::PROVIDER)?, *pending)?;
        }
        self.pending.borrow_mut().clear();
        for watch in self.watches.borrow().iter().filter_map(Weak::upgrade) {
            watch.end()?;
        }
        self.kernel.borrow_mut().take();
        self.index_hint.borrow_mut().take();
        Ok(())
    }
}

fn release(kernel: &Connection, watched: Watching) -> Result<(), Error> {
    kernel.close(watched.watch).into_inner().map_err(cause)?;
    kernel.close(watched.snapshot.id).into_inner().map_err(cause)
}

pub fn cause(error: RequestError) -> Error {
    match error {
        RequestError::Rejected(e) | RequestError::Failed(e) => e,
        RequestError::Unknown { cause, recovery } => {
            drop(recovery);
            cause
        }
    }
}

pub fn provider(view: Rc<View>) -> ResourceProvider {
    let closing = view.clone();
    ResourceProvider::new(
        view,
        ResourceOps {
            unload: None,
            read: |view, subject, input| {
                let active = view.admit();
                let view = view.clone();
                Box::pin(async move {
                    let _active = active?;
                    view.observe(subject, input).await
                })
            },
            write: |view, _, input| {
                let active = view.admit();
                let view = view.clone();
                Box::pin(async move {
                    let _active = active?;
                    view.replace(input).await
                })
            },
        },
        Box::new([
            Surface {
                contract: Contract::new("text.observe", 1),
                methods: Permissions::READ,
                input: Box::new([]),
                output: Box::new([Permissions::READ; 6]),
            },
            Surface {
                contract: Contract::new("text.replace", 1),
                methods: Permissions::WRITE,
                input: Box::new([Permissions::READ]),
                output: Box::new([Permissions::READ; 2]),
            },
        ]),
        Rc::new(()),
    )
    .try_owned(move || closing.close())
}
