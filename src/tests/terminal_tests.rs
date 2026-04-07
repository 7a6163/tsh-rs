use crate::terminal::{KeyAction, TerminalHandler};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

/// Create a TerminalHandler for testing.
/// Note: Some tests may fail in environments without a terminal (CI headless).
/// We guard those with a helper that skips if terminal init fails.
fn make_handler() -> Option<TerminalHandler> {
    TerminalHandler::new().ok()
}

// ─── History tests ──────────────────────────────────────────────────────────

#[test]
fn test_add_to_history() {
    let Some(mut handler) = make_handler() else {
        return;
    };

    handler.add_to_history_pub("ls -la".to_string());
    handler.add_to_history_pub("pwd".to_string());

    assert_eq!(handler.history_len(), 2);
}

#[test]
fn test_add_to_history_dedup_consecutive() {
    let Some(mut handler) = make_handler() else {
        return;
    };

    handler.add_to_history_pub("ls".to_string());
    handler.add_to_history_pub("ls".to_string());

    assert_eq!(handler.history_len(), 1);
}

#[test]
fn test_add_to_history_allows_non_consecutive_dup() {
    let Some(mut handler) = make_handler() else {
        return;
    };

    handler.add_to_history_pub("ls".to_string());
    handler.add_to_history_pub("pwd".to_string());
    handler.add_to_history_pub("ls".to_string());

    assert_eq!(handler.history_len(), 3);
}

#[test]
fn test_history_max_size() {
    let Some(mut handler) = make_handler() else {
        return;
    };

    for i in 0..1100 {
        handler.add_to_history_pub(format!("cmd_{i}"));
    }

    // HISTORY_SIZE is 1000
    assert_eq!(handler.history_len(), 1000);
}

// ─── Line editing state tests ───────────────────────────────────────────────

#[test]
fn test_initial_state() {
    let Some(handler) = make_handler() else {
        return;
    };

    assert_eq!(handler.current_line(), "");
    assert_eq!(handler.cursor_pos(), 0);
    assert_eq!(handler.history_len(), 0);
}

#[test]
fn test_set_prompt() {
    let Some(mut handler) = make_handler() else {
        return;
    };

    handler.set_prompt(">>> ".to_string());
    assert_eq!(handler.prompt(), ">>> ");
}

// ─── Cursor position / line editing (unit-level, no display) ────────────────

#[test]
fn test_insert_char_advances_cursor() {
    let Some(mut handler) = make_handler() else {
        return;
    };

    handler.insert_char('a');
    handler.insert_char('b');
    handler.insert_char('c');

    assert_eq!(handler.current_line(), "abc");
    assert_eq!(handler.cursor_pos(), 3);
}

#[test]
fn test_backspace_at_start_does_nothing() {
    let Some(mut handler) = make_handler() else {
        return;
    };

    assert_eq!(handler.cursor_pos(), 0);
    handler.backspace();
    assert_eq!(handler.cursor_pos(), 0);
    assert_eq!(handler.current_line(), "");
}

#[test]
fn test_backspace_removes_char() {
    let Some(mut handler) = make_handler() else {
        return;
    };

    handler.insert_char('a');
    handler.insert_char('b');
    handler.backspace();

    assert_eq!(handler.current_line(), "a");
    assert_eq!(handler.cursor_pos(), 1);
}

#[test]
fn test_cursor_left_right() {
    let Some(mut handler) = make_handler() else {
        return;
    };

    handler.insert_char('a');
    handler.insert_char('b');
    handler.insert_char('c');

    handler.move_cursor_left();
    assert_eq!(handler.cursor_pos(), 2);

    handler.move_cursor_left();
    assert_eq!(handler.cursor_pos(), 1);

    handler.move_cursor_right();
    assert_eq!(handler.cursor_pos(), 2);
}

#[test]
fn test_cursor_left_at_start() {
    let Some(mut handler) = make_handler() else {
        return;
    };

    handler.move_cursor_left();
    assert_eq!(handler.cursor_pos(), 0);
}

#[test]
fn test_cursor_right_at_end() {
    let Some(mut handler) = make_handler() else {
        return;
    };

    handler.insert_char('x');
    handler.move_cursor_right();
    assert_eq!(handler.cursor_pos(), 1); // should not go past end
}

#[test]
fn test_home_end() {
    let Some(mut handler) = make_handler() else {
        return;
    };

    handler.insert_char('a');
    handler.insert_char('b');
    handler.insert_char('c');

    handler.move_to_home();
    assert_eq!(handler.cursor_pos(), 0);

    handler.move_to_end();
    assert_eq!(handler.cursor_pos(), 3);
}

