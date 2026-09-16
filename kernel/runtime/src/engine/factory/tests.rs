use super::*;
use core::cell::Cell;
use reovim_kernel_interface::provider::ResourceOps;

#[derive(Default)]
struct Probe {
    destroyed: Cell<bool>,
    busy: Cell<bool>,
}
struct Device {
    probe: Rc<Probe>,
    parent: Option<Rc<Probe>>,
}
impl Drop for Device {
    fn drop(&mut self) {
        if let Some(parent) = &self.parent {
            assert!(!parent.destroyed.get());
        }
        self.probe.destroyed.set(true);
    }
}
fn provider(probe: &Rc<Probe>, parent: Option<Rc<Probe>>) -> ResourceProvider {
    ResourceProvider::new(
        Device { probe: probe.clone(), parent },
        ResourceOps {
            open: None,
            namespace: None,
            io: None,
            stop: |device| {
                if device.probe.busy.get() { Err(Error::BUSY) } else { Ok(()) }
            },
        },
        Box::new([]),
        Rc::new(()),
    )
    .ok()
    .unwrap()
}

struct Fixture {
    stage: Rc<RefCell<Stage>>,
    source: Rc<Probe>,
    primary: Rc<Probe>,
    auxiliary: Rc<Probe>,
}

fn fixture() -> Fixture {
    let source = Rc::new(Probe::default());
    let primary = Rc::new(Probe::default());
    let auxiliary = Rc::new(Probe::default());
    let subject = Rc::new(Probe::default());
    let factory = Rc::new(Entry {
        closed: Cell::new(false),
        identity: ResourceId { epoch: [1; 16], serial: 1 },
        path: Box::new([]),
        implementation: crate::engine::Implementation::new(provider(
            &subject, None,
        )),
        imports: None,
        _origin: None,
        namespace_owner: None,
        directory: false,
        parent: None,
        publication: None,
    });
    let stage = Rc::new(RefCell::new(Stage {
        registry: Weak::new(),
        identity: ResourceId { epoch: [1; 16], serial: 2 },
        imports: None,
        api: None,
        closed: false,
        primary: None,
        auxiliary: Vec::new(),
        ceiling: 0,
        retain_loans: false,
        implementation: crate::engine::Implementation::new(provider(
            &source, None,
        )),
        origin: factory.clone(),
    }));
    // These are the shared Entry construction path for both creation slots.
    let first = entry(
        &stage.borrow(),
        stage.borrow().identity,
        provider(&primary, Some(source.clone())),
    );
    let second = entry(
        &stage.borrow(),
        ResourceId { epoch: [1; 16], serial: 3 },
        provider(&auxiliary, Some(source.clone())),
    );
    stage.borrow_mut().primary = Some(first);
    stage.borrow_mut().auxiliary.push(Auxiliary {
        identity: second.identity,
        token: AccessId(1),
        handle: Some(Handle {
            implementation: second.implementation.clone(),
            entry: second,
            token: AccessId(1),
            opened: Rc::new(RefCell::new(Opened {
                held: 0,
                ceiling: 0,
                create_access: 0,
                closed: false,
                ownership: super::super::access::Ownership::Owned {
                    retain_loans: false,
                },
            })),
            interface: None,
            provisional: None,
        }),
        returned: false,
    });
    factory.implementation.provider.borrow_mut().stop().unwrap();
    stage.borrow().implementation.provider.borrow_mut().stop().unwrap();
    drop(factory);
    Fixture { stage, source, primary, auxiliary }
}

#[test]
fn staged_cleanup_keeps_factory_state_through_busy_destructors() {
    let Fixture { stage, source, primary, auxiliary } = fixture();
    primary.busy.set(true);
    auxiliary.busy.set(true);
    assert_eq!(finish_primary(&stage), Err(Error::BUSY));
    assert_eq!(auxiliary::finish(&stage), Err(Error::BUSY));
    assert!(!source.destroyed.get());
    primary.busy.set(false);
    finish_primary(&stage).unwrap();
    assert!(primary.destroyed.get());
    assert!(!source.destroyed.get());
    auxiliary.busy.set(false);
    auxiliary::finish(&stage).unwrap();
    assert!(auxiliary.destroyed.get());
    drop(stage);
    assert!(source.destroyed.get());
}

#[test]
fn transferred_children_keep_factory_state_after_creation_is_released() {
    let Fixture { stage, source, primary, auxiliary } = fixture();
    let first = stage.borrow_mut().primary.take().unwrap();
    let second = stage.borrow_mut().auxiliary[0].handle.take().unwrap().entry;
    drop(stage);
    assert!(!source.destroyed.get());
    first.implementation.provider.borrow_mut().stop().unwrap();
    drop(first);
    assert!(primary.destroyed.get());
    assert!(!source.destroyed.get());
    auxiliary.busy.set(true);
    assert_eq!(
        second.implementation.provider.borrow_mut().stop(),
        Err(Error::BUSY)
    );
    assert!(!source.destroyed.get());
    auxiliary.busy.set(false);
    second.implementation.provider.borrow_mut().stop().unwrap();
    drop(second);
    assert!(auxiliary.destroyed.get());
    assert!(source.destroyed.get());
}
