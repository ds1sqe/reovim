//! What a typed line decodes to, held against the record built by
//! hand: the same run of the same Driver in the same rig, so the cell
//! each answers with holds the same bytes. The declaration is the whole
//! of what the shell reads, so this is the rule and nothing else.

use reovim_client::{Client, ROOT, Rd, Rights};
use reovim_client_sh::{Shell, run};
use reovim_module_coreutils::bins::ls;
use reovim_rig::Rig;

/// One typed line from where `shell` stands; its output, or the line a
/// Miss would print.
fn typed(client: &mut Client, shell: &mut Shell, line: &str) -> String {
    match run(client, shell, line) {
        Ok(ran) => String::from_utf8(ran.out).unwrap(),
        Err(miss) => miss.to_string(),
    }
}

/// `call /bin/<name> run <bytes>`, its output cell read at an offset to
/// the empty read that ends it and closed, which is how a shell reads
/// one.
fn called(client: &mut Client, name: &str, input: Vec<u8>) -> String {
    let rd = client.resolve(&format!("/bin/{name}")).unwrap();
    let out = client.call_by_name(rd, "run", input).unwrap();
    let cell: Rd = reovim_sdk::decode(&out).unwrap();
    let h = client.open(cell, Rights::READ).unwrap();
    let mut bytes = Vec::new();
    loop {
        let at = bytes.len() as u64;
        let chunk = client.read(h, at, 64).unwrap();
        if chunk.is_empty() {
            break;
        }
        bytes.extend(chunk);
    }
    client.close(h).unwrap();
    String::from_utf8(bytes).unwrap()
}

#[test]
fn a_line_fills_the_record_the_declaration_asks_for() {
    let rig = Rig::new("sh-decode");
    let mut client = rig.client();
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
    let by_hand = called(&mut client, "ls", reovim_sdk::encode(&args));
    assert_eq!(typed(&mut client, &mut shell, "ls -l -a /bin"), by_hand);
    assert!(by_hand.contains("\x1b["), "color is the shell's to fill");

    // The same two fields in one token under a dash.
    assert_eq!(typed(&mut client, &mut shell, "ls -la /bin"), by_hand);
}
