//! What a typed line decodes to, held against the record built by
//! hand: the same run of the same Driver in the same rig, so the cell
//! each answers with holds the same bytes. The declaration is the whole
//! of what the shell reads, so this is the rule and nothing else.

use reovim_client_sh::Shell;
use reovim_kernel_interface::{Calls, Rd, Rights};
use reovim_module_coreutils::bins::{ls, stat};
use reovim_rig::Rig;
use reovim_sdk::dynamic::Value;
use reovim_sdk::walk::ROOT;
use reovim_sdk::{cell, ops};

mod common;
use common::typed;

/// `call /bin/<name> run <bytes>`, its output cell read at an offset to
/// the empty read that ends it and closed, which is how a shell reads
/// one.
fn called(ctx: &dyn Calls, name: &str, input: Vec<u8>) -> String {
    let rd = cell::resolve(ctx, ROOT, &format!("/bin/{name}")).unwrap();
    let out = ops::call_by_name(ctx, rd, "run", input).unwrap();
    let cell: Rd = reovim_sdk::decode(&out).unwrap();
    let h = ctx.open(cell, Rights::READ).unwrap();
    let mut bytes = Vec::new();
    loop {
        let at = bytes.len() as u64;
        let chunk = ops::read(ctx, h, at, 64).unwrap();
        if chunk.is_empty() {
            break;
        }
        bytes.extend(chunk);
    }
    ops::close(ctx, h).unwrap();
    String::from_utf8(bytes).unwrap()
}

#[test]
fn a_line_fills_the_record_the_declaration_asks_for() {
    let rig = Rig::new("sh-decode");
    let host = rig.host();
    let ctx = &host.own();
    let mut shell = Shell::default();
    shell.screen(80, true);

    // The record of modules/coreutils/tests/ls.rs, built by hand: the
    // two `bool` fields under their first letters, the one target in
    // the sequence, and the environment from the shell alone.
    let args = ls::Args {
        long: true,
        all: true,
        w: false,
        cwd: ROOT,
        width: 80,
        color: true,
        targets: vec!["/bin".to_string()],
    };
    let by_hand = called(ctx, "ls", reovim_sdk::encode(&args));
    assert_eq!(typed(ctx, &mut shell, "ls -l -a /bin"), by_hand);
    assert!(by_hand.contains("\x1b["), "color is the shell's to fill");

    // The same two fields in one token under a dash.
    assert_eq!(typed(ctx, &mut shell, "ls -la /bin"), by_hand);
}

#[test]
fn a_field_the_host_gave_a_value_for_is_no_token_of_the_line() {
    let rig = Rig::new("sh-env");
    let host = rig.host();
    let ctx = &host.own();
    let mut shell = Shell::default();
    shell.screen(80, true);

    // The shell names no field of anyone's: `width` and `color` are
    // what `screen` put in the environment, and a host with a language
    // of its own puts its own names there beside them. `long` given is
    // `ls -l` with no flag on the line.
    shell.give("long", Value::Bool(true));
    let args = ls::Args {
        long: true,
        all: false,
        w: false,
        cwd: ROOT,
        width: 80,
        color: true,
        targets: vec!["/bin".to_string()],
    };
    let by_hand = called(ctx, "ls", reovim_sdk::encode(&args));
    assert_eq!(typed(ctx, &mut shell, "ls /bin"), by_hand);

    // And the token that used to name it is nobody's, the usage line
    // no longer holding it either.
    assert_eq!(
        typed(ctx, &mut shell, "ls -l /bin"),
        "ls -l: usage: ls [-a] [-w] [targets..]"
    );

    // A value of the wrong type is no environment at all: the field is
    // a flag again and the line reads as it always did.
    shell.give("long", Value::Utf8("yes".to_string()));
    assert_eq!(typed(ctx, &mut shell, "ls -l /bin"), by_hand);
}

#[test]
fn two_dashes_are_the_flags_whole_name() {
    let rig = Rig::new("sh-two-dashes");
    let host = rig.host();
    let ctx = &host.own();
    let mut shell = Shell::default();
    shell.screen(80, true);

    // A `--name` token reads as the field of that whole name, the same
    // field `-name` and its first letter already read.
    let args = ls::Args {
        long: true,
        all: false,
        w: false,
        cwd: ROOT,
        width: 80,
        color: true,
        targets: vec!["/bin".to_string()],
    };
    let by_hand = called(ctx, "ls", reovim_sdk::encode(&args));
    assert_eq!(typed(ctx, &mut shell, "ls --long /bin"), by_hand);

    // `--` before a name no field has is a Miss: two dashes read no
    // letter by letter, even where `-la` would read `l` and `a` as
    // two fields each.
    assert_eq!(
        typed(ctx, &mut shell, "ls --zzz /bin"),
        "ls --zzz: usage: ls [-l] [-a] [-w] [targets..]"
    );
    assert_eq!(
        typed(ctx, &mut shell, "ls --la /bin"),
        "ls --la: usage: ls [-l] [-a] [-w] [targets..]",
        "no letter-by-letter reading under two dashes"
    );

    // One dash under the whole name, and one dash under the first
    // letter, are unchanged.
    assert_eq!(
        typed(ctx, &mut shell, "ls -long /bin"),
        by_hand,
        "one dash, the whole name"
    );
    assert_eq!(
        typed(ctx, &mut shell, "ls -l /bin"),
        by_hand,
        "one dash, the first letter"
    );

    // A lone `--` names no field either: today's Miss, unchanged.
    assert_eq!(
        typed(ctx, &mut shell, "ls --"),
        "ls --: usage: ls [-l] [-a] [-w] [targets..]"
    );
}

