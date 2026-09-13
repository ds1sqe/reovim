use crate::{remote::Remote, session::Session};
use reovim_client_tui::operation::Operation;
use reovim_kernel_interface::ResourceId;
use reovim_service::transport;
use std::{io::BufReader, net::TcpListener, thread};

const CLIENT: &str = "0123456789abcdef0123456789abcdef";
const OBSERVATION: &str = concat!(
    "ok\ttext.observe\t1\t",
    "0000000000000000",
    "0000000000000000",
    "0000000000000000",
    "\t@70,@70,@1,@4,@80,@81",
);
const EMPTY_WINDOW: &str = concat!(
    "ok\ttext.window\t1\t01",
    "0000000000000000",
    "0000000000000000",
    "\t",
);

fn fixture(
    script: Vec<(&'static str, Option<&'static str>)>,
) -> (Session, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let worker = thread::spawn(move || serve(&listener, script));
    let session = Session {
        remote: Remote {
            address,
            client: CLIENT.into(),
            pending: None,
            reserved: None,
        },
        buffer: ResourceId(1),
        access: None,
        operation: Operation::Idle,
        observed: None,
        retiring: Vec::new(),
        accepted: None,
        cancel: std::sync::Arc::default(),
        journal: None,
    };
    (session, worker)
}

fn serve(listener: &TcpListener, script: Vec<(&str, Option<&str>)>) {
    for (expected, reply) in script {
        let (mut stream, _) = listener.accept().unwrap();
        let request = transport::receive(&mut BufReader::new(&stream)).unwrap();
        assert_eq!(request, format!("v1\t{CLIENT}\t{expected}"));
        if let Some(reply) = reply {
            transport::send(&mut stream, reply).unwrap();
        }
    }
}

#[test]
fn lost_receipt_cleanup_keeps_the_adopted_attachment() {
    let (mut session, worker) = fixture(vec![
        ("reserve", Some("ok\t9")),
        (
            "call\t9\twrite\t@2\ttext.attach\t1\t\t@1",
            Some("ok\ttext.attach\t1\t\t@70"),
        ),
        ("release\t9", None),
        ("release\t9", Some("ok")),
        ("reserve", Some("ok\t10")),
        ("call\t10\tclose\t@70", Some("ok")),
        ("release\t10", Some("ok")),
    ]);
    assert!(session.attach(ResourceId(2)).is_err());
    assert_eq!(session.access, Some(ResourceId(70)));
    assert_eq!(session.remote.pending.as_ref().unwrap().id(), 9);
    session.close().unwrap();
    assert!(session.remote.pending.is_none());
    assert_eq!(session.access, None);
    worker.join().unwrap();
}

#[test]
fn rejected_attachment_cleanup_precedes_explicit_startup_retry() {
    let (mut session, worker) = fixture(vec![
        ("reserve", Some("ok\t9")),
        (
            "call\t9\twrite\t@2\ttext.attach\t1\t\t@1",
            Some("error\trejected\t2\t\t"),
        ),
        ("release\t9", None),
        ("release\t9", Some("ok")),
        ("reserve", Some("ok\t10")),
        ("call\t10\twrite\t@2\ttext.attach\t1\t\t@1", None),
    ]);
    assert!(session.attach(ResourceId(2)).is_err());
    assert!(session.refresh(ResourceId(2), 0).is_err());
    assert_eq!(session.remote.pending.as_ref().unwrap().id(), 9);
    assert!(session.recover().is_err());
    assert_eq!(session.operation, Operation::Idle);
    assert_eq!(session.access, None);
    assert!(session.remote.pending.is_none());
    assert!(session.refresh(ResourceId(2), 0).is_err());
    assert_eq!(session.operation, Operation::Attach(ResourceId(2)));
    assert_eq!(session.remote.pending.as_ref().unwrap().id(), 10);
    worker.join().unwrap();
}

