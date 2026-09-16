//! Canonical exchanges retain the exact pending frame across observation loss.

mod journal;
use journal::Journal;
use reovim_kernel_interface::{
    Outcome, Value,
    caller::{Call, Operation, Status},
    resource::AccessId,
};
use reovim_service::protocol::{Action, Frame, Reply};
use std::{
    io::{self, BufReader},
    net::{SocketAddr, TcpStream},
    path::Path,
    time::{Duration, Instant},
};

pub struct Client {
    address: SocketAddr,
    token: [u8; 16],
    journal: Journal,
}
impl Client {
    pub fn open(
        address: &str,
        token: [u8; 16],
        path: &Path,
    ) -> io::Result<Self> {
        let address: SocketAddr = address.parse().map_err(io::Error::other)?;
        let journal = Journal::open(path, &address.to_string(), token)?;
        Ok(Self { address, token, journal })
    }
    pub fn exchange(&mut self, action: Action) -> io::Result<Reply> {
        self.journal.begin(Frame { caller: self.token, action })?;
        self.retry()
    }
    pub fn retry(&mut self) -> io::Result<Reply> {
        let pending = self
            .journal
            .pending()?
            .ok_or_else(|| io::Error::other("no pending frame"))?;
        let frame = pending.encode().map_err(io::Error::other)?;
        let mut stream =
            TcpStream::connect_timeout(&self.address, Duration::from_secs(5))?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        stream.set_write_timeout(Some(Duration::from_secs(5)))?;
        reovim_service::transport::send(&mut stream, &frame)?;
        let line =
            reovim_service::transport::receive(&mut BufReader::new(stream))?;
        let reply = Reply::decode(&line).map_err(io::Error::other)?;
        self.journal.complete(&reply)?;
        Ok(reply)
    }
    pub fn receipts(&self) -> impl Iterator<Item = &u64> {
        self.journal.receipts()
    }
    pub fn call(
        &mut self,
        operation: Operation,
        subject: AccessId,
        command: u32,
        input: Value,
    ) -> io::Result<Value> {
        let sequence = self.journal.next()?;
        let Reply::Reserved(request) =
            self.exchange(Action::Reserve(sequence))?
        else {
            return Err(io::Error::other(
                "reservation rejected; inspect journal",
            ));
        };
        accepted(&self.exchange(Action::Call(Call {
            request,
            operation,
            subject,
            command,
            expected: None,
            input,
        }))?)
        .map_err(|error| {
            io::Error::other(format!("request {request}: {error}"))
        })?;
        let value = outcome(self, request)?;
        accepted(&self.exchange(Action::Adopt(request))?)?;
        accepted(&self.exchange(Action::Release(request))?)?;
        Ok(value)
    }
}
fn outcome(client: &mut Client, request: u64) -> io::Result<Value> {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let reply = client.exchange(Action::Result(request))?;
        if let Some(value) = completed(reply, request)? {
            return Ok(value);
        }
        if Instant::now() >= deadline {
            return Err(io::Error::other(format!(
                "request {request} remains pending"
            )));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}
fn accepted(reply: &Reply) -> io::Result<()> {
    if *reply == Reply::Accepted {
        Ok(())
    } else {
        Err(io::Error::other(format!("{reply:?}")))
    }
}
fn completed(reply: Reply, request: u64) -> io::Result<Option<Value>> {
    match reply {
        Reply::Status(Status::Pending) => Ok(None),
        Reply::Status(Status::Outcome(outcome)) => match outcome.as_ref() {
            Outcome::Ok(value) => Ok(Some(value.clone())),
            other => {
                Err(io::Error::other(format!("request {request}: {other:?}")))
            }
        },
        other => Err(io::Error::other(format!("request {request}: {other:?}"))),
    }
}

#[cfg(test)]
mod tests;
