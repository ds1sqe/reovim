use super::*;
use reovim_kernel_interface::{publication, requests};

#[test]
fn repaired_atomicity_violation_settles_without_becoming_a_successful_save() {
    let mut f = setup_model(true);
    save::edit(&mut f, true);
    f.device.partial.set(true);
    let original = publication::report(
        &finish(
            &mut f.runtime,
            publication::save(&f.client, f.buffer.state, &[1]),
        )
        .unwrap(),
    )
    .unwrap();
    let violation = protocol::Status::PartiallyApplied {
        cause: Error::UNAVAILABLE.code,
        atomicity_violated: true,
    };
    assert_eq!(original.status, violation);
    let primary = f.device.current.borrow().clone();
    assert!(primary.active);
    assert!(!f.device.mirror.get());
    assert_eq!(pair(&mut f), [1, 0]);
    assert_eq!(
        f.client.close(original.work).into_inner(),
        Err(RequestError::Rejected(Error::BUSY))
    );
    let key = f.runtime.resource(&original.recovery).unwrap();
    let root = f.runtime.register_namespace("recovery").unwrap();
    let owner = f.runtime.bind_at(root, &[(key, Permissions::ALL)]).unwrap();
    let recovery = owner.connection();
    let work = recovery.resolve(&original.recovery).into_inner().unwrap();
    finish(&mut f.runtime, requests::adopt(&recovery, work)).unwrap();
    f.device.settlement.set(false);
    let repaired = publication::report(
        &finish(&mut f.runtime, publication::reconcile(&recovery, work))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(repaired.status, violation);
    assert!(f.device.mirror.get());
    assert_eq!(pair(&mut f), [1, 1]);
    assert!(Rc::ptr_eq(&primary, &f.device.current.borrow()));
    assert_eq!(
        recovery.close(work).into_inner(),
        Err(RequestError::Rejected(Error::BUSY))
    );
    assert_eq!(f.runtime.retire_client(&owner), Err(Error::BUSY));
    f.device.settlement.set(true);
    reject_malformed_settlement(&mut f, &recovery, work);
    let settled = publication::report(
        &finish(&mut f.runtime, publication::reconcile(&recovery, work))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(settled, repaired);
    assert_eq!(original.status, violation);
    let status = finish(
        &mut f.runtime,
        recovery.read(
            work,
            &Message::new(Contract::new("kernel.request.status", 1), &[]),
        ),
    )
    .unwrap();
    assert_eq!(status.data.as_ref(), b"finished");
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
    f.runtime.retire_client(&owner).unwrap();
    old_base_still_conflicts(&mut f);
    stop(f);
}

fn old_base_still_conflicts(f: &mut Fixture) {
    let result = publication::report(
        &finish(
            &mut f.runtime,
            publication::save(&f.client, f.buffer.state, &[]),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        result.status,
        protocol::Status::DefinitelyNotApplied(Error::STALE.code)
    );
    save::close_work(&mut f.runtime, &f.client, result.work);
}

fn reject_malformed_settlement(
    f: &mut Fixture,
    recovery: &Connection,
    work: ResourceId,
) {
    f.device.invalid_settlement.set(true);
    assert_eq!(
        finish(&mut f.runtime, publication::reconcile(recovery, work)),
        Err(RequestError::Failed(Error::INVALID))
    );
    assert_eq!(
        recovery.close(work).into_inner(),
        Err(RequestError::Rejected(Error::BUSY))
    );
    f.device.invalid_settlement.set(false);
}

fn pair(f: &mut Fixture) -> Vec<u8> {
    finish(
        &mut f.runtime,
        f.client.read(
            f.target,
            &Message::new(Contract::new("logical.pair", 1), &[]),
        ),
    )
    .unwrap()
    .data
    .into_vec()
}

#[test]
fn atomic_visibility_mode_publishes_both_values_on_its_normal_path() {
    let mut f = setup_model(true);
    save::edit(&mut f, true);
    let report = publication::report(
        &finish(
            &mut f.runtime,
            publication::save(&f.client, f.buffer.state, &[1]),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(report.status, protocol::Status::Applied);
    assert_eq!(pair(&mut f), [1, 1]);
    save::close_work(&mut f.runtime, &f.client, report.work);
    stop(f);
}
