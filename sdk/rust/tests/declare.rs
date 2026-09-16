//! `declare!` against the text Module's published declarations: the
//! same bytes, a Driver answering through `dispatch`, a caller through
//! the wrapper.

extern crate alloc;

use reovim_kernel_interface::{Calls, Error, Handle, Hash, Rd};
use reovim_sdk::declare;

declare! {
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
    api text ver 1 {
        0 replace(edit: Edit) -> (): WRITE;
        1 replace_many(edits: Many) -> (): WRITE;
    }
    record Edit { at: u64, len: u64, text: utf8(1 << 20) }
    record Many { expected: u64, edits: [Edit; 256] }
}

#[test]
fn the_declared_bytes_are_the_hand_built_ones() {
    assert_eq!(history::decl(), reovim_text_api::history::decl());
    assert_eq!(history::hash(), reovim_text_api::history::hash());
    assert_eq!(text::decl(), reovim_text_api::text::decl());
}

/// A Driver with one revision that refuses to move.
struct One;

impl history::Api for One {
    fn tree(&self, _: &dyn Calls, _: Rd) -> Result<history::Tree, Error> {
        let node = history::Node { id: 1, parent: 0, label: "open".into() };
        Ok(history::Tree { nodes: vec![node], head: 1, published: 1 })
    }

    fn goto(&self, _: &dyn Calls, _: Rd, rev: u64) -> Result<(), Error> {
        if rev == 1 { Ok(()) } else { Err(Error::Invalid) }
    }

    fn undo(&self, _: &dyn Calls, _: Rd) -> Result<(), Error> {
        Err(Error::Invalid)
    }

    fn redo(&self, _: &dyn Calls, _: Rd) -> Result<(), Error> {
        Err(Error::Invalid)
    }
}

/// The `Calls` no method here uses.
struct NoCalls;

impl Calls for NoCalls {
    fn register(
        &self,
        _: reovim_kernel_interface::DriverId,
        _: Rd,
        _: Option<&str>,
    ) -> Result<Rd, Error> {
        Err(Error::Unsupported)
    }

    fn name(&self, _: Rd, _: Option<&str>) -> Result<(), Error> {
        Err(Error::Unsupported)
    }

    fn report(
        &self,
        _: Rd,
        _: reovim_kernel_interface::Flags,
        _: &[reovim_kernel_interface::Decl],
    ) -> Result<(), Error> {
        Err(Error::Unsupported)
    }

    fn unregister(&self, _: Rd) -> Result<(), Error> {
        Err(Error::Unsupported)
    }

    fn changed(
        &self,
        _: Rd,
        _: reovim_kernel_interface::Kind,
    ) -> Result<(), Error> {
        Err(Error::Unsupported)
    }

    fn done(
        &self,
        _: reovim_kernel_interface::Token,
        _: reovim_kernel_interface::Outcome,
    ) -> Result<(), Error> {
        Err(Error::Unsupported)
    }

    fn open(
        &self,
        _: Rd,
        _: reovim_kernel_interface::Rights,
    ) -> Result<Handle, Error> {
        Err(Error::Unsupported)
    }

    fn submit(
        &self,
        _: reovim_kernel_interface::Op,
    ) -> reovim_kernel_interface::Reply<reovim_kernel_interface::Out> {
        reovim_kernel_interface::Reply::err(Error::Unsupported)
    }
}

/// A caller whose `call` runs `dispatch` on `One` and remembers what
/// was sent.
struct Direct {
    sent: Vec<(Hash, u16, Vec<u8>)>,
}

impl reovim_sdk::Call for Direct {
    type Error = Error;

    fn call(
        &mut self,
        _: Handle,
        api: Hash,
        number: u16,
        input: Vec<u8>,
    ) -> Result<Vec<u8>, Error> {
        self.sent.push((api, number, input.clone()));
        if api != history::hash() {
            return Err(Error::Unsupported);
        }
        history::dispatch(&One, &NoCalls, Rd(9), number, &input)
    }
}

#[test]
fn dispatch_and_the_wrapper_round_trip() {
    let mut direct = Direct { sent: Vec::new() };
    let mut history = history::Call(&mut direct, Handle(1));
    let tree = history.tree().unwrap();
    assert_eq!(tree.head, 1);
    assert_eq!(tree.nodes[0].label, "open");
    assert_eq!(history.goto(1), Ok(()));
    assert_eq!(history.goto(2), Err(Error::Invalid));
    assert_eq!(history.undo(), Err(Error::Invalid));
    let mut text = text::Call(&mut direct, Handle(1));
    let edit = text::Edit { at: 0, len: 0, text: "x".into() };
    assert_eq!(text.replace(edit), Err(Error::Unsupported), "another api");
    assert_eq!(direct.sent.len(), 5);
    assert_eq!(direct.sent[1].2, 1u64.to_le_bytes(), "goto's input");
    assert_eq!(direct.sent[4].0, text::hash());
}

#[test]
fn dispatch_refuses_bad_input_and_unknown_numbers() {
    let bad = history::dispatch(&One, &NoCalls, Rd(9), 1, &[1, 2, 3]);
    assert_eq!(bad, Err(Error::Invalid));
    let none = history::dispatch(&One, &NoCalls, Rd(9), 7, &[]);
    assert_eq!(none, Err(Error::Unsupported));
    let tree = history::dispatch(&One, &NoCalls, Rd(9), 0, &[]).unwrap();
    let decoded: history::Tree = reovim_sdk::decode(&tree).unwrap();
    assert_eq!(decoded.nodes.len(), 1);
}
