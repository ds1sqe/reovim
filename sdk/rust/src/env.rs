//! Named text values, read from the nearest environment first.
//!
//! ```ignore
//! let value = reovim_sdk::env::get(ctx, home, "PATH")?;
//! ```

use alloc::format;
use alloc::string::String;

use reovim_kernel_interface::{Calls, Error, Rd};

use crate::walk::ROOT;
use crate::{cell, declare, decode, encode, ops};

declare! {
    #[allow(clippy::module_inception, unused_imports)]
    /// Named text values; an empty value is still set.
    api env ver 1 {
        0 get(key: utf8(256)) -> Entry: READ;
        1 set(entry: Set) -> (): WRITE;
    }
    record Entry { set: bool, value: utf8(4096) }
    record Set { key: utf8(256), value: utf8(4096) }
}

pub use env::{Entry, Set};

/// The environment a write belongs to.
///
/// ```
/// use reovim_sdk::env::Level;
/// let level = Level::Client;
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Level {
    /// The running Driver's home, for example `/self/cli`.
    Surface,
    /// The Client's `/self` directory.
    Client,
    /// The ancestor directly under `/session`, when present.
    Session,
}

/// Read `key` at `from`, then each parent through the root.
///
/// Missing `env` children and children without this api are skipped.
/// An explicitly set empty string shadows every outer value.
///
/// ```ignore
/// assert_eq!(reovim_sdk::env::get(ctx, home, "missing")?, None);
/// ```
///
/// # Errors
/// Tree and provider failures are returned, including denied access.
pub fn get(
    ctx: &dyn Calls,
    from: Rd,
    key: &str,
) -> Result<Option<String>, Error> {
    let mut at = from;
    loop {
        if let Some(value) = value(ctx, at, key)? {
            return Ok(Some(value));
        }
        if at == ROOT {
            return Ok(None);
        }
        at = ops::parent(ctx, at)?;
    }
}

/// Write at `level`, then record `env: <level> <key>=<value>` in the log.
///
/// ```ignore
/// reovim_sdk::env::set(ctx, home, Level::Client, "PATH", "/bin")?;
/// ```
///
/// # Errors
/// `Unsupported` when the level has no environment. Other tree, call
/// and log failures propagate; a log failure leaves the value set.
pub fn set(
    ctx: &dyn Calls,
    from: Rd,
    level: Level,
    key: &str,
    value: &str,
) -> Result<(), Error> {
    let (at, name) = match level {
        Level::Surface => (from, "surface"),
        Level::Client => (directory(ctx, "/self")?, "client"),
        Level::Session => (session(ctx, from)?, "session"),
    };
    let rd = offered(ctx, at)?.ok_or(Error::Unsupported)?;
    let entry = Set { key: key.into(), value: value.into() };
    ops::call(ctx, rd, env::hash(), &env::decl().methods[1], encode(&entry))?;
    let log = cell::resolve(ctx, ROOT, "/sys/log")?;
    ops::put(ctx, log, format!("env: {name} {key}={value}").into_bytes())?;
    Ok(())
}

/// A missing child is ordinary fallback; other failures remain visible.
fn offered(ctx: &dyn Calls, at: Rd) -> Result<Option<Rd>, Error> {
    let rd = match ops::lookup(ctx, at, "env") {
        Ok(rd) => rd,
        Err(Error::Invalid | Error::Unsupported) => return Ok(None),
        Err(error) => return Err(error),
    };
    Ok(ops::info(ctx, rd)?.apis.contains(&env::hash()).then_some(rd))
}

/// The ancestor directly under `/session`; no provider is made here.
fn session(ctx: &dyn Calls, from: Rd) -> Result<Rd, Error> {
    let sessions = directory(ctx, "/session")?;
    let mut at = from;
    while at != ROOT {
        let parent = ops::parent(ctx, at)?;
        if parent == sessions {
            return Ok(at);
        }
        at = parent;
    }
    Err(Error::Unsupported)
}

/// Read one level without treating an empty string as an unset entry.
fn value(ctx: &dyn Calls, at: Rd, key: &str) -> Result<Option<String>, Error> {
    let Some(rd) = offered(ctx, at)? else { return Ok(None) };
    let method = &env::decl().methods[0];
    let out =
        ops::call(ctx, rd, env::hash(), method, encode(&String::from(key)))?;
    let entry: Entry = decode(&out)?;
    Ok(entry.set.then_some(entry.value))
}

/// An absent level is unsupported, just like an absent env child.
fn directory(ctx: &dyn Calls, path: &str) -> Result<Rd, Error> {
    match cell::resolve(ctx, ROOT, path) {
        Err(Error::Invalid | Error::Unsupported) => Err(Error::Unsupported),
        found => found,
    }
}
