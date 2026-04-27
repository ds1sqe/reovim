use super::*;

#[test]
fn test_cmdline_state_default() {
    let state = CmdlineState::default();
    assert!(!state.is_active());
    assert_eq!(state.prompt(), CmdlinePrompt::Command);
    assert!(state.input().is_empty());
}

#[test]
fn test_cmdline_prompt_chars() {
    assert_eq!(CmdlinePrompt::Command.char(), ':');
    assert_eq!(CmdlinePrompt::SearchForward.char(), '/');
    assert_eq!(CmdlinePrompt::SearchBackward.char(), '?');
}

#[test]
fn test_cmdline_prompt_is_search() {
    assert!(!CmdlinePrompt::Command.is_search());
    assert!(CmdlinePrompt::SearchForward.is_search());
    assert!(CmdlinePrompt::SearchBackward.is_search());
}

#[test]
fn test_enter_and_exit() {
    let mut state = CmdlineState::default();

    state.enter(CmdlinePrompt::SearchForward);
    assert!(state.is_active());
    assert_eq!(state.prompt(), CmdlinePrompt::SearchForward);

    state.exit();
    assert!(!state.is_active());
    // prompt is preserved after exit so runner can read it
    assert_eq!(state.prompt(), CmdlinePrompt::SearchForward);
}

#[test]
fn test_insert_and_take() {
    let mut state = CmdlineState::default();
    state.enter(CmdlinePrompt::SearchForward);

    state.insert_char('f');
    state.insert_char('o');
    state.insert_char('o');
    assert_eq!(state.input(), "foo");
    assert_eq!(state.cursor(), 3);

    let taken = state.take_cmdline_input();
    assert_eq!(taken, "foo");
    assert!(state.input().is_empty());
}

#[test]
fn test_backspace() {
    let mut state = CmdlineState::default();
    state.enter(CmdlinePrompt::SearchForward);

    state.insert_char('a');
    state.insert_char('b');
    state.backspace();
    assert_eq!(state.input(), "a");

    state.backspace();
    assert!(state.input().is_empty());

    // Backspace on empty should be no-op
    state.backspace();
    assert!(state.input().is_empty());
}

#[test]
fn test_cancel_clears_input() {
    let mut state = CmdlineState::default();
    state.enter(CmdlinePrompt::SearchForward);

    state.insert_char('t');
    state.insert_char('e');
    state.insert_char('s');
    state.insert_char('t');

    state.cancel();
    assert!(!state.is_active());
    assert!(state.was_cancelled());
    assert!(state.input().is_empty());
}

#[test]
fn test_cancel_resets_cursor() {
    let mut state = CmdlineState::default();
    state.enter(CmdlinePrompt::Command);
    state.insert_char('w');
    state.insert_char('q');
    assert_eq!(state.cursor(), 2);

    state.cancel();
    assert_eq!(state.cursor(), 0);
}

#[test]
fn test_exit_preserves_prompt() {
    let mut state = CmdlineState::default();
    state.enter(CmdlinePrompt::SearchBackward);
    state.exit();

    // Prompt should be preserved after exit for runner to read
    assert_eq!(state.prompt(), CmdlinePrompt::SearchBackward);
    assert!(!state.was_cancelled());
}

#[test]
fn test_enter_resets_cancelled_flag() {
    let mut state = CmdlineState::default();
    state.enter(CmdlinePrompt::Command);
    state.cancel();
    assert!(state.was_cancelled());

    // Entering again should clear cancelled flag
    state.enter(CmdlinePrompt::SearchForward);
    assert!(!state.was_cancelled());
}

#[test]
fn test_enter_clears_previous_input() {
    let mut state = CmdlineState::default();
    state.enter(CmdlinePrompt::Command);
    state.insert_char('w');
    state.insert_char('q');
    assert_eq!(state.input(), "wq");

    // Entering again should clear input
    state.enter(CmdlinePrompt::SearchForward);
    assert!(state.input().is_empty());
    assert_eq!(state.cursor(), 0);
}

