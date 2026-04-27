//! Tests for the Codec trait — object-safety and basic contract.

use {
    super::codec::Codec,
    crate::InputPayloadError,
    std::{any::Any, sync::Arc},
};

struct MockCodec {
    kind: u16,
}

impl Codec for MockCodec {
    fn kind(&self) -> u16 {
        self.kind
    }

    fn encode(&self, value: &dyn Any) -> Result<Vec<u8>, InputPayloadError> {
        value
            .downcast_ref::<Vec<u8>>()
            .map_or(Err(InputPayloadError::TooShort { got: 0, min: 1 }), |bytes| Ok(bytes.clone()))
    }

    fn decode(&self, body: &[u8]) -> Result<Box<dyn Any + Send>, InputPayloadError> {
        if body.is_empty() {
            return Err(InputPayloadError::TooShort { got: 0, min: 1 });
        }
        Ok(Box::new(body.to_vec()))
    }
}

#[test]
fn codec_is_object_safe_via_arc() {
    // Verifies the trait is object-safe — Arc<dyn Codec> must compile.
    let codec: Arc<dyn Codec> = Arc::new(MockCodec { kind: 0x0001 });
    assert_eq!(codec.kind(), 0x0001);
}

#[test]
fn encode_and_decode_roundtrip() {
    let codec = MockCodec { kind: 0x0001 };
    let body: Vec<u8> = vec![0x01, 0x02, 0x03];
    let encoded = codec.encode(&body).unwrap();
    let decoded = codec.decode(&encoded).unwrap();
    let recovered = decoded.downcast::<Vec<u8>>().unwrap();
    assert_eq!(*recovered, body);
}

#[test]
fn decode_empty_body_returns_too_short() {
    let codec = MockCodec { kind: 0x0002 };
    let err = codec.decode(&[]).unwrap_err();
    assert_eq!(err, InputPayloadError::TooShort { got: 0, min: 1 });
}

#[test]
fn encode_wrong_type_returns_error() {
    let codec = MockCodec { kind: 0x0003 };
    let not_bytes: u32 = 42;
    let err = codec.encode(&not_bytes).unwrap_err();
    assert_eq!(err, InputPayloadError::TooShort { got: 0, min: 1 });
}

#[test]
fn multiple_codecs_held_as_trait_objects() {
    let codecs: Vec<Arc<dyn Codec>> = vec![
        Arc::new(MockCodec { kind: 0x0001 }),
        Arc::new(MockCodec { kind: 0x0002 }),
        Arc::new(MockCodec { kind: 0x0003 }),
    ];
    let kinds: Vec<u16> = codecs.iter().map(|c| c.kind()).collect();
    assert_eq!(kinds, vec![0x0001, 0x0002, 0x0003]);
}