/// Two dashes name any declared field whole, not only one shaped like
/// a flag: `stat`'s `target` is an ordinary positional, never a
/// `bool` or a one-letter name, yet `--target` reaches it all the
/// same, proving the rule keyed by no field's own name.
#[test]
fn two_dashes_name_a_positional_field_too() {
    let rig = Rig::new("sh-two-dashes-positional");
    rig.file("a b", b"");
    let host = rig.host();
    let ctx = &host.own();
    let root = cell::resolve(ctx, ROOT, "/module/fs/root").unwrap();
    let mut shell = Shell::at(root, "/".to_string());
    shell.screen(80, true);

    let args = stat::Args { cwd: root, color: true, target: "a b".to_string() };
    let by_hand = called(ctx, "stat", reovim_sdk::encode(&args));
    assert_eq!(typed(ctx, &mut shell, "stat --target \"a b\""), by_hand);

    // A name past the declaration is still a Miss, same as any other.
    assert_eq!(
        typed(ctx, &mut shell, "stat --zzz x"),
        "stat --zzz: usage: stat [target]"
    );
}

/// A field the host of the shell named an option of is filled by its
/// whole name under two dashes and by no token of the line's own. The
/// rule is the host's word and this crate's shape, keyed to no name
/// of anyone's: `stat`'s `target`, named nothing like Vim's own
/// `address`, reads as one the moment a host says so.
#[test]
fn a_field_the_host_named_an_option_of_takes_no_positional() {
    let rig = Rig::new("sh-option");
    rig.file("a b", b"");
    let host = rig.host();
    let ctx = &host.own();
    let root = cell::resolve(ctx, ROOT, "/module/fs/root").unwrap();
    let mut shell = Shell::at(root, "/".to_string());
    shell.screen(80, true);
    shell.option("target");

    let args = stat::Args { cwd: root, color: true, target: "a b".to_string() };
    let by_hand = called(ctx, "stat", reovim_sdk::encode(&args));
    assert_eq!(typed(ctx, &mut shell, "stat --target \"a b\""), by_hand);

    // A positional token reaches it no more, and the form `help`
    // prints spells it under two dashes, there being no other door.
    assert_eq!(
        typed(ctx, &mut shell, "stat a b"),
        "stat a: usage: stat [--target TARGET]"
    );
    assert_eq!(typed(ctx, &mut shell, "help stat"), "stat [--target TARGET]\n");

    // The same line over a shell whose host named no option of it:
    // `target` is the positional it was declared as.
    let mut plain = Shell::at(root, "/".to_string());
    plain.screen(80, true);
    assert_eq!(typed(ctx, &mut plain, "stat a b"), by_hand);
}

/// `stat`'s last positional, `target`, is a `utf8` and its only one:
/// a blank inside the name it names does not cut it into two.
#[test]
fn a_last_utf8_positional_takes_the_rest_of_the_line() {
    let rig = Rig::new("sh-verbatim");
    rig.file("a b", b"");
    let host = rig.host();
    let ctx = &host.own();
    let root = cell::resolve(ctx, ROOT, "/module/fs/root").unwrap();
    let mut shell = Shell::at(root, "/".to_string());
    shell.screen(80, true);

    let args = stat::Args { cwd: root, color: true, target: "a b".to_string() };
    let by_hand = called(ctx, "stat", reovim_sdk::encode(&args));
    assert_eq!(typed(ctx, &mut shell, "stat a b"), by_hand);
    assert!(by_hand.contains("a b"), "the whole name, not half of it");

    // A name that does not exist is `stat`'s own refusal, the whole
    // three-word target named in it and not split in three.
    assert_eq!(
        typed(ctx, &mut shell, "stat no such name"),
        "stat no such name: gone"
    );
}

/// `ls`'s last positional, `targets`, is a sequence and not a `utf8`:
/// each blank-separated token is still its own target, and `|` still
/// cuts a line at a bare one, unclaimed by any field.
#[test]
fn a_sequence_still_splits_and_pipes() {
    let rig = Rig::new("sh-sequence");
    let host = rig.host();
    let ctx = &host.own();
    let mut shell = Shell::default();
    shell.screen(80, true);

    let args = ls::Args {
        long: false,
        all: false,
        w: false,
        cwd: ROOT,
        width: 80,
        color: true,
        targets: vec!["/bin".to_string(), "/module".to_string()],
    };
    let by_hand = called(ctx, "ls", reovim_sdk::encode(&args));
    assert_eq!(typed(ctx, &mut shell, "ls /bin /module"), by_hand);
    assert_eq!(typed(ctx, &mut shell, "ls /module | grep text"), "text\n");
}