#[test]
fn test_take_cmdline_input_resets_cursor() {
    let mut state = CmdlineState::default();
    state.enter(CmdlinePrompt::Command);
    state.insert_char('w');
    assert_eq!(state.cursor(), 1);

    let input = state.take_cmdline_input();
    assert_eq!(input, "w");
    assert_eq!(state.cursor(), 0);
    assert!(state.input().is_empty());
}

#[test]
fn test_insert_char_at_beginning_of_input() {
    let mut state = CmdlineState::default();
    state.enter(CmdlinePrompt::Command);

    state.insert_char('b');
    state.insert_char('c');
    assert_eq!(state.input(), "bc");
    assert_eq!(state.cursor(), 2);

    state.cursor = 0;
    state.insert_char('a');
    assert_eq!(state.input(), "abc");
    assert_eq!(state.cursor(), 1);
}

#[test]
fn test_insert_char_in_middle_of_input() {
    let mut state = CmdlineState::default();
    state.enter(CmdlinePrompt::Command);
    state.insert_char('a');
    state.insert_char('c');
    assert_eq!(state.input(), "ac");

    state.cursor = 1;
    state.insert_char('b');
    assert_eq!(state.input(), "abc");
    assert_eq!(state.cursor(), 2);
}

#[test]
fn test_backspace_at_beginning_is_noop() {
    let mut state = CmdlineState::default();
    state.enter(CmdlinePrompt::Command);
    state.cursor = 0;
    state.backspace();
    assert!(state.input().is_empty());
    assert_eq!(state.cursor(), 0);
}

#[test]
fn test_cmdline_prompt_default() {
    let prompt = CmdlinePrompt::default();
    assert_eq!(prompt, CmdlinePrompt::Command);
    assert_eq!(prompt.char(), ':');
    assert!(!prompt.is_search());
}

#[test]
fn test_cmdline_prompt_clone_copy() {
    let prompt = CmdlinePrompt::SearchForward;
    let cloned = prompt;
    assert_eq!(prompt, cloned);
}

#[test]
fn test_cmdline_state_debug() {
    let state = CmdlineState::default();
    let debug = format!("{state:?}");
    assert!(debug.contains("CmdlineState"));
}

#[test]
fn test_session_extension_create() {
    let state = CmdlineState::create();
    assert!(!state.is_active());
    assert!(state.input().is_empty());
}

#[test]
fn test_text_input_sink_integration() {
    let mut state = CmdlineState::default();
    state.enter(CmdlinePrompt::Command);

    // Use the TextInputSink trait method
    TextInputSink::insert_char(&mut state, 'h');
    TextInputSink::insert_char(&mut state, 'i');

    assert_eq!(state.input(), "hi");
}

#[test]
fn test_as_text_input_sink_returns_some() {
    let mut state = CmdlineState::default();
    let sink = SessionExtension::as_text_input_sink(&mut state);
    assert!(sink.is_some());
}

// -- Enhanced editing tests (#451) --

#[test]
fn test_move_cursor_left() {
    let mut state = CmdlineState::default();
    state.enter(CmdlinePrompt::Command);
    state.insert_char('a');
    state.insert_char('b');
    assert_eq!(state.cursor(), 2);

    state.move_cursor_left();
    assert_eq!(state.cursor(), 1);
}

#[test]
fn test_move_cursor_left_at_start() {
    let mut state = CmdlineState::default();
    state.enter(CmdlinePrompt::Command);
    state.move_cursor_left();
    assert_eq!(state.cursor(), 0);
}

#[test]
fn test_move_cursor_right() {
    let mut state = CmdlineState::default();
    state.enter(CmdlinePrompt::Command);
    state.insert_char('a');
    state.insert_char('b');
    state.cursor = 0;

    state.move_cursor_right();
    assert_eq!(state.cursor(), 1);
}