#[test]
fn known_attachment_rejection_clears_startup_intent() {
    let (mut session, worker) = fixture(vec![
        ("reserve", Some("ok\t9")),
        (
            "call\t9\twrite\t@2\ttext.attach\t1\t\t@1",
            Some("error\trejected\t2\t\t"),
        ),
        ("release\t9", Some("ok")),
    ]);
    assert!(session.attach(ResourceId(2)).is_err());
    assert_eq!(session.operation, Operation::Idle);
    assert_eq!(session.access, None);
    assert!(session.remote.pending.is_none());
    worker.join().unwrap();
}

#[test]
fn worker_shutdown_reports_unresolved_attachment_as_failure() {
    let (mut session, server) = fixture(vec![
        ("reserve", Some("ok\t9")),
        ("call\t9\twrite\t@2\ttext.attach\t1\t\t@1", None),
    ]);
    let (send, receive) = std::sync::mpsc::channel();
    let (reply, results) = std::sync::mpsc::channel();
    send.send(crate::Action::Close).unwrap();
    let result =
        crate::worker::run(&mut session, ResourceId(2), &receive, &reply);
    assert!(result.is_err());
    let crate::Notice::Outcome(response) = results.recv().unwrap() else {
        panic!("unexpected capture without journal");
    };
    assert!(response.result.is_err());
    assert_eq!(session.operation, Operation::Attach(ResourceId(2)));
    assert_eq!(session.remote.pending.as_ref().unwrap().id(), 9);
    server.join().unwrap();
}

#[test]
fn lost_close_receipt_cleanup_does_not_repeat_resource_close() {
    let (mut session, worker) = fixture(vec![
        ("reserve", Some("ok\t11")),
        ("call\t11\tclose\t@70", Some("ok")),
        ("release\t11", None),
        ("release\t11", Some("ok")),
    ]);
    session.retiring.push(ResourceId(70));
    assert!(session.close().is_err());
    assert_eq!(session.retiring, [] as [ResourceId; 0]);
    session.close().unwrap();
    worker.join().unwrap();
}

#[test]
fn shutdown_recovers_original_close_and_then_its_lost_cleanup() {
    let (mut session, worker) = fixture(vec![
        ("reserve", Some("ok\t50")),
        ("call\t50\tclose\t@70", None),
        ("result\t50", Some("ok")),
        ("release\t50", None),
        ("release\t50", Some("ok")),
    ]);
    session.access = Some(ResourceId(70));
    assert!(session.close().is_err());
    assert_eq!(session.operation, Operation::Closing);
    assert_eq!(session.access, None);
    assert_eq!(session.retiring, [ResourceId(70)]);
    assert!(session.close().is_err());
    assert_eq!(session.operation, Operation::Closing);
    assert_eq!(session.retiring, [] as [ResourceId; 0]);
    session.close().unwrap();
    assert_eq!(session.operation, Operation::Idle);
    assert!(session.remote.pending.is_none());
    worker.join().unwrap();
}

#[test]
fn undecoded_result_cannot_be_adopted_by_teardown() {
    let (mut session, worker) = fixture(vec![
        ("reserve", Some("ok\t12")),
        ("call\t12\tread\t@2\tinvalid\t1", Some("ok\twrong\t1\t\t@70")),
    ]);
    let result = session.remote.call(
        &["read".into(), "@2".into(), "invalid".into(), "1".into()],
        |_| Err::<(), _>(std::io::Error::other("foreign semantic result")),
    );
    assert!(result.is_err());
    assert!(session.close().is_err());
    assert_eq!(
        session.remote.pending.as_ref().unwrap().response(),
        Some("ok\twrong\t1\t\t@70")
    );
    worker.join().unwrap();
}

#[test]
fn unsent_reservation_cleanup_retries_without_a_call() {
    let (mut session, worker) =
        fixture(vec![("release\t13", None), ("release\t13", Some("ok"))]);
    session.remote.reserved = Some(13);
    assert!(session.close().is_err());
    assert_eq!(session.remote.reserved, Some(13));
    session.close().unwrap();
    assert_eq!(session.remote.reserved, None);
    worker.join().unwrap();
}

