use {
    super::*,
    crate::{CursorCodec, CursorHeader, PositionCodec, PositionHeader},
    std::{collections::HashMap, sync::Mutex},
};

// --- Test implementation of CoordinationRegistry ---
//
// This test mock uses Mutex for registration (interior mutability) and
// leaked Box pointers for codec storage so that `&dyn PositionCodec`
// references can be returned without lifetime issues.
//
// The real implementation should use a proper two-phase pattern
// (mutable builder -> frozen immutable map) rather than leaking.

struct DomainEntry {
    name: &'static str,
    #[allow(dead_code)]
    description: &'static str,
}

struct TestRegistry {
    inner: Mutex<TestRegistryInner>,
}

struct TestRegistryInner {
    domains: HashMap<u32, DomainEntry>,
    domain_names: HashMap<String, u32>,
    next_id: u32,
    position_codecs: HashMap<(u32, u16), &'static dyn PositionCodec>,
    cursor_codecs: HashMap<(u32, u16), &'static dyn CursorCodec>,
}

impl TestRegistry {
    fn new() -> Self {
        Self {
            inner: Mutex::new(TestRegistryInner {
                domains: HashMap::new(),
                domain_names: HashMap::new(),
                next_id: 1, // 0 is sentinel
                position_codecs: HashMap::new(),
                cursor_codecs: HashMap::new(),
            }),
        }
    }
}

impl CoordinationRegistry for TestRegistry {
    fn enlist_domain(
        &self,
        name: &'static str,
        description: &'static str,
    ) -> Result<u32, EnlistError> {
        let mut inner = self.inner.lock().unwrap();
        if inner.domain_names.contains_key(name) {
            return Err(EnlistError::DomainNameTaken(name.to_string()));
        }
        let id = inner.next_id;
        inner.next_id += 1;
        inner.domain_names.insert(name.to_string(), id);
        inner.domains.insert(id, DomainEntry { name, description });
        drop(inner);
        Ok(id)
    }

    fn register_position_codec(
        &self,
        domain_id: u32,
        inner_id: u16,
        _name: &'static str,
        codec: Box<dyn PositionCodec>,
    ) -> Result<(), EnlistError> {
        let mut inner = self.inner.lock().unwrap();
        if !inner.domains.contains_key(&domain_id) {
            return Err(EnlistError::DomainNotFound(domain_id));
        }
        let key = (domain_id, inner_id);
        if inner.position_codecs.contains_key(&key) {
            return Err(EnlistError::CodecAlreadyRegistered {
                domain_id,
                inner_id,
            });
        }
        // Leak the box so we can return &'static references from lookups.
        // This is acceptable in tests; real impl uses frozen immutable storage.
        let leaked: &'static dyn PositionCodec = Box::leak(codec);
        inner.position_codecs.insert(key, leaked);
        drop(inner);
        Ok(())
    }

    fn register_cursor_codec(
        &self,
        domain_id: u32,
        inner_id: u16,
        _name: &'static str,
        codec: Box<dyn CursorCodec>,
    ) -> Result<(), EnlistError> {
        let mut inner = self.inner.lock().unwrap();
        if !inner.domains.contains_key(&domain_id) {
            return Err(EnlistError::DomainNotFound(domain_id));
        }
        let key = (domain_id, inner_id);
        if inner.cursor_codecs.contains_key(&key) {
            return Err(EnlistError::CodecAlreadyRegistered {
                domain_id,
                inner_id,
            });
        }
        let leaked: &'static dyn CursorCodec = Box::leak(codec);
        inner.cursor_codecs.insert(key, leaked);
        drop(inner);
        Ok(())
    }

    fn domain_name(&self, domain_id: u32) -> Option<&str> {
        let inner = self.inner.lock().unwrap();
        inner.domains.get(&domain_id).map(|e| e.name)
    }

    fn position_codec(&self, domain_id: u32, inner_id: u16) -> Option<&dyn PositionCodec> {
        let inner = self.inner.lock().unwrap();
        inner.position_codecs.get(&(domain_id, inner_id)).copied()
    }

