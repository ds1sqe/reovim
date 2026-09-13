use super::*;
use reovim_kernel_interface::publication;

fn edit(f: &mut Fixture, active: bool) {
    let mut input = Message::reference(
        Contract::new("logical.set", 1),
        f.buffer.state.buffer,
    );
    input.data =
        [f.buffer.state.revision.to_le_bytes().as_slice(), &[u8::from(active)]]
            .concat()
            .into();
    f.buffer = buffer::observation(
        finish(&mut f.runtime, f.client.write(f.buffer.state.buffer, &input))
            .unwrap(),
    )
    .unwrap();
}

fn retire(f: &mut Fixture, work: ResourceId) {
    close_work(&mut f.runtime, &f.client, work);
}

fn close_work(runtime: &mut Runtime, client: &Connection, work: ResourceId) {
    for _ in 0..128 {
        match client.close(work).into_inner() {
            Ok(()) => return,
            Err(RequestError::Rejected(error)) if error == Error::BUSY => (),
            other => panic!("proof close: {other:?}"),
        }
        runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    }
    panic!("proof cleanup did not finish");
}

#[test]
fn admission_state_survives_history_change_before_save_dispatch() {
    let mut f = setup_model(true);
    edit(&mut f, true);
    let first = buffer::describe(&f.client, f.buffer.state.buffer);
    let second = buffer::describe(&f.client, f.buffer.state.buffer);
    let remaining = buffer::describe(&f.client, f.buffer.state.buffer);
    // Complete two immediate requests before retiring them. The reference
    // scheduler then begins the later Undo before the newly queued save.
    let mut context = Context::from_waker(Waker::noop());
    f.runtime.advance(&mut context).unwrap();
    f.runtime.advance(&mut context).unwrap();
    finish(&mut f.runtime, first).unwrap();
    finish(&mut f.runtime, second).unwrap();
    let selected = f.buffer.state;
    let saving = publication::save(&f.client, selected, &[]);
    let undo = Message {
        contract: Contract::new("kernel.history.undo", 1),
        data: selected.revision.to_le_bytes().into(),
        resources: Box::new([selected.buffer]),
    };
    finish(&mut f.runtime, f.client.write(f.buffer.history, &undo)).unwrap();
    f.buffer = buffer::observation(
        finish(
            &mut f.runtime,
            f.client.read(
                selected.buffer,
                &Message::new(Contract::new("logical.value", 1), &[]),
            ),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(f.buffer.value.data.as_ref(), [0]);
    finish(&mut f.runtime, remaining).unwrap();
    let report =
        publication::report(&finish(&mut f.runtime, saving).unwrap()).unwrap();
    assert_eq!(report.selected, selected);
    assert_eq!(report.status, protocol::Status::Applied);
    assert!(f.device.current.borrow().active);
    retire(&mut f, report.work);
    stop(f);
}

#[test]
fn concurrent_saves_keep_the_original_base_instead_of_refreshing_it() {
    use protocol::Status::{Applied, DefinitelyNotApplied};
    let mut f = setup_model(true);
    edit(&mut f, true);
    let first = publication::save(&f.client, f.buffer.state, &[]);
    let second = publication::save(&f.client, f.buffer.state, &[]);
    let first =
        publication::report(&finish(&mut f.runtime, first).unwrap()).unwrap();
    let second =
        publication::report(&finish(&mut f.runtime, second).unwrap()).unwrap();
    assert!(matches!((first.status, second.status),
        (Applied, DefinitelyNotApplied(code))
        | (DefinitelyNotApplied(code), Applied)
        if code == Error::STALE.code));
    retire(&mut f, first.work);
    retire(&mut f, second.work);
    stop(f);
}

#[test]
fn known_application_survives_interrupted_base_adoption() {
    let mut f = setup_model(true);
    edit(&mut f, true);
    f.device.base_ready.set(false);
    let report = publication::report(
        &finish(
            &mut f.runtime,
            publication::save(&f.client, f.buffer.state, &[]),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(report.status, protocol::Status::Applied);
    let original = f.device.current.borrow().clone();
    assert_eq!(
        f.client.close(report.work).into_inner(),
        Err(RequestError::Rejected(Error::BUSY))
    );
    let status = finish(
        &mut f.runtime,
        f.client.read(
            report.work,
            &Message::new(Contract::new("kernel.request.status", 1), &[]),
        ),
    )
    .unwrap();
    assert_eq!(status.data.as_ref(), b"pending");
    f.device.base_ready.set(true);
    let recovered = publication::report(
        &finish(&mut f.runtime, publication::reconcile(&f.client, report.work))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(recovered.status, protocol::Status::Applied);
    assert!(Rc::ptr_eq(&original, &f.device.current.borrow()));
    retire(&mut f, report.work);
    stop(f);
}

#[test]
fn save_does_not_grant_its_private_readers_to_a_write_only_caller() {
    let mut f = setup_model(true);
    edit(&mut f, true);
    let (caller, selected) = writer(&mut f, Permissions::WRITE);
    let buffer = selected.buffer;
    let report = publication::report(
        &finish(&mut f.runtime, publication::save(&caller, selected, &[]))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(report.status, protocol::Status::Applied);
    assert_eq!(
        finish(
            &mut f.runtime,
            caller.read(
                buffer,
                &Message::new(Contract::new("logical.value", 1), &[])
            )
        ),
        Err(RequestError::Rejected(Error::DENIED))
    );
    close_work(&mut f.runtime, &caller, report.work);
    drop(caller);
    stop(f);
}

fn writer(
    f: &mut Fixture,
    source_rights: Permissions,
) -> (Connection, buffer::BufferState) {
    let parent = f.client.resolve("buffers").into_inner().unwrap();
    let entry = f
        .client
        .children(parent)
        .into_inner()
        .unwrap()
        .into_iter()
        .find(|entry| entry.resource == f.buffer.state.buffer)
        .unwrap();
    let buffer =
        f.runtime.resource(&format!("buffers/{}", entry.name)).unwrap();
    let source = f.runtime.resource("target").unwrap();
    let caller = reovim_sdk::local(
        f.runtime
            .connect(&[(buffer, Permissions::WRITE), (source, source_rights)])
            .unwrap(),
    );
    let buffer = caller
        .resolve(&format!("buffers/{}", entry.name))
        .into_inner()
        .unwrap();
    let selected =
        buffer::BufferState { buffer, revision: f.buffer.state.revision };
    (caller, selected)
}

#[test]
fn buffer_write_does_not_replace_target_write_authority() {
    let mut f = setup_model(true);
    edit(&mut f, true);
    let (caller, selected) = writer(&mut f, Permissions::READ);
    assert_eq!(
        finish(&mut f.runtime, publication::save(&caller, selected, &[])),
        Err(RequestError::Rejected(Error::DENIED))
    );
    assert!(!f.device.current.borrow().active);
    drop(caller);
    stop(f);
}

#[test]
fn another_save_uses_the_proved_base_from_the_previous_save() {
    let mut f = setup_model(true);
    edit(&mut f, true);
    let first = publication::report(
        &finish(
            &mut f.runtime,
            publication::save(&f.client, f.buffer.state, &[]),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(first.status, protocol::Status::Applied);
    let original = f.device.current.borrow().clone();
    retire(&mut f, first.work);
    edit(&mut f, false);
    let second = publication::report(
        &finish(
            &mut f.runtime,
            publication::save(&f.client, f.buffer.state, &[]),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(second.status, protocol::Status::Applied);
    assert!(!f.device.current.borrow().active);
    assert!(f.device.current.borrow().generation > original.generation);
    retire(&mut f, second.work);
    stop(f);
}

#[test]
fn queued_cancellation_retires_without_backend_handoff() {
    let mut f = setup_model(true);
    edit(&mut f, true);
    let (pending, work) = pending_save(&f);
    finish(
        &mut f.runtime,
        f.client.write(
            work,
            &Message::new(Contract::new("kernel.request.cancel", 1), &[]),
        ),
    )
    .unwrap();
    assert_eq!(
        finish(&mut f.runtime, pending),
        Err(RequestError::Failed(Error::CANCELLED))
    );
    assert!(!f.device.current.borrow().active);
    assert!(
        finish(
            &mut f.runtime,
            f.client.read(
                work,
                &Message::new(Contract::new("kernel.request.status", 1), &[])
            )
        )
        .is_err()
    );
    stop(f);
}

#[test]
fn save_captures_before_queueing_and_marks_only_selected_history_published() {
    let mut f = setup_model(true);
    edit(&mut f, true);
    let selected = f.buffer.state;
    let waiting = publication::save(&f.client, selected, &[]);
    edit(&mut f, false);
    let report =
        publication::report(&finish(&mut f.runtime, waiting).unwrap()).unwrap();
    assert_eq!(report.selected, selected);
    assert_eq!(report.target, f.target);
    assert_eq!(report.status, protocol::Status::Applied);
    assert!(f.device.current.borrow().active);
    assert_eq!(f.buffer.value.data.as_ref(), [0]);
    assert_eq!(
        finish(
            &mut f.runtime,
            buffer::close(
                &f.client,
                f.buffer.state,
                buffer::ClosePolicy::RejectModified
            )
        ),
        Err(RequestError::Failed(Error::BUSY))
    );
    let undo = Message {
        contract: Contract::new("kernel.history.undo", 1),
        data: f.buffer.state.revision.to_le_bytes().into(),
        resources: Box::new([f.buffer.state.buffer]),
    };
    finish(&mut f.runtime, f.client.write(f.buffer.history, &undo)).unwrap();
    f.buffer = buffer::observation(
        finish(
            &mut f.runtime,
            f.client.read(
                f.buffer.state.buffer,
                &Message::new(Contract::new("logical.value", 1), &[]),
            ),
        )
        .unwrap(),
    )
    .unwrap();
    finish(
        &mut f.runtime,
        buffer::close(
            &f.client,
            f.buffer.state,
            buffer::ClosePolicy::RejectModified,
        ),
    )
    .unwrap();
    retire(&mut f, report.work);
    stop(f);
}

#[test]
fn unresolved_save_work_reconciles_without_republication_and_then_cleans_up() {
    let mut f = setup_model(true);
    edit(&mut f, true);
    f.device.uncertain.set(true);
    let report = publication::report(
        &finish(
            &mut f.runtime,
            publication::save(&f.client, f.buffer.state, &[]),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(matches!(report.status, protocol::Status::OutcomeUnknown(_)));
    assert_eq!(
        f.client.resolve(&report.recovery).into_inner().unwrap(),
        report.work
    );
    assert_eq!(
        f.client.close(report.work).into_inner(),
        Err(RequestError::Rejected(Error::BUSY))
    );
    let committed = f.device.current.borrow().clone();
    f.device.reconcile.set(false);
    assert_eq!(
        finish(&mut f.runtime, publication::reconcile(&f.client, report.work)),
        Err(RequestError::Failed(Error::UNSUPPORTED))
    );
    let unchanged = finish(
        &mut f.runtime,
        reovim_kernel_interface::requests::result(&f.client, report.work),
    )
    .unwrap()
    .unwrap();
    assert_eq!(publication::report(&unchanged).unwrap(), report);
    f.device.reconcile.set(true);
    let refined = publication::report(
        &finish(&mut f.runtime, publication::reconcile(&f.client, report.work))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(refined.work, report.work);
    assert_eq!(refined.status, protocol::Status::Applied);
    assert!(Rc::ptr_eq(&committed, &f.device.current.borrow()));
    finish(
        &mut f.runtime,
        buffer::close(
            &f.client,
            f.buffer.state,
            buffer::ClosePolicy::RejectModified,
        ),
    )
    .unwrap();
    retire(&mut f, report.work);
    stop(f);
}

#[test]
fn stale_save_is_rejected_at_admission_without_an_effect() {
    let mut f = setup_model(true);
    let stale = f.buffer.state;
    edit(&mut f, true);
    assert_eq!(
        finish(&mut f.runtime, publication::save(&f.client, stale, &[])),
        Err(RequestError::Rejected(Error::STALE))
    );
    assert!(!f.device.current.borrow().active);
    stop(f);
}

fn pending_save(
    f: &Fixture,
) -> (reovim_kernel_interface::access::Reply, ResourceId) {
    let root = f.client.resolve("requests").into_inner().unwrap();
    let before = f.client.children(root).into_inner().unwrap();
    let pending = publication::save(&f.client, f.buffer.state, &[]);
    let work = f
        .client
        .children(root)
        .into_inner()
        .unwrap()
        .into_iter()
        .find(|entry| !before.iter().any(|old| old.resource == entry.resource))
        .unwrap()
        .resource;
    (pending, work)
}

#[test]
fn cancellation_respects_the_effect_handoff() {
    for after_handoff in [false, true] {
        let mut f = setup_model(true);
        edit(&mut f, true);
        f.device.ready.set(false);
        let (pending, work) = pending_save(&f);
        let mut context = Context::from_waker(Waker::noop());
        f.runtime.advance(&mut context).unwrap();
        if after_handoff {
            await_backend(&mut f, &mut context);
        }
        finish(
            &mut f.runtime,
            f.client.write(
                work,
                &Message::new(Contract::new("kernel.request.cancel", 1), &[]),
            ),
        )
        .unwrap();
        f.device.ready.set(true);
        if let Some(wake) = f.device.wake.borrow_mut().take() {
            wake.wake();
        }
        let report =
            publication::report(&finish(&mut f.runtime, pending).unwrap())
                .unwrap();
        assert_eq!(
            report.status,
            if after_handoff {
                protocol::Status::Applied
            } else {
                protocol::Status::DefinitelyNotApplied(Error::CANCELLED.code)
            }
        );
        assert_eq!(f.device.current.borrow().active, after_handoff);
        retire(&mut f, report.work);
        stop(f);
    }
}

fn await_backend(f: &mut Fixture, context: &mut Context<'_>) {
    for _ in 0..64 {
        f.runtime.advance(context).unwrap();
        if f.device.wake.borrow().is_some() {
            return;
        }
    }
    panic!("backend was not reached");
}

#[test]
fn proof_close_retries_rejected_cleanup_without_losing_application() {
    let mut f = setup_model(true);
    edit(&mut f, true);
    let report = publication::report(
        &finish(
            &mut f.runtime,
            publication::save(&f.client, f.buffer.state, &[]),
        )
        .unwrap(),
    )
    .unwrap();
    let evidence = f.client.resolve("target/effect-1").into_inner().unwrap();
    let held = f.client.capture(evidence, Permissions::READ).unwrap();
    assert_eq!(
        f.client.close(report.work).into_inner(),
        Err(RequestError::Rejected(Error::BUSY))
    );
    let during_cleanup = publication::report(
        &finish(&mut f.runtime, publication::reconcile(&f.client, report.work))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(during_cleanup.status, protocol::Status::Applied);
    drop(held);
    retire(&mut f, report.work);
    stop(f);
}

#[test]
fn reconnect_recovers_the_original_save_with_fresh_scoped_ids() {
    let mut f = setup_model(true);
    edit(&mut f, true);
    let snapshots = f.client.resolve("snapshots").into_inner().unwrap();
    let snapshot = f
        .client
        .children(snapshots)
        .into_inner()
        .unwrap()
        .into_iter()
        .find(|entry| entry.resource == f.selected.id)
        .unwrap()
        .name;
    let pending = publication::save(&f.client, f.buffer.state, &[]);
    let next = f.client.reconnect().unwrap();
    let error = finish(&mut f.runtime, pending).unwrap_err();
    let RequestError::Unknown { recovery, .. } = error else {
        panic!("lost save delivery");
    };
    f.client = next;
    let work = f.client.resolve(&recovery).into_inner().unwrap();
    let report = publication::report(
        &finish(&mut f.runtime, publication::reconcile(&f.client, work))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(report.work, work);
    assert_eq!(report.status, protocol::Status::Applied);
    f.target = f.client.resolve("target").into_inner().unwrap();
    assert_eq!(report.target, f.target);
    f.selected.id = f
        .client
        .resolve(&format!("snapshots/{snapshot}"))
        .into_inner()
        .unwrap();
    f.buffer = buffer::observation(
        finish(
            &mut f.runtime,
            f.client.read(
                report.selected.buffer,
                &Message::new(Contract::new("logical.value", 1), &[]),
            ),
        )
        .unwrap(),
    )
    .unwrap();
    let original = f.device.current.borrow().clone();
    let repeated = publication::report(
        &finish(&mut f.runtime, publication::reconcile(&f.client, work))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(repeated, report);
    assert!(Rc::ptr_eq(&original, &f.device.current.borrow()));
    retire(&mut f, work);
    stop(f);
}
