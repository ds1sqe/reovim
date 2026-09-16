//! Bounded socket workers never own Kernel execution or delivery receipts.

#[path = "transport/delivery.rs"]
mod delivery;

use alloc::{string::String, sync::Arc, vec::Vec};
use reovim_service::transport::{receive, send};
use std::{
    io::{self, BufReader},
    net::{Shutdown, TcpListener, TcpStream},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender, TryRecvError},
    },
    thread::{self, JoinHandle, Thread},
    time::Duration,
};

pub(super) const CAPACITY: usize = 32;
pub(super) struct Command {
    pub line: String,
    pub reply: SyncSender<String>,
}
pub(super) struct Transport {
    commands: Option<Receiver<Command>>,
    stop: Arc<AtomicBool>,
    listener: Option<JoinHandle<io::Result<()>>>,
}
struct Worker {
    stream: TcpStream,
    thread: JoinHandle<()>,
}
impl Transport {
    pub fn start(listener: TcpListener) -> io::Result<Self> {
        listener.set_nonblocking(true)?;
        let mut workers = Vec::new();
        workers.try_reserve_exact(CAPACITY).map_err(io::Error::other)?;
        let (sender, commands) = mpsc::sync_channel(CAPACITY);
        let owner = thread::current();
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        let listener =
            thread::Builder::new().name("resource-listener".into()).spawn(
                move || listen(listener, &sender, &owner, &stopping, workers),
            )?;
        Ok(Self { commands: Some(commands), stop, listener: Some(listener) })
    }
    pub fn next(&self) -> io::Result<Option<Command>> {
        let commands = self
            .commands
            .as_ref()
            .ok_or_else(|| io::Error::other("transport stopped"))?;
        match commands.try_recv() {
            Ok(command) => Ok(Some(command)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "transport listener stopped",
            )),
        }
    }
    pub fn stop(&mut self) -> io::Result<()> {
        // Drop queued reply senders and the receiver to unblock workers.
        self.commands.take();
        self.stop.store(true, Ordering::Release);
        let Some(listener) = self.listener.take() else { return Ok(()) };
        listener.thread().unpark();
        listener.join().map_err(|_| io::Error::other("listener panicked"))?
    }
}
impl Drop for Transport {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

fn listen(
    listener: TcpListener,
    sender: &SyncSender<Command>,
    owner: &Thread,
    stop: &AtomicBool,
    mut workers: Vec<Worker>,
) -> io::Result<()> {
    let result = accept(&listener, sender, owner, stop, &mut workers);
    drop(listener);
    for worker in &workers {
        let _ = worker.stream.shutdown(Shutdown::Both);
    }
    let joined = join(workers);
    owner.unpark();
    result.and(joined)
}
fn accept(
    listener: &TcpListener,
    sender: &SyncSender<Command>,
    owner: &Thread,
    stop: &AtomicBool,
    workers: &mut Vec<Worker>,
) -> io::Result<()> {
    while !stop.load(Ordering::Acquire) {
        reap(workers)?;
        match listener.accept() {
            Ok((stream, _)) => admit(stream, sender, owner, workers)?,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::park_timeout(Duration::from_millis(10));
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => (),
            Err(error) => return Err(error),
        }
    }
    Ok(())
}
fn admit(
    stream: TcpStream,
    sender: &SyncSender<Command>,
    owner: &Thread,
    workers: &mut Vec<Worker>,
) -> io::Result<()> {
    if workers.len() == CAPACITY {
        return Ok(());
    }
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    let retained = stream.try_clone()?;
    let sender = sender.clone();
    let owner = owner.clone();
    let thread = thread::Builder::new().name("resource-socket".into()).spawn(
        move || {
            let _ = handle(stream, &sender, &owner);
        },
    )?;
    workers.push(Worker { stream: retained, thread });
    Ok(())
}
fn reap(workers: &mut Vec<Worker>) -> io::Result<()> {
    let mut index = 0;
    while index < workers.len() {
        if workers[index].thread.is_finished() {
            workers
                .swap_remove(index)
                .thread
                .join()
                .map_err(|_| io::Error::other("socket worker panicked"))?;
        } else {
            index += 1;
        }
    }
    Ok(())
}
fn join(workers: Vec<Worker>) -> io::Result<()> {
    let mut result = Ok(());
    for worker in workers {
        if worker.thread.join().is_err() {
            result = Err(io::Error::other("socket worker panicked"));
        }
    }
    result
}
fn handle(
    mut stream: TcpStream,
    sender: &SyncSender<Command>,
    owner: &Thread,
) -> io::Result<()> {
    let line = receive(&mut BufReader::new(&mut stream))?;
    let (reply, result) = mpsc::sync_channel(1);
    sender.send(Command { line, reply }).map_err(io::Error::other)?;
    owner.unpark();
    let reply = delivery::response(&result, Duration::from_secs(5))?;
    send(&mut stream, &reply)
}
