use std::rc::Rc;

use reovim_host_reference::DeterministicClock;
use reovim_kernel_interface::clock::ClockDescriptor;
use reovim_kernel_interface::interaction::delivery::{
    intent::InteractionOutcomeRequest,
    update::{
        RepresentationUpdateAcknowledgement,
        RepresentationUpdateAcknowledgementResult,
        RepresentationUpdateApplication, RepresentationUpdateRequest,
    },
};
use reovim_kernel_interface::interaction::descriptor::{
    InteractionCapabilitySupport, InteractionSessionCapabilities,
    InteractionSessionDescriptor, RepresentationAttachmentDescriptor,
};
use reovim_kernel_interface::interaction::history::InteractionHistoryAction;
use reovim_kernel_interface::interaction::identity::RepresentationUpdateCursor;
use reovim_kernel_interface::interaction::port::InteractionSessionPort;
use reovim_kernel_interface::interaction::update::{
    RepresentationUpdate, RepresentationUpdateBindingOutcome,
    RepresentationUpdateLineage, RepresentationUpdatePayloadClaim,
    RepresentationUpdateStamp,
};
use reovim_kernel_interface::lifecycle::LifecycleIdentity;
use reovim_kernel_runtime::diagnostics::Telemetry;
use reovim_resource_local::{Key, profile};
use reovim_server::assembly::{self, KeyedService, life};
use reovim_server::client::{
    ClientError, KeySet, PartialAcceptance, ResponsePolicy, SelectionError,
    SelectionStatus, TranslationStatus, selection,
};
use reovim_server::driver::DriverError;

#[test]
fn follow_and_keep_pair_actual_evidence_in_either_delivery_order() {
    let mut service = keyed(21);
    service.clients[0]
        .select_keys(KeySet::both(), ResponsePolicy::Follow)
        .unwrap();
    service.clients[1]
        .select_keys(KeySet::one(Key::Omega), ResponsePolicy::Keep)
        .unwrap();
    let kept = service.clients[1].selection().unwrap().clone();
    let original = service.clients[0].state().unwrap();
    let result =
        service.edit(0, 10, &[17]).unwrap().accepted_state().unwrap().result();

    assert_eq!(service.receive_update(0).unwrap(), None);
    assert_eq!(service.receive_change(1).unwrap(), None);
    assert_eq!(service.clients[0].state(), Some(original));
    assert_eq!(service.clients[1].state(), Some(original));
    assert!(service.receive_change(0).unwrap().is_some());
    assert!(service.receive_update(1).unwrap().is_some());
    assert_eq!(service.clients[0].state(), Some(result));
    assert_eq!(service.clients[1].state(), Some(result));
    assert_eq!(
        service.clients[0].selection_status(),
        Some(SelectionStatus::Current)
    );
    assert_eq!(
        service.clients[0].selection().unwrap().keys(),
        Some(KeySet::both())
    );
    assert_eq!(service.clients[1].selection(), Some(&kept));
    assert_eq!(
        service.clients[1].selection_status(),
        Some(SelectionStatus::Stale)
    );
    assert!(matches!(
        service.clients[1].selection_intent(11, &[93]),
        Err(ClientError::Selection(SelectionError::Stale))
    ));

    // A fresh explicit semantic edit is independent of kept local selection.
    let later =
        service.edit(1, 11, &[93]).unwrap().accepted_state().unwrap().result();
    service.receive(0).unwrap();
    service.receive(1).unwrap();
    assert_eq!(service.clients[1].state(), Some(later));
    assert_eq!(service.clients[1].selection(), Some(&kept));
    service.clients[1].revalidate_selection().unwrap();
    assert_eq!(
        service.clients[1].selection_status(),
        Some(SelectionStatus::Current)
    );
    assert!(service.clients[1].selection_intent(12, &[93]).is_ok());
    service.close().unwrap();
}

