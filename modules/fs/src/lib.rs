#![no_std]
#![forbid(unsafe_code)]
//! The fs Driver: one directory mirrored under `/module/fs/root`.
//! Directories are Containers whose children appear on `lookup`; files
//! are Read/Write byte rds. It speaks to the machine through host/os
//! only and learns of edits behind its back by comparing `mtime` on
//! the next `read`, so an edit inside the host's timestamp tick after
//! the Driver's own write goes unseen.

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::{format, vec};
use core::cell::RefCell;

use reovim_host::fs;
use reovim_kernel_interface::{
    Calls, Decl, Driver, DriverId, Error, Flags, Hash, Kind, Method, Module,
    Rd, Reply, Rights, Ty,
};

/// The api every directory offers: `0 create(name: Utf8(255)) -> Rd`,
/// WRITE.
#[must_use]
pub fn decl() -> Decl {
    let create = Method {
        number: 0,
        name: "create".to_string(),
        input: Ty::Utf8(255),
        output: Ty::Rd,
        right: Rights::WRITE,
    };
    Decl { api: "fs".to_string(), ver: 1, methods: vec![create] }
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

/// The `Utf8` payload of `create`, already checked to fit.
fn utf8(input: &[u8]) -> Result<&str, Error> {
    let bytes = input.get(4..).ok_or(Error::Invalid)?;
    core::str::from_utf8(bytes).map_err(|_| Error::Invalid)
}

impl Fs {
    /// Mirror the directory at `root`.
    #[must_use]
    pub fn new(root: String) -> Self {
        Self { root, decl: decl(), nodes: RefCell::new(BTreeMap::new()) }
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
        let path = format!("{path}/{name}");
        let stat = fs::stat(&path).map_err(map)?;
        self.add(ctx, rd, name, path, stat.kind == reovim_host::Kind::Dir)
    }

    fn list(&self, ctx: &dyn Calls, rd: Rd, path: &str) -> Result<Rd, Error> {
        let entries = fs::list(path).map_err(map)?;
        for entry in entries.iter().filter(|e| valid(&e.name)) {
            let dir = entry.kind == reovim_host::Kind::Dir;
            let child = format!("{path}/{}", entry.name);
            self.add(ctx, rd, &entry.name, child, dir)?;
        }
        Ok(rd)
    }

    /// `stat` the file behind `rd`; a moved `mtime` is a Modify.
    fn check(&self, ctx: &dyn Calls, rd: Rd) -> Result<String, Error> {
        let (path, dir) = self.path(rd)?;
        if dir {
            return Err(Error::Unsupported);
        }
        let stat = match fs::stat(&path) {
            Ok(stat) => stat,
            Err(reovim_host::Error::NotFound) => {
                return Err(self.lost(ctx, rd, &path));
            }
            Err(other) => return Err(map(other)),
        };
        let mut nodes = self.nodes.borrow_mut();
        let seen = nodes.get_mut(&rd).and_then(|n| n.seen.replace(stat.mtime));
        drop(nodes);
        if seen.is_some_and(|last| last != stat.mtime) {
            ctx.changed(rd, Kind::Modify)?;
        }
        Ok(path)
    }

    fn write(
        &self,
        ctx: &dyn Calls,
        rd: Rd,
        at: u64,
        bytes: &[u8],
    ) -> Result<usize, Error> {
        let path = self.check(ctx, rd)?;
        let count = fs::write(&path, at, bytes).map_err(map)?;
        let mtime = fs::stat(&path).ok().map(|s| s.mtime);
        if let Some(node) = self.nodes.borrow_mut().get_mut(&rd) {
            node.seen = mtime;
        }
        ctx.changed(rd, Kind::Modify)?;
        Ok(count)
    }

    fn create(&self, ctx: &dyn Calls, rd: Rd, name: &str) -> Result<Rd, Error> {
        let (path, _) = self.path(rd)?;
        if !valid(name) {
            return Err(Error::Invalid);
        }
        let path = format!("{path}/{name}");
        fs::create(&path).map_err(map)?;
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
        let bytes = self
            .check(ctx, Rd(h))
            .and_then(|path| fs::read(&path, at, len).map_err(map));
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
        _: u16,
        input: &[u8],
    ) -> Reply<Vec<u8>> {
        let made = utf8(input).and_then(|name| self.create(ctx, Rd(h), name));
        made.map(|rd| rd.0.to_le_bytes().to_vec()).into()
    }
}
