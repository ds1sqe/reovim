use {
    super::Transaction,
    crate::{Edit, Position},
};

#[test]
fn test_new_transaction_is_empty() {
    let txn = Transaction::new();
    assert!(txn.is_empty());
    assert_eq!(txn.len(), 0);
}

#[test]
fn test_push_and_edits() {
    let mut txn = Transaction::new();
    txn.push(Edit::insert(Position::new(0, 0), "Hello"));
    txn.push(Edit::insert(Position::new(0, 5), " World"));

    assert!(!txn.is_empty());
    assert_eq!(txn.len(), 2);

    let edits = txn.edits();
    assert_eq!(edits.len(), 2);
    assert_eq!(edits[0].text(), "Hello");
    assert_eq!(edits[1].text(), " World");
}

#[test]
fn test_inverse() {
    let mut txn = Transaction::new();
    txn.push(Edit::insert(Position::new(0, 0), "A"));
    txn.push(Edit::insert(Position::new(0, 1), "B"));
    txn.push(Edit::insert(Position::new(0, 2), "C"));

    let inverse = txn.inverse();

    let edits = inverse.edits();
    assert_eq!(edits.len(), 3);

    assert!(edits[0].is_delete());
    assert!(edits[1].is_delete());
    assert!(edits[2].is_delete());

    assert_eq!(edits[0].text(), "C");
    assert_eq!(edits[1].text(), "B");
    assert_eq!(edits[2].text(), "A");
}

#[test]
fn test_from_vec() {
    let edits = vec![
        Edit::insert(Position::new(0, 0), "X"),
        Edit::insert(Position::new(0, 1), "Y"),
    ];

    let txn: Transaction = edits.into();
    assert_eq!(txn.len(), 2);
}

#[test]
fn test_from_single_edit() {
    let edit = Edit::insert(Position::new(0, 0), "Z");
    let txn: Transaction = edit.into();
    assert_eq!(txn.len(), 1);
}

#[test]
fn test_into_iter() {
    let mut txn = Transaction::new();
    txn.push(Edit::insert(Position::new(0, 0), "A"));
    txn.push(Edit::insert(Position::new(0, 1), "B"));

    assert_eq!(txn.into_iter().count(), 2);
}

#[test]
fn test_clear() {
    let mut txn = Transaction::new();
    txn.push(Edit::insert(Position::new(0, 0), "A"));
    assert!(!txn.is_empty());

    txn.clear();
    assert!(txn.is_empty());
}

#[test]
fn test_default_is_empty() {
    let txn = Transaction::default();
    assert!(txn.is_empty());
    assert_eq!(txn.len(), 0);
}

#[test]
fn test_with_capacity() {
    let txn = Transaction::with_capacity(10);
    assert!(txn.is_empty());
    assert_eq!(txn.len(), 0);
}

#[test]
fn test_with_capacity_push() {
    let mut txn = Transaction::with_capacity(2);
    txn.push(Edit::insert(Position::new(0, 0), "A"));
    txn.push(Edit::insert(Position::new(0, 1), "B"));
    assert_eq!(txn.len(), 2);
}

#[test]
fn test_into_edits() {
    let mut txn = Transaction::new();
    txn.push(Edit::insert(Position::new(0, 0), "Hello"));
    txn.push(Edit::insert(Position::new(0, 5), " World"));

    let edits = txn.into_edits();
    assert_eq!(edits.len(), 2);
    assert_eq!(edits[0].text(), "Hello");
    assert_eq!(edits[1].text(), " World");
}

#[test]
fn test_iter() {
    let mut txn = Transaction::new();
    txn.push(Edit::insert(Position::new(0, 0), "A"));
    txn.push(Edit::insert(Position::new(0, 1), "B"));
    txn.push(Edit::insert(Position::new(0, 2), "C"));

    let texts: Vec<&str> = txn.iter().map(Edit::text).collect();
    assert_eq!(texts, vec!["A", "B", "C"]);
}

#[test]
fn test_ref_into_iter() {
    let mut txn = Transaction::new();
    txn.push(Edit::insert(Position::new(0, 0), "X"));
    txn.push(Edit::insert(Position::new(0, 1), "Y"));

    let mut count = 0;
    for edit in &txn {
        assert!(edit.is_insert());
        count += 1;
    }
    assert_eq!(count, 2);
}

#[test]
fn test_inverse_of_empty() {
    let txn = Transaction::new();
    let inverse = txn.inverse();
    assert!(inverse.is_empty());
}

#[test]
fn test_inverse_of_deletes() {
    let mut txn = Transaction::new();
    txn.push(Edit::delete(Position::new(0, 0), "A"));
    txn.push(Edit::delete(Position::new(0, 0), "B"));

    let inverse = txn.inverse();
    let edits = inverse.edits();

    assert_eq!(edits.len(), 2);
    assert!(edits[0].is_insert());
    assert!(edits[1].is_insert());
    assert_eq!(edits[0].text(), "B");
    assert_eq!(edits[1].text(), "A");
}

#[test]
fn test_from_single_edit_content() {
    let edit = Edit::delete(Position::new(1, 2), "removed");
    let txn: Transaction = edit.into();
    assert_eq!(txn.len(), 1);
    assert!(txn.edits()[0].is_delete());
    assert_eq!(txn.edits()[0].text(), "removed");
}

#[test]
fn test_clear_then_push() {
    let mut txn = Transaction::new();
    txn.push(Edit::insert(Position::new(0, 0), "A"));
    txn.clear();
    assert!(txn.is_empty());

    txn.push(Edit::insert(Position::new(0, 0), "B"));
    assert_eq!(txn.len(), 1);
    assert_eq!(txn.edits()[0].text(), "B");
}

#[test]
fn test_into_iter_consumes() {
    let txn: Transaction = vec![
        Edit::insert(Position::new(0, 0), "A"),
        Edit::insert(Position::new(0, 1), "B"),
        Edit::insert(Position::new(0, 2), "C"),
    ]
    .into();

    let collected: Vec<Edit> = txn.into_iter().collect();
    assert_eq!(collected.len(), 3);
    assert_eq!(collected[0].text(), "A");
    assert_eq!(collected[1].text(), "B");
    assert_eq!(collected[2].text(), "C");
}
