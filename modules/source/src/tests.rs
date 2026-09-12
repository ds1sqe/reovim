use super::{api, initialize, observe, prepare, storage::Source};
use alloc::{boxed::Box, rc::Rc, vec, vec::Vec};
use core::{
    future::{Future, ready},
    ops::Range,
};
use reovim_host_reference::run;
use reovim_kernel_interface::{
    Contract, Error, Message, RequestError,
    buffer::{self, ModelOps, ModelProvider},
    provider::Permissions,
};
use reovim_kernel_runtime::{Limits, Runtime};

fn finish<T>(runtime: &mut Runtime, future: impl Future<Output = T>) -> T {
    run(future, |context| runtime.advance(context).map(|_| ())).unwrap()
}

fn model() -> ModelProvider {
    ModelProvider::new(
        (),
        ModelOps {
            unload: None,
            initialize: |(), input| Box::pin(ready(initialize(input))),
            prepare: |(), state, input| Box::pin(ready(prepare(state, input))),
            observe: |(), state, input| observe(state, input),
        },
        Contract::new("source.initial", 1),
        Contract::new("source.bytes", 1),
        Contract::new("source.splice", 1),
        Rc::new(()),
    )
}

#[test]
fn public_byte_api_preserves_old_windows_and_undoes_exact_source() {
    let mut runtime = Runtime::new(201, Limits::default()).unwrap();
    runtime.start().unwrap();
    let id = runtime
        .register_model("source", model())
        .map_err(|(error, _)| error)
        .unwrap();
    let client =
        reovim_sdk::local(runtime.connect(&[(id, Permissions::ALL)]).unwrap());
    let model = client.resolve("source").into_inner().unwrap();
    let original = b"# Draft\r\n\xff\0tail";
    let first =
        finish(&mut runtime, api::create(&client, model, original)).unwrap();
    let snapshot = buffer::retained(
        &finish(&mut runtime, buffer::retain(&client, first.state)).unwrap(),
    )
    .unwrap();
    let changed = finish(
        &mut runtime,
        api::splice(&client, &snapshot, 2..7, "한글 🦀".as_bytes()),
    )
    .unwrap();
    assert_eq!(
        finish(&mut runtime, api::splice(&client, &snapshot, 0..0, b"stale")),
        Err(RequestError::Failed(Error::STALE))
    );
    let current = buffer::retained(
        &finish(&mut runtime, buffer::retain(&client, changed.state)).unwrap(),
    )
    .unwrap();
    let bytes = finish(
        &mut runtime,
        api::read(&client, &current, 0..api::length(&changed).unwrap()),
    )
    .unwrap();
    assert_eq!(
        bytes.bytes(),
        "# 한글 🦀\r\n"
            .as_bytes()
            .iter()
            .copied()
            .chain(b"\xff\0tail".iter().copied())
            .collect::<Vec<_>>()
    );
    assert_eq!(bytes.snapshot(), &current);
    assert_eq!(bytes.range(), &(0..api::length(&changed).unwrap()));
    let old = finish(
        &mut runtime,
        api::read(&client, &snapshot, 0..original.len() as u64),
    )
    .unwrap();
    assert_eq!(old.bytes(), original);
    let undo = buffer::edit(
        changed.state,
        Message::new(Contract::new("kernel.history.undo", 1), &[]),
    );
    finish(&mut runtime, client.write(changed.history, &undo)).unwrap();
    let restored = buffer::observation(
        finish(
            &mut runtime,
            client.read(
                first.state.buffer,
                &Message::new(Contract::new("source.bytes", 1), &[]),
            ),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(restored.state.revision > changed.state.revision);
    let restored = buffer::retained(
        &finish(&mut runtime, buffer::retain(&client, restored.state)).unwrap(),
    )
    .unwrap();
    assert_eq!(
        finish(
            &mut runtime,
            api::read(&client, &restored, 0..original.len() as u64)
        )
        .unwrap()
        .bytes(),
        original
    );
    runtime.close().unwrap();
}

#[test]
fn segmented_edits_and_all_windows_match_exact_byte_splicing() {
    let original = b"a\xff\0z";
    let retained = Source::new(original.as_slice().into()).unwrap();
    let mut source = Source::new(original.as_slice().into()).unwrap();
    let mut bytes = original.to_vec();
    let operations: &[(Range<usize>, &[u8])] = &[
        (1..1, b"BCD"),
        (0..2, b""),
        (3..5, b"\xc3\xa9\xfe"),
        (0..6, b""),
        (0..0, b"again"),
        (2..3, b"\0\xff"),
    ];
    for (range, replacement) in operations {
        bytes.splice(range.clone(), replacement.iter().copied());
        source = source
            .splice(range.start as u64..range.end as u64, replacement)
            .unwrap();
        check_windows(&source, &bytes);
    }
    check_windows(&retained, original);
    assert!(matches!(source.splice(0..u64::MAX, b"bad"), Err(Error::INVALID)));
    let mut untouched = vec![42];
    let reversed = Range { start: 4, end: 3 };
    assert_eq!(source.read(reversed, &mut untouched), Err(Error::INVALID));
    assert_eq!(untouched, [42]);
}

fn check_windows(source: &Source, bytes: &[u8]) {
    for start in 0..=bytes.len() {
        for end in start..=bytes.len() {
            let mut window = Vec::new();
            source.read(start as u64..end as u64, &mut window).unwrap();
            assert_eq!(window, bytes[start..end]);
        }
    }
}
