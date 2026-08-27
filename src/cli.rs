use std::{ffi::OsString, path::PathBuf};

use clap::{CommandFactory, FromArgMatches, Parser, ValueEnum};

use crate::document::encoding::ContentOverride;
use crate::document::format::LineEndingMode;
use crate::input::KeybindingProfile;

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum LineEnding {
    Lf,
    Crlf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
pub enum KeymapMode {
    #[default]
    Auto,
    Mac,
    Linux,
    LinuxConsole,
    Windows,
}

#[derive(Debug, Parser)]
#[command(
    name = "ebba",
    about = "EBBA - a minimal terminal editor for text data editing and binary viewing"
)]
pub struct Cli {
    #[arg(value_name = "FILE", help = "Path to the file to open")]
    pub file: PathBuf,
    #[arg(
        long,
        help = "Save encoding override (utf-8, utf-8-bom, utf-16le-bom, utf-16be-bom)"
    )]
    pub encoding: Option<String>,
    #[arg(
        long,
        value_enum,
        help = "Force line endings on save (defaults to preserving existing endings)"
    )]
    pub line_ending: Option<LineEnding>,
    #[arg(long, conflicts_with = "binary", help = "Force text mode at startup")]
    pub text: bool,
    #[arg(
        long,
        conflicts_with = "text",
        help = "Force binary fallback mode (read-only hex view)"
    )]
    pub binary: bool,
    #[arg(
        long,
        num_args = 0..=1,
        value_name = "COLUMN",
        value_parser = parse_wrap_column,
        help = "Enable wrapping; optionally wrap at fixed text column (e.g. --wrap 80)"
    )]
    pub wrap: Option<Option<usize>>,
    #[arg(
        long,
        help = "Show invisible characters (spaces as ·, LF as ␊, CRLF as ␍)"
    )]
    pub invisibles: bool,
    #[arg(long, help = "Load YAML config from this path")]
    pub config: Option<PathBuf>,
    #[arg(
        long,
        value_enum,
        default_value_t = KeymapMode::Auto,
        help = "Force keybinding profile for testing (auto, mac, linux, linux-console, windows)"
    )]
    pub keymap: KeymapMode,
}

fn parse_wrap_column(raw: &str) -> Result<usize, String> {
    let value = raw
        .parse::<usize>()
        .map_err(|_| format!("invalid wrap column `{raw}`"))?;
    if value == 0 {
        return Err("wrap column must be greater than 0".to_string());
    }
    Ok(value)
}

impl Cli {
    pub fn parse_args() -> Self {
        let profile = requested_keymap_from_args()
            .map(keymap_mode_to_profile)
            .unwrap_or_else(KeybindingProfile::current);
        let mut command = Self::command();
        command = command.after_help(Self::key_bindings_help(profile));
        let matches = command.get_matches();
        Self::from_arg_matches(&matches).unwrap_or_else(|error| error.exit())
    }

    pub fn content_override(&self) -> ContentOverride {
        if self.binary {
            ContentOverride::Binary
        } else if self.text {
            ContentOverride::Text
        } else {
            ContentOverride::Auto
        }
    }

    pub fn line_ending_mode(&self) -> LineEndingMode {
        match self.line_ending {
            None => LineEndingMode::Preserve,
            Some(LineEnding::Lf) => LineEndingMode::Lf,
            Some(LineEnding::Crlf) => LineEndingMode::Crlf,
        }
    }

    pub fn keybinding_profile(&self) -> KeybindingProfile {
        keymap_mode_to_profile(self.keymap)
    }

