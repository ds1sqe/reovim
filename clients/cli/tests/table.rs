//! The word table: what a name resolves to, the abbreviation the
//! order computes for a word, and the text `help` prints, which is
//! the table's and must not have moved a byte.

use reovim_client_cli::{WORDS, Word, Words};

/// `help` as it printed before the table, byte for byte: the words in
/// their order, then the paragraphs that belong to no one word.
const WAS: &str = "\
ls [-l] [-a] [-w] [TARGET..]    children by name; -l long, -a rds
cat [-f] [-n N] <target>..      the bytes of each in turn; -f follows
stat <target>                   the long line, the times, the apis
tree [-L N] [TARGET]            the tree under it, N levels deep
dmesg [-tTHxdrkwW] [-l LEVELS]  the log as dmesg reads it; -w follows
watch <target>                  seq kind name, per event, as they come
cd [TARGET]                     go there; none is /, - the one before
pwd                             where the line stands now
lookup <target>                 rd
info <target>                   name, flags, times, apis
children <target>               rd and name per line
read <target>                   bytes to stdout
write <target> [bytes]          bytes, or stdin without them, at 0
call <target> <method> [arg]    output decoded by the declaration
log [FROM]                      entries from seq FROM, default 1
load <path>                     the Module in that .rvm file
unload <name>                   the Module of that name
help [WORD]                     these lines, or the one for WORD
quit                            end an interactive session
<target> is a path from /, a name under the cwd, . or .., or rd:<n>

the line: Tab completes, Up and Down walk the lines that begin as this
one does, Ctrl-A the start, Ctrl-E the end, Ctrl-U cuts to the start,
Ctrl-W the word before the cursor, Ctrl-L clears the screen; Ctrl-D
deletes under the cursor, and ends the session on an empty line, as
Ctrl-C does";

#[test]
fn the_usage_is_the_table_printed_and_has_not_moved_a_byte() {
    assert_eq!(reovim_client_cli::usage(), WAS);
}

#[test]
fn a_name_resolves_to_the_first_word_of_the_table_it_begins() {
    let words = Words::cli();
    let named = |typed: &str| words.resolve(typed).map(|word| word.name);

    // A whole name is itself, and a beginning is the first word that
    // has it, in the order `help` lists them.
    assert_eq!(named("ls"), Some("ls"));
    assert_eq!(named("l"), Some("ls"), "before lookup, load and log");
    assert_eq!(named("lo"), Some("lookup"), "before log and load");
    assert_eq!(named("c"), Some("cat"), "before cd, children and call");
    assert_eq!(named("w"), Some("watch"), "before write");
    assert_eq!(named("h"), Some("help"));

    // A name no word begins with is no word, and neither is nothing.
    assert_eq!(named("zz"), None);
    assert_eq!(named("lsx"), None);
    assert_eq!(named("Ls"), None, "the names are lower-case");
    assert_eq!(named(""), None);
}

#[test]
fn an_abbreviation_is_the_shortest_beginning_that_resolves() {
    let words = Words::cli();
    let short = |name: &str| words.abbrev(words.resolve(name).unwrap());

    assert_eq!(short("ls"), "l");
    assert_eq!(short("lookup"), "lo");
    assert_eq!(short("log"), "log");
    assert_eq!(short("load"), "loa");
    assert_eq!(short("cat"), "c");
    assert_eq!(short("cd"), "cd");
    assert_eq!(short("children"), "ch");
    assert_eq!(short("call"), "cal");
    assert_eq!(short("watch"), "w");
    assert_eq!(short("write"), "wr");
    assert_eq!(short("quit"), "q");

    // Every word has one, and typing it is that word again.
    for word in WORDS {
        let abbrev = words.abbrev(word);
        assert_eq!(words.resolve(abbrev), Some(word), "{}", word.name);
    }
}

#[test]
fn a_prefix_begins_the_words_tab_offers() {
    let words = Words::cli();
    let starting = |prefix: &str| -> Vec<&str> {
        words.starting(prefix).into_iter().map(|word| word.name).collect()
    };

    // In the table's order, which is the order they resolve in.
    assert_eq!(starting("l"), ["ls", "lookup", "log", "load"]);
    assert_eq!(starting("lo"), ["lookup", "log", "load"]);
    assert_eq!(starting("lookup"), ["lookup"]);
    assert_eq!(starting("w"), ["watch", "write"]);
    assert_eq!(starting("pwd"), ["pwd"]);

    // A name no word begins with offers none; nothing begins them all.
    assert_eq!(starting("zz"), [] as [&str; 0]);
    assert_eq!(starting("lsx"), [] as [&str; 0]);
    assert_eq!(starting("").len(), WORDS.len());

    // Every word is offered by its own abbreviation, and the one that
    // resolves is the first offered.
    for word in WORDS {
        let abbrev = words.abbrev(word);
        let offered = starting(abbrev);
        let resolved = words.resolve(abbrev).unwrap().name;
        assert_eq!(offered.first(), Some(&resolved), "{}", word.name);
        assert!(offered.contains(&word.name), "{}", word.name);
    }
}

#[test]
fn a_shadowed_name_is_offered_once_and_by_the_table_that_has_it_first() {
    let first = &[
        Word { name: "write", args: "[PATH]", help: "the buffer" },
        Word { name: "wq", args: "", help: "write, then quit" },
    ];
    let words = Words { first, then: WORDS };
    let starting = |prefix: &str| -> Vec<&Word> { words.starting(prefix) };

    // The Plugin's two, then the cli's `watch`; the cli's own `write`,
    // which no beginning can reach, is not a second `write`.
    let names: Vec<&str> = starting("w").iter().map(|w| w.name).collect();
    assert_eq!(names, ["write", "wq", "watch"]);
    assert_eq!(starting("w")[0], &first[0], "the Plugin's, not the cli's");
    assert_eq!(starting("write"), [&first[0]]);
    assert_eq!(starting("wa").len(), 1, "the cli's watch alone");
}

#[test]
fn a_plugins_words_come_first_and_shadow_the_clis() {
    let first = &[
        Word { name: "write", args: "[PATH]", help: "the buffer" },
        Word { name: "wq", args: "", help: "write, then quit" },
    ];
    let words = Words { first, then: WORDS };
    let named = |typed: &str| words.resolve(typed).map(|word| word.name);

    // `w` is the Plugin's `write`, not the cli's `watch`, and the cli's
    // own `write` is not reached by any beginning of it at all.
    assert_eq!(named("w"), Some("write"));
    assert_eq!(named("wq"), Some("wq"));
    assert_eq!(named("wa"), Some("watch"));
    assert_eq!(words.abbrev(&first[0]), "w");
    assert_eq!(words.abbrev(&first[1]), "wq");
    assert_eq!(words.abbrev(&WORDS[5]), "wa", "the cli's watch");

    // A word another shadows whole has no abbreviation of its own: the
    // shortest beginning that resolves to it is nothing shorter than
    // its name, and even that is the Plugin's.
    let shadowed = WORDS.iter().find(|word| word.name == "write").unwrap();
    assert_eq!(words.abbrev(shadowed), "write");
    assert_eq!(named("write"), Some("write"));
    assert_eq!(words.resolve("write"), Some(&first[0]));
}
