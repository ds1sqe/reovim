//! Output validation shares the wire value rules.

use std::collections::BTreeMap;

use reovim_kernel_interface::{
    Decl, Error, Field, Hash, Method, Rights, Ty, check_reply,
};

#[test]
fn a_reply_is_exactly_one_record_of_the_declared_type() {
    let output = Ty::Record(
        "sample.Answer".into(),
        vec![
            Field { name: "value".into(), ty: Ty::U64 },
            Field { name: "valid".into(), ty: Ty::Bool },
        ],
    );
    let decl = Decl {
        api: "sample".into(),
        ver: 1,
        methods: vec![Method {
            number: 0,
            name: "get".into(),
            input_name: None,
            input: Ty::Unit,
            output,
            right: Rights::READ,
        }],
        records: vec![],
    };
    let api = decl.hash();
    let decls = BTreeMap::from([(api, decl)]);
    let mut bytes = 42_u64.to_le_bytes().to_vec();
    bytes.push(1);
    assert_eq!(check_reply(&decls, api, 0, &bytes), Ok(()));
    assert_eq!(check_reply(&decls, api, 0, &bytes[..8]), Err(Error::Invalid));
    bytes[8] = 2;
    assert_eq!(check_reply(&decls, api, 0, &bytes), Err(Error::Invalid));
    bytes[8] = 1;
    bytes.push(0);
    assert_eq!(check_reply(&decls, api, 0, &bytes), Err(Error::Invalid));
    assert_eq!(check_reply(&decls, api, 1, &[]), Err(Error::Invalid));
    assert_eq!(check_reply(&decls, Hash(0), 0, &[]), Err(Error::Invalid));
}
