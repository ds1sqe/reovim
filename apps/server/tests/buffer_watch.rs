use reovim_host_reference::run;
use reovim_kernel_interface::{
    Connection, Contract, Error, Message, RequestError,
    buffer::{self, Change, Observation, Transition, Watching},
    notify::Next,
    provider::Permissions,
};
use reovim_kernel_runtime::{Limits, Runtime};
use std::{
    future::Future,
    task::{Context, Waker},
};

fn finish<T>(runtime: &mut Runtime, future: impl Future<Output = T>) -> T {
    run(future, |context| runtime.advance(context).map(|_| ())).unwrap()
}

fn setup() -> (Runtime, Connection, Observation) {
    setup_with(Limits::default())
}

fn setup_with(limits: Limits) -> (Runtime, Connection, Observation) {
    let mut runtime = Runtime::new(202, limits).unwrap();
    runtime.start().unwrap();
    let id = runtime
        .register_model("model", reovim_representation_basic::heading())
        .map_err(|(error, _)| error)
        .unwrap();
    let client =
        reovim_sdk::local(runtime.connect(&[(id, Permissions::ALL)]).unwrap());
    let model = client.resolve("model").into_inner().unwrap();
    let initial = buffer::observation(
        finish(
            &mut runtime,
            client.write(
                model,
                &Message::new(
                    Contract::new("heading.initial", 1),
                    b"# Draft\n\xff",
                ),
            ),
        )
        .unwrap(),
    )
    .unwrap();
    (runtime, client, initial)
}

fn observe(
    runtime: &mut Runtime,
    client: &Connection,
    state: &Observation,
) -> Watching {
    buffer::watching(
        &finish(runtime, buffer::observe(client, state.state.buffer)).unwrap(),
    )
    .unwrap()
}

fn next(
    runtime: &mut Runtime,
    client: &Connection,
    observing: Watching,
) -> Next<Change> {
    buffer::change(
        &finish(runtime, buffer::next(client, observing.watch, false)).unwrap(),
    )
    .unwrap()
}

fn edit(
    runtime: &mut Runtime,
    client: &Connection,
    state: &Observation,
) -> Observation {
    let input = buffer::edit(
        state.state,
        Message::new(Contract::new("heading.replace", 1), b"Plan"),
    );
    buffer::observation(
        finish(runtime, client.write(state.state.buffer, &input)).unwrap(),
    )
    .unwrap()
}

#[test]
fn coherent_snapshot_and_independent_readers_follow_actual_history_edges() {
    let (mut runtime, client, initial) = setup();
    let first = observe(&mut runtime, &client, &initial);
    let second = observe(&mut runtime, &client, &initial);
    assert_eq!(first.snapshot.state, initial.state);
    assert_eq!(next(&mut runtime, &client, first), Next::Pending);
    let changed = edit(&mut runtime, &client, &initial);
    let Next::Item(accepted) = next(&mut runtime, &client, first) else {
        panic!("accepted edit")
    };
    assert_eq!(
        (accepted.before, accepted.after),
        (initial.state.revision, changed.state.revision)
    );
    assert_eq!(
        (accepted.from_node, accepted.to_node),
        (initial.node, changed.node)
    );
    assert_eq!(accepted.transition, Transition::Edit);
    assert_eq!(next(&mut runtime, &client, second), Next::Item(accepted));
    let retained = finish(
        &mut runtime,
        client.read(
            first.snapshot.id,
            &Message::new(Contract::new("heading.source", 1), &[]),
        ),
    )
    .unwrap();
    assert_eq!(retained.data, initial.value.data);
    let invalid = buffer::edit(
        changed.state,
        Message::new(Contract::new("heading.replace", 1), b"!"),
    );
    assert_eq!(
        finish(&mut runtime, client.write(initial.state.buffer, &invalid)),
        Err(RequestError::Failed(Error::INVALID))
    );
    assert_eq!(next(&mut runtime, &client, first), Next::Pending);
    let undo = buffer::edit(
        changed.state,
        Message::new(Contract::new("kernel.history.undo", 1), &[]),
    );
    let restored =
        finish(&mut runtime, client.write(initial.history, &undo)).unwrap();
    let Next::Item(undone) = next(&mut runtime, &client, first) else {
        panic!("Undo")
    };
    assert_eq!(undone.transition, Transition::Undo);
    assert_eq!(
        (undone.from_node, undone.to_node),
        (changed.node, initial.node)
    );
    assert_eq!(undone.before, accepted.after);
    assert_eq!(undone.after, buffer::integer(&restored.data[..8]).unwrap());
    assert_ne!(undone.request, accepted.request);
    runtime.close().unwrap();
}

