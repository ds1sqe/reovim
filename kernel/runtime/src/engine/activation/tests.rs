use super::*;
use crate::engine::{Limits, Runtime};
use core::cell::Cell;
use reovim_kernel_interface::provider::ResourceOps;

struct Probe {
    ready: Cell<bool>,
    destroyed: Cell<bool>,
    registry: Weak<RefCell<Registry>>,
}

struct Device(Rc<Probe>);

impl Drop for Device {
    fn drop(&mut self) {
        let registry = self.0.registry.upgrade().unwrap();
        assert!(registry.try_borrow_mut().is_ok());
        assert!(!self.0.destroyed.replace(true));
    }
}

fn provider(probe: &Rc<Probe>) -> ResourceProvider {
    ResourceProvider::new(
        Device(probe.clone()),
        ResourceOps {
            open: None,
            namespace: None,
            io: None,
            stop: |device| {
                let registry = device.0.registry.upgrade().unwrap();
                assert!(registry.try_borrow_mut().is_ok());
                if !device.0.ready.get() {
                    return Err(Error::BUSY);
                }
                Ok(())
            },
        },
        Box::new([]),
        Rc::new(()),
    )
    .ok()
    .unwrap()
}

fn probe(runtime: &Runtime) -> Rc<Probe> {
    Rc::new(Probe {
        ready: Cell::new(true),
        destroyed: Cell::new(false),
        registry: Rc::downgrade(&runtime.registry),
    })
}

fn batch(runtime: &Runtime, required: &[&[u8]]) -> Batch {
    Batch::new(&runtime.registry, None, b"/module", required, 2, false).unwrap()
}

fn available(runtime: &Runtime, path: &[u8]) -> bool {
    runtime.registry.borrow().validate(path, &[]).is_ok()
}

#[test]
fn complete_batch_is_invisible_until_publication_and_fences_every_clone() {
    let mut runtime = Runtime::new([0x73; 16], Limits::default()).unwrap();
    let batch = batch(&runtime, &[b"first", b"second"]);
    let registrar = batch.registrar();
    let clone = registrar.clone();
    let first = probe(&runtime);
    let second = probe(&runtime);
    assert!(registrar.stage(b"first", provider(&first)).is_ok());
    assert!(available(&runtime, b"/module/first"));
    assert!(clone.stage(b"second", provider(&second)).is_ok());
    assert!(available(&runtime, b"/module/second"));
    let identities = batch.publish().unwrap();
    assert!(!available(&runtime, b"/module/first"));
    assert!(!available(&runtime, b"/module/second"));
    let registry = runtime.registry.borrow();
    assert_eq!(registry.entries[0].identity, identities[0]);
    assert_eq!(registry.entries[0].path.as_ref(), b"/module/first");
    assert_eq!(registry.entries[1].identity, identities[1]);
    assert_eq!(registry.entries[1].path.as_ref(), b"/module/second");
    drop(registry);
    assert_eq!(batch.publish(), Err(Error::CLOSED));
    batch.inert();
    batch.starting();
    let late = probe(&runtime);
    let (error, mut returned) =
        clone.stage(b"late", provider(&late)).err().unwrap();
    assert_eq!(error, Error::CLOSED);
    assert!(!late.destroyed.get());
    returned.stop().unwrap();
    drop(returned);
    assert!(late.destroyed.get());
    drop(batch);
    runtime.close().unwrap();
    assert!(first.destroyed.get() && second.destroyed.get());
}

#[test]
fn incomplete_batch_keeps_busy_rollback_and_capacity_until_cleanup() {
    let limits = Limits { resources: 1, ..Limits::default() };
    let mut runtime = Runtime::new([0x74; 16], limits).unwrap();
    let batch = batch(&runtime, &[b"first", b"second"]);
    let registrar = batch.registrar();
    let pending = probe(&runtime);
    pending.ready.set(false);
    assert!(registrar.stage(b"first", provider(&pending)).is_ok());
    assert_eq!(batch.publish(), Err(Error::UNAVAILABLE));
    assert!(available(&runtime, b"/module/first"));
    assert_eq!(batch.close(), Err(Error::BUSY));
    assert!(!pending.destroyed.get());
    let spare = probe(&runtime);
    let (error, spare_provider) =
        runtime.register(b"/spare", provider(&spare), None).err().unwrap();
    assert_eq!(error, Error::PRESSURE);
    pending.ready.set(true);
    batch.close().unwrap();
    assert!(pending.destroyed.get());
    runtime.register(b"/spare", spare_provider, None).ok().unwrap();
    runtime.close().unwrap();
    assert!(spare.destroyed.get());
}

