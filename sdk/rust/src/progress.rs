//! `Progress`: how far a long-running fill has gone.
//!
//! The units are its own — bytes for a cell, files for a directory —
//! and unknown while `total` is 0. It is a plain cell: a declared
//! record served as a READ cell's bytes, holding the two numbers and
//! nothing of how they are shown; a reader words them itself.
//! [`eta`] is the one arithmetic every reader shares, from the rd's
//! own `ctime` and `mtime`, so a Driver serving one must never move
//! `ctime` with an `Attrib` after `open`.

use reovim_kernel_interface::{Calls, Error, Info, Rd};

use crate::declare;
use crate::ops;

declare! {
    // `progress` is both this file's own name and the api's (hashed,
    // so it cannot be renamed); the nesting this makes and the unused
    // imports a zero-method arm leaves are both silenced here.
    #[allow(clippy::module_inception)]
    #[allow(unused_imports, unused_variables)]
    /// `Progress { done, total }`, and no method: nothing calls it,
    /// only reads its bytes.
    api progress ver 1 {}
    record Progress {
        done: u64,
        total: u64,
    }
}

pub use progress::{Progress, decl, hash};

/// `rd`'s bytes as a [`Progress`]: what any watcher reads, a slice
/// ending in one `Modify` on it. Sixteen bytes, and cranks nothing.
///
/// # Errors
/// As `Op::Open` and `Op::Read`; `Invalid` for bytes that are not one
/// `Progress`.
pub fn read(ctx: &dyn Calls, rd: Rd) -> Result<Progress, Error> {
    crate::decode(&ops::bytes(ctx, rd)?)
}

/// The time left, from `info`'s `ctime` and `mtime` and `progress`'s
/// own two numbers.
///
/// `None` while nothing has been measured yet (`done` or `total` 0),
/// else the time already spent scaled by what is left, in the
/// clock's own units. 0 once `done` reaches `total`.
#[must_use]
pub fn eta(info: &Info, progress: &Progress) -> Option<u64> {
    if progress.done == 0 || progress.total == 0 {
        return None;
    }
    let spent = u128::from(info.mtime.saturating_sub(info.ctime));
    let left = u128::from(progress.total.saturating_sub(progress.done));
    let eta = spent * left / u128::from(progress.done);
    Some(u64::try_from(eta).unwrap_or(u64::MAX))
}