#[test]
fn slow_reader_gap_is_sticky_and_renewal_keeps_a_fast_reader_independent() {
    let (mut runtime, client, mut state) = setup();
    let slow = observe(&mut runtime, &client, &state);
    let fast = observe(&mut runtime, &client, &state);
    // The declared delivery bound is 32 accepted transitions per reader.
    for _ in 0..33 {
        let changed = edit(&mut runtime, &client, &state);
        let Next::Item(item) = next(&mut runtime, &client, fast) else {
            panic!("fast reader")
        };
        assert_eq!(
            (item.before, item.after),
            (state.state.revision, changed.state.revision)
        );
        state = changed;
    }
    assert_eq!(next(&mut runtime, &client, slow), Next::Gap);
    assert_eq!(next(&mut runtime, &client, slow), Next::Gap);
    let renewed = observe(&mut runtime, &client, &state);
    assert_eq!(renewed.snapshot.state, state.state);
    let changed = edit(&mut runtime, &client, &state);
    let Next::Item(item) = next(&mut runtime, &client, renewed) else {
        panic!("renewed reader")
    };
    assert_eq!(
        (item.before, item.after),
        (state.state.revision, changed.state.revision)
    );
    assert_eq!(next(&mut runtime, &client, fast), Next::Item(item));
    finish(
        &mut runtime,
        buffer::close(
            &client,
            changed.state,
            buffer::ClosePolicy::DiscardLocal,
        ),
    )
    .unwrap();
    assert_eq!(
        next(&mut runtime, &client, slow),
        Next::End { lost: true, error: None }
    );
    assert_eq!(
        next(&mut runtime, &client, renewed),
        Next::End { lost: false, error: None }
    );
    runtime.close().unwrap();
}

#[test]
fn buffer_close_drains_events_while_snapshot_and_watch_have_separate_owners() {
    let (mut runtime, client, initial) = setup();
    let observing = observe(&mut runtime, &client, &initial);
    let changed = edit(&mut runtime, &client, &initial);
    finish(
        &mut runtime,
        buffer::close(
            &client,
            changed.state,
            buffer::ClosePolicy::DiscardLocal,
        ),
    )
    .unwrap();
    assert!(matches!(next(&mut runtime, &client, observing), Next::Item(_)));
    assert_eq!(
        next(&mut runtime, &client, observing),
        Next::End { lost: false, error: None }
    );
    let value = finish(
        &mut runtime,
        client.read(
            observing.snapshot.id,
            &Message::new(Contract::new("heading.source", 1), &[]),
        ),
    )
    .unwrap();
    assert_eq!(value.data, initial.value.data);
    client.close(observing.watch).into_inner().unwrap();
    assert_eq!(
        finish(&mut runtime, buffer::next(&client, observing.watch, false)),
        Err(RequestError::Rejected(Error::CLOSED))
    );
    client.close(observing.snapshot.id).into_inner().unwrap();
    runtime.close().unwrap();
}

#[test]
fn closing_a_watch_wakes_pending_read_and_fences_delivery() {
    let (mut runtime, client, initial) = setup();
    let observing = observe(&mut runtime, &client, &initial);
    let mut waiting = buffer::next(&client, observing.watch, true);
    let mut context = Context::from_waker(Waker::noop());
    assert!(waiting.as_mut().poll(&mut context).is_pending());
    runtime.advance(&mut context).unwrap();
    runtime.advance(&mut context).unwrap();
    assert_eq!(
        finish(&mut runtime, buffer::next(&client, observing.watch, false)),
        Err(RequestError::Failed(Error::BUSY))
    );
    client.close(observing.watch).into_inner().unwrap();
    assert_eq!(
        finish(&mut runtime, waiting),
        Err(RequestError::Failed(Error::CLOSED))
    );
    let changed = edit(&mut runtime, &client, &initial);
    assert!(changed.state.revision > initial.state.revision);
    runtime.close().unwrap();
}

#[test]
fn lost_observe_reply_recovers_the_same_snapshot_and_watch() {
    let (mut runtime, client, initial) = setup();
    let mut observing = buffer::observe(&client, initial.state.buffer);
    let mut context = Context::from_waker(Waker::noop());
    assert!(observing.as_mut().poll(&mut context).is_pending());
    let client = client.reconnect().unwrap();
    let Err(RequestError::Unknown { recovery, .. }) =
        finish(&mut runtime, observing)
    else {
        panic!("retained observation outcome")
    };
    let work = client.resolve(&recovery).into_inner().unwrap();
    let first = finish(
        &mut runtime,
        reovim_kernel_interface::requests::result(&client, work),
    )
    .unwrap()
    .unwrap();
    let repeated = finish(
        &mut runtime,
        reovim_kernel_interface::requests::result(&client, work),
    )
    .unwrap()
    .unwrap();
    assert_eq!(first, repeated);
    client.close(work).into_inner().unwrap();
    let observing = buffer::watching(&first).unwrap();
    assert_eq!(next(&mut runtime, &client, observing), Next::Pending);
    runtime.close().unwrap();
}

