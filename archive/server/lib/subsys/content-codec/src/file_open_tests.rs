//! Tests for `decode_file_bytes` — every branch covered.
//!
//! Coverage hits all four control-flow conditions in
//! [`super::decode_file_bytes`]: the size gate, the classifier outcome,
//! the factory outcome, and the codec decode outcome. Plus error
//! formatting and the standard-error-trait exposure.

use {
    crate::file_open::{FileOpenError, MAX_FILE_SIZE, decode_file_bytes},
    reovim_content_codec::{
        Annotation, CodecError, CodecMetadata, ContentClassifier, ContentClassifierStore,
        ContentCodec, ContentCodecFactory, ContentCodecFactoryStore, ContentType, DecodeResult,
    },
    std::sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

const TEST_TYPE: &str = "test/echo";

/// Classifier that maps a byte prefix to a fixed content type. Used to
/// drive the classifier-match arm without depending on real codec impls.
struct PrefixClassifier {
    prefix: Vec<u8>,
    content_type: ContentType,
    last_path: Arc<Mutex<Option<String>>>,
}

impl ContentClassifier for PrefixClassifier {
    fn classify(&self, raw: &[u8], path: &str) -> Option<ContentType> {
        *self.last_path.lock().unwrap() = Some(path.to_owned());
        if raw.starts_with(&self.prefix) {
            Some(self.content_type.clone())
        } else {
            None
        }
    }
    fn name(&self) -> &'static str {
        "test-prefix-classifier"
    }
}

/// Codec that returns a fixed string on `decode`. `fail_count` lets a
/// test force the first N decode calls to error out, exercising the
/// codec-decode-failure fallback arm.
struct EchoCodec {
    output: &'static str,
    fail_count: AtomicUsize,
}

impl ContentCodec for EchoCodec {
    fn decode(&self, _raw: &[u8]) -> Result<DecodeResult, CodecError> {
        if self.fail_count.fetch_sub(1, Ordering::SeqCst) > 0 {
            return Err(CodecError::InvalidSequence {
                offset: 0,
                detail: "test-injected".into(),
            });
        }
        Ok(DecodeResult {
            content: self.output.to_string(),
            annotations: Vec::<Annotation>::new(),
            metadata: CodecMetadata::new(ContentType::new(TEST_TYPE)),
            lossy: false,
            readonly: false,
            truncated: false,
        })
    }
}

struct EchoFactory {
    codec: Arc<EchoCodec>,
    matches_type: ContentType,
}

impl ContentCodecFactory for EchoFactory {
    fn create(&self, content_type: &ContentType) -> Option<Arc<dyn ContentCodec>> {
        if content_type == &self.matches_type {
            Some(self.codec.clone() as Arc<dyn ContentCodec>)
        } else {
            None
        }
    }
    fn supported_content_types(&self) -> Vec<&str> {
        vec![TEST_TYPE]
    }
    fn name(&self) -> &'static str {
        "test-echo-factory"
    }
}

fn empty_stores() -> (ContentClassifierStore, ContentCodecFactoryStore) {
    (ContentClassifierStore::new(), ContentCodecFactoryStore::new())
}

fn classifier_with_prefix(
    prefix: &[u8],
    content_type: ContentType,
) -> (Arc<PrefixClassifier>, ContentClassifierStore) {
    let store = ContentClassifierStore::new();
    let classifier = Arc::new(PrefixClassifier {
        prefix: prefix.to_vec(),
        content_type,
        last_path: Arc::new(Mutex::new(None)),
    });
    store.add(classifier.clone());
    (classifier, store)
}

fn factory_with_codec(
    matches_type: ContentType,
    output: &'static str,
    fail_count: usize,
) -> (Arc<EchoCodec>, ContentCodecFactoryStore) {
    let factory_store = ContentCodecFactoryStore::new();
    let codec = Arc::new(EchoCodec {
        output,
        fail_count: AtomicUsize::new(fail_count),
    });
    factory_store.add_factory(Arc::new(EchoFactory {
        codec: codec.clone(),
        matches_type,
    }));
    (codec, factory_store)
}

#[test]
fn rejects_bytes_above_max_size() {
    let (classifiers, factories) = empty_stores();
    let bytes = vec![0_u8; MAX_FILE_SIZE + 1];
    let result = decode_file_bytes(&bytes, "", &classifiers, &factories);
    assert_eq!(
        result,
        Err(FileOpenError::TooLarge {
            len: MAX_FILE_SIZE + 1,
            limit: MAX_FILE_SIZE,
        })
    );
}

