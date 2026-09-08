use std::{cell::Cell, rc::Rc};

use reovim_kernel_interface::interaction::delivery::update as delivery;
use reovim_kernel_interface::interaction::identity;
use reovim_kernel_interface::interaction::identity::RepresentationUpdateCursor;
use reovim_kernel_interface::interaction::outcome::InteractionOutcomeStatus;
use reovim_kernel_interface::interaction::port::InteractionSessionPort;
use reovim_kernel_interface::interaction::root::InteractionRootPort;
use reovim_kernel_interface::interaction::update::{
    RepresentationUpdate, RepresentationUpdateIdentityStamp,
    RepresentationUpdateLineage, RepresentationUpdateStamp,
};
use reovim_kernel_interface::lifecycle::ShutdownOutcome;
use reovim_resource_local::{MemoryResource, profile};

use super::harness::{Harness, build};
use super::providers::SnapshotMode;

#[test]
fn structurally_valid_wrong_gap_token_does_not_claim_recovery_ownership() {
    let mode = Rc::new(Cell::new(SnapshotMode::Good));
    let mut service = build(core::convert::identity, Rc::clone(&mode), false);
    service.boot();
    mode.set(SnapshotMode::Failure);
    assert_eq!(
        service.edit(1, b"b").status(),
        InteractionOutcomeStatus::Accepted
    );
    let gap = service.next_update();
    let forged = wrong_token(&service, &gap);
    let forged = request(&service, &forged);
    assert!(matches!(
        service.port().request_resynchronization(forged),
        delivery::RepresentationResynchronizationAdmission::NotAdmitted {
            cause: delivery::RepresentationResynchronizationFailure::UnknownGap,
            ..
        }
    ));
    let receipt = admit(&mut service, &gap);
    mode.set(SnapshotMode::Good);
    service.runtime.progress(profile::count(1)).unwrap();
    let snapshot = take(&mut service, receipt.cursor().value() + 1);
    receipt.validate_result(&snapshot).unwrap();
    assert_eq!(snapshot.carrier().unwrap().bytes(), b"H\x01b");
    assert_eq!(
        service.runtime.shutdown(service.lifecycle()).unwrap(),
        ShutdownOutcome::Closed
    );
}

fn wrong_token(
    service: &Harness<MemoryResource>,
    gap: &RepresentationUpdate,
) -> RepresentationUpdate {
    let RepresentationUpdateLineage::Gap {
        expected,
        available,
        cause,
        resynchronization,
    } = gap.lineage()
    else {
        panic!("actual qualified Gap");
    };
    let session = &service.client.session;
    let attachment = &service.client.attachment;
    let state = gap.stamp().resolve_target(session, attachment).unwrap();
    let projection =
        gap.stamp().resolve_projection(session, attachment).unwrap();
    let stamp = RepresentationUpdateStamp::new(
        session,
        attachment,
        gap.stamp().update(),
        gap.stamp().cursor(),
        &state,
        &projection,
        &attachment.coordinate(),
    )
    .unwrap();
    RepresentationUpdate::gap(
        session,
        attachment,
        stamp,
        expected,
        available,
        cause,
        identity::RepresentationResynchronizationIdentity::new(
            resynchronization.value() + 1,
        ),
    )
    .unwrap()
}

#[test]
fn failed_or_replaced_recovery_returns_exact_terminal_then_allows_retry() {
    for mode in [SnapshotMode::Failure, SnapshotMode::OtherSchema] {
        failure_then_retry(mode);
    }
}

fn failure_then_retry(failure: SnapshotMode) {
    use delivery::RepresentationResynchronizationFailure as Failure;
    let mode = Rc::new(Cell::new(SnapshotMode::Good));
    let mut service = build(core::convert::identity, Rc::clone(&mode), false);
    service.boot();
    let old = exhaust_data(&mut service);
    mode.set(SnapshotMode::Failure);
    let accepted = service.edit(100, b"r");
    assert_eq!(accepted.status(), InteractionOutcomeStatus::Accepted);
    let gap = service.next_update();
    let receipt = admit(&mut service, &gap);
    assert_eq!(
        ack(&mut service, &old),
        delivery::RepresentationUpdateAcknowledgementResult::UnknownUpdate
    );
    let duplicate = request(&service, &gap);
    assert!(matches!(
        service.port().request_resynchronization(duplicate),
        delivery::RepresentationResynchronizationAdmission::NotAdmitted {
            cause: Failure::DuplicateRequest,
            ..
        }
    ));
    mode.set(failure);
    service.runtime.progress(profile::count(1)).unwrap();
    let terminal = take(&mut service, receipt.cursor().value() + 1);
    receipt.validate_result(&terminal).unwrap();
    let expected = if failure == SnapshotMode::OtherSchema {
        Failure::Replaced
    } else {
        Failure::RuntimeFailure
    };
    assert_eq!(
        terminal.lineage(),
        RepresentationUpdateLineage::ResynchronizationFailed {
            resynchronization: receipt.resynchronization(),
            cause: expected
        }
    );
    assert!(terminal.carrier().is_none());
    ack(&mut service, &terminal.stamp().identity());
    let stale = request(&service, &gap);
    assert!(matches!(
        service.port().request_resynchronization(stale),
        delivery::RepresentationResynchronizationAdmission::NotAdmitted {
            cause: Failure::UnknownGap,
            ..
        }
    ));
    let next_gap = take(&mut service, terminal.stamp().cursor().value() + 1);
    let next = admit(&mut service, &next_gap);
    assert_ne!(next.resynchronization(), receipt.resynchronization());
    mode.set(SnapshotMode::Good);
    service.runtime.progress(profile::count(1)).unwrap();
    let snapshot = take(&mut service, next.cursor().value() + 1);
    next.validate_result(&snapshot).unwrap();
    assert!(receipt.validate_result(&snapshot).is_err());
    assert_eq!(snapshot.carrier().unwrap().bytes(), b"H\x01r");
    assert_eq!(
        snapshot
            .stamp()
            .resolve_target(&service.client.session, &service.client.attachment)
            .unwrap(),
        accepted.accepted_state().unwrap().result()
    );
    assert_eq!(
        service.runtime.shutdown(service.lifecycle()).unwrap(),
        ShutdownOutcome::Closed
    );
}

