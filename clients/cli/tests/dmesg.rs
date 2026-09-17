//! `dmesg`: the exact line of each form over entries the test makes
//! itself, the filters, and the follow over the rig — a second client
//! writes an entry and the watching one prints it.

use reovim_client::log::Entry;
use reovim_client::{Client, Rights};
use reovim_client_cli::dmesg::{Form, Tail, boot, form, show};
use reovim_client_cli::{Out, Paint, following, input};
use reovim_rig::Rig;

/// 2026-09-17 12:34:56 UTC, where the Kernel's first entry stands.
const BOOT: u64 = 1_789_648_496_000_000_000;

/// One entry `after` nanoseconds past the boot.
fn entry(seq: u64, after: u64, mark: &str, level: u8, text: &str) -> Entry {
    Entry {
        seq,
        time: BOOT + after,
        mark: mark.to_string(),
        level,
        bytes: text.as_bytes().to_vec(),
    }
}

/// The flags as a line types them.
fn flags(line: &str) -> Vec<String> {
    line.split_whitespace().map(str::to_string).collect()
}

/// What `dmesg FLAGS` prints of `entries`, from the first one's time.
fn printed(entries: &[Entry], line: &str) -> String {
    let (form, _) = form(&flags(line), Paint::off()).unwrap();
    let mut last = None;
    show(entries, boot(entries), &mut last, &form)
}

/// One command line over the rig; its output, or the line a fault
/// would print.
fn run(client: &mut Client, command: &str, out: Out) -> String {
    match input(client, &flags(command), out) {
        Ok(bytes) => String::from_utf8(bytes).unwrap(),
        Err(miss) => miss.to_string(),
    }
}

#[test]
fn every_form_writes_its_own_stamp() {
    let entries = [
        entry(1, 0, "kernel", 6, "load module=fs"),
        entry(2, 12_345_678_000, "kernel", 6, "connect session=1 client=2"),
    ];
    let second = |text: &str| text.lines().nth(1).unwrap().to_string();
    let text = "connect session=1 client=2";

    // The default: seconds since the first entry, six decimals, right
    // in twelve, as util-linux prints `[%5lu.%06lu]`.
    assert_eq!(
        printed(&entries, "").lines().next().unwrap(),
        "[    0.000000] load module=fs"
    );
    assert_eq!(
        second(&printed(&entries, "")),
        format!("[   12.345678] {text}")
    );
    // `-t` drops the stamp; `-T` is the wall clock of the Kernel.
    assert_eq!(second(&printed(&entries, "-t")), text);
    assert_eq!(
        second(&printed(&entries, "-T")),
        format!("[Thu Sep 17 12:35:08 2026] {text}")
    );
    // `-x` decodes the mark and the level in front of the stamp.
    assert_eq!(
        second(&printed(&entries, "-x")),
        format!("kernel:info  : [   12.345678] {text}")
    );
    // Flags combine, in any order.
    assert_eq!(printed(&entries, "-xt"), printed(&entries, "-tx"));
    assert_eq!(
        second(&printed(&entries, "-tx")),
        format!("kernel:info  : {text}")
    );
}

#[test]
fn the_human_form_says_the_minute_once() {
    let entries = [
        entry(1, 0, "kernel", 6, "first"),
        entry(2, 12_345_000, "kernel", 6, "within"),
        entry(3, 61_000_000_000, "kernel", 6, "later"),
    ];
    let said = printed(&entries, "-H");
    assert_eq!(
        said,
        "[Sep17 12:34] first\n\
         [  +0.012345] within\n\
         [Sep17 12:35] later\n"
    );
}

