use std::io::stdout;

use crossterm::{
    cursor::{Hide, Show},
    event::{
        DisableBracketedPaste, EnableBracketedPaste, KeyboardEnhancementFlags,
        PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    execute,
    terminal::{
        EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode, size,
        supports_keyboard_enhancement,
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
pub struct TerminalModeGuard {
    keyboard_enhancement_enabled: bool,
}

impl TerminalModeGuard {
    pub fn enter() -> AppResult<Self> {
        enable_raw_mode()?;
        execute!(stdout(), EnterAlternateScreen, EnableBracketedPaste, Hide)?;
        let keyboard_enhancement_enabled =
            supports_keyboard_enhancement().unwrap_or(false) && !is_linux_console_term();
        if keyboard_enhancement_enabled {
            execute!(
                stdout(),
                PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
            )?;
        }
        Ok(Self {
            keyboard_enhancement_enabled,
        })
    }
}

fn is_linux_console_term() -> bool {
    cfg!(target_os = "linux")
        && std::env::var("TERM")
            .ok()
            .is_some_and(|term| term.eq_ignore_ascii_case("linux"))
}

impl Drop for TerminalModeGuard {
    fn drop(&mut self) {
        if self.keyboard_enhancement_enabled {
            let _ = execute!(stdout(), PopKeyboardEnhancementFlags);
        }
        let _ = execute!(stdout(), Show, DisableBracketedPaste, LeaveAlternateScreen);
        let _ = disable_raw_mode();
    }
}
