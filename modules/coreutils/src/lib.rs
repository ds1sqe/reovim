#![no_std]
#![forbid(unsafe_code)]
//! coreutils: the Module that provides the Drivers a typed line runs,
//! under the `bin` of whichever host loaded it.
//!
//! Five declared apis — `stat`, `ls`, `tree`, `cat` and `grep` — as
//! the sdk's `drivers!`, which is the whole of the shape: one Driver,
//! one name per api under the nearest `bin`, and a run answering with
//! the rd of one output cell that its reader drains and closes.
//!
//! Each api is a file of `bins`, holding its declaration and its `run`.
//! The text more than one of them prints is `format`; what they ask the
//! Kernel is the sdk's `ops`.

extern crate alloc;

pub mod bins;
mod format;

use bins::{cat, grep, ls, stat, tree};

reovim_sdk::drivers!(Coreutils: stat, ls, tree, cat, grep);
