use alloc::sync::Arc;
use core::sync::atomic::{AtomicBool, Ordering};
use reovim_kernel_interface::Error;
use std::{sync::mpsc, time::Duration};

struct Discard(mpsc::Sender<()>);
impl Drop for Discard {
    fn drop(&mut self) {
        self.0.send(()).unwrap();
    }
}

#[test]
fn abandoned_capture_wait_retains_work_and_destroys_untaken_result() {
    let busy = Arc::new(AtomicBool::new(false));
    let (started, entered) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let (discarded, disposed) = mpsc::channel();
    let waiting = super::worker::start(&busy, move || {
        started.send(()).unwrap();
        released.recv().unwrap();
        Ok(Discard(discarded))
    });
    entered.recv_timeout(Duration::from_secs(5)).unwrap();
    drop(waiting);
    assert!(busy.load(Ordering::Acquire));
    let executed = Arc::new(AtomicBool::new(false));
    let observed = executed.clone();
    let second = super::worker::start(&busy, move || {
        observed.store(true, Ordering::Release);
        Ok(())
    });
    assert_eq!(super::run(second, |_| Ok(())), Ok(Err(Error::BUSY)));
    assert!(!executed.load(Ordering::Acquire));
    release.send(()).unwrap();
    disposed.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(!busy.load(Ordering::Acquire));
}