#[test]
fn follow_and_keep_remain_independent_across_actual_undo_and_redo() {
    let mut service = keyed(28);
    service.clients[0]
        .select_keys(KeySet::both(), ResponsePolicy::Follow)
        .unwrap();
    service.clients[1]
        .select_keys(KeySet::one(Key::Omega), ResponsePolicy::Keep)
        .unwrap();
    let kept = service.clients[1].selection().unwrap().clone();
    let root = service.clients[0].state().unwrap().history().node();
    let edit = service.edit(0, 10, &[17]).unwrap();
    let child = edit.accepted_state().unwrap().result().history().node();
    service.receive(0).unwrap();
    service.receive(1).unwrap();
    assert_eq!(service.clients[0].payload(), Some(&[b'K', 17, 1, 93, 1][..]));
    assert_eq!(service.clients[1].selection(), Some(&kept));

    for (request, action, node, alpha) in [
        (11, InteractionHistoryAction::Undo, root, 0),
        (12, InteractionHistoryAction::Redo { child: Some(child) }, child, 1),
    ] {
        let before = service.clients[0].state().unwrap();
        let outcome = service.history(1, request, action).unwrap();
        let accepted = outcome.accepted_state().unwrap();
        let current = accepted.result();
        assert_eq!(current.history().node(), node);
        assert_ne!(current.version(), before.version());
        assert_eq!(service.receive_update(0).unwrap(), None);
        assert_eq!(service.receive_change(1).unwrap(), None);
        assert_eq!(service.clients[0].state(), Some(before));
        assert_eq!(service.clients[1].state(), Some(before));
        assert!(service.receive_change(0).unwrap().is_some());
        assert!(service.receive_update(1).unwrap().is_some());
        assert_eq!(service.clients[0].state(), Some(current));
        assert_eq!(service.clients[1].state(), Some(current));
        assert_eq!(
            service.clients[0].payload(),
            Some(&[b'K', 17, alpha, 93, 1][..])
        );
        assert_eq!(service.clients[1].payload(), service.clients[0].payload());
        assert_eq!(
            service.clients[0].selection_status(),
            Some(SelectionStatus::Current)
        );
        assert_eq!(service.clients[0].selection().unwrap().source(), current);
        assert_eq!(
            service.clients[0].selection().unwrap().keys(),
            Some(KeySet::both())
        );
        assert_eq!(service.clients[1].selection(), Some(&kept));
        assert_eq!(
            service.clients[1].selection_status(),
            Some(SelectionStatus::Stale)
        );
    }
    service.close().unwrap();
}

#[test]
fn explicit_partial_consent_preserves_omitted_meaning_and_kernel_state() {
    let mut service = keyed(22);
    service.clients[0]
        .select_keys(KeySet::both(), ResponsePolicy::Follow)
        .unwrap();
    let state = service.clients[0].state();
    let original = service.clients[0].selection().unwrap().clone();
    let partial =
        service.clients[0].translate_selection(selection::alpha_schema());
    assert_eq!(partial.status(), TranslationStatus::Partial);
    assert_eq!(partial.omitted(), Some(KeySet::one(Key::Omega)));
    assert_eq!(
        partial.selected().unwrap().keys(),
        Some(KeySet::one(Key::Alpha))
    );
    assert_eq!(
        service.clients[0]
            .accept_translation(partial, PartialAcceptance::Reject),
        Err(SelectionError::PartialNeedsConsent)
    );
    assert_eq!(service.clients[0].selection(), Some(&original));
    let accepted =
        service.clients[0].translate_selection(selection::alpha_schema());
    service.clients[0]
        .accept_translation(accepted, PartialAcceptance::Accept)
        .unwrap();
    assert_eq!(
        service.clients[0].selection().unwrap().schema(),
        selection::alpha_schema()
    );
    assert_eq!(service.clients[0].state(), state);
    assert_eq!(service.clients[1].state(), state);

    service.clients[1]
        .select_keys(KeySet::one(Key::Omega), ResponsePolicy::Keep)
        .unwrap();
    let missing =
        service.clients[1].translate_selection(selection::alpha_schema());
    assert_eq!(missing.status(), TranslationStatus::Unrepresentable);
    assert_eq!(
        service.clients[1]
            .accept_translation(missing, PartialAcceptance::Accept),
        Err(SelectionError::Unrepresentable)
    );
    let unsupported =
        service.clients[1].translate_selection(profile::schema(0x9999));
    assert_eq!(unsupported.status(), TranslationStatus::Unsupported);
    service.clients[0]
        .select_keys(KeySet::one(Key::Alpha), ResponsePolicy::Follow)
        .unwrap();
    let exact =
        service.clients[0].translate_selection(selection::alpha_schema());
    assert_eq!(exact.status(), TranslationStatus::Exact);
    service.clients[0]
        .accept_translation(exact, PartialAcceptance::Reject)
        .unwrap();
    assert_eq!(service.clients[0].state(), state);
    service.close().unwrap();
}

