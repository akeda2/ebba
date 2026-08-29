use std::{ffi::OsString, path::PathBuf};

use clap::{CommandFactory, FromArgMatches, Parser, ValueEnum};

use crate::document::encoding::ContentOverride;
use crate::document::format::LineEndingMode;
use crate::input::KeybindingProfile;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum LineEnding {
    Lf,
    Cr,
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
        short = 'e',
        long,
        help = "Save encoding override (utf-8, utf-8-bom, utf-16le-bom, utf-16be-bom)"
    )]
    pub encoding: Option<String>,
    #[arg(
        short = 'l',
        long,
        value_enum,
        help = "Force line endings on save: lf, cr, crlf (defaults to preserving existing endings)"
    )]
    pub line_ending: Option<LineEnding>,
    #[arg(
        short = 't',
        long,
        conflicts_with = "binary",
        help = "Force text mode at startup"
    )]
    pub text: bool,
    #[arg(
        short = 'b',
        long,
        conflicts_with = "text",
        help = "Force binary fallback mode (read-only hex view)"
    )]
    pub binary: bool,
    #[arg(
        short = 'w',
        long,
        num_args = 0..=1,
        value_name = "COLUMN",
        value_parser = parse_wrap_column,
        help = "Enable wrapping; optionally wrap at fixed text column (e.g. --wrap 80)"
    )]
    pub wrap: Option<Option<usize>>,
    #[arg(
        short = 'c',
        long,
        help = "Center wrapped text after the gutter and enable wrap (uses 80 columns when --wrap has no number)"
    )]
    pub center: bool,
    #[arg(
        short = 'i',
        long,
        help = "Show invisible characters (spaces as ·, LF as ␊, CRLF as ␍)"
    )]
    pub invisibles: bool,
    #[arg(
        long,
        value_parser = parse_tab_width,
        help = "Set tab width at startup (2, 4, or 8)"
    )]
    pub tab_width: Option<usize>,
    #[arg(long, help = "Render one frame and exit (non-interactive mode)")]
    pub render_once: bool,
    #[arg(
        long,
        default_value_t = 80,
        value_parser = clap::value_parser!(u16).range(1..),
        requires = "render_once",
        help = "Render width for --render-once"
    )]
    pub render_width: u16,
    #[arg(
        long,
        default_value_t = 24,
        value_parser = clap::value_parser!(u16).range(1..),
        requires = "render_once",
        help = "Render height for --render-once"
    )]
    pub render_height: u16,
    #[arg(
        long = "hard-tabs",
        help = "Start in hard-tabs mode (Tab inserts literal tab bytes)"
    )]
    pub hard_tabs: bool,
    #[arg(short = 'C', long, help = "Load YAML config from this path")]
    pub config: Option<PathBuf>,
    #[arg(
        short = 'k',
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