#[test]
fn the_filters_choose_the_marks_and_the_levels() {
    let entries = [
        entry(1, 0, "kernel", 6, "load module=fs"),
        entry(2, 1_000_000, "caller", 3, "boom"),
        entry(3, 2_000_000, "fs", 4, "slow"),
    ];
    let texts = |line: &str| {
        let said = printed(&entries, line);
        said.lines().map(ToString::to_string).collect::<Vec<_>>()
    };
    assert_eq!(texts("-tk"), ["load module=fs"]);
    assert_eq!(texts("-t -f caller,fs"), ["boom", "slow"]);
    assert_eq!(texts("-t -l err"), ["boom"]);
    assert_eq!(texts("-t -l err,warn"), ["boom", "slow"]);
    assert_eq!(texts("-t -l info -f kernel"), ["load module=fs"]);

    // A level that is not one of the eight, and a flag dmesg has not.
    let bad = form(&flags("-l nope"), Paint::off()).unwrap_err();
    assert_eq!(
        bad.to_string(),
        "dmesg: no such level nope; they are: \
         emerg, alert, crit, err, warn, notice, info, debug"
    );
    let flag = form(&flags("-z"), Paint::off()).unwrap_err();
    assert!(flag.to_string().contains("usage: dmesg"), "{flag}");
}

#[test]
fn color_paints_the_stamp_and_what_is_urgent() {
    let entries = [
        entry(1, 0, "kernel", 6, "load module=fs"),
        entry(2, 1_000_000, "caller", 3, "boom"),
        entry(3, 2_000_000, "fs", 4, "slow"),
        entry(4, 3_000_000, "fs", 2, "worse"),
    ];
    // `-L` is `--color=always`, so a pipe gets color when it asks.
    let said = printed(&entries, "-L");
    let lines: Vec<&str> = said.lines().collect();
    assert_eq!(lines[0], "\x1b[32m[    0.000000]\x1b[0m load module=fs");
    assert_eq!(lines[1], "\x1b[32m[    0.001000]\x1b[0m \x1b[31mboom\x1b[0m");
    assert_eq!(lines[2], "\x1b[32m[    0.002000]\x1b[0m \x1b[1mslow\x1b[0m");
    assert_eq!(
        lines[3],
        "\x1b[32m[    0.003000]\x1b[0m \x1b[1;31mworse\x1b[0m"
    );
    // The `-H` minute break is bold blue and the `-x` decode dim.
    let human = printed(&entries, "-HxL");
    let first = human.lines().next().unwrap();
    assert_eq!(
        first,
        "\x1b[2mkernel:info  : \x1b[0m\x1b[1;34m[Sep17 12:34]\x1b[0m \
         load module=fs"
    );
    // `--color=never` takes it away again, whatever the app allowed.
    let (never, _) = form(&flags("--color=never"), Paint::on()).unwrap();
    let mut last = None;
    assert!(!show(&entries, BOOT, &mut last, &never).contains('\x1b'));
}

#[test]
fn the_words_of_a_listing_wear_their_kind() {
    let rig = Rig::new("dmesg-color");
    rig.file("note", b"hi\n");
    let mut client = rig.client();
    let out = Out { width: None, paint: Paint::on() };

    // A Container bold blue, a cell that offers an api green, a plain
    // cell as it is.
    let listed = run(&mut client, "ls /module", out);
    assert!(listed.contains("\x1b[1;34mfs\x1b[0m\n"), "{listed}");
    let open = run(&mut client, "ls /module/text", out);
    assert!(open.contains("\x1b[32mopen\x1b[0m\n"), "{open}");
    let note = run(&mut client, "ls /module/fs/root", out);
    assert_eq!(note, "note\n");
    // The columns count the name, not the sequences around it.
    let wide = Out { width: Some(20), paint: Paint::on() };
    let two = run(&mut client, "ls /module", wide);
    assert_eq!(
        two.lines().next().unwrap(),
        "\x1b[1;34mclipboard\x1b[0m  \x1b[1;34mfs\x1b[0m  \x1b[1;34mtext\x1b[0m"
    );
}