#[test]
fn an_actual_outcome_can_join_a_snapshot_before_duplicate_notification() {
    let mut service = keyed(23);
    service.clients[0]
        .select_keys(KeySet::both(), ResponsePolicy::Follow)
        .unwrap();
    let intent = service.clients[0].intent(10, &[17]).unwrap();
    let expected = service.clients[0].expect_edit(&intent).unwrap();
    let lookup = InteractionOutcomeRequest::new(
        &service.clients[0].session,
        intent.stamp(),
    )
    .unwrap();
    assert!(
        service
            .endpoint(0)
            .unwrap()
            .submit_intent(intent)
            .non_admission()
            .is_none()
    );
    service.progress().unwrap();
    let outcome =
        service.endpoint(0).unwrap().take_outcome(lookup).outcome().unwrap();
    assert_eq!(service.receive_update(0).unwrap(), None);
    let identity = service.clients[0]
        .accept_edit_outcome(&expected, &outcome)
        .unwrap()
        .expect("actual acceptance completes pending snapshot");
    let ack = RepresentationUpdateAcknowledgement::new(
        &service.clients[0].session,
        &service.clients[0].attachment,
        identity,
        RepresentationUpdateApplication::Installed,
    )
    .unwrap();
    assert_eq!(
        service.endpoint(0).unwrap().acknowledge_update(ack),
        RepresentationUpdateAcknowledgementResult::Accepted
    );
    assert_eq!(
        service.clients[0].selection_status(),
        Some(SelectionStatus::Current)
    );
    // Notification delivery has its own frontier, not the snapshot's version.
    assert_eq!(service.receive_change(0).unwrap(), None);
    service.receive(1).unwrap();
    service.edit(0, 11, &[93]).unwrap();
    service.receive(0).unwrap();
    service.receive(1).unwrap();
    assert_eq!(service.clients[0].payload(), Some(&[b'K', 17, 1, 93, 0][..]));
    service.close().unwrap();
}

#[test]
fn wrong_stream_and_old_updates_cannot_install_or_move_selection() {
    let mut service = keyed(24);
    service.clients[0]
        .select_keys(KeySet::both(), ResponsePolicy::Follow)
        .unwrap();
    let original = service.clients[0].state();
    service.edit(0, 10, &[17]).unwrap();
    let update = take_update(&mut service, 0);
    let foreign = copy_update(&service, 0, &update);
    let old = copy_update(&service, 0, &update);
    let delta = delta_update(&service, &update);
    assert_eq!(
        service.clients[0].accept_update(delta),
        Err(ClientError::Binding)
    );
    assert_eq!(service.clients[0].state(), original);
    assert_eq!(
        service.clients[1].accept_update(foreign),
        Err(ClientError::Binding)
    );
    assert_eq!(service.clients[1].state(), original);
    assert_eq!(service.clients[0].accept_update(update).unwrap(), None);
    service.receive_change(0).unwrap();
    let current = service.clients[0].state();
    let selected = service.clients[0].selection().unwrap().clone();
    assert_eq!(service.clients[0].accept_update(old), Err(ClientError::Stale));
    assert_eq!(service.clients[0].state(), current);
    assert_eq!(service.clients[0].selection(), Some(&selected));
    service.receive(1).unwrap();
    service.close().unwrap();
}

