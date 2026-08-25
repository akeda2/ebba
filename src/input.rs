use std::time::Duration;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, poll, read};

use crate::command::{AppResult, Command, MoveCommand};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventLoop {
    pub poll_timeout: Duration,
    pub max_events_per_tick: usize,
}

impl Default for EventLoop {
    fn default() -> Self {
        Self {
            poll_timeout: Duration::from_millis(16),
            max_events_per_tick: 32,
        }
    }
}

impl EventLoop {
    pub fn tick(&self) -> AppResult<Vec<Command>> {
        poll_commands(self.poll_timeout, self.max_events_per_tick)
    }

    pub fn process_events<I>(&self, events: I) -> Vec<Command>
    where
        I: IntoIterator<Item = Event>,
    {
        commands_from_events(events)
    }
}

pub fn poll_commands(timeout: Duration, max_events: usize) -> AppResult<Vec<Command>> {
    if max_events == 0 || !poll(timeout)? {
        return Ok(Vec::new());
    }

    let mut events = Vec::with_capacity(max_events);
    events.push(read()?);

    while events.len() < max_events && poll(Duration::from_millis(0))? {
        events.push(read()?);
    }

    Ok(commands_from_events(events))
}

pub fn commands_from_events<I>(events: I) -> Vec<Command>
where
    I: IntoIterator<Item = Event>,
{
    events.into_iter().filter_map(command_from_event).collect()
}

pub fn command_from_event(event: Event) -> Option<Command> {
    match event {
        Event::Key(key_event) => command_from_key_event(key_event),
        Event::Paste(_) => Some(Command::Paste),
        _ => None,
    }
}

pub fn command_from_key_event(key_event: KeyEvent) -> Option<Command> {
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

    match code {
        KeyCode::Char(ch) if ctrl => {
            let lower = ch.to_ascii_lowercase();
            match lower {
                'q' if shift || ch.is_ascii_uppercase() => Some(Command::ForceQuit),
                'q' if alt => Some(Command::ForceQuit),
                'q' => Some(Command::Quit),
                'g' if shift || ch.is_ascii_uppercase() => Some(Command::ForceQuit),
                'g' => Some(Command::ForceQuit),
                's' => Some(Command::Save),
                't' => Some(Command::CycleTabWidth),
                'w' => Some(Command::ToggleWrap),
                'c' => Some(Command::Copy),
                'x' => Some(Command::Cut),
                'v' => Some(Command::Paste),
                'a' => Some(Command::SelectAll),
                'z' if shift || ch.is_ascii_uppercase() => Some(Command::Redo),
                'z' => Some(Command::Undo),
                'y' => Some(Command::Redo),
                _ => None,
            }
        }
        KeyCode::Char('q') if alt && shift => Some(Command::ForceQuit),
        KeyCode::Char('q') if alt => Some(Command::Quit),
        KeyCode::Char('Q') if alt => Some(Command::ForceQuit),
        KeyCode::Char(ch) if !alt => Some(Command::InsertChar(ch)),
        KeyCode::Tab => Some(Command::InsertChar('\t')),
        KeyCode::BackTab => Some(Command::OutdentSelection),
        KeyCode::Enter => Some(Command::NewLine),
        KeyCode::Backspace => Some(Command::Backspace),
        KeyCode::Delete => Some(Command::Delete),
        KeyCode::Esc => Some(Command::ForceQuit),
        KeyCode::F(10) => Some(Command::Quit),
        KeyCode::F(12) => Some(Command::ForceQuit),
        KeyCode::Left if ctrl => Some(Command::Move {
            direction: MoveCommand::WordLeft,
            extend: shift,
        }),
        KeyCode::Right if ctrl => Some(Command::Move {
            direction: MoveCommand::WordRight,
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

fn decode_ascii_control(ch: char) -> Option<char> {
    if ch.is_ascii() {
        let byte = ch as u8;
        if (1..=26).contains(&byte) {
            return Some((b'a' + (byte - 1)) as char);
        }
    }
    None
}