#[test]
fn test_insert_at_middle() {
    let Some(mut handler) = make_handler() else {
        return;
    };

    handler.insert_char('a');
    handler.insert_char('c');
    handler.move_cursor_left(); // cursor at pos 1
    handler.insert_char('b'); // insert 'b' at pos 1

    assert_eq!(handler.current_line(), "abc");
    assert_eq!(handler.cursor_pos(), 2);
}

#[test]
fn test_delete_at_cursor() {
    let Some(mut handler) = make_handler() else {
        return;
    };

    handler.insert_char('a');
    handler.insert_char('b');
    handler.insert_char('c');
    handler.move_to_home();
    handler.delete_at_cursor();

    assert_eq!(handler.current_line(), "bc");
    assert_eq!(handler.cursor_pos(), 0);
}

#[test]
fn test_delete_at_end_does_nothing() {
    let Some(mut handler) = make_handler() else {
        return;
    };

    handler.insert_char('a');
    handler.delete_at_cursor();

    assert_eq!(handler.current_line(), "a");
    assert_eq!(handler.cursor_pos(), 1);
}

#[test]
fn test_clear_line() {
    let Some(mut handler) = make_handler() else {
        return;
    };

    handler.insert_char('a');
    handler.insert_char('b');
    handler.clear_line();

    assert_eq!(handler.current_line(), "");
    assert_eq!(handler.cursor_pos(), 0);
}

// ─── Enter key behavior ────────────────────────────────────────────────────

#[test]
fn test_submit_line() {
    let Some(mut handler) = make_handler() else {
        return;
    };

    handler.insert_char('l');
    handler.insert_char('s');

    let line = handler.submit_line();
    assert_eq!(line, "ls");

    // After submit, line should be cleared and history updated
    assert_eq!(handler.current_line(), "");
    assert_eq!(handler.cursor_pos(), 0);
    assert_eq!(handler.history_len(), 1);
}

#[test]
fn test_submit_empty_line_not_in_history() {
    let Some(mut handler) = make_handler() else {
        return;
    };

    let line = handler.submit_line();
    assert_eq!(line, "");
    assert_eq!(handler.history_len(), 0);
}

// ─── Helper to create KeyEvent ──────────────────────────────────────────────

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    }
}

fn ctrl_key(c: char) -> KeyEvent {
    KeyEvent {
        code: KeyCode::Char(c),
        modifiers: KeyModifiers::CONTROL,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    }
}

// ─── process_key_logic tests ────────────────────────────────────────────────

#[test]
fn test_key_logic_ctrl_c_exits() {
    let Some(mut handler) = make_handler() else {
        return;
    };

    let action = handler.process_key_logic(ctrl_key('c'));
    assert!(matches!(action, KeyAction::Exit));
}

#[test]
fn test_key_logic_ctrl_d_exits_on_empty_line() {
    let Some(mut handler) = make_handler() else {
        return;
    };

    let action = handler.process_key_logic(ctrl_key('d'));
    assert!(matches!(action, KeyAction::Exit));
}

#[test]
fn test_key_logic_ctrl_d_noop_on_nonempty_line() {
    let Some(mut handler) = make_handler() else {
        return;
    };

    handler.insert_char('a');
    let action = handler.process_key_logic(ctrl_key('d'));
    assert!(matches!(action, KeyAction::Noop));
}

#[test]
fn test_key_logic_ctrl_l_clears_screen() {
    let Some(mut handler) = make_handler() else {
        return;
    };

    let action = handler.process_key_logic(ctrl_key('l'));
    assert!(matches!(action, KeyAction::ClearScreen));
}

#[test]
fn test_key_logic_enter_submits_line() {
    let Some(mut handler) = make_handler() else {
        return;
    };

    handler.insert_char('l');
    handler.insert_char('s');
    let action = handler.process_key_logic(key(KeyCode::Enter));

    match action {
        KeyAction::SubmitLine(data) => {
            assert_eq!(data, b"ls\n");
        }
        _ => panic!("Expected SubmitLine"),
    }

    assert_eq!(handler.current_line(), "");
    assert_eq!(handler.cursor_pos(), 0);
    assert_eq!(handler.history_len(), 1);
}

