use super::*;

fn byte(runtime: &mut Runtime, caller: &Caller, token: AccessId) -> u8 {
    let value =
        query(runtime, caller, Operation::Read, token, &1_u64.to_le_bytes())
            .unwrap();
    value.data[4]
}

#[test]
fn cold_identity_and_path_open_distinct_cursors_after_one_materialization() {
    let (mut runtime, library, caller, directory) =
        installed_source("open_namespace.c");
    let parent = open(&mut runtime, &caller, directory);
    query(
        &mut runtime,
        &caller,
        Operation::SetAccess,
        parent,
        &1_u32.to_le_bytes(),
    )
    .unwrap();
    let rows = ids(&entries(&mut runtime, &caller, parent, 128).unwrap());
    assert_eq!(rows[0].0, b"item");
    let identity = rows[0].1;
    query(&mut runtime, &caller, Operation::Close, parent, &[]).unwrap();
    let first = acquisition(&caller, identity);
    caller.submit(first.clone()).unwrap();
    let path = b"/mount/directory/item";
    let mut data = 0_u32.to_le_bytes().to_vec();
    data.extend_from_slice(&u32::try_from(path.len()).unwrap().to_le_bytes());
    data.extend_from_slice(path);
    let second = Call {
        request: caller.reserve().unwrap(),
        operation: Operation::Acquire,
        subject: AccessId(0),
        command: 0,
        expected: None,
        input: Value::copy(&data, &[]).unwrap(),
    };
    caller.submit(second.clone()).unwrap();
    let a = completed(&mut runtime, &caller, first.request).unwrap();
    let b = completed(&mut runtime, &caller, second.request).unwrap();
    assert_eq!(&a.data[..24], &b.data[..24]);
    let a = a.references[0];
    let b = b.references[0];
    query(&mut runtime, &caller, Operation::SetAccess, a, &1_u32.to_le_bytes())
        .unwrap();
    query(&mut runtime, &caller, Operation::SetAccess, b, &1_u32.to_le_bytes())
        .unwrap();
    assert_eq!(byte(&mut runtime, &caller, a), b'a');
    assert_eq!(byte(&mut runtime, &caller, a), b'b');
    assert_eq!(byte(&mut runtime, &caller, b), b'a');
    assert_eq!(library.count(c"fixture_materializations"), 1);
    assert_eq!(library.count(c"fixture_opens"), 2);
    query(&mut runtime, &caller, Operation::Close, a, &[]).unwrap();
    assert_eq!(byte(&mut runtime, &caller, b), b'b');
    query(&mut runtime, &caller, Operation::Close, b, &[]).unwrap();
    assert_eq!(library.count(c"fixture_cursors_destroyed"), 2);
    runtime.close().unwrap();
}
