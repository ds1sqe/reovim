#![no_std]
#![forbid(unsafe_code)]
//! The fs Driver: one directory mirrored under `/module/fs/root`.
//! Directories are Containers whose children appear on `lookup`; files
//! are Read/Write byte rds. A name that is not on disk is an empty
//! file until it is written. It speaks to the machine through host/os
//! only and learns of edits behind its back by comparing `mtime` on
//! the next `read`, so an edit inside the host's timestamp tick after
//! the Driver's own write goes unseen.

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::RefCell;

use reovim_host::fs as host;
use reovim_kernel_interface::{
    Booted, Calls, Decl, Driver, DriverId, Error, Flags, Hash, Kind, Module,
    Rd, Reply, Rights,
};
use reovim_sdk::declare;

declare! {
    /// Every directory: a new empty file below it.
    api fs ver 1 {
        0 create(name: utf8(255)) -> Rd: WRITE;
    }
}

/// A directory: Container, WRITE for `create`.
const DIR: Flags = Flags::READ.union(Flags::WRITE).union(Flags::CONTAINER);
/// A file: bytes.
const FILE: Flags = Flags::READ.union(Flags::WRITE);

struct Node {
    path: String,
    dir: bool,
    /// `mtime` at the last read or write; a different one is a Modify.
    seen: Option<u64>,
}

/// The Driver and its Module in one; pass it twice at `load`.
pub struct Fs {
    root: String,
    decl: Decl,
    nodes: RefCell<BTreeMap<Rd, Node>>,
}

/// `name` below `dir`, with one slash between even for a root of `/`.
fn join(dir: &str, name: &str) -> String {
    format!("{}/{name}", dir.trim_end_matches('/'))
}

/// One path segment the mirror accepts.
fn valid(name: &str) -> bool {
    !name.is_empty()
        && !name.contains(['/', '\0'])
        && name != "."
        && name != ".."
}

/// Host failures on a path that is not (yet) an rd.
const fn map(error: reovim_host::Error) -> Error {
    match error {
        reovim_host::Error::NotFound | reovim_host::Error::Exists => {
            Error::Invalid
        }
        reovim_host::Error::Denied => Error::Denied,
        reovim_host::Error::Io => Error::Busy,
        reovim_host::Error::Unsupported => Error::Unsupported,
    }
}

/// The fs Module over `root`, ready for `load`.
#[must_use]
pub fn boot(root: &str) -> Booted {
    let fs = Rc::new(Fs::new(String::from(root)));
    Booted { module: fs.clone(), drivers: alloc::vec![fs] }
}

impl Fs {
    /// Mirror the directory at `root`.
    #[must_use]
    pub fn new(root: String) -> Self {
        Self { root, decl: fs::decl(), nodes: RefCell::new(BTreeMap::new()) }
    }

    fn path(&self, rd: Rd) -> Result<(String, bool), Error> {
        let nodes = self.nodes.borrow();
        let node = nodes.get(&rd).ok_or(Error::Invalid)?;
        Ok((node.path.clone(), node.dir))
    }

    fn known(&self, path: &str) -> Option<Rd> {
        let nodes = self.nodes.borrow();
        nodes.iter().find(|(_, n)| n.path == path).map(|(rd, _)| *rd)
    }

    /// Register `path` as `name` under `parent`, unless it already is.
    fn add(
        &self,
        ctx: &dyn Calls,
        parent: Rd,
        name: &str,
        path: String,
        dir: bool,
    ) -> Result<Rd, Error> {
        if let Some(rd) = self.known(&path) {
            return Ok(rd);
        }
        let rd = ctx.register(DriverId(0), parent, Some(name))?;
        let (flags, apis): (Flags, &[Decl]) = if dir {
            (DIR, core::slice::from_ref(&self.decl))
        } else {
            (FILE, &[])
        };
        ctx.report(rd, flags, apis)?;
        self.nodes.borrow_mut().insert(rd, Node { path, dir, seen: None });
        Ok(rd)
    }

    /// The rd's path is gone from disk: forget the rd and its subtree.
    fn lost(&self, ctx: &dyn Calls, rd: Rd, path: &str) -> Error {
        let _ = ctx.unregister(rd);
        let below = |n: &Node| {
            n.path.strip_prefix(path).is_none_or(|rest| !rest.starts_with('/'))
        };
        self.nodes.borrow_mut().retain(|_, n| n.path != path && below(n));
        Error::Gone
    }