#[test]
fn attachment_recovery_keeps_identity_across_result_and_cleanup_loss() {
    let (mut session, worker) = fixture(vec![
        ("reserve", Some("ok\t20")),
        ("call\t20\twrite\t@2\ttext.attach\t1\t\t@1", None),
        ("result\t20", Some("ok\ttext.attach\t1\t\t@70")),
        ("release\t20", None),
        ("release\t20", Some("ok")),
        ("reserve", Some("ok\t21")),
        ("call\t21\tread\t@70\ttext.observe\t1\t", None),
    ]);
    assert!(session.attach(ResourceId(2)).is_err());
    assert_eq!(session.access, None);
    assert!(session.recover_attachment().is_err());
    assert_eq!(session.access, Some(ResourceId(70)));
    assert_eq!(session.remote.pending.as_ref().unwrap().id(), 20);
    assert!(session.recover_attachment().is_err());
    assert_eq!(session.access, Some(ResourceId(70)));
    assert_eq!(session.operation, Operation::Observe { origin: 0 });
    assert_eq!(session.remote.pending.as_ref().unwrap().id(), 21);
    worker.join().unwrap();
}

#[test]
fn lost_observation_is_adopted_before_recovered_receipt_cleanup() {
    let (mut session, worker) = fixture(vec![
        ("reserve", Some("ok\t30")),
        ("call\t30\tread\t@70\ttext.observe\t1\t", None),
        ("result\t30", Some(OBSERVATION)),
        ("release\t30", None),
    ]);
    session.access = Some(ResourceId(70));
    assert!(session.observe(0).is_err());
    assert!(session.observe(0).is_err());
    assert!(session.recover().is_err());
    assert_eq!(session.observed.unwrap().snapshot(), ResourceId(80));
    assert_eq!(session.operation, Operation::Observe { origin: 0 });
    assert_eq!(session.remote.pending.as_ref().unwrap().id(), 30);
    worker.join().unwrap();
}

#[test]
fn lost_window_read_keeps_its_original_snapshot_and_range() {
    let (mut session, worker) = fixture(vec![
        ("reserve", Some("ok\t30")),
        ("call\t30\tread\t@70\ttext.observe\t1\t", Some(OBSERVATION)),
        ("release\t30", Some("ok")),
        ("reserve", Some("ok\t31")),
        (
            concat!(
                "call\t31\tread\t@80\ttext.window\t1\t",
                "00000000000000000004000000000000",
            ),
            None,
        ),
        ("result\t31", Some(EMPTY_WINDOW)),
        ("release\t31", None),
    ]);
    session.access = Some(ResourceId(70));
    assert!(session.observe(0).is_err());
    assert!(session.observe(0).is_err());
    assert_eq!(session.observed.unwrap().snapshot(), ResourceId(80));
    assert!(session.recover().is_err());
    assert_eq!(
        session.operation,
        Operation::Context { origin: 0, reading: Some((0, 1024)) }
    );
    assert_eq!(session.remote.pending.as_ref().unwrap().id(), 31);
    let saved = crate::checkpoint::encode(&session, ResourceId(2)).unwrap();
    let (restored, _) = crate::checkpoint::decode(&saved).unwrap();
    assert_eq!(restored.observed, session.observed);
    assert_eq!(restored.operation, session.operation);
    session.operation =
        Operation::Context { origin: 0, reading: Some((0, 1023)) };
    assert!(crate::checkpoint::encode(&session, ResourceId(2)).is_err());
    worker.join().unwrap();
}

