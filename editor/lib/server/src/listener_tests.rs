//! Tests for `listener.rs` — `start_listener` path validation.
//!
//! Registered under the `selftest` feature; runs on the arch no_std
//! selftest runner (arch_test! + testrt::run). The server-selftest bin in
//! tests/fixtures/ runs these.
//!
//! Full integration smoke (bind → accept → Hello→Attach→SendInput) lives in
//! the server-selftest fixture bin rather than inline here, because it
//! requires an active kernel and UDS connection.

use {
    reovim_arch::arch_test,
    reovim_uapi::{
        net::NetControl,
        sched::{DetachedThreadSpawner, SpawnError},
    },
};

use crate::{error::RuntimeError, listener::UNIX_PATH_MAX};

#[derive(Clone, Copy)]
struct NoopSpawner;

impl DetachedThreadSpawner for NoopSpawner {
    fn spawn_detached<F>(self, _f: F) -> Result<(), SpawnError>
    where
        F: FnOnce() + Send + 'static,
    {
        Ok(())
    }
}

arch_test!(unix_path_max_is_107, {
    // Linux UNIX_PATH_MAX = 108 bytes including NUL → 107 usable bytes.
    assert_eq!(UNIX_PATH_MAX, 107);
});

arch_test!(start_listener_rejects_overlong_path, {
    use reovim_kernel::{Init, LauncherArgs};

    let kernel = Init::new(LauncherArgs::default())
        .boot()
        .expect("boot succeeds");

    // A 108-byte path — one byte over UNIX_PATH_MAX.
    let long_path = [b'x'; 108];
    let result = crate::start_listener(&kernel, &long_path, NetControl::noop(), NoopSpawner);
    assert!(matches!(result, Err(RuntimeError::InvalidPath)));
});
