use reovim_kernel_interface::{
    Error, Outcome, Pending, Value,
    call::{Cancellation, Completion, PendingOps},
    declaration::{Declaration, Operation},
    provider::{ProviderInterface, ResourceOps, ResourceProvider, State},
};
use std::{cell::Cell, rc::Rc, task::Poll};

struct Revision {
    value: u64,
    destroyed: Rc<Cell<bool>>,
}
impl Drop for Revision {
    fn drop(&mut self) {
        self.destroyed.set(true);
    }
}
struct Code {
    state_destroyed: Rc<Cell<bool>>,
    destroyed: Rc<Cell<bool>>,
}
impl Drop for Code {
    fn drop(&mut self) {
        assert!(self.state_destroyed.get());
        self.destroyed.set(true);
    }
}

fn read_declaration() -> Declaration {
    let mut bytes = vec![0x76; 16];
    bytes.extend_from_slice(&1_u32.to_le_bytes());
    bytes.extend_from_slice(&[
        2, 1, 0, 0, 0, 0, 1, 0, 0, 0, 1, 4, 0, b'r', b'e', b'a', b'd', 0, 4, 0,
    ]);
    Declaration::decode(&bytes).unwrap()
}

fn read(value: u64) -> Pending {
    Pending::new(
        Some(value),
        PendingOps {
            poll: |_, _| Ok(Poll::Ready(())),
            take: |value| {
                Ok(Completion {
                    outcome: Outcome::Ok(Value::copy(
                        &value.take().ok_or(Error::CLOSED)?.to_le_bytes(),
                        &[],
                    )?),
                    activity: 1,
                    owner: None,
                })
            },
            cancel: |_| Ok(Cancellation::TooLate),
            reconcile: |_| Err(Error::UNSUPPORTED),
            finish: |_| Ok(()),
        },
    )
}

fn snapshot(revision: Rc<Revision>) -> Result<ResourceProvider, Error> {
    let value = revision.value;
    // Materialize the value so only State's wrapper retains the source owner.
    drop(revision);
    ResourceProvider::new(
        value,
        ResourceOps { open: None, namespace: None, io: None, stop: |_| Ok(()) },
        Box::new([ProviderInterface {
            prepare: None,
            declaration: read_declaration(),
            begin: Some(|value, _, _, _| Ok(read(*value))),
            create: None,
        }]),
        Rc::new(()),
    )
    .map_err(|(error, _)| error)
}

#[test]
fn snapshots_and_pending_reads_retain_private_state_before_code_release() {
    let state_destroyed = Rc::new(Cell::new(false));
    let code_destroyed = Rc::new(Cell::new(false));
    let revision =
        Rc::new(Revision { value: 41, destroyed: state_destroyed.clone() });
    let state = State::new(
        revision,
        13,
        snapshot,
        Rc::new(Code {
            state_destroyed: state_destroyed.clone(),
            destroyed: code_destroyed.clone(),
        }),
    );
    assert_eq!(state.cost(), 13);
    assert_eq!(state.data::<Revision>().unwrap().value, 41);
    assert!(state.data::<u64>().is_none());
    let retained = state.clone();
    let mut provider = state.snapshot().unwrap();
    assert!(provider.io().is_none());
    let mut request =
        provider.begin(0, Operation::Ioctl(1), &Value::unit()).unwrap();
    drop(state);
    drop(retained);
    provider.stop().unwrap();
    drop(provider);
    assert!(!state_destroyed.get());
    assert!(!code_destroyed.get());
    let completion = request.take().unwrap();
    assert_eq!(
        completion.outcome,
        Outcome::Ok(Value::copy(&41_u64.to_le_bytes(), &[]).unwrap())
    );
    drop(completion);
    request.finish().unwrap();
    drop(request);
    assert!(state_destroyed.get());
    assert!(code_destroyed.get());
}

#[test]
fn failed_snapshot_keeps_the_selected_state_and_its_retained_alias() {
    let destroyed = Rc::new(Cell::new(false));
    let state = State::new(
        Rc::new(Revision { value: 91, destroyed: destroyed.clone() }),
        17,
        |_| Err(Error::PRESSURE),
        Rc::new(()),
    );
    let retained = state.clone();
    assert!(matches!(state.snapshot(), Err(Error::PRESSURE)));
    drop(state);
    assert!(!destroyed.get());
    assert_eq!(retained.data::<Revision>().unwrap().value, 91);
    assert_eq!(retained.cost(), 17);
    drop(retained);
    assert!(destroyed.get());
}
