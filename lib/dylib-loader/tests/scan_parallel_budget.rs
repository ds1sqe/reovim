//! Cold + warm scan-budget benchmark for 100 cdylibs.
//!
//! Asserts the master-plan budget (§Phase-1 ACs): **≤ 600 ms cold,
//! ≤ 150 ms warm** for 100 copies of the Phase 0 `PoC` cdylib scanned
//! in parallel by `rayon`. Gated behind `#[ignore]` so the normal
//! `cargo test -p reovim-dylib-loader` run stays fast; the dedicated
//! CI job in Phase 1.F (and the local contributor command
//! `cargo test -p reovim-dylib-loader --test scan_parallel_budget
//! -- --ignored`) runs it explicitly.
//!
//! The benchmark seeds a `tempfile::tempdir` with 100 symlinks to the
//! same `PoC` cdylib (so the filesystem cost is one inode's worth of
//! data; the measurement is dominated by `dlopen` plus filesystem
//! enumeration). A cold pass deliberately drops the rayon thread
//! pool's warm caches — by running in a freshly spawned pool — and
//! the warm pass reuses the same pool.
//!
//! If the budget fails, the plan's escalation path is:
//! record the measurement in the flight-log landing entry, file
//! `02b-manifest-cache.md` deferral, call `/houston`. Do NOT weaken
//! the budget.

#![allow(unsafe_code)]

use {
    reovim_dylib_loader::{library_filename, scan_paths},
    std::{env, path::PathBuf, time::Instant},
};

const COUNT: usize = 100;
const COLD_BUDGET_MS: u128 = 600;
const WARM_BUDGET_MS: u128 = 150;

fn poc_cdylib_source() -> PathBuf {
    let manifest = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let workspace = PathBuf::from(&manifest)
        .ancestors()
        .nth(2)
        .expect("workspace root")
        .to_path_buf();
    let target = env::var("CARGO_TARGET_DIR")
        .map_or_else(|_| workspace.join("target"), PathBuf::from);
    target.join("debug").join(library_filename("reovim_driver_abi_poc"))
}

#[test]
#[ignore = "benchmark — opt in with `-- --ignored` or the Phase 1.F CI job"]
fn scan_100_cdylibs_cold_and_warm_are_within_budget() {
    let src = poc_cdylib_source();
    assert!(
        src.exists(),
        "`PoC` cdylib not found at {}; run `cargo build -p reovim-driver-abi-poc` first",
        src.display()
    );

    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().to_path_buf();
    for i in 0..COUNT {
        let dest = root.join(library_filename(&format!("poc_{i:03}")));
        #[cfg(unix)]
        std::os::unix::fs::symlink(&src, &dest).expect("symlink");
        #[cfg(not(unix))]
        std::fs::copy(&src, &dest).expect("copy");
    }

    // Cold pass — the rayon global pool is fresh for this test binary,
    // and the kernel page cache for this tempdir has never seen it.
    let cold_start = Instant::now();
    let cold = scan_paths(std::slice::from_ref(&root));
    let cold_elapsed = cold_start.elapsed();
    assert_eq!(
        cold.len(),
        COUNT,
        "cold pass found {} entries, expected {COUNT}",
        cold.len()
    );
    let cold_oks = cold.entries().iter().filter(|e| e.outcome.is_ok()).count();
    assert_eq!(cold_oks, COUNT, "cold pass had {cold_oks} successes, expected {COUNT}");

    // Warm pass — kernel caches are hot, rayon pool is warm.
    let warm_start = Instant::now();
    let warm = scan_paths(&[root]);
    let warm_elapsed = warm_start.elapsed();
    assert_eq!(warm.len(), COUNT);

    let cold_ms = cold_elapsed.as_millis();
    let warm_ms = warm_elapsed.as_millis();
    eprintln!("scan_100_cdylibs cold={cold_ms} ms warm={warm_ms} ms");

    assert!(
        cold_ms <= COLD_BUDGET_MS,
        "cold scan exceeded budget: {cold_ms} ms > {COLD_BUDGET_MS} ms \
         (master-plan §Phase-1 AC #7). File Phase 1.5 manifest-cache \
         deferral + /houston before landing; do NOT weaken the budget."
    );
    assert!(
        warm_ms <= WARM_BUDGET_MS,
        "warm scan exceeded budget: {warm_ms} ms > {WARM_BUDGET_MS} ms \
         (master-plan §Phase-1 AC #7)."
    );
}
