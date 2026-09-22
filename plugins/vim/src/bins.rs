//! The names of Vim's `:` line, the declarations they take.
//!
//! A declaration is a shape and not a name, shared by matching names,
//! and the rd a call comes in on is which name it is — where a Module
//! whose names each have a shape of their own tells them apart by the
//! api's hash. What a name takes is its declaration and nothing beside
//! it: the range and the bang are Vim's own language, and the engine's
//! own `ex` puts them back on the line as `--address` and `--bang`,
//! for the shell to read as it reads any other flag. An address is
//! text wherever it goes and never a pair of lines: a name that
//! declares one resolves it at the run through the engine's one
//! resolver, [`crate::Vim::lines_of`].
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
    /// `write`: a bang, a range and a path. `address` is the range's
    /// own text — `:{range}w PATH` writes the lines it names apart.
    /// The path is the rest of the line, as nvim's is: `:w my file`
    /// names one file.
    api file ver 1 {
        0 run(args: Args) -> Rd: READ;
    }
    record Args { bang: bool, address: utf8(256), path: rest(512) }
}

reovim_sdk::declare! {
    /// `edit` and `explore`: a bang and a path, the rest of the line
    /// as nvim reads it (`:e my file` opens one file). No `address`:
    /// nvim answers E481 for a range on either, and a record that
    /// declares no field for the `--address` the engine's `ex` puts
    /// on the line is what the host words that way.
    api opened ver 1 {
        0 run(args: Args) -> Rd: READ;
    }
    record Args { bang: bool, path: rest(512) }
}

reovim_sdk::declare! {
    /// `substitute`, its pattern and replacement over a range: the
    /// address as text, resolved once through [`Vim::lines_of`]
    /// (`reovim_plugin_vim::Vim`) at the run.
    api subst ver 1 {
        0 run(args: Args) -> Rd: READ;
    }
    record Args { address: utf8(256), spec: rest(256) }
}

reovim_sdk::declare! {
    /// `set`, whose options are the rest of the line. No `address`:
    /// `:'<,'>set ic` is E481 in nvim.
    api option ver 1 {
        0 run(args: Args) -> Rd: READ;
    }
    record Args { options: rest(128) }
}

reovim_sdk::declare! {
    /// A global selection and the command to run on every marked line.
    api each ver 1 {
        0 run(args: Args) -> Rd: READ;
    }
    record Args { bang: bool, address: utf8(256), spec: rest(512) }
}

reovim_sdk::declare! {
    /// Whole lines deleted into an optional register.
    api lines ver 1 {
        0 run(args: Args) -> Rd: READ;
    }
    record Args { address: utf8(256), register: utf8(8) }
}

reovim_sdk::declare! {
    /// Select or close a draft by its name under the Client's
    /// buffers. The name is the rest of the line: nvim's `:b my file`
    /// names the draft over `my file`.
    api numbered ver 1 {
        0 run(args: Args) -> Rd: READ;
    }
    record Args { bang: bool, name: rest(64) }
}

reovim_sdk::declare! {
    /// `bind`: the keys and the `:` line they run. The line is the
    /// rest of this one, so a bound line may hold a flag of its own
    /// (`bind <C-r> search --backward`) and a `>` of its own
    /// (`bind <Leader>y point > /self/dev/clipboard`); it is as long
    /// as a `:` line, which the resolver's `Command` bounds at 4096.
    api bound ver 1 {
        0 run(args: Args) -> Rd: READ;
    }
    record Args { keys: utf8(64), line: rest(4096) }
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

reovim_sdk::declare! {
    /// `search`: a pattern to look for and the way to go. The pattern
    /// is the rest of the line, verbatim, so `:search a b` looks for
    /// `a b`; none at all opens the prompt, which is what makes a key
    /// bound to this name the `/` key.
    api sought ver 1 {
        0 run(args: Args) -> Rd: READ;
    }
    record Args { backward: bool, pattern: rest(1024) }
}

/// The full names, each with the declaration of the shape it takes.
///
/// [`abbreviated`] is every shorter name nvim also takes for one of
/// them, the same shape. Registered under `/self/vim/bin` as data at
/// `start`, both tables: the shell's PATH lookup is by exact name, so
/// no order here resolves a prefix the way it once did.
#[must_use]
pub fn declared() -> [(&'static str, Decl); 26] {
    let (plain, bang, opened) = (plain::decl(), bang::decl(), opened::decl());
    [
        ("write", file::decl()),
        ("wq", plain.clone()),
        ("quit", bang.clone()),
        ("xit", plain.clone()),
        ("cquit", plain.clone()),
        ("substitute", subst::decl()),
        ("set", option::decl()),
        ("nohlsearch", plain.clone()),
        ("edit", opened.clone()),
        ("explore", opened),
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
        ("search", sought::decl()),
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
/// `buffers`, `bind`, `point`, `goto` and `search` take no shorter
/// form at all.
static ABBREVIATED: &[(&str, usize)] = &[
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
///
/// Read straight off the table [`abbreviated`] expands, as the tree
/// holds it: `name` stands for `full` exactly where it is a prefix of
/// it no shorter than that name's minimum and shorter than the whole
/// spelling, so the eighty pairs are never built here. A `:` line
/// asks this twice and a Tab candidate once.
#[must_use]
pub fn canonical(name: &str) -> &str {
    let stands_for = |(full, min): &&(&'static str, usize)| {
        (*min..full.len()).contains(&name.len()) && full.starts_with(name)
    };
    ABBREVIATED.iter().find(stands_for).map_or(name, |(full, _)| *full)
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
        for name in ["wq", "buffers", "bind", "point", "goto", "search"] {
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
