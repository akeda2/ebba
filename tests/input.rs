#[path = "../src/command.rs"]
mod command;
#[path = "../src/input.rs"]
mod input;

use command::{Command, MoveCommand};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use input::{
    KeybindingProfile, command_from_event_with_profile, command_from_key_event_with_profile,
};

fn map_default(key: KeyEvent) -> Option<Command> {
    command_from_key_event_with_profile(key, KeybindingProfile::Linux)
}

fn map_macos(key: KeyEvent) -> Option<Command> {
    command_from_key_event_with_profile(key, KeybindingProfile::MacOs)
}

fn map_windows(key: KeyEvent) -> Option<Command> {
    command_from_key_event_with_profile(key, KeybindingProfile::Windows)
}

#[test]
fn maps_ctrl_q_to_quit() {
    let key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL);
    assert_eq!(map_default(key), Some(Command::Quit));
}

#[test]
fn maps_ctrl_alt_q_to_force_quit() {
    let key = KeyEvent::new(
        KeyCode::Char('q'),
        KeyModifiers::CONTROL | KeyModifiers::ALT,
    );
    assert_eq!(map_default(key), Some(Command::ForceQuit));
}

#[test]
fn maps_ctrl_shift_q_to_force_quit_when_represented() {
    let key = KeyEvent::new(
        KeyCode::Char('q'),
        KeyModifiers::CONTROL | KeyModifiers::SHIFT,
    );
    assert_eq!(map_default(key), Some(Command::ForceQuit));
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
        (KeyCode::Char('h'), Command::ShowHelp),
        (KeyCode::Char('b'), Command::ToggleBom),
        (KeyCode::Char('t'), Command::CycleTabWidth),
        (KeyCode::Char('w'), Command::ToggleWrap),
        (KeyCode::Char('k'), Command::ToggleInvisibles),
        (KeyCode::Char('u'), Command::DeleteToLineStart),
    ];

    for (code, expected) in cases {
        let key = KeyEvent::new(code, KeyModifiers::CONTROL);
        assert_eq!(map_default(key), Some(expected));
    }
}

#[test]
fn maps_ctrl_q_control_character_form_to_quit() {
    let key = KeyEvent::new(KeyCode::Char('\u{11}'), KeyModifiers::NONE);
    assert_eq!(map_default(key), Some(Command::Quit));
}

#[test]
fn esc_is_not_bound_to_force_quit() {
    let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    assert_eq!(map_default(key), None);
}

#[test]
fn maps_alt_q_to_quit_in_default_profile() {
    let key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::ALT);
    assert_eq!(map_default(key), Some(Command::Quit));
}

#[test]
fn maps_f10_to_quit() {
    let key = KeyEvent::new(KeyCode::F(10), KeyModifiers::NONE);
    assert_eq!(map_default(key), Some(Command::Quit));
}

#[test]
fn maps_ctrl_g_to_force_quit() {
    let key = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL);
    assert_eq!(map_default(key), Some(Command::ForceQuit));
}

#[test]
fn maps_alt_i_to_toggle_invisibles_in_default_profile() {
    let key = KeyEvent::new(KeyCode::Char('i'), KeyModifiers::ALT);
    assert_eq!(map_default(key), Some(Command::ToggleInvisibles));
}

#[test]
fn maps_alt_b_to_toggle_bom_in_default_profile() {
    let key = KeyEvent::new(KeyCode::Char('b'), KeyModifiers::ALT);
    assert_eq!(map_default(key), Some(Command::ToggleBom));
}

#[test]
fn maps_alt_h_to_show_help_in_default_profile() {
    let key = KeyEvent::new(KeyCode::Char('h'), KeyModifiers::ALT);
    assert_eq!(map_default(key), Some(Command::ShowHelp));
}

#[test]
fn maps_ctrl_shift_b_to_toggle_bom() {
    let key = KeyEvent::new(
        KeyCode::Char('b'),
        KeyModifiers::CONTROL | KeyModifiers::SHIFT,
    );
    assert_eq!(map_default(key), Some(Command::ToggleBom));
}

#[test]
fn maps_shift_arrow_to_extending_selection_move() {
    let key = KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT);
    assert_eq!(
        map_default(key),
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
        map_default(key),
        Some(Command::Move {
            direction: MoveCommand::Right,
            extend: false
        })
    );
}

