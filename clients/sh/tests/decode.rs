//! What a typed line decodes to, held against the record built by
//! hand: the same run of the same Driver in the same rig, so the cell
//! each answers with holds the same bytes. The declaration is the whole
//! of what the shell reads, so this is the rule and nothing else.

use reovim_client_sh::{Shell, run};
use reovim_kernel_interface::{Calls, Rd, Rights};
use reovim_module_coreutils::bins::ls;
use reovim_rig::Rig;
use reovim_sdk::dynamic::Value;
use reovim_sdk::walk::ROOT;
use reovim_sdk::{cell, ops};

/// One typed line from where `shell` stands; its output, or the line a
/// Miss would print.
fn typed(ctx: &dyn Calls, shell: &mut Shell, line: &str) -> String {
    match run(ctx, shell, line) {
        Ok(ran) => String::from_utf8(ran.out).unwrap(),
        Err(miss) => miss.to_string(),
    }
}

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