#[test]
fn acquisition_bounds_reject_before_publishing_an_unusable_watch() {
    let (mut runtime, client, initial) =
        setup_with(Limits { message_bytes: 40, ..Limits::default() });
    assert_eq!(
        finish(&mut runtime, buffer::observe(&client, initial.state.buffer)),
        Err(RequestError::Failed(Error::PRESSURE))
    );
    let retained = buffer::retained(
        &finish(&mut runtime, buffer::retain(&client, initial.state)).unwrap(),
    )
    .unwrap();
    assert_eq!(retained.state, initial.state);
    runtime.close().unwrap();
    let (mut runtime, client, initial) = setup();
    let mut readers = Vec::new();
    for _ in 0..32 {
        readers.push(observe(&mut runtime, &client, &initial));
    }
    assert_eq!(
        finish(&mut runtime, buffer::observe(&client, initial.state.buffer)),
        Err(RequestError::Failed(Error::PRESSURE))
    );
    let released = readers.pop().unwrap();
    client.close(released.watch).into_inner().unwrap();
    let renewed = observe(&mut runtime, &client, &initial);
    assert_eq!(renewed.snapshot.state, initial.state);
    assert_ne!(renewed.watch, released.watch);
    let changed = edit(&mut runtime, &client, &initial);
    let Next::Item(item) = next(&mut runtime, &client, renewed) else {
        panic!("new watch")
    };
    assert_eq!(item.after, changed.state.revision);
    runtime.close().unwrap();
}

#[test]
fn read_only_observer_receives_changes_and_pending_end_without_edit_grants() {
    let (mut runtime, writer, initial) = setup();
    let buffers = runtime.resource("buffers").unwrap();
    let reader = reovim_sdk::local(
        runtime.connect(&[(buffers, Permissions::READ)]).unwrap(),
    );
    let root = reader.resolve("buffers").into_inner().unwrap();
    let entries = reader.children(root).into_inner().unwrap();
    let buffer = entries[0].resource;
    let observing = buffer::watching(
        &finish(&mut runtime, buffer::observe(&reader, buffer)).unwrap(),
    )
    .unwrap();
    let denied = buffer::edit(
        observing.snapshot.state,
        Message::new(Contract::new("heading.replace", 1), b"forbidden"),
    );
    assert_eq!(
        finish(&mut runtime, reader.write(buffer, &denied)),
        Err(RequestError::Rejected(Error::DENIED))
    );
    let changed = edit(&mut runtime, &writer, &initial);
    let Next::Item(item) = next(&mut runtime, &reader, observing) else {
        panic!("read-only event")
    };
    assert_eq!(item.after, changed.state.revision);
    let mut pending = buffer::next(&reader, observing.watch, true);
    let mut context = Context::from_waker(Waker::noop());
    assert!(pending.as_mut().poll(&mut context).is_pending());
    runtime.advance(&mut context).unwrap();
    runtime.advance(&mut context).unwrap();
    finish(
        &mut runtime,
        buffer::close(
            &writer,
            changed.state,
            buffer::ClosePolicy::DiscardLocal,
        ),
    )
    .unwrap();
    let ended =
        buffer::change(&finish(&mut runtime, pending).unwrap()).unwrap();
    assert_eq!(ended, Next::End { lost: false, error: None });
    runtime.close().unwrap();
}

#[test]
fn acceptance_wakes_a_parked_reader_without_changing_the_driver_waker() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    struct Signal(AtomicBool);
    impl std::task::Wake for Signal {
        fn wake(self: Arc<Self>) {
            self.0.store(true, Ordering::Release);
        }
    }
    let (mut runtime, client, initial) = setup();
    let observing = observe(&mut runtime, &client, &initial);
    let signal = Arc::new(Signal(AtomicBool::new(false)));
    let waker = Waker::from(signal.clone());
    let mut caller = Context::from_waker(&waker);
    let mut driver = Context::from_waker(Waker::noop());
    let mut waiting = buffer::next(&client, observing.watch, true);
    assert!(waiting.as_mut().poll(&mut caller).is_pending());
    runtime.advance(&mut driver).unwrap();
    runtime.advance(&mut driver).unwrap();
    assert!(!signal.0.load(Ordering::Acquire));
    let request = buffer::edit(
        initial.state,
        Message::new(Contract::new("heading.replace", 1), b"Plan"),
    );
    let editing = client.write(initial.state.buffer, &request);
    // Bound this diagnostic drive; no timer or replacement driver wakes it.
    for _ in 0..8 {
        runtime.advance(&mut driver).unwrap();
    }
    assert!(signal.0.load(Ordering::Acquire));
    let std::task::Poll::Ready(Ok(reply)) = waiting.as_mut().poll(&mut caller)
    else {
        panic!("accepted edit did not wake the watch")
    };
    assert!(matches!(buffer::change(&reply), Ok(Next::Item(_))));
    finish(&mut runtime, editing).unwrap();
    runtime.close().unwrap();
}
