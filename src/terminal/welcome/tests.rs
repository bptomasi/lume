use crossterm::event::{KeyEvent, ModifierKeyCode};

use super::*;

#[test]
fn entry_keys_are_consumed_without_becoming_workspace_actions() {
    for code in [
        KeyCode::Char('q'),
        KeyCode::Char('d'),
        KeyCode::Char(':'),
        KeyCode::Enter,
        KeyCode::Esc,
        KeyCode::Down,
    ] {
        let mut visible = true;
        let event = Event::Key(KeyEvent::new(code, KeyModifiers::NONE));
        assert!(matches!(handle(&mut visible, event), InputOutcome::Redraw));
        assert!(!visible);
    }
}

#[test]
fn releases_repeats_modifiers_paste_and_focus_do_not_dismiss_the_art() {
    for event in [
        Event::Key(KeyEvent::new_with_kind(
            KeyCode::Enter,
            KeyModifiers::NONE,
            KeyEventKind::Release,
        )),
        Event::Key(KeyEvent::new_with_kind(
            KeyCode::Down,
            KeyModifiers::NONE,
            KeyEventKind::Repeat,
        )),
        Event::Key(KeyEvent::new(
            KeyCode::Modifier(ModifierKeyCode::LeftShift),
            KeyModifiers::SHIFT,
        )),
        Event::Key(KeyEvent::new(KeyCode::Null, KeyModifiers::NONE)),
        Event::Paste("dd".into()),
        Event::FocusGained,
        Event::FocusLost,
    ] {
        let mut visible = true;
        assert!(matches!(handle(&mut visible, event), InputOutcome::Idle));
        assert!(visible);
    }
}

#[test]
fn resize_redraws_and_ctrl_c_quits_without_entering_the_workspace() {
    let mut visible = true;
    assert!(matches!(
        handle(&mut visible, Event::Resize(80, 24)),
        InputOutcome::Redraw
    ));
    assert!(visible);
    let cancel =
        Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
    assert!(matches!(handle(&mut visible, cancel), InputOutcome::Quit));
    assert!(visible);
}
