#![no_std]
#![forbid(unsafe_code)]
//! The Client: one per link, using the tree with the same ops a Module
//! uses (Documentation/Design/02-client.md).
//!
//! Bytes go out and come back through a `Carrier` the app provides; the
//! `Tree` owns the `Endpoint`, asks a Pending reply until it is ready,
//! and offers what every user of the tree needs above the ops: a path,
//! a whole read, a method by name. The `Client` is that tree and the
//! watch cursor it drains for the `Plugin`s it holds, each of which
//! makes its own calls and reaches the Client itself by capability.

extern crate alloc;

mod caps;
mod client;
mod complete;
mod fault;
pub mod input;
pub mod log;
mod plugin;
pub mod value;

pub use caps::{BAR, BLOCK, Caps, MATCH, SELECTION, grid, keys, open, process};
pub use client::{Carrier, Client, ROOT, Tree, under};
pub use complete::{paths, walk};
pub use fault::Fault;
pub use input::Key;
pub use plugin::{Ctx, Plugin, Start};
// The contract types a consumer of the Client names, so it depends on
// this crate only.
pub use reovim_kernel_interface::{
    Decl, Error, Event, Field, Flags, Handle, Hash, Info, Kind, Method, Rd,
    Rights, Ty,
};

/// The cells `c` takes on the screen.
///
/// Two for the East Asian wide and fullwidth chars a terminal draws
/// double (Hangul, the CJK ideographs, kana, the fullwidth forms,
/// emoji), one for the rest. A tab is the caller's, as its width
/// depends on where it stands.
#[must_use]
pub const fn width(c: char) -> usize {
    let wide = matches!(
        c as u32,
        0x1100..=0x115F
            | 0x2E80..=0x303E
            | 0x3041..=0x33FF
            | 0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xA000..=0xA4CF
            | 0xAC00..=0xD7A3
            | 0xF900..=0xFAFF
            | 0xFE30..=0xFE4F
            | 0xFF00..=0xFF60
            | 0xFFE0..=0xFFE6
            | 0x1F300..=0x1F64F
            | 0x1F900..=0x1F9FF
            | 0x20000..=0x3FFFD
    );
    if wide { 2 } else { 1 }
}