#[test]
fn test_move_cursor_right_at_end() {
    let mut state = CmdlineState::default();
    state.enter(CmdlinePrompt::Command);
    state.insert_char('a');
    assert_eq!(state.cursor(), 1);

    state.move_cursor_right();
    assert_eq!(state.cursor(), 1);
}

#[test]
fn test_move_to_start() {
    let mut state = CmdlineState::default();
    state.enter(CmdlinePrompt::Command);
    state.insert_char('a');
    state.insert_char('b');
    state.insert_char('c');
    assert_eq!(state.cursor(), 3);

    state.move_to_start();
    assert_eq!(state.cursor(), 0);
}

#[test]
fn test_move_to_end() {
    let mut state = CmdlineState::default();
    state.enter(CmdlinePrompt::Command);
    state.insert_char('a');
    state.insert_char('b');
    state.cursor = 0;

    state.move_to_end();
    assert_eq!(state.cursor(), 2);
}

#[test]
fn test_delete_at_cursor() {
    let mut state = CmdlineState::default();
    state.enter(CmdlinePrompt::Command);
    state.insert_char('a');
    state.insert_char('b');
    state.insert_char('c');
    state.cursor = 1;

    state.delete_at_cursor();
    assert_eq!(state.input(), "ac");
    assert_eq!(state.cursor(), 1);
}

#[test]
fn test_delete_at_cursor_at_end() {
    let mut state = CmdlineState::default();
    state.enter(CmdlinePrompt::Command);
    state.insert_char('a');
    assert_eq!(state.cursor(), 1);

    state.delete_at_cursor();
    assert_eq!(state.input(), "a");
}

#[test]
fn test_delete_word_back() {
    let mut state = CmdlineState::default();
    state.enter(CmdlinePrompt::Command);
    for ch in "hello world".chars() {
        state.insert_char(ch);
    }
    assert_eq!(state.cursor(), 11);

    state.delete_word_back();
    assert_eq!(state.input(), "hello ");
    assert_eq!(state.cursor(), 6);
}

#[test]
fn test_delete_word_back_at_start() {
    let mut state = CmdlineState::default();
    state.enter(CmdlinePrompt::Command);
    state.cursor = 0;

    state.delete_word_back();
    assert_eq!(state.cursor(), 0);
}

#[test]
fn test_delete_word_back_trailing_spaces() {
    let mut state = CmdlineState::default();
    state.enter(CmdlinePrompt::Command);
    for ch in "foo   ".chars() {
        state.insert_char(ch);
    }
    assert_eq!(state.cursor(), 6);

    state.delete_word_back();
    assert_eq!(state.input(), "");
    assert_eq!(state.cursor(), 0);
}

#[test]
fn test_delete_to_start() {
    let mut state = CmdlineState::default();
    state.enter(CmdlinePrompt::Command);
    for ch in "hello world".chars() {
        state.insert_char(ch);
    }
    state.cursor = 5;

    state.delete_to_start();
    assert_eq!(state.input(), " world");
    assert_eq!(state.cursor(), 0);
}

#[test]
fn test_delete_to_start_at_start() {
    let mut state = CmdlineState::default();
    state.enter(CmdlinePrompt::Command);
    state.insert_char('a');
    state.cursor = 0;

    state.delete_to_start();
    assert_eq!(state.input(), "a");
    assert_eq!(state.cursor(), 0);
}

// -- History tests (#451) --

#[test]
fn test_history_up_empty() {
    let mut state = CmdlineState::default();
    state.enter(CmdlinePrompt::Command);
    state.history_up();
    assert!(state.input().is_empty());
    assert!(state.history_index.is_none());
}

#[test]
fn test_history_up_saves_input() {
    let mut state = CmdlineState::default();
    state.enter(CmdlinePrompt::Command);
    for ch in "w".chars() {
        state.insert_char(ch);
    }
    state.push_to_history();

    state.enter(CmdlinePrompt::Command);
    for ch in "current".chars() {
        state.insert_char(ch);
    }

    state.history_up();
    assert_eq!(state.input(), "w");
    assert_eq!(state.saved_input, "current");
}

