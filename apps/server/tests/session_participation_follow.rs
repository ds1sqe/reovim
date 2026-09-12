use reovim_kernel_interface::{
    Connection, Contract, Error, Message, RequestError, ResourceId,
    notify::Next,
    session::{self, Watching},
};
use reovim_kernel_runtime::Runtime;

#[path = "support/session.rs"]
mod support;
use support::{attach, finish, read, setup, update};

fn follow(
    runtime: &mut Runtime,
    client: &Connection,
    participant: ResourceId,
) -> Watching {
    session::followed(
        &finish(runtime, session::follow(client, participant)).unwrap(),
    )
    .unwrap()
}

fn release(runtime: &mut Runtime, client: &Connection, observed: &Watching) {
    finish(runtime, session::release(client, observed.observation.snapshot))
        .unwrap();
    client.close(observed.watch).into_inner().unwrap();
}

#[test]
fn seek_and_follow_fence_prior_streams_and_return_to_live_context() {
    let (mut runtime, client, initial) = setup();
    let own = attach(&mut runtime, &client, initial.session);
    let first = follow(&mut runtime, &client, own);
    assert_eq!(first.observation.value.data.as_ref(), &[1]);
    let old_read = session::next(&client, first.watch, true);
    finish(&mut runtime, session::seek(&client, own, initial.snapshot))
        .unwrap();
    assert_eq!(
        finish(&mut runtime, old_read),
        Err(RequestError::Failed(Error::CLOSED))
    );
    assert_eq!(read(&mut runtime, &client, own).unwrap().data.as_ref(), &[1]);
    let changed = update(&mut runtime, &client, &initial, 2);
    let live = follow(&mut runtime, &client, own);
    assert_eq!(live.observation.session, initial.session);
    assert_eq!(live.observation.value.data.as_ref(), &[2]);
    assert_eq!(read(&mut runtime, &client, own).unwrap().data.as_ref(), &[2]);
    let renewed = follow(&mut runtime, &client, own);
    assert_eq!(
        finish(&mut runtime, session::next(&client, live.watch, false)),
        Err(RequestError::Failed(Error::CLOSED))
    );
    let pending =
        finish(&mut runtime, session::next(&client, renewed.watch, false))
            .unwrap();
    assert_eq!(session::change(&pending).unwrap(), Next::Pending);
    finish(&mut runtime, session::detach(&client, own)).unwrap();
    assert_eq!(
        finish(&mut runtime, session::next(&client, renewed.watch, false)),
        Err(RequestError::Failed(Error::CLOSED))
    );
    for observed in [&first, &live, &renewed] {
        release(&mut runtime, &client, observed);
    }
    finish(&mut runtime, session::close(&client, changed.session)).unwrap();
    runtime.close().unwrap();
}

#[test]
fn follow_failure_preserves_stream_and_historical_selection() {
    let (mut runtime, client, initial) = setup();
    let own = attach(&mut runtime, &client, initial.session);
    let first = follow(&mut runtime, &client, own);
    let mut readers = Vec::new();
    for _ in 0..31 {
        readers.push(
            session::watching(
                &finish(
                    &mut runtime,
                    session::observe(&client, initial.session),
                )
                .unwrap(),
            )
            .unwrap(),
        );
    }
    let changed = update(&mut runtime, &client, &initial, 2);
    assert_eq!(
        finish(&mut runtime, session::follow(&client, own)),
        Err(RequestError::Failed(Error::PRESSURE))
    );
    let event = session::change(
        &finish(&mut runtime, session::next(&client, first.watch, false))
            .unwrap(),
    )
    .unwrap();
    let Next::Item(event) = event else {
        panic!("old stream remains live");
    };
    assert_eq!(event.before, first.revision);
    assert_eq!(event.after, first.revision + 1);
    finish(&mut runtime, session::seek(&client, own, initial.snapshot))
        .unwrap();
    readers.push(
        session::watching(
            &finish(&mut runtime, session::observe(&client, initial.session))
                .unwrap(),
        )
        .unwrap(),
    );
    assert_eq!(
        finish(&mut runtime, session::follow(&client, own)),
        Err(RequestError::Failed(Error::PRESSURE))
    );
    assert_eq!(read(&mut runtime, &client, own).unwrap().data.as_ref(), &[1]);
    release(&mut runtime, &client, &readers.pop().unwrap());
    let live = follow(&mut runtime, &client, own);
    assert_eq!(live.observation.value.data.as_ref(), &[2]);
    finish(&mut runtime, session::detach(&client, own)).unwrap();
    for observed in readers.iter().chain([&first, &live]) {
        release(&mut runtime, &client, observed);
    }
    finish(&mut runtime, session::close(&client, changed.session)).unwrap();
    runtime.close().unwrap();
}

