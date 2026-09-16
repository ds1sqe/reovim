use super::*;
use core::cell::{Cell, RefCell};
use reovim_kernel_interface::{
    Pending,
    call::{Cancellation, Completion, PendingOps},
    provider::{IoInfo, IoOps, ResourceOps, ResourceProvider},
};
use std::rc::Rc;

fn setup(provider: ResourceProvider) -> (Runtime, Caller, AccessId) {
    let mut runtime = Runtime::new([0x63; 16], Limits::default()).unwrap();
    let id = runtime
        .register(b"/io", provider, None)
        .map_err(|(error, _)| error)
        .unwrap();
    let caller = runtime.bind(&[grant(id, 7)]).unwrap().caller();
    let export = Rc::new(ExportedCaller::new(caller, LIMITS, 8).unwrap());
    let caller =
        unsafe { reovim_sdk::import_caller(export.raw(), export, LIMITS, 64) }
            .unwrap();
    runtime.start().unwrap();
    let cx = &mut Context::from_waker(Waker::noop());
    let opened = acquire(&caller, id, cx);
    (runtime, caller, opened)
}

fn flags(caller: &Caller, opened: AccessId, cx: &mut Context<'_>) {
    success(
        caller,
        call(
            caller,
            Operation::SetAccess,
            opened,
            Value::copy(&7_u32.to_le_bytes(), &[]).unwrap(),
        ),
        cx,
    );
}

fn read(
    caller: &Caller,
    opened: AccessId,
    offset: Option<u64>,
    maximum: u64,
) -> Call {
    let mut input =
        offset.map_or_else(Vec::new, |offset| offset.to_le_bytes().to_vec());
    input.extend_from_slice(&maximum.to_le_bytes());
    call(
        caller,
        if offset.is_some() { Operation::ReadAt } else { Operation::Read },
        opened,
        Value::copy(&input, &[]).unwrap(),
    )
}

fn finish(
    runtime: &mut Runtime,
    caller: &Caller,
    request: Call,
    cx: &mut Context<'_>,
) -> Rc<Outcome> {
    let id = request.request;
    caller.submit(request).unwrap();
    runtime.advance(cx).unwrap();
    runtime.advance(cx).unwrap();
    let result = outcome(caller, id, cx);
    caller.release(id).unwrap();
    runtime.advance(cx).unwrap();
    result
}

fn read_result(result: &Outcome, expected: &[u8], eof: bool) {
    let Outcome::Ok(value) = result else { panic!("read failed") };
    assert!(value.references.is_empty());
    let data = &value.data;
    let size = u32::from_le_bytes(data[..4].try_into().unwrap()) as usize;
    assert_eq!(&data[4..4 + size], expected);
    assert_eq!(&data[4 + size..], &[u8::from(eof)]);
}

#[test]
fn memory_provider_reads_opaque_bytes_with_explicit_eof_and_zero_right_open() {
    let (mut runtime, caller, opened) =
        setup(crate::memory::provider(b"\xffabc\0").unwrap());
    let cx = &mut Context::from_waker(Waker::noop());
    let denied = read(&caller, opened, Some(0), 2);
    assert_eq!(caller.submit(denied.clone()), Err(Error::DENIED));
    caller.release(denied.request).unwrap();
    flags(&caller, opened, cx);
    read_result(
        &finish(&mut runtime, &caller, read(&caller, opened, Some(0), 2), cx),
        b"\xffa",
        false,
    );
    read_result(
        &finish(&mut runtime, &caller, read(&caller, opened, Some(2), 16), cx),
        b"bc\0",
        true,
    );
    read_result(
        &finish(&mut runtime, &caller, read(&caller, opened, Some(100), 2), cx),
        b"",
        true,
    );
    read_result(
        &finish(&mut runtime, &caller, read(&caller, opened, Some(100), 0), cx),
        b"",
        false,
    );
    let sequential = read(&caller, opened, None, 2);
    assert_eq!(caller.submit(sequential.clone()), Err(Error::UNSUPPORTED));
    caller.release(sequential.request).unwrap();
    let overflow = read(&caller, opened, Some(u64::MAX), 2);
    assert_eq!(caller.submit(overflow.clone()), Err(Error::INVALID));
    caller.release(overflow.request).unwrap();
    runtime.advance(cx).unwrap();
    runtime.close().unwrap();
}

