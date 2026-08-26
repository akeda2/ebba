use std::path::PathBuf;

use clap::{Parser, ValueEnum};

use crate::document::encoding::ContentOverride;
use crate::document::format::LineEndingMode;

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum LineEnding {
    Lf,
    Crlf,
}

#[derive(Debug, Parser)]
#[command(
    name = "ebba",
    about = "EBBA - a minimal terminal editor for text data editing and binary viewing",
    after_help = "Key bindings:\n  Save: Ctrl+S\n  Help: Ctrl+H, Alt+H\n  Quit: Ctrl+Q, Alt+Q, F10\n  Force quit: Ctrl+Alt+Q, Alt+Shift+Q, Ctrl+Shift+Q, Ctrl+G, F12\n  Undo/Redo: Ctrl+Z, Ctrl+Y, Ctrl+Shift+Z\n  Clipboard: Ctrl+C, Ctrl+X, Ctrl+V, Ctrl+A\n  Toggle BOM: Ctrl+B, Alt+B, Ctrl+Shift+B\n  Toggle tab width: Ctrl+T\n  Toggle wrap: Ctrl+W\n  Toggle invisibles: Ctrl+K, Alt+I\n  Move cursor: Arrow keys, Home/End, Ctrl+Home/Ctrl+End, PageUp/PageDown\n  Extend selection: Shift+Arrow keys, Shift+PageUp/PageDown\n  Edit keys: Enter, Backspace, Delete, Tab, Shift+Tab\n\nExamples:\n  ebba README.md\n  ebba script.sh --wrap 80 --invisibles\n  ebba data.bin --binary"
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
    #[arg(
        long,
        conflicts_with = "binary",
        help = "Force text mode at startup"
    )]
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
        Self::parse()
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
}
