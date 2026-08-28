use std::cmp::{max, min};
use std::io::Write;

use thiserror::Error;
use unicode_segmentation::UnicodeSegmentation;

use crate::document::format::is_line_terminator;
use crate::document::source::{FileSourceRange, Source};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Piece {
    pub source_index: usize,
    pub source_start: usize,
    pub len: usize,
    pub line_feeds: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PieceMetadata {
    pub byte_start: usize,
    pub byte_end: usize,
    pub len: usize,
    pub line_feeds: Option<usize>,
}

#[derive(Debug, Error)]
pub enum PieceTreeError {
    #[error("range [{start}, {end}) is out of bounds for document length {doc_len}")]
    OutOfBounds {
        start: usize,
        end: usize,
        doc_len: usize,
    },
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Default)]
pub struct PieceTree {
    sources: Vec<Source>,
    pieces: Vec<Piece>,
    len: usize,
    revision: u64,
}

impl PieceTree {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        if bytes.is_empty() {
            return Self::new();
        }

        let source = Source::from_bytes(bytes);
        let len = source.len();
        let line_feeds = source.line_feed_count();

        Self {
            sources: vec![source],
            pieces: vec![Piece {
                source_index: 0,
                source_start: 0,
                len,
                line_feeds,
            }],
            len,
            revision: 0,
        }
    }

    pub fn from_file_ranges(ranges: impl IntoIterator<Item = FileSourceRange>) -> Self {
        let mut tree = Self::new();
        for range in ranges {
            let source = Source::from_file_range(range);
            tree.push_source_as_piece(source);
        }
        tree
    }

    pub fn from_sources(sources: impl IntoIterator<Item = Source>) -> Self {
        let mut tree = Self::new();
        for source in sources {
            tree.push_source_as_piece(source);
        }
        tree
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn piece_count(&self) -> usize {
        self.pieces.len()
    }

    pub fn source_count(&self) -> usize {
        self.sources.len()
    }

    pub fn source(&self, index: usize) -> Option<&Source> {
        self.sources.get(index)
    }

    pub fn line_feed_count_exact(&self) -> Option<usize> {
        self.pieces
            .iter()
            .try_fold(0usize, |acc, piece| piece.line_feeds.map(|lf| acc + lf))
    }

    pub fn known_line_feed_count(&self) -> usize {
        self.pieces
            .iter()
            .filter_map(|piece| piece.line_feeds)
            .sum()
    }

    pub fn piece_metadata(&self) -> Vec<PieceMetadata> {
        let mut byte_cursor = 0usize;
        self.pieces
            .iter()
            .map(|piece| {
                let start = byte_cursor;
                byte_cursor += piece.len;
                PieceMetadata {
                    byte_start: start,
                    byte_end: byte_cursor,
                    len: piece.len,
                    line_feeds: piece.line_feeds,
                }
            })
            .collect()
    }

    pub fn insert(&mut self, at: usize, bytes: Vec<u8>) -> Result<(), PieceTreeError> {
        if at > self.len {
            return Err(PieceTreeError::OutOfBounds {
                start: at,
                end: at,
                doc_len: self.len,
            });
        }
        if bytes.is_empty() {
            return Ok(());
        }

        let source_index = self.sources.len();
        let source = Source::from_bytes(bytes);
        let source_len = source.len();
        let line_feeds = source.line_feed_count();
        self.sources.push(source);

        let insert_piece = Piece {
            source_index,
            source_start: 0,
            len: source_len,
            line_feeds,
        };

        if self.pieces.is_empty() {
            self.pieces.push(insert_piece);
            self.len = source_len;
            self.revision += 1;
            return Ok(());
        }

        let insertion_index = self.split_at(at)?;
        self.pieces.insert(insertion_index, insert_piece);
        self.len += source_len;
        self.revision += 1;
        self.normalize();
        Ok(())
    }

    pub fn delete(&mut self, start: usize, len: usize) -> Result<(), PieceTreeError> {
        if len == 0 {
            return Ok(());
        }
        let end = start.saturating_add(len);
        if start > self.len || end > self.len {
            return Err(PieceTreeError::OutOfBounds {
                start,
                end,
                doc_len: self.len,
            });
        }

        let begin_index = self.split_at(start)?;
        let end_index = self.split_at(end)?;
        if end_index > begin_index {
            self.pieces.drain(begin_index..end_index);
        }
        self.len -= len;
        self.revision += 1;
        self.normalize();
        Ok(())
    }

    pub fn replace(
        &mut self,
        start: usize,
        delete_len: usize,
        insert_bytes: Vec<u8>,
    ) -> Result<(), PieceTreeError> {
        self.delete(start, delete_len)?;
        self.insert(start, insert_bytes)?;
        Ok(())
    }

    pub fn read_range(&self, start: usize, len: usize) -> Result<Vec<u8>, PieceTreeError> {
        let mut out = Vec::with_capacity(len);
        self.write_range(start, len, &mut out)?;
        Ok(out)
    }

    pub fn read_all(&self) -> Result<Vec<u8>, PieceTreeError> {
        self.read_range(0, self.len)
    }

    pub fn replace_all(&mut self, bytes: Vec<u8>) -> Result<(), PieceTreeError> {
        self.replace(0, self.len, bytes)
    }

    pub fn previous_grapheme_offset(&self, offset: usize) -> Result<usize, PieceTreeError> {
        let offset = offset.min(self.len);
        let bytes = self.read_all()?;
        let Some(text) = std::str::from_utf8(&bytes).ok() else {
            return Ok(offset.saturating_sub(1));
        };

        let offset = nearest_char_boundary_left(text, offset);
        if offset == 0 {
            return Ok(0);
        }

        let mut previous = 0usize;
        for (index, _) in text.grapheme_indices(true) {
            if index >= offset {
                break;
            }
            previous = index;
        }
        Ok(previous)
    }

    pub fn next_grapheme_offset(&self, offset: usize) -> Result<usize, PieceTreeError> {
        let offset = offset.min(self.len);
        let bytes = self.read_all()?;
        let Some(text) = std::str::from_utf8(&bytes).ok() else {
            return Ok((offset + 1).min(self.len));
        };

        let offset = nearest_char_boundary_left(text, offset);
        if offset >= text.len() {
            return Ok(text.len());
        }

        let tail = &text[offset..];
        if let Some(grapheme) = tail.graphemes(true).next() {
            return Ok((offset + grapheme.len()).min(text.len()));
        }
        Ok(text.len())
    }

    pub fn line_start_offset(&self, offset: usize) -> Result<usize, PieceTreeError> {
        let offset = offset.min(self.len);
        let bytes = self.read_all()?;
        let mut cursor = offset;
        while cursor > 0 {
            if is_line_terminator(&bytes, cursor - 1) {
                break;
            }
            cursor -= 1;
        }
        Ok(cursor)
    }

    pub fn line_end_offset(&self, offset: usize) -> Result<usize, PieceTreeError> {
        let offset = offset.min(self.len);
        let bytes = self.read_all()?;
        let mut cursor = offset;
        while cursor < bytes.len() && !is_line_terminator(&bytes, cursor) {
            cursor += 1;
        }
        Ok(cursor)
    }

    pub fn line_column(&self, offset: usize) -> Result<usize, PieceTreeError> {
        let offset = offset.min(self.len);
        let bytes = self.read_all()?;
        let mut line_start = offset;
        while line_start > 0 && !is_line_terminator(&bytes, line_start - 1) {
            line_start -= 1;
        }
        let slice = &bytes[line_start..offset];
        Ok(grapheme_count_or_bytes(slice))
    }

    pub fn offset_for_line_column(
        &self,
        line_start: usize,
        line_end: usize,
        column: usize,
    ) -> Result<usize, PieceTreeError> {
        let bytes = self.read_all()?;
        let line_start = line_start.min(bytes.len());
        let line_end = line_end.min(bytes.len()).max(line_start);
        let line = &bytes[line_start..line_end];
        let relative = byte_offset_for_grapheme_column(line, column);
        Ok(line_start + relative)
    }

    pub fn move_vertical(
        &self,
        offset: usize,
        direction: VerticalDirection,
        preferred_column: usize,
    ) -> Result<usize, PieceTreeError> {
        let offset = offset.min(self.len);
        let bytes = self.read_all()?;
        let line_start = self.line_start_offset(offset)?;
        let line_end = self.line_end_offset(offset)?;

        let (target_line_start, target_line_end) = match direction {
            VerticalDirection::Up => {
                if line_start == 0 {
                    return Ok(0);
                }
                let prev_end = line_start - 1;
                let mut prev_start = prev_end;
                while prev_start > 0 && !is_line_terminator(&bytes, prev_start - 1) {
                    prev_start -= 1;
                }
                (prev_start, prev_end)
            }
            VerticalDirection::Down => {
                if line_end >= bytes.len() || !is_line_terminator(&bytes, line_end) {
                    return Ok(bytes.len());
                }
                let next_start = line_end + 1;
                let mut next_end = next_start;
                while next_end < bytes.len() && !is_line_terminator(&bytes, next_end) {
                    next_end += 1;
                }
                (next_start, next_end)
            }
        };

        self.offset_for_line_column(target_line_start, target_line_end, preferred_column)
    }

    pub fn write_range(
        &self,
        start: usize,
        len: usize,
        writer: &mut impl Write,
    ) -> Result<(), PieceTreeError> {
        let end = start.saturating_add(len);
        if start > self.len || end > self.len {
            return Err(PieceTreeError::OutOfBounds {
                start,
                end,
                doc_len: self.len,
            });
        }
        if len == 0 {
            return Ok(());
        }

        let mut doc_pos = 0usize;
        let mut remaining = len;
        for piece in &self.pieces {
            if remaining == 0 {
                break;
            }

            let piece_start = doc_pos;
            let piece_end = doc_pos + piece.len;
            doc_pos = piece_end;

            if piece_end <= start || piece_start >= end {
                continue;
            }

            let overlap_start = max(piece_start, start);
            let overlap_end = min(piece_end, end);
            let overlap_len = overlap_end - overlap_start;
            let source_offset = piece.source_start + (overlap_start - piece_start);

            let bytes = self.sources[piece.source_index].read_range(source_offset, overlap_len)?;
            writer.write_all(&bytes)?;
            remaining -= overlap_len;
        }
        Ok(())
    }

    fn push_source_as_piece(&mut self, source: Source) {
        if source.is_empty() {
            return;
        }

        let source_index = self.sources.len();
        let len = source.len();
        let line_feeds = source.line_feed_count();
        self.sources.push(source);
        self.pieces.push(Piece {
            source_index,
            source_start: 0,
            len,
            line_feeds,
        });
        self.len += len;
    }

    fn split_at(&mut self, at: usize) -> Result<usize, PieceTreeError> {
        if at > self.len {
            return Err(PieceTreeError::OutOfBounds {
                start: at,
                end: at,
                doc_len: self.len,
            });
        }
        if at == self.len {
            return Ok(self.pieces.len());
        }

        let mut doc_pos = 0usize;
        for index in 0..self.pieces.len() {
            let piece = self.pieces[index].clone();
            let piece_start = doc_pos;
            let piece_end = doc_pos + piece.len;
            doc_pos = piece_end;

            if at < piece_start || at > piece_end {
                continue;
            }
            if at == piece_start {
                return Ok(index);
            }
            if at == piece_end {
                return Ok(index + 1);
            }

            let left_len = at - piece_start;
            let right_len = piece_end - at;

            let left = self.slice_piece(&piece, 0, left_len);
            let right = self.slice_piece(&piece, left_len, right_len);
            self.pieces[index] = left;
            self.pieces.insert(index + 1, right);
            return Ok(index + 1);
        }

        Ok(self.pieces.len())
    }

    fn slice_piece(&self, piece: &Piece, offset: usize, len: usize) -> Piece {
        let source = &self.sources[piece.source_index];
        let source_start = piece.source_start + offset;
        let line_feeds = source.in_memory_slice_line_feeds(source_start, len);
        Piece {
            source_index: piece.source_index,
            source_start,
            len,
            line_feeds,
        }
    }

    fn normalize(&mut self) {
        self.pieces.retain(|piece| piece.len > 0);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerticalDirection {
    Up,
    Down,
}

fn nearest_char_boundary_left(text: &str, mut offset: usize) -> usize {
    offset = offset.min(text.len());
    while offset > 0 && !text.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

fn grapheme_count_or_bytes(bytes: &[u8]) -> usize {
    if let Ok(text) = std::str::from_utf8(bytes) {
        text.graphemes(true).count()
    } else {
        bytes.len()
    }
}

fn byte_offset_for_grapheme_column(line: &[u8], column: usize) -> usize {
    let Ok(text) = std::str::from_utf8(line) else {
        return column.min(line.len());
    };

    let mut grapheme_offsets = text.grapheme_indices(true).map(|(index, _)| index);
    for _ in 0..column {
        if grapheme_offsets.next().is_none() {
            return text.len();
        }
    }
    grapheme_offsets.next().unwrap_or(text.len())
}
