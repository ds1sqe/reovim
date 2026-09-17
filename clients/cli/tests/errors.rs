//! The line a failed command prints: `WORD TARGET: cause; way out`,
//! which the app puts after `reovim: `. The rows of the cause table
//! through real calls where the rig can make the fault, on the table
//! itself where it cannot.

use reovim_client::{Client, Error, Fault};
use reovim_client_cli::{CAUSES, Miss, Out, Step, USAGE, help, input, plain};
use reovim_rig::Rig;

/// One command line; the line it would print, or its output as text.
fn line(client: &mut Client, command: &str) -> String {
    let words: Vec<String> =
        command.split_whitespace().map(str::to_string).collect();
    match input(client, &words, Out::plain()) {
        Ok(out) => String::from_utf8(out).unwrap(),
        Err(miss) => miss.to_string(),
    }
}

/// The nine the Kernel may answer with.
const ERRORS: [Error; 9] = [
    Error::Denied,
    Error::Unsupported,
    Error::Invalid,
    Error::Stale,
    Error::Busy,
    Error::NoSpace,
    Error::Closed,
    Error::Gone,
    Error::Cancelled,
];

#[test]
fn a_refused_command_says_its_word_its_target_and_why() {
    let rig = Rig::new("errors");
    rig.file("note", b"hi\n");
    let mut client = rig.client();
    let says = |client: &mut Client, command: &str| line(client, command);

    // read of a Container, and of a path that does not resolve: the
    // first segment that did not, wherever the walk stopped.
    assert_eq!(
        says(&mut client, "read /module/fs/root"),
        "read /module/fs/root: a Container has no bytes; ls it"
    );
    assert_eq!(
        says(&mut client, "read /nope/deep"),
        "read /nope/deep: no such rd (nope)"
    );
    assert_eq!(
        says(&mut client, "read /module/fs/nope"),
        "read /module/fs/nope: no such rd (nope)"
    );
    // Below a cell there is nothing to look up.
    assert_eq!(
        says(&mut client, "read /module/fs/root/note/below"),
        "read /module/fs/root/note/below: no such rd (below)"
    );
    // A name that is not on disk is an empty file until it is written
    // (modules/fs), so it resolves and reads empty; the cli knows no
    // Module and has nothing to say about it.
    assert_eq!(says(&mut client, "read /module/fs/root/nope"), "");

    // A method the rd does not offer lists the ones it does.
    assert_eq!(
        says(&mut client, "call /module/fs/root nothing"),
        "call /module/fs/root: no method nothing; it offers: create"
    );
    assert_eq!(
        says(&mut client, "call /module/fs/root/note nothing"),
        "call /module/fs/root/note: no method nothing; it offers: none"
    );

    // A Kernel Container that grants no WRITE, and one that does but
    // takes no bytes.
    assert_eq!(says(&mut client, "write /sys x"), "write /sys: not writable");
    assert_eq!(
        says(&mut client, "write /module/fs x"),
        "write /module/fs: a Container takes no bytes"
    );

    // A target that is neither a path nor an rd, said before sending.
    assert_eq!(
        says(&mut client, "lookup here"),
        "lookup here: not a path or rd:<n>: here"
    );
}

#[test]
fn a_wrong_word_or_count_gets_one_line_and_points_at_help() {
    let rig = Rig::new("usage");
    let mut client = rig.client();
    let says = |client: &mut Client, command: &str| line(client, command);

    assert_eq!(
        says(&mut client, "nonsense /"),
        "unknown word nonsense; help lists them"
    );
    assert_eq!(
        says(&mut client, "read"),
        "usage: read <target>; help read says more"
    );
    assert_eq!(
        says(&mut client, "read / and-more"),
        "usage: read <target>; help read says more"
    );
    assert_eq!(
        says(&mut client, "write /sys/log a b"),
        "usage: write <target> [bytes]; help write says more"
    );
    assert_eq!(
        says(&mut client, "log seven"),
        "usage: log [FROM]; help log says more"
    );
    assert_eq!(says(&mut client, ""), "no word; help lists them");
    // `quit` ends an interactive session; the app takes it there and
    // it never reaches the words.
    assert_eq!(says(&mut client, "quit"), "quit ends an interactive session");

    // `help` is the table, `help WORD` one entry, to stdout.
    assert_eq!(says(&mut client, "help"), format!("{USAGE}\n"));
    assert_eq!(
        says(&mut client, "help read"),
        "read <target>                   bytes to stdout\n"
    );
    assert_eq!(
        says(&mut client, "help quit"),
        "quit                            end an interactive session\n"
    );
    assert_eq!(
        says(&mut client, "help nonsense"),
        "unknown word nonsense; help lists them"
    );
    let words = |line: &str| -> Vec<String> {
        line.split_whitespace().map(str::to_string).collect()
    };
    // The app answers these before it opens a link; anything else is
    // not a help request.
    assert_eq!(help(&words("--help")).unwrap().unwrap(), format!("{USAGE}\n"));
    assert!(help(&words("-h read")).unwrap().unwrap().starts_with("read "));
    assert!(help(&words("read /")).is_none());
    assert!(help(&words("")).is_none());
}

#[test]
fn a_link_that_is_not_there_names_the_socket() {
    // The rig cannot cut a link, so the three faults that are not the
    // Kernel's are read from the words the app prints them with.
    let socket = "/run/user/1000/reovim.sock";
    assert_eq!(
        plain(&Fault::Carrier, Some(socket)).to_string(),
        "no server at /run/user/1000/reovim.sock"
    );
    assert_eq!(plain(&Fault::Carrier, None).to_string(), "link gone");
    assert_eq!(
        plain(&Fault::Wire, None).to_string(),
        "not a message from the server"
    );
    assert_eq!(Miss::line("said as it is").to_string(), "said as it is");
}

#[test]
fn every_row_of_the_cause_table_is_one_word_of_the_usage_and_no_two_are_one() {
    let mut seen: Vec<(&str, Step, Error)> = Vec::new();
    for (word, step, error, cause) in CAUSES {
        let key = (*word, *step, *error);
        assert!(!seen.contains(&key), "{word} {step:?} {error:?} twice");
        seen.push(key);
        assert!(
            word.is_empty()
                || USAGE
                    .lines()
                    .any(|line| line.starts_with(&format!("{word} "))),
            "{word} is not a word of the usage"
        );
        assert!(!cause.is_empty() && !cause.ends_with(' '), "{cause}");
        // `{}` is filled by the call site: the name that did not
        // resolve, or the method and what the rd offers.
        let holes = cause.matches("{}").count();
        assert_eq!(
            holes,
            usize::from(*step == Step::Resolve)
                + 2 * usize::from(*step == Step::Method),
            "{cause}"
        );
    }
    // The op step answers for all nine, so no error is ever nameless.
    for error in ERRORS {
        assert!(seen.contains(&("", Step::Op, error)), "{error:?} has no row");
    }
}