#[test]
fn accepts_bytes_at_max_size() {
    let (classifiers, factories) = empty_stores();
    let bytes = vec![b'a'; MAX_FILE_SIZE];
    let (text, ct) = decode_file_bytes(&bytes, "", &classifiers, &factories)
        .expect("MAX-sized valid UTF-8 must succeed");
    assert_eq!(text.len(), MAX_FILE_SIZE);
    assert!(ct.is_none());
}

#[test]
fn empty_bytes_decode_to_empty_string() {
    let (classifiers, factories) = empty_stores();
    let (text, ct) = decode_file_bytes(b"", "", &classifiers, &factories).unwrap();
    assert!(text.is_empty());
    assert!(ct.is_none());
}

#[test]
fn no_classifier_match_valid_utf8_returns_text_none() {
    let (classifiers, factories) = empty_stores();
    let (text, ct) = decode_file_bytes(b"hello", "file.txt", &classifiers, &factories).unwrap();
    assert_eq!(text, "hello");
    assert!(ct.is_none());
}

#[test]
fn no_classifier_match_invalid_utf8_returns_not_utf8() {
    let (classifiers, factories) = empty_stores();
    let bytes = b"abc\xFFdef";
    let err = decode_file_bytes(bytes, "", &classifiers, &factories).unwrap_err();
    assert_eq!(err, FileOpenError::NotUtf8 { offset: 3 });
}

#[test]
fn classifier_match_with_codec_returns_decoded_text() {
    let ct = ContentType::new(TEST_TYPE);
    let (_, classifiers) = classifier_with_prefix(b"\x7fELF", ct.clone());
    let (_, factories) = factory_with_codec(ct.clone(), "[ELF summary]", 0);

    let (text, returned_ct) =
        decode_file_bytes(b"\x7fELF\x00rest", "binary", &classifiers, &factories).unwrap();
    assert_eq!(text, "[ELF summary]");
    assert_eq!(returned_ct, Some(ct));
}

#[test]
fn classifier_match_no_factory_falls_back_to_utf8() {
    let ct = ContentType::new("test/unknown");
    let (_, classifiers) = classifier_with_prefix(b"hello", ct);
    let factories = ContentCodecFactoryStore::new();

    let (text, returned_ct) =
        decode_file_bytes(b"hello world", "x", &classifiers, &factories).unwrap();
    assert_eq!(text, "hello world");
    assert!(returned_ct.is_none());
}

#[test]
fn classifier_match_decode_error_falls_back_to_utf8() {
    let ct = ContentType::new(TEST_TYPE);
    let (_, classifiers) = classifier_with_prefix(b"hello", ct.clone());
    let (_, factories) = factory_with_codec(ct, "ignored", 1);

    let (text, returned_ct) =
        decode_file_bytes(b"hello world", "x", &classifiers, &factories).unwrap();
    assert_eq!(text, "hello world");
    assert!(returned_ct.is_none());
}

#[test]
fn classifier_match_decode_error_invalid_utf8_returns_not_utf8() {
    let ct = ContentType::new(TEST_TYPE);
    let (_, classifiers) = classifier_with_prefix(b"\xFF", ct.clone());
    let (_, factories) = factory_with_codec(ct, "ignored", 1);

    let err = decode_file_bytes(b"\xFFnope", "x", &classifiers, &factories).unwrap_err();
    assert_eq!(err, FileOpenError::NotUtf8 { offset: 0 });
}

#[test]
fn filename_is_forwarded_to_classifier() {
    let ct = ContentType::new(TEST_TYPE);
    let (classifier, classifiers) = classifier_with_prefix(b"never-matches", ct);
    let factories = ContentCodecFactoryStore::new();

    let _ = decode_file_bytes(b"data", "/tmp/widget.elf", &classifiers, &factories).unwrap();
    let captured = classifier.last_path.lock().unwrap().clone();
    assert_eq!(captured.as_deref(), Some("/tmp/widget.elf"));
}

#[test]
fn display_too_large_formats_with_len_and_limit() {
    let err = FileOpenError::TooLarge {
        len: 100,
        limit: 64,
    };
    assert_eq!(err.to_string(), "file is 100 bytes, exceeds limit of 64");
}

#[test]
fn display_not_utf8_formats_with_offset() {
    let err = FileOpenError::NotUtf8 { offset: 7 };
    assert_eq!(err.to_string(), "file is not valid UTF-8 (invalid byte at offset 7)");
}

#[test]
fn file_open_error_implements_std_error() {
    fn assert_error<E: std::error::Error>(_: &E) {}
    let err = FileOpenError::NotUtf8 { offset: 0 };
    assert_error(&err);
}
