//! The resolver's records, from a consumer linking this api and the
//! sdk and nothing of any Plugin: the five records through the public
//! `Encode`/`Decode`, the continuation pairing a wait with the reply
//! that answers it, a refusal's error, the `Press` a synthetic key
//! carries, the number a mode is, and the declaration's own identity
//! over the records it names.

use core::fmt::Debug;

use reovim_client_api::keyboard::Press;
use reovim_client_api::resolver;
use reovim_client_api::resolver::{Command, Fed, Input, Reply, State};
use reovim_client_api::{NONE, mode, origin, reply};
use reovim_sdk::{Decode, Encode, decode, encode};

/// Whose a key is: every origin an `Input` may carry, which a
/// release keeps rather than trading away.
const ORIGINS: [u64; 3] = [origin::TYPED, origin::FED, origin::REPEAT];

/// Every mode a `State` may be in, in the order the numbers stand.
const MODES: [u64; 6] = [
    mode::NORMAL,
    mode::INSERT,
    mode::COMMAND,
    mode::VISUAL,
    mode::VISUAL_LINE,
    mode::VISUAL_BLOCK,
];

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

/// A `Reply` at rest: it answers no continuation and refuses nothing.
const fn blank_reply() -> Reply {
    Reply {
        continuation: 0,
        kind: reply::PASTED,
        text: String::new(),
        error: 0,
    }
}

/// A `State` at rest: the mode a resolver opens in, nothing typed,
/// nothing waited for.
const fn blank_state() -> State {
    State { mode: mode::NORMAL, showcmd: String::new(), waiting: 0 }
}

#[test]
fn every_record_round_trips() {
    round_trip(&Input {
        press: press(),
        origin: origin::TYPED,
        released: false,
    });
    round_trip(&Fed { keys: vec![0x61, 0x62], repeat: false });
    round_trip(&Command { line: "write --address 1,9".to_owned() });
    round_trip(&Reply {
        continuation: 7,
        kind: reply::PASTED,
        text: "yanked".to_owned(),
        ..blank_reply()
    });
    round_trip(&State {
        mode: mode::INSERT,
        showcmd: "3d".to_owned(),
        waiting: 7,
    });
    // Five records, and none of them the text: what resolving needs
    // in and out, and nothing of what a resolver drives.
    assert_eq!(distinct(&[reply::PASTED, reply::REFUSED]), 2);
}

#[test]
fn a_continuation_pairs_a_reply_with_its_wait() {
    // `State.waiting` is the continuation still waited for, 0 for
    // none: a reply the state no longer names is a stale one.
    let waiting = State { waiting: 91, ..blank_state() };
    let answer = Reply {
        continuation: waiting.waiting,
        kind: reply::PASTED,
        text: "yanked".to_owned(),
        ..blank_reply()
    };
    let waiting: State = decode(&encode(&waiting)).expect("one state");
    let answer: Reply = decode(&encode(&answer)).expect("one reply");
    assert_eq!(answer.continuation, waiting.waiting);
    assert_eq!(answer.text, "yanked");
    assert_ne!(blank_state().waiting, answer.continuation, "nothing waits");
    assert_eq!(blank_reply().continuation, 0, "and nothing answers");
}

#[test]
fn a_refused_reply_carries_its_error() {
    // `error` is the `Error` as the Kernel numbers it on the wire,
    // and is read only where `kind` is `REFUSED`.
    let stale = 3;
    let refused = Reply {
        continuation: 12,
        kind: reply::REFUSED,
        text: "edit".to_owned(),
        error: stale,
    };
    let back: Reply = decode(&encode(&refused)).expect("one reply");
    assert_eq!(back.kind, reply::REFUSED);
    assert_eq!(back.error, stale);
    assert_eq!(back.text, "edit", "the operation the refusal is of");
    assert_eq!(blank_reply().error, 0, "an answer that refuses nothing");
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
fn a_mode_is_the_apis_number() {
    // The one field a consumer that draws a frame cannot avoid: the
    // numbers are the api's, here, and not a resolver's own, so a
    // reader that is not Rust has them from the declaration and a
    // resolver spells its own enum out of them.
    assert_eq!(MODES, [0, 1, 2, 3, 4, 5], "the numbering a State spells");
    assert_eq!(distinct(&MODES), MODES.len(), "one number each");
    assert_eq!(blank_state().mode, mode::NORMAL, "the mode one opens in");
    for mode in MODES {
        let state = State { mode, ..blank_state() };
        let back: State = decode(&encode(&state)).expect("one state");
        assert_eq!(back.mode, mode);
        assert_eq!(back.showcmd, "", "nothing typed yet");
    }
}

#[test]
fn a_record_only_api_hashes_its_records() {
    // `api resolver ver 1 {}`: no method, five records. Its name and
    // its version are all a hash over the methods can see, so a
    // reshaped record would be the same declaration to the Kernel's
    // identity check, on both sides of the door.
    let decl = resolver::decl();
    assert!(decl.methods.is_empty(), "the resolver declares no method");
    let named: Vec<&str> =
        decl.records.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(named, ["Input", "Fed", "Command", "Reply", "State"]);

    let fields: Vec<&str> =
        decl.records[0].1.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(fields, ["press", "origin", "released"], "Input's own");

    assert_eq!(resolver::hash(), decl.hash(), "two builds of one api");

    let mut renamed = decl.clone();
    renamed.records[0].1[1].name = String::from("whose");
    assert_ne!(renamed.hash(), decl.hash(), "one field renamed");

    let mut dropped = decl.clone();
    dropped.records.pop();
    assert_ne!(dropped.hash(), decl.hash(), "one record gone");
}