struct Probe {
    reads: Cell<u32>,
    writes: Cell<u32>,
    ready: Cell<bool>,
    bytes: RefCell<Vec<u8>>,
    unknown: bool,
    regress: bool,
}

fn device(unknown: bool, regress: bool) -> (ResourceProvider, Rc<Probe>) {
    let state = Rc::new(Probe {
        reads: Cell::new(0),
        writes: Cell::new(0),
        ready: Cell::new(false),
        bytes: RefCell::new(Vec::new()),
        unknown,
        regress,
    });
    let provider = ResourceProvider::new(
        state.clone(),
        ResourceOps {
            open: None,
            namespace: None,
            io: Some(IoOps {
                receive: None,
                send: None,
                info: IoInfo {
                    read_modes: 3,
                    write_modes: 1,
                    alignment: 2,
                    transfer_limit: 4,
                },
                read: Some(|state, _| {
                    state.reads.set(state.reads.get() + 1);
                    Ok(Pending::new(
                        state.clone(),
                        PendingOps {
                            poll: |state, _| Ok(read_ready(state)),
                            take: |_| {
                                Ok(Completion {
                                    outcome: Outcome::Ok(Value::copy(
                                        &[2, 0, 0, 0, 255, 0, 0],
                                        &[],
                                    )?),
                                    activity: 1,
                                    owner: None,
                                })
                            },
                            cancel: |_| Ok(Cancellation::TooLate),
                            reconcile: |_| Err(Error::UNSUPPORTED),
                            finish: |_| Ok(()),
                        },
                    ))
                }),
                write: Some(|state, write| Ok(write_device(state, &write))),
            }),
            stop: |_| Ok(()),
        },
        Box::new([]),
        Rc::new(()),
    )
    .map_err(|(error, _)| error)
    .unwrap();
    (provider, state)
}

const fn read_ready(state: &Probe) -> Poll<()> {
    if state.ready.get() { Poll::Ready(()) } else { Poll::Pending }
}

fn write_result(state: &Probe, reconciled: bool) -> Result<Completion, Error> {
    let count =
        if state.regress { 1 } else { state.bytes.borrow().len() as u64 };
    let outcome = if state.unknown && !reconciled {
        Outcome::Unknown {
            error: Error::UNAVAILABLE,
            progress: Some(Value::copy(&2_u64.to_le_bytes(), &[])?),
        }
    } else {
        Outcome::Ok(Value::copy(&count.to_le_bytes(), &[])?)
    };
    Ok(Completion { outcome, activity: 2, owner: None })
}

fn write_device(
    state: &Rc<Probe>,
    write: &reovim_kernel_interface::provider::Write<'_>,
) -> Pending {
    state.writes.set(state.writes.get() + 1);
    *state.bytes.borrow_mut() = write.data.to_vec();
    Pending::new(
        (state.clone(), false),
        PendingOps {
            poll: |_, _| Ok(Poll::Ready(())),
            take: |(state, reconciled)| write_result(state, *reconciled),
            cancel: |_| Ok(Cancellation::TooLate),
            reconcile: |(_, reconciled)| {
                *reconciled = true;
                Ok(())
            },
            finish: |_| Ok(()),
        },
    )
}

#[test]
fn zero_transfer_and_invalid_alignment_never_enter_provider() {
    let (provider, probe) = device(false, false);
    let (mut runtime, caller, opened) = setup(provider);
    let cx = &mut Context::from_waker(Waker::noop());
    flags(&caller, opened, cx);
    for (offset, maximum, expected) in [
        (1, 2, Error::INVALID),
        (0, 3, Error::INVALID),
        (0, 6, Error::PRESSURE),
    ] {
        let rejected = read(&caller, opened, Some(offset), maximum);
        assert_eq!(caller.submit(rejected.clone()), Err(expected));
        caller.release(rejected.request).unwrap();
    }
    read_result(
        &finish(&mut runtime, &caller, read(&caller, opened, None, 0), cx),
        b"",
        false,
    );
    let empty = call(&caller, Operation::Write, opened, bytes(&[]));
    assert_eq!(
        finish(&mut runtime, &caller, empty, cx).as_ref(),
        &Outcome::Ok(Value::copy(&0_u64.to_le_bytes(), &[]).unwrap())
    );
    assert_eq!((probe.reads.get(), probe.writes.get()), (0, 0));
    runtime.close().unwrap();
}