#[test]
fn close_keeps_pending_recovery_until_closing_control_transfers() {
    let mode = Rc::new(Cell::new(SnapshotMode::Good));
    let mut service = build(core::convert::identity, Rc::clone(&mode), false);
    service.boot();
    exhaust_data(&mut service);
    mode.set(SnapshotMode::Failure);
    assert_eq!(
        service.edit(100, b"b").status(),
        InteractionOutcomeStatus::Accepted
    );
    let gap = service.next_update();
    let receipt = admit(&mut service, &gap);
    assert!(matches!(
        service.runtime.shutdown(service.lifecycle()).unwrap(),
        ShutdownOutcome::IncompleteShutdown
    ));
    let terminal = take(&mut service, receipt.cursor().value() + 1);
    receipt.validate_result(&terminal).unwrap();
    assert_eq!(
        terminal.lineage(),
        RepresentationUpdateLineage::ResynchronizationFailed {
            resynchronization: receipt.resynchronization(),
            cause: delivery::RepresentationResynchronizationFailure::Closing,
        }
    );
    assert!(terminal.carrier().is_none());
    assert_eq!(
        service.runtime.shutdown(service.lifecycle()).unwrap(),
        ShutdownOutcome::Closed
    );
}

fn exhaust_data(
    service: &mut Harness<MemoryResource>,
) -> RepresentationUpdateIdentityStamp {
    let mut last = None;
    let credit = service.client.attachment.initial_credit().value();
    for id in 1..=credit {
        assert_eq!(
            service.edit(u128::from(id), b"b").status(),
            InteractionOutcomeStatus::Accepted
        );
        let update = service.next_update();
        last = Some(update.stamp().identity());
        service.client.install(update).unwrap();
    }
    last.expect("selected nonzero data credit exhausted without ACKs")
}

fn request(
    service: &Harness<MemoryResource>,
    gap: &RepresentationUpdate,
) -> delivery::RepresentationResynchronizationRequest {
    delivery::RepresentationResynchronizationRequest::new(
        &service.client.session,
        &service.client.attachment,
        gap,
    )
    .unwrap()
}

fn admit(
    service: &mut Harness<MemoryResource>,
    gap: &RepresentationUpdate,
) -> delivery::RepresentationResynchronizationReceipt {
    let request = request(service, gap);
    assert_eq!(
        ack(service, &gap.stamp().identity()),
        delivery::RepresentationUpdateAcknowledgementResult::Accepted
    );
    let replay = take(service, gap.stamp().cursor().value());
    assert_eq!(replay.stamp().identity(), gap.stamp().identity());
    assert_eq!(replay.lineage(), gap.lineage());
    assert_eq!(
        ack(service, &replay.stamp().identity()),
        delivery::RepresentationUpdateAcknowledgementResult::AlreadyRecorded
    );
    let delivery::RepresentationResynchronizationAdmission::Admitted(receipt) =
        service.port().request_resynchronization(request)
    else {
        panic!("exact delivered Gap admitted");
    };
    receipt
}

fn ack(
    service: &mut Harness<MemoryResource>,
    stamp: &RepresentationUpdateIdentityStamp,
) -> delivery::RepresentationUpdateAcknowledgementResult {
    let request = delivery::RepresentationUpdateAcknowledgement::new(
        &service.client.session,
        &service.client.attachment,
        *stamp,
        delivery::RepresentationUpdateApplication::Installed,
    )
    .unwrap();
    service.port().acknowledge_update(request)
}

fn take(
    service: &mut Harness<MemoryResource>,
    cursor: u64,
) -> RepresentationUpdate {
    let request = delivery::RepresentationUpdateRequest::new(
        &service.client.session,
        &service.client.attachment,
        RepresentationUpdateCursor::new(cursor),
    )
    .unwrap();
    service
        .port()
        .take_update(request)
        .into_update()
        .expect("exact owned terminal or independent control")
}
