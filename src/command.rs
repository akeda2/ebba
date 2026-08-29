use thiserror::Error;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveCommand {
    Left,
    Right,
    Up,
    Down,
    LineStart,
    LineEnd,
    WordLeft,
    WordRight,
    DocumentStart,
    DocumentEnd,
    PageUp,
    PageDown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Quit,
    ForceQuit,
    Save,
    InsertChar(char),
    NewLine,
    Backspace,
    Delete,
    DeleteWordBackward,
    DeleteToLineStart,
    CycleTabWidth,
    ToggleHardTabs,
    ShowHelp,
    ToggleBom,
    ToggleWrap,
    ToggleInvisibles,
    ToggleSelectionMode,
    OutdentSelection,
    Copy,
    Cut,
    Paste,
    PasteText(String),
    SelectAll,
    Undo,
    Redo,
    Move {
        direction: MoveCommand,
        extend: bool,
    },
}

#[derive(Debug, Error)]
pub enum AppError {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Message(String),
}
