use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use thiserror::Error;

use crate::document::encoding::DetectedEncoding;
use crate::document::format::LineEndingMode;
use crate::document::piece_tree::{PieceTree, PieceTreeError};

const STREAM_CHUNK_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveEncoding {
    PreserveBytes,
    Utf8,
    Utf8Bom,
    Utf16LeBom,
    Utf16BeBom,
}

impl std::fmt::Display for SaveEncoding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            Self::PreserveBytes => "preserve-bytes",
            Self::Utf8 => "utf-8",
            Self::Utf8Bom => "utf-8-bom",
            Self::Utf16LeBom => "utf-16le-bom",
            Self::Utf16BeBom => "utf-16be-bom",
        };
        f.write_str(label)
    }
}

impl SaveEncoding {
    pub fn from_detected(encoding: DetectedEncoding) -> Self {
        match encoding {
            DetectedEncoding::Utf8 => Self::Utf8,
            DetectedEncoding::Utf8Bom => Self::Utf8Bom,
            DetectedEncoding::Utf16LeBom => Self::Utf16LeBom,
            DetectedEncoding::Utf16BeBom => Self::Utf16BeBom,
            DetectedEncoding::Unknown8Bit => Self::PreserveBytes,
        }
    }

    pub fn parse(label: &str) -> Option<Self> {
        match label.trim().to_ascii_lowercase().as_str() {
            "utf8" | "utf-8" => Some(Self::Utf8),
            "utf8bom" | "utf-8-bom" => Some(Self::Utf8Bom),
            "utf16le" | "utf-16le" | "utf16le-bom" | "utf-16le-bom" => Some(Self::Utf16LeBom),
            "utf16be" | "utf-16be" | "utf16be-bom" | "utf-16be-bom" => Some(Self::Utf16BeBom),
            _ => None,
        }
    }

