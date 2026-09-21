//! The names of Vim's `:` line, the declarations they take.
//!
//! A declaration is a shape and not a name, shared by matching names,
//! and the rd a call comes in on is which name it is — where a Module
//! whose names each have a shape of their own tells them apart by the
//! api's hash. What a name takes is its declaration and nothing beside
//! it: the range and the bang are Vim's own language and reach a run as
//! the environment its host set, matched by name and type, `from` and
//! `to` being lines from 0 as the engine counts them.
//!
//! What a run of one does is the host's: it holds the `Api` impls,
//! each answering the one effect the arm `ex` used to have.

use reovim_kernel_interface::{Decl, Ty};

reovim_sdk::declare! {
    /// A name that reads nothing after it: `wq`, `xit`, `cquit`,
    /// `nohlsearch`.
    api plain ver 1 {
        0 run(args: Args) -> Rd: READ;
    }
    record Args {}
}

reovim_sdk::declare! {
    /// A name that reads a bang and nothing else: `quit`, `cli`.
    api bang ver 1 {
        0 run(args: Args) -> Rd: READ;
    }
    record Args { bang: bool }
}

reovim_sdk::declare! {
    /// A name that reads a bang and a path: `write`, `edit`,
    /// `explore`.
    api file ver 1 {
        0 run(args: Args) -> Rd: READ;
    }
    record Args { bang: bool, path: utf8(512) }
}

reovim_sdk::declare! {
    /// `substitute`, its pattern and replacement over a range.
    api subst ver 1 {
        0 run(args: Args) -> Rd: READ;
    }
    record Args { from: u64, to: u64, spec: utf8(256) }
}

reovim_sdk::declare! {
    /// `set`, whose options are the rest of the line.
    api option ver 1 {
        0 run(args: Args) -> Rd: READ;
    }
    record Args { options: utf8(128) }
}

reovim_sdk::declare! {
    /// A global selection and the command to run on every marked line.
    api each ver 1 {
        0 run(args: Args) -> Rd: READ;
    }
    record Args {
        bang: bool, ranged: bool, from: u64, to: u64, spec: utf8(512)
    }
}

reovim_sdk::declare! {
    /// Whole lines deleted into an optional register.
    api lines ver 1 {
        0 run(args: Args) -> Rd: READ;
    }
    record Args { from: u64, to: u64, register: utf8(8) }
}

reovim_sdk::declare! {
    /// Select or close a draft by its name under the Client's buffers.
    api numbered ver 1 {
        0 run(args: Args) -> Rd: READ;
    }
    record Args { bang: bool, name: utf8(64) }
}

reovim_sdk::declare! {
    /// `bind`: the keys and the `:` line they run.
    api bound ver 1 {
        0 run(args: Args) -> Rd: READ;
    }
    record Args { keys: utf8(64), line: utf8(512) }
}

reovim_sdk::declare! {
    /// `point`: where the cursor or the selection is, and how to
    /// spell it. The range reaches it as the environment its host
    /// set, as `global`'s does.
    api point ver 1 {
        0 run(args: Args) -> Rd: READ;
    }
    record Args {
        ranged: bool, from: u64, to: u64, col: bool, relative: bool
    }
}

reovim_sdk::declare! {
    /// `goto`: where in the text to land, in the grammar `point`
    /// writes it. One argument and nothing else; another domain's
    /// locator is said back, never read.
    api located ver 1 {
        0 run(args: Args) -> Rd: READ;
    }
    record Args { locator: utf8(64) }
}

/// The names, in the order a `:` line resolves one, each with the
/// declaration of the shape it takes.
///
/// `:w` is `write` because it stands before `wq`, `:s` is `substitute`
/// before `set`, `:e` is `edit` before `explore`, and `:c` is `cquit`
/// before `cli`, which takes `:cl`. That order is the order they are
/// registered in and the order `children(/self/vim/bin)` lists them,
/// which is where the abbreviation is read from; nothing lists a short
/// form by hand.
#[must_use]
pub fn declared() -> [(&'static str, Decl); 25] {
    let (plain, bang, file) = (plain::decl(), bang::decl(), file::decl());
    [
        ("write", file.clone()),
        ("wq", plain.clone()),
        ("quit", bang.clone()),
        ("xit", plain.clone()),
        ("cquit", plain.clone()),
        ("substitute", subst::decl()),
        ("set", option::decl()),
        ("nohlsearch", plain.clone()),
        ("edit", file.clone()),
        ("explore", file),
        ("cli", bang.clone()),
        ("global", each::decl()),
        ("vglobal", each::decl()),
        ("delete", lines::decl()),
        ("buffer", numbered::decl()),
        ("buffers", plain.clone()),
        ("bnext", plain.clone()),
        ("bprevious", plain.clone()),
        ("bdelete", numbered::decl()),
        ("qall", bang),
        ("wall", plain.clone()),
        ("wqall", plain),
        // Last, after `wqall`: `:b` is `buffer` still and `:bi` is
        // this one, the listing being the whole of the ex rule.
        ("bind", bound::decl()),
        // And after `bind`: `:p` is this one, no other name of Vim's
        // beginning with it, and `:po` is this one too.
        ("point", point::decl()),
        // And after `point`: `:g` is `global` still and `:go` is this
        // one, no other name beginning with those two.
        ("goto", located::decl()),
    ]
}

/// How many arguments of a `:` line the run of `name` fills.
///
/// They are the `utf8` fields of the record it takes. Every
/// positional of Vim's own names is one of those and nothing else of
/// theirs is — the range and the bang reach a run as the environment
/// its host set. Zero for a name that is none of Vim's.
#[must_use]
pub fn positionals(name: &str) -> usize {
    let Some((_, decl)) = declared().into_iter().find(|(at, _)| *at == name)
    else {
        return 0;
    };
    let Some(Ty::Record(fields)) =
        decl.methods.first().map(|method| &method.input)
    else {
        return 0;
    };
    fields.iter().filter(|field| matches!(field.ty, Ty::Utf8(_))).count()
}