#[test]
fn test_key_logic_enter_empty_not_in_history() {
    let Some(mut handler) = make_handler() else {
        return;
    };

    let action = handler.process_key_logic(key(KeyCode::Enter));
    match action {
        KeyAction::SubmitLine(data) => {
            assert_eq!(data, b"\n");
        }
        _ => panic!("Expected SubmitLine"),
    }
    assert_eq!(handler.history_len(), 0);
}

#[test]
fn test_key_logic_char_inserts() {
    let Some(mut handler) = make_handler() else {
        return;
    };

    let action = handler.process_key_logic(key(KeyCode::Char('x')));
    assert!(matches!(action, KeyAction::Redisplay));
    assert_eq!(handler.current_line(), "x");
    assert_eq!(handler.cursor_pos(), 1);
}

#[test]
fn test_key_logic_backspace() {
    let Some(mut handler) = make_handler() else {
        return;
    };

    handler.insert_char('a');
    handler.insert_char('b');
    let action = handler.process_key_logic(key(KeyCode::Backspace));
    assert!(matches!(action, KeyAction::Redisplay));
    assert_eq!(handler.current_line(), "a");
}

#[test]
fn test_key_logic_delete() {
    let Some(mut handler) = make_handler() else {
        return;
    };

    handler.insert_char('a');
    handler.insert_char('b');
    handler.move_to_home();
    let action = handler.process_key_logic(key(KeyCode::Delete));
    assert!(matches!(action, KeyAction::Redisplay));
    assert_eq!(handler.current_line(), "b");
}

#[test]
fn test_key_logic_left_right() {
    let Some(mut handler) = make_handler() else {
        return;
    };

    handler.insert_char('a');
    handler.insert_char('b');

    handler.process_key_logic(key(KeyCode::Left));
    assert_eq!(handler.cursor_pos(), 1);

    handler.process_key_logic(key(KeyCode::Right));
    assert_eq!(handler.cursor_pos(), 2);
}

#[test]
fn test_key_logic_home_end() {
    let Some(mut handler) = make_handler() else {
        return;
    };

    handler.insert_char('a');
    handler.insert_char('b');

    handler.process_key_logic(key(KeyCode::Home));
    assert_eq!(handler.cursor_pos(), 0);

    handler.process_key_logic(key(KeyCode::End));
    assert_eq!(handler.cursor_pos(), 2);
}

#[test]
fn test_key_logic_tab_sends_tab() {
    let Some(mut handler) = make_handler() else {
        return;
    };

    let action = handler.process_key_logic(key(KeyCode::Tab));
    match action {
        KeyAction::Send(data) => assert_eq!(data, vec![b'\t']),
        _ => panic!("Expected Send"),
    }
}

#[test]
fn test_key_logic_unknown_key_noop() {
    let Some(mut handler) = make_handler() else {
        return;
    };

    let action = handler.process_key_logic(key(KeyCode::F(1)));
    assert!(matches!(action, KeyAction::Noop));
}

// ─── History navigation via key logic ───────────────────────────────────────

#[test]
fn test_key_logic_up_down_history() {
    let Some(mut handler) = make_handler() else {
        return;
    };

    handler.add_to_history_pub("cmd1".to_string());
    handler.add_to_history_pub("cmd2".to_string());

    // Up: should get most recent (cmd2)
    handler.process_key_logic(key(KeyCode::Up));
    assert_eq!(handler.current_line(), "cmd2");

    // Up again: should get cmd1
    handler.process_key_logic(key(KeyCode::Up));
    assert_eq!(handler.current_line(), "cmd1");

    // Down: should go back to cmd2
    handler.process_key_logic(key(KeyCode::Down));
    assert_eq!(handler.current_line(), "cmd2");

    // Down again: should clear (back to input)
    handler.process_key_logic(key(KeyCode::Down));
    assert_eq!(handler.current_line(), "");
}

#[test]
fn test_navigate_up_empty_history() {
    let Some(mut handler) = make_handler() else {
        return;
    };

    handler.navigate_up();
    assert_eq!(handler.current_line(), "");
}

#[test]
fn test_navigate_up_at_end_of_history() {
    let Some(mut handler) = make_handler() else {
        return;
    };

    handler.add_to_history_pub("only".to_string());
    handler.navigate_up(); // gets "only"
    handler.navigate_up(); // at end, should stay
    assert_eq!(handler.current_line(), "only");
}

#[test]
fn test_navigate_down_without_up_noop() {
    let Some(mut handler) = make_handler() else {
        return;
    };

    handler.add_to_history_pub("cmd".to_string());
    handler.navigate_down(); // no-op, history_pos is None
    assert_eq!(handler.current_line(), "");
}
