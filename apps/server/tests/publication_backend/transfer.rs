use super::*;
use reovim_kernel_interface::{publication, requests};

fn buffer_key(f: &Fixture) -> ResourceId {
    let root = f.client.resolve("buffers").into_inner().unwrap();
    let entry = f
        .client
        .children(root)
        .into_inner()
        .unwrap()
        .into_iter()
        .find(|entry| entry.resource == f.buffer.state.buffer)
        .unwrap();
    f.runtime.resource(&format!("buffers/{}", entry.name)).unwrap()
}

#[test]
fn adopted_save_outlives_its_retired_client_without_republication() {
    transfer(false);
    transfer(true);
}

fn transfer(partial: bool) {
    let mut f = setup_model(true);
    save::edit(&mut f, true);
    f.device.uncertain.set(!partial);
    f.device.partial.set(partial);
    let buffer = buffer_key(&f);
    let source = f.runtime.resource("target").unwrap();
    let root = f.runtime.register_namespace("temporary").unwrap();
    let temporary = f
        .runtime
        .bind_at(
            root,
            &[(buffer, Permissions::ALL), (source, Permissions::WRITE)],
        )
        .unwrap();
    f.runtime.mount(&temporary, "buffer", buffer).unwrap();
    let caller = temporary.connection();
    let selected = buffer::BufferState {
        buffer: caller.resolve("buffer").into_inner().unwrap(),
        revision: f.buffer.state.revision,
    };
    let original = publication::report(
        &finish(&mut f.runtime, publication::save(&caller, selected, &[]))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        original.status,
        if partial {
            protocol::Status::PartiallyApplied {
                cause: Error::UNAVAILABLE.code,
                atomicity_violated: false,
            }
        } else {
            protocol::Status::OutcomeUnknown(Error::UNAVAILABLE.code)
        }
    );
    let applied = f.device.current.borrow().clone();
    assert!(applied.active);
    assert_eq!(f.device.mirror.get(), !partial);
    assert_eq!(f.runtime.retire_client(&temporary), Err(Error::BUSY));
    let work_key = f.runtime.resource(&original.recovery).unwrap();
    let root = f.runtime.register_namespace("recovery").unwrap();
    let custodian =
        f.runtime.bind_at(root, &[(work_key, Permissions::ALL)]).unwrap();
    let recovery = custodian.connection();
    let work = recovery.resolve(&original.recovery).into_inner().unwrap();
    finish(&mut f.runtime, requests::adopt(&recovery, work)).unwrap();
    finish(&mut f.runtime, requests::adopt(&recovery, work)).unwrap();
    f.runtime.retire_client(&temporary).unwrap();
    assert_eq!(
        caller.resolve("buffer").into_inner(),
        Err(RequestError::Rejected(Error::CLOSED))
    );
    assert_eq!(f.runtime.retire_client(&custodian), Err(Error::BUSY));
    save::edit(&mut f, false);
    let report = publication::report(
        &finish(&mut f.runtime, publication::reconcile(&recovery, work))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(report.status, protocol::Status::Applied);
    assert!(f.device.mirror.get());
    assert_eq!(report.recovery, original.recovery);
    assert!(Rc::ptr_eq(&applied, &f.device.current.borrow()));
    assert_ne!(original.status, report.status);
    assert_eq!(
        finish(
            &mut f.runtime,
            buffer::close(
                &f.client,
                f.buffer.state,
                buffer::ClosePolicy::RejectModified,
            ),
        ),
        Err(RequestError::Failed(Error::BUSY))
    );
    save::close_work(&mut f.runtime, &recovery, work);
    f.runtime.retire_client(&custodian).unwrap();
    stop(f);
}

#[test]
fn adoption_rejects_missing_authority_and_active_readers() {
    let mut f = setup_model(true);
    save::edit(&mut f, true);
    f.device.uncertain.set(true);
    let original = publication::report(
        &finish(
            &mut f.runtime,
            publication::save(&f.client, f.buffer.state, &[]),
        )
        .unwrap(),
    )
    .unwrap();
    let key = f.runtime.resource(&original.recovery).unwrap();
    for rights in [Permissions::READ, Permissions::WRITE] {
        let binding = f.runtime.bind(&[(key, rights)]).unwrap();
        let caller = binding.connection();
        let work = caller.resolve(&original.recovery).into_inner().unwrap();
        assert_eq!(
            finish(&mut f.runtime, requests::adopt(&caller, work)),
            Err(RequestError::Rejected(Error::DENIED))
        );
    }
    let reading = f.client.read(
        original.work,
        &Message::new(Contract::new("kernel.request.result", 1), &[]),
    );
    assert_eq!(
        finish(&mut f.runtime, requests::adopt(&f.client, original.work)),
        Err(RequestError::Rejected(Error::BUSY))
    );
    finish(&mut f.runtime, reading).unwrap();
    let pending = publication::reconcile(&f.client, original.work);
    assert_eq!(
        finish(&mut f.runtime, requests::adopt(&f.client, original.work)),
        Err(RequestError::Rejected(Error::BUSY))
    );
    finish(&mut f.runtime, pending).unwrap();
    finish(&mut f.runtime, requests::adopt(&f.client, original.work)).unwrap();
    save::close_work(&mut f.runtime, &f.client, original.work);
    stop(f);
}

#[test]
fn adoption_preserves_private_buffer_retirement_obligations() {
    let mut f = setup_model(true);
    let model = f.runtime.resource("model").unwrap();
    let source = f.runtime.resource("target").unwrap();
    let root = f.runtime.register_namespace("temporary").unwrap();
    let temporary = f
        .runtime
        .bind_at(root, &[(model, Permissions::ALL), (source, Permissions::ALL)])
        .unwrap();
    f.runtime.mount(&temporary, "model", model).unwrap();
    f.runtime.mount(&temporary, "source", source).unwrap();
    let caller = temporary.connection();
    let model = caller.resolve("model").into_inner().unwrap();
    let source = caller.resolve("source").into_inner().unwrap();
    let opened = buffer::descriptor(
        &finish(&mut f.runtime, buffer::open(&caller, model, source, &[]))
            .unwrap(),
    )
    .unwrap();
    f.device.uncertain.set(true);
    let original = publication::report(
        &finish(&mut f.runtime, publication::save(&caller, opened.state, &[]))
            .unwrap(),
    )
    .unwrap();
    let key = f.runtime.resource(&original.recovery).unwrap();
    let recovery = f.runtime.bind(&[(key, Permissions::ALL)]).unwrap();
    let access = recovery.connection();
    let work = access.resolve(&original.recovery).into_inner().unwrap();
    finish(&mut f.runtime, requests::adopt(&access, work)).unwrap();
    assert_eq!(f.runtime.retire_client(&temporary), Err(Error::BUSY));
    let report = publication::report(
        &finish(&mut f.runtime, publication::reconcile(&access, work)).unwrap(),
    )
    .unwrap();
    assert_eq!(report.status, protocol::Status::Applied);
    assert_eq!(f.runtime.retire_client(&temporary), Err(Error::BUSY));
    finish(
        &mut f.runtime,
        buffer::close(
            &caller,
            opened.state,
            buffer::ClosePolicy::RejectModified,
        ),
    )
    .unwrap();
    save::close_work(&mut f.runtime, &access, work);
    f.runtime.retire_client(&temporary).unwrap();
    stop(f);
}
