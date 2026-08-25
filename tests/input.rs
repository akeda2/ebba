#[path = "../src/command.rs"]
mod command;
#[path = "../src/input.rs"]
mod input;

use command::{Command, MoveCommand};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use input::command_from_key_event;

#[test]
fn maps_ctrl_q_to_quit() {
    let key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL);
    assert_eq!(command_from_key_event(key), Some(Command::Quit));
}

#[test]
fn maps_ctrl_alt_q_to_force_quit() {
    let key = KeyEvent::new(
        KeyCode::Char('q'),
        KeyModifiers::CONTROL | KeyModifiers::ALT,
    );
    assert_eq!(command_from_key_event(key), Some(Command::ForceQuit));
}

#[test]
fn maps_ctrl_shift_q_to_force_quit_when_represented() {
    let key = KeyEvent::new(
        KeyCode::Char('q'),
        KeyModifiers::CONTROL | KeyModifiers::SHIFT,
    );
    assert_eq!(command_from_key_event(key), Some(Command::ForceQuit));
}

#[test]
fn maps_core_ctrl_shortcuts() {
    let cases = [
        (KeyCode::Char('c'), Command::Copy),
        (KeyCode::Char('x'), Command::Cut),
        (KeyCode::Char('v'), Command::Paste),
        (KeyCode::Char('a'), Command::SelectAll),
        (KeyCode::Char('z'), Command::Undo),
        (KeyCode::Char('y'), Command::Redo),
        (KeyCode::Char('s'), Command::Save),
        (KeyCode::Char('t'), Command::CycleTabWidth),
        (KeyCode::Char('w'), Command::ToggleWrap),
    ];

    for (code, expected) in cases {
        let key = KeyEvent::new(code, KeyModifiers::CONTROL);
        assert_eq!(command_from_key_event(key), Some(expected));
    }
}

#[test]
fn maps_ctrl_q_control_character_form_to_quit() {
    let key = KeyEvent::new(KeyCode::Char('\u{11}'), KeyModifiers::NONE);
    assert_eq!(command_from_key_event(key), Some(Command::Quit));
}

#[test]
fn maps_escape_to_force_quit() {
    let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    assert_eq!(command_from_key_event(key), Some(Command::ForceQuit));
}

#[test]
fn maps_alt_q_to_quit() {
    let key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::ALT);
    assert_eq!(command_from_key_event(key), Some(Command::Quit));
}

#[test]
fn maps_f10_to_quit() {
    let key = KeyEvent::new(KeyCode::F(10), KeyModifiers::NONE);
    assert_eq!(command_from_key_event(key), Some(Command::Quit));
}

#[test]
fn maps_ctrl_g_to_force_quit() {
    let key = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL);
    assert_eq!(command_from_key_event(key), Some(Command::ForceQuit));
}

#[test]
fn maps_shift_arrow_to_extending_selection_move() {
    let key = KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT);
    assert_eq!(
        command_from_key_event(key),
        Some(Command::Move {
            direction: MoveCommand::Right,
            extend: true
        })
    );
}

#[test]
fn maps_plain_arrow_to_non_extending_move() {
    let key = KeyEvent::new(KeyCode::Right, KeyModifiers::NONE);
    assert_eq!(
        command_from_key_event(key),
        Some(Command::Move {
            direction: MoveCommand::Right,
            extend: false
        })
    );
}

#[test]
fn maps_tab_to_insert_char_tab() {
    let key = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
    assert_eq!(command_from_key_event(key), Some(Command::InsertChar('\t')));
}

#[test]
fn maps_shift_tab_to_outdent_selection() {
    let key = KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT);
    assert_eq!(command_from_key_event(key), Some(Command::OutdentSelection));
}