#[test]
fn test_history_up_multiple() {
    let mut state = CmdlineState::default();
    state.enter(CmdlinePrompt::Command);
    state.insert_char('a');
    state.push_to_history();

    state.enter(CmdlinePrompt::Command);
    state.insert_char('b');
    state.push_to_history();

    state.enter(CmdlinePrompt::Command);
    state.insert_char('c');
    state.push_to_history();

    state.enter(CmdlinePrompt::Command);
    state.history_up();
    assert_eq!(state.input(), "c");
    state.history_up();
    assert_eq!(state.input(), "b");
    state.history_up();
    assert_eq!(state.input(), "a");
}

#[test]
fn test_history_up_at_oldest() {
    let mut state = CmdlineState::default();
    state.enter(CmdlinePrompt::Command);
    state.insert_char('a');
    state.push_to_history();

    state.enter(CmdlinePrompt::Command);
    state.history_up();
    assert_eq!(state.input(), "a");

    state.history_up();
    assert_eq!(state.input(), "a");
}

#[test]
fn test_history_down() {
    let mut state = CmdlineState::default();
    state.enter(CmdlinePrompt::Command);
    state.insert_char('a');
    state.push_to_history();

    state.enter(CmdlinePrompt::Command);
    state.insert_char('b');
    state.push_to_history();

    state.enter(CmdlinePrompt::Command);
    state.history_up();
    state.history_up();
    state.history_down();
    assert_eq!(state.input(), "b");
}

#[test]
fn test_history_down_restores_input() {
    let mut state = CmdlineState::default();
    state.enter(CmdlinePrompt::Command);
    state.insert_char('w');
    state.push_to_history();

    state.enter(CmdlinePrompt::Command);
    for ch in "new".chars() {
        state.insert_char(ch);
    }

    state.history_up();
    state.history_down();
    assert_eq!(state.input(), "new");
    assert!(state.history_index.is_none());
}

#[test]
fn test_history_down_not_navigating() {
    let mut state = CmdlineState::default();
    state.enter(CmdlinePrompt::Command);
    state.insert_char('x');
    state.history_down();
    assert_eq!(state.input(), "x");
}

#[test]
fn test_push_to_history() {
    let mut state = CmdlineState::default();
    state.enter(CmdlinePrompt::Command);
    for ch in "wq".chars() {
        state.insert_char(ch);
    }
    state.push_to_history();
    assert_eq!(state.command_history.len(), 1);
    assert_eq!(state.command_history[0], "wq");
}

#[test]
fn test_push_to_history_empty() {
    let mut state = CmdlineState::default();
    state.enter(CmdlinePrompt::Command);
    state.push_to_history();
    assert!(state.command_history.is_empty());
}

#[test]
fn test_push_to_history_dedup() {
    let mut state = CmdlineState::default();

    state.enter(CmdlinePrompt::Command);
    state.insert_char('w');
    state.push_to_history();

    state.enter(CmdlinePrompt::Command);
    state.insert_char('q');
    state.push_to_history();

    state.enter(CmdlinePrompt::Command);
    state.insert_char('w');
    state.push_to_history();

    assert_eq!(state.command_history.len(), 2);
    assert_eq!(state.command_history[0], "q");
    assert_eq!(state.command_history[1], "w");
}

#[test]
fn test_push_to_history_max() {
    let mut state = CmdlineState::default();
    for i in 0..=MAX_HISTORY {
        state.enter(CmdlinePrompt::Command);
        state.input = format!("cmd{i}");
        state.push_to_history();
    }
    assert_eq!(state.command_history.len(), MAX_HISTORY);
    assert_eq!(state.command_history[0], "cmd1");
}

