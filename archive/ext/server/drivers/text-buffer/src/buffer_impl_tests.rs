//! In-process tests for [`crate::TextBufferImpl`].

use {
    crate::{BufferOps, TextBufferImpl},
    parking_lot::Mutex as PMutex,
    reovim_arch::sync::RwLock as ArchRwLock,
    reovim_content_codec::ByteNotifiable,
    reovim_kernel::api::v1::ByteEdit,
    reovim_provider_text::Buffer as ProviderBuffer,
    reovim_subsys_buffer::{Buffer, BufferError, BufferSubscribable},
    std::sync::Arc,
    tokio::sync::broadcast::error::TryRecvError,
};

// ── Helpers ────────────────────────────────────────────────────────────

fn make_buffer(content: &str) -> Arc<TextBufferImpl> {
    let pb = ProviderBuffer::from_string(content);
    let id = pb.id();
    let inner: Arc<ArchRwLock<dyn BufferOps>> = Arc::new(ArchRwLock::new(pb));
    Arc::new(TextBufferImpl::new(id, inner, None))
}

/// Records every `notify` call for assertions.
struct RecorderCodec {
    log: Arc<PMutex<Vec<ByteEdit>>>,
    built_with: Arc<PMutex<Option<Vec<u8>>>>,
}

impl ByteNotifiable for RecorderCodec {
    fn build(&mut self, raw: &[u8]) {
        *self.built_with.lock() = Some(raw.to_vec());
    }
    fn notify(&mut self, edit: &ByteEdit) {
        self.log.lock().push(edit.clone());
    }
}

/// Panics on `notify`. The buffer must isolate the panic and keep the
/// slot in place.
struct PanickingCodec;

impl ByteNotifiable for PanickingCodec {
    fn build(&mut self, _raw: &[u8]) {}
    fn notify(&mut self, _edit: &ByteEdit) {
        panic!("intentional codec panic");
    }
}

// ── Tests ──────────────────────────────────────────────────────────────

#[test]
fn attach_then_detach_round_trip() {
    let buf = make_buffer("hello");
    let log = Arc::new(PMutex::new(Vec::new()));
    let built = Arc::new(PMutex::new(None));
    let id = buf
        .attach_codec(
            "rec",
            Box::new(RecorderCodec {
                log: Arc::clone(&log),
                built_with: Arc::clone(&built),
            }),
        )
        .expect("attach succeeds");
    assert_eq!(built.lock().as_deref(), Some(b"hello".as_slice()));

    let listed = buf.list_codecs();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].0, id);
    assert_eq!(listed[0].1, "rec");

    let _detached = buf.detach_codec(id).expect("detach succeeds");
    assert!(buf.list_codecs().is_empty());
}

#[test]
fn attach_propagates_edit_via_notify() {
    let buf = make_buffer("ab");
    let log = Arc::new(PMutex::new(Vec::new()));
    let built = Arc::new(PMutex::new(None));
    buf.attach_codec(
        "rec",
        Box::new(RecorderCodec {
            log: log.clone(),
            built_with: built,
        }),
    )
    .unwrap();

    buf.apply_edit(ByteEdit::insert(2, b"c")).unwrap();
    buf.apply_edit(ByteEdit::insert(3, b"d")).unwrap();

    let entries = log.lock().clone();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0], ByteEdit::insert(2, b"c"));
    assert_eq!(entries[1], ByteEdit::insert(3, b"d"));
}

#[test]
fn attach_panicked_slot_does_not_break_others() {
    let buf = make_buffer("xy");
    let log = Arc::new(PMutex::new(Vec::new()));
    let built = Arc::new(PMutex::new(None));
    buf.attach_codec("panic", Box::new(PanickingCodec)).unwrap();
    buf.attach_codec(
        "rec",
        Box::new(RecorderCodec {
            log: log.clone(),
            built_with: built,
        }),
    )
    .unwrap();

    // First edit triggers the panicking slot AND the recorder.
    buf.apply_edit(ByteEdit::insert(2, b"z")).unwrap();
    // Second edit should still be delivered to the recorder despite
    // the panicking slot being "poisoned" — the subsys contract says
    // panicked slots stay attached.
    buf.apply_edit(ByteEdit::insert(3, b"w")).unwrap();

    let entries = log.lock().clone();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0], ByteEdit::insert(2, b"z"));
    assert_eq!(entries[1], ByteEdit::insert(3, b"w"));
    // Panicked slot still in the table.
    assert_eq!(buf.list_codecs().len(), 2);
}

