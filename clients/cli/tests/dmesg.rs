//! `dmesg`: the exact line of each form over entries the test makes
//! itself, the filters, and the follow over the rig — a second client
//! writes an entry and the watching one prints it.

use reovim_client::log::Entry;
use reovim_client::{Client, Rights};
use reovim_client_cli::dmesg::{Form, Names, Tail, boot, form, show};
use reovim_client_cli::{Out, Paint, Shell, following, input};
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
    show(entries, boot(entries), &mut last, &form, &Names::default())
}

/// One command line over the rig; its output, or the line a fault
/// would print.
fn run(client: &mut Client, command: &str, out: Out) -> String {
    match input(client, &mut Shell::default(), &flags(command), out) {
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
    let plain = show(&entries, BOOT, &mut last, &never, &Names::default());
    assert!(!plain.contains('\x1b'));
}

#[test]
fn the_delta_form_says_the_time_since_the_line_before() {
    let entries = [
        entry(1, 0, "kernel", 6, "first"),
        entry(2, 1_545_690_000, "kernel", 6, "second"),
        entry(3, 1_545_698_000, "kernel", 6, "third"),
    ];
    let third = |line: &str| {
        printed(&entries, line).lines().nth(2).unwrap().to_string()
    };
    // `-d`, as util-linux writes it: the stamp, then the time since the
    // line printed before, six decimals right in twelve.
    assert_eq!(third("-d"), "[    1.545698 <    0.000008>] third");
    assert_eq!(
        third("-d -T"),
        "[Thu Sep 17 12:34:57 2026 <    0.000008>] third"
    );
    // `-H` already says the time since the line before; `-d` adds
    // nothing to it. With no stamp at all the delta stands alone.
    assert_eq!(third("-d -H"), third("-H"));
    assert_eq!(third("-d -H"), "[  +0.000008] third");
    assert_eq!(third("-d -t"), "[<    0.000008>] third");
    // The first line of a run has nothing before it to count from.
    assert!(
        printed(&entries, "-d").starts_with("[    0.000000 <    0.000000>]")
    );
}

#[test]
fn a_number_no_one_can_name_stays_the_number() {
    let call = "call rd=120 api=30d1a3b10a4306ee method=2 reply took=8123";
    let entries = [
        entry(1, 0, "kernel", 7, call),
        entry(2, 1000, "kernel", 7, "op=read caller=3 rd=120 took=1200000"),
        entry(3, 2000, "kernel", 7, "op=write caller=3 rd=120 took=1500000000"),
        entry(4, 3000, "kernel", 7, "op=close caller=3 rd=120 took=812"),
        entry(5, 4000, "fs", 6, "rd=120 is the Module's own word"),
    ];
    // An rd the tree no longer has is `rd:<n>`, which is what the cli
    // takes as a target for it; a hash with no declaration stays the
    // hash, and a method with none to name it stays the number. The
    // keys stay whatever the value becomes. `took=` is the cli's to
    // say in a unit.
    let said = printed(&entries, "-t");
    let lines: Vec<&str> = said.lines().collect();
    assert_eq!(
        lines[0],
        "call rd=rd:120 api=30d1a3b10a4306ee method=2 reply took=8.123\u{b5}s"
    );
    assert_eq!(lines[1], "op=read caller=3 rd=rd:120 took=1.2ms");
    assert_eq!(lines[2], "op=write caller=3 rd=rd:120 took=1.5s");
    assert_eq!(lines[3], "op=close caller=3 rd=rd:120 took=812ns");
    // Only the Kernel writes in numbers; another mark's words stand.
    assert_eq!(lines[4], "rd=120 is the Module's own word");
    // `-r` is the entry as the Kernel wrote it, whatever else is asked.
    assert_eq!(printed(&entries, "-r -T").lines().next().unwrap(), call);
}

#[test]
fn a_kernel_line_says_the_rd_the_api_and_the_method_in_words() {
    let rig = Rig::new("dmesg-decode");
    let mut client = rig.client();
    let out = Out::plain();
    let root = "/module/fs/root";

    // The trace and the `call` pair are measurement, kept at `debug`;
    // the level goes back to `info` so the `dmesg` below adds nothing.
    assert_eq!(run(&mut client, "write /sys/loglevel debug", out), "5\n");
    assert_eq!(run(&mut client, "cat /sys/loglevel", out), "debug");
    let made = run(&mut client, &format!("call {root} create made"), out);
    assert!(made.trim().parse::<u64>().is_ok(), "{made}");
    run(&mut client, "write /sys/loglevel info", out);

    let said = run(&mut client, "dmesg -t -l debug", out);
    let pair: Vec<&str> =
        said.lines().filter(|line| line.starts_with("call ")).collect();
    assert_eq!(pair.len(), 2, "{said}");
    assert_eq!(
        pair[0],
        format!("call rd={root} api=fs method=create dispatch")
    );
    let head = format!("call rd={root} api=fs method=create reply took=");
    let spent = pair[1].strip_prefix(&head).unwrap_or_else(|| panic!("{said}"));
    // What the round trip took and what it cost the heap; `heap=` is
    // bytes already, so `dmesg` leaves it as the Kernel wrote it.
    let (span, bytes) = spent.split_once(" heap=").unwrap();
    // A unit, not the Kernel's bare nanoseconds.
    assert!(span.ends_with('s') && !span.ends_with("=s"), "{said}");
    assert!(span.starts_with(|c: char| c.is_ascii_digit()), "{said}");
    assert!(bytes.starts_with(['+', '-']), "{said}");
    assert!(bytes[1..].parse::<u64>().is_ok(), "{said}");
    // The op the call was, and the ops around it, name the same path.
    let ops = said.lines().filter(|line| line.starts_with("op="));
    let call = ops.clone().find(|line| line.starts_with("op=call ")).unwrap();
    assert!(call.contains(&format!(" rd={root} ")), "{call}");
    assert!(call.contains(" api=fs method=create "), "{call}");
    assert!(ops.clone().any(|line| line.starts_with("op=open ")), "{said}");
    // Nothing the log's own reading did is in it.
    assert!(!said.contains("rd=/sys/log"), "{said}");

    // `-r` is what the Kernel wrote: the numbers it had.
    let rd = client.resolve(root).unwrap();
    let (hash, decl) = client.decls(rd).unwrap().pop().unwrap();
    let raw = run(&mut client, "dmesg -r -l debug", out);
    let number = decl.methods.iter().find(|m| m.name == "create").unwrap();
    let head = format!(
        "call rd={} api={:016x} method={} dispatch",
        rd.0, hash.0, number.number
    );
    assert!(raw.lines().any(|line| line == head), "{raw}");
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
        "\x1b[1;34mclipboard\x1b[0m  \x1b[1;34msysutils\x1b[0m"
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
    let mut plan = following(&mut client, &Shell::default(), &words, out)
        .unwrap()
        .unwrap();
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
    let mut plan =
        following(&mut client, &Shell::default(), &words, Out::plain())
            .unwrap();
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
    let mut plan =
        following(&mut client, &Shell::default(), &words, Out::plain())
            .unwrap();
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
        following(client, &Shell::default(), &flags(line), out)
            .unwrap()
            .is_none()
    };
    assert!(none(&mut client, "dmesg"));
    assert!(none(&mut client, "ls /module"));
    assert!(none(&mut client, "cat /module/fs/root/nope"));
    assert!(none(&mut client, "lookup /"));
    assert!(none(&mut client, ""));

    let words = flags("ls -w /module");
    let plan = following(&mut client, &Shell::default(), &words, out)
        .unwrap()
        .unwrap();
    let listed = String::from_utf8(plan.first).unwrap();
    assert_eq!(listed, "clipboard\ncoreutils\nfs\nsysutils\ntext\n");
    let (form, tail) = form(&flags("-wT"), Paint::off()).unwrap();
    assert!(tail == Tail::All, "-w follows");
    assert!(matches!(form, Form { decode: false, .. }));
}