    /// One child by name, or every child for an empty name.
    fn child(&self, ctx: &dyn Calls, rd: Rd, name: &str) -> Result<Rd, Error> {
        let (path, dir) = self.path(rd)?;
        if !dir {
            return Err(Error::Unsupported);
        }
        if name.is_empty() {
            return self.list(ctx, rd, &path);
        }
        if !valid(name) {
            return Err(Error::Invalid);
        }
        let path = join(&path, name);
        let dir = match host::stat(&path) {
            Ok(stat) => stat.kind == reovim_host::Kind::Dir,
            // Not on disk: an empty file until it is written.
            Err(reovim_host::Error::NotFound) => false,
            Err(other) => return Err(map(other)),
        };
        self.add(ctx, rd, name, path, dir)
    }

    fn list(&self, ctx: &dyn Calls, rd: Rd, path: &str) -> Result<Rd, Error> {
        let entries = host::list(path).map_err(map)?;
        for entry in entries.iter().filter(|e| valid(&e.name)) {
            let dir = entry.kind == reovim_host::Kind::Dir;
            let child = join(path, &entry.name);
            self.add(ctx, rd, &entry.name, child, dir)?;
        }
        Ok(rd)
    }

    /// `stat` the file behind `rd`; a moved `mtime` is a Modify, a
    /// file that was there and is not is Gone. One that never was is
    /// still empty.
    fn check(&self, ctx: &dyn Calls, rd: Rd) -> Result<String, Error> {
        let (path, dir) = self.path(rd)?;
        if dir {
            return Err(Error::Unsupported);
        }
        let mtime = match host::stat(&path) {
            Ok(stat) => Some(stat.mtime),
            Err(reovim_host::Error::NotFound) => None,
            Err(other) => return Err(map(other)),
        };
        let mut nodes = self.nodes.borrow_mut();
        let node = nodes.get_mut(&rd).ok_or(Error::Invalid)?;
        let seen = core::mem::replace(&mut node.seen, mtime);
        drop(nodes);
        match (seen, mtime) {
            (Some(_), None) => Err(self.lost(ctx, rd, &path)),
            (Some(last), Some(now)) if last != now => {
                ctx.changed(rd, Kind::Modify)?;
                Ok(path)
            }
            _ => Ok(path),
        }
    }

    fn write(
        &self,
        ctx: &dyn Calls,
        rd: Rd,
        at: u64,
        bytes: &[u8],
    ) -> Result<usize, Error> {
        let path = self.check(ctx, rd)?;
        // The whole file is replaced or left alone; a tail is written.
        let count = if at == 0 {
            host::replace(&path, bytes).map(|()| bytes.len())
        } else {
            host::write(&path, at, bytes)
        };
        let count = count.map_err(map)?;
        let mtime = host::stat(&path).ok().map(|s| s.mtime);
        if let Some(node) = self.nodes.borrow_mut().get_mut(&rd) {
            node.seen = mtime;
        }
        ctx.changed(rd, Kind::Modify)?;
        Ok(count)
    }

    fn make(&self, ctx: &dyn Calls, rd: Rd, name: &str) -> Result<Rd, Error> {
        let (path, _) = self.path(rd)?;
        if !valid(name) {
            return Err(Error::Invalid);
        }
        let path = join(&path, name);
        host::create(&path).map_err(map)?;
        self.add(ctx, rd, name, path, false)
    }
}

impl Module for Fs {
    fn start(&self, ctx: &dyn Calls, home: Rd) -> Result<(), Error> {
        self.add(ctx, home, "root", self.root.clone(), true).map(|_| ())
    }
}

impl Driver for Fs {
    fn lookup(&self, ctx: &dyn Calls, rd: Rd, name: &str) -> Reply<Rd> {
        self.child(ctx, rd, name).into()
    }

    fn open(&self, _: &dyn Calls, rd: Rd, _: Rights) -> Reply<u64> {
        Reply::ok(rd.0)
    }

    fn read(
        &self,
        ctx: &dyn Calls,
        h: u64,
        at: u64,
        len: usize,
    ) -> Reply<Vec<u8>> {
        let bytes = self.check(ctx, Rd(h)).and_then(|path| {
            match host::read(&path, at, len) {
                // Not written yet.
                Err(reovim_host::Error::NotFound) => Ok(Vec::new()),
                bytes => bytes.map_err(map),
            }
        });
        bytes.into()
    }

    fn write(
        &self,
        ctx: &dyn Calls,
        h: u64,
        at: u64,
        buf: &[u8],
    ) -> Reply<usize> {
        self.write(ctx, Rd(h), at, buf).into()
    }

    fn call(
        &self,
        ctx: &dyn Calls,
        h: u64,
        _: Hash,
        number: u16,
        input: &[u8],
    ) -> Reply<Vec<u8>> {
        fs::dispatch(self, ctx, Rd(h), number, input).into()
    }
}

impl fs::Api for Fs {
    fn create(
        &self,
        ctx: &dyn Calls,
        rd: Rd,
        name: String,
    ) -> Result<Rd, Error> {
        self.make(ctx, rd, &name)
    }
}
