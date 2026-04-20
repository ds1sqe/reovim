use super::*;

// Verify Domain can be implemented with simple types.
struct TestDomain;

#[derive(Debug, Clone, PartialEq)]
struct TestPos(usize);

#[derive(Debug, Clone)]
struct TestEdit {
    _pos: TestPos,
    data: Vec<u8>,
}

impl Domain for TestDomain {
    type Position = TestPos;
    type Edit = TestEdit;
    type Content = Vec<u8>;
}

#[test]
fn domain_associated_types() {
    // Verify the associated types resolve correctly.
    let pos: <TestDomain as Domain>::Position = TestPos(42);
    assert_eq!(pos.0, 42);

    let edit: <TestDomain as Domain>::Edit = TestEdit {
        _pos: TestPos(0),
        data: vec![1, 2, 3],
    };
    assert_eq!(edit.data.len(), 3);

    let content: <TestDomain as Domain>::Content = vec![0xFF];
    assert_eq!(content.len(), 1);
}

fn assert_send_sync<T: Send + Sync + 'static>() {}

#[test]
fn domain_bounds() {
    assert_send_sync::<TestPos>();
    assert_send_sync::<TestEdit>();
    assert_send_sync::<Vec<u8>>();
}
