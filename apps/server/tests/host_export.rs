#![cfg(all(feature = "std", target_os = "linux"))]

use reovim_host_reference::export::Directory;
use reovim_kernel_interface::{
    Contract, Error, Message, RequestError, ResourceId, access::Method,
    provider::ResourceProvider,
};
use std::{
    fs,
    future::Future,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    task::{Context, Poll, Wake, Waker},
    time::{Duration, Instant},
};

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "reovim-export-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
struct WakeThread(std::thread::Thread);
impl Wake for WakeThread {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}
fn finish<T>(future: impl Future<Output = T>) -> T {
    let mut future = Box::pin(future);
    let wake = Waker::from(Arc::new(WakeThread(std::thread::current())));
    let mut context = Context::from_waker(&wake);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return value;
        }
        assert!(Instant::now() < deadline, "Host operation did not finish");
        std::thread::park_timeout(Duration::from_millis(10));
    }
}
fn write(
    provider: &mut ResourceProvider,
    contract: &str,
    data: &[u8],
) -> Result<Message, RequestError> {
    finish(provider.call(
        Method::Write,
        ResourceId(0),
        Message::new(Contract::new(contract, 1), data),
    ))
}
#[test]
fn export_preserves_bytes_and_never_replaces_an_existing_file() {
    let scratch = Scratch::new();
    let mut provider = Directory::open(&scratch.0).unwrap().provider();
    let created =
        write(&mut provider, "host.export.directory", b"dump").unwrap();
    assert_eq!(created.data[0], 0);
    let bytes = b"dump/children/key/value.bin\0opaque\0\xff\n";
    let saved = write(&mut provider, "host.export.file", bytes).unwrap();
    assert_eq!(saved.data[0], 0);
    assert_eq!(
        fs::read(scratch.0.join("dump/children/key/value.bin")).unwrap(),
        b"opaque\0\xff\n"
    );
    let refused = write(
        &mut provider,
        "host.export.file",
        b"dump/children/key/value.bin\0replacement",
    )
    .unwrap();
    assert_eq!(refused.data[0], 1);
    assert_eq!(
        fs::read(scratch.0.join("dump/children/key/value.bin")).unwrap(),
        b"opaque\0\xff\n"
    );
    let mut large = b"dump/large\0".to_vec();
    large.extend(std::iter::repeat_n(0, 16 * 1024 + 1));
    assert_eq!(
        write(&mut provider, "host.export.file", &large),
        Err(RequestError::Failed(Error::PRESSURE))
    );
    assert!(!scratch.0.join("dump/large").exists());
    provider.unload().unwrap();
}
#[test]
fn output_grants_reject_traversal_and_symlink_following() {
    let scratch = Scratch::new();
    fs::create_dir(scratch.0.join("granted")).unwrap();
    fs::create_dir(scratch.0.join("outside")).unwrap();
    std::os::unix::fs::symlink(
        scratch.0.join("outside"),
        scratch.0.join("granted/link"),
    )
    .unwrap();
    assert!(Directory::open(&scratch.0.join("granted/link")).is_err());
    let mut provider =
        Directory::open(&scratch.0.join("granted")).unwrap().provider();
    let absolute = scratch.0.join("absolute");
    for path in
        ["../outside/escaped", "link/escaped", absolute.to_str().unwrap()]
    {
        let result = write(
            &mut provider,
            "host.export.file",
            format!("{path}\0bytes").as_bytes(),
        )
        .unwrap();
        assert_eq!(result.data[0], 1);
    }
    assert!(!scratch.0.join("outside/escaped").exists());
    assert!(!absolute.exists());
    assert!(fs::read_dir(scratch.0.join("outside")).unwrap().next().is_none());
    provider.unload().unwrap();
}