#[test]
fn lost_history_result_and_cleanup_keep_direction_and_acceptance() {
    let (mut session, worker) = fixture(vec![
        ("reserve", Some("ok\t40")),
        (
            concat!(
                "call\t40\twrite\t@4\tkernel.history.undo\t1\t",
                "0000000000000000\t@1"
            ),
            None,
        ),
        (
            "result\t40",
            Some(concat!(
                "ok\tkernel.history.undo\t1\t",
                "01000000000000000000000000000000\t"
            )),
        ),
        ("release\t40", None),
        ("release\t40", Some("ok")),
        ("reserve", Some("ok\t41")),
        ("call\t41\tclose\t@80", None),
        ("result\t41", Some("ok")),
        ("release\t41", None),
        ("release\t41", Some("ok")),
        ("reserve", Some("ok\t42")),
        ("call\t42\tclose\t@81", Some("ok")),
        ("release\t42", Some("ok")),
        ("reserve", Some("ok\t43")),
        ("call\t43\tread\t@70\ttext.observe\t1\t", None),
    ]);
    session.access = Some(ResourceId(70));
    session.observed = Some(
        reovim_module_text::api::observation(
            &crate::remote::message(OBSERVATION).unwrap(),
            ResourceId(70),
        )
        .unwrap(),
    );
    assert!(session.history(false).is_err());
    assert!(session.history(true).is_err());
    assert!(session.recover().is_err());
    assert_eq!(session.accepted.unwrap().revision, 1);
    assert_eq!(session.remote.pending.as_ref().unwrap().id(), 40);
    assert!(session.recover().is_err());
    assert_eq!(session.accepted.unwrap().revision, 1);
    assert_eq!(session.remote.pending.as_ref().unwrap().id(), 41);
    assert_eq!(session.operation, Operation::Retire { origin: 0 });
    let saved = crate::checkpoint::encode(&session, ResourceId(2)).unwrap();
    let (restored, factory) = crate::checkpoint::decode(&saved).unwrap();
    assert_eq!(factory, ResourceId(2));
    session = restored;
    assert!(session.recover().is_err());
    assert_eq!(session.retiring, [ResourceId(81)]);
    assert!(session.recover().is_err());
    assert_eq!(session.retiring, [] as [ResourceId; 0]);
    assert_eq!(session.operation, Operation::Observe { origin: 0 });
    assert_eq!(session.accepted.unwrap().revision, 1);
    assert_eq!(session.remote.pending.as_ref().unwrap().id(), 43);
    worker.join().unwrap();
}

#[test]
fn checkpoint_restores_unknown_attachment_without_replaying_it() {
    let (mut session, worker) = fixture(vec![
        ("reserve", Some("ok\t60")),
        ("call\t60\twrite\t@2\ttext.attach\t1\t\t@1", None),
        ("result\t60", Some("ok\ttext.attach\t1\t\t@70")),
        ("release\t60", None),
    ]);
    assert!(session.attach(ResourceId(2)).is_err());
    let saved = crate::checkpoint::encode(&session, ResourceId(2)).unwrap();
    let (mut restored, _) = crate::checkpoint::decode(&saved).unwrap();
    let mut wrong_contract = saved;
    let offset = wrong_contract
        .windows(b"text.attach".len())
        .position(|bytes| bytes == b"text.attach")
        .unwrap();
    wrong_contract[offset..offset + b"text.attach".len()]
        .copy_from_slice(b"text.window");
    assert!(crate::checkpoint::decode(&wrong_contract).is_err());
    drop(session);
    assert!(restored.remote.pending.as_mut().unwrap().send().is_err());
    assert!(restored.recover().is_err());
    assert_eq!(restored.access, Some(ResourceId(70)));
    let saved = crate::checkpoint::encode(&restored, ResourceId(2)).unwrap();
    restored.access = Some(ResourceId(71));
    assert!(crate::checkpoint::encode(&restored, ResourceId(2)).is_err());
    let (restored, _) = crate::checkpoint::decode(&saved).unwrap();
    assert_eq!(restored.access, Some(ResourceId(70)));
    assert!(restored.remote.pending.as_ref().unwrap().release().is_ok());
    let mut trailing = saved.clone();
    trailing.push(0);
    assert!(crate::checkpoint::decode(&trailing).is_err());
    assert!(crate::checkpoint::decode(&saved[..saved.len() - 1]).is_err());
    worker.join().unwrap();
}

