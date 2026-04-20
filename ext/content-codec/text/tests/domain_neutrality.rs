//! Domain-neutrality integration test (Plan 14 Phase C.3 telemetry note).
//!
//! Proves that `DecodedEdit` is generic over the domain-edit payload by
//! exercising the type-erased `DecodedEdit::Domain` variant with a
//! synthetic `MockEdit` type — distinct from the crate's native
//! `TextEdit` — and verifying that the concrete type round-trips
//! through `DomainEdit::new` / `DomainEdit::downcast_ref`.
//!
//! If someone accidentally re-couples the codec uapi to a specific
//! domain (e.g. by reintroducing a `DecodedEdit::Text` variant with
//! `reovim_domain_text::Position` hard-coded in uapi), this test will
//! stop compiling or fail at runtime — that is the intended trip wire.

use reovim_content_codec::{DecodedEdit, DomainEdit, impl_domain_edit};

/// Mock non-text domain edit. The codec uapi must not care about this
/// type's shape; it flows through opaquely.
#[derive(Debug, Clone, PartialEq, Eq)]
struct MockEdit {
    tag: &'static str,
    offset: u64,
    len: u64,
}

impl_domain_edit!(MockEdit);

#[test]
fn mock_edit_roundtrips_through_domain_variant() {
    let edit = DecodedEdit::Domain(DomainEdit::new(MockEdit {
        tag: "mock:insert",
        offset: 42,
        len: 7,
    }));

    match edit {
        DecodedEdit::Domain(domain_edit) => {
            let recovered = domain_edit
                .downcast_ref::<MockEdit>()
                .expect("MockEdit should downcast cleanly");
            assert_eq!(recovered.tag, "mock:insert");
            assert_eq!(recovered.offset, 42);
            assert_eq!(recovered.len, 7);
        }
        _ => panic!("expected DecodedEdit::Domain"),
    }
}

#[test]
fn mock_and_text_edits_are_distinct_concrete_types() {
    use reovim_content_codec_text::TextEdit;
    use reovim_domain_text::Position;

    let mock_edit = DecodedEdit::Domain(DomainEdit::new(MockEdit {
        tag: "mock",
        offset: 0,
        len: 0,
    }));
    let text_edit = DecodedEdit::Domain(DomainEdit::new(TextEdit {
        start: Position::new(0, 0),
        end: Position::new(0, 0),
        replacement: "x".to_string(),
    }));

    // Both are `DecodedEdit::Domain`, but the wrapped types differ.
    let DecodedEdit::Domain(mock_wrapper) = mock_edit else {
        panic!("expected Domain variant");
    };
    let DecodedEdit::Domain(text_wrapper) = text_edit else {
        panic!("expected Domain variant");
    };

    assert!(mock_wrapper.downcast_ref::<MockEdit>().is_some());
    assert!(mock_wrapper.downcast_ref::<TextEdit>().is_none());

    assert!(text_wrapper.downcast_ref::<TextEdit>().is_some());
    assert!(text_wrapper.downcast_ref::<MockEdit>().is_none());
}

#[test]
fn byte_shortcuts_are_domain_agnostic() {
    let edit = DecodedEdit::Domain(DomainEdit::new(MockEdit {
        tag: "mock",
        offset: 0,
        len: 0,
    }));
    assert!(
        !edit.is_byte_insertion(),
        "domain-shaped edits are not byte insertions at this layer"
    );
    assert!(
        !edit.is_byte_deletion(),
        "domain-shaped edits are not byte deletions at this layer"
    );
}
