//! Socket observers do not own or cancel retained request execution.

use super::{clients::Clients, transport::Command};
use alloc::{boxed::Box, format, string::String, vec::Vec};
use core::{future::Future, pin::Pin, task::Context};
use reovim_kernel_runtime::Runtime;
use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, SyncSender, TryRecvError},
    },
};

pub(super) type Response =
    Pin<Box<dyn Future<Output = Result<String, String>>>>;

pub(super) struct Pending {
    response: Response,
    sender: SyncSender<String>,
    alive: Arc<AtomicBool>,
}
impl Pending {
    fn new(
        command: Command,
        runtime: &mut Runtime,
        clients: &mut Clients,
    ) -> Self {
        let response = clients
            .dispatch(runtime, &command.line)
            .unwrap_or_else(|error| Box::pin(core::future::ready(Err(error))));
        Self { response, sender: command.reply, alive: command.alive }
    }
    pub fn poll(pending: &mut Vec<Self>, context: &mut Context<'_>) {
        pending.retain_mut(|request| !request.finish(context));
    }
    fn finish(&mut self, context: &mut Context<'_>) -> bool {
        if !self.alive.load(Ordering::Acquire) {
            return true;
        }
        let core::task::Poll::Ready(result) =
            self.response.as_mut().poll(context)
        else {
            return false;
        };
        let reply = result.unwrap_or_else(|error| format!("error\t{error}"));
        // The one-slot channel cannot block; only one result is sent.
        let _ = self.sender.send(reply);
        true
    }
}

pub(super) fn receive(
    commands: &Receiver<Command>,
    runtime: &mut Runtime,
    clients: &mut Clients,
    pending: &mut Vec<Pending>,
) -> io::Result<()> {
    while pending.len() < super::transport::CAPACITY {
        match commands.try_recv() {
            Ok(command) => {
                pending.push(Pending::new(command, runtime, clients));
            }
            Err(TryRecvError::Empty) => return Ok(()),
            Err(TryRecvError::Disconnected) => {
                return Err(io::Error::other("transport stopped"));
            }
        }
    }
    Ok(())
}
