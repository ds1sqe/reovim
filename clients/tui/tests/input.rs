use reovim_client_tui::context::Window;
use reovim_client_tui::{
    editing,
    input::{Action, Code, Input, Key, Modifiers},
};
use reovim_kernel_interface::{ResourceId, buffer::BufferState};
use reovim_module_text::api::{Observed, Unit, Window as SourceWindow};

fn window() -> Window {
    Window::complete(SourceWindow {
        observed: Observed {
            representation: ResourceId(2),
            space: ResourceId(2),
            buffer: BufferState { buffer: ResourceId(1), revision: 4 },
        },
        snapshot: ResourceId(3),
        range: 0..2,
        eof: true,
        units: vec![Unit::Scalar('é'), Unit::Opaque(255)],
    })
    .unwrap()
}

const fn key(code: Code, modifiers: Modifiers) -> Input {
    Input::Key(Key { code, modifiers })
}

#[test]
fn logical_keys_preserve_modifiers_and_selected_source_coordinates() {
    let value = window();
    let mut cursor = 1;
    assert_eq!(
        editing::action(
            key(Code::Char('x'), Modifiers::META),
            &value,
            &mut cursor
        ),
        None
    );
    assert_eq!(
        editing::action(
            key(Code::Char('z'), Modifiers::CONTROL),
            &value,
            &mut cursor
        ),
        Some(Action::Undo)
    );
    assert_eq!(
        editing::action(
            key(Code::Delete, Modifiers::empty()),
            &value,
            &mut cursor
        ),
        Some(Action::Insert(1, 2, String::new(), 0))
    );
    assert_eq!(cursor, 1);
    let selected =
        editing::action(Input::Text("漢".into()), &value, &mut cursor).unwrap();
    assert_eq!(selected, Action::Insert(1, 1, "漢".into(), 0));
    assert_eq!(selected.cursor_after(), Some(2));
}

#[test]
fn invalid_intents_and_unobserved_positions_do_not_produce_edits() {
    assert_eq!(
        Action::Insert(101, 102, "漢".into(), 100).cursor_after(),
        Some(102)
    );
    assert_eq!(Action::Insert(4, 3, "x".into(), 0).cursor_after(), None);
    assert_eq!(Action::Insert(2, 3, "x".into(), 4).cursor_after(), None);
    let mut cursor = 3;
    assert_eq!(
        editing::action(Input::Text("x".into()), &window(), &mut cursor),
        None
    );
    assert_eq!(cursor, 3);
}
