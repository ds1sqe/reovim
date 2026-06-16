//! Tests for `stream.rs` — stream state, control ops, and backpressure.

use core::num::NonZeroU32;

use reovim_arch::{arch_test, ds::Bytes};

use crate::{
    id::{BufferId, CdylibId, SessionId, StreamId},
    routing::DomainApiVersion,
    stream::{
        BackpressureBlock, BackpressureCounters, StreamControlClass, StreamControlOp, StreamHandle,
        StreamOpts, StreamScheme, StreamState,
    },
};

fn owner(raw: u32) -> CdylibId {
    CdylibId::new(NonZeroU32::new(raw).unwrap())
}

arch_test!(stream_state_accepts_emit_only_while_running_or_blocked, {
    assert!(StreamState::Running.accepts_emit());
    assert!(StreamState::BackpressureBlocked.accepts_emit());
    assert!(!StreamState::Init.accepts_emit());
    assert!(!StreamState::Stale.accepts_emit());
    assert!(StreamState::Closed.is_closed());
});

arch_test!(backpressure_counters_report_first_blocking_cap, {
    assert_eq!(
        BackpressureCounters::new(20, 99, 0).blocked_by(10, 16),
        Some(BackpressureBlock::InFlight)
    );
    assert_eq!(
        BackpressureCounters::new(5, 17, 0).blocked_by(10, 16),
        Some(BackpressureBlock::Buffered)
    );
    assert_eq!(BackpressureCounters::new(5, 15, 0).blocked_by(10, 16), None);
});

arch_test!(stream_control_ops_follow_s5_partition, {
    assert_eq!(StreamControlOp::new(0).class(), StreamControlClass::Invalid);
    assert_eq!(StreamControlOp::KERNEL_INSPECT.class(), StreamControlClass::Kernel);
    assert_eq!(StreamControlOp::new(101).class(), StreamControlClass::SchemePrivate);
    assert_eq!(StreamControlOp::new(256).class(), StreamControlClass::Reserved);
});

arch_test!(stream_scheme_and_options_keep_typed_payload_bytes, {
    let scheme = StreamScheme::new(
        Bytes::try_from_slice(b"pty").expect("alloc"),
        owner(1),
        DomainApiVersion::new(0, 16),
    );
    assert_eq!(scheme.name.as_slice(), b"pty");

    let opts = StreamOpts::new(
        Bytes::try_from_slice(b"pty://1").expect("alloc"),
        Bytes::try_from_slice(&[1, 2, 3]).expect("alloc"),
    );
    assert_eq!(opts.url.as_slice(), b"pty://1");
    assert_eq!(opts.scheme_opts.as_slice(), &[1, 2, 3]);
});

arch_test!(stream_handle_carries_session_buffer_and_backpressure_state, {
    let handle = StreamHandle::new(
        StreamId::new(7),
        Bytes::try_from_slice(b"watch").expect("alloc"),
        StreamState::Running,
        Some(SessionId::new(1)),
        Some(BufferId::new(2)),
        BackpressureCounters::new(3, 4, 5),
    );
    assert_eq!(handle.id, StreamId::new(7));
    assert_eq!(handle.scheme.as_slice(), b"watch");
    assert_eq!(handle.session_id, Some(SessionId::new(1)));
    assert_eq!(handle.buffer_id, Some(BufferId::new(2)));
    assert_eq!(handle.backpressure.bytes_buffered, 4);
});
