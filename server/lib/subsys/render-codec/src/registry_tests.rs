//! Tests for `RenderCodecRegistry` — register/unregister/get contract.

use {
    super::registry::{DefaultRenderCodecRegistry, RenderCodecRegistry},
    reovim_render_codec::{Codec, RenderPayloadError},
    std::{any::Any, sync::Arc},
};

struct StubCodec(u16);

impl Codec for StubCodec {
    fn kind(&self) -> u16 {
        self.0
    }

    fn encode(&self, _value: &dyn Any) -> Result<Vec<u8>, RenderPayloadError> {
        Ok(vec![])
    }

    fn decode(&self, _body: &[u8]) -> Result<Box<dyn Any + Send>, RenderPayloadError> {
        Ok(Box::new(()))
    }
}

#[test]
fn get_on_empty_registry_returns_none() {
    let reg = DefaultRenderCodecRegistry::new();
    assert!(reg.get(0x0001).is_none());
}

#[test]
fn register_then_get_returns_codec() {
    let reg = DefaultRenderCodecRegistry::new();
    reg.register(Arc::new(StubCodec(0x0001)));
    let codec = reg.get(0x0001).expect("codec should be registered");
    assert_eq!(codec.kind(), 0x0001);
}

#[test]
fn unregister_removes_codec() {
    let reg = DefaultRenderCodecRegistry::new();
    reg.register(Arc::new(StubCodec(0x0002)));
    assert!(reg.get(0x0002).is_some());
    reg.unregister(0x0002);
    assert!(reg.get(0x0002).is_none());
}

#[test]
fn unregister_nonexistent_is_noop() {
    let reg = DefaultRenderCodecRegistry::new();
    reg.unregister(0xFFFF); // should not panic
}

#[test]
fn register_replaces_existing_codec() {
    let reg = DefaultRenderCodecRegistry::new();
    reg.register(Arc::new(StubCodec(0x0003)));
    reg.register(Arc::new(StubCodec(0x0003)));
    assert!(reg.get(0x0003).is_some());
}

#[test]
fn multiple_codecs_coexist() {
    let reg = DefaultRenderCodecRegistry::new();
    reg.register(Arc::new(StubCodec(0x0001)));
    reg.register(Arc::new(StubCodec(0x0002)));
    reg.register(Arc::new(StubCodec(0x0003)));

    assert_eq!(reg.get(0x0001).unwrap().kind(), 0x0001);
    assert_eq!(reg.get(0x0002).unwrap().kind(), 0x0002);
    assert_eq!(reg.get(0x0003).unwrap().kind(), 0x0003);
    assert!(reg.get(0x0004).is_none());
}

#[test]
fn default_registry_is_empty() {
    let reg = DefaultRenderCodecRegistry::default();
    assert!(reg.get(0x0001).is_none());
}

#[test]
fn default_registry_debug_lists_registered_kinds() {
    let reg = DefaultRenderCodecRegistry::new();
    reg.register(Arc::new(StubCodec(0x0042)));
    let dbg = format!("{reg:?}");
    assert!(dbg.contains("DefaultRenderCodecRegistry"));
}

#[test]
fn concurrent_register_and_get() {
    use std::thread;

    let reg = Arc::new(DefaultRenderCodecRegistry::new());
    let mut handles = Vec::new();

    for i in 0u16..8 {
        let reg_clone = Arc::clone(&reg);
        handles.push(thread::spawn(move || {
            let kind = i + 1;
            reg_clone.register(Arc::new(StubCodec(kind)));
            let _ = reg_clone.get(kind);
        }));
    }

    for h in handles {
        h.join().expect("thread panicked");
    }

    for i in 0u16..8 {
        assert!(reg.get(i + 1).is_some(), "codec {i} missing after concurrent write");
    }
}

#[test]
fn arc_dyn_trait_object_works() {
    let reg: Arc<dyn RenderCodecRegistry> = Arc::new(DefaultRenderCodecRegistry::new());
    reg.register(Arc::new(StubCodec(0x00FF)));
    assert!(reg.get(0x00FF).is_some());
    reg.unregister(0x00FF);
    assert!(reg.get(0x00FF).is_none());
}
