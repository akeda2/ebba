use std::io::stdout;

use crossterm::{
    cursor::{Hide, Show},
    event::{DisableBracketedPaste, EnableBracketedPaste},
    execute,
    terminal::{
        EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode, size,
    },
};

use crate::command::AppResult;

#[derive(Debug, Default)]
pub struct Terminal {
    pub width: u16,
    pub height: u16,
}

impl Terminal {
    pub fn new() -> AppResult<Self> {
        let (width, height) = size()?;
        Ok(Self { width, height })
    }
}

#[derive(Debug)]
pub struct TerminalModeGuard;

impl TerminalModeGuard {
    pub fn enter() -> AppResult<Self> {
        enable_raw_mode()?;
        execute!(stdout(), EnterAlternateScreen, EnableBracketedPaste, Hide)?;
        Ok(Self)
    }
}

impl Drop for TerminalModeGuard {
    fn drop(&mut self) {
        let _ = execute!(stdout(), Show, DisableBracketedPaste, LeaveAlternateScreen);
        let _ = disable_raw_mode();
    }
}
