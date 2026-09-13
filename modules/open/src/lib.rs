#![no_std]
#![forbid(unsafe_code)]
#![allow(clippy::future_not_send)]
//! Per-client file opening through granted capture and Source Resources.

extern crate alloc;

mod recovery;
#[cfg(test)]
mod tests;

use alloc::{boxed::Box, rc::Rc, vec::Vec};
use core::cell::{Cell, RefCell};
use reovim_kernel_interface::buffer;
use reovim_module::{
    Connection, Contract, Error, Message, Permissions, RequestError, ResourceId,
};

const OPEN: Contract = Contract::new("command.open", 1)
    .input(&[Permissions::WRITE, Permissions::WRITE])
    .output(&[Permissions::ALL, Permissions::ALL]);
const DRAIN: Contract = Contract::new("command.open.cleanup", 1);
const RESUME: Contract = Contract::new("command.open.resume", 1)
    .input(&[Permissions::READ])
    .output(&[Permissions::ALL, Permissions::ALL]);
const INFO: Contract = Contract::new("resource.bytes", 1);

struct Pending {
    model: RefCell<Option<(Connection, ResourceId)>>,
    snapshot: Cell<Option<ResourceId>>,
    recovery: RefCell<Option<Box<str>>>,
    work: RefCell<Vec<ResourceId>>,
    running: Cell<bool>,
}
struct Active(Rc<Pending>);
impl Drop for Active {
    fn drop(&mut self) {
        self.0.running.set(false);
    }
}

struct Command {
    kernel: Connection,
    pending: RefCell<Vec<Rc<Pending>>>,
}
impl Command {
    const fn new(kernel: Connection) -> Self {
        Self { kernel, pending: RefCell::new(Vec::new()) }
    }

    fn reserve(&self) -> Result<Active, Error> {
        self.cleanup()?;
        let mut pending = self.pending.borrow_mut();
        if pending.len() == 16 {
            return Err(Error::PRESSURE);
        }
        pending.try_reserve(1).map_err(|_| Error::PRESSURE)?;
        let mut work = Vec::new();
        work.try_reserve_exact(recovery::LIMIT).map_err(|_| Error::PRESSURE)?;
        let job = Rc::new(Pending {
            model: RefCell::new(None),
            snapshot: Cell::new(None),
            recovery: RefCell::new(None),
            work: RefCell::new(work),
            running: Cell::new(true),
        });
        pending.push(job.clone());
        Ok(Active(job))
    }

    async fn open(
        &self,
        _: ResourceId,
        input: Message,
    ) -> Result<Message, RequestError> {
        let name =
            core::str::from_utf8(&input.data).map_err(|_| Error::INVALID)?;
        if name.is_empty() || input.resources.len() != 2 {
            return Err(Error::INVALID.into());
        }
        let active = self.reserve()?;
        let result = self.execute(&active.0, &input).await;
        drop(active);
        // Cleanup retains any rejected close for retry and module unload.
        // A created Buffer remains the successful result even if close fails.
        let _ = self.cleanup();
        result
    }

    async fn execute(
        &self,
        pending: &Pending,
        input: &Message,
    ) -> Result<Message, RequestError> {
        let model = input.resources[1];
        *pending.model.borrow_mut() =
            Some((self.kernel.capture(model, Permissions::WRITE)?, model));
        let capture = self.kernel.write(
            input.resources[0],
            &Message::new(
                reovim_kernel_interface::Contract::new("host.files.capture", 1),
                &input.data,
            ),
        );
        let captured = self.settle(pending, capture.await).await?;
        self.open_captured(pending, captured, &input.contract).await
    }

    async fn open_captured(
        &self,
        pending: &Pending,
        captured: Message,
        contract: &reovim_kernel_interface::Contract,
    ) -> Result<Message, RequestError> {
        let [snapshot] = captured.resources.as_ref() else {
            return Err(Error::PROVIDER.into());
        };
        pending.snapshot.set(Some(*snapshot));
        let (kernel, model) =
            pending.model.borrow().clone().ok_or(Error::PROVIDER)?;
        let opened = kernel.write(
            model,
            &Message::reference(
                reovim_kernel_interface::Contract::new("source.open", 1),
                *snapshot,
            ),
        );
        let reply = self.settle(pending, opened.await).await?;
        Self::accepted(reply, contract)
    }

    fn accepted(
        mut reply: Message,
        contract: &reovim_kernel_interface::Contract,
    ) -> Result<Message, RequestError> {
        let observed = buffer::observation(reply.clone())?;
        if observed.value.contract
            != reovim_kernel_interface::Contract::new("source.bytes", 1)
            || observed.value.data.len() != 8
        {
            return Err(Error::PROVIDER.into());
        }
        reply.contract = contract.clone();
        Ok(reply)
    }

    async fn resume(
        &self,
        _: ResourceId,
        input: Message,
    ) -> Result<Message, RequestError> {
        let [work] = input.resources.as_ref() else {
            return Err(Error::INVALID.into());
        };
        if !input.data.is_empty() {
            return Err(Error::INVALID.into());
        }
        let (job, path) = recovery::opening(self, *work).await?;
        if job.running.replace(true) {
            return Err(Error::BUSY.into());
        }
        let active = Active(job);
        let result = self.continue_open(&active.0, path, &input.contract).await;
        drop(active);
        let _ = self.cleanup();
        result
    }