    fn key_bindings_help(profile: KeybindingProfile) -> &'static str {
        match profile {
            KeybindingProfile::MacOs => {
                "Key bindings:\n  Save: ⌘S, Ctrl+S\n  Help: ⇧⌘?, Ctrl+H\n  Quit: ⌘Q, Ctrl+Q, F10\n  Force quit: Ctrl+Alt+Q, Ctrl+Shift+Q, Ctrl+G, F12\n  Undo/Redo: ⌘Z, ⇧⌘Z, Ctrl+Y\n  Clipboard: ⌘C, ⌘X, ⌘V, Ctrl+C, Ctrl+X, Ctrl+V, Ctrl+Shift+C, Ctrl+Shift+V, ⌘A\n  Toggle BOM: Ctrl+B\n  Toggle tab width: Ctrl+T\n  Toggle wrap: Ctrl+W\n  Toggle invisibles: Ctrl+K\n  Move cursor: Arrow keys, Home/End, ⌥+←/→, ⌘+←/→, ⌘+↑/↓, Ctrl+Home/Ctrl+End, PageUp/PageDown\n  Select: Shift+Arrow keys, Shift+PageUp/PageDown, Shift+⌥+←/→, Shift+⌘+←/→, Shift+⌘+↑/↓\n  Edit keys: Enter, Backspace, Delete, ⌥Backspace, ⌘Backspace, Ctrl+Backspace, Ctrl+U, Tab, Shift+Tab\n\nExamples:\n  ebba README.md\n  ebba script.sh --wrap 80 --invisibles\n  ebba data.bin --binary"
            }
            KeybindingProfile::Linux => {
                "Key bindings:\n  Save: Ctrl+S\n  Help: Ctrl+H, Alt+H\n  Quit: Ctrl+Q, Alt+Q, F10\n  Force quit: Ctrl+Alt+Q, Alt+Shift+Q, Ctrl+Shift+Q, Ctrl+G, F12\n  Undo/Redo: Ctrl+Z, Ctrl+Y, Ctrl+Shift+Z\n  Clipboard: Ctrl+C, Ctrl+X, Ctrl+V, Ctrl+Shift+C, Ctrl+Shift+V, Ctrl+A\n  Toggle BOM: Ctrl+B, Alt+B, Ctrl+Shift+B\n  Toggle tab width: Ctrl+T\n  Toggle wrap: Ctrl+W\n  Toggle invisibles: Ctrl+K, Alt+I\n  Move cursor: Arrow keys, Home/End, Ctrl+←/→, Ctrl+Home/Ctrl+End, PageUp/PageDown\n  Select: Shift+Arrow keys, Shift+PageUp/PageDown\n  Edit keys: Enter, Backspace, Delete, Ctrl+Backspace, Ctrl+U, Tab, Shift+Tab\n\nExamples:\n  ebba README.md\n  ebba script.sh --wrap 80 --invisibles\n  ebba data.bin --binary"
            }
            KeybindingProfile::LinuxConsole => {
                "Key bindings:\n  Save: F2, Ctrl+S\n  Help: F1, Alt+H (Ctrl+H terminal-dependent)\n  Quit: F10, Ctrl+Q\n  Force quit: F12, Ctrl+Alt+Q, Ctrl+Shift+Q, Ctrl+G\n  Undo/Redo: Ctrl+Z, Ctrl+Y, Ctrl+Shift+Z\n  Clipboard: Ctrl+C, Ctrl+X, Ctrl+V, Ctrl+A (Ctrl+C/X copy/cut whole line on caret)\n  Toggle selection mode: F3, Ctrl+Space\n  Toggle BOM: Ctrl+B\n  Toggle tab width: Ctrl+T\n  Toggle wrap: Ctrl+W\n  Toggle invisibles: Ctrl+K\n  Move cursor: Arrow keys, Home/End, Ctrl+←/→, Ctrl+Home/Ctrl+End, PageUp/PageDown\n  Select: Shift+Arrow keys, Shift+PageUp/PageDown, or selection mode + move keys\n  Edit keys: Enter, Backspace, Delete, Ctrl+Backspace, Ctrl+U, Tab, Shift+Tab\n\nExamples:\n  ebba README.md\n  ebba script.sh --wrap 80 --invisibles\n  ebba data.bin --binary"
            }
            KeybindingProfile::Windows => {
                "Key bindings:\n  Save: Ctrl+S\n  Help: F1, Ctrl+H\n  Quit: Ctrl+Q, F10\n  Force quit: Ctrl+Alt+Q, Ctrl+Shift+Q, Ctrl+G, F12\n  Undo/Redo: Ctrl+Z, Ctrl+Y, Ctrl+Shift+Z\n  Clipboard: Ctrl+C, Ctrl+X, Ctrl+V, Ctrl+Shift+C, Ctrl+Shift+V, Ctrl+A\n  Toggle BOM: Ctrl+B\n  Toggle tab width: Ctrl+T\n  Toggle wrap: Ctrl+W\n  Toggle invisibles: Ctrl+K\n  Move cursor: Arrow keys, Home/End, Ctrl+←/→, Ctrl+Home/Ctrl+End, PageUp/PageDown\n  Select: Shift+Arrow keys, Shift+PageUp/PageDown\n  Edit keys: Enter, Backspace, Delete, Ctrl+Backspace, Ctrl+U, Tab, Shift+Tab\n\nExamples:\n  ebba README.md\n  ebba script.sh --wrap 80 --invisibles\n  ebba data.bin --binary"
            }
        }
    }
}

