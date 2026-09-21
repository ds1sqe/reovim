//! The resolver's records, from a consumer linking this api and the
//! sdk and nothing of any Plugin: every record through the public
//! `Encode`/`Decode`, the continuation pairing an `Outcome` with the
//! `Reply` that answers it, a refusal's error, a match's entries and
//! how far it filled, the `Press` a synthetic key carries, and the
//! fields a kind leaves where they were.

use core::fmt::Debug;

use reovim_client_api::keyboard::Press;
use reovim_client_api::resolver::{
    Edit, Entry, Fed, Goto, Input, Line, Open, Outcome, Place, Reply, Search,
    State,
};
use reovim_client_api::{NONE, filled, kind, land, origin, reply};
use reovim_sdk::{Decode, Encode, decode, encode};

/// Every kind an `Outcome` may be: what the engine's effects narrow
/// to where they cross the boundary.
const KINDS: [u64; 22] = [
    kind::NOTHING,
    kind::MOVED,
    kind::EDITS,
    kind::UNDO,
    kind::REDO,
    kind::SAVE,
    kind::SAVE_AS,
    kind::QUIT,
    kind::SWITCH,
    kind::CLOSE,
    kind::LIST,
    kind::SAVE_ALL,
    kind::EXPLORE,
    kind::OPEN,
    kind::RELOAD,
    kind::LINE,
    kind::CLI,
    kind::COMPLETE,
    kind::CLIPBOARD,
    kind::GOTO,
    kind::SEARCH,
    kind::FED,
];

/// Whose a key is: every origin an `Input` may carry, which a
/// release keeps rather than trading away.
const ORIGINS: [u64; 3] = [origin::TYPED, origin::FED, origin::REPEAT];

/// The value's bytes read back as the value.
fn round_trip<T: Debug + Decode + Encode + PartialEq>(value: &T) {
    let back: T = decode(&encode(value)).expect("one value");
    assert_eq!(&back, value);
}

/// How many of `values` differ: a tag that repeats another's value
/// would not be a tag of its own.
fn distinct(values: &[u64]) -> usize {
    let mut seen = values.to_vec();
    seen.sort_unstable();
    seen.dedup();
    seen.len()
}

/// A genuine press: the key's code, the Client's clock, and the rd
/// that put the key there.
const fn press() -> Press {
    Press { key: 0x78, when: 1_700_000_000, source: NONE }
}

fn edit() -> Edit {
    Edit { at: 12, len: 3, text: "abc".to_owned(), join: true }
}

fn line() -> Line {
    Line { ranged: true, from: 1, to: 9, line: "write".to_owned() }
}

fn open() -> Open {
    Open { path: "/tmp/x".to_owned(), bang: false, locator: "12:4".to_owned() }
}

const fn goto() -> Goto {
    Goto { over: NONE, line: 42, column: 7, land: land::EXACT }
}

fn search() -> Search {
    Search {
        pattern: "wo.d".to_owned(),
        ignorecase: true,
        smartcase: true,
        from: 30,
        epoch: 4,
    }
}

fn fed() -> Fed {
    Fed { keys: vec![0x61, 0x62], repeat: false }
}

/// An `Outcome` at rest: `NOTHING`, nothing waiting, every payload
/// empty — what a kind that uses none of them leaves behind.
const fn blank_outcome() -> Outcome {
    Outcome {
        kind: kind::NOTHING,
        continuation: 0,
        edits: Vec::new(),
        line: Line { ranged: false, from: 0, to: 0, line: String::new() },
        open: Open { path: String::new(), bang: false, locator: String::new() },
        goto: Goto { over: NONE, line: 0, column: 0, land: land::LINE },
        search: Search {
            pattern: String::new(),
            ignorecase: false,
            smartcase: false,
            from: 0,
            epoch: 0,
        },
        fed: Fed { keys: Vec::new(), repeat: false },
        name: String::new(),
        flag: false,
    }
}

