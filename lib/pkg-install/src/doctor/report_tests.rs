//! Tests for [`super::DoctorReport`] and the four finding types.

use std::path::PathBuf;

use super::{DoctorReport, DriftFinding, MissingFinding, OrphanFinding, UnloadableFinding};

fn unloadable() -> UnloadableFinding {
    UnloadableFinding {
        name: "broken".into(),
        path: PathBuf::from("/lib/modules/libreovim_pkg_broken.so"),
        reason: "file too short".into(),
    }
}

fn orphan(filename: &str) -> OrphanFinding {
    OrphanFinding {
        filename: filename.into(),
        path: PathBuf::from(format!("/lib/driver/{filename}")),
    }
}

fn missing() -> MissingFinding {
    MissingFinding {
        name: "gone".into(),
        path: PathBuf::from("/lib/modules/libreovim_pkg_gone.so"),
    }
}

fn drift() -> DriftFinding {
    DriftFinding {
        name: "skewed".into(),
        path: PathBuf::from("/lib/modules/libreovim_pkg_skewed.so"),
        expected: "abcd".into(),
        actual: "ef01".into(),
    }
}

#[test]
fn is_clean_when_all_vecs_empty() {
    let report = DoctorReport::default();
    assert!(report.is_clean());
    assert_eq!(report.total_findings(), 0);
}

#[test]
fn is_clean_false_when_unloadable_populated() {
    let report = DoctorReport {
        unloadable: vec![unloadable()],
        ..DoctorReport::default()
    };
    assert!(!report.is_clean());
}

#[test]
fn is_clean_false_when_orphan_populated() {
    let report = DoctorReport {
        orphan: vec![orphan("libreovim_pkg_ghost.so")],
        ..DoctorReport::default()
    };
    assert!(!report.is_clean());
}

#[test]
fn is_clean_false_when_missing_populated() {
    let report = DoctorReport {
        missing: vec![missing()],
        ..DoctorReport::default()
    };
    assert!(!report.is_clean());
}

#[test]
fn is_clean_false_when_drift_populated() {
    let report = DoctorReport {
        drift: vec![drift()],
        ..DoctorReport::default()
    };
    assert!(!report.is_clean());
}

#[test]
fn total_findings_sums_every_class() {
    let report = DoctorReport {
        unloadable: vec![unloadable()],
        orphan: vec![
            orphan("libreovim_pkg_ghost.so"),
            orphan("libreovim_pkg_phantom.so"),
        ],
        missing: vec![missing()],
        drift: vec![drift()],
    };
    assert_eq!(report.total_findings(), 5);
    assert!(!report.is_clean());
}

#[test]
fn orphan_finding_decodes_package_name_when_filename_matches_convention() {
    let f = orphan(&reovim_dylib_loader::cdylib_filename("my-orphan"));
    assert_eq!(f.package_name().as_deref(), Some("my-orphan"));
}

#[test]
fn orphan_finding_decodes_none_for_foreign_filename() {
    let f = orphan("libsomeoneelse.so");
    assert!(f.package_name().is_none());
}
