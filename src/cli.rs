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
#[command(name = "ebba", about = "A terminal editor for text and binary data")]
pub struct Cli {
    #[arg(value_name = "FILE")]
    pub file: PathBuf,
    #[arg(long)]
    pub encoding: Option<String>,
    #[arg(long, value_enum)]
    pub line_ending: Option<LineEnding>,
    #[arg(long, conflicts_with = "binary")]
    pub text: bool,
    #[arg(long, conflicts_with = "text")]
    pub binary: bool,
    #[arg(long)]
    pub config: Option<PathBuf>,
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
