use super::*;
use reovim_kernel_interface::{
    call::{Cancellation, Completion, PendingOps},
    declaration::Declaration,
    provider::{Edit, ProviderInterface, ResourceOps, ResourceProvider, State},
};
use reovim_sdk::{ExportedProvider, ExportedState, import_state_init};
use std::cell::{Cell, RefCell};

fn revision(value: u64) -> State {
    State::new(
        Rc::new(value),
        17,
        |value| {
            let provider = crate::memory::provider(&value.to_le_bytes());
            drop(value);
            provider
        },
        Rc::new(()),
    )
}
struct Probe {
    edit: RefCell<Option<Edit>>,
    reject: bool,
    busy: Cell<bool>,
    finish_proposal: Cell<Option<Error>>,
}
impl Probe {
    fn new(mode: u32) -> Rc<Self> {
        Rc::new(Self {
            edit: RefCell::new(None),
            reject: mode == 1,
            busy: Cell::new(mode == 3),
            finish_proposal: Cell::new(None),
        })
    }
}
fn declaration() -> Declaration {
    let mut bytes = vec![0x78; 16];
    bytes.extend_from_slice(&1_u32.to_le_bytes());
    bytes.extend_from_slice(&[
        2, 7, 0, 0, 0, 1, 2, 0, 0, 0, 2, 4, 0, b'm', b'o', b'v', b'e', 4, 0, 0,
    ]);
    Declaration::decode(&bytes).unwrap()
}
fn provider(probe: Rc<Probe>) -> ResourceProvider {
    ResourceProvider::new(
        probe,
        ResourceOps { open: None, namespace: None, io: None, stop: |_| Ok(()) },
        Box::new([ProviderInterface {
            declaration: declaration(),
            begin: None,
            create: None,
            prepare: Some(prepare),
        }]),
        Rc::new(()),
    )
    .ok()
    .unwrap()
}
struct Work {
    probe: Rc<Probe>,
    edit: Edit,
    candidate: State,
    taken: bool,
}
fn prepare(
    probe: &mut Rc<Probe>,
    _: usize,
    _: Operation,
    edit: &Edit,
    input: &Value,
) -> Result<Pending, Error> {
    *probe.edit.borrow_mut() = Some(edit.clone());
    if probe.reject {
        return Err(Error::DENIED);
    }
    let before = edit.before().data::<u64>().ok_or(Error::INVALID)?;
    let delta = u64::from_le_bytes(
        input.data.as_ref().try_into().map_err(|_| Error::INVALID)?,
    );
    let candidate = revision(before + delta);
    Ok(Pending::new(
        Work {
            probe: probe.clone(),
            edit: edit.clone(),
            candidate,
            taken: false,
        },
        PendingOps {
            poll: |_, _| Ok(Poll::Ready(())),
            take,
            cancel: |_| Ok(Cancellation::TooLate),
            reconcile: |_| Err(Error::UNSUPPORTED),
            finish,
        },
    ))
}
fn take(work: &mut Work) -> Result<Completion, Error> {
    if work.taken {
        return Err(Error::CLOSED);
    }
    work.taken = true;
    let outcome = match work.edit.propose(&work.candidate) {
        Ok(()) => Outcome::Ok(Value::unit()),
        Err(error) => Outcome::Failed(error),
    };
    Ok(Completion { outcome, activity: 0, owner: None })
}
fn finish(work: &mut Work) -> Result<(), Error> {
    if work.probe.busy.replace(false) {
        work.probe
            .finish_proposal
            .set(work.edit.propose(&work.candidate).err());
        return Err(Error::BUSY);
    }
    Ok(())
}

#[test]
fn independent_c_edits_rust_state_and_retained_capabilities_expire() {
    for mode in 0..4 {
        check_native(mode);
    }
}
fn check_native(mode: u32) {
    let library = Library::compile_source("edit_export.c");
    let invoke: unsafe extern "C" fn(
        *mut abi::Provider,
        *const abi::State,
        u32,
    ) = unsafe { library.symbol(c"fixture_edit") };
    let probe = Probe::new(mode);
    let export =
        ExportedProvider::new(provider(probe.clone()), LIMITS).ok().unwrap();
    let mut raw = export.into_raw();
    let before = ExportedState::new(revision(91), LIMITS);
    let state = before.raw();
    unsafe { invoke(&raw mut raw, &raw const state, mode) };
    drop(before);
    let borrowed = probe.edit.borrow().as_ref().unwrap().clone();
    assert_eq!(borrowed.before().data::<u64>(), Some(&91));
    let calls = library.count(c"fixture_proposals");
    assert_eq!(borrowed.propose(&revision(99)), Err(Error::CLOSED));
    assert_eq!(library.count(c"fixture_proposals"), calls);
    if mode == 3 {
        assert_eq!(probe.finish_proposal.get(), Some(Error::CONFLICT));
    }
    assert_eq!(unsafe { raw.stop.unwrap()(raw.context) }, 0);
    unsafe { raw.destroy.unwrap()(raw.context) };
}

#[test]
fn editing_roundtrip_preserves_state_binding_and_fences_finished_loans() {
    let probe = Probe::new(0);
    let before = ExportedState::new(revision(91), LIMITS);
    let implementation =
        ExportedProvider::new(provider(probe.clone()), LIMITS).ok().unwrap();
    let raw = abi::StateInit {
        initial: before.raw(),
        implementation: implementation.raw(),
        byte_edits: std::ptr::null(),
    };
    let (mut state, mut provider) =
        unsafe { import_state_init(&raw const raw, Rc::new(()), LIMITS) }
            .unwrap();
    let _ = implementation.into_raw();
    drop(before);
    for expected in [98_u64, 105] {
        state = roundtrip(&mut provider, state, expected);
        let retained = probe.edit.borrow().as_ref().unwrap().clone();
        assert_eq!(retained.propose(&revision(99)), Err(Error::CLOSED));
    }
    provider.stop().unwrap();
    drop(provider);
    drop(state);
}
fn roundtrip(
    provider: &mut ResourceProvider,
    before: State,
    expected: u64,
) -> State {
    let slot = Rc::new(RefCell::new(None));
    let edit = Edit::new(before, slot.clone(), |slot, candidate| {
        *slot.borrow_mut() = Some(candidate.clone());
        Ok(())
    });
    let mut pending = provider
        .prepare(
            0,
            Operation::Ioctl(7),
            &edit,
            &Value::copy(&7_u64.to_le_bytes(), &[]).unwrap(),
        )
        .unwrap();
    assert_eq!(
        pending.poll(&mut Context::from_waker(Waker::noop())),
        Ok(Poll::Ready(()))
    );
    let outcome = pending.take().unwrap();
    assert!(matches!(outcome.outcome, Outcome::Ok(_)));
    drop(outcome);
    pending.finish().unwrap();
    let candidate = slot.borrow_mut().take().unwrap();
    let mut snapshot = candidate.snapshot().unwrap();
    let mut read = snapshot
        .read(reovim_kernel_interface::provider::Read {
            offset: Some(0),
            maximum: 8,
        })
        .unwrap();
    assert_eq!(
        read.poll(&mut Context::from_waker(Waker::noop())),
        Ok(Poll::Ready(()))
    );
    let completion = read.take().unwrap();
    let Outcome::Ok(value) = &completion.outcome else {
        panic!("snapshot");
    };
    assert_eq!(&value.data[4..12], &expected.to_le_bytes());
    drop(completion);
    read.finish().unwrap();
    snapshot.stop().unwrap();
    candidate
}
