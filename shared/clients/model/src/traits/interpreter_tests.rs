use super::*;

#[test]
fn test_interpreter_single() {
    let interpreter = DefaultLayoutInterpreter::new();
    let logical = LogicalLayout::single(1, 100);
    let screen = Size::new(80, 24);

    let tree = interpreter.interpret(&logical, screen);
    assert!(tree.is_leaf());
    assert_eq!(tree.bounds(), Rect::new(0, 0, 80, 24));
}

#[test]
fn test_interpreter_vsplit() {
    let interpreter = DefaultLayoutInterpreter::new();
    let logical = LogicalLayout::vsplit(vec![
        LogicalLayout::single(1, 100),
        LogicalLayout::single(2, 101),
    ]);
    let screen = Size::new(80, 24);

    let tree = interpreter.interpret(&logical, screen);
    assert_eq!(tree.window_count(), 2);

    let windows = tree.all_windows();
    assert_eq!(windows[0].bounds.width, 40);
    assert_eq!(windows[1].bounds.width, 40);
    assert_eq!(windows[0].bounds.x, 0);
    assert_eq!(windows[1].bounds.x, 40);
}

#[test]
fn test_interpreter_hsplit() {
    let interpreter = DefaultLayoutInterpreter::new();
    let logical = LogicalLayout::hsplit(vec![
        LogicalLayout::single(1, 100),
        LogicalLayout::single(2, 101),
    ]);
    let screen = Size::new(80, 24);

    let tree = interpreter.interpret(&logical, screen);
    assert_eq!(tree.window_count(), 2);

    let windows = tree.all_windows();
    assert_eq!(windows[0].bounds.height, 12);
    assert_eq!(windows[1].bounds.height, 12);
    assert_eq!(windows[0].bounds.y, 0);
    assert_eq!(windows[1].bounds.y, 12);
}

#[test]
fn test_interpreter_nested() {
    let interpreter = DefaultLayoutInterpreter::new();
    let logical = LogicalLayout::vsplit(vec![
        LogicalLayout::hsplit(vec![
            LogicalLayout::single(1, 100),
            LogicalLayout::single(2, 101),
        ]),
        LogicalLayout::single(3, 102),
    ]);
    let screen = Size::new(80, 24);

    let tree = interpreter.interpret(&logical, screen);
    assert_eq!(tree.window_count(), 3);
}

#[test]
fn test_interpreter_tabs() {
    let interpreter = DefaultLayoutInterpreter::new();
    let logical = LogicalLayout::tabs(
        vec![LogicalLayout::single(1, 100), LogicalLayout::single(2, 101)],
        0,
    );
    let screen = Size::new(80, 24);

    let tree = interpreter.interpret(&logical, screen);
    assert_eq!(tree.window_count(), 2);
    // All tabs share the same bounds
    let windows = tree.all_windows();
    assert_eq!(windows[0].bounds, windows[1].bounds);
}