#[test]
fn publication_revalidates_without_partially_installing_the_batch() {
    let mut runtime = Runtime::new([0x75; 16], Limits::default()).unwrap();
    let batch = batch(&runtime, &[b"first", b"second"]);
    let first = probe(&runtime);
    let second = probe(&runtime);
    assert!(batch.registrar().stage(b"first", provider(&first)).is_ok());
    assert!(batch.registrar().stage(b"second", provider(&second)).is_ok());
    let winner = probe(&runtime);
    let identity = runtime
        .register(b"/module/second", provider(&winner), None)
        .ok()
        .unwrap();
    assert_eq!(batch.publish(), Err(Error::CONFLICT));
    assert!(available(&runtime, b"/module/first"));
    assert_eq!(runtime.registry.borrow().entries[0].identity, identity);
    batch.close().unwrap();
    assert!(first.destroyed.get() && second.destroyed.get());
    assert!(!winner.destroyed.get());
    runtime.close().unwrap();
}

#[test]
fn relative_exports_cannot_escape_the_mount_or_change_its_name_policy() {
    let mut runtime = Runtime::new([0x76; 16], Limits::default()).unwrap();
    let batch = batch(&runtime, &[]);
    let registrar = batch.registrar();
    for path in [b"/escape".as_slice(), b"../escape", b"a//b", b"a/..", b"\xff"]
    {
        let rejected = probe(&runtime);
        let (error, mut returned) =
            registrar.stage(path, provider(&rejected)).err().unwrap();
        assert_eq!(error, Error::INVALID);
        assert!(!rejected.destroyed.get());
        returned.stop().unwrap();
        drop(returned);
        assert!(rejected.destroyed.get());
    }
    drop(batch);
    let late = probe(&runtime);
    let (error, mut returned) =
        registrar.stage(b"late", provider(&late)).err().unwrap();
    assert_eq!(error, Error::CLOSED);
    returned.stop().unwrap();
    drop(returned);
    runtime.close().unwrap();
}

#[test]
fn closed_runtime_rejects_even_an_empty_publication() {
    let mut runtime = Runtime::new([0x77; 16], Limits::default()).unwrap();
    let batch = batch(&runtime, &[]);
    runtime.close().unwrap();
    assert_eq!(batch.publish(), Err(Error::CLOSED));
}

#[test]
fn module_without_exports_can_publish_an_empty_batch() {
    let mut runtime = Runtime::new([0x79; 16], Limits::default()).unwrap();
    let batch =
        Batch::new(&runtime.registry, None, b"/module", &[], 0, false).unwrap();
    let rejected = probe(&runtime);
    let (error, mut provider) =
        batch.registrar().stage(b"extra", provider(&rejected)).err().unwrap();
    assert_eq!(error, Error::PRESSURE);
    provider.stop().unwrap();
    drop(provider);
    assert_eq!(batch.publish().unwrap(), []);
    assert!(available(&runtime, b"/module/extra"));
    runtime.close().unwrap();
}

#[test]
fn publication_rejects_a_changed_parent_before_installing_any_subject() {
    let mut runtime = Runtime::new([0x7a; 16], Limits::default()).unwrap();
    let batch = batch(&runtime, &[b"child"]);
    let child = probe(&runtime);
    assert!(batch.registrar().stage(b"child", provider(&child)).is_ok());
    runtime.register_namespace(b"/module").unwrap();
    assert_eq!(batch.publish(), Err(Error::CONFLICT));
    assert!(available(&runtime, b"/module/child"));
    assert!(!child.destroyed.get());
    batch.close().unwrap();
    assert!(child.destroyed.get());
    runtime.close().unwrap();
}
