//! `pkg doctor`: audit the library root and lockfile inventory for
//! the four fault classes (unloadable, orphan, missing, drift); with
//! `--fix`, auto-remediate orphans + missing entries.

use {anyhow::Result, reovim_pkg_install::DoctorReport};

use crate::cli::DoctorArgs;

/// Exit signal from a doctor run. The CLI dispatch maps `Clean` to
/// exit 0 and `Findings` to exit 1; errors map to exit 1 too.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoctorExit {
    /// No fault classes detected (or all auto-fixed under `--fix`).
    Clean,
    /// At least one finding remains in the report.
    Findings,
}

pub fn run(args: &DoctorArgs) -> Result<DoctorExit> {
    let report = reovim_pkg_install::audit(&args.library_root, &args.lockfile)?;
    let final_report = if args.fix && !report.is_clean() {
        let outcome = reovim_pkg_install::repair(&report, &args.library_root, &args.lockfile)?;
        for path in &outcome.removed_orphans {
            println!("removed orphan: {}", path.display());
        }
        for name in &outcome.dropped_missing {
            println!("dropped stale inventory entry: {name}");
        }
        outcome.residual
    } else {
        report
    };
    print_report(&final_report);
    Ok(if final_report.is_clean() {
        DoctorExit::Clean
    } else {
        DoctorExit::Findings
    })
}

fn print_report(report: &DoctorReport) {
    for f in &report.unloadable {
        println!("unloadable {} {} {}", f.name, f.path.display(), f.reason);
    }
    for f in &report.orphan {
        println!("orphan {} {}", f.filename, f.path.display());
    }
    for f in &report.missing {
        println!("missing {} {}", f.name, f.path.display());
    }
    for f in &report.drift {
        println!(
            "drift {} {} expected={} actual={}",
            f.name,
            f.path.display(),
            f.expected.get(..8).unwrap_or(&f.expected),
            f.actual.get(..8).unwrap_or(&f.actual),
        );
    }
}