#[test]
fn maps_tab_to_insert_char_tab() {
    let key = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
    assert_eq!(map_default(key), Some(Command::InsertChar('\t')));
}

#[test]
fn maps_shift_tab_to_outdent_selection() {
    let key = KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT);
    assert_eq!(map_default(key), Some(Command::OutdentSelection));
}

#[test]
fn maps_ctrl_backspace_to_delete_word_backward() {
    let key = KeyEvent::new(KeyCode::Backspace, KeyModifiers::CONTROL);
    assert_eq!(map_default(key), Some(Command::DeleteWordBackward));
}

#[test]
fn maps_terminal_paste_payload_to_paste_text_command() {
    let event = Event::Paste("hello".to_string());
    assert_eq!(
        command_from_event_with_profile(event, KeybindingProfile::Linux),
        Some(Command::PasteText("hello".to_string()))
    );
}

#[test]
fn windows_maps_f1_to_show_help() {
    let key = KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE);
    assert_eq!(map_windows(key), Some(Command::ShowHelp));
}

#[test]
fn windows_does_not_consume_linux_alt_aliases() {
    let key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::ALT);
    assert_eq!(map_windows(key), None);
}

#[test]
fn macos_maps_command_core_shortcuts() {
    let cases = [
        (KeyCode::Char('s'), Command::Save),
        (KeyCode::Char('q'), Command::Quit),
        (KeyCode::Char('c'), Command::Copy),
        (KeyCode::Char('x'), Command::Cut),
        (KeyCode::Char('v'), Command::Paste),
        (KeyCode::Char('a'), Command::SelectAll),
        (KeyCode::Char('z'), Command::Undo),
    ];

    for (code, expected) in cases {
        let key = KeyEvent::new(code, KeyModifiers::SUPER);
        assert_eq!(map_macos(key), Some(expected));
    }
}

#[test]
fn macos_maps_command_shift_shortcuts() {
    let redo = KeyEvent::new(
        KeyCode::Char('z'),
        KeyModifiers::SUPER | KeyModifiers::SHIFT,
    );
    assert_eq!(map_macos(redo), Some(Command::Redo));

    let help = KeyEvent::new(
        KeyCode::Char('?'),
        KeyModifiers::SUPER | KeyModifiers::SHIFT,
    );
    assert_eq!(map_macos(help), Some(Command::ShowHelp));
}

#[test]
fn macos_maps_option_and_command_navigation() {
    let word_left = KeyEvent::new(KeyCode::Left, KeyModifiers::ALT);
    assert_eq!(
        map_macos(word_left),
        Some(Command::Move {
            direction: MoveCommand::WordLeft,
            extend: false
        })
    );

    let line_start = KeyEvent::new(KeyCode::Left, KeyModifiers::SUPER);
    assert_eq!(
        map_macos(line_start),
        Some(Command::Move {
            direction: MoveCommand::LineStart,
            extend: false
        })
    );

    let doc_end = KeyEvent::new(KeyCode::Down, KeyModifiers::SUPER | KeyModifiers::SHIFT);
    assert_eq!(
        map_macos(doc_end),
        Some(Command::Move {
            direction: MoveCommand::DocumentEnd,
            extend: true
        })
    );
}

#[test]
fn macos_maps_standard_deletion_shortcuts_and_fallbacks() {
    let delete_word = KeyEvent::new(KeyCode::Backspace, KeyModifiers::ALT);
    assert_eq!(map_macos(delete_word), Some(Command::DeleteWordBackward));

    let delete_line = KeyEvent::new(KeyCode::Backspace, KeyModifiers::SUPER);
    assert_eq!(map_macos(delete_line), Some(Command::DeleteToLineStart));

    let fallback_delete_word = KeyEvent::new(KeyCode::Backspace, KeyModifiers::CONTROL);
    assert_eq!(
        map_macos(fallback_delete_word),
        Some(Command::DeleteWordBackward)
    );

    let fallback_delete_line = KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL);
    assert_eq!(
        map_macos(fallback_delete_line),
        Some(Command::DeleteToLineStart)
    );
}

#[test]
fn macos_does_not_use_option_letter_aliases_for_commands() {
    let alt_h = KeyEvent::new(KeyCode::Char('h'), KeyModifiers::ALT);
    assert_eq!(map_macos(alt_h), Some(Command::InsertChar('h')));
}