fn parse_tab_width(raw: &str) -> Result<usize, String> {
    let value = raw
        .parse::<usize>()
        .map_err(|_| format!("invalid tab width `{raw}`"))?;
    match value {
        2 | 4 | 8 => Ok(value),
        _ => Err("tab width must be one of: 2, 4, 8".to_string()),
    }
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
            Some(LineEnding::Cr) => LineEndingMode::Cr,
            Some(LineEnding::Crlf) => LineEndingMode::Crlf,
        }
    }

    pub fn keybinding_profile(&self) -> KeybindingProfile {
        keymap_mode_to_profile(self.keymap)
    }

    fn key_bindings_help(profile: KeybindingProfile) -> &'static str {
        match profile {
            KeybindingProfile::MacOs => {
                "Key bindings:\n  Save: ⌘S, Ctrl+S\n  Help: ⇧⌘?, Ctrl+H\n  Quit: ⌘Q, Ctrl+Q, F10\n  Force quit: Ctrl+Alt+Q, Ctrl+Shift+Q, Ctrl+G, F12\n  Undo/Redo: ⌘Z, ⇧⌘Z, Ctrl+Y\n  Clipboard: ⌘C, ⌘X, ⌘V, Ctrl+C, Ctrl+X, Ctrl+V, Ctrl+Shift+C, Ctrl+Shift+V, ⌘A\n  Toggle BOM: Ctrl+B\n  Toggle tab width: Ctrl+T\n  Toggle hard tabs: Ctrl+Shift+T, Ctrl+Alt+T, Ctrl+Shift+H, F4\n  Toggle wrap: Ctrl+W\n  Toggle invisibles: Ctrl+K\n  Move cursor: Arrow keys, Home/End, ⌥+←/→, ⌘+←/→, ⌘+↑/↓, Ctrl+Home/Ctrl+End, PageUp/PageDown\n  Select: Shift+Arrow keys, Shift+PageUp/PageDown, Shift+⌥+←/→, Shift+⌘+←/→, Shift+⌘+↑/↓\n  Toggle selection mode: F3, Ctrl+Space\n  Edit keys: Enter, Backspace, Delete, ⌥Backspace, ⌘Backspace, Ctrl+Backspace, Ctrl+U, Tab, Shift+Tab\n\nExamples:\n  ebba README.md\n  ebba script.sh -w 80 -c -i\n  ebba data.bin -b"
            }
            KeybindingProfile::Linux => {
                "Key bindings:\n  Save: Ctrl+S\n  Help: Ctrl+H, Alt+H\n  Quit: Ctrl+Q, Alt+Q, F10\n  Force quit: Ctrl+Alt+Q, Alt+Shift+Q, Ctrl+Shift+Q, Ctrl+G, F12\n  Undo/Redo: Ctrl+Z, Ctrl+Y, Ctrl+Shift+Z\n  Clipboard: Ctrl+C, Ctrl+X, Ctrl+V, Ctrl+Shift+C, Ctrl+Shift+V, Ctrl+A\n  Toggle BOM: Ctrl+B, Alt+B, Ctrl+Shift+B\n  Toggle tab width: Ctrl+T\n  Toggle hard tabs: Ctrl+Shift+T, Ctrl+Alt+T, Alt+Shift+T, Ctrl+Shift+H, F4\n  Toggle wrap: Ctrl+W\n  Toggle invisibles: Ctrl+K, Alt+I\n  Move cursor: Arrow keys, Home/End, Ctrl+←/→, Ctrl+Home/Ctrl+End, PageUp/PageDown\n  Select: Shift+Arrow keys, Shift+PageUp/PageDown\n  Toggle selection mode: F3, Ctrl+Space\n  Edit keys: Enter, Backspace, Delete, Ctrl+Backspace, Ctrl+U, Tab, Shift+Tab\n\nExamples:\n  ebba README.md\n  ebba script.sh -w 80 -c -i\n  ebba data.bin -b"
            }
            KeybindingProfile::LinuxConsole => {
                "Key bindings:\n  Save: F2, Ctrl+S\n  Help: F1, Alt+H (Ctrl+H terminal-dependent)\n  Quit: F10, Ctrl+Q\n  Force quit: F12, Ctrl+Alt+Q, Ctrl+Shift+Q, Ctrl+G\n  Undo/Redo: Ctrl+Z, Ctrl+Y, Ctrl+Shift+Z\n  Clipboard: Ctrl+C, Ctrl+X, Ctrl+V, Ctrl+A (Ctrl+C/X copy/cut whole line on caret)\n  Toggle selection mode: F3, Ctrl+Space\n  Toggle BOM: Ctrl+B\n  Toggle tab width: Ctrl+T\n  Toggle hard tabs: Ctrl+Shift+T, Ctrl+Alt+T, Alt+Shift+T, Ctrl+Shift+H, F4\n  Toggle wrap: Ctrl+W\n  Toggle invisibles: Ctrl+K\n  Move cursor: Arrow keys, Home/End, Ctrl+←/→, Ctrl+Home/Ctrl+End, PageUp/PageDown\n  Select: Shift+Arrow keys, Shift+PageUp/PageDown, or selection mode + move keys\n  Edit keys: Enter, Backspace, Delete, Ctrl+Backspace, Ctrl+U, Tab, Shift+Tab\n\nExamples:\n  ebba README.md\n  ebba script.sh -w 80 -c -i\n  ebba data.bin -b"
            }
            KeybindingProfile::Windows => {
                "Key bindings:\n  Save: Ctrl+S\n  Help: F1, Ctrl+H\n  Quit: Ctrl+Q, F10\n  Force quit: Ctrl+Alt+Q, Ctrl+Shift+Q, Ctrl+G, F12\n  Undo/Redo: Ctrl+Z, Ctrl+Y, Ctrl+Shift+Z\n  Clipboard: Ctrl+C, Ctrl+X, Ctrl+V, Ctrl+Shift+C, Ctrl+Shift+V, Ctrl+A\n  Toggle BOM: Ctrl+B\n  Toggle tab width: Ctrl+T\n  Toggle hard tabs: Ctrl+Shift+T, Ctrl+Alt+T, Alt+Shift+T, Ctrl+Shift+H, F4\n  Toggle wrap: Ctrl+W\n  Toggle invisibles: Ctrl+K\n  Move cursor: Arrow keys, Home/End, Ctrl+←/→, Ctrl+Home/Ctrl+End, PageUp/PageDown\n  Select: Shift+Arrow keys, Shift+PageUp/PageDown\n  Toggle selection mode: F3, Ctrl+Space\n  Edit keys: Enter, Backspace, Delete, Ctrl+Backspace, Ctrl+U, Tab, Shift+Tab\n\nExamples:\n  ebba README.md\n  ebba script.sh -w 80 -c -i\n  ebba data.bin -b"
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
        if arg == "-k"
            && let Some(value) = args.next()
        {
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

    use clap::Parser;

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
            center: false,
            invisibles: false,
            tab_width: None,
            render_once: false,
            render_width: 80,
            render_height: 24,
            hard_tabs: false,
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

    #[test]
    fn parses_short_aliases() {
        let cli = Cli::try_parse_from([
            "ebba",
            "README.md",
            "-e",
            "utf-8",
            "-l",
            "lf",
            "-t",
            "-w",
            "80",
            "-c",
            "-i",
            "--tab-width",
            "8",
            "--hard-tabs",
            "-C",
            "config.yaml",
            "-k",
            "linux",
        ])
        .expect("short aliases should parse");

        assert_eq!(cli.encoding.as_deref(), Some("utf-8"));
        assert_eq!(cli.line_ending, Some(LineEnding::Lf));
        assert!(cli.text);
        assert_eq!(cli.wrap, Some(Some(80)));
        assert!(cli.center);
        assert!(cli.invisibles);
        assert_eq!(cli.tab_width, Some(8));
        assert!(cli.hard_tabs);
        assert_eq!(cli.config, Some(PathBuf::from("config.yaml")));
        assert_eq!(cli.keymap, KeymapMode::Linux);
    }

    #[test]
    fn parses_bare_wrap_with_center() {
        let cli = Cli::try_parse_from(["ebba", "README.md", "--wrap", "--center"])
            .expect("center with bare wrap should parse");
        assert_eq!(cli.wrap, Some(None));
        assert!(cli.center);
    }

    #[test]
    fn parses_center_without_wrap() {
        let cli = Cli::try_parse_from(["ebba", "README.md", "--center"])
            .expect("center should parse and imply wrapping at runtime");
        assert!(cli.center);
        assert_eq!(cli.wrap, None);
    }

    #[test]
    fn parses_cr_line_ending() {
        use crate::document::format::LineEndingMode;

        let cli = Cli::try_parse_from(["ebba", "README.md", "-l", "cr"])
            .expect("cr line ending should parse");
        assert_eq!(cli.line_ending, Some(LineEnding::Cr));
        assert_eq!(cli.line_ending_mode(), LineEndingMode::Cr);
    }

    #[test]
    fn parses_render_once_dimensions() {
        let cli = Cli::try_parse_from([
            "ebba",
            "README.md",
            "--render-once",
            "--render-width",
            "120",
            "--render-height",
            "40",
        ])
        .expect("render-once options should parse");
        assert!(cli.render_once);
        assert_eq!(cli.render_width, 120);
        assert_eq!(cli.render_height, 40);
    }

    #[test]
    fn rejects_unsupported_tab_width() {
        let parsed = Cli::try_parse_from(["ebba", "README.md", "--tab-width", "3"]);
        assert!(parsed.is_err());
    }
}