    fn find(&self, work: ResourceId) -> Result<(Rc<Pending>, Box<str>), Error> {
        let jobs = self.pending.borrow().clone();
        jobs.into_iter()
            .find_map(|job| self.match_work(job, work))
            .ok_or(Error::UNAVAILABLE)
    }

    fn match_work(
        &self,
        job: Rc<Pending>,
        work: ResourceId,
    ) -> Option<(Rc<Pending>, Box<str>)> {
        let path = job.recovery.borrow().clone()?;
        let id = self.kernel.resolve(&path).into_inner().ok()?;
        (id == work).then_some((job, path))
    }

    async fn continue_open(
        &self,
        job: &Pending,
        path: Box<str>,
        contract: &reovim_kernel_interface::Contract,
    ) -> Result<Message, RequestError> {
        let unknown = Err(RequestError::Unknown {
            cause: Error::UNAVAILABLE,
            recovery: path,
        });
        let result = self.settle(job, unknown).await?;
        if job.snapshot.get().is_some() {
            Self::accepted(result, contract)
        } else {
            self.open_captured(job, result, contract).await
        }
    }

    async fn settle(
        &self,
        pending: &Pending,
        result: Result<Message, RequestError>,
    ) -> Result<Message, RequestError> {
        recovery::settle(&self.kernel, pending, result, true).await
    }

    fn cleanup(&self) -> Result<(), Error> {
        let jobs = self.pending.borrow().clone();
        for job in jobs {
            self.release(&job)?;
        }
        self.pending.borrow_mut().retain(|job| {
            job.running.get()
                || job.snapshot.get().is_some()
                || !job.work.borrow().is_empty()
                || job.recovery.borrow().is_some()
        });
        Ok(())
    }

    fn release(&self, job: &Pending) -> Result<(), Error> {
        if job.running.get() || job.recovery.borrow().is_some() {
            return Ok(());
        }
        self.close(&job.snapshot)?;
        recovery::release(&self.kernel, job)
    }

    fn close(&self, slot: &Cell<Option<ResourceId>>) -> Result<(), Error> {
        let Some(id) = slot.get() else { return Ok(()) };
        self.kernel.close(id).into_inner().map_err(|error| code(&error))?;
        slot.set(None);
        Ok(())
    }

    fn unload(&self) -> Result<(), Error> {
        self.cleanup()?;
        if !self.pending.borrow().is_empty() {
            return Err(Error::BUSY);
        }
        Ok(())
    }

    async fn drain(
        &self,
        _: ResourceId,
        input: Message,
    ) -> Result<Message, RequestError> {
        if !input.data.is_empty() || !input.resources.is_empty() {
            return Err(Error::INVALID.into());
        }
        let jobs = self.pending.borrow().clone();
        for job in jobs {
            self.reconcile(&job).await?;
        }
        self.unload()?;
        Ok(Message::new(input.contract, &[]))
    }

    async fn reconcile(&self, job: &Pending) -> Result<(), RequestError> {
        if job.running.get() {
            return Err(Error::BUSY.into());
        }
        let recovery = job.recovery.borrow().clone();
        let Some(recovery) = recovery else { return Ok(()) };
        let unknown =
            Err(RequestError::Unknown { cause: Error::UNAVAILABLE, recovery });
        let result = recovery::settle(&self.kernel, job, unknown, false).await;
        if let Err(error @ RequestError::Unknown { .. }) = result {
            return Err(error);
        }
        if job.snapshot.get().is_none() {
            Self::recovered_capture(job, &result)?;
        }
        // The original caller still owns this exposed recovery result. Drain
        // settles capture cleanup without closing that caller's work access.
        job.recovery.borrow_mut().take();
        Ok(())
    }

    fn recovered_capture(
        job: &Pending,
        result: &Result<Message, RequestError>,
    ) -> Result<(), Error> {
        let Ok(reply) = result else { return Ok(()) };
        let [snapshot] = reply.resources.as_ref() else {
            return Err(Error::PROVIDER);
        };
        job.snapshot.set(Some(*snapshot));
        Ok(())
    }

    #[allow(clippy::unused_self)]
    fn info(
        &self,
        _: ResourceId,
        input: Message,
    ) -> core::future::Ready<Result<Message, Error>> {
        core::future::ready(Ok(Message::new(
            input.contract,
            b"write\tcommand.open\t1\tutf8\noutput\tmessage\n",
        )))
    }
}

const fn code(error: &RequestError) -> Error {
    match error {
        RequestError::Rejected(error) | RequestError::Failed(error) => *error,
        RequestError::Unknown { cause, .. } => *cause,
    }
}

reovim_module::rv_module! {
    pub MODULE {
        contract: ("open", 1),
        state: Command::new,
        unload: Command::unload,
        resources: {
            "" => { write OPEN => Command::open,
                write RESUME => Command::resume,
                write DRAIN => Command::drain },
            "info" => { read INFO => Command::info },
        },
    }
}
