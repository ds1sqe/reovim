use super::*;
use reovim_kernel_interface::publication;

#[test]
fn shutdown_drains_original_save_and_rejects_unrelated_admission() {
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
    f.client.close(f.selected.id).into_inner().unwrap();
    let applied = f.device.current.borrow().clone();
    assert_eq!(f.runtime.close(), Err(Error::BUSY));
    assert_eq!(
        finish(
            &mut f.runtime,
            publication::save(&f.client, f.buffer.state, &[]),
        ),
        Err(RequestError::Rejected(Error::CLOSED))
    );
    let recovered = publication::report(
        &finish(
            &mut f.runtime,
            publication::reconcile(&f.client, original.work),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(recovered.status, protocol::Status::Applied);
    assert!(Rc::ptr_eq(&applied, &f.device.current.borrow()));
    save::close_work(&mut f.runtime, &f.client, original.work);
    drain(&mut f.runtime);
}

fn drain(runtime: &mut Runtime) {
    for _ in 0..128 {
        match runtime.close() {
            Ok(()) => return,
            Err(error) => assert_eq!(error, Error::BUSY),
        }
        runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    }
    panic!("settled save prevented shutdown");
}

#[test]
fn queued_save_can_prepare_and_publish_evidence_during_shutdown() {
    queued(false);
}

#[test]
fn queued_atomicity_violation_can_repair_and_settle_during_shutdown() {
    queued(true);
}

fn queued(violation: bool) {
    let mut f = setup_model(true);
    save::edit(&mut f, true);
    f.device.partial.set(violation);
    let pending = publication::save(&f.client, f.buffer.state, &[1]);
    f.client.close(f.selected.id).into_inner().unwrap();
    assert_eq!(f.runtime.close(), Err(Error::BUSY));
    assert!(matches!(f.client.read_view(f.target), Err(Error::CLOSED)));
    assert!(matches!(
        f.client.capture(f.target, Permissions::READ),
        Err(Error::CLOSED)
    ));
    let original =
        publication::report(&finish(&mut f.runtime, pending).unwrap()).unwrap();
    let applied = f.device.current.borrow().clone();
    assert!(applied.active);
    let expected = if violation {
        protocol::Status::PartiallyApplied {
            cause: Error::UNAVAILABLE.code,
            atomicity_violated: true,
        }
    } else {
        protocol::Status::Applied
    };
    assert_eq!(original.status, expected);
    let recovered = publication::report(
        &finish(
            &mut f.runtime,
            publication::reconcile(&f.client, original.work),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(recovered.status, expected);
    assert!(f.device.mirror.get());
    assert!(Rc::ptr_eq(&applied, &f.device.current.borrow()));
    save::close_work(&mut f.runtime, &f.client, original.work);
    drain(&mut f.runtime);
}
