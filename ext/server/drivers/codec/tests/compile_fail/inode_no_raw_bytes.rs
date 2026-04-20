#![no_main]

use std::{collections::HashMap, sync::Arc};

use {reovim_driver_codec::Inode, reovim_subsys_vfs::HeapByteSource};

fn main() {
    let _ = Inode {
        bytes: Arc::new(HeapByteSource::new(Vec::<u8>::new())),
        mounts: HashMap::new(),
        raw_bytes: Vec::<u8>::new(),
    };
}