    fn cursor_codec(&self, domain_id: u32, inner_id: u16) -> Option<&dyn CursorCodec> {
        let inner = self.inner.lock().unwrap();
        inner.cursor_codecs.get(&(domain_id, inner_id)).copied()
    }
}

// --- Simple test codecs ---

struct DummyPositionCodec {
    domain: u32,
    inner: u16,
}

impl PositionCodec for DummyPositionCodec {
    fn domain_id(&self) -> u32 {
        self.domain
    }
    fn inner_id(&self) -> u16 {
        self.inner
    }
    fn decode(
        &self,
        _header: &PositionHeader,
        _content: &[u8],
    ) -> Option<Box<dyn crate::Position>> {
        None // dummy
    }
}

struct DummyCursorCodec {
    domain: u32,
    inner: u16,
}

impl CursorCodec for DummyCursorCodec {
    fn domain_id(&self) -> u32 {
        self.domain
    }
    fn inner_id(&self) -> u16 {
        self.inner
    }
    fn decode(&self, _header: &CursorHeader, _content: &[u8]) -> Option<Box<dyn crate::Cursor>> {
        None // dummy
    }
}

// --- Registry tests ---

#[test]
fn registry_object_safety() {
    let _: Box<dyn CoordinationRegistry> = Box::new(TestRegistry::new());
}

#[test]
fn enlist_domain_assigns_nonzero_id() {
    let reg = TestRegistry::new();
    let id = reg.enlist_domain("text", "Text editing domain").unwrap();
    assert_ne!(id, 0, "domain_id=0 is sentinel");
}

#[test]
fn enlist_domain_increments() {
    let reg = TestRegistry::new();
    let id1 = reg.enlist_domain("text", "Text").unwrap();
    let id2 = reg.enlist_domain("mesh", "Mesh").unwrap();
    assert_ne!(id1, id2);
    assert!(id2 > id1);
}

#[test]
fn enlist_domain_name_uniqueness() {
    let reg = TestRegistry::new();
    reg.enlist_domain("text", "Text").unwrap();
    let err = reg.enlist_domain("text", "Also text").unwrap_err();
    assert!(matches!(err, EnlistError::DomainNameTaken(ref name) if name == "text"));
}

#[test]
fn domain_name_lookup() {
    let reg = TestRegistry::new();
    let id = reg.enlist_domain("text", "Text editing").unwrap();
    assert_eq!(reg.domain_name(id), Some("text"));
}

#[test]
fn domain_name_not_found() {
    let reg = TestRegistry::new();
    assert_eq!(reg.domain_name(999), None);
}

#[test]
fn register_position_codec_success() {
    let reg = TestRegistry::new();
    let id = reg.enlist_domain("text", "Text").unwrap();
    let codec = Box::new(DummyPositionCodec {
        domain: id,
        inner: 0,
    });
    reg.register_position_codec(id, 0, "TextPosition", codec)
        .unwrap();
}

#[test]
fn register_position_codec_domain_not_found() {
    let reg = TestRegistry::new();
    let codec = Box::new(DummyPositionCodec {
        domain: 99,
        inner: 0,
    });
    let err = reg
        .register_position_codec(99, 0, "Ghost", codec)
        .unwrap_err();
    assert!(matches!(err, EnlistError::DomainNotFound(99)));
}

#[test]
fn register_position_codec_duplicate() {
    let reg = TestRegistry::new();
    let id = reg.enlist_domain("text", "Text").unwrap();
    let codec1 = Box::new(DummyPositionCodec {
        domain: id,
        inner: 0,
    });
    let codec2 = Box::new(DummyPositionCodec {
        domain: id,
        inner: 0,
    });
    reg.register_position_codec(id, 0, "TextPosition", codec1)
        .unwrap();
    let err = reg
        .register_position_codec(id, 0, "TextPosition2", codec2)
        .unwrap_err();
    assert!(matches!(
        err,
        EnlistError::CodecAlreadyRegistered {
            domain_id,
            inner_id: 0
        } if domain_id == id
    ));
}

#[test]
fn register_cursor_codec_success() {
    let reg = TestRegistry::new();
    let id = reg.enlist_domain("text", "Text").unwrap();
    let codec = Box::new(DummyCursorCodec {
        domain: id,
        inner: 0,
    });
    reg.register_cursor_codec(id, 0, "TextCursor", codec)
        .unwrap();
}

