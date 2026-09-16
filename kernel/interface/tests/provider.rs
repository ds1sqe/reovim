use std::{cell::RefCell, rc::Rc};

use reovim_kernel_interface::{
    Error, Pending, Value,
    call::{Cancellation, Completion, Outcome, PendingOps},
    declaration::{Declaration, Operation},
    provider::{
        Creation, CreationOps, ProviderInterface, ResourceOps, ResourceProvider,
    },
};

fn declaration(factory: bool) -> Declaration {
    let mut bytes = vec![0x71; 16];
    bytes.extend_from_slice(&1_u32.to_le_bytes());
    bytes.push(1); // Exec with reserved zero command.
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.push(if factory { 2 } else { 0 });
    bytes.extend_from_slice(&4_u32.to_le_bytes()); // EXEC.
    bytes.extend_from_slice(&[0, 4, 0]); // No activity, four-byte name.
    bytes.extend_from_slice(b"make");
    bytes.extend_from_slice(&[0, 0, 0]); // Unit input, output and progress.
    if factory {
        bytes.push(12); // Created Ref(AnyResource, READ).
        bytes.extend_from_slice(&[0; 16]);
        bytes.extend_from_slice(&1_u32.to_le_bytes());
    }
    Declaration::decode(&bytes).unwrap()
}

fn pending(creation: Creation) -> Pending {
    Pending::new(
        creation,
        PendingOps {
            poll: |_, _| Ok(std::task::Poll::Ready(())),
            take: |creation| {
                creation.resource(provider(false)).map_err(|(e, _)| e)?;
                Ok(Completion {
                    outcome: Outcome::Ok(Value::unit()),
                    activity: 0,
                    owner: None,
                })
            },
            cancel: |_| Ok(Cancellation::TooLate),
            reconcile: |_| Err(Error::UNSUPPORTED),
            finish: |_| Ok(()),
        },
    )
}

fn provider(factory: bool) -> ResourceProvider {
    with_code(factory, Rc::new(()))
}

fn with_code(factory: bool, code: Rc<()>) -> ResourceProvider {
    ResourceProvider::new(
        (),
        ResourceOps {
            open: None,
            namespace: None,
            io: None,
            stop: |()| Ok(()),
        },
        Box::new([ProviderInterface {
            prepare: None,
            declaration: declaration(factory),
            begin: if factory {
                None
            } else {
                Some(|(), _, _, _| Err(Error::UNAVAILABLE))
            },
            create: Some(|(), _, _, _, creation| Ok(pending(creation.clone()))),
        }]),
        code,
    )
    .ok()
    .unwrap()
}

#[derive(Default)]
struct Stage {
    closed: bool,
    primary: Option<ResourceProvider>,
}

fn creation(state: Rc<RefCell<Stage>>) -> Creation {
    Creation::new(
        state,
        CreationOps {
            keyed: None,
            auxiliary: None,
            resource: |state, provider| {
                let mut target = state.borrow_mut();
                if target.closed {
                    return Err((Error::CLOSED, provider));
                }
                if target.primary.is_some() {
                    return Err((Error::CONFLICT, provider));
                }
                target.primary = Some(provider);
                Ok(())
            },
        },
    )
}

#[test]
fn factory_dispatch_retains_request_staging_and_code() {
    let state = Rc::new(RefCell::new(Stage::default()));
    let staging = creation(state.clone());
    let code = Rc::new(());
    let weak = Rc::downgrade(&code);
    let mut factory = with_code(true, code);
    assert!(matches!(
        factory.begin(0, Operation::Exec, &Value::unit()),
        Err(Error::UNSUPPORTED)
    ));
    let mut call =
        factory.create(0, Operation::Exec, &Value::unit(), &staging).unwrap();
    drop(staging);
    factory.stop().unwrap();
    drop(factory);
    assert!(weak.upgrade().is_some());
    assert!(state.borrow().primary.is_none());
    assert_eq!(call.take().unwrap().outcome, Outcome::Ok(Value::unit()));
    let mut primary = state.borrow_mut().primary.take().unwrap();
    assert!(matches!(
        primary.begin(0, Operation::Exec, &Value::unit()),
        Err(Error::UNAVAILABLE)
    ));
    primary.stop().unwrap();
    call.finish().unwrap();
    drop(call);
    assert!(weak.upgrade().is_none());
}

