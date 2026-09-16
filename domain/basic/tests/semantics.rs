use reovim_domain_basic::{decode_keys, encode_keys, replace_heading, toggle};
use reovim_kernel_interface::Error;
use std::collections::BTreeMap;

#[test]
fn heading_replacement_preserves_opaque_tail_and_rejects_invalid_edits() {
    let source = b"## old\n\xffuntouched\0";
    assert_eq!(
        replace_heading(source, b"new").unwrap(),
        b"## new\n\xffuntouched\0"
    );
    assert_eq!(source, b"## old\n\xffuntouched\0");
    assert_eq!(replace_heading(b"# old", b"new").unwrap(), b"# new");
    for (source, label) in [
        (b"plain".as_slice(), b"new".as_slice()),
        (b"###", b"new"),
        (b"#old", b"new"),
        (b"# old", b""),
        (b"# old", b"two words"),
        (b"# old", b"\xff"),
    ] {
        assert_eq!(replace_heading(source, label), Err(Error::INVALID));
    }
}

#[test]
fn key_edits_are_independent_and_preserve_semantic_identities() {
    let before = BTreeMap::from([(17, true), (u64::MAX, false)]);
    let after = toggle(&before, u64::MAX).unwrap();
    assert_eq!(after, BTreeMap::from([(17, true), (u64::MAX, true)]));
    assert_eq!(before.get(&u64::MAX), Some(&false));
    assert_eq!(toggle(&before, 9001), Err(Error::UNAVAILABLE));
    assert_eq!(decode_keys(&encode_keys(&after)).unwrap(), after);
    assert_eq!(decode_keys(&[]).unwrap(), BTreeMap::new());
}

#[test]
fn malformed_key_records_reject_without_accepting_a_partial_map() {
    let valid = encode_keys(&BTreeMap::from([(17, true)]));
    let mut duplicate = valid.clone();
    duplicate.extend_from_slice(&valid);
    assert_eq!(decode_keys(&duplicate), Err(Error::INVALID));
    let mut invalid = valid.clone();
    invalid[8] = 2;
    assert_eq!(decode_keys(&invalid), Err(Error::INVALID));
    assert_eq!(decode_keys(&valid[..8]), Err(Error::INVALID));
}