#[test]
fn double_attach_same_name_errors() {
    let buf = make_buffer("");
    let log = Arc::new(PMutex::new(Vec::new()));
    let built = Arc::new(PMutex::new(None));
    buf.attach_codec(
        "dup",
        Box::new(RecorderCodec {
            log: log.clone(),
            built_with: built.clone(),
        }),
    )
    .unwrap();
    let err = buf
        .attach_codec(
            "dup",
            Box::new(RecorderCodec {
                log,
                built_with: built,
            }),
        )
        .unwrap_err();
    assert!(matches!(err, BufferError::DuplicateCodecName(ref s) if s == "dup"));
}

#[test]
fn detach_unknown_id_errors() {
    let buf = make_buffer("");
    // Allocate-then-detach to get an id we know is invalid.
    let log = Arc::new(PMutex::new(Vec::new()));
    let built = Arc::new(PMutex::new(None));
    let id = buf
        .attach_codec(
            "rec",
            Box::new(RecorderCodec {
                log,
                built_with: built,
            }),
        )
        .unwrap();
    buf.detach_codec(id).unwrap();
    let result = buf.detach_codec(id);
    assert!(matches!(result, Err(BufferError::NotFound(_))));
}

#[tokio::test]
async fn subscribe_edits_observes_apply_edit() {
    let buf = make_buffer("a");
    let mut rx = buf.subscribe_edits();
    buf.apply_edit(ByteEdit::insert(1, b"b")).unwrap();
    let edit = rx.recv().await.unwrap();
    assert_eq!(edit, ByteEdit::insert(1, b"b"));
}

#[test]
fn subscribe_edits_lags_when_receiver_falls_behind() {
    use tokio::sync::broadcast::error::TryRecvError as TR;
    let buf = make_buffer("");
    let mut rx = buf.subscribe_edits();
    // Send more edits than the channel capacity so the receiver lags.
    for i in 0..300_usize {
        buf.apply_edit(ByteEdit::insert(i, b"x")).unwrap();
    }
    // The first try_recv on a lagged channel must surface `Lagged`.
    let outcome = rx.try_recv();
    assert!(matches!(outcome, Err(TR::Lagged(_))), "expected Lagged, got {outcome:?}");
}

#[test]
fn read_bytes_round_trips_apply_edit() {
    let buf = make_buffer("hello");
    buf.apply_edit(ByteEdit::insert(5, b"!")).unwrap();
    let got = buf.read_bytes(0..6).unwrap();
    assert_eq!(got, b"hello!".to_vec());
}

#[test]
fn read_bytes_rejects_out_of_bounds_range() {
    let buf = make_buffer("hi");
    let err = buf.read_bytes(0..10).unwrap_err();
    assert!(matches!(err, BufferError::InvalidEdit(_)));
}

#[test]
fn read_bytes_rejects_inverted_range() {
    let buf = make_buffer("hi");
    #[allow(clippy::reversed_empty_ranges)]
    let result = buf.read_bytes(2..1);
    assert!(matches!(result, Err(BufferError::InvalidEdit(_))));
}

#[test]
fn is_modified_set_after_edit() {
    let buf = make_buffer("a");
    assert!(!buf.is_modified());
    buf.apply_edit(ByteEdit::insert(1, b"b")).unwrap();
    assert!(buf.is_modified());
}

#[test]
fn apply_edit_rejects_out_of_bounds() {
    let buf = make_buffer("a");
    let err = buf.apply_edit(ByteEdit::delete(5, b"xyz")).unwrap_err();
    assert!(matches!(err, BufferError::InvalidEdit(_)));
}

#[test]
fn write_to_emits_full_content() {
    let buf = make_buffer("hello");
    let mut sink = Vec::new();
    buf.write_to(&mut sink).unwrap();
    assert_eq!(sink, b"hello".to_vec());
}

#[test]
fn file_path_round_trip() {
    let buf = make_buffer("");
    assert!(buf.file_path().is_none());
    buf.set_file_path(Some("/tmp/x".into())).unwrap();
    assert_eq!(buf.file_path().as_deref(), Some("/tmp/x"));
    buf.set_file_path(None).unwrap();
    assert!(buf.file_path().is_none());
}

#[test]
fn size_matches_inner_byte_len() {
    let buf = make_buffer("hello");
    assert_eq!(buf.size(), 5);
    buf.apply_edit(ByteEdit::insert(5, b"!")).unwrap();
    assert_eq!(buf.size(), 6);
}

// Defensively assert that a fresh subscriber observes no historical
// edits — broadcast subscribers only see edits sent after subscribe.
#[tokio::test]
async fn fresh_subscriber_skips_prior_edits() {
    let buf = make_buffer("");
    buf.apply_edit(ByteEdit::insert(0, b"a")).unwrap();
    let mut rx = buf.subscribe_edits();
    assert!(matches!(rx.try_recv(), Err(TryRecvError::Empty)));
    buf.apply_edit(ByteEdit::insert(1, b"b")).unwrap();
    let edit = rx.recv().await.unwrap();
    assert_eq!(edit, ByteEdit::insert(1, b"b"));
}
