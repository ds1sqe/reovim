//! `/self/bin`: the two names the Host serves itself, and the Drivers
//! this Client loaded beside them.
//!
//! A Module's `start` finds the nearest `bin` above its home, so the
//! same coreutils registers under `/bin` at a Kernel and under
//! `/self/bin` at a Client, nothing in it saying which it is under. A
//! Driver of the Client's own reads both sides, the `Calls` it was
//! handed being the union; the server's cannot reach the Client's.

use std::cell::Cell;
use std::rc::Rc;

use reovim_client::{Calls, Error, Host, ROOT, Rd, Rvm, SELF, bin};
use reovim_host::clock;
use reovim_module_coreutils::bins::ls;
use reovim_rig::Rig;
use reovim_sdk::{Encode, cell, encode, ops};

/// A Host of its own over `rig`, `/self/bin` served and `rvm` behind
/// `-l`. The rig's own Host is one that was given no files.
fn over(rig: &Rig, rvm: Rvm) -> Rc<Host> {
    let host = Rc::new(Host::over(rig.client(), clock::now));
    bin::boot(&host, rvm).unwrap();
    host
}

/// The names under `rd`, in the order the tree lists them.
fn names(ctx: &dyn Calls, rd: Rd) -> Vec<String> {
    let under = |rd| ops::info(ctx, rd).ok()?.name;
    ops::children(ctx, rd).unwrap().into_iter().filter_map(under).collect()
}

/// One run of the name `rd` with `args`, its output cell read to the
/// empty read that ends it and closed, as a shell reads one.
fn run<A: Encode>(ctx: &dyn Calls, rd: Rd, args: &A) -> Result<String, Error> {
    let out = ops::call_by_name(ctx, rd, "run", encode(args))?;
    let cell: Rd = reovim_sdk::decode(&out)?;
    let held = ops::bytes(ctx, cell)?;
    Ok(String::from_utf8(held).unwrap())
}

/// A listing of `cwd` itself, as a shell fills the record: no flag, no
/// width, no color, no target.
const fn listing(cwd: Rd) -> ls::Args {
    ls::Args {
        long: false,
        all: false,
        w: false,
        cwd,
        width: 0,
        color: false,
        targets: Vec::new(),
    }
}

/// `load [-l] PATH`, `local` being the field `-l` sets.
fn loading(local: bool, path: &str) -> bin::load::Args {
    bin::load::Args { local, path: path.to_string() }
}

/// `unload [-l] NAME`.
fn unloading(local: bool, name: &str) -> bin::unload::Args {
    bin::unload::Args { local, name: name.to_string() }
}

/// The rd of `path`, which must be there.
fn at(ctx: &dyn Calls, path: &str) -> Rd {
    cell::resolve(ctx, ROOT, path).unwrap()
}

#[test]
fn a_module_registers_under_the_bin_above_its_home() {
    let rig = Rig::new("bin-home");
    let host = over(&rig, Rvm::none());
    let own = host.own();
    let ctx: &dyn Calls = &own;

    // The row is the Host's own and the two names stand in it before
    // anything is loaded, as `/bin` holds nothing on a Kernel with no
    // coreutils but is there all the same.
    assert_eq!(names(ctx, SELF), ["bin"]);
    let bin = at(ctx, "/self/bin");
    assert_eq!(names(ctx, bin), ["load", "unload"]);

    // The same Module the server links, loaded here: `start` walks up
    // from `/self/coreutils`, finds `/self/bin` in two ops, and
    // registers its five there in declared order.
    let coreutils = reovim_module_coreutils::boot("");
    host.load("coreutils", coreutils.module, coreutils.drivers).unwrap();
    let held = ["load", "unload", "stat", "ls", "tree", "cat", "grep"];
    assert_eq!(names(ctx, bin), held);
    assert_eq!(ops::where_is(ctx, at(ctx, "/self/bin/ls")).unwrap(), {
        "/self/bin/ls"
    });
    // The home is empty, as `/module/coreutils` is at the server: it
    // is how a person sees what this Client loaded.
    assert_eq!(names(ctx, at(ctx, "/self/coreutils")), Vec::<String>::new());

    // The server's own `/bin` gained nothing and lost nothing.
    assert_eq!(names(ctx, at(ctx, "/bin")).len(), 11);

    // A Client that was given no files loads nothing of its own.
    let load = at(ctx, "/self/bin/load");
    assert_eq!(
        run(ctx, load, &loading(true, "x.rvm")),
        Err(Error::Unsupported)
    );

    // `unload` takes every rd the Module serves, by owner and not by
    // subtree, so the five under `/self/bin` go with the home.
    host.unload("coreutils").unwrap();
    assert_eq!(names(ctx, bin), ["load", "unload"]);
    assert_eq!(
        cell::resolve(ctx, ROOT, "/self/coreutils"),
        Err(Error::Invalid)
    );
}