#[test]
fn a_slow_client_recovers_real_gap_without_relabeling_its_kept_selection() {
    let mut service = keyed(25);
    service.clients[1]
        .select_keys(KeySet::both(), ResponsePolicy::Keep)
        .unwrap();
    let kept = service.clients[1].selection().unwrap().clone();
    let retention = service.clients[1]
        .session
        .bounds()
        .updates()
        .retained_updates()
        .aggregate()
        .value();
    // Cross the actual selected retention limit; compare semantic recovery,
    // not an expected record count or serialized length.
    for request in 1..=retention + 1 {
        assert!(
            service
                .edit(0, u128::from(request), &[17])
                .unwrap()
                .accepted_state()
                .is_some()
        );
        service.receive(0).unwrap();
    }
    let expected = service.clients[0].payload().unwrap().to_vec();
    let current = service.clients[0].state();
    assert_eq!(
        service.receive(1),
        Err(DriverError::Client(ClientError::Invalidated))
    );
    assert_eq!(service.clients[1].payload(), None);
    service.recover(1).unwrap();
    assert_eq!(service.clients[1].state(), current);
    assert_eq!(service.clients[1].payload(), Some(expected.as_slice()));
    assert_eq!(service.clients[1].selection(), Some(&kept));
    assert_eq!(
        service.clients[1].selection_status(),
        Some(SelectionStatus::Stale)
    );
    service.clients[1].revalidate_selection().unwrap();
    assert_eq!(
        service.clients[1].selection_status(),
        Some(SelectionStatus::Current)
    );
    assert!(service.edit(0, 100, &[93]).unwrap().accepted_state().is_some());
    service.receive(0).unwrap();
    service.receive(1).unwrap();
    assert_eq!(service.clients[1].payload(), service.clients[0].payload());
    service.close().unwrap();
}

#[test]
fn lost_unacknowledged_data_does_not_block_a_forward_gap_and_real_recovery() {
    let mut service = keyed(26);
    let initial = service.clients[1].state();
    let mut cursor = service.clients[1].cursor();
    let credits = service.clients[1].attachment.initial_credit().value();
    for request in 1..=credits {
        service.edit(0, u128::from(request), &[17]).unwrap();
        service.receive(0).unwrap();
        cursor += 1;
        let lost = take_at(&mut service, 1, cursor);
        assert_eq!(lost.stamp().cursor().value(), cursor);
        drop(lost); // Actual transferred frame is lost; deliberately no ACK.
    }
    let retention = service.clients[1]
        .session
        .bounds()
        .updates()
        .retained_updates()
        .aggregate()
        .value();
    for request in credits + 1..=credits + retention + 1 {
        service.edit(0, u128::from(request), &[93]).unwrap();
        service.receive(0).unwrap();
    }
    assert_eq!(service.clients[1].state(), initial);
    let current = service.clients[0].state();
    let expected = service.clients[0].payload().unwrap().to_vec();
    service.recover(1).unwrap();
    assert_eq!(service.clients[1].state(), current);
    assert_eq!(service.clients[1].payload(), Some(expected.as_slice()));
    assert!(service.clients[1].cursor() > cursor);
    let restored = service
        .history(0, 100, InteractionHistoryAction::Undo)
        .unwrap()
        .accepted_state()
        .unwrap()
        .result();
    service.receive(0).unwrap();
    service.receive(1).unwrap();
    assert_eq!(service.clients[1].state(), Some(restored));
    assert_eq!(service.clients[1].payload(), service.clients[0].payload());
    assert!(service.edit(1, 101, &[17]).unwrap().accepted_state().is_some());
    service.receive(0).unwrap();
    service.receive(1).unwrap();
    service.close().unwrap();
}

#[test]
fn heading_follow_revalidates_meaning_and_refuses_stale_translation() {
    let owner = life(27);
    let mut service =
        assembly::heading(owner, b"## a\n\xff".to_vec(), telemetry(owner))
            .unwrap();
    service.open(0).unwrap();
    service.observe_changes(0, profile::count(1)).unwrap();
    service.clients[0].select_heading_label(ResponsePolicy::Follow).unwrap();
    let original = service.clients[0].selection().unwrap().clone();
    let old_translation =
        service.clients[0].translate_selection(selection::heading_schema());
    assert_eq!(old_translation.status(), TranslationStatus::Exact);
    let accepted =
        service.edit(0, 10, b"b").unwrap().accepted_state().unwrap().result();
    service.receive(0).unwrap();
    assert_eq!(
        service.clients[0].selection_status(),
        Some(SelectionStatus::Current)
    );
    assert_eq!(service.clients[0].selection().unwrap().source(), accepted);
    assert_ne!(service.clients[0].selection(), Some(&original));
    assert_eq!(service.clients[0].payload(), Some(&b"H\x02b"[..]));
    let current = service.clients[0].selection().unwrap().clone();
    assert_eq!(
        service.clients[0]
            .accept_translation(old_translation, PartialAcceptance::Accept),
        Err(SelectionError::Stale)
    );
    assert_eq!(service.clients[0].selection(), Some(&current));
    service.close().unwrap();
}