/// A `Reply` at rest: it answers no continuation, fills nothing and
/// refuses nothing.
const fn blank_reply() -> Reply {
    Reply {
        continuation: 0,
        kind: reply::PASTED,
        text: String::new(),
        place: Place { cursor: 0, top: 0 },
        entries: Vec::new(),
        filled: filled::ABSENT,
        error: 0,
    }
}

/// A `State` at rest: no mode's word, nothing pending, nothing waited
/// for.
const fn blank_state() -> State {
    State {
        mode: 0,
        status: String::new(),
        showcmd: String::new(),
        pending: false,
        waiting: 0,
    }
}

#[test]
fn every_record_round_trips() {
    round_trip(&Input {
        press: press(),
        origin: origin::TYPED,
        released: false,
    });
    round_trip(&edit());
    round_trip(&line());
    round_trip(&open());
    round_trip(&goto());
    round_trip(&search());
    round_trip(&fed());
    round_trip(&Outcome {
        kind: kind::EDITS,
        continuation: 7,
        edits: vec![edit(), edit()],
        ..blank_outcome()
    });
    round_trip(&Place { cursor: 3, top: 1 });
    round_trip(&Entry { at: 5, len: 2 });
    round_trip(&Reply {
        continuation: 7,
        kind: reply::LOADED,
        place: Place { cursor: 3, top: 1 },
        ..blank_reply()
    });
    round_trip(&State {
        mode: 1,
        status: "-- INSERT --".to_owned(),
        showcmd: "3d".to_owned(),
        pending: true,
        waiting: 7,
    });
    assert_eq!(distinct(&[land::LINE, land::EXACT, land::WALKED]), 3);
}

#[test]
fn a_continuation_pairs_an_outcome_with_its_reply() {
    let asked =
        Outcome { kind: kind::CLIPBOARD, continuation: 91, ..blank_outcome() };
    let answer = Reply {
        continuation: asked.continuation,
        kind: reply::PASTED,
        text: "yanked".to_owned(),
        ..blank_reply()
    };
    let asked: Outcome = decode(&encode(&asked)).expect("one outcome");
    let answer: Reply = decode(&encode(&answer)).expect("one reply");
    assert_eq!(answer.continuation, asked.continuation);
    assert_eq!(answer.text, "yanked");
    // `State.waiting` is the continuation still waited for, 0 for
    // none: a reply the state no longer names is a stale one.
    let waiting = State { waiting: asked.continuation, ..blank_state() };
    assert_eq!(waiting.waiting, answer.continuation);
    assert_ne!(blank_state().waiting, answer.continuation);
    assert_eq!(blank_outcome().continuation, 0, "nothing waits");
}

#[test]
fn a_refused_reply_carries_its_error() {
    // `error` is the `Error` as the Kernel numbers it on the wire,
    // and is read only where `kind` is `REFUSED`.
    let stale = 3;
    let refused = Reply {
        continuation: 12,
        kind: reply::REFUSED,
        filled: filled::REFUSED,
        error: stale,
        ..blank_reply()
    };
    let back: Reply = decode(&encode(&refused)).expect("one reply");
    assert_eq!(back.kind, reply::REFUSED);
    assert_eq!(back.error, stale);
    assert_eq!(back.filled, filled::REFUSED);
    assert_eq!(back.text, blank_reply().text, "a refusal pastes nothing");
    let kinds = [reply::PASTED, reply::LOADED, reply::MATCHED, reply::REFUSED];
    assert_eq!(distinct(&kinds), 4);
}

