use std::path::PathBuf;

use clap::{CommandFactory, FromArgMatches, Parser, ValueEnum};

use crate::document::encoding::ContentOverride;
use crate::document::format::LineEndingMode;
use crate::input::KeybindingProfile;

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum LineEnding {
    Lf,
    Crlf,
}

#[derive(Debug, Clone, Copy, ValueEnum, Default)]
pub enum KeymapMode {
    #[default]
    Auto,
    Mac,
    Linux,
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
        help = "Force keybinding profile for testing (auto, mac, linux)"
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
        let profile = KeybindingProfile::current();
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
        match self.keymap {
            KeymapMode::Auto => KeybindingProfile::current(),
            KeymapMode::Mac => KeybindingProfile::MacOs,
            KeymapMode::Linux => KeybindingProfile::Default,
        }
    }

    fn key_bindings_help(profile: KeybindingProfile) -> &'static str {
        match profile {
            KeybindingProfile::MacOs => {
                "Key bindings:\n  Save: ⌘S, Ctrl+S\n  Help: ⇧⌘?, Ctrl+H\n  Quit: ⌘Q, Ctrl+Q, F10\n  Force quit: Ctrl+Alt+Q, Ctrl+Shift+Q, Ctrl+G, F12\n  Undo/Redo: ⌘Z, ⇧⌘Z, Ctrl+Y\n  Clipboard: ⌘C, ⌘X, ⌘V, Ctrl+C, Ctrl+X, Ctrl+V, Ctrl+Shift+C, Ctrl+Shift+V, ⌘A\n  Toggle BOM: Ctrl+B\n  Toggle tab width: Ctrl+T\n  Toggle wrap: Ctrl+W\n  Toggle invisibles: Ctrl+K\n  Move cursor: Arrow keys, Home/End, ⌥+←/→, ⌘+←/→, ⌘+↑/↓, Ctrl+Home/Ctrl+End, PageUp/PageDown\n  Select: Shift+Arrow keys, Shift+PageUp/PageDown, Shift+⌥+←/→, Shift+⌘+←/→, Shift+⌘+↑/↓\n  Edit keys: Enter, Backspace, Delete, ⌥Backspace, ⌘Backspace, Ctrl+Backspace, Ctrl+U, Tab, Shift+Tab\n\nExamples:\n  ebba README.md\n  ebba script.sh --wrap 80 --invisibles\n  ebba data.bin --binary"
            }
            KeybindingProfile::Default => {
                "Key bindings:\n  Save: Ctrl+S\n  Help: Ctrl+H, Alt+H\n  Quit: Ctrl+Q, Alt+Q, F10\n  Force quit: Ctrl+Alt+Q, Alt+Shift+Q, Ctrl+Shift+Q, Ctrl+G, F12\n  Undo/Redo: Ctrl+Z, Ctrl+Y, Ctrl+Shift+Z\n  Clipboard: Ctrl+C, Ctrl+X, Ctrl+V, Ctrl+Shift+C, Ctrl+Shift+V, Ctrl+A\n  Toggle BOM: Ctrl+B, Alt+B, Ctrl+Shift+B\n  Toggle tab width: Ctrl+T\n  Toggle wrap: Ctrl+W\n  Toggle invisibles: Ctrl+K, Alt+I\n  Move cursor: Arrow keys, Home/End, Ctrl+←/→, Ctrl+Home/Ctrl+End, PageUp/PageDown\n  Select: Shift+Arrow keys, Shift+PageUp/PageDown\n  Edit keys: Enter, Backspace, Delete, Ctrl+Backspace, Ctrl+U, Tab, Shift+Tab\n\nExamples:\n  ebba README.md\n  ebba script.sh --wrap 80 --invisibles\n  ebba data.bin --binary"
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{Cli, KeymapMode, LineEnding};
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
        assert_eq!(cli.keybinding_profile(), KeybindingProfile::Default);
    }
}
