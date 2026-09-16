use super::*;
use alloc::boxed::Box;
use core::task::{Context, Poll, Waker};
use reovim_kernel_interface::{
    Outcome,
    call::{Cancellation, Completion, PendingOps},
    provider::{OpenFn, ResourceOps},
};

type Saved = Rc<RefCell<Vec<Open>>>;

fn provider(saved: Saved, callback: Option<OpenFn<Saved>>) -> ResourceProvider {
    ResourceProvider::new(
        saved,
        ResourceOps {
            open: callback,
            namespace: None,
            io: None,
            stop: |_| Ok(()),
        },
        Box::new([]),
        Rc::new(()),
    )
    .ok()
    .unwrap()
}

fn empty() -> ResourceProvider {
    provider(Rc::new(RefCell::new(Vec::new())), None)
}

fn begin(saved: &mut Saved, opened: &Open) -> Result<Pending, Error> {
    saved.borrow_mut().push(opened.clone());
    opened.stage(empty()).map_err(|(error, _)| error)?;
    Ok(Pending::new(
        Some(Completion {
            outcome: Outcome::Failed(Error::UNAVAILABLE),
            activity: 0,
            owner: None,
        }),
        PendingOps {
            poll: |_, _| Ok(Poll::Ready(())),
            take: |result| result.take().ok_or(Error::CLOSED),
            cancel: |_| Ok(Cancellation::TooLate),
            reconcile: |_| Err(Error::CLOSED),
            finish: |_| Ok(()),
        },
    ))
}

fn failed(mut pending: Pending) {
    assert_eq!(
        pending.poll(&mut Context::from_waker(Waker::noop())),
        Ok(Poll::Ready(()))
    );
    assert_eq!(
        pending.take().unwrap().outcome,
        Outcome::Failed(Error::UNAVAILABLE)
    );
    pending.finish().unwrap();
}

fn rejected(target: &Open, expected: Error) {
    let Err((error, mut provider)) = target.stage(empty()) else {
        panic!("staging unexpectedly accepted a provider");
    };
    assert_eq!(error, expected);
    provider.stop().unwrap();
}

#[test]
fn a_failed_attempts_staging_clone_cannot_enter_the_next_attempt() {
    let saved = Rc::new(RefCell::new(Vec::new()));
    let mut runtime =
        super::super::Runtime::new([0x75; 16], super::super::Limits::default())
            .unwrap();
    runtime
        .register(b"/subject", provider(saved.clone(), Some(begin)), None)
        .ok()
        .unwrap();
    let subject = runtime.registry.borrow().entries[0].clone();
    let opening = runtime.registry.borrow_mut().opening.reserve().unwrap();
    failed(opening.begin(&subject).unwrap().unwrap());
    opening.reset().unwrap();
    rejected(&saved.borrow()[0], Error::CLOSED);
    let next = opening.begin(&subject).unwrap().unwrap();
    rejected(&saved.borrow()[0], Error::CLOSED);
    rejected(&saved.borrow()[1], Error::CONFLICT);
    failed(next);
    opening.finish();
    drop(opening);
    drop(subject);
    runtime.close().unwrap();
    rejected(&saved.borrow()[1], Error::CLOSED);
}

fn packet_provider(receive: Option<u8>, send: Option<u8>) -> ResourceProvider {
    use reovim_kernel_interface::{
        declaration::ValueType,
        provider::{IoInfo, IoOps, ReceiveOps, SendOps},
    };
    ResourceProvider::new(
        (),
        ResourceOps {
            open: None,
            namespace: None,
            io: Some(IoOps {
                info: IoInfo {
                    read_modes: 0,
                    write_modes: 0,
                    alignment: 1,
                    transfer_limit: 8,
                },
                read: None,
                write: None,
                receive: receive.map(|tag| ReceiveOps {
                    packet: ValueType::decode(&[tag]).unwrap(),
                    begin: |(), _| Err(Error::UNAVAILABLE),
                }),
                send: send.map(|tag| SendOps {
                    packet: ValueType::decode(&[tag]).unwrap(),
                    begin: |(), _| Err(Error::UNAVAILABLE),
                }),
            }),
            stop: |()| Ok(()),
        },
        Box::new([]),
        Rc::new(()),
    )
    .ok()
    .unwrap()
}

#[test]
fn opened_packets_preserve_subject_types_and_independent_direction_offers() {
    let subject = Implementation::new(packet_provider(Some(3), None));
    assert!(support::compatible(&subject, &packet_provider(Some(3), None)));
    assert!(support::compatible(&subject, &packet_provider(None, None)));
    assert!(!support::compatible(&subject, &packet_provider(Some(4), None)));
    assert!(!support::compatible(&subject, &packet_provider(None, Some(3))));
    let subject = Implementation::new(packet_provider(None, Some(4)));
    assert!(support::compatible(&subject, &packet_provider(None, Some(4))));
    assert!(!support::compatible(&subject, &packet_provider(Some(4), None)));
}
