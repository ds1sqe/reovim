//! Build script for reovim.
//!
//! This script extracts build metadata (git hash, build date) and makes it
//! available to the binary at compile time via environment variables.

use std::process::Command;

fn main() {
    // Tell Cargo to re-run if git HEAD changes (for tracking git hash)
    println!("cargo::rerun-if-changed=../.git/HEAD");
    println!("cargo::rerun-if-changed=../.git/index");

    // Get git commit hash (short form)
    let git_hash = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout).ok()
            } else {
                None
            }
        })
        .map(|s| s.trim().to_string());

    if let Some(hash) = git_hash {
        println!("cargo::rustc-env=REOVIM_GIT_HASH={hash}");
    }

    // Get git commit hash (full form) - #419
    let git_commit = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout).ok()
            } else {
                None
            }
        })
        .map(|s| s.trim().to_string());

    if let Some(commit) = git_commit {
        println!("cargo::rustc-env=REOVIM_GIT_COMMIT={commit}");
    }

    // Get git dirty flag - #419
    // Exit code 0 = clean, non-zero = dirty (has changes)
    let git_dirty = Command::new("git")
        .args(["diff", "--quiet"])
        .status()
        .is_ok_and(|status| !status.success());

    println!("cargo::rustc-env=REOVIM_GIT_DIRTY={git_dirty}");

    // Get build date (ISO 8601 format, date only) - legacy field
    let build_date = Command::new("date")
        .args(["+%Y-%m-%d"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout).ok()
            } else {
                None
            }
        })
        .map(|s| s.trim().to_string());

    if let Some(date) = build_date {
        println!("cargo::rustc-env=REOVIM_BUILD_DATE={date}");
    }

    // Get build timestamp (full ISO 8601 with time) - #419
    let build_timestamp = Command::new("date")
        .args(["+%Y-%m-%dT%H:%M:%S%z"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout).ok()
            } else {
                None
            }
        })
        .map(|s| s.trim().to_string());

    if let Some(timestamp) = build_timestamp {
        println!("cargo::rustc-env=REOVIM_BUILD_TIMESTAMP={timestamp}");
    }

    // Get Rust version
    let rust_version = Command::new("rustc")
        .args(["--version"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout).ok()
            } else {
                None
            }
        })
        .map(|s| {
            // Extract just the version number from "rustc 1.92.0 (..."
            s.split_whitespace().nth(1).unwrap_or("unknown").to_string()
        });

    if let Some(version) = rust_version {
        println!("cargo::rustc-env=REOVIM_RUST_VERSION={version}");
    }

    // Get target triple from environment (set by Cargo)
    // Note: std::env::var works in build.rs, env!() works in main code
    if let Ok(target) = std::env::var("TARGET") {
        println!("cargo::rustc-env=REOVIM_TARGET={target}");
    }
}
