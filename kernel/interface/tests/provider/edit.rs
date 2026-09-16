use reovim_kernel_interface::{
    Error, Outcome, Pending, Value,
    call::{Cancellation, Completion, PendingOps},
    declaration::{Declaration, Operation},
    provider::{Edit, ProviderInterface, ResourceOps, ResourceProvider, State},
};
use std::{cell::RefCell, rc::Rc, task::Poll};

#[derive(Clone, Debug, PartialEq, Eq)]
struct Position {
    x: i64,
    y: i64,
}

fn position(value: Position) -> State {
    State::new(Rc::new(value), 16, |_| Err(Error::UNSUPPORTED), Rc::new(()))
}

struct Request {
    open: bool,
    write: bool,
    candidate: Option<State>,
}

fn propose(request: &Rc<RefCell<Request>>, state: &State) -> Result<(), Error> {
    let mut request = request.try_borrow_mut().map_err(|_| Error::BUSY)?;
    if !request.open {
        return Err(Error::CLOSED);
    }
    if !request.write {
        return Err(Error::DENIED);
    }
    if request.candidate.is_some() {
        return Err(Error::CONFLICT);
    }
    request.candidate = Some(state.clone());
    Ok(())
}

fn edit(write: bool) -> (Edit, Rc<RefCell<Request>>) {
    let request =
        Rc::new(RefCell::new(Request { open: true, write, candidate: None }));
    let edit =
        Edit::new(position(Position { x: 3, y: 5 }), request.clone(), propose);
    (edit, request)
}

fn declaration() -> Declaration {
    let mut bytes = vec![0x77; 16];
    bytes.extend_from_slice(&1_u32.to_le_bytes());
    // ioctl1, Edit, WRITE access/activity, Unit input/output/progress.
    bytes.extend_from_slice(&[
        2, 1, 0, 0, 0, 1, 2, 0, 0, 0, 2, 4, 0, b'm', b'o', b'v', b'e', 0, 0, 0,
    ]);
    Declaration::decode(&bytes).unwrap()
}

fn provider() -> ResourceProvider {
    ResourceProvider::new(
        (),
        ResourceOps {
            open: None,
            namespace: None,
            io: None,
            stop: |()| Ok(()),
        },
        Box::new([ProviderInterface {
            declaration: declaration(),
            begin: None,
            create: None,
            prepare: Some(|(), _, _, edit, _| prepare(edit)),
        }]),
        Rc::new(()),
    )
    .ok()
    .unwrap()
}

fn prepare(edit: &Edit) -> Result<Pending, Error> {
    let before = edit.before().data::<Position>().ok_or(Error::INVALID)?;
    let candidate = position(Position { x: before.x + 7, y: before.y });
    Ok(Pending::new(
        Some((edit.clone(), candidate)),
        PendingOps {
            poll: |_, _| Ok(Poll::Ready(())),
            take: |state| {
                let (edit, candidate) = state.take().ok_or(Error::CLOSED)?;
                let outcome = match edit.propose(&candidate) {
                    Ok(()) => Outcome::Ok(Value::unit()),
                    Err(error) => Outcome::Failed(error),
                };
                Ok(Completion { outcome, activity: 0, owner: None })
            },
            cancel: |_| Ok(Cancellation::TooLate),
            reconcile: |_| Err(Error::UNSUPPORTED),
            finish: |_| Ok(()),
        },
    ))
}

#[test]
fn editing_stages_a_nonbyte_candidate_without_changing_selected_state() {
    let (edit, request) = edit(true);
    let mut provider = provider();
    assert_eq!(
        provider.begin(0, Operation::Ioctl(1), &Value::unit()).err(),
        Some(Error::UNSUPPORTED)
    );
    let mut pending = provider
        .prepare(0, Operation::Ioctl(1), &edit, &Value::unit())
        .unwrap();
    assert!(request.borrow().candidate.is_none());
    assert!(matches!(pending.take().unwrap().outcome, Outcome::Ok(_)));
    assert_eq!(
        edit.before().data::<Position>(),
        Some(&Position { x: 3, y: 5 })
    );
    let candidate = request.borrow().candidate.clone().unwrap();
    assert_eq!(candidate.data::<Position>(), Some(&Position { x: 10, y: 5 }));
    assert_eq!(edit.propose(&candidate), Err(Error::CONFLICT));
    let retained = edit.clone();
    request.borrow_mut().open = false;
    assert_eq!(retained.propose(&candidate), Err(Error::CLOSED));
    pending.finish().unwrap();
    provider.stop().unwrap();
}

#[test]
fn denied_preparation_keeps_selected_state_and_never_stages_candidate() {
    let (edit, request) = edit(false);
    let mut provider = provider();
    let mut pending = provider
        .prepare(0, Operation::Ioctl(1), &edit, &Value::unit())
        .unwrap();
    assert!(matches!(
        pending.take().unwrap().outcome,
        Outcome::Failed(Error::DENIED)
    ));
    assert!(request.borrow().candidate.is_none());
    assert_eq!(
        edit.before().data::<Position>(),
        Some(&Position { x: 3, y: 5 })
    );
    pending.finish().unwrap();
    provider.stop().unwrap();
}

#[test]
fn preparation_retains_selected_state_after_capability_and_provider_drop() {
    let (edit, request) = edit(true);
    let owner = Rc::downgrade(&request);
    let mut provider = provider();
    let mut pending = provider
        .prepare(0, Operation::Ioctl(1), &edit, &Value::unit())
        .unwrap();
    drop(edit);
    drop(request);
    assert!(matches!(pending.take().unwrap().outcome, Outcome::Ok(_)));
    pending.finish().unwrap();
    provider.stop().unwrap();
    drop(provider);
    let request = owner.upgrade().unwrap();
    assert_eq!(
        request.borrow().candidate.as_ref().unwrap().data::<Position>(),
        Some(&Position { x: 10, y: 5 })
    );
    drop(request);
    drop(pending);
    assert!(owner.upgrade().is_none());
}

#[test]
fn missing_edit_callback_and_closed_or_wrong_selector_never_prepare() {
    let rejected = ResourceProvider::new(
        91,
        ResourceOps { open: None, namespace: None, io: None, stop: |_| Ok(()) },
        Box::new([ProviderInterface {
            declaration: declaration(),
            begin: Some(|_, _, _, _| panic!("ordinary dispatch")),
            create: None,
            prepare: None,
        }]),
        Rc::new(()),
    );
    assert_eq!(rejected.err(), Some((Error::UNSUPPORTED, 91)));
    let (edit, request) = edit(true);
    let mut provider = provider();
    assert_eq!(
        provider.prepare(0, Operation::Exec, &edit, &Value::unit()).err(),
        Some(Error::UNSUPPORTED)
    );
    provider.stop().unwrap();
    assert_eq!(
        provider.prepare(0, Operation::Ioctl(1), &edit, &Value::unit()).err(),
        Some(Error::CLOSED)
    );
    assert!(request.borrow().candidate.is_none());
}