#[test]
fn a_matched_reply_carries_its_entries_and_filled() {
    let matched = Reply {
        continuation: 5,
        kind: reply::MATCHED,
        entries: vec![Entry { at: 4, len: 2 }, Entry { at: 40, len: 2 }],
        filled: filled::MORE,
        ..blank_reply()
    };
    let back: Reply = decode(&encode(&matched)).expect("one reply");
    assert_eq!(back.entries.len(), 2);
    assert_eq!(back.entries[1], Entry { at: 40, len: 2 });
    assert_eq!(back.filled, filled::MORE, "the slice is not the whole");
    assert_eq!(blank_reply().entries, Vec::new(), "a reply fills nothing");
    let spent = Reply { filled: filled::SPENT, ..matched };
    let spent: Reply = decode(&encode(&spent)).expect("one reply");
    assert_eq!(spent.filled, filled::SPENT);
    assert_eq!(spent.entries.len(), 2, "what it found it keeps");
    let fills = [
        filled::ABSENT,
        filled::MORE,
        filled::WHOLE,
        filled::SPENT,
        filled::REFUSED,
    ];
    assert_eq!(distinct(&fills), 5);
}

#[test]
fn a_synthetic_input_carries_the_press_that_made_it() {
    let typed =
        Input { press: press(), origin: origin::TYPED, released: false };
    // The `@a` and the `.` invent no clock and no source: each
    // synthetic key carries the `Press` of the key that made it, and
    // a failed prefix's release carries the prefix's own.
    let synthetic = [origin::FED, origin::REPEAT].map(|origin| Input {
        press: typed.press.clone(),
        origin,
        released: false,
    });
    for input in &synthetic {
        assert_eq!(input.press, typed.press);
        assert_ne!(input.origin, typed.origin);
        round_trip(input);
    }
    let back: Input = decode(&encode(&synthetic[0])).expect("one input");
    assert_eq!(back.press.when, press().when);
    assert_eq!(back.press.source, press().source);
    assert_eq!(back.press.key, press().key);
    assert_eq!(distinct(&ORIGINS), 3);
}

#[test]
fn a_released_input_says_a_release_apart_from_whose_key_it_is() {
    // Whose a key is and whether a failed prefix let it go are two
    // facts, not one tag: a macro's released key is still the
    // macro's, so a bell drops it and a person's key beside it
    // stands.
    for origin in ORIGINS {
        let released = Input { press: press(), origin, released: true };
        round_trip(&released);
        let back: Input = decode(&encode(&released)).expect("one input");
        assert_eq!(back.origin, origin, "whose the key is");
        assert!(back.released, "and that it was let go");
        assert_eq!(back.press, press(), "with the press it was taken with");
        let fresh = Input { released: false, ..released };
        let fresh: Input = decode(&encode(&fresh)).expect("one input");
        assert!(!fresh.released, "a key no prefix has let go yet");
    }
}

#[test]
fn an_outcome_of_a_kind_leaves_the_other_fields_at_default() {
    let blank = blank_outcome();
    let edits =
        Outcome { kind: kind::EDITS, edits: vec![edit()], ..blank_outcome() };
    assert_eq!(edits.line, blank.line);
    assert_eq!(edits.open, blank.open);
    assert_eq!(edits.goto, blank.goto);
    assert_eq!(edits.search, blank.search);
    assert_eq!(edits.fed, blank.fed);
    assert_eq!(edits.name, blank.name);
    assert_eq!(edits.flag, blank.flag);
    // `quit`'s `how` is `flag` and `switch`'s `to` is `name`.
    let quit = Outcome { kind: kind::QUIT, flag: true, ..blank_outcome() };
    assert_eq!(quit.edits, blank.edits);
    assert_eq!(quit.name, blank.name);
    let switch = Outcome {
        kind: kind::SWITCH,
        name: "other".to_owned(),
        ..blank_outcome()
    };
    assert_eq!(switch.edits, blank.edits);
    assert_eq!(switch.search, blank.search);
    assert_eq!(switch.flag, blank.flag);
    assert_eq!(Outcome { kind: kind::NOTHING, ..blank_outcome() }, blank);
    assert_eq!(distinct(&KINDS), KINDS.len());
    for kind in KINDS {
        let outcome = Outcome { kind, ..blank_outcome() };
        assert_eq!(outcome.edits, blank.edits);
        assert_eq!(outcome.continuation, 0, "nothing waits yet");
    }
}
