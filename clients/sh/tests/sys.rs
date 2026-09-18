//! `/sys` from the shell: the parameters read as files, the facts
//! beside them, the rights a listing shows, and a core read in one
//! `cat`. What a write of a parameter does is
//! kernel/runtime/tests/sys.rs's.

use reovim_client_sh::{Shell, run};
use reovim_kernel_interface::Calls;
use reovim_rig::Rig;

/// One typed line; its output, or the line a Miss would print.
fn typed(ctx: &dyn Calls, shell: &mut Shell, line: &str) -> String {
    match run(ctx, shell, line) {
        Ok(ran) => String::from_utf8(ran.out).unwrap(),
        Err(miss) => miss.to_string(),
    }
}

#[test]
fn a_parameter_is_a_file_under_sys_limit() {
    let rig = Rig::new("sh-sys-limit");
    let host = rig.host();
    let ctx = &host.own();
    let mut shell = Shell::default();
    let mut go = |ctx: &dyn Calls, line: &str| typed(ctx, &mut shell, line);

    // One value per file, in decimal, as `/proc/sys/kernel/*` holds one.
    assert_eq!(go(ctx, "cat /sys/limit/log_ring"), "4096");
    assert_eq!(go(ctx, "cat /sys/limit/reentry"), "8");

    // The five are settable, so the listing says `rw`.
    let listed = go(ctx, "ls -l /sys/limit");
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
}

#[test]
fn a_fact_is_a_file_under_sys_info() {
    let rig = Rig::new("sh-sys-info");
    let host = rig.host();
    let ctx = &host.own();
    let mut shell = Shell::default();
    let mut go = |ctx: &dyn Calls, line: &str| typed(ctx, &mut shell, line);

    // The five are made at the read, so the listing says `r` of each.
    let listed = go(ctx, "ls -l /sys/info");
    let rows: Vec<&str> = listed.lines().collect();
    assert_eq!(rows.len(), 5, "{listed}");
    assert!(rows.iter().all(|row| row.starts_with("- r-  ")), "{listed}");
    let names: Vec<&str> =
        rows.iter().filter_map(|row| row.rsplit("  ").next()).collect();
    assert_eq!(names, ["mem", "time", "uptime", "version", "watchers"]);

    // One fact, one path: the heap does not stand loose under `/sys`
    // beside the Container its kin are in.
    assert_eq!(go(ctx, "ls /sys"), "core\ndecls\ninfo\nlimit\nlog\nloglevel\n");
    assert_eq!(go(ctx, "cat /sys/mem"), "cat /sys/mem: invalid");
}

#[test]
fn the_heap_and_the_core_are_read_and_never_written() {
    let rig = Rig::new("sh-sys-core");
    let host = rig.host();
    let ctx = &host.own();
    let mut shell = Shell::default();
    let mut go = |ctx: &dyn Calls, line: &str| typed(ctx, &mut shell, line);

    // `ls -l /sys` shows what each of them is: the core the Kernel
    // makes at the read is `r`, the facts stand in a Container of
    // their own, the parameters beside them are `rw`.
    let listed = go(ctx, "ls -l /sys");
    let row = |name: &str| {
        listed
            .lines()
            .find(|row| row.ends_with(&format!("  {name}")))
            .unwrap_or_else(|| panic!("no {name}: {listed}"))
            .to_string()
    };
    assert!(row("core").starts_with("- r-  "), "{listed}");
    assert!(row("info").starts_with("c r-  "), "{listed}");
    assert!(row("limit").starts_with("c r-  "), "{listed}");
    assert!(row("loglevel").starts_with("- rw  "), "{listed}");
    // A redirect is the only way bytes go into an rd, and the heap
    // takes none: the shell says the table's sentence for what it was
    // refused with.
    assert_eq!(go(ctx, "pwd"), "/\n");
    assert_eq!(go(ctx, "ls / > /sys/info/mem"), "ls /: denied");

    // The heap the host counts, three numbers; it moves as the
    // process takes memory.
    let mem = go(ctx, "cat /sys/info/mem");
    assert_eq!(mem.split(' ').count(), 3, "{mem}");
    let live = |text: &str| {
        let number = text.split(' ').next().unwrap();
        number.strip_prefix("live=").unwrap().parse::<u64>().unwrap()
    };
    assert!(live(&mem) > 0, "{mem}");
    // Held past the read, or a release build would never take it.
    let held = std::hint::black_box(vec![7_u8; 4 << 20]);
    let grown = go(ctx, "cat /sys/info/mem");
    drop(std::hint::black_box(held));
    assert!(live(&grown) > live(&mem) + (1 << 20), "{mem} then {grown}");

    // A core is everything the Kernel holds, in one read: the
    // parameters first, the heap last, one line per thing between.
    let core = go(ctx, "cat /sys/core");
    assert!(core.starts_with("limit watch_ring="), "{core}");
    assert!(core.lines().next_back().unwrap().starts_with("live="), "{core}");
    // Its `rd` lines are the tree's rds, as `tree /` counts them.
    let counted = go(ctx, "tree /");
    let tail = counted.lines().next_back().unwrap();
    let (containers, cells) = tail.split_once(", ").unwrap();
    let number =
        |text: &str| text.split(' ').next().unwrap().parse::<usize>().unwrap();
    let rds = core.lines().filter(|line| line.starts_with("rd rd=")).count();
    // `tree` counts what it drew, which leaves out the root it stood on.
    assert_eq!(rds, number(containers) + number(cells) + 1, "{core}");
}
