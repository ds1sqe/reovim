use reovim_kernel_interface::cancellation::DeadlineState;
use reovim_kernel_interface::event::editing::EditingStage::{
    Admitted, Committed, Installed, Prepared, Received, UpdateReady,
};
use reovim_kernel_interface::event::trace::{
    SemanticCaptureKey, TraceCut, TraceError, TraceLocation, TraceReadPort,
    TraceStart,
};
use reovim_kernel_interface::interaction::identity::InteractionRequestIdentity;
use reovim_resource_local::profile;
use reovim_server::assembly::life;

#[path = "tracing/harness.rs"]
mod harness;
use harness::{
    budget, edit, grant, key, limits, report, service, stage, stages,
};

#[test]
fn actual_request_fanout_preserves_origin_observers_and_explicit_joins() {
    let mut service = service(harness::telemetry(Some(limits())));
    edit(&mut service, 7, b"b");
    let report = report(&service, 7);
    assert_eq!(report.cut, TraceCut::Complete);
    assert_eq!(
        stages(&report),
        [
            Admitted,
            Prepared,
            Committed,
            UpdateReady,
            UpdateReady,
            Received,
            Installed,
            Received,
            Installed,
        ]
    );
    let origin = service.clients[0].session.stamp().handle();
    assert!(report.nodes.iter().all(|node| {
        node.context.session == Some(origin)
            && node.context.request == Some(InteractionRequestIdentity::new(7))
    }));
    let admitted = stage(&report, Admitted);
    let prepared = stage(&report, Prepared);
    let committed = stage(&report, Committed);
    assert_eq!(admitted.context.parent, None);
    assert_eq!(prepared.context.parent, Some(admitted.location.sequence));
    assert_eq!(committed.context.parent, Some(prepared.location.sequence));
    for client in &service.clients {
        let observer = client.session.stamp().handle();
        let nodes = report
            .nodes
            .iter()
            .filter(|node| node.context.observer == Some(observer))
            .copied()
            .collect::<Vec<_>>();
        let ready =
            nodes.iter().find(|node| node.stage == UpdateReady).unwrap();
        let received =
            nodes.iter().find(|node| node.stage == Received).unwrap();
        let installed =
            nodes.iter().find(|node| node.stage == Installed).unwrap();
        assert_eq!(ready.context.parent, Some(committed.location.sequence));
        assert_eq!(received.context.parent, Some(ready.location.sequence));
        assert_eq!(installed.context.parent, Some(received.location.sequence));
        assert_eq!(
            installed.context.additional_parent,
            Some(committed.location.sequence)
        );
        assert_eq!(ready.context.update, received.context.update);
        assert_eq!(received.context.update, installed.context.update);
    }
    assert!(report.nodes.iter().all(|node| {
        [node.context.parent, node.context.additional_parent]
            .into_iter()
            .flatten()
            .all(|parent| parent.value() < node.location.sequence.value())
    }));
    let mistaken_origin = TraceStart::Request {
        session: service.clients[1].session.stamp().handle(),
        request: InteractionRequestIdentity::new(7),
    };
    assert_eq!(
        service
            .telemetry
            .backtrace(grant(), mistaken_origin, budget())
            .unwrap()
            .cut,
        TraceCut::NotCaptured
    );
    service.close().unwrap();
}

#[test]
fn reverse_event_and_update_queries_follow_real_selected_edges() {
    let mut service = service(harness::telemetry(Some(limits())));
    edit(&mut service, 7, b"b");
    let all = report(&service, 7);
    let installed = stage(&all, Installed);
    let event = TraceStart::Event(installed.location);
    let update = TraceStart::Update(installed.context.update.unwrap());
    for start in [event, update] {
        let selected =
            service.telemetry.backtrace(grant(), start, budget()).unwrap();
        assert_eq!(selected.cut, TraceCut::Complete);
        assert_eq!(
            stages(&selected),
            [Admitted, Prepared, Committed, UpdateReady, Received, Installed,]
        );
        assert!(selected.nodes.iter().all(|node| {
            node.context.observer.is_none()
                || node.context.observer == installed.context.observer
        }));
    }
    let mut selected = budget();
    selected.primary = false;
    selected.joins = false;
    let isolated =
        service.telemetry.backtrace(grant(), event, selected).unwrap();
    assert_eq!(isolated.nodes, [installed]);
    assert_eq!(isolated.cut, TraceCut::Complete);
    selected.joins = true;
    let joined = service.telemetry.backtrace(grant(), event, selected).unwrap();
    assert_eq!(stages(&joined), [Committed, Installed]);
    assert_eq!(joined.cut, TraceCut::Complete);
    selected.primary = true;
    selected.joins = false;
    let primary =
        service.telemetry.backtrace(grant(), event, selected).unwrap();
    assert_eq!(
        stages(&primary),
        [Admitted, Prepared, Committed, UpdateReady, Received, Installed,]
    );
    service.close().unwrap();
}

