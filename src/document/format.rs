#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEnding {
    Lf,
    Cr,
    Crlf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineEndingMode {
    #[default]
    Preserve,
    Lf,
    Cr,
    Crlf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LineEndingStats {
    pub lf_count: usize,
    pub cr_count: usize,
    pub crlf_count: usize,
}

impl LineEndingStats {
    pub fn dominant(&self) -> Option<LineEnding> {
        let mut best: Option<(LineEnding, usize)> = None;
        for (kind, count, priority) in [
            (LineEnding::Cr, self.cr_count, 0usize),
            (LineEnding::Lf, self.lf_count, 1usize),
            (LineEnding::Crlf, self.crlf_count, 2usize),
        ] {
            if count == 0 {
                continue;
            }
            match best {
                None => best = Some((kind, count)),
                Some((_, best_count)) if count > best_count => best = Some((kind, count)),
                Some((existing, best_count))
                    if count == best_count
                        && priority
                            > match existing {
                                LineEnding::Cr => 0,
                                LineEnding::Lf => 1,
                                LineEnding::Crlf => 2,
                            } =>
                {
                    best = Some((kind, count))
                }
                _ => {}
            }
        }
        best.map(|(kind, _)| kind)
    }

    pub fn has_mixed_endings(&self) -> bool {
        let present = usize::from(self.lf_count > 0)
            + usize::from(self.cr_count > 0)
            + usize::from(self.crlf_count > 0);
        present > 1
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEndingIndicator {
    Lf,
    Cr,
    Crlf,
    Mixed,
    None,
}

impl LineEndingIndicator {
    pub fn label(self) -> &'static str {
        match self {
            Self::Lf => "LF",
            Self::Cr => "CR",
            Self::Crlf => "CRLF",
            Self::Mixed => "MIXED",
            Self::None => "NONE",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineEndingMetadata {
    pub mode: LineEndingMode,
    pub stats: LineEndingStats,
}

impl LineEndingMetadata {
    pub fn apply_saved_mode(&mut self, mode: LineEndingMode) {
        self.mode = mode;
        match mode {
            LineEndingMode::Preserve => {}
            LineEndingMode::Lf => {
                self.stats = LineEndingStats {
                    lf_count: 1,
                    cr_count: 0,
                    crlf_count: 0,
                };
            }
            LineEndingMode::Cr => {
                self.stats = LineEndingStats {
                    lf_count: 0,
                    cr_count: 1,
                    crlf_count: 0,
                };
            }
            LineEndingMode::Crlf => {
                self.stats = LineEndingStats {
                    lf_count: 0,
                    cr_count: 0,
                    crlf_count: 1,
                };
            }
        }
    }

    pub fn indicator(&self) -> LineEndingIndicator {
        if self.has_mixed_endings() {
            return LineEndingIndicator::Mixed;
        }
        match self.detected() {
            Some(LineEnding::Lf) => LineEndingIndicator::Lf,
            Some(LineEnding::Cr) => LineEndingIndicator::Cr,
            Some(LineEnding::Crlf) => LineEndingIndicator::Crlf,
            None => LineEndingIndicator::None,
        }
    }

    pub fn detected(&self) -> Option<LineEnding> {
        self.stats.dominant()
    }

    pub fn has_mixed_endings(&self) -> bool {
        self.stats.has_mixed_endings()
    }

    pub fn effective_for_save(&self) -> Option<LineEnding> {
        match self.mode {
            LineEndingMode::Preserve => self.detected(),
            LineEndingMode::Lf => Some(LineEnding::Lf),
            LineEndingMode::Cr => Some(LineEnding::Cr),
            LineEndingMode::Crlf => Some(LineEnding::Crlf),
        }
    }
}

/// Returns whether `bytes[index]` is the byte at which a line ends: `\n`,
/// or a lone `\r` that is not immediately followed by `\n` (a `\r\n` pair is
/// terminated by its `\n`, not its `\r`). Used throughout cursor movement,
/// selection, and rendering so that `\n`, `\r\n`, and bare `\r` line endings
/// are all navigable/renderable as line breaks, regardless of the mix of
/// endings present in the document.
pub fn is_line_terminator(bytes: &[u8], index: usize) -> bool {
    match bytes.get(index) {
        Some(b'\n') => true,
        Some(b'\r') => bytes.get(index + 1) != Some(&b'\n'),
        _ => false,
    }
}

pub fn analyze_line_endings(bytes: &[u8], mode: LineEndingMode) -> LineEndingMetadata {
    let mut stats = LineEndingStats::default();
    let mut idx = 0;

    while idx < bytes.len() {
        match bytes[idx] {
            b'\r' if idx + 1 < bytes.len() && bytes[idx + 1] == b'\n' => {
                stats.crlf_count += 1;
                idx += 2;
            }
            b'\r' => {
                stats.cr_count += 1;
                idx += 1;
            }
            b'\n' => {
                stats.lf_count += 1;
                idx += 1;
            }
            _ => {
                idx += 1;
            }
        }
    }

    LineEndingMetadata { mode, stats }
}
