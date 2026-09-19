//! The output cell a run answers with: what is left behind when the
//! tree will not hold one.
//!
//! A run that follows has opened something before it asks for its cell
//! — a handle on the log, a watch on its target — and the cell's
//! [`Follow`] is what closes it. The tree here answers as a Host that
//! refuses would, which is the one thing a Kernel's own never does.

use std::cell::Cell as Flag;
use std::cell::RefCell;
use std::rc::Rc;

use reovim_kernel_interface::{
    Calls, Decl, DriverId, Error, Flags, Handle, Kind, Op, Out, Outcome, Rd,
    Reply, Rights, Token,
};
use reovim_sdk::cell::{Cells, Follow};

/// A tree that hands out rds and remembers which of them still stand:
/// its `report` answers what the case asked for, and nothing else here
/// is asked of it.
struct Tree {
    next: Flag<u64>,
    live: RefCell<Vec<Rd>>,
    reports: Result<(), Error>,
}

impl Tree {
    /// One whose `report` answers `reports`.
    fn answering(reports: Result<(), Error>) -> Self {
        Self { next: Flag::new(0), live: RefCell::default(), reports }
    }

    /// The rds registered on it and not taken back.
    fn live(&self) -> Vec<Rd> {
        self.live.borrow().clone()
    }
}

impl Calls for Tree {
    fn register(
        &self,
        _: DriverId,
        _: Rd,
        _: Option<&str>,
    ) -> Result<Rd, Error> {
        self.next.set(self.next.get() + 1);
        let rd = Rd(self.next.get());
        self.live.borrow_mut().push(rd);
        Ok(rd)
    }

    fn name(&self, _: Rd, _: Option<&str>) -> Result<(), Error> {
        Err(Error::Unsupported)
    }

    fn report(&self, _: Rd, _: Flags, _: &[Decl]) -> Result<(), Error> {
        self.reports
    }

    fn unregister(&self, rd: Rd) -> Result<(), Error> {
        self.live.borrow_mut().retain(|live| *live != rd);
        Ok(())
    }

    fn changed(&self, _: Rd, _: Kind) -> Result<(), Error> {
        Err(Error::Unsupported)
    }

    fn done(&self, _: Token, _: Outcome) -> Result<(), Error> {
        Err(Error::Unsupported)
    }

    fn open(&self, _: Rd, _: Rights) -> Result<Handle, Error> {
        Err(Error::Unsupported)
    }

    fn submit(&self, _: Op) -> Reply<Out> {
        Reply::err(Error::Unsupported)
    }
}

/// A follow that says whether it was ended, which is where a run hands
/// back what it opened.
struct Ending(Rc<Flag<bool>>);

impl Follow for Ending {
    fn more(&mut self, _: &dyn Calls) -> Result<Option<Vec<u8>>, Error> {
        Ok(None)
    }

    fn end(&mut self, _: &dyn Calls) {
        self.0.set(true);
    }
}

#[test]
fn a_cell_the_tree_would_not_hold_leaves_no_rd_and_ends_its_follow() {
    let tree = Tree::answering(Err(Error::Denied));
    let cells = Cells::default();
    let ended = Rc::new(Flag::new(false));
    let follow = Box::new(Ending(ended.clone()));

    let made = cells.follow(&tree, DriverId(0), Rd(9), Vec::new(), follow);
    assert_eq!(made, Err(Error::Denied));
    assert_eq!(tree.live(), Vec::new(), "the rd went back out of the tree");
    assert!(ended.get(), "and what the run opened for it was closed");
}

#[test]
fn a_cell_that_was_made_stands_and_its_follow_ends_at_the_close() {
    let tree = Tree::answering(Ok(()));
    let cells = Cells::default();
    let ended = Rc::new(Flag::new(false));
    let follow = Box::new(Ending(ended.clone()));

    let rd = cells.follow(&tree, DriverId(0), Rd(9), Vec::new(), follow);
    let rd = rd.unwrap();
    assert_eq!(tree.live(), [rd]);
    assert!(!ended.get(), "the follow is the cell's while it stands");

    cells.close(&tree, rd).unwrap();
    assert_eq!(tree.live(), Vec::new());
    assert!(ended.get());
}
