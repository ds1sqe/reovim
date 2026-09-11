//! Bounded socket workers never own Kernel execution or delivery receipts.

use alloc::string::String;
use reovim_service::transport::{receive, send};
use std::{
    io::{self, BufReader},
    net::{TcpListener, TcpStream},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError, SyncSender},
    },
    thread::{self, Thread},
    time::Duration,
};

pub(super) const CAPACITY: usize = 32;
pub(super) struct Command {
    pub line: String,
    pub reply: SyncSender<String>,
    pub alive: Arc<AtomicBool>,
}
struct Worker {
    count: Arc<AtomicUsize>,
    alive: Arc<AtomicBool>,
    owner: Thread,
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.alive.store(false, Ordering::Release);
        self.count.fetch_sub(1, Ordering::Relaxed);
        self.owner.unpark();
    }
}

pub(super) fn start(listener: TcpListener) -> io::Result<Receiver<Command>> {
    let (sender, receiver) = mpsc::sync_channel(CAPACITY);
    let owner = thread::current();
    thread::Builder::new()
        .name("resource-listener".into())
        .spawn(move || listen(&listener, &sender, &owner))?;
    Ok(receiver)
}
fn listen(
    listener: &TcpListener,
    sender: &SyncSender<Command>,
    owner: &Thread,
) {
    let count = Arc::new(AtomicUsize::new(0));
    for stream in listener.incoming() {
        let result = stream
            .and_then(|stream| start_worker(stream, sender, owner, &count));
        if let Err(error) = result {
            std::eprintln!("connection: {error}");
        }
    }
}
fn start_worker(
    mut stream: TcpStream,
    sender: &SyncSender<Command>,
    owner: &Thread,
    count: &Arc<AtomicUsize>,
) -> io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    if count.fetch_add(1, Ordering::Relaxed) >= CAPACITY {
        count.fetch_sub(1, Ordering::Relaxed);
        return send(&mut stream, "error\ttransport capacity");
    }
    let worker = Worker {
        count: count.clone(),
        alive: Arc::new(AtomicBool::new(true)),
        owner: owner.clone(),
    };
    let sender = sender.clone();
    thread::Builder::new().name("resource-socket".into()).spawn(move || {
        let _ = handle(stream, &sender, &worker);
    })?;
    Ok(())
}
fn handle(
    mut stream: TcpStream,
    sender: &SyncSender<Command>,
    worker: &Worker,
) -> io::Result<()> {
    let line = receive(&mut BufReader::new(&mut stream))?;
    let (reply, result) = mpsc::sync_channel(1);
    sender
        .send(Command { line, reply, alive: worker.alive.clone() })
        .map_err(io::Error::other)?;
    worker.owner.unpark();
    let reply = wait(&mut stream, &result)?;
    send(&mut stream, &reply)
}
fn wait(
    stream: &mut TcpStream,
    result: &Receiver<String>,
) -> io::Result<String> {
    loop {
        match result.recv_timeout(Duration::from_millis(500)) {
            Ok(reply) => return Ok(reply),
            Err(RecvTimeoutError::Disconnected) => {
                return Err(io::Error::other("request observer stopped"));
            }
            Err(RecvTimeoutError::Timeout) => send(stream, "pending")?,
        }
    }
}
