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

use alloc::vec::Vec;

use reovim_kernel_interface::Decl;

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
    /// `explore`. `address` is read and not used — this door writes
    /// no range apart yet, and a bound or typed `:` from visual
    /// always puts `'<,'>` on its line, so the field stands here only
    /// so that keeps reaching no farther than it always has, in
    /// silence, rather than the shell's own refusal of a flag no
    /// field is named for.
    api file ver 1 {
        0 run(args: Args) -> Rd: READ;
    }
    record Args { bang: bool, address: utf8(256), path: utf8(512) }
}

reovim_sdk::declare! {
    /// `substitute`, its pattern and replacement over a range: the
    /// address as text, resolved once through [`Vim::lines_of`]
    /// (`reovim_plugin_vim::Vim`) at the run.
    api subst ver 1 {
        0 run(args: Args) -> Rd: READ;
    }
    record Args { address: utf8(256), spec: utf8(256) }
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
    record Args { bang: bool, address: utf8(256), spec: utf8(512) }
}

reovim_sdk::declare! {
    /// Whole lines deleted into an optional register.
    api lines ver 1 {
        0 run(args: Args) -> Rd: READ;
    }
    record Args { address: utf8(256), register: utf8(8) }
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
    /// spell it. The address, empty for none, is the host's to
    /// resolve through [`crate::Vim::lines_of`] — a selection's is
    /// put on the line as `'<,'>` text, as `binding` and the `:`
    /// prompt both do, not read here at all.
    api point ver 1 {
        0 run(args: Args) -> Rd: READ;
    }
    record Args { address: utf8(256), col: bool, relative: bool }
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

/// The full names, each with the declaration of the shape it takes.
///
/// [`abbreviated`] is every shorter name nvim also takes for one of
/// them, the same shape. Registered under `/self/vim/bin` as data at
/// `start`, both tables: the shell's PATH lookup is by exact name, so
/// no order here resolves a prefix the way it once did.
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
        // Reovim's own, none of nvim's: no abbreviation of any.
        ("bind", bound::decl()),
        ("point", point::decl()),
        ("goto", located::decl()),
    ]
}

/// The shortest legal abbreviation of a name, and the name itself:
/// `:help`'s own `w[rite]` read as the pair `(1, "write")`, nvim's
/// own minimum for every name it has, `cq[uit]` included — `c` is
/// nvim's `:change`, which reovim has not, and a name nvim never
/// spells this way is better E492 than something else. `explore` and
/// `cli` are reovim's own, which nvim names no abbreviation for at
/// all, and take one each short of what they would collide with:
/// `edit` standing first takes `e`/`ed`/`edi`, so `explore` is
/// `ex`/`exp`/`expl`/`explo`/`explor`, and `cli` is `cl` alone. `wq`,
/// `buffers`, `bind`, `point` and `goto` take no shorter form at all.
const ABBREVIATED: &[(&str, usize)] = &[
    ("write", 1),
    ("quit", 1),
    ("xit", 1),
    ("cquit", 2),
    ("substitute", 1),
    ("set", 2),
    ("nohlsearch", 3),
    ("edit", 1),
    ("explore", 2),
    ("cli", 2),
    ("global", 1),
    ("vglobal", 1),
    ("delete", 1),
    ("buffer", 1),
    ("bnext", 2),
    ("bprevious", 2),
    ("bdelete", 2),
    ("qall", 2),
    ("wall", 2),
    ("wqall", 3),
];

/// Every abbreviation nvim takes for a name it has, next to the full
/// name it stands for.
///
/// `w[rite]`'s brackets read out whole, `w`,
/// `wr`, `wri`, `writ` for `write`, the full spelling itself being
/// [`declared`]'s own and not repeated here. Registered under
/// `/self/vim/bin` the same as a full name, the exact same
/// declaration, so the shell's exact-name lookup finds either.
#[must_use]
pub fn abbreviated() -> Vec<(&'static str, &'static str)> {
    ABBREVIATED
        .iter()
        .flat_map(|(name, min)| {
            (*min..name.len()).map(move |n| (&name[..n], *name))
        })
        .collect()
}

/// The full name `name` stands for.
///
/// Itself where it already is one, else the one [`abbreviated`]
/// answers for it, else `name` back unchanged — every name a `:`
/// line's dispatch is ever handed is one `/self/vim/bin` registered,
/// so that last case names nothing real.
#[must_use]
pub fn canonical(name: &str) -> &str {
    abbreviated()
        .into_iter()
        .find(|(short, _)| *short == name)
        .map_or(name, |(_, full)| full)
}

#[cfg(test)]
mod tests {
    use super::{abbreviated, canonical, declared};

    #[test]
    fn an_abbreviation_is_a_name_nvim_takes_and_none_others() {
        let short = abbreviated();
        assert!(short.contains(&("s", "substitute")));
        assert!(short.contains(&("su", "substitute")));
        assert!(short.contains(&("se", "set")));
        assert!(short.contains(&("w", "write")));
        assert!(short.contains(&("noh", "nohlsearch")));
        assert!(short.contains(&("bd", "bdelete")));
        assert!(short.contains(&("ex", "explore")), "e/ed/edi are edit's");
        assert!(short.contains(&("cq", "cquit")), "c is nvim's :change");
        assert!(!short.iter().any(|(s, _)| *s == "c"), "c names nothing");
        assert!(short.contains(&("cl", "cli")));
        // The rest of reovim's own take no shorter form at all.
        for name in ["wq", "buffers", "bind", "point", "goto"] {
            assert!(
                !short.iter().any(|(_, full)| *full == name),
                "{name} has no abbreviation"
            );
        }
        // Every abbreviation is shorter than the name it stands for,
        // and stands for one `declared` actually has.
        let full: alloc::vec::Vec<&str> =
            declared().iter().map(|(name, _)| *name).collect();
        for (short, name) in short {
            assert!(short.len() < name.len(), "{short} {name}");
            assert!(full.contains(&name), "{name} is not a declared name");
        }
    }

    #[test]
    fn canonical_reads_a_short_name_back_to_its_own() {
        assert_eq!(canonical("s"), "substitute");
        assert_eq!(canonical("se"), "set");
        assert_eq!(canonical("write"), "write", "already full");
        assert_eq!(canonical("wq"), "wq", "none of nvim's, none shorter");
        assert_eq!(canonical("cq"), "cquit");
        assert_eq!(canonical("c"), "c", "nvim's :change, which is not here");
    }
}
