//! `Progress`: its bytes, and `eta`'s arithmetic against a fixed
//! `Info`, no clock read.

use reovim_kernel_interface::{Flags, Info};
use reovim_sdk::progress::{self, Progress};

/// `info` with `ctime` and `mtime` set and nothing else that `eta`
/// reads.
const fn info(ctime: u64, mtime: u64) -> Info {
    Info {
        name: None,
        flags: Flags::empty(),
        ctime,
        mtime,
        atime: 0,
        apis: Vec::new(),
    }
}

#[test]
fn eta_is_the_time_spent_scaled_by_what_is_left() {
    let spent = info(1000, 3000);
    // 2000 spent on 25 of 100: 75 left at 80 a unit is 6000.
    let going = Progress { done: 25, total: 100 };
    assert_eq!(progress::eta(&spent, &going), Some(6000));

    assert_eq!(progress::eta(&spent, &Progress { done: 0, total: 100 }), None);
    assert_eq!(progress::eta(&spent, &Progress { done: 25, total: 0 }), None);
    assert_eq!(progress::eta(&spent, &Progress { done: 0, total: 0 }), None);

    let whole = Progress { done: 100, total: 100 };
    assert_eq!(progress::eta(&spent, &whole), Some(0));
}

#[test]
fn eta_clamps_past_u64_and_done_past_total_is_no_time_left() {
    // `spent * left` overflows u64 comfortably inside u128; the
    // result past u64::MAX clamps rather than wrapping.
    let spent = info(0, u64::MAX);
    let huge = Progress { done: 1, total: u64::MAX };
    assert_eq!(progress::eta(&spent, &huge), Some(u64::MAX));

    // `done` past `total`: `total - done` saturates to 0, so `eta` is
    // 0 rather than underflowing.
    let over = Progress { done: 10, total: 5 };
    assert_eq!(progress::eta(&spent, &over), Some(0));
}

#[test]
fn a_progress_is_sixteen_bytes() {
    let bytes = reovim_sdk::encode(&Progress { done: 25, total: 100 });
    assert_eq!(bytes.len(), 16);
    assert_eq!(
        reovim_sdk::decode::<Progress>(&bytes),
        Ok(Progress { done: 25, total: 100 })
    );
}