#[test]
fn register_cursor_codec_domain_not_found() {
    let reg = TestRegistry::new();
    let codec = Box::new(DummyCursorCodec {
        domain: 99,
        inner: 0,
    });
    let err = reg
        .register_cursor_codec(99, 0, "Ghost", codec)
        .unwrap_err();
    assert!(matches!(err, EnlistError::DomainNotFound(99)));
}

#[test]
fn register_cursor_codec_duplicate() {
    let reg = TestRegistry::new();
    let id = reg.enlist_domain("text", "Text").unwrap();
    let codec1 = Box::new(DummyCursorCodec {
        domain: id,
        inner: 0,
    });
    let codec2 = Box::new(DummyCursorCodec {
        domain: id,
        inner: 0,
    });
    reg.register_cursor_codec(id, 0, "TextCursor", codec1)
        .unwrap();
    let err = reg
        .register_cursor_codec(id, 0, "TextCursor2", codec2)
        .unwrap_err();
    assert!(matches!(
        err,
        EnlistError::CodecAlreadyRegistered {
            domain_id,
            inner_id: 0
        } if domain_id == id
    ));
}

#[test]
fn register_multiple_inner_ids() {
    let reg = TestRegistry::new();
    let id = reg.enlist_domain("text", "Text").unwrap();
    let c0 = Box::new(DummyCursorCodec {
        domain: id,
        inner: 0,
    });
    let c1 = Box::new(DummyCursorCodec {
        domain: id,
        inner: 1,
    });
    reg.register_cursor_codec(id, 0, "TextCursor", c0).unwrap();
    reg.register_cursor_codec(id, 1, "TextBlockCursor", c1)
        .unwrap();
}

#[test]
fn position_codec_lookup() {
    let reg = TestRegistry::new();
    let id = reg.enlist_domain("text", "Text").unwrap();
    let codec = Box::new(DummyPositionCodec {
        domain: id,
        inner: 0,
    });
    reg.register_position_codec(id, 0, "TextPosition", codec)
        .unwrap();
    let found = reg.position_codec(id, 0);
    assert!(found.is_some());
    assert_eq!(found.unwrap().domain_id(), id);
    assert_eq!(found.unwrap().inner_id(), 0);
}

#[test]
fn position_codec_lookup_not_found() {
    let reg = TestRegistry::new();
    assert!(reg.position_codec(99, 0).is_none());
}

#[test]
fn cursor_codec_lookup() {
    let reg = TestRegistry::new();
    let id = reg.enlist_domain("text", "Text").unwrap();
    let codec = Box::new(DummyCursorCodec {
        domain: id,
        inner: 1,
    });
    reg.register_cursor_codec(id, 1, "TextBlockCursor", codec)
        .unwrap();
    let found = reg.cursor_codec(id, 1);
    assert!(found.is_some());
    assert_eq!(found.unwrap().domain_id(), id);
    assert_eq!(found.unwrap().inner_id(), 1);
}

#[test]
fn cursor_codec_lookup_not_found() {
    let reg = TestRegistry::new();
    assert!(reg.cursor_codec(99, 0).is_none());
}

#[test]
fn enlist_error_display_domain_name_taken() {
    let err = EnlistError::DomainNameTaken("text".to_string());
    assert_eq!(err.to_string(), "domain name already taken: \"text\"");
}

#[test]
fn enlist_error_display_domain_not_found() {
    let err = EnlistError::DomainNotFound(42);
    assert_eq!(err.to_string(), "domain not found: domain_id=42");
}

#[test]
fn enlist_error_display_codec_already_registered() {
    let err = EnlistError::CodecAlreadyRegistered {
        domain_id: 1,
        inner_id: 0,
    };
    assert_eq!(err.to_string(), "codec already registered: domain_id=1, inner_id=0");
}

#[test]
fn enlist_error_is_std_error() {
    let err: Box<dyn std::error::Error> =
        Box::new(EnlistError::DomainNameTaken("test".to_string()));
    assert!(!err.to_string().is_empty());
}
