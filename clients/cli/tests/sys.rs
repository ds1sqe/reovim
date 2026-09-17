//! `/sys` from the shell: the parameters read and written as files,
//! the rights a listing shows, the heap, and a core saved in one `cat`.

use reovim_client::Client;
use reovim_client_cli::{Out, Shell, input};
use reovim_rig::Rig;

/// One command line; its output, or the line a fault would print.
fn run(client: &mut Client, command: &str) -> String {
    let words: Vec<String> =
        command.split_whitespace().map(str::to_string).collect();
    let out = Out { width: None, paint: reovim_client_cli::Paint::off() };
    match input(client, &mut Shell::default(), &words, out) {
        Ok(out) => String::from_utf8(out).unwrap(),
        Err(miss) => miss.to_string(),
    }
}

#[test]
fn a_parameter_is_a_file_under_sys_limit() {
    let rig = Rig::new("sys-limit");
    let mut client = rig.client();

    // One value per file, in decimal, as `/proc/sys/kernel/*` holds
    // one; the five are settable, so the listing says `rw`.
    assert_eq!(run(&mut client, "cat /sys/limit/log_ring"), "4096");
    assert_eq!(run(&mut client, "write /sys/limit/log_ring 65536"), "5\n");
    assert_eq!(run(&mut client, "cat /sys/limit/log_ring"), "65536");
    let listed = run(&mut client, "ls -l /sys/limit");
    let rows: Vec<&str> = listed.lines().collect();
    assert_eq!(rows.len(), 5, "{listed}");
    assert!(rows.iter().all(|row| row.starts_with("- rw  ")), "{listed}");
    let names: Vec<&str> =
        rows.iter().filter_map(|row| row.rsplit("  ").next()).collect();
    assert_eq!(
        names,
        [
            "log_ring",
            "pending_per_handle",
            "reentry",
            "reply_window",
            "watch_ring"
        ]
    );

    // A word that is no number, and a number the Kernel cannot hold.
    assert_eq!(
        run(&mut client, "write /sys/limit/reentry x"),
        "write /sys/limit/reentry: invalid"
    );
    assert_eq!(
        run(&mut client, "write /sys/limit/reentry 0"),
        "write /sys/limit/reentry: invalid"
    );
    assert_eq!(run(&mut client, "cat /sys/limit/reentry"), "8");
}

#[test]
fn the_heap_and_the_core_are_read_and_never_written() {
    let rig = Rig::new("sys-core");
    let mut client = rig.client();

    // `ls -l /sys` shows what each of them is: the two the Kernel
    // makes at the read are `r`, the parameters beside them `rw`.
    let listed = run(&mut client, "ls -l /sys");
    let row = |name: &str| {
        listed
            .lines()
            .find(|row| row.ends_with(&format!("  {name}")))
            .unwrap_or_else(|| panic!("no {name}: {listed}"))
            .to_string()
    };
    assert!(row("mem").starts_with("- r-  "), "{listed}");
    assert!(row("core").starts_with("- r-  "), "{listed}");
    assert!(row("limit").starts_with("c r-  "), "{listed}");
    assert!(row("loglevel").starts_with("- rw  "), "{listed}");
    assert_eq!(
        run(&mut client, "write /sys/mem 0"),
        "write /sys/mem: not writable"
    );

    // The heap the host counts, three numbers; it moves as the
    // process takes memory.
    let mem = run(&mut client, "cat /sys/mem");
    assert_eq!(mem.split(' ').count(), 3, "{mem}");
    let live = |text: &str| {
        let number = text.split(' ').next().unwrap();
        number.strip_prefix("live=").unwrap().parse::<u64>().unwrap()
    };
    assert!(live(&mem) > 0, "{mem}");
    // Held past the read, or a release build would never take it.
    let held = std::hint::black_box(vec![7_u8; 4 << 20]);
    let grown = run(&mut client, "cat /sys/mem");
    drop(std::hint::black_box(held));
    assert!(live(&grown) > live(&mem) + (1 << 20), "{mem} then {grown}");

    // A core is everything the Kernel holds, in one read: the
    // parameters first, the heap last, one line per thing between.
    let core = run(&mut client, "cat /sys/core");
    assert!(core.starts_with("limit watch_ring="), "{core}");
    assert!(core.lines().next_back().unwrap().starts_with("live="), "{core}");
    // Its `rd` lines are the tree's rds, as `tree /` counts them.
    let counted = run(&mut client, "tree /");
    let tail = counted.lines().next_back().unwrap();
    let (containers, cells) = tail.split_once(", ").unwrap();
    let number =
        |text: &str| text.split(' ').next().unwrap().parse::<usize>().unwrap();
    let rds = core.lines().filter(|line| line.starts_with("rd rd=")).count();
    // `tree` counts what it drew, which leaves out the root it stood on.
    assert_eq!(rds, number(containers) + number(cells) + 1, "{core}");
}