#[test]
fn test_separate_command_search_history() {
    let mut state = CmdlineState::default();

    state.enter(CmdlinePrompt::Command);
    state.insert_char('w');
    state.push_to_history();

    state.enter(CmdlinePrompt::SearchForward);
    for ch in "foo".chars() {
        state.insert_char(ch);
    }
    state.push_to_history();

    assert_eq!(state.command_history.len(), 1);
    assert_eq!(state.search_history.len(), 1);
    assert_eq!(state.command_history[0], "w");
    assert_eq!(state.search_history[0], "foo");
}

// -- Completion tests (#451) --

#[test]
fn test_set_completions() {
    let mut state = CmdlineState::default();
    state.enter(CmdlinePrompt::Command);
    state.set_completions("w".to_string(), vec!["write".to_string(), "wq".to_string()]);
    assert_eq!(state.completions().len(), 2);
    assert!(state.completion_index().is_none());
}

#[test]
fn test_clear_completions() {
    let mut state = CmdlineState::default();
    state.enter(CmdlinePrompt::Command);
    state.set_completions("w".to_string(), vec!["write".to_string()]);
    state.clear_completions();
    assert!(state.completions().is_empty());
    assert!(state.completion_index().is_none());
}

#[test]
fn test_complete_next() {
    let mut state = CmdlineState::default();
    state.enter(CmdlinePrompt::Command);
    state.set_completions("w".to_string(), vec!["write".to_string(), "wq".to_string()]);

    assert!(state.complete_next());
    assert_eq!(state.input(), "write");
    assert_eq!(state.completion_index(), Some(0));

    assert!(state.complete_next());
    assert_eq!(state.input(), "wq");
    assert_eq!(state.completion_index(), Some(1));
}

#[test]
fn test_complete_next_wraps() {
    let mut state = CmdlineState::default();
    state.enter(CmdlinePrompt::Command);
    state.set_completions("w".to_string(), vec!["write".to_string(), "wq".to_string()]);

    state.complete_next();
    state.complete_next();
    state.complete_next();
    assert_eq!(state.input(), "write");
    assert_eq!(state.completion_index(), Some(0));
}

#[test]
fn test_complete_prev() {
    let mut state = CmdlineState::default();
    state.enter(CmdlinePrompt::Command);
    state.set_completions(
        "w".to_string(),
        vec!["write".to_string(), "wq".to_string(), "wall".to_string()],
    );

    assert!(state.complete_prev());
    assert_eq!(state.input(), "wall");
    assert_eq!(state.completion_index(), Some(2));

    assert!(state.complete_prev());
    assert_eq!(state.input(), "wq");
    assert_eq!(state.completion_index(), Some(1));
}

#[test]
fn test_complete_prev_wraps() {
    let mut state = CmdlineState::default();
    state.enter(CmdlinePrompt::Command);
    state.set_completions("w".to_string(), vec!["write".to_string(), "wq".to_string()]);

    state.complete_next();
    state.complete_prev();
    assert_eq!(state.input(), "wq");
}

#[test]
fn test_complete_empty() {
    let mut state = CmdlineState::default();
    state.enter(CmdlinePrompt::Command);
    assert!(!state.complete_next());
    assert!(!state.complete_prev());
}

#[test]
fn test_insert_char_clears_completions() {
    let mut state = CmdlineState::default();
    state.enter(CmdlinePrompt::Command);
    state.set_completions("w".to_string(), vec!["write".to_string()]);
    state.complete_next();
    assert_eq!(state.completion_index(), Some(0));

    state.insert_char('x');
    assert!(state.completions().is_empty());
    assert!(state.completion_index().is_none());
}

#[test]
fn test_backspace_clears_completions() {
    let mut state = CmdlineState::default();
    state.enter(CmdlinePrompt::Command);
    state.insert_char('w');
    state.set_completions("w".to_string(), vec!["write".to_string()]);
    state.complete_next();

    state.backspace();
    assert!(state.completions().is_empty());
}