    pub fn detected_encoding(self) -> DetectedEncoding {
        match self {
            Self::PreserveBytes => DetectedEncoding::Unknown8Bit,
            Self::Utf8 => DetectedEncoding::Utf8,
            Self::Utf8Bom => DetectedEncoding::Utf8Bom,
            Self::Utf16LeBom => DetectedEncoding::Utf16LeBom,
            Self::Utf16BeBom => DetectedEncoding::Utf16BeBom,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SaveOverrides {
    pub encoding: Option<SaveEncoding>,
    pub line_ending_mode: Option<LineEndingMode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SaveOutcome {
    pub encoding: SaveEncoding,
    pub line_ending_mode: LineEndingMode,
}

#[derive(Debug, Error)]
pub enum SaveError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    PieceTree(#[from] PieceTreeError),
    #[error("save path is not set")]
    MissingPath,
    #[error("unsupported encoding override `{label}`")]
    UnsupportedEncoding { label: String },
    #[error("document bytes are not valid UTF-8 for `{target}` conversion")]
    InvalidUtf8ForConversion { target: SaveEncoding },
}

pub fn save_piece_tree_atomic(
    tree: &PieceTree,
    destination: &Path,
    detected_encoding: DetectedEncoding,
    detected_line_endings_mode: LineEndingMode,
    overrides: SaveOverrides,
) -> Result<SaveOutcome, SaveError> {
    let encoding = overrides
        .encoding
        .unwrap_or_else(|| SaveEncoding::from_detected(detected_encoding));
    let line_ending_mode = overrides
        .line_ending_mode
        .unwrap_or(detected_line_endings_mode);

    let parent = destination
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let temp_path = unique_temp_path(
        parent,
        destination.file_name().and_then(|name| name.to_str()),
    );

    let write_result = (|| -> Result<(), SaveError> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)?;
        stream_write(tree, &mut file, encoding, line_ending_mode)?;
        file.flush()?;
        file.sync_all()?;
        drop(file);
        if let Ok(metadata) = fs::metadata(destination) {
            fs::set_permissions(&temp_path, metadata.permissions())?;
        }
        fs::rename(&temp_path, destination)?;
        sync_parent_directory(parent)?;
        Ok(())
    })();

    if write_result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }

    write_result?;
    Ok(SaveOutcome {
        encoding,
        line_ending_mode,
    })
}

fn stream_write(
    tree: &PieceTree,
    writer: &mut File,
    encoding: SaveEncoding,
    line_ending_mode: LineEndingMode,
) -> Result<(), SaveError> {
    if encoding == SaveEncoding::PreserveBytes && line_ending_mode == LineEndingMode::Preserve {
        return stream_passthrough(tree, writer);
    }

    let mut offset = 0usize;
    let mut utf8_pending = Vec::new();
    let mut pending_carriage_return = false;
    let mut wrote_bom = false;

    while offset < tree.len() {
        let chunk_len = (tree.len() - offset).min(STREAM_CHUNK_BYTES);
        let chunk = tree.read_range(offset, chunk_len)?;
        offset += chunk_len;

        utf8_pending.extend_from_slice(&chunk);
        loop {
            match std::str::from_utf8(&utf8_pending) {
                Ok(valid) => {
                    let normalized = normalize_line_endings(
                        valid.as_bytes(),
                        line_ending_mode,
                        &mut pending_carriage_return,
                    );
                    encode_chunk(writer, &normalized, encoding, &mut wrote_bom)?;
                    utf8_pending.clear();
                    break;
                }
                Err(error) => {
                    if error.error_len().is_some() {
                        return Err(SaveError::InvalidUtf8ForConversion { target: encoding });
                    }

                    let valid_up_to = error.valid_up_to();
                    if valid_up_to == 0 {
                        break;
                    }

                    let normalized = normalize_line_endings(
                        &utf8_pending[..valid_up_to],
                        line_ending_mode,
                        &mut pending_carriage_return,
                    );
                    encode_chunk(writer, &normalized, encoding, &mut wrote_bom)?;
                    utf8_pending = utf8_pending.split_off(valid_up_to);
                }
            }
        }
    }

    if !utf8_pending.is_empty() {
        return Err(SaveError::InvalidUtf8ForConversion { target: encoding });
    }

    if pending_carriage_return {
        encode_chunk(writer, b"\r", encoding, &mut wrote_bom)?;
    }

    Ok(())
}

fn stream_passthrough(tree: &PieceTree, writer: &mut File) -> Result<(), SaveError> {
    let mut offset = 0usize;
    while offset < tree.len() {
        let chunk_len = (tree.len() - offset).min(STREAM_CHUNK_BYTES);
        let chunk = tree.read_range(offset, chunk_len)?;
        writer.write_all(&chunk)?;
        offset += chunk_len;
    }
    Ok(())
}

fn normalize_line_endings(
    input: &[u8],
    mode: LineEndingMode,
    pending_carriage_return: &mut bool,
) -> Vec<u8> {
    if mode == LineEndingMode::Preserve {
        return input.to_vec();
    }

    let mut output = Vec::with_capacity(input.len() + (input.len() / 8));
    let mut idx = 0usize;

    if *pending_carriage_return {
        if input.first() == Some(&b'\n') {
            match mode {
                LineEndingMode::Lf => output.push(b'\n'),
                LineEndingMode::Crlf => output.extend_from_slice(b"\r\n"),
                LineEndingMode::Preserve => output.extend_from_slice(b"\r\n"),
            }
            idx = 1;
        } else {
            output.push(b'\r');
        }
        *pending_carriage_return = false;
    }

    while idx < input.len() {
        match input[idx] {
            b'\r' => {
                if idx + 1 == input.len() {
                    *pending_carriage_return = true;
                    break;
                }
                if input[idx + 1] == b'\n' {
                    match mode {
                        LineEndingMode::Lf => output.push(b'\n'),
                        LineEndingMode::Crlf => output.extend_from_slice(b"\r\n"),
                        LineEndingMode::Preserve => output.extend_from_slice(b"\r\n"),
                    }
                    idx += 2;
                    continue;
                }
                output.push(b'\r');
                idx += 1;
            }
            b'\n' if mode == LineEndingMode::Crlf => {
                output.extend_from_slice(b"\r\n");
                idx += 1;
            }
            byte => {
                output.push(byte);
                idx += 1;
            }
        }
    }

    output
}

fn encode_chunk(
    writer: &mut File,
    utf8_bytes: &[u8],
    encoding: SaveEncoding,
    wrote_bom: &mut bool,
) -> Result<(), SaveError> {
    match encoding {
        SaveEncoding::PreserveBytes | SaveEncoding::Utf8 => {
            writer.write_all(utf8_bytes)?;
        }
        SaveEncoding::Utf8Bom => {
            if !*wrote_bom {
                writer.write_all(&[0xEF, 0xBB, 0xBF])?;
                *wrote_bom = true;
            }
            writer.write_all(utf8_bytes)?;
        }
        SaveEncoding::Utf16LeBom => {
            let text = std::str::from_utf8(utf8_bytes)
                .map_err(|_| SaveError::InvalidUtf8ForConversion { target: encoding })?;
            if !*wrote_bom {
                writer.write_all(&[0xFF, 0xFE])?;
                *wrote_bom = true;
            }
            let mut out = Vec::with_capacity(text.len() * 2 + 2);
            for code_unit in text.encode_utf16() {
                out.extend_from_slice(&code_unit.to_le_bytes());
            }
            writer.write_all(&out)?;
        }
        SaveEncoding::Utf16BeBom => {
            let text = std::str::from_utf8(utf8_bytes)
                .map_err(|_| SaveError::InvalidUtf8ForConversion { target: encoding })?;
            if !*wrote_bom {
                writer.write_all(&[0xFE, 0xFF])?;
                *wrote_bom = true;
            }
            let mut out = Vec::with_capacity(text.len() * 2 + 2);
            for code_unit in text.encode_utf16() {
                out.extend_from_slice(&code_unit.to_be_bytes());
            }
            writer.write_all(&out)?;
        }
    }
    Ok(())
}

fn unique_temp_path(parent: &Path, file_name_hint: Option<&str>) -> PathBuf {
    let hint = file_name_hint.unwrap_or("document");
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    parent.join(format!(
        ".{hint}.ebba-save-{}-{stamp}.tmp",
        std::process::id()
    ))
}

#[cfg(unix)]
fn sync_parent_directory(parent: &Path) -> Result<(), SaveError> {
    File::open(parent)?.sync_all()?;
    Ok(())
}

#[cfg(not(unix))]
fn sync_parent_directory(_parent: &Path) -> Result<(), SaveError> {
    Ok(())
}