#[test]
fn rejected_staging_returns_provider_and_clones_share_fence() {
    let state = Rc::new(RefCell::new(Stage::default()));
    let staging = creation(state.clone());
    let alias = staging.clone();
    assert!(staging.resource(provider(false)).is_ok());
    let (error, mut rejected) = alias.resource(provider(false)).err().unwrap();
    assert_eq!(error, Error::CONFLICT);
    assert!(matches!(
        rejected.begin(0, Operation::Exec, &Value::unit()),
        Err(Error::UNAVAILABLE)
    ));
    state.borrow_mut().closed = true;
    let (error, mut returned) = staging.resource(rejected).err().unwrap();
    assert_eq!(error, Error::CLOSED);
    returned.stop().unwrap();
    state.borrow_mut().primary.take().unwrap().stop().unwrap();
}

#[test]
fn missing_factory_table_and_wrong_dispatch_reject() {
    let result = ResourceProvider::new(
        42,
        ResourceOps { open: None, namespace: None, io: None, stop: |_| Ok(()) },
        (vec![declaration(true)].into_boxed_slice())
            .into_iter()
            .map(|declaration| ProviderInterface {
                prepare: None,
                declaration,
                begin: Some(|_, _, _, _| panic!("ordinary callback entered")),
                create: None,
            })
            .collect(),
        Rc::new(()),
    );
    let (error, state) = result.err().unwrap();
    assert_eq!((error, state), (Error::UNSUPPORTED, 42));
    let staging = creation(Rc::default());
    assert!(matches!(
        provider(false).create(0, Operation::Exec, &Value::unit(), &staging),
        Err(Error::UNSUPPORTED)
    ));
}

#[test]
fn invalid_io_metadata_returns_state_without_entering_callbacks() {
    use reovim_kernel_interface::provider::{IoInfo, IoOps};

    for (read_modes, write_modes, alignment, transfer_limit) in
        [(1, 0, 1, 8), (0, 1, 1, 8), (4, 0, 1, 8), (0, 0, 0, 8), (0, 0, 1, 0)]
    {
        let state = Rc::new(RefCell::new(b"owned".to_vec()));
        let result = ResourceProvider::new(
            state.clone(),
            ResourceOps {
                open: None,
                namespace: None,
                io: Some(IoOps {
                    receive: None,
                    send: None,
                    info: IoInfo {
                        read_modes,
                        write_modes,
                        alignment,
                        transfer_limit,
                    },
                    read: None,
                    write: None,
                }),
                stop: |_| panic!("rejected provider stopped"),
            },
            Box::new([]),
            Rc::new(()),
        );
        let Err((error, returned)) = result else {
            panic!("invalid I/O metadata accepted")
        };
        assert_eq!(error, Error::INVALID);
        assert!(Rc::ptr_eq(&state, &returned));
        assert_eq!(returned.borrow().as_slice(), b"owned");
    }
}

#[test]
fn ordinary_declaration_requires_callback_and_returns_private_state() {
    let state = Rc::new(RefCell::new(b"private".to_vec()));
    let result = ResourceProvider::new(
        state.clone(),
        ResourceOps {
            open: None,
            namespace: None,
            io: None,
            stop: |_| panic!("rejected provider stopped"),
        },
        Box::new([ProviderInterface {
            prepare: None,
            declaration: declaration(false),
            begin: None,
            create: None,
        }]),
        Rc::new(()),
    );
    let (error, returned) = result.err().unwrap();
    assert_eq!(error, Error::UNSUPPORTED);
    assert!(Rc::ptr_eq(&state, &returned));
    assert_eq!(returned.borrow().as_slice(), b"private");
}