#[test]
fn a_clients_own_name_reads_both_sides_and_the_servers_reads_one() {
    let rig = Rig::new("bin-run");
    rig.file("note", b"hi\n");
    let host = over(&rig, Rvm::none());
    let own = host.own();
    let ctx: &dyn Calls = &own;
    let coreutils = reovim_module_coreutils::boot("");
    host.load("coreutils", coreutils.module, coreutils.drivers).unwrap();

    // One Driver, two cwds: the `Calls` it was handed is the union, so
    // the Client's own tree and the server's are the same op to it.
    let mine = at(ctx, "/self/bin/ls");
    assert_eq!(run(ctx, mine, &listing(SELF)).unwrap(), "bin\ncoreutils\n");
    let root = at(ctx, "/module/fs/root");
    assert_eq!(run(ctx, mine, &listing(root)).unwrap(), "note\n");

    // The server's `/bin/ls` handed the same local cwd is refused, and
    // rightly: `/bin` is the server's userland and that rd is no rd of
    // the tree it reads. Nothing a person can type reaches this any
    // more, PATH holding `/self/bin` before `/bin` and the shell
    // resolving a bare name and never a path; the refusal stays, and
    // milestone 5, which serves `/self` on the wire as `client/<cid>`,
    // is what ends it for good.
    let theirs = at(ctx, "/bin/ls");
    assert_eq!(run(ctx, theirs, &listing(SELF)), Err(Error::Invalid));
    assert_eq!(run(ctx, theirs, &listing(root)).unwrap(), "note\n");
}

#[test]
fn load_gives_this_client_the_module_the_rvm_answered_with() {
    let rig = Rig::new("bin-load");
    let asked = Rc::new(Cell::new(0));
    let reads = asked.clone();
    let rvm = Rvm::new(Box::new(move |path: &str| {
        reads.set(reads.get() + 1);
        assert_eq!(path, "anything.rvm");
        Ok(("coreutils".to_string(), reovim_module_coreutils::boot("")))
    }));
    let host = over(&rig, rvm);
    let own = host.own();
    let ctx: &dyn Calls = &own;
    let bin = at(ctx, "/self/bin");
    let load = at(ctx, "/self/bin/load");
    let unload = at(ctx, "/self/bin/unload");

    // `-l` is the `local` field, which no token ever matches: the run
    // reads the file through the closure the app injected and loads
    // what it answered into this Client, from inside the Driver slot
    // the Host called, which is the depth guard doing its work. The
    // cell it answers with holds nothing, as neither name prints.
    assert_eq!(run(ctx, load, &loading(true, "anything.rvm")).unwrap(), "");
    assert_eq!(asked.get(), 1);
    let held = ["load", "unload", "stat", "ls", "tree", "cat", "grep"];
    assert_eq!(names(ctx, bin), held);
    let mine = at(ctx, "/self/bin/ls");
    assert_eq!(run(ctx, mine, &listing(SELF)).unwrap(), "bin\ncoreutils\n");

    // A path nobody typed is the Driver's own answer, before the file
    // is reached for: a declaration says which fields a run has and
    // not which of them it needs.
    assert_eq!(run(ctx, load, &loading(true, "")), Err(Error::Invalid));
    assert_eq!(run(ctx, unload, &unloading(true, "")), Err(Error::Invalid));
    assert_eq!(asked.get(), 1);

    // `unload -l` takes it back out of this Client; without the flag
    // both names speak to the link instead, and the rig's server loads
    // nothing for anybody.
    assert_eq!(run(ctx, unload, &unloading(true, "coreutils")).unwrap(), "");
    assert_eq!(names(ctx, bin), ["load", "unload"]);
    let sent = run(ctx, load, &loading(false, "anything.rvm"));
    assert_eq!(sent, Err(Error::Unsupported));
    assert_eq!(asked.get(), 1, "the wire's side reads no file here");
}
