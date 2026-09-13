use crate::remote::{Remote, closed, fields, message};
use reovim_client_tui::context::Window;
use reovim_client_tui::operation::{Change, Operation};
use reovim_kernel_interface::{Contract, Message, ResourceId, buffer};
use reovim_module_text::api::{self, Observation, Window as SourceWindow};
use std::io;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub struct Session {
    pub journal: Option<crate::journal::Journal>,
    pub remote: Remote,
    pub buffer: ResourceId,
    pub access: Option<ResourceId>,
    pub operation: Operation,
    pub observed: Option<Observation>,
    pub retiring: Vec<ResourceId>,
    pub accepted: Option<buffer::BufferState>,
    pub cancel: Arc<AtomicBool>,
}

pub fn change_state(
    change: Change,
    reply: &str,
    id: ResourceId,
) -> io::Result<buffer::BufferState> {
    let reply = message(reply)?;
    let state = match change {
        Change::Replace { .. } => {
            buffer::observation(reply).map(|value| value.state)
        }
        Change::History { redo } => buffer::history_state(&reply, id, redo),
    }
    .map_err(io::Error::other)?;
    if state.buffer != id {
        return Err(io::Error::other("foreign Buffer acceptance"));
    }
    Ok(state)
}

impl Session {
    pub fn persist(&self) -> io::Result<()> {
        self.journal.as_ref().map_or(Ok(()), |journal| journal.save(self))
    }

    fn call<T>(
        &mut self,
        fields: &[String],
        decode: impl FnOnce(&str) -> io::Result<T>,
    ) -> io::Result<T> {
        self.persist()?;
        let frame = self.remote.prepare(fields)?;
        if let Err(error) = self.persist() {
            self.remote.abandon_prepared();
            return Err(error);
        }
        let reply = self.remote.transmit(&frame)?;
        self.persist()?;
        self.outcome(reply, decode)
    }

    fn result<T>(
        &mut self,
        decode: impl FnOnce(&str) -> io::Result<T>,
    ) -> io::Result<T> {
        self.persist()?;
        let reply = self.remote.recover_response()?;
        self.persist()?;
        self.outcome(reply, decode)
    }

    fn outcome<T>(
        &mut self,
        reply: String,
        decode: impl FnOnce(&str) -> io::Result<T>,
    ) -> io::Result<T> {
        if reply.starts_with("error\t") {
            self.finish()?;
            return Err(io::Error::other(reply));
        }
        decode(&reply)
    }

    fn finish(&mut self) -> io::Result<()> {
        self.remote
            .pending
            .as_mut()
            .ok_or_else(|| io::Error::other("no outcome to adopt"))?
            .adopt()
            .map_err(io::Error::other)?;
        self.cleanup()
    }

    fn cleanup(&mut self) -> io::Result<()> {
        self.persist()?;
        self.remote.cleanup()?;
        self.persist()
    }
    pub fn attach(&mut self, factory: ResourceId) -> io::Result<Window> {
        if self.access.is_some() || self.operation != Operation::Idle {
            return Err(io::Error::other(
                "attachment already owned or pending",
            ));
        }
        self.operation = Operation::Attach(factory);
        let input =
            Message::reference(Contract::new("text.attach", 1), self.buffer);
        let result = self.call(&fields("write", factory, &input), attachment);
        self.retain_attachment(result)?;
        self.finish()?;
        self.operation = Operation::Idle;
        self.observe(0)
    }

    pub fn recover_attachment(&mut self) -> io::Result<Window> {
        if !matches!(self.operation, Operation::Attach(_)) {
            return Err(io::Error::other("no pending attachment"));
        }
        let adopted = self
            .remote
            .pending
            .as_ref()
            .is_some_and(|request| request.release().is_ok());
        if adopted {
            self.cleanup()?;
        } else {
            let result = self.result(attachment);
            self.retain_attachment(result)?;
            self.finish()?;
        }
        self.operation = Operation::Idle;
        if self.access.is_none() {
            return Err(io::Error::other("attachment rejected; F5 retry"));
        }
        self.observe(0)
    }

    fn retain_attachment(
        &mut self,
        result: io::Result<ResourceId>,
    ) -> io::Result<()> {
        if result.is_err()
            && self.remote.pending.is_none()
            && self.remote.reserved.is_none()
        {
            self.operation = Operation::Idle;
        }
        self.access = Some(result?);
        Ok(())
    }

    pub fn refresh(
        &mut self,
        factory: ResourceId,
        start: u64,
    ) -> io::Result<Window> {
        if self.access.is_none() {
            return self.attach(factory);
        }
        self.observe(start)
    }