#[test]
fn session_close_drains_followed_events_and_ends_without_fencing() {
    let (mut runtime, client, initial) = setup();
    let own = attach(&mut runtime, &client, initial.session);
    let followed = follow(&mut runtime, &client, own);
    let changed = update(&mut runtime, &client, &initial, 2);
    finish(&mut runtime, session::close(&client, changed.session)).unwrap();
    let event = session::change(
        &finish(&mut runtime, session::next(&client, followed.watch, false))
            .unwrap(),
    )
    .unwrap();
    let Next::Item(event) = event else {
        panic!("queued accepted transition");
    };
    assert_eq!(event.before, followed.revision);
    assert_eq!(event.after, followed.revision + 1);
    let end = session::change(
        &finish(&mut runtime, session::next(&client, followed.watch, false))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(end, Next::End { lost: false, error: None });
    assert_eq!(
        read(&mut runtime, &client, own),
        Err(RequestError::Rejected(Error::CLOSED))
    );
    assert_eq!(
        read(&mut runtime, &client, followed.observation.snapshot)
            .unwrap()
            .data
            .as_ref(),
        &[1]
    );
    release(&mut runtime, &client, &followed);
    runtime.close().unwrap();
}

#[test]
fn gap_renewal_observes_latest_context_and_fences_the_lost_stream() {
    let (mut runtime, client, initial) = setup();
    let own = attach(&mut runtime, &client, initial.session);
    let first = follow(&mut runtime, &client, own);
    let mut current = initial.clone();
    for value in 2_u8..35 {
        let next = update(&mut runtime, &client, &current, value);
        if current.snapshot != initial.snapshot {
            finish(&mut runtime, session::release(&client, current.snapshot))
                .unwrap();
        }
        current = next;
    }
    let gap = session::change(
        &finish(&mut runtime, session::next(&client, first.watch, false))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(gap, Next::Gap);
    let live = follow(&mut runtime, &client, own);
    assert_eq!(live.observation.value, current.value);
    assert_eq!(
        finish(&mut runtime, session::next(&client, first.watch, false)),
        Err(RequestError::Failed(Error::CLOSED))
    );
    assert_eq!(
        session::change(
            &finish(&mut runtime, session::next(&client, live.watch, false))
                .unwrap()
        )
        .unwrap(),
        Next::Pending
    );
    finish(&mut runtime, session::detach(&client, own)).unwrap();
    release(&mut runtime, &client, &first);
    release(&mut runtime, &client, &live);
    finish(&mut runtime, session::close(&client, initial.session)).unwrap();
    runtime.close().unwrap();
}

#[test]
fn failed_foreign_seek_keeps_the_current_stream_usable() {
    let (mut runtime, client, initial) = setup();
    let model = client.resolve("model").into_inner().unwrap();
    let foreign = finish(
        &mut runtime,
        session::create(
            &client,
            model,
            &Message::new(Contract::new("initial", 1), &[9]),
        ),
    )
    .unwrap();
    let own = attach(&mut runtime, &client, initial.session);
    let first = follow(&mut runtime, &client, own);
    assert_eq!(
        finish(&mut runtime, session::seek(&client, own, foreign.snapshot)),
        Err(RequestError::Failed(Error::STALE))
    );
    let changed = update(&mut runtime, &client, &initial, 2);
    let event = session::change(
        &finish(&mut runtime, session::next(&client, first.watch, false))
            .unwrap(),
    )
    .unwrap();
    let Next::Item(event) = event else {
        panic!("preserved stream");
    };
    assert_eq!(event.before, first.revision);
    assert_eq!(
        read(&mut runtime, &client, own).unwrap().data,
        changed.value.data
    );
    finish(&mut runtime, session::detach(&client, own)).unwrap();
    release(&mut runtime, &client, &first);
    finish(&mut runtime, session::close(&client, initial.session)).unwrap();
    finish(&mut runtime, session::close(&client, foreign.session)).unwrap();
    runtime.close().unwrap();
}
