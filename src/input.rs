use std::{env, time::Duration};

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, poll, read};

use crate::command::{AppResult, Command, MoveCommand};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeybindingProfile {
    Linux,
    LinuxConsole,
    MacOs,
    Windows,
}

impl KeybindingProfile {
    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::MacOs
        } else if cfg!(target_os = "windows") {
            Self::Windows
        } else if is_linux_console_term() {
            Self::LinuxConsole
        } else {
            Self::Linux
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventLoop {
    pub poll_timeout: Duration,
    pub max_events_per_tick: usize,
    pub profile: KeybindingProfile,
}

impl Default for EventLoop {
    fn default() -> Self {
        Self {
            poll_timeout: Duration::from_millis(16),
            max_events_per_tick: 32,
            profile: KeybindingProfile::current(),
        }
    }
}

impl EventLoop {
    pub fn tick(&self) -> AppResult<Vec<Command>> {
        poll_commands_with_profile(self.poll_timeout, self.max_events_per_tick, self.profile)
    }

    pub fn process_events<I>(&self, events: I) -> Vec<Command>
    where
        I: IntoIterator<Item = Event>,
    {
        commands_from_events_with_profile(events, self.profile)
    }
}

pub fn poll_commands(timeout: Duration, max_events: usize) -> AppResult<Vec<Command>> {
    poll_commands_with_profile(timeout, max_events, KeybindingProfile::current())
}

pub fn poll_commands_with_profile(
    timeout: Duration,
    max_events: usize,
    profile: KeybindingProfile,
) -> AppResult<Vec<Command>> {
    if max_events == 0 || !poll(timeout)? {
        return Ok(Vec::new());
    }

    let mut events = Vec::with_capacity(max_events);
    events.push(read()?);

    while events.len() < max_events && poll(Duration::from_millis(0))? {
        events.push(read()?);
    }

    Ok(commands_from_events_with_profile(events, profile))
}

pub fn commands_from_events<I>(events: I) -> Vec<Command>
where
    I: IntoIterator<Item = Event>,
{
    commands_from_events_with_profile(events, KeybindingProfile::current())
}

pub fn commands_from_events_with_profile<I>(events: I, profile: KeybindingProfile) -> Vec<Command>
where
    I: IntoIterator<Item = Event>,
{
    events
        .into_iter()
        .filter_map(|event| command_from_event_with_profile(event, profile))
        .collect()
}

pub fn command_from_event(event: Event) -> Option<Command> {
    command_from_event_with_profile(event, KeybindingProfile::current())
}

pub fn command_from_event_with_profile(
    event: Event,
    profile: KeybindingProfile,
) -> Option<Command> {
    match event {
        Event::Key(key_event) => command_from_key_event_with_profile(key_event, profile),
        Event::Paste(text) => Some(Command::PasteText(text)),
        _ => None,
    }
}

pub fn command_from_key_event(key_event: KeyEvent) -> Option<Command> {
    command_from_key_event_with_profile(key_event, KeybindingProfile::current())
}

pub fn command_from_key_event_with_profile(
    key_event: KeyEvent,
    profile: KeybindingProfile,
) -> Option<Command> {
    if key_event.kind == KeyEventKind::Release {
        return None;
    }

    let mut code = key_event.code;
    let mut modifiers = key_event.modifiers;
    if let KeyCode::Char(ch) = code
        && let Some(ctrl_ch) = decode_ascii_control(ch)
    {
        code = KeyCode::Char(ctrl_ch);
        modifiers |= KeyModifiers::CONTROL;
    }

    let ctrl = modifiers.contains(KeyModifiers::CONTROL);
    let shift = modifiers.contains(KeyModifiers::SHIFT);
    let alt = modifiers.contains(KeyModifiers::ALT);
    let super_key = modifiers.contains(KeyModifiers::SUPER);

    if let KeyCode::Char(ch) = code
        && super_key
        && let Some(command) = map_super_shortcut(ch, shift, alt, profile)
    {
        return Some(command);
    }

    if let KeyCode::Char(ch) = code
        && ctrl
    {
        let lower = ch.to_ascii_lowercase();
        if let Some(command) = map_ctrl_shortcut(lower, ch, shift, alt, profile) {
            return Some(command);
        }
    }

    if matches!(code, KeyCode::Null | KeyCode::Char('\0')) {
        return Some(Command::ToggleSelectionMode);
    }

    match code {
        KeyCode::Char('q') if alt && shift && profile == KeybindingProfile::Linux => {
            Some(Command::ForceQuit)
        }
        KeyCode::Char('s') | KeyCode::Char('S')
            if alt
                && matches!(
                    profile,
                    KeybindingProfile::Linux
                        | KeybindingProfile::LinuxConsole
                        | KeybindingProfile::Windows
                ) =>
        {
            Some(Command::ToggleSelectionMode)
        }
        KeyCode::Char('q') if alt && profile == KeybindingProfile::Linux => Some(Command::Quit),
        KeyCode::Char('Q') if alt && profile == KeybindingProfile::Linux => {
            Some(Command::ForceQuit)
        }
        KeyCode::Char('i') | KeyCode::Char('I') if alt && profile == KeybindingProfile::Linux => {
            Some(Command::ToggleInvisibles)
        }
        KeyCode::Char('k') | KeyCode::Char('K')
            if alt
                && matches!(
                    profile,
                    KeybindingProfile::Linux
                        | KeybindingProfile::LinuxConsole
                        | KeybindingProfile::Windows
                ) =>
        {
            Some(Command::ToggleInvisibles)
        }
        KeyCode::Char('.') if alt && profile == KeybindingProfile::Linux => {
            Some(Command::ToggleInvisibles)
        }
        KeyCode::Char('.') if alt && profile == KeybindingProfile::LinuxConsole => {
            Some(Command::ToggleInvisibles)
        }
        KeyCode::Char('t') | KeyCode::Char('T')
            if alt
                && shift
                && matches!(
                    profile,
                    KeybindingProfile::Linux
                        | KeybindingProfile::LinuxConsole
                        | KeybindingProfile::Windows
                ) =>
        {
            Some(Command::ToggleHardTabs)
        }
        KeyCode::Char('h') | KeyCode::Char('H') if alt && profile == KeybindingProfile::Linux => {
            Some(Command::ShowHelp)
        }
        KeyCode::Char('h') | KeyCode::Char('H')
            if alt && profile == KeybindingProfile::LinuxConsole =>
        {
            Some(Command::ShowHelp)
        }
        KeyCode::Char('b') | KeyCode::Char('B') if alt && profile == KeybindingProfile::Linux => {
            Some(Command::ToggleBom)
        }
        KeyCode::Char(ch) if should_insert_char(modifiers, profile) => {
            Some(Command::InsertChar(ch))
        }
        KeyCode::Tab => Some(Command::InsertChar('\t')),
        KeyCode::BackTab => Some(Command::OutdentSelection),
        KeyCode::Enter => Some(Command::NewLine),
        KeyCode::Backspace if matches!(profile, KeybindingProfile::MacOs) && super_key => {
            Some(Command::DeleteToLineStart)
        }
        KeyCode::Backspace if alt || ctrl => Some(Command::DeleteWordBackward),
        KeyCode::Backspace => Some(Command::Backspace),
        KeyCode::Delete => Some(Command::Delete),
        KeyCode::F(1) => Some(Command::ShowHelp),
        KeyCode::F(2) => Some(Command::Save),
        KeyCode::F(3) => Some(Command::ToggleSelectionMode),
        KeyCode::F(4) => Some(Command::ToggleHardTabs),
        KeyCode::F(5) => Some(Command::CycleTabWidth),
        KeyCode::F(6) => Some(Command::ToggleInvisibles),
        KeyCode::F(7) => Some(Command::ToggleWrap),
        KeyCode::F(8) => Some(Command::ToggleBom),
        KeyCode::F(9) => Some(Command::ShowHelp),
        KeyCode::F(10) => Some(Command::Quit),
        KeyCode::F(12) => Some(Command::ForceQuit),
        KeyCode::Left if is_word_left(modifiers, profile) => Some(Command::Move {
            direction: MoveCommand::WordLeft,
            extend: shift,
        }),
        KeyCode::Right if is_word_right(modifiers, profile) => Some(Command::Move {
            direction: MoveCommand::WordRight,
            extend: shift,
        }),
        KeyCode::Left if is_line_start(modifiers, profile) => Some(Command::Move {
            direction: MoveCommand::LineStart,
            extend: shift,
        }),
        KeyCode::Right if is_line_end(modifiers, profile) => Some(Command::Move {
            direction: MoveCommand::LineEnd,
            extend: shift,
        }),
        KeyCode::Up if is_document_start(modifiers, profile) => Some(Command::Move {
            direction: MoveCommand::DocumentStart,
            extend: shift,
        }),
        KeyCode::Down if is_document_end(modifiers, profile) => Some(Command::Move {
            direction: MoveCommand::DocumentEnd,
            extend: shift,
        }),
        KeyCode::Home if ctrl => Some(Command::Move {
            direction: MoveCommand::DocumentStart,
            extend: shift,
        }),
        KeyCode::End if ctrl => Some(Command::Move {
            direction: MoveCommand::DocumentEnd,
            extend: shift,
        }),
        KeyCode::Home => Some(Command::Move {
            direction: MoveCommand::LineStart,
            extend: shift,
        }),
        KeyCode::End => Some(Command::Move {
            direction: MoveCommand::LineEnd,
            extend: shift,
        }),
        KeyCode::Left => Some(Command::Move {
            direction: MoveCommand::Left,
            extend: shift,
        }),
        KeyCode::Right => Some(Command::Move {
            direction: MoveCommand::Right,
            extend: shift,
        }),
        KeyCode::Up => Some(Command::Move {
            direction: MoveCommand::Up,
            extend: shift,
        }),
        KeyCode::Down => Some(Command::Move {
            direction: MoveCommand::Down,
            extend: shift,
        }),
        KeyCode::PageUp => Some(Command::Move {
            direction: MoveCommand::PageUp,
            extend: shift,
        }),
        KeyCode::PageDown => Some(Command::Move {
            direction: MoveCommand::PageDown,
            extend: shift,
        }),
        _ => None,
    }
}

fn map_super_shortcut(
    ch: char,
    shift: bool,
    alt: bool,
    profile: KeybindingProfile,
) -> Option<Command> {
    if profile != KeybindingProfile::MacOs {
        return None;
    }

    let lower = ch.to_ascii_lowercase();
    match lower {
        'q' if alt => Some(Command::ForceQuit),
        'q' => Some(Command::Quit),
        's' => Some(Command::Save),
        'c' => Some(Command::Copy),
        'x' => Some(Command::Cut),
        'v' => Some(Command::Paste),
        'a' => Some(Command::SelectAll),
        'z' if shift || ch.is_ascii_uppercase() => Some(Command::Redo),
        'z' => Some(Command::Undo),
        'g' => Some(Command::ForceQuit),
        '/' | '?' => Some(Command::ShowHelp),
        _ => None,
    }
}

fn map_ctrl_shortcut(
    lower: char,
    ch: char,
    shift: bool,
    alt: bool,
    _profile: KeybindingProfile,
) -> Option<Command> {
    if ch == ' ' {
        return Some(Command::ToggleSelectionMode);
    }

    match lower {
        'q' if shift || ch.is_ascii_uppercase() => Some(Command::ForceQuit),
        'q' if alt => Some(Command::ForceQuit),
        'q' => Some(Command::Quit),
        'g' if shift || ch.is_ascii_uppercase() => Some(Command::ForceQuit),
        'g' => Some(Command::ForceQuit),
        's' => Some(Command::Save),
        'h' if shift || ch.is_ascii_uppercase() => Some(Command::ToggleHardTabs),
        'h' => Some(Command::ShowHelp),
        'b' => Some(Command::ToggleBom),
        't' if alt => Some(Command::ToggleHardTabs),
        't' if shift || ch.is_ascii_uppercase() => Some(Command::ToggleHardTabs),
        't' => Some(Command::CycleTabWidth),
        'w' => Some(Command::ToggleWrap),
        '.' => Some(Command::ToggleInvisibles),
        'k' => Some(Command::ToggleInvisibles),
        'c' => Some(Command::Copy),
        'x' => Some(Command::Cut),
        'v' => Some(Command::Paste),
        'a' => Some(Command::SelectAll),
        'z' if shift || ch.is_ascii_uppercase() => Some(Command::Redo),
        'z' => Some(Command::Undo),
        'y' => Some(Command::Redo),
        'u' => Some(Command::DeleteToLineStart),
        _ => None,
    }
}

fn should_insert_char(modifiers: KeyModifiers, profile: KeybindingProfile) -> bool {
    let super_key = modifiers.contains(KeyModifiers::SUPER);
    if super_key {
        return false;
    }
    if profile == KeybindingProfile::MacOs {
        return !modifiers.contains(KeyModifiers::CONTROL);
    }
    !modifiers.contains(KeyModifiers::ALT) && !modifiers.contains(KeyModifiers::CONTROL)
}

fn is_word_left(modifiers: KeyModifiers, profile: KeybindingProfile) -> bool {
    let ctrl = modifiers.contains(KeyModifiers::CONTROL);
    let alt = modifiers.contains(KeyModifiers::ALT);
    match profile {
        KeybindingProfile::MacOs => alt || ctrl,
        KeybindingProfile::Linux | KeybindingProfile::LinuxConsole | KeybindingProfile::Windows => {
            ctrl
        }
    }
}

fn is_word_right(modifiers: KeyModifiers, profile: KeybindingProfile) -> bool {
    is_word_left(modifiers, profile)
}

fn is_line_start(modifiers: KeyModifiers, profile: KeybindingProfile) -> bool {
    match profile {
        KeybindingProfile::MacOs => modifiers.contains(KeyModifiers::SUPER),
        KeybindingProfile::Linux | KeybindingProfile::LinuxConsole | KeybindingProfile::Windows => {
            false
        }
    }
}

fn is_line_end(modifiers: KeyModifiers, profile: KeybindingProfile) -> bool {
    is_line_start(modifiers, profile)
}

fn is_document_start(modifiers: KeyModifiers, profile: KeybindingProfile) -> bool {
    match profile {
        KeybindingProfile::MacOs => modifiers.contains(KeyModifiers::SUPER),
        KeybindingProfile::Linux | KeybindingProfile::LinuxConsole | KeybindingProfile::Windows => {
            false
        }
    }
}

fn is_linux_console_term() -> bool {
    cfg!(target_os = "linux")
        && env::var("TERM")
            .ok()
            .is_some_and(|term| term.eq_ignore_ascii_case("linux"))
}

fn is_document_end(modifiers: KeyModifiers, profile: KeybindingProfile) -> bool {
    is_document_start(modifiers, profile)
}

fn decode_ascii_control(ch: char) -> Option<char> {
    if ch.is_ascii() {
        let byte = ch as u8;
        if (1..=26).contains(&byte) {
            return Some((b'a' + (byte - 1)) as char);
        }
    }
    None
}
