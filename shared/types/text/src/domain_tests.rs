use super::*;

#[test]
fn text_domain_types() {
    // Verify associated types resolve correctly.
    let pos: <Text as Domain>::Position = TextPosition::new(1, 2);
    assert_eq!(pos.line, 1);
    assert_eq!(pos.column, 2);

    let edit: <Text as Domain>::Edit = TextEdit::insert(TextPosition::origin(), "hello");
    assert!(edit.is_insert());
    assert_eq!(edit.text(), "hello");

    let content: <Text as Domain>::Content = String::from("text content");
    assert_eq!(content, "text content");
}
