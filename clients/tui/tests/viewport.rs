use reovim_client_tui::context::Window;
use reovim_client_tui::{
    editing,
    input::{Action, Code, Input, Key, Modifiers},
    viewport,
};
use reovim_kernel_interface::{ResourceId, buffer::BufferState};
use reovim_module_text::api::{Observed, Unit, Window as SourceWindow};

fn window(text: &str) -> Window {
    let units: Vec<_> = text.chars().map(Unit::Scalar).collect();
    unit_window(units)
}

fn unit_window(units: Vec<Unit>) -> Window {
    Window::complete(SourceWindow {
        observed: Observed {
            representation: ResourceId(2),
            space: ResourceId(2),
            buffer: BufferState { buffer: ResourceId(1), revision: 1 },
        },
        snapshot: ResourceId(3),
        range: 0..units.len() as u64,
        units,
        eof: true,
    })
    .unwrap()
}

#[test]
fn wide_and_combining_text_maps_cells_without_splitting_graphemes() {
    let value = window("#漢a\u{301}");
    let frame = viewport::project(&value, 4, 5, 2, 0).unwrap();
    assert_eq!(frame.lines, ["#漢a\u{301}"]);
    assert_eq!(frame.cursor, Some((4, 0)));
    assert_eq!(viewport::project(&value, 3, 5, 2, 0).unwrap().cursor, None);
    assert_eq!(viewport::cursor_boundary(&value.units, 3), 4);
    let emoji = window("👩\u{200d}💻x");
    let frame = viewport::project(&emoji, 3, 4, 2, 0).unwrap();
    assert_eq!(frame.lines, ["👩\u{200d}💻x"]);
    assert_eq!(frame.cursor, Some((2, 0)));
    assert!(matches!(
        viewport::project(&window("漢"), 0, 2, 2, 0),
        Err(viewport::Error::TooNarrow)
    ));
}

#[test]
fn crlf_tabs_and_scrolling_use_source_units_and_bounded_rows() {
    let value = window("a\r\nb\tc");
    let frame = viewport::project(&value, 3, 6, 3, 0).unwrap();
    assert_eq!(frame.lines, ["a", "b   c"]);
    assert_eq!(frame.cursor, Some((0, 1)));
    assert_eq!(viewport::cursor_boundary(&value.units, 2), 3);
    let value = window("one\ntwo\nthree\nfour");
    let frame = viewport::project(&value, 18, 7, 2, 0).unwrap();
    assert_eq!(frame.lines, ["three", "four"]);
    assert_eq!(frame.cursor, Some((4, 1)));
    assert_eq!(frame.top, 2);
    let resized = viewport::project(&value, 18, 7, 1, frame.top).unwrap();
    assert_eq!(resized.lines, ["four"]);
    assert_eq!(resized.cursor, Some((4, 0)));
}

#[test]
fn controls_are_visible_data_and_partial_context_stays_explicit() {
    let value = unit_window(vec![
        Unit::Scalar('a'),
        Unit::Opaque(255),
        Unit::Scalar('\x1b'),
    ]);
    let frame = viewport::project(&value, 1, 8, 3, 0).unwrap();
    assert_eq!(frame.lines, ["a⟦ff⟧", "⟦1b⟧"]);
    assert_eq!(frame.cursor, Some((1, 0)));
    assert_eq!(viewport::status("漢ab", 3), "漢a");
    assert!(!viewport::status("\x1b[31m", 20).contains('\x1b'));
}

#[test]
fn editing_moves_and_deletes_the_same_graphemes_that_are_projected() {
    let value = window("a\r\n👩\u{200d}💻");
    let mut cursor = 6;
    let backspace = Input::Key(Key {
        code: Code::Backspace,
        modifiers: Modifiers::empty(),
    });
    assert_eq!(
        editing::action(backspace.clone(), &value, &mut cursor),
        Some(Action::Insert(3, 6, String::new(), 0))
    );
    cursor = 3;
    assert_eq!(
        editing::action(backspace, &value, &mut cursor),
        Some(Action::Insert(1, 3, String::new(), 0))
    );
}
