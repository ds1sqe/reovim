#![no_std]
#![forbid(unsafe_code)]
//! The api the search Module publishes, for it and for whoever calls
//! it: the bytes both sides hash are these declarations', and the
//! byte shape of one `matches` entry, which is fixed and carries no
//! framing of its own.

extern crate alloc;

use alloc::vec::Vec;

use reovim_sdk::declare;

declare! {
    /// `/self/search/open` and `/module/search/open`: a view over
    /// `source` from `from`, filled by `pattern`'s matches. `open`
    /// answers a `Rd`; `/self/search/<n>` and `/module/search/<n>` are
    /// its shape, `n` a count from 0 that never goes back.
    api search ver 1 {
        0 open(args: Open) -> Rd: WRITE;
    }
    record Open {
        source: Rd,
        pattern: utf8(1024),
        ignorecase: bool,
        smartcase: bool,
        from: u64,
    }
    record Match {
        over: Rd,
        at: u64,
        len: u64,
    }
}

/// The bytes of one [`search::Match`]: `over`, `at` and `len`, each a
/// `u64`, one after the other with no framing between them.
pub const ENTRY: usize = 24;

/// The whole entries `bytes` holds, dropping a trailing partial one: a
/// read of `matches` always ends on an entry's own bound, but a
/// caller stitching together reads of its own might not.
#[must_use]
pub fn entries(bytes: &[u8]) -> Vec<search::Match> {
    let (whole, _) = bytes.as_chunks::<ENTRY>();
    whole.iter().filter_map(|one| reovim_sdk::decode(one).ok()).collect()
}
