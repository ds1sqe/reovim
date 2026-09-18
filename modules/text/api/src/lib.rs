#![no_std]
#![forbid(unsafe_code)]
//! The apis the text Module publishes, for it and for whoever calls
//! them: the bytes both sides hash are these declarations'.

extern crate alloc;

use reovim_sdk::declare;

declare! {
    /// `/module/text/open`: a Buffer over any rd, or over nothing:
    /// `new` makes one that is empty and cannot save.
    api open ver 1 {
        0 open(source: Rd) -> Rd: WRITE;
        1 new() -> Rd: WRITE;
    }
}

declare! {
    /// Every Buffer: the edits of one call are one state; `expected`
    /// is the head revision the caller saw, 0 for any, and a different
    /// one is `Stale`. What makes one insert one undo step is the
    /// draft a Client holds, which sends a session as one call.
    api text ver 1 {
        0 replace(edit: Edit) -> (): WRITE;
        1 replace_many(many: Many) -> (): WRITE;
    }
    record Edit { at: u64, len: u64, text: utf8(1 << 20) }
    record Many { expected: u64, edits: [Edit; 256] }
}

declare! {
    /// Every Buffer: the `history` api of 00-core.md. Revisions count
    /// from 1; 0 is "none" as a parent or as `published`.
    api history ver 1 {
        0 tree() -> Tree: READ;
        1 goto(rev: u64) -> (): WRITE;
        2 undo() -> (): WRITE;
        3 redo() -> (): WRITE;
    }
    record Tree { nodes: [Node; 4096], head: u64, published: u64 }
    record Node { id: u64, parent: u64, label: utf8(16) }
}

declare! {
    /// Every Buffer: the head written back to the source. `save_as`
    /// writes it to another rd instead; a Buffer over nothing takes
    /// that rd as its source and goes on as its Buffer, which is what
    /// `:w NAME` on a scratch Buffer asks for.
    api publish ver 1 {
        0 save() -> (): WRITE;
        1 save_as(target: Rd) -> (): WRITE;
    }
}