#[test]
fn checkpoint_rejects_duplicate_resource_owners() {
    let (mut session, worker) = fixture(vec![]);
    session.operation = Operation::Closing;
    session.retiring = vec![ResourceId(70), ResourceId(70)];
    assert!(crate::checkpoint::encode(&session, ResourceId(2)).is_err());
    worker.join().unwrap();
}

#[test]
fn input_checkpoint_preserves_cursor_basis_and_authored_unicode() {
    use reovim_client_tui::input::{Code, Input, Key, Modifiers};
    let (session, worker) = fixture(vec![]);
    let mut view = crate::View::new();
    view.current = Some(window(7));
    view.cursor = 2;
    view.pending_cursor = Some(3);
    view.pending = false;
    view.halted = true;
    view.queued.push_back(Input::Text("🦀e\u{301}".into()));
    view.queued.push_back(Input::Key(Key {
        code: Code::Char('z'),
        modifiers: Modifiers::CONTROL,
    }));
    let saved =
        crate::checkpoint::encode_complete(&session, ResourceId(2), &view)
            .unwrap();
    let (_, _, input) = crate::checkpoint::decode_complete(&saved).unwrap();
    let input = input.unwrap();
    assert_eq!(input.basis, Some(window(7).observed.buffer));
    assert_eq!(input.cursor, 2);
    assert_eq!(input.planned, Some(3));
    assert!(!input.pending);
    assert!(input.halted);
    assert_eq!(input.queued, view.queued);
    let mut malformed = saved;
    let last = malformed.len() - 9;
    malformed[last..last + 8].copy_from_slice(&0xd800_u64.to_le_bytes());
    assert!(crate::checkpoint::decode_complete(&malformed).is_err());
    worker.join().unwrap();
}

#[test]
fn refresh_does_not_erase_a_failed_submitted_edit() {
    let mut view = crate::View::new();
    let submitted = reovim_client_tui::custody::Submitted {
        basis: window(1).observed.buffer,
        action: crate::Action::Insert(0, 0, "retained".into(), 0),
    };
    view.submitted = Some(submitted.clone());
    crate::accept(&mut view, Err(std::io::Error::other("failed edit")));
    crate::accept(
        &mut view,
        Ok(crate::Update { window: window(2), accepted: None }),
    );
    assert_eq!(view.submitted, Some(submitted.clone()));
    assert!(view.halted);
    view.receipt = Some(90);
    view.discard_retained();
    assert_eq!(view.submitted, Some(submitted));
    view.receipt = None;
    view.discard_retained();
    assert!(view.submitted.is_none());
}

#[test]
fn portable_view_fences_invalid_ranges_and_overlapping_actions() {
    use reovim_client_tui::view::Error;
    let mut view = crate::View::new();
    view.accept(window(1), None);
    view.visible = true;
    let foreign = crate::Action::Insert(1, 2, "x".into(), 1);
    assert_eq!(view.begin(&foreign), Err(Error::InvalidAction));
    let outside = crate::Action::Insert(0, 5, "x".into(), 0);
    assert_eq!(view.begin(&outside), Err(Error::InvalidAction));
    let edit = crate::Action::Insert(0, 0, "🦀".into(), 0);
    view.begin(&edit).unwrap();
    assert_eq!(view.pending_cursor, Some(1));
    assert_eq!(view.submitted.as_ref().unwrap().action, edit);
    assert_eq!(view.begin(&crate::Action::Undo), Err(Error::Unavailable));
    view.failed(&"unknown delivery");
    assert_eq!(view.begin(&crate::Action::Redo), Err(Error::Unavailable));
    assert_eq!(view.submitted.as_ref().unwrap().action, edit);
    let input = view.retained_input().unwrap();
    assert_eq!(input.submitted.as_ref().unwrap().action, edit);
    view.cursor = 5;
    assert!(matches!(view.retained_input(), Err(Error::InvalidCursor)));
}

