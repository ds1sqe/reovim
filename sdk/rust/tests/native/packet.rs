use super::*;
use reovim_kernel_interface::{
    Pending, abi,
    provider::{Oversize, Read, Receive, ResourceProvider},
};
use std::rc::Rc;

fn provider(roundtrip: bool) -> (Rc<Library>, ResourceProvider) {
    let library = Library::compile_source("packet.c");
    let raw = library.create();
    let provider = unsafe {
        reovim_sdk::import_provider(&raw const raw, library.clone(), LIMITS)
    }
    .unwrap();
    if !roundtrip {
        return (library, provider);
    }
    let raw = reovim_sdk::ExportedProvider::new(provider, LIMITS)
        .ok()
        .unwrap()
        .into_raw();
    let provider = unsafe {
        reovim_sdk::import_provider(&raw const raw, library.clone(), LIMITS)
    }
    .unwrap();
    (library, provider)
}

fn complete(mut pending: Pending, activity: u8) -> Value {
    let cx = &mut Context::from_waker(Waker::noop());
    assert_eq!(pending.poll(cx), Ok(Poll::Ready(())));
    let completion = pending.take().unwrap();
    assert_eq!(completion.activity, activity);
    let Outcome::Ok(value) = &completion.outcome else {
        panic!("{:?}", completion.outcome)
    };
    let value = value.clone();
    drop(completion);
    pending.finish().unwrap();
    value
}

fn received(
    provider: &mut ResourceProvider,
    maximum: u64,
    oversize: Oversize,
) -> Value {
    complete(provider.receive(Receive { maximum, oversize }).unwrap(), 1)
}

fn item(
    value: &Value,
    sender: u32,
    data: &[u8],
    truncated: bool,
    original: u64,
) {
    let mut expected = 0_u32.to_le_bytes().to_vec();
    expected.extend_from_slice(&sender.to_le_bytes());
    expected
        .extend_from_slice(&u32::try_from(data.len()).unwrap().to_le_bytes());
    expected.extend_from_slice(data);
    expected.push(u8::from(truncated));
    expected.extend_from_slice(&1_u32.to_le_bytes());
    expected.extend_from_slice(&original.to_le_bytes());
    assert_eq!(value.data.as_ref(), expected);
    assert!(value.references.is_empty());
}

#[test]
fn native_packet_roundtrip_preserves_empty_items_metadata_and_oversize_policy()
{
    for roundtrip in [false, true] {
        let (library, mut provider) = provider(roundtrip);
        assert_eq!(
            provider.receive_type().unwrap().bytes(),
            b"\x09\x02\0\x06\0sender\x03\x07\0payload\x07\x08\0\0\0"
        );
        assert_eq!(provider.receive_type(), provider.send_type());
        assert_eq!(
            provider.read(Read { offset: None, maximum: 1 }).err(),
            Some(Error::UNSUPPORTED)
        );
        item(&received(&mut provider, 0, Oversize::Reject), 41, &[], false, 0);
        assert_eq!(
            provider
                .receive(Receive { maximum: 2, oversize: Oversize::Reject })
                .err(),
            Some(Error::PRESSURE)
        );
        assert_eq!(library.count(c"fixture_received"), 1);
        item(
            &received(&mut provider, 2, Oversize::Truncate),
            42,
            &[b'a', 0xff],
            true,
            3,
        );
        let dropped = received(&mut provider, 0, Oversize::Discard);
        let mut expected = vec![1, 0, 0, 0, 1, 0, 0, 0];
        expected.extend_from_slice(&1_u64.to_le_bytes());
        assert_eq!(dropped.data.as_ref(), expected);
        assert_eq!(
            received(&mut provider, 8, Oversize::Reject).data.as_ref(),
            &[2, 0, 0, 0]
        );
        provider.stop().unwrap();
        drop(provider);
        assert_eq!(library.count(c"fixture_destroyed"), 1);
    }
}

#[test]
fn native_packet_send_accepts_empty_and_binary_payloads_without_byte_io() {
    let (library, mut provider) = provider(true);
    for data in [&[][..], &[0, 0xff, b'z'][..]] {
        let mut bytes = 77_u32.to_le_bytes().to_vec();
        bytes.extend_from_slice(
            &u32::try_from(data.len()).unwrap().to_le_bytes(),
        );
        bytes.extend_from_slice(data);
        let value = Value::copy(&bytes, &[]).unwrap();
        let receipt = complete(provider.send(&value).unwrap(), 2);
        assert_eq!(number(&receipt), data.len() as u64);
        assert_eq!(library.count(c"fixture_sender"), 77);
        assert_eq!(library.count(c"fixture_size"), data.len() as u64);
    }
    let byte: unsafe extern "C" fn(u32) -> u64 =
        unsafe { library.symbol(c"fixture_byte") };
    assert_eq!(unsafe { byte(0) }, 0);
    assert_eq!(unsafe { byte(1) }, 0xff);
    assert_eq!(unsafe { byte(2) }, u64::from(b'z'));
    assert_eq!(library.count(c"fixture_sent"), 2);
    provider.stop().unwrap();
    assert_eq!(provider.send(&Value::unit()).err(), Some(Error::CLOSED));
    drop(provider);
    assert_eq!(library.count(c"fixture_destroyed"), 1);
}