#[test]
fn semantic_capture_keeps_old_state_schema_and_projection_after_later_edit() {
    let mut service = service(harness::telemetry(Some(limits())));
    edit(&mut service, 7, b"b");
    let old = key(&stage(&report(&service, 7), UpdateReady));
    let capture = service
        .telemetry
        .captured(grant(), &old, profile::bytes(4096))
        .unwrap();
    assert_eq!(capture.payload.as_ref(), [b'H', 1, b'b']);
    assert_eq!(capture.stamp.state, service.clients[0].state().unwrap());
    assert_eq!(capture.stamp.update, old.update);
    assert_eq!(
        capture.stamp.projection,
        service.clients[0].attachment.projection()
    );
    assert_eq!(
        capture.stamp.coordinate,
        service.clients[0].attachment.coordinate()
    );
    assert_eq!(
        capture.stamp.schema,
        service.clients[0].session.update_contract().selected_stamp()
    );
    edit(&mut service, 8, b"c");
    assert_eq!(service.clients[0].payload(), Some([b'H', 1, b'c'].as_slice()));
    let retained = service
        .telemetry
        .captured(grant(), &old, profile::bytes(4096))
        .unwrap();
    assert_eq!(retained.stamp, capture.stamp);
    assert_eq!(retained.payload, capture.payload);
    // Ring eviction must not erase independently retained graph/state.
    assert!(service.telemetry.health().evicted > 0);
    assert_eq!(report(&service, 7).cut, TraceCut::Complete);
    assert_eq!(
        service
            .telemetry
            .installed_update(
                service.clients[1].session.stamp().handle(),
                old.update,
                &old.state,
            )
            .unwrap_err(),
        TraceError::Invalid
    );
    let wrong_state = SemanticCaptureKey {
        state: service.clients[0].state().unwrap(),
        ..old
    };
    assert_eq!(
        service
            .telemetry
            .captured(grant(), &wrong_state, profile::bytes(4096))
            .unwrap_err(),
        TraceError::NotCaptured
    );
    assert_eq!(
        service
            .telemetry
            .installed_update(
                service.clients[0].session.stamp().handle(),
                old.update,
                &wrong_state.state,
            )
            .unwrap_err(),
        TraceError::Invalid
    );
    assert_eq!(
        service
            .telemetry
            .captured(life(91), &old, profile::bytes(4096))
            .unwrap_err(),
        TraceError::Denied
    );
    assert_eq!(
        service
            .telemetry
            .captured(grant(), &old, profile::bytes(0))
            .unwrap_err(),
        TraceError::Pressure
    );
    service.close().unwrap();
}

#[test]
fn graph_queries_obey_each_budget_and_do_not_disclose_under_wrong_grants() {
    let mut service = service(harness::telemetry(Some(limits())));
    edit(&mut service, 7, b"b");
    let installed = stage(&report(&service, 7), Installed);
    let start = TraceStart::Event(installed.location);
    let mut nodes = budget();
    nodes.nodes = profile::count(1);
    let mut edges = budget();
    edges.edges = profile::count(0);
    let mut depth = budget();
    depth.depth = profile::count(0);
    let mut work = budget();
    work.work = profile::count(0);
    for restricted in [nodes, edges, depth, work] {
        assert_eq!(
            service
                .telemetry
                .backtrace(grant(), start, restricted)
                .unwrap()
                .cut,
            TraceCut::Budget
        );
    }
    let mut bytes = budget();
    bytes.bytes = profile::bytes(0);
    assert_eq!(
        service.telemetry.backtrace(grant(), start, bytes).unwrap_err(),
        TraceError::Pressure
    );
    let mut deadline = budget();
    deadline.deadline = DeadlineState::Reached;
    assert_eq!(
        service.telemetry.backtrace(grant(), start, deadline).unwrap_err(),
        TraceError::Deadline
    );
    assert_eq!(
        service.telemetry.backtrace(life(91), start, budget()).unwrap_err(),
        TraceError::Denied
    );
    let wrong_stream = TraceStart::Event(TraceLocation {
        stream: life(95),
        ..installed.location
    });
    assert_eq!(
        service
            .telemetry
            .backtrace(grant(), wrong_stream, budget())
            .unwrap_err(),
        TraceError::Replaced
    );
    service.close().unwrap();
}