    pub fn observe(&mut self, start: u64) -> io::Result<Window> {
        if self.remote.pending.is_some() || self.remote.reserved.is_some() {
            return Err(io::Error::other(
                "recover pending work before observing",
            ));
        }
        self.operation = Operation::Retire { origin: start };
        self.release_observation()?;
        self.operation = Operation::Idle;
        let access =
            self.access.ok_or_else(|| io::Error::other("not attached"))?;
        let input = Message::new(Contract::new("text.observe", 1), &[]);
        let buffer = self.buffer;
        self.operation = Operation::Observe { origin: start };
        let observed = self.call(&fields("read", access, &input), |reply| {
            observation(reply, access, buffer)
        })?;
        self.observed = Some(observed);
        self.finish()?;
        self.read_context(observed, start)
    }

    fn read_context(
        &mut self,
        observed: Observation,
        start: u64,
    ) -> io::Result<Window> {
        self.operation = Operation::Context { origin: start, reading: None };
        let window = reovim_client_tui::context::read(
            &observed,
            start,
            |first, count| self.read_window(&observed, first, count),
        )
        .map_err(io::Error::other)?;
        self.operation = Operation::Idle;
        Ok(window)
    }

    pub fn recover(&mut self) -> io::Result<Window> {
        if self.remote.reserved.is_some() {
            self.cleanup()?;
            self.operation = Operation::Idle;
            return Err(io::Error::other(
                "unsent request released; input remains retained",
            ));
        }
        match self.operation {
            Operation::Closing => {
                Err(io::Error::other("shutdown pending; finish closing"))
            }
            Operation::Idle => Err(io::Error::other("no pending operation")),
            Operation::Attach(_) => self.recover_attachment(),
            Operation::Change(change) => self.recover_change(change),
            Operation::Retire { origin } => {
                self.recover_retirement()?;
                self.observe(origin)
            }
            Operation::Observe { origin } => self.resume_observation(origin),
            Operation::Context { origin, reading } => {
                self.resume_context(origin, reading)
            }
        }
    }

    fn resume_observation(&mut self, origin: u64) -> io::Result<Window> {
        if self.observed.is_none() {
            self.recover_observation()?;
        } else {
            self.cleanup()?;
        }
        self.read_context(self.observed.unwrap(), origin)
    }

    fn resume_context(
        &mut self,
        origin: u64,
        reading: Option<(u64, u64)>,
    ) -> io::Result<Window> {
        if let Some((first, count)) = reading {
            self.recover_read(first, count)?;
        }
        let observed = self
            .observed
            .ok_or_else(|| io::Error::other("no retained observation"))?;
        self.read_context(observed, origin)
    }

    fn recover_retirement(&mut self) -> io::Result<()> {
        let pending = self
            .remote
            .pending
            .as_ref()
            .ok_or_else(|| io::Error::other("no pending retirement"))?;
        if pending.release().is_ok() {
            return self.cleanup();
        }
        if self.retiring.is_empty() {
            return Err(io::Error::other("no retained Resource to close"));
        }
        self.result(closed)?;
        // Remove the original owner before release; cleanup can fail again.
        self.retiring.pop();
        self.finish()
    }

    fn recover_change(&mut self, change: Change) -> io::Result<Window> {
        let adopted = self
            .remote
            .pending
            .as_ref()
            .is_some_and(|request| request.release().is_ok());
        if adopted {
            self.cleanup()?;
        } else {
            let id = self.buffer;
            let result = self.result(|reply| change_state(change, reply, id));
            self.retain_change(result)?;
            self.finish()?;
        }
        self.operation = Operation::Idle;
        if self.accepted.is_none() {
            return Err(io::Error::other("change rejected; refresh required"));
        }
        self.observe(change.origin())
    }

    fn retain_change(
        &mut self,
        result: io::Result<buffer::BufferState>,
    ) -> io::Result<()> {
        if result.is_err()
            && self.remote.pending.is_none()
            && self.remote.reserved.is_none()
        {
            self.operation = Operation::Idle;
        }
        self.accepted = Some(result?);
        Ok(())
    }

    fn recover_observation(&mut self) -> io::Result<()> {
        let access =
            self.access.ok_or_else(|| io::Error::other("no interpretation"))?;
        let buffer = self.buffer;
        let value = self.result(|reply| observation(reply, access, buffer))?;
        self.observed = Some(value);
        self.finish()
    }

    fn recover_read(&mut self, first: u64, count: u64) -> io::Result<()> {
        let observed = self
            .observed
            .ok_or_else(|| io::Error::other("no retained observation"))?;
        let adopted = self
            .remote
            .pending
            .as_ref()
            .is_some_and(|value| value.release().is_ok());
        let decode =
            |reply: &str| decode_window(reply, &observed, first, count);
        if adopted {
            self.cleanup()?;
        } else {
            self.result(decode)?;
            self.finish()?;
        }
        Ok(())
    }