fn keymap_mode_to_profile(mode: KeymapMode) -> KeybindingProfile {
    match mode {
        KeymapMode::Auto => KeybindingProfile::current(),
        KeymapMode::Mac => KeybindingProfile::MacOs,
        KeymapMode::Linux => KeybindingProfile::Linux,
        KeymapMode::LinuxConsole => KeybindingProfile::LinuxConsole,
        KeymapMode::Windows => KeybindingProfile::Windows,
    }
}

fn requested_keymap_from_args() -> Option<KeymapMode> {
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        if let Some(value) = parse_keymap_arg(&arg) {
            return parse_keymap_mode(&value);
        }
        if arg == "--keymap"
            && let Some(value) = args.next()
        {
            return parse_keymap_mode(&value);
        }
    }
    None
}

fn parse_keymap_arg(arg: &OsString) -> Option<OsString> {
    let arg = arg.to_string_lossy();
    let prefix = "--keymap=";
    if arg.starts_with(prefix) {
        Some(OsString::from(&arg[prefix.len()..]))
    } else {
        None
    }
}

fn parse_keymap_mode(value: &OsString) -> Option<KeymapMode> {
    let value = value.to_string_lossy();
    if value.eq_ignore_ascii_case("auto") {
        Some(KeymapMode::Auto)
    } else if value.eq_ignore_ascii_case("mac") {
        Some(KeymapMode::Mac)
    } else if value.eq_ignore_ascii_case("linux") {
        Some(KeymapMode::Linux)
    } else if value.eq_ignore_ascii_case("linux-console")
        || value.eq_ignore_ascii_case("linux_console")
    {
        Some(KeymapMode::LinuxConsole)
    } else if value.eq_ignore_ascii_case("windows") {
        Some(KeymapMode::Windows)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use std::{ffi::OsString, path::PathBuf};

    use super::{Cli, KeymapMode, LineEnding, parse_keymap_mode};
    use crate::input::KeybindingProfile;

    fn base_cli() -> Cli {
        Cli {
            file: PathBuf::from("README.md"),
            encoding: None,
            line_ending: Some(LineEnding::Lf),
            text: false,
            binary: false,
            wrap: None,
            invisibles: false,
            config: None,
            keymap: KeymapMode::Auto,
        }
    }

    #[test]
    fn keybinding_profile_follows_explicit_keymap_switch() {
        let mut cli = base_cli();
        cli.keymap = KeymapMode::Mac;
        assert_eq!(cli.keybinding_profile(), KeybindingProfile::MacOs);

        cli.keymap = KeymapMode::Linux;
        assert_eq!(cli.keybinding_profile(), KeybindingProfile::Linux);

        cli.keymap = KeymapMode::LinuxConsole;
        assert_eq!(cli.keybinding_profile(), KeybindingProfile::LinuxConsole);

        cli.keymap = KeymapMode::Windows;
        assert_eq!(cli.keybinding_profile(), KeybindingProfile::Windows);
    }

    #[test]
    fn parses_explicit_keymap_modes() {
        assert_eq!(
            parse_keymap_mode(&OsString::from("linux-console")),
            Some(KeymapMode::LinuxConsole)
        );
        assert_eq!(
            parse_keymap_mode(&OsString::from("windows")),
            Some(KeymapMode::Windows)
        );
    }
}