#[test]
fn graph_and_capture_eviction_are_separate_visible_losses() {
    let mut bounded = limits();
    bounded.nodes = profile::count(8);
    bounded.snapshots = profile::count(2);
    let mut service = service(harness::telemetry(Some(bounded)));
    edit(&mut service, 7, b"b");
    let first = report(&service, 7);
    let old_event = stage(&first, Committed).location;
    let old = key(&stage(&first, UpdateReady));
    assert_eq!(
        service
            .telemetry
            .captured(grant(), &old, profile::bytes(4096))
            .unwrap()
            .payload
            .as_ref(),
        [b'H', 1, b'b']
    );
    edit(&mut service, 8, b"c");
    assert_eq!(
        service
            .telemetry
            .captured(grant(), &old, profile::bytes(4096))
            .unwrap_err(),
        TraceError::Evicted
    );
    let disappeared = service
        .telemetry
        .backtrace(grant(), TraceStart::Event(old_event), budget())
        .unwrap();
    assert_eq!(disappeared.cut, TraceCut::Evicted);
    let latest = report(&service, 8);
    assert_eq!(latest.cut, TraceCut::Evicted);
    assert!(latest.gaps.iter().any(|gap| gap.cause == TraceCut::Evicted));
    let newest = key(&stage(&latest, UpdateReady));
    assert_eq!(
        service
            .telemetry
            .captured(grant(), &newest, profile::bytes(4096))
            .unwrap()
            .payload
            .as_ref(),
        [b'H', 1, b'c']
    );
    let health = service.telemetry.trace_health().unwrap();
    assert!(health.evicted_nodes > 0);
    assert!(health.evicted_snapshots > 0);
    assert!(
        service.telemetry.trace_resident_bytes().unwrap() as u64
            <= bounded.metadata_bytes.value() + bounded.payload_bytes.value()
    );
    service.close().unwrap();
}

#[test]
fn capture_pressure_or_disabled_tracing_never_changes_edit_acceptance() {
    let mut bounded = limits();
    bounded.payload_bytes = profile::bytes(2);
    let mut pressured = service(harness::telemetry(Some(bounded)));
    edit(&mut pressured, 7, b"b");
    let produced = key(&stage(&report(&pressured, 7), UpdateReady));
    assert_eq!(
        pressured
            .telemetry
            .captured(grant(), &produced, profile::bytes(4096))
            .unwrap_err(),
        TraceError::NotCaptured
    );
    assert!(pressured.telemetry.trace_health().unwrap().lost > 0);
    assert_eq!(
        pressured.clients[0].payload(),
        Some([b'H', 1, b'b'].as_slice())
    );
    pressured.close().unwrap();

    let telemetry = harness::telemetry(Some(limits()));
    telemetry.set_enabled(false);
    let mut disabled = service(telemetry);
    edit(&mut disabled, 7, b"b");
    assert_eq!(report(&disabled, 7).cut, TraceCut::NotCaptured);
    assert_eq!(disabled.telemetry.trace_health().unwrap().lost, 0);
    disabled.telemetry.set_enabled(true);
    edit(&mut disabled, 8, b"c");
    assert_eq!(report(&disabled, 8).cut, TraceCut::Complete);
    disabled.close().unwrap();

    let mut unselected = service(harness::telemetry(None));
    edit(&mut unselected, 7, b"b");
    assert_eq!(
        unselected
            .telemetry
            .backtrace(grant(), harness::request(&unselected, 7), budget())
            .unwrap_err(),
        TraceError::NotCaptured
    );
    assert_eq!(unselected.telemetry.trace_health(), None);
    unselected.close().unwrap();
}

#[test]
fn evicted_producer_and_receipt_count_lost_real_client_observations() {
    let mut bounded = limits();
    bounded.nodes = profile::count(2);
    let mut service = service(harness::telemetry(Some(bounded)));
    let outcome = service.edit(0, 7, b"b").unwrap();
    let accepted = outcome.accepted_state().unwrap().result();
    let before = service.telemetry.trace_health().unwrap().lost;
    // The second observer still has its producer. Receiving it evicts the
    // first observer's producer before that client consumes the same edit.
    service.receive(1).unwrap();
    assert_eq!(service.telemetry.trace_health().unwrap().lost, before);
    service.receive(0).unwrap();
    // Received lacks UpdateReady; Installed then lacks Received. These are
    // two lost observations, not two rejected editing operations.
    assert_eq!(service.telemetry.trace_health().unwrap().lost, before + 2);
    assert_eq!(service.clients[0].state(), Some(accepted));
    assert_eq!(service.clients[1].state(), Some(accepted));
    assert_eq!(service.clients[0].payload(), Some([b'H', 1, b'b'].as_slice()));
    assert_eq!(service.clients[1].payload(), service.clients[0].payload());
    service.close().unwrap();
}

#[test]
fn installed_stage_does_not_hide_an_evicted_required_join() {
    let mut bounded = limits();
    bounded.nodes = profile::count(1);
    let mut service = service(harness::telemetry(Some(bounded)));
    let outcome = service.edit(0, 7, b"b").unwrap();
    let accepted = outcome.accepted_state().unwrap().result();
    let ready = stage(&report(&service, 7), UpdateReady);
    let before = service.telemetry.trace_health().unwrap().lost;
    // Received replaces UpdateReady. Installation still succeeds locally,
    // but the graph must not claim a complete Installed node without its join.
    service.receive(1).unwrap();
    assert_eq!(service.clients[1].state(), Some(accepted));
    assert_eq!(service.telemetry.trace_health().unwrap().lost, before + 1);
    let mut joins = budget();
    joins.primary = false;
    let retained = service
        .telemetry
        .backtrace(
            grant(),
            TraceStart::Update(ready.context.update.unwrap()),
            joins,
        )
        .unwrap();
    assert_eq!(stages(&retained), [Received]);
    assert_eq!(retained.cut, TraceCut::Complete);
    service.receive(0).unwrap();
    service.close().unwrap();
}