#[test]
fn test_delete_at_cursor_clears_completions() {
    let mut state = CmdlineState::default();
    state.enter(CmdlinePrompt::Command);
    state.insert_char('w');
    state.insert_char('r');
    state.cursor = 0;
    state.set_completions("w".to_string(), vec!["write".to_string()]);

    state.delete_at_cursor();
    assert!(state.completions().is_empty());
}

#[test]
fn test_search_history_navigation() {
    let mut state = CmdlineState::default();

    // Build search history
    state.enter(CmdlinePrompt::SearchForward);
    for ch in "foo".chars() {
        state.insert_char(ch);
    }
    state.push_to_history();

    state.enter(CmdlinePrompt::SearchForward);
    for ch in "bar".chars() {
        state.insert_char(ch);
    }
    state.push_to_history();

    // Navigate search history (exercises current_history with search prompt)
    state.enter(CmdlinePrompt::SearchForward);
    state.history_up();
    assert_eq!(state.input(), "bar");
    state.history_up();
    assert_eq!(state.input(), "foo");
}

#[test]
fn test_enter_resets_history_navigation() {
    let mut state = CmdlineState::default();
    state.enter(CmdlinePrompt::Command);
    state.insert_char('w');
    state.push_to_history();

    state.enter(CmdlinePrompt::Command);
    state.history_up();
    assert!(state.history_index.is_some());

    state.enter(CmdlinePrompt::Command);
    assert!(state.history_index.is_none());
    assert!(state.saved_input.is_empty());
}

// -- Message tests (#558) --

#[test]
fn test_message_error() {
    let msg = CmdlineMessage::Error("E492: Not an editor command".to_string());
    assert_eq!(msg.text(), "E492: Not an editor command");
    assert_eq!(msg.kind(), "error");
}

#[test]
fn test_message_info() {
    let msg = CmdlineMessage::Info("Press ENTER to continue".to_string());
    assert_eq!(msg.text(), "Press ENTER to continue");
    assert_eq!(msg.kind(), "info");
}

#[test]
fn test_message_clone_eq() {
    let msg1 = CmdlineMessage::Error("E42".to_string());
    let msg2 = msg1.clone();
    assert_eq!(msg1, msg2);
}

#[test]
fn test_message_debug() {
    let msg = CmdlineMessage::Error("test".to_string());
    let debug = format!("{msg:?}");
    assert!(debug.contains("Error"));
}

#[test]
fn test_message_inequality() {
    let err = CmdlineMessage::Error("msg".to_string());
    let info = CmdlineMessage::Info("msg".to_string());
    assert_ne!(err, info);
}

#[test]
fn test_set_and_get_message() {
    let mut state = CmdlineState::default();
    assert!(state.message().is_none());

    state.set_message(CmdlineMessage::Error("E492: foo".to_string()));
    assert!(state.message().is_some());
    assert_eq!(state.message().unwrap().text(), "E492: foo");
    assert_eq!(state.message().unwrap().kind(), "error");
}

#[test]
fn test_clear_message() {
    let mut state = CmdlineState::default();
    state.set_message(CmdlineMessage::Info("done".to_string()));
    assert!(state.message().is_some());

    state.clear_message();
    assert!(state.message().is_none());
}

#[test]
fn test_enter_clears_message() {
    let mut state = CmdlineState::default();
    state.set_message(CmdlineMessage::Error("old error".to_string()));

    state.enter(CmdlinePrompt::Command);
    assert!(state.message().is_none());
}

#[test]
fn test_message_default_is_none() {
    let state = CmdlineState::default();
    assert!(state.message().is_none());
}

#[test]
fn test_set_message_overwrites() {
    let mut state = CmdlineState::default();
    state.set_message(CmdlineMessage::Error("first".to_string()));
    state.set_message(CmdlineMessage::Info("second".to_string()));
    assert_eq!(state.message().unwrap().text(), "second");
    assert_eq!(state.message().unwrap().kind(), "info");
}