#[test]
fn pending_read_does_not_stall_independent_write_or_recovery_after_close() {
    let (provider, probe) = device(false, false);
    let (mut runtime, caller, opened) = setup(provider);
    let cx = &mut Context::from_waker(Waker::noop());
    flags(&caller, opened, cx);
    let pending = read(&caller, opened, None, 2);
    caller.submit(pending.clone()).unwrap();
    runtime.advance(cx).unwrap();
    let written = finish(
        &mut runtime,
        &caller,
        call(&caller, Operation::Write, opened, bytes(b"\xff\0ab")),
        cx,
    );
    assert_eq!(
        written.as_ref(),
        &Outcome::Ok(Value::copy(&4_u64.to_le_bytes(), &[]).unwrap())
    );
    assert_eq!(probe.bytes.borrow().as_slice(), b"\xff\0ab");
    let close = call(&caller, Operation::Close, opened, Value::unit());
    caller.submit(close.clone()).unwrap();
    runtime.advance(cx).unwrap();
    assert_eq!(caller.snapshot(close.request).unwrap(), Status::Pending);
    caller.submit(pending.clone()).unwrap();
    probe.ready.set(true);
    runtime.advance(cx).unwrap();
    let result = outcome(&caller, pending.request, cx);
    read_result(&result, b"\xff\0", false);
    caller.release(pending.request).unwrap();
    runtime.advance(cx).unwrap();
    assert_eq!(probe.reads.get(), 1);
    assert_eq!(
        outcome(&caller, close.request, cx).as_ref(),
        &Outcome::Ok(Value::unit())
    );
    caller.release(close.request).unwrap();
    runtime.advance(cx).unwrap();
    runtime.close().unwrap();
}

#[test]
fn uncertain_write_refines_original_count_without_repeating_transfer() {
    write_recovery(false);
}

#[test]
fn regressing_transfer_count_fences_with_previous_known_progress() {
    write_recovery(true);
}

fn write_recovery(regress: bool) {
    let (provider, probe) = device(true, regress);
    let (mut runtime, caller, opened) = setup(provider);
    let cx = &mut Context::from_waker(Waker::noop());
    flags(&caller, opened, cx);
    let request = call(&caller, Operation::Write, opened, bytes(b"data"));
    caller.submit(request.clone()).unwrap();
    runtime.advance(cx).unwrap();
    runtime.advance(cx).unwrap();
    let unknown = outcome(&caller, request.request, cx);
    assert_eq!(
        unknown.as_ref(),
        &Outcome::Unknown {
            error: Error::UNAVAILABLE,
            progress: Some(Value::copy(&2_u64.to_le_bytes(), &[]).unwrap())
        }
    );
    caller.submit(request.clone()).unwrap();
    caller.reconcile(request.request).unwrap();
    runtime.advance(cx).unwrap();
    let result = outcome(&caller, request.request, cx);
    assert_eq!(probe.writes.get(), 1);
    caller.release(request.request).unwrap();
    if regress {
        assert_eq!(
            result.as_ref(),
            &Outcome::Unknown {
                error: Error::PROVIDER,
                progress: Some(Value::copy(&2_u64.to_le_bytes(), &[]).unwrap())
            }
        );
        assert_eq!(runtime.close(), Err(Error::INCOMPLETE));
    } else {
        assert_eq!(
            result.as_ref(),
            &Outcome::Ok(Value::copy(&4_u64.to_le_bytes(), &[]).unwrap())
        );
        runtime.advance(cx).unwrap();
        runtime.close().unwrap();
    }
}