#[test]
fn action_survives_checkpoint_when_request_construction_fails() {
    let (mut session, worker) =
        fixture(vec![("reserve", Some("ok\t90")), ("release\t90", Some("ok"))]);
    session.access = Some(ResourceId(70));
    session.observed = Some(
        reovim_module_text::api::observation(
            &crate::remote::message(OBSERVATION).unwrap(),
            ResourceId(70),
        )
        .unwrap(),
    );
    let text = "é".repeat(20_000);
    let submitted = reovim_client_tui::custody::Submitted {
        basis: session.observed.unwrap().observed().buffer,
        action: crate::Action::Insert(0, 0, text.clone(), 0),
    };
    assert!(session.replace(0, 0, &text, 0).is_err());
    assert!(session.remote.pending.is_none());
    assert_eq!(session.remote.reserved, Some(90));
    let mut view = crate::View::new();
    view.submitted = Some(submitted.clone());
    let saved =
        crate::checkpoint::encode_complete(&session, ResourceId(2), &view)
            .unwrap();
    let (_, _, input) = crate::checkpoint::decode_complete(&saved).unwrap();
    assert_eq!(input.unwrap().submitted, Some(submitted));
    assert!(session.recover().is_err());
    assert_eq!(session.remote.reserved, None);
    assert_eq!(session.operation, Operation::Idle);
    worker.join().unwrap();
}

#[test]
fn submitted_action_must_match_the_original_pending_edit() {
    let (mut session, worker) = fixture(vec![
        ("reserve", Some("ok\t91")),
        (
            concat!(
                "call\t91\twrite\t@70\ttext.replace\t1\t",
                "0000000000000000000000000000000078\t@80"
            ),
            None,
        ),
    ]);
    session.access = Some(ResourceId(70));
    session.observed = Some(
        reovim_module_text::api::observation(
            &crate::remote::message(OBSERVATION).unwrap(),
            ResourceId(70),
        )
        .unwrap(),
    );
    assert!(session.replace(0, 0, "x", 0).is_err());
    let mut view = crate::View::new();
    let mut submitted = reovim_client_tui::custody::Submitted {
        basis: session.observed.unwrap().observed().buffer,
        action: crate::Action::Insert(0, 0, "x".into(), 0),
    };
    view.submitted = Some(submitted.clone());
    view.pending_cursor = Some(1);
    let saved =
        crate::checkpoint::encode_complete(&session, ResourceId(2), &view)
            .unwrap();
    assert!(crate::checkpoint::decode_complete(&saved).is_ok());
    let mut changed = saved;
    *changed.last_mut().unwrap() = b'y';
    assert!(crate::checkpoint::decode_complete(&changed).is_err());
    submitted.basis.revision = 1;
    view.submitted = Some(submitted);
    assert!(
        crate::checkpoint::encode_complete(&session, ResourceId(2), &view)
            .is_err()
    );
    view.submitted = None;
    assert!(
        crate::checkpoint::encode_complete(&session, ResourceId(2), &view)
            .is_err()
    );
    worker.join().unwrap();
}