    fn read_window(
        &mut self,
        observed: &Observation,
        start: u64,
        count: u64,
    ) -> io::Result<SourceWindow> {
        if self.cancel.load(Ordering::Relaxed) {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "view read cancelled",
            ));
        }
        let input = Message::new(
            Contract::new("text.window", 1),
            &[start.to_le_bytes(), count.to_le_bytes()].concat(),
        );
        let Operation::Context { origin, .. } = self.operation else {
            return Err(io::Error::other("read outside context operation"));
        };
        self.operation =
            Operation::Context { origin, reading: Some((start, count)) };
        let window = self.call(
            // The exact range stays paired with a lost receipt.
            &fields("read", observed.snapshot(), &input),
            |reply| decode_window(reply, observed, start, count),
        )?;
        self.finish()?;
        self.operation = Operation::Context { origin, reading: None };
        Ok(window)
    }

    pub fn replace(
        &mut self,
        start: u64,
        end: u64,
        text: &str,
        window_start: u64,
    ) -> io::Result<Window> {
        if self.remote.pending.is_some() || self.operation != Operation::Idle {
            return Err(io::Error::other("recover pending change first"));
        }
        let observed =
            self.observed.ok_or_else(|| io::Error::other("not observed"))?;
        let mut data = [start.to_le_bytes(), end.to_le_bytes()].concat();
        data.extend_from_slice(text.as_bytes());
        let input = Message {
            contract: Contract::new("text.replace", 1),
            data: data.into(),
            resources: Box::new([observed.snapshot()]),
        };
        let cursor = start
            .checked_add(
                u64::try_from(text.chars().count())
                    .map_err(io::Error::other)?,
            )
            .ok_or_else(|| io::Error::other("cursor overflow"))?;
        let change = Change::Replace {
            origin: window_start.max(cursor.saturating_sub(512)),
        };
        self.accepted = None;
        self.operation = Operation::Change(change);
        let id = self.buffer;
        let result = self.call(
            &fields("write", observed.observed().representation, &input),
            |reply| change_state(change, reply, id),
        );
        self.retain_change(result)?;
        self.finish()?;
        self.operation = Operation::Idle;
        self.observe(change.origin())
    }

    pub fn close(&mut self) -> io::Result<()> {
        if self.operation == Operation::Closing && self.remote.pending.is_some()
        {
            self.recover_retirement()?;
        } else {
            self.cleanup()?;
        }
        self.operation = Operation::Closing;
        if let Some(access) = self.access.take() {
            self.retiring.insert(0, access);
        }
        self.release_observation()?;
        self.operation = Operation::Idle;
        Ok(())
    }

    pub fn history(&mut self, redo: bool) -> io::Result<Window> {
        if self.remote.pending.is_some() || self.operation != Operation::Idle {
            return Err(io::Error::other("recover pending change first"));
        }
        let observed =
            self.observed.ok_or_else(|| io::Error::other("not observed"))?;
        let name =
            if redo { "kernel.history.redo" } else { "kernel.history.undo" };
        let input = Message {
            contract: Contract::new(name, 1),
            data: observed.observed().buffer.revision.to_le_bytes().into(),
            resources: Box::new([self.buffer]),
        };
        let change = Change::History { redo };
        self.accepted = None;
        self.operation = Operation::Change(change);
        let id = self.buffer;
        let result = self
            .call(&fields("write", observed.history(), &input), |reply| {
                change_state(change, reply, id)
            });
        self.retain_change(result)?;
        self.finish()?;
        self.operation = Operation::Idle;
        self.observe(0)
    }

    fn release_observation(&mut self) -> io::Result<()> {
        if let Some(observed) = self.observed.take() {
            self.retiring.extend([observed.watch(), observed.snapshot()]);
        }
        while let Some(id) = self.retiring.last().copied() {
            self.close_resource(id)?;
            self.retiring.pop();
            self.finish()?;
        }
        Ok(())
    }

    fn close_resource(&mut self, id: ResourceId) -> io::Result<()> {
        self.call(&["close".into(), format!("@{}", id.0)], closed)
    }
}

fn attachment(reply: &str) -> io::Result<ResourceId> {
    api::attachment(&message(reply)?, true).map_err(io::Error::other)
}

fn decode_window(
    reply: &str,
    observed: &Observation,
    start: u64,
    count: u64,
) -> io::Result<SourceWindow> {
    api::window(&message(reply)?, observed, start, count)
        .map_err(io::Error::other)
}

fn observation(
    reply: &str,
    access: ResourceId,
    buffer: ResourceId,
) -> io::Result<Observation> {
    let value =
        api::observation(&message(reply)?, access).map_err(io::Error::other)?;
    if value.observed().buffer.buffer != buffer {
        return Err(io::Error::other("foreign Buffer observation"));
    }
    Ok(value)
}