#[test]
fn stopping_fences_new_work_and_retries_cleanup_until_success() {
    use reovim_kernel_interface::provider::{Read, Write};

    let ready = Rc::new(std::cell::Cell::new(false));
    let mut provider = ResourceProvider::new(
        ready.clone(),
        ResourceOps {
            open: None,
            namespace: None,
            io: None,
            stop: |ready| {
                if !ready.get() {
                    return Err(Error::BUSY);
                }
                ready.set(false);
                Ok(())
            },
        },
        Box::new([]),
        Rc::new(()),
    )
    .ok()
    .unwrap();
    assert!(matches!(
        provider.begin(0, Operation::Exec, &Value::unit()),
        Err(Error::UNSUPPORTED)
    ));
    assert_eq!(provider.stop(), Err(Error::BUSY));
    assert!(matches!(
        provider.begin(0, Operation::Exec, &Value::unit()),
        Err(Error::CLOSED)
    ));
    assert!(matches!(
        provider.read(Read { offset: None, maximum: 1 }),
        Err(Error::CLOSED)
    ));
    assert!(matches!(
        provider.write(Write { offset: None, data: b"x" }),
        Err(Error::CLOSED)
    ));
    assert!(matches!(
        provider.create(
            0,
            Operation::Exec,
            &Value::unit(),
            &creation(Rc::default()),
        ),
        Err(Error::CLOSED)
    ));
    ready.set(true);
    provider.stop().unwrap();
    assert!(!ready.get());
    provider.stop().unwrap(); // A repeated callback would return Busy.
}

fn another_declaration(factory: bool) -> Declaration {
    let mut bytes = declaration(factory).bytes().to_vec();
    bytes[0] = 0x72;
    Declaration::decode(&bytes).unwrap()
}

#[test]
fn callback_on_another_interface_cannot_satisfy_a_declaration() {
    let result = ResourceProvider::new(
        42,
        ResourceOps {
            open: None,
            namespace: None,
            io: None,
            stop: |_| panic!("rejected provider stopped"),
        },
        Box::new([
            ProviderInterface {
                prepare: None,
                declaration: declaration(false),
                begin: Some(|_, _, _, _| panic!("rejected provider invoked")),
                create: None,
            },
            ProviderInterface {
                prepare: None,
                declaration: another_declaration(false),
                begin: None,
                create: None,
            },
        ]),
        Rc::new(()),
    );
    assert_eq!(result.err().unwrap(), (Error::UNSUPPORTED, 42));
}

#[test]
fn each_interface_uses_its_own_callback_with_shared_private_state() {
    let state = Rc::new(RefCell::new(Vec::new()));
    let mut provider = ResourceProvider::new(
        state.clone(),
        ResourceOps { open: None, namespace: None, io: None, stop: |_| Ok(()) },
        Box::new([
            ProviderInterface {
                prepare: None,
                declaration: declaration(false),
                begin: Some(|state, index, operation, _| {
                    assert_eq!((index, operation), (0, Operation::Exec));
                    state.borrow_mut().push("first");
                    Err(Error::UNAVAILABLE)
                }),
                create: None,
            },
            ProviderInterface {
                prepare: None,
                declaration: another_declaration(false),
                begin: Some(|state, index, operation, _| {
                    assert_eq!((index, operation), (1, Operation::Exec));
                    state.borrow_mut().push("second");
                    Err(Error::UNAVAILABLE)
                }),
                create: None,
            },
        ]),
        Rc::new(()),
    )
    .ok()
    .unwrap();
    for index in [1, 0] {
        assert!(matches!(
            provider.begin(index, Operation::Exec, &Value::unit()),
            Err(Error::UNAVAILABLE)
        ));
    }
    assert!(matches!(
        provider.begin(2, Operation::Exec, &Value::unit()),
        Err(Error::UNSUPPORTED)
    ));
    assert_eq!(&*state.borrow(), &["second", "first"]);
    provider.stop().unwrap();
}

#[path = "provider/packet.rs"]
mod packets;

#[path = "provider/state.rs"]
mod states;

#[path = "provider/edit.rs"]
mod edits;
