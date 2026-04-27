//! Tests for `ContentCodecRegistry` — register/unregister/get contract.

use {
    super::registry::{ContentCodecRegistry, DefaultContentCodecRegistry},
    reovim_content_codec::{
        CodecError, CodecMetadata, ContentCodec, ContentType, DecodeResult, TranslateEditError,
    },
    reovim_kernel::api::v1::ByteEdit,
    reovim_subsys_vfs::ByteSource,
    std::sync::Arc,
};

struct StubCodec {
    tag: &'static str,
}

impl ContentCodec for StubCodec {
    fn decode(&self, _raw: &[u8]) -> Result<DecodeResult, CodecError> {
        Ok(DecodeResult {
            content: self.tag.to_string(),
            annotations: vec![],
            metadata: CodecMetadata::new(ContentType::new(self.tag)),
            lossy: false,
            readonly: false,
            truncated: false,
        })
    }

    fn translate_edit(
        &self,
        _bytes: &dyn ByteSource,
        _edit: &reovim_content_codec::DecodedEdit,
    ) -> Result<Option<ByteEdit>, TranslateEditError> {
        Err(TranslateEditError::ReadOnly)
    }
}

fn stub(tag: &'static str) -> Arc<dyn ContentCodec> {
    Arc::new(StubCodec { tag })
}

#[test]
fn get_on_empty_registry_returns_none() {
    let reg = DefaultContentCodecRegistry::new();
    assert!(reg.get(&ContentType::new("text/missing")).is_none());
}

#[test]
fn register_then_get_returns_codec() {
    let reg = DefaultContentCodecRegistry::new();
    let ct = ContentType::new("text/utf-8");
    reg.register(ct.clone(), stub("utf-8"));
    let codec = reg.get(&ct).expect("codec should be registered");
    let decoded = codec.decode(b"ignored").unwrap();
    assert_eq!(decoded.content, "utf-8");
}

#[test]
fn unregister_removes_codec() {
    let reg = DefaultContentCodecRegistry::new();
    let ct = ContentType::new("text/hex");
    reg.register(ct.clone(), stub("hex"));
    assert!(reg.get(&ct).is_some());
    reg.unregister(&ct);
    assert!(reg.get(&ct).is_none());
}

#[test]
fn unregister_nonexistent_is_noop() {
    let reg = DefaultContentCodecRegistry::new();
    // Should not panic.
    reg.unregister(&ContentType::new("text/absent"));
}

#[test]
fn register_replaces_existing_codec() {
    let reg = DefaultContentCodecRegistry::new();
    let ct = ContentType::new("text/dup");
    reg.register(ct.clone(), stub("first"));
    reg.register(ct.clone(), stub("second"));
    let codec = reg.get(&ct).expect("codec should be registered");
    assert_eq!(codec.decode(b"").unwrap().content, "second");
}

#[test]
fn multiple_codecs_coexist() {
    let reg = DefaultContentCodecRegistry::new();
    let ct_a = ContentType::new("application/a");
    let ct_b = ContentType::new("application/b");
    let ct_c = ContentType::new("application/c");
    reg.register(ct_a.clone(), stub("a"));
    reg.register(ct_b.clone(), stub("b"));
    reg.register(ct_c.clone(), stub("c"));

    assert_eq!(reg.get(&ct_a).unwrap().decode(b"").unwrap().content, "a");
    assert_eq!(reg.get(&ct_b).unwrap().decode(b"").unwrap().content, "b");
    assert_eq!(reg.get(&ct_c).unwrap().decode(b"").unwrap().content, "c");
    assert!(reg.get(&ContentType::new("application/d")).is_none());
}

#[test]
fn default_registry_is_empty() {
    let reg = DefaultContentCodecRegistry::default();
    assert!(reg.get(&ContentType::new("anything")).is_none());
}

#[test]
fn default_registry_debug_lists_registered_types() {
    let reg = DefaultContentCodecRegistry::new();
    let ct = ContentType::new("text/debug");
    reg.register(ct, stub("dbg"));
    let dbg = format!("{reg:?}");
    assert!(dbg.contains("DefaultContentCodecRegistry"));
    assert!(dbg.contains("text/debug"));
}

#[test]
fn concurrent_register_and_get() {
    use std::thread;

    let reg = Arc::new(DefaultContentCodecRegistry::new());
    let mut handles = Vec::new();

    // Spawn 8 threads that each register and read a codec.
    for i in 0u8..8 {
        let reg_clone = Arc::clone(&reg);
        handles.push(thread::spawn(move || {
            let ct = ContentType::new(format!("type/{i}"));
            reg_clone.register(ct.clone(), stub("concurrent"));
            let _ = reg_clone.get(&ct);
        }));
    }

    for h in handles {
        h.join().expect("thread panicked");
    }

    // All 8 codecs should be present.
    for i in 0u8..8 {
        let ct = ContentType::new(format!("type/{i}"));
        assert!(reg.get(&ct).is_some(), "codec {i} missing after concurrent write");
    }
}

#[test]
fn arc_dyn_trait_object_works() {
    let reg: Arc<dyn ContentCodecRegistry> = Arc::new(DefaultContentCodecRegistry::new());
    let ct = ContentType::new("text/traitobj");
    reg.register(ct.clone(), stub("traitobj"));
    assert!(reg.get(&ct).is_some());
    reg.unregister(&ct);
    assert!(reg.get(&ct).is_none());
}