fn telemetry(owner: LifecycleIdentity) -> Rc<Telemetry> {
    let clock = Rc::new(DeterministicClock::new(
        ClockDescriptor::new(
            profile::schema(0x8001),
            life(800),
            owner,
            1,
            u64::MAX,
            0x8002,
        )
        .unwrap(),
    ));
    Rc::new(Telemetry::new(owner, life(801), clock, None, 64).unwrap())
}

fn keyed(owner: u128) -> KeyedService {
    let owner = life(owner);
    let mut service =
        assembly::keyed(owner, vec![17, 0, 93, 1], telemetry(owner)).unwrap();
    service.open(0).unwrap();
    service.open(1).unwrap();
    service.observe_changes(0, profile::count(1)).unwrap();
    service.observe_changes(1, profile::count(1)).unwrap();
    service
}

fn take_update(
    service: &mut KeyedService,
    index: usize,
) -> RepresentationUpdate {
    let next = service.clients[index].cursor() + 1;
    take_at(service, index, next)
}

fn take_at(
    service: &mut KeyedService,
    index: usize,
    cursor: u64,
) -> RepresentationUpdate {
    let client = &service.clients[index];
    let request = RepresentationUpdateRequest::new(
        &client.session,
        &client.attachment,
        RepresentationUpdateCursor::new(cursor),
    )
    .unwrap();
    service.endpoint(index).unwrap().take_update(request).into_update().unwrap()
}

fn copy_update(
    service: &KeyedService,
    index: usize,
    update: &RepresentationUpdate,
) -> RepresentationUpdate {
    let client = &service.clients[index];
    copy_frame(&client.session, &client.attachment, update, update.lineage())
}

fn delta_update(
    service: &KeyedService,
    update: &RepresentationUpdate,
) -> RepresentationUpdate {
    let client = &service.clients[0];
    let original = client.session.capabilities();
    let capabilities = InteractionSessionCapabilities::new(
        original.cancellation(),
        original.reconciliation(),
        original.updates(),
        InteractionCapabilitySupport::Supported,
        original.resynchronization(),
    )
    .unwrap();
    // A peer's wider contract does not authorize this Client to apply deltas.
    let peer = InteractionSessionDescriptor::new(
        client.session.stamp(),
        client.session.scope(),
        capabilities,
        client.session.bounds().clone(),
        client.session.update_contract().clone(),
        client.session.operations(),
    )
    .unwrap();
    let RepresentationUpdateLineage::FullSnapshot { supersedes, result } =
        update.lineage()
    else {
        panic!("actual full snapshot");
    };
    copy_frame(
        &peer,
        &client.attachment,
        update,
        RepresentationUpdateLineage::Delta { predecessor: supersedes, result },
    )
}

fn copy_frame(
    session: &InteractionSessionDescriptor,
    attachment: &RepresentationAttachmentDescriptor,
    update: &RepresentationUpdate,
    lineage: RepresentationUpdateLineage,
) -> RepresentationUpdate {
    let original = update.stamp();
    let state = original.resolve_target(session, attachment).unwrap();
    let projection = original.resolve_projection(session, attachment).unwrap();
    let stamp = RepresentationUpdateStamp::new(
        session,
        attachment,
        original.update(),
        original.cursor(),
        &state,
        &projection,
        &attachment.coordinate(),
    )
    .unwrap();
    let carrier = update.carrier().unwrap();
    let claim = RepresentationUpdatePayloadClaim::new(
        session.update_contract().selected_stamp(),
        carrier.shape().items().value(),
        carrier.shape().depth().value(),
        carrier.shape().bytes().value(),
    );
    let prepared =
        session.prepare_update(attachment, stamp, lineage, claim).unwrap();
    let RepresentationUpdateBindingOutcome::Bound(bound) =
        prepared.bind(carrier.bytes().into())
    else {
        panic!("duplicate of an actual qualified public update");
    };
    bound.into_update()
}
