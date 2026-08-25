#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEnding {
    Lf,
    Crlf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineEndingMode {
    #[default]
    Preserve,
    Lf,
    Crlf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LineEndingStats {
    pub lf_count: usize,
    pub crlf_count: usize,
}

impl LineEndingStats {
    pub fn dominant(&self) -> Option<LineEnding> {
        match (self.lf_count > 0, self.crlf_count > 0) {
            (false, false) => None,
            (true, false) => Some(LineEnding::Lf),
            (false, true) => Some(LineEnding::Crlf),
            (true, true) => {
                if self.crlf_count >= self.lf_count {
                    Some(LineEnding::Crlf)
                } else {
                    Some(LineEnding::Lf)
                }
            }
        }
    }

    pub fn has_mixed_endings(&self) -> bool {
        self.lf_count > 0 && self.crlf_count > 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineEndingMetadata {
    pub mode: LineEndingMode,
    pub stats: LineEndingStats,
}

impl LineEndingMetadata {
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
            LineEndingMode::Crlf => Some(LineEnding::Crlf),
        }
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