#[test]
fn native_byte_provider_transfers_opaque_values_and_drains_ownership() {
    let library = Library::compile_source("io.c");
    let raw = library.create();
    let provider = unsafe {
        reovim_sdk::import_provider(&raw const raw, library.clone(), LIMITS)
    }
    .unwrap();
    let (mut runtime, caller, opened) = setup(provider);
    let cx = &mut Context::from_waker(Waker::noop());
    flags(&caller, opened, cx);
    let written = finish(
        &mut runtime,
        &caller,
        call(&caller, Operation::Write, opened, bytes(b"\xff\0ab")),
        cx,
    );
    assert_eq!(
        written.as_ref(),
        &Outcome::Ok(Value::copy(&4_u64.to_le_bytes(), &[]).unwrap())
    );
    read_result(
        &finish(&mut runtime, &caller, read(&caller, opened, Some(1), 8), cx),
        b"\0ab",
        true,
    );
    read_result(
        &finish(&mut runtime, &caller, read(&caller, opened, Some(0), 0), cx),
        b"",
        false,
    );
    assert_eq!(library.count(c"fixture_reads"), 1);
    assert_eq!(library.count(c"fixture_writes"), 1);
    runtime.close().unwrap();
    drop(runtime);
    drop(caller);
    assert_eq!(library.count(c"fixture_destroyed"), 1);
}

#[test]
fn native_io_rejection_keeps_provider_owned_by_supplier() {
    let library = Library::compile_source("io.c");
    let mut raw = library.create();
    let original = raw.io;
    let table = unsafe { *original };
    for invalid in [
        reovim_kernel_interface::abi::IoOps { read: None, ..table },
        reovim_kernel_interface::abi::IoOps { write_modes: 4, ..table },
        reovim_kernel_interface::abi::IoOps { alignment: 0, ..table },
        reovim_kernel_interface::abi::IoOps { bytes: 0, ..table },
    ] {
        raw.io = &raw const invalid;
        let rejected = unsafe {
            reovim_sdk::import_provider(&raw const raw, library.clone(), LIMITS)
        };
        assert!(matches!(rejected, Err(Error::INVALID)));
        assert_eq!(library.count(c"fixture_destroyed"), 0);
    }
    raw.io = original;
    let mut provider = unsafe {
        reovim_sdk::import_provider(&raw const raw, library.clone(), LIMITS)
    }
    .unwrap();
    provider.stop().unwrap();
    drop(provider);
    assert_eq!(library.count(c"fixture_destroyed"), 1);
}

#[test]
fn resource_info_exposes_canonical_packet_types_without_byte_directions() {
    let library = Library::compile_source("packet.c");
    let raw = library.create();
    let provider = unsafe {
        reovim_sdk::import_provider(&raw const raw, library.clone(), LIMITS)
    }
    .unwrap();
    let packet = provider.receive_type().unwrap().clone();
    let (mut runtime, caller, opened) = setup(provider);
    let cx = &mut Context::from_waker(Waker::noop());
    let info = success(
        &caller,
        call(&caller, Operation::Info, opened, Value::unit()),
        cx,
    );
    assert_eq!(&info.data[44..48], &1_u32.to_le_bytes());
    assert_eq!(&info.data[48..56], &[0; 8]);
    let receive = packet_descriptor(&info.data[72..]);
    assert_eq!(receive.0, packet);
    let send = packet_descriptor(receive.1);
    assert_eq!(send.0, packet);
    assert_eq!(send.1, &0_u32.to_le_bytes());
    assert_eq!(library.count(c"fixture_received"), 0);
    assert_eq!(library.count(c"fixture_sent"), 0);
    runtime.advance(cx).unwrap();
    runtime.close().unwrap();
    assert_eq!(library.count(c"fixture_destroyed"), 1);
}

fn packet_descriptor(
    bytes: &[u8],
) -> (reovim_kernel_interface::declaration::ValueType, &[u8]) {
    let length = u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize;
    let packet = reovim_kernel_interface::declaration::ValueType::decode(
        &bytes[4..4 + length],
    )
    .unwrap();
    (packet, &bytes[4 + length..])
}

#[path = "packet_runtime.rs"]
mod packets;
