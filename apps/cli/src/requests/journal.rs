//! One exclusively owned, append-only transcript for one retained Caller.

use reovim_service::protocol::{Action, Frame, Reply};
use std::{
    collections::BTreeSet,
    fs::{File, OpenOptions},
    io::{self, BufRead, BufReader, Seek, SeekFrom, Write},
    path::Path,
};

pub struct Journal {
    file: File,
    poisoned: bool,
    token: [u8; 16],
    sequence: u64,
    pending: Option<Frame>,
    receipts: BTreeSet<u64>,
}
impl Journal {
    pub fn open(
        path: &Path,
        address: &str,
        token: [u8; 16],
    ) -> io::Result<Self> {
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        private(&mut options);
        let file = options.open(path)?;
        file.try_lock().map_err(io::Error::other)?;
        let mut journal = Self {
            file,
            poisoned: false,
            token,
            sequence: 0,
            pending: None,
            receipts: BTreeSet::new(),
        };
        let header =
            format!("caller\t{address}\t{}", reovim_service::hex(&token));
        journal.restore(&header)?;
        sync_parent(path)?;
        Ok(journal)
    }
    fn restore(&mut self, header: &str) -> io::Result<()> {
        if self.file.metadata()?.len() == 0 {
            return self.append(header);
        }
        let reader_file = self.file.try_clone()?;
        let mut reader = BufReader::new(reader_file);
        let mut position = 0_u64;
        let first = line(&mut reader)?
            .ok_or_else(|| io::Error::other("missing journal header"))?;
        if !first.ends_with('\n') || first.trim_end_matches('\n') != header {
            return Err(io::Error::other("journal Caller/address mismatch"));
        }
        position += first.len() as u64;
        while let Some(record) = complete_line(&mut reader)? {
            self.record(record.trim_end_matches('\n'))?;
            position += record.len() as u64;
        }
        self.file.set_len(position)?;
        self.file.seek(SeekFrom::End(0))?;
        self.file.sync_all()
    }
    fn record(&mut self, record: &str) -> io::Result<()> {
        if let Some(frame) = record.strip_prefix('>') {
            let frame = Frame::decode(frame).map_err(io::Error::other)?;
            self.validate(&frame)?;
            self.pending = Some(frame);
            return Ok(());
        }
        let reply = record
            .strip_prefix('<')
            .ok_or_else(|| io::Error::other("invalid journal record"))?;
        let reply = Reply::decode(reply).map_err(io::Error::other)?;
        self.apply(&reply)
    }
    pub fn begin(&mut self, frame: Frame) -> io::Result<()> {
        self.validate(&frame)?;
        self.append(&format!(
            ">{}",
            frame.encode().map_err(io::Error::other)?
        ))?;
        self.pending = Some(frame);
        Ok(())
    }
    fn validate(&self, frame: &Frame) -> io::Result<()> {
        self.healthy()?;
        if self.pending.is_some() {
            return Err(io::Error::other("pending exchange: use retry"));
        }
        if frame.caller != self.token {
            return Err(io::Error::other("foreign journal Caller"));
        }
        if matches!(frame.action, Action::Reserve(n) if n != self.next()?) {
            return Err(io::Error::other("invalid reservation sequence"));
        }
        Ok(())
    }
    pub fn complete(&mut self, reply: &Reply) -> io::Result<()> {
        self.validate_reply(reply)?;
        self.append(&format!(
            "<{}",
            reply.encode().map_err(io::Error::other)?
        ))?;
        self.apply(reply)
    }
    fn validate_reply(&self, reply: &Reply) -> io::Result<()> {
        self.healthy()?;
        let action = &self
            .pending
            .as_ref()
            .ok_or_else(|| io::Error::other("reply without frame"))?
            .action;
        let valid = match (action, reply) {
            (Action::Reserve(_), Reply::Reserved(id)) => {
                *id != 0 && !self.receipts.contains(id)
            }
            (_, Reply::Error(_))
            | (Action::Result(_), Reply::Status(_))
            | (Action::Cancel(_), Reply::Cancelled(_))
            | (
                Action::Call(_)
                | Action::Adopt(_)
                | Action::Release(_)
                | Action::Reconcile(_),
                Reply::Accepted,
            ) => true,
            _ => false,
        };
        if !valid {
            return Err(io::Error::other("reply does not match pending frame"));
        }
        Ok(())
    }
    fn apply(&mut self, reply: &Reply) -> io::Result<()> {
        self.validate_reply(reply)?;
        let pending = self
            .pending
            .take()
            .ok_or_else(|| io::Error::other("reply without frame"))?;
        match (&pending.action, reply) {
            (Action::Reserve(sequence), Reply::Reserved(id)) => {
                self.sequence = *sequence;
                self.receipts.insert(*id);
            }
            (Action::Release(id), Reply::Accepted) => {
                self.receipts.remove(id);
            }
            _ => {}
        }
        Ok(())
    }
    fn append(&mut self, record: &str) -> io::Result<()> {
        self.healthy()?;
        // A partial write or failed flush makes this in-memory view uncertain.
        // Reopening reconstructs the last complete record before any next send.
        self.poisoned = true;
        self.file.write_all(record.as_bytes())?;
        self.file.write_all(b"\n")?;
        self.file.sync_all()?;
        self.poisoned = false;
        Ok(())
    }
    pub fn next(&self) -> io::Result<u64> {
        self.healthy()?;
        if self.receipts.len() >= 64 {
            return Err(io::Error::other(
                "release retained receipts before new work",
            ));
        }
        self.sequence
            .checked_add(1)
            .ok_or_else(|| io::Error::other("reservation sequence exhausted"))
    }
    fn healthy(&self) -> io::Result<()> {
        if self.poisoned {
            return Err(io::Error::other("journal write failed: reopen CLI"));
        }
        Ok(())
    }
    pub fn pending(&self) -> io::Result<Option<&Frame>> {
        self.healthy()?;
        Ok(self.pending.as_ref())
    }
    pub fn receipts(&self) -> impl Iterator<Item = &u64> {
        self.receipts.iter()
    }
}
fn line(reader: &mut impl BufRead) -> io::Result<Option<String>> {
    let mut output = Vec::new();
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            break;
        }
        let count = available
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(available.len(), |index| index + 1);
        if output.len() + count > reovim_service::MAX_LINE + 2 {
            return Err(io::Error::other("oversized journal record"));
        }
        output.extend_from_slice(&available[..count]);
        reader.consume(count);
        if output.last() == Some(&b'\n') {
            break;
        }
    }
    if output.is_empty() {
        return Ok(None);
    }
    String::from_utf8(output).map(Some).map_err(io::Error::other)
}
#[cfg(unix)]
fn private(options: &mut OpenOptions) {
    use std::os::unix::fs::OpenOptionsExt;
    options.mode(0o600);
}
#[cfg(not(unix))]
fn private(_: &mut OpenOptions) {}

fn complete_line(reader: &mut impl BufRead) -> io::Result<Option<String>> {
    Ok(line(reader)?.filter(|record| record.ends_with('\n')))
}
#[cfg(unix)]
fn sync_parent(path: &Path) -> io::Result<()> {
    let parent = path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    File::open(parent)?.sync_all()
}
#[cfg(not(unix))]
fn sync_parent(_: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests;
