use crate::document::binary::{DEFAULT_BINARY_SCAN_LIMIT, inspect_binary};
use crate::document::format::{LineEndingMetadata, LineEndingMode, analyze_line_endings};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentOverride {
    Auto,
    Text,
    Binary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectedEncoding {
    Utf8,
    Utf8Bom,
    Utf16LeBom,
    Utf16BeBom,
    Unknown8Bit,
}

impl DetectedEncoding {
    pub fn name(self) -> &'static str {
        match self {
            Self::Utf8 => "utf-8",
            Self::Utf8Bom => "utf-8-bom",
            Self::Utf16LeBom => "utf-16le",
            Self::Utf16BeBom => "utf-16be",
            Self::Unknown8Bit => "unknown-8bit",
        }
    }

    pub fn is_non_resynchronizable_for_lazy_load(self) -> bool {
        matches!(self, Self::Utf16LeBom | Self::Utf16BeBom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartupContentMode {
    DecodedText,
    BytePreservingFallbackText,
    HexReadOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartupPayload {
    DecodedText { text: String, strip_utf8_bom: bool },
    BytePreservingText { bytes: Vec<u8> },
    HexReadOnly { bytes: Vec<u8> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartupPlan {
    pub mode: StartupContentMode,
    pub encoding: DetectedEncoding,
    pub payload: StartupPayload,
    pub line_endings: LineEndingMetadata,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmationReason {
    LargeFile {
        size_bytes: usize,
        threshold_bytes: usize,
    },
    NonResynchronizableEncoding {
        encoding: DetectedEncoding,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmationRequired {
    pub reasons: Vec<ConfirmationReason>,
    pub proposed: StartupPlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartupDecision {
    Ready(StartupPlan),
    RequiresConfirmation(ConfirmationRequired),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DetectionOptions {
    pub content_override: ContentOverride,
    pub line_ending_mode: LineEndingMode,
    pub binary_scan_limit: usize,
    pub large_file_threshold_bytes: Option<usize>,
    pub confirm_non_resynchronizable_encoding: bool,
}

impl Default for DetectionOptions {
    fn default() -> Self {
        Self {
            content_override: ContentOverride::Auto,
            line_ending_mode: LineEndingMode::Preserve,
            binary_scan_limit: DEFAULT_BINARY_SCAN_LIMIT,
            large_file_threshold_bytes: None,
            confirm_non_resynchronizable_encoding: true,
        }
    }
}

pub fn detect_startup_mode(bytes: &[u8], options: DetectionOptions) -> StartupDecision {
    let plan = match options.content_override {
        ContentOverride::Binary => hex_read_only_plan(bytes, options.line_ending_mode),
        ContentOverride::Text => {
            forced_text_plan(bytes, options.line_ending_mode, options.binary_scan_limit)
        }
        ContentOverride::Auto => auto_detect_plan(bytes, &options),
    };

    let mut reasons = Vec::new();
    if let Some(threshold) = options.large_file_threshold_bytes
        && bytes.len() > threshold
    {
        reasons.push(ConfirmationReason::LargeFile {
            size_bytes: bytes.len(),
            threshold_bytes: threshold,
        });
    }
    if options.confirm_non_resynchronizable_encoding
        && plan.encoding.is_non_resynchronizable_for_lazy_load()
    {
        reasons.push(ConfirmationReason::NonResynchronizableEncoding {
            encoding: plan.encoding,
        });
    }

    if reasons.is_empty() {
        StartupDecision::Ready(plan)
    } else {
        StartupDecision::RequiresConfirmation(ConfirmationRequired {
            reasons,
            proposed: plan,
        })
    }
}

fn auto_detect_plan(bytes: &[u8], options: &DetectionOptions) -> StartupPlan {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return utf8_bom_plan(bytes, options.line_ending_mode);
    }

    if bytes.starts_with(&[0xFF, 0xFE]) {
        return utf16_plan(bytes, true, options.line_ending_mode);
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        return utf16_plan(bytes, false, options.line_ending_mode);
    }

    let binary = inspect_binary(bytes, options.binary_scan_limit);
    if binary.is_binary_conservative() {
        return hex_read_only_plan(bytes, options.line_ending_mode);
    }

    if let Ok(text) = std::str::from_utf8(bytes) {
        return decoded_text_plan(
            DetectedEncoding::Utf8,
            text.to_string(),
            false,
            bytes,
            options.line_ending_mode,
        );
    }

    fallback_text_plan(bytes, options.line_ending_mode)
}

fn forced_text_plan(
    bytes: &[u8],
    line_ending_mode: LineEndingMode,
    binary_scan_limit: usize,
) -> StartupPlan {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return utf8_bom_plan(bytes, line_ending_mode);
    }
    if bytes.starts_with(&[0xFF, 0xFE]) {
        return utf16_plan(bytes, true, line_ending_mode);
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        return utf16_plan(bytes, false, line_ending_mode);
    }

    if inspect_binary(bytes, binary_scan_limit).is_binary_conservative() {
        return fallback_text_plan(bytes, line_ending_mode);
    }
    if let Ok(text) = std::str::from_utf8(bytes) {
        return decoded_text_plan(
            DetectedEncoding::Utf8,
            text.to_string(),
            false,
            bytes,
            line_ending_mode,
        );
    }

    fallback_text_plan(bytes, line_ending_mode)
}

fn utf8_bom_plan(bytes: &[u8], line_ending_mode: LineEndingMode) -> StartupPlan {
    let body = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    if let Ok(text) = std::str::from_utf8(body) {
        decoded_text_plan(
            DetectedEncoding::Utf8Bom,
            text.to_string(),
            true,
            body,
            line_ending_mode,
        )
    } else {
        fallback_text_plan(bytes, line_ending_mode)
    }
}

fn utf16_plan(bytes: &[u8], little_endian: bool, line_ending_mode: LineEndingMode) -> StartupPlan {
    if let Some(text) = decode_utf16_body(bytes, little_endian) {
        let encoding = if little_endian {
            DetectedEncoding::Utf16LeBom
        } else {
            DetectedEncoding::Utf16BeBom
        };
        decoded_text_plan(encoding, text, false, bytes, line_ending_mode)
    } else {
        fallback_text_plan(bytes, line_ending_mode)
    }
}

fn decoded_text_plan(
    encoding: DetectedEncoding,
    text: String,
    strip_utf8_bom: bool,
    line_ending_bytes: &[u8],
    line_ending_mode: LineEndingMode,
) -> StartupPlan {
    StartupPlan {
        mode: StartupContentMode::DecodedText,
        encoding,
        payload: StartupPayload::DecodedText {
            text,
            strip_utf8_bom,
        },
        line_endings: analyze_line_endings(line_ending_bytes, line_ending_mode),
    }
}

fn fallback_text_plan(bytes: &[u8], line_ending_mode: LineEndingMode) -> StartupPlan {
    StartupPlan {
        mode: StartupContentMode::BytePreservingFallbackText,
        encoding: DetectedEncoding::Unknown8Bit,
        payload: StartupPayload::BytePreservingText {
            bytes: bytes.to_vec(),
        },
        line_endings: analyze_line_endings(bytes, line_ending_mode),
    }
}

fn hex_read_only_plan(bytes: &[u8], line_ending_mode: LineEndingMode) -> StartupPlan {
    StartupPlan {
        mode: StartupContentMode::HexReadOnly,
        encoding: DetectedEncoding::Unknown8Bit,
        payload: StartupPayload::HexReadOnly {
            bytes: bytes.to_vec(),
        },
        line_endings: analyze_line_endings(bytes, line_ending_mode),
    }
}

fn decode_utf16_body(bytes: &[u8], little_endian: bool) -> Option<String> {
    let body = bytes.get(2..)?;
    if body.len() % 2 != 0 {
        return None;
    }

    let mut code_units = Vec::with_capacity(body.len() / 2);
    for chunk in body.chunks_exact(2) {
        let unit = if little_endian {
            u16::from_le_bytes([chunk[0], chunk[1]])
        } else {
            u16::from_be_bytes([chunk[0], chunk[1]])
        };
        code_units.push(unit);
    }

    String::from_utf16(&code_units).ok()
}