#[test]
fn uncertain_receive_retains_the_original_packet_and_code_until_cleanup() {
    let (library, mut provider) = provider(true);
    let set: unsafe extern "C" fn() =
        unsafe { library.symbol(c"fixture_uncertain") };
    unsafe { set() };
    let mut pending = provider
        .receive(Receive { maximum: 8, oversize: Oversize::Reject })
        .unwrap();
    let cx = &mut Context::from_waker(Waker::noop());
    assert_eq!(pending.poll(cx), Ok(Poll::Ready(())));
    let unknown = pending.take().unwrap();
    assert!(matches!(unknown.outcome, Outcome::Unknown { .. }));
    assert_eq!(provider.stop(), Err(Error::BUSY));
    assert_eq!(pending.reconcile(), Err(Error::BUSY));
    drop(unknown);
    pending.reconcile().unwrap();
    assert_eq!(library.count(c"fixture_received"), 1);
    let code = Rc::downgrade(&library);
    drop(library);
    assert!(code.upgrade().is_some());
    item(&complete(pending, 1), 41, &[], false, 0);
    provider.stop().unwrap();
    drop(provider);
    assert!(code.upgrade().is_none());
}

#[test]
fn malformed_packet_table_rejects_without_taking_supplier_ownership() {
    let library = Library::compile_source("packet.c");
    let raw = library.create();
    let table = unsafe { *raw.io };
    let malformed = [255];
    for io in [
        abi::IoOps { receive: None, ..table },
        abi::IoOps {
            send_type: abi::Bytes { ptr: core::ptr::null(), len: 0 },
            ..table
        },
        abi::IoOps {
            receive_type: abi::Bytes { ptr: malformed.as_ptr(), len: 1 },
            ..table
        },
    ] {
        let offered = abi::Provider { io: &raw const io, ..raw };
        assert_eq!(
            unsafe {
                reovim_sdk::import_provider(
                    &raw const offered,
                    library.clone(),
                    LIMITS,
                )
            }
            .err(),
            Some(Error::INVALID)
        );
        assert_eq!(library.count(c"fixture_destroyed"), 0);
    }
    let mut provider = unsafe {
        reovim_sdk::import_provider(&raw const raw, library.clone(), LIMITS)
    }
    .unwrap();
    provider.stop().unwrap();
    drop(provider);
    assert_eq!(library.count(c"fixture_destroyed"), 1);
}

#[test]
fn uncertain_send_recovers_local_acceptance_without_resending() {
    let (library, mut provider) = provider(true);
    let set: unsafe extern "C" fn() =
        unsafe { library.symbol(c"fixture_uncertain") };
    unsafe { set() };
    let packet = Value::copy(&[77, 0, 0, 0, 2, 0, 0, 0, 0xff, 0], &[]).unwrap();
    let mut pending = provider.send(&packet).unwrap();
    let cx = &mut Context::from_waker(Waker::noop());
    assert_eq!(pending.poll(cx), Ok(Poll::Ready(())));
    let unknown = pending.take().unwrap();
    assert!(matches!(unknown.outcome, Outcome::Unknown { progress: None, .. }));
    assert_eq!(pending.finish(), Err(Error::BUSY));
    drop(unknown);
    pending.reconcile().unwrap();
    assert_eq!(number(&complete(pending, 2)), 2);
    assert_eq!(library.count(c"fixture_sent"), 1);
    provider.stop().unwrap();
    drop(provider);
    assert_eq!(library.count(c"fixture_destroyed"), 1);
}

#[test]
fn absent_receive_does_not_disable_an_independent_send_direction() {
    let library = Library::compile_source("packet.c");
    let mut raw = library.create();
    let table = abi::IoOps {
        receive: None,
        receive_type: abi::Bytes { ptr: core::ptr::null(), len: 0 },
        ..unsafe { *raw.io }
    };
    raw.io = &raw const table;
    let mut provider = unsafe {
        reovim_sdk::import_provider(&raw const raw, library.clone(), LIMITS)
    }
    .unwrap();
    assert!(provider.receive_type().is_none());
    assert!(provider.send_type().is_some());
    assert_eq!(
        provider
            .receive(Receive { maximum: 0, oversize: Oversize::Reject })
            .err(),
        Some(Error::UNSUPPORTED)
    );
    let packet = Value::copy(&[77, 0, 0, 0, 0, 0, 0, 0], &[]).unwrap();
    assert_eq!(number(&complete(provider.send(&packet).unwrap(), 2)), 0);
    assert_eq!(library.count(c"fixture_received"), 0);
    assert_eq!(library.count(c"fixture_sent"), 1);
    provider.stop().unwrap();
    drop(provider);
    assert_eq!(library.count(c"fixture_destroyed"), 1);
}

#[test]
fn native_receive_export_rejects_invalid_options_before_provider_entry() {
    let (library, provider) = provider(false);
    let exported =
        reovim_sdk::ExportedProvider::new(provider, LIMITS).ok().unwrap();
    let raw = exported.raw();
    let receive = unsafe { (*raw.io).receive.unwrap() };
    let mut bytes = [0; 12];
    bytes[8] = 3; // No fourth oversize choice exists.
    let input = abi::Args {
        data: abi::Bytes { ptr: bytes.as_ptr(), len: bytes.len() },
        refs: abi::Refs { ptr: core::ptr::null(), len: 0 },
    };
    let mut pending =
        abi::Pending { context: core::ptr::null_mut(), ops: core::ptr::null() };
    let mut error = abi::Error::default();
    assert_eq!(
        unsafe {
            receive(
                raw.context,
                &raw const input,
                &raw mut pending,
                &raw mut error,
            )
        },
        Error::INVALID.code.0
    );
    assert_eq!(error.code, Error::INVALID.code.0);
    assert!(pending.context.is_null());
    assert_eq!(library.count(c"fixture_received"), 0);
    let mut provider = exported.into_provider();
    provider.stop().unwrap();
    drop(provider);
    assert_eq!(library.count(c"fixture_destroyed"), 1);
}