#[test]
fn failed_pre_send_journal_keeps_reservation_without_transmitting_call() {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    let directory = std::env::temp_dir().join(format!(
        "reovim-journal-test-{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos(),
    ));
    let (mut session, empty) = fixture(vec![]);
    empty.join().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    session.remote.address = listener.local_addr().unwrap().to_string();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let frame = transport::receive(&mut BufReader::new(&stream)).unwrap();
        assert_eq!(frame, format!("v1\t{CLIENT}\treserve"));
        transport::send(&mut stream, "ok\t92").unwrap();
        listener
    });
    let (send, notices) = std::sync::mpsc::channel();
    session.journal = Some(
        crate::journal::Journal::create(
            directory.to_str().unwrap(),
            ResourceId(2),
            send,
        )
        .unwrap(),
    );
    let run = thread::spawn(move || {
        let result = session.attach(ResourceId(2));
        (session, result)
    });
    for before_call in [false, true] {
        let crate::Notice::Capture(reply) =
            notices.recv_timeout(Duration::from_secs(5)).unwrap()
        else {
            panic!("expected persistence barrier");
        };
        if before_call {
            std::fs::write(directory.join("next"), "occupied").unwrap();
        }
        reply
            .send(
                crate::View::new()
                    .retained_input()
                    .map_err(std::io::Error::other),
            )
            .unwrap();
    }
    let (session, result) = run.join().unwrap();
    assert!(result.is_err());
    assert_eq!(session.remote.reserved, Some(92));
    assert!(session.remote.pending.is_none());
    let listener = server.join().unwrap();
    listener.set_nonblocking(true).unwrap();
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
    let saved = std::fs::read(directory.join("state")).unwrap();
    let (saved, _, _) = crate::checkpoint::decode_complete(&saved).unwrap();
    assert_eq!(saved.operation, Operation::Attach(ResourceId(2)));
    assert!(saved.remote.pending.is_none());
    assert_eq!(
        std::fs::read_to_string(directory.join("next")).unwrap(),
        "occupied"
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn failed_adoption_journal_prevents_receipt_release() {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    let directory = std::env::temp_dir().join(format!(
        "reovim-adoption-journal-{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos(),
    ));
    let (mut session, empty) = fixture(vec![]);
    empty.join().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    session.remote.address = listener.local_addr().unwrap().to_string();
    let server = thread::spawn(move || {
        serve(
            &listener,
            vec![
                ("reserve", Some("ok\t93")),
                (
                    "call\t93\twrite\t@2\ttext.attach\t1\t\t@1",
                    Some("ok\ttext.attach\t1\t\t@70"),
                ),
            ],
        );
        listener
    });
    let (send, notices) = std::sync::mpsc::channel();
    session.journal = Some(
        crate::journal::Journal::create(
            directory.to_str().unwrap(),
            ResourceId(2),
            send,
        )
        .unwrap(),
    );
    let run = thread::spawn(move || {
        let result = session.attach(ResourceId(2));
        (session, result)
    });
    for phase in ["reserve", "send", "result", "adoption"] {
        let crate::Notice::Capture(reply) =
            notices.recv_timeout(Duration::from_secs(5)).unwrap()
        else {
            panic!("expected persistence barrier");
        };
        if phase == "adoption" {
            std::fs::write(directory.join("next"), "occupied").unwrap();
        }
        reply
            .send(
                crate::View::new()
                    .retained_input()
                    .map_err(std::io::Error::other),
            )
            .unwrap();
    }
    let (session, result) = run.join().unwrap();
    assert!(result.is_err());
    assert_eq!(session.access, Some(ResourceId(70)));
    assert!(session.remote.pending.as_ref().unwrap().release().is_ok());
    let listener = server.join().unwrap();
    listener.set_nonblocking(true).unwrap();
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
    let saved = std::fs::read(directory.join("state")).unwrap();
    let (saved, _, _) = crate::checkpoint::decode_complete(&saved).unwrap();
    assert_eq!(saved.access, None);
    let receipt = saved.remote.pending.as_ref().unwrap();
    assert_eq!(receipt.id(), 93);
    assert_eq!(receipt.response(), Some("ok\ttext.attach\t1\t\t@70"));
    assert!(receipt.release().is_err());
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn rejected_change_does_not_leave_a_pending_effect() {
    let (mut session, worker) = fixture(vec![
        ("reserve", Some("ok\t40")),
        (
            concat!(
                "call\t40\twrite\t@70\ttext.replace\t1\t",
                "0000000000000000000000000000000078\t@80"
            ),
            Some("error\tfailed\t7\t\t"),
        ),
        ("release\t40", Some("ok")),
    ]);
    session.observed = Some(
        reovim_module_text::api::observation(
            &crate::remote::message(OBSERVATION).unwrap(),
            ResourceId(70),
        )
        .unwrap(),
    );
    assert!(session.replace(0, 0, "x", 0).is_err());
    assert_eq!(session.operation, Operation::Idle);
    assert!(session.accepted.is_none());
    assert!(session.remote.pending.is_none());
    worker.join().unwrap();
}

fn window(revision: u64) -> reovim_client_tui::context::Window {
    use reovim_module_text::api::{Observed, Unit, Window};
    reovim_client_tui::context::Window::complete(Window {
        observed: Observed {
            representation: ResourceId(3),
            space: ResourceId(3),
            buffer: reovim_kernel_interface::buffer::BufferState {
                buffer: ResourceId(1),
                revision,
            },
        },
        snapshot: ResourceId(4),
        range: 0..4,
        eof: true,
        units: vec![
            Unit::Scalar('a'),
            Unit::Opaque(255),
            Unit::Scalar('🦀'),
            Unit::Scalar('é'),
        ],
    })
    .unwrap()
}

#[test]
fn cursor_waits_for_matching_acceptance_and_resets_for_peer_observation() {
    let mut view = crate::View {
        current: Some(window(1)),
        cursor: 2,
        pending_cursor: Some(3),
        pending: true,
        halted: false,
        status: String::new(),
        queued: std::collections::VecDeque::new(),
        submitted: None,
        receipt: None,
        top: 0,
        visible: true,
    };
    crate::accept(&mut view, Err(std::io::Error::other("stale edit")));
    assert_eq!(view.cursor, 2);
    assert!(view.halted);
    let accepted = window(2).observed.buffer;
    assert_eq!(view.pending_cursor, Some(3));
    crate::accept(
        &mut view,
        Ok(crate::Update { window: window(2), accepted: Some(accepted) }),
    );
    assert_eq!(view.cursor, 3);
    view.pending_cursor = Some(4);
    view.queued
        .push_back(reovim_client_tui::input::Input::Text("retained".into()));
    crate::accept(
        &mut view,
        Ok(crate::Update { window: window(3), accepted: Some(accepted) }),
    );
    assert_eq!(view.cursor, 0);
    assert!(view.halted);
    assert_eq!(
        view.queued.front(),
        Some(&reovim_client_tui::input::Input::Text("retained".into()))
    );
    assert_eq!(view.current.as_ref().unwrap().observed.buffer.revision, 3);
}

#[test]
fn paste_and_backspace_plan_scalar_ranges_without_moving_the_cursor() {
    use crate::Action;
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
    let value = window(1);
    let mut cursor = 2;
    let event = Event::Paste("🦀é".into());
    let paste =
        crate::action(crate::input::input(event).unwrap(), &value, &mut cursor)
            .unwrap();
    assert_eq!(cursor, 2);
    assert_eq!(paste.cursor_after(), Some(4));
    let delete = crate::action(
        crate::input::input(Event::Key(KeyEvent::new(
            KeyCode::Backspace,
            KeyModifiers::NONE,
        )))
        .unwrap(),
        &value,
        &mut cursor,
    )
    .unwrap();
    assert_eq!(cursor, 2);
    assert!(
        matches!(delete, Action::Insert(1, 2, ref text, 0) if text.is_empty())
    );
    assert_eq!(delete.cursor_after(), Some(1));
}

#[test]
fn terminal_normalization_does_not_turn_release_or_meta_into_text() {
    use crossterm::event::{
        Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers,
    };
    use reovim_client_tui::input::{Code, Modifiers};
    let mut key = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::SUPER);
    let logical = crate::input::key(&Event::Key(key)).unwrap();
    assert_eq!(logical.code, Code::Char('x'));
    assert_eq!(logical.modifiers, Modifiers::META);
    let mut cursor = 0;
    assert_eq!(
        crate::action(
            crate::input::input(Event::Key(key)).unwrap(),
            &window(1),
            &mut cursor
        ),
        None
    );
    key.kind = KeyEventKind::Release;
    assert_eq!(crate::input::key(&Event::Key(key)), None);
    assert!(crate::input::event(Event::Key(key)).is_none());
    assert!(crate::input::event(Event::FocusLost).is_none());
    assert!(matches!(
        crate::input::event(Event::Resize(80, 24)),
        Some(crate::input::Event::Resize)
    ));
}