#[test]
fn a_follow_prints_the_entry_another_client_writes() {
    let rig = Rig::new("dmesg-follow");
    let mut client = rig.client();
    let mut other = rig.client();
    let out = Out::plain();

    // `-W` takes only what comes after it; the err entry the other
    // client writes with printk's `<3>` in front.
    let words = flags("dmesg -W -t -l err");
    let mut plan = following(&mut client, &words, out).unwrap().unwrap();
    assert_eq!(plan.first, Vec::<u8>::new());
    let log = other.resolve("/sys/log").unwrap();
    other.write_all(log, b"<3>boom".to_vec()).unwrap();

    let mut said = Vec::new();
    for event in client.since().unwrap() {
        if let Some(bytes) = plan.event(&mut client, event).unwrap() {
            said.extend(bytes);
        }
    }
    assert_eq!(String::from_utf8(said).unwrap(), "boom\n");
    // What the log already held is what `dmesg` prints without `-w`.
    let now = run(&mut client, "dmesg -t -l err", out);
    assert_eq!(now, "boom\n");
    assert_eq!(run(&mut client, "dmesg -t -l emerg", out), "");
    assert!(run(&mut client, "dmesg", out).starts_with("[    0.000000] "));
}

#[test]
fn watch_says_what_happened_under_a_container() {
    let rig = Rig::new("watch");
    let mut client = rig.client();
    let mut other = rig.client();
    let root = "/module/fs/root";

    let words = flags(&format!("watch {root}"));
    let mut plan = following(&mut client, &words, Out::plain()).unwrap();
    let mut plan = plan.take().unwrap();
    // `input` prints nothing for it: the loop is the app's.
    assert_eq!(run(&mut client, &format!("watch {root}"), Out::plain()), "");

    let made = format!("call {root} create made");
    assert_ne!(run(&mut other, &made, Out::plain()), "");
    let mut said = String::new();
    for event in client.since().unwrap() {
        if let Some(bytes) = plan.event(&mut client, event).unwrap() {
            said.push_str(&String::from_utf8(bytes).unwrap());
        }
    }
    // What happened after the watch began, and nothing the ring held
    // before it: the Create of the new child, with its own seq.
    let line = said.lines().find(|l| l.contains("Create")).unwrap();
    let words: Vec<&str> = line.split(' ').collect();
    assert_eq!((words[1], words[2]), ("Create", "made"), "{said}");
    assert!(words[0].parse::<u64>().is_ok(), "{said}");
    assert!(!said.contains("Attrib"), "the backlog is not ours: {said}");
}

#[test]
fn cat_follows_a_cell_and_stops_after_the_count() {
    let rig = Rig::new("cat-follow");
    rig.file("note", b"first\n");
    let mut client = rig.client();
    let mut other = rig.client();
    let target = "/module/fs/root/note";

    let words = flags(&format!("cat -f -n 1 {target}"));
    let mut plan = following(&mut client, &words, Out::plain()).unwrap();
    let mut plan = plan.take().unwrap();
    assert_eq!(plan.first, b"first\n".to_vec());
    assert_eq!(plan.limit, Some(1));

    let rd = other.resolve(target).unwrap();
    let h = other.open(rd, Rights::WRITE).unwrap();
    other.write(h, 0, b"second\n".to_vec()).unwrap();
    other.close(h).unwrap();
    let mut said = Vec::new();
    for event in client.since().unwrap() {
        if let Some(bytes) = plan.event(&mut client, event).unwrap() {
            said.extend(bytes);
        }
    }
    assert_eq!(String::from_utf8(said).unwrap(), "second\n");
}

/// `-w` is what tells the app to follow; without it a word is one
/// command, and `ls -w` watches what it listed.
#[test]
fn only_a_following_word_asks_for_the_loop() {
    let rig = Rig::new("following");
    let mut client = rig.client();
    let out = Out::plain();
    let none = |client: &mut Client, line: &str| {
        following(client, &flags(line), out).unwrap().is_none()
    };
    assert!(none(&mut client, "dmesg"));
    assert!(none(&mut client, "ls /module"));
    assert!(none(&mut client, "cat /module/fs/root/nope"));
    assert!(none(&mut client, "lookup /"));
    assert!(none(&mut client, ""));

    let words = flags("ls -w /module");
    let plan = following(&mut client, &words, out).unwrap().unwrap();
    let listed = String::from_utf8(plan.first).unwrap();
    assert_eq!(listed, "clipboard\nfs\ntext\n");
    let (form, tail) = form(&flags("-wT"), Paint::off()).unwrap();
    assert!(tail == Tail::All, "-w follows");
    assert!(matches!(form, Form { decode: false, .. }));
}
