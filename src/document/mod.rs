use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::document::cursor::Cursor;
use crate::document::encoding::DetectedEncoding;
use crate::document::format::{LineEndingMetadata, LineEndingMode, analyze_line_endings};
use crate::document::piece_tree::{PieceTree, PieceTreeError, VerticalDirection};
use crate::document::save::{SaveError, SaveOverrides, save_piece_tree_atomic};
use crate::document::selection::Selection;
use crate::document::transaction::{EditGroupKind, TransactionLog};

pub mod binary;
pub mod cursor;
pub mod encoding;
pub mod format;
pub mod line_index;
pub mod piece_tree;
pub mod save;
pub mod selection;
pub mod source;
pub mod transaction;

#[derive(Debug, Error)]
pub enum DocumentError {
    #[error(transparent)]
    PieceTree(#[from] PieceTreeError),
    #[error(transparent)]
    Save(#[from] SaveError),
}

#[derive(Debug)]
pub struct Document {
    tree: PieceTree,
    selection: Selection,
    clipboard: String,
    history: TransactionLog,
    preferred_column: Option<usize>,
    persistence: PersistenceState,
}

#[derive(Debug, Clone)]
struct PersistenceState {
    path: Option<PathBuf>,
    detected_encoding: DetectedEncoding,
    line_endings: LineEndingMetadata,
    dirty: bool,
}

impl Default for Document {
    fn default() -> Self {
        Self::from_bytes(Vec::new())
    }
}

impl Document {
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        let line_endings = analyze_line_endings(&bytes, LineEndingMode::Preserve);
        let tree = PieceTree::from_bytes(bytes);
        Self {
            selection: Selection::caret(0),
            tree,
            clipboard: String::new(),
            history: TransactionLog::default(),
            preferred_column: None,
            persistence: PersistenceState {
                path: None,
                detected_encoding: DetectedEncoding::Unknown8Bit,
                line_endings,
                dirty: false,
            },
        }
    }

    pub fn len(&self) -> usize {
        self.tree.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tree.is_empty()
    }

    pub fn selection(&self) -> Selection {
        self.selection
    }

    pub fn clipboard(&self) -> &str {
        &self.clipboard
    }

    pub fn bytes(&self) -> Result<Vec<u8>, DocumentError> {
        Ok(self.tree.read_all()?)
    }

    pub fn set_path(&mut self, path: impl Into<PathBuf>) {
        self.persistence.path = Some(path.into());
    }

    pub fn path(&self) -> Option<&Path> {
        self.persistence.path.as_deref()
    }

    pub fn detected_encoding(&self) -> DetectedEncoding {
        self.persistence.detected_encoding
    }

    pub fn line_endings(&self) -> &LineEndingMetadata {
        &self.persistence.line_endings
    }

    pub fn configure_save_metadata(
        &mut self,
        detected_encoding: DetectedEncoding,
        line_endings: LineEndingMetadata,
    ) {
        self.persistence.detected_encoding = detected_encoding;
        self.persistence.line_endings = line_endings;
    }

    pub fn is_dirty(&self) -> bool {
        self.persistence.dirty
    }

    pub fn can_quit(&self) -> bool {
        !self.persistence.dirty
    }

    pub fn save(&mut self, overrides: SaveOverrides) -> Result<(), DocumentError> {
        let path = self
            .persistence
            .path
            .clone()
            .ok_or(SaveError::MissingPath)?;
        let outcome = save_piece_tree_atomic(
            &self.tree,
            path.as_path(),
            self.persistence.detected_encoding,
            self.persistence.line_endings.mode,
            overrides,
        )?;
        self.persistence.detected_encoding = outcome.encoding.detected_encoding();
        self.persistence
            .line_endings
            .apply_saved_mode(outcome.line_ending_mode);
        self.persistence.dirty = false;
        Ok(())
    }

    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    pub fn flush_edit_group(&mut self) {
        self.history.flush_pending();
    }

    pub fn clear_selection(&mut self) {
        self.history.flush_pending();
        self.selection.collapse_to_active();
        self.preferred_column = None;
    }

    pub fn extend_selection_to(&mut self, byte_offset: usize) {
        self.history.flush_pending();
        let target = byte_offset.min(self.tree.len());
        self.selection.set_active(target);
        self.preferred_column = None;
    }

    pub fn move_to_byte_offset(&mut self, byte_offset: usize, extend: bool) {
        let target = byte_offset.min(self.tree.len());
        if extend {
            self.selection.set_active(target);
        } else {
            self.selection = Selection::caret(target);
        }
        self.preferred_column = None;
    }

    pub fn select_all(&mut self) {
        self.history.flush_pending();
        let end = self.tree.len();
        self.selection = Selection::new(Cursor::new(0), Cursor::new(end));
        self.preferred_column = None;
    }

    pub fn move_left(&mut self, extend: bool) -> Result<(), DocumentError> {
        self.history.flush_pending();
        self.preferred_column = None;

        if !extend && !self.selection.is_caret() {
            self.selection.collapse_to_start();
            return Ok(());
        }

        let target = self
            .tree
            .previous_grapheme_offset(self.selection.active.byte_offset)?;
        self.set_active_cursor(target, extend);
        Ok(())
    }

    pub fn move_right(&mut self, extend: bool) -> Result<(), DocumentError> {
        self.history.flush_pending();
        self.preferred_column = None;

        if !extend && !self.selection.is_caret() {
            self.selection.collapse_to_end();
            return Ok(());
        }

        let target = self
            .tree
            .next_grapheme_offset(self.selection.active.byte_offset)?;
        self.set_active_cursor(target, extend);
        Ok(())
    }

    pub fn move_up(&mut self, extend: bool) -> Result<(), DocumentError> {
        self.history.flush_pending();
        let column = match self.preferred_column {
            Some(column) => column,
            None => self.tree.line_column(self.selection.active.byte_offset)?,
        };

        let target = self.tree.move_vertical(
            self.selection.active.byte_offset,
            VerticalDirection::Up,
            column,
        )?;
        self.preferred_column = Some(column);
        self.set_active_cursor(target, extend);
        Ok(())
    }

    pub fn move_down(&mut self, extend: bool) -> Result<(), DocumentError> {
        self.history.flush_pending();
        let column = match self.preferred_column {
            Some(column) => column,
            None => self.tree.line_column(self.selection.active.byte_offset)?,
        };

        let target = self.tree.move_vertical(
            self.selection.active.byte_offset,
            VerticalDirection::Down,
            column,
        )?;
        self.preferred_column = Some(column);
        self.set_active_cursor(target, extend);
        Ok(())
    }

    pub fn move_page_up(&mut self, lines: usize, extend: bool) -> Result<(), DocumentError> {
        let steps = lines.max(1);
        for _ in 0..steps {
            let before = self.selection.active.byte_offset;
            self.move_up(extend)?;
            if self.selection.active.byte_offset == before {
                break;
            }
        }
        Ok(())
    }

    pub fn move_page_down(&mut self, lines: usize, extend: bool) -> Result<(), DocumentError> {
        let steps = lines.max(1);
        for _ in 0..steps {
            let before = self.selection.active.byte_offset;
            self.move_down(extend)?;
            if self.selection.active.byte_offset == before {
                break;
            }
        }
        Ok(())
    }

    pub fn move_line_start(&mut self, extend: bool) -> Result<(), DocumentError> {
        self.history.flush_pending();
        self.preferred_column = None;
        let target = self
            .tree
            .line_start_offset(self.selection.active.byte_offset)?;
        self.set_active_cursor(target, extend);
        Ok(())
    }

    pub fn move_line_end(&mut self, extend: bool) -> Result<(), DocumentError> {
        self.history.flush_pending();
        self.preferred_column = None;
        let target = self
            .tree
            .line_end_offset(self.selection.active.byte_offset)?;
        self.set_active_cursor(target, extend);
        Ok(())
    }

    pub fn move_document_start(&mut self, extend: bool) {
        self.history.flush_pending();
        self.preferred_column = None;
        self.set_active_cursor(0, extend);
    }

    pub fn move_document_end(&mut self, extend: bool) {
        self.history.flush_pending();
        self.preferred_column = None;
        self.set_active_cursor(self.tree.len(), extend);
    }

    pub fn insert_text(&mut self, text: &str) -> Result<(), DocumentError> {
        if text.is_empty() {
            return Ok(());
        }
        self.edit_replace_selection(text.as_bytes().to_vec(), EditGroupKind::Typing)
    }

    pub fn replace_selection(&mut self, text: &str) -> Result<(), DocumentError> {
        self.edit_replace_selection(text.as_bytes().to_vec(), EditGroupKind::Other)
    }

    pub fn delete_backward(&mut self) -> Result<(), DocumentError> {
        if !self.selection.is_caret() {
            return self.edit_replace_selection(Vec::new(), EditGroupKind::Other);
        }

        let caret = self.selection.active.byte_offset;
        if caret == 0 {
            return Ok(());
        }

        let start = self.tree.previous_grapheme_offset(caret)?;
        let before_bytes = self.tree.read_all()?;
        let before_selection = self.selection;
        self.tree.delete(start, caret - start)?;
        self.selection = Selection::caret(start);
        let after_bytes = self.tree.read_all()?;
        self.history.record_edit(
            EditGroupKind::Backspace,
            before_bytes,
            before_selection,
            after_bytes,
            self.selection,
        );
        self.preferred_column = None;
        self.persistence.dirty = true;
        Ok(())
    }

    pub fn delete_forward(&mut self) -> Result<(), DocumentError> {
        if !self.selection.is_caret() {
            return self.edit_replace_selection(Vec::new(), EditGroupKind::Other);
        }

        let caret = self.selection.active.byte_offset;
        if caret >= self.tree.len() {
            return Ok(());
        }

        let end = self.tree.next_grapheme_offset(caret)?;
        let before_bytes = self.tree.read_all()?;
        let before_selection = self.selection;
        self.tree.delete(caret, end - caret)?;
        self.selection = Selection::caret(caret);
        let after_bytes = self.tree.read_all()?;
        self.history.record_edit(
            EditGroupKind::DeleteForward,
            before_bytes,
            before_selection,
            after_bytes,
            self.selection,
        );
        self.preferred_column = None;
        self.persistence.dirty = true;
        Ok(())
    }

    pub fn delete_word_backward(&mut self) -> Result<(), DocumentError> {
        if !self.selection.is_caret() {
            return self.edit_replace_selection(Vec::new(), EditGroupKind::Other);
        }

        let caret = self.selection.active.byte_offset;
        if caret == 0 {
            return Ok(());
        }

        let before_bytes = self.tree.read_all()?;
        let before_selection = self.selection;
        let start = previous_word_start_offset(&before_bytes, caret);
        if start == caret {
            return Ok(());
        }

        self.tree.delete(start, caret - start)?;
        self.selection = Selection::caret(start);
        let after_bytes = self.tree.read_all()?;
        self.history.record_edit(
            EditGroupKind::Other,
            before_bytes,
            before_selection,
            after_bytes,
            self.selection,
        );
        self.preferred_column = None;
        self.persistence.dirty = true;
        Ok(())
    }

    pub fn delete_to_line_start(&mut self) -> Result<(), DocumentError> {
        if !self.selection.is_caret() {
            return self.edit_replace_selection(Vec::new(), EditGroupKind::Other);
        }

        let caret = self.selection.active.byte_offset;
        if caret == 0 {
            return Ok(());
        }

        let start = self.tree.line_start_offset(caret)?;
        if start == caret {
            return Ok(());
        }

        let before_bytes = self.tree.read_all()?;
        let before_selection = self.selection;
        self.tree.delete(start, caret - start)?;
        self.selection = Selection::caret(start);
        let after_bytes = self.tree.read_all()?;
        self.history.record_edit(
            EditGroupKind::Other,
            before_bytes,
            before_selection,
            after_bytes,
            self.selection,
        );
        self.preferred_column = None;
        self.persistence.dirty = true;
        Ok(())
    }

    pub fn copy_selection(&mut self) -> Result<bool, DocumentError> {
        let len = self.selection.len();
        if len == 0 {
            return Ok(false);
        }
        let bytes = self.tree.read_range(self.selection.start(), len)?;
        self.clipboard = String::from_utf8_lossy(&bytes).into_owned();
        Ok(true)
    }

    pub fn cut_selection(&mut self) -> Result<bool, DocumentError> {
        if !self.copy_selection()? {
            return Ok(false);
        }
        self.edit_replace_selection(Vec::new(), EditGroupKind::Other)?;
        Ok(true)
    }

    pub fn paste_clipboard(&mut self) -> Result<bool, DocumentError> {
        if self.clipboard.is_empty() {
            return Ok(false);
        }
        self.edit_replace_selection(self.clipboard.as_bytes().to_vec(), EditGroupKind::Typing)?;
        Ok(true)
    }

    pub fn indent_selection_lines(&mut self, tab_width: usize) -> Result<bool, DocumentError> {
        if self.selection.is_caret() || tab_width == 0 {
            return Ok(false);
        }

        let before_bytes = self.tree.read_all()?;
        let before_selection = self.selection;
        let indent = vec![b' '; tab_width];

        let sel_start = self.selection.start().min(before_bytes.len());
        let sel_end = self.selection.end().min(before_bytes.len());
        let scan_end = if sel_end > sel_start
            && sel_end > 0
            && before_bytes.get(sel_end - 1) == Some(&b'\n')
        {
            sel_end - 1
        } else {
            sel_end
        };
        let first_line_start = self.tree.line_start_offset(sel_start)?;

        let mut line_starts = vec![first_line_start];
        let mut cursor = first_line_start;
        while cursor < scan_end {
            if before_bytes[cursor] == b'\n' && cursor + 1 <= before_bytes.len() {
                line_starts.push(cursor + 1);
            }
            cursor += 1;
        }

        line_starts.sort_unstable();
        line_starts.dedup();

        let mut out = Vec::with_capacity(before_bytes.len() + line_starts.len() * tab_width);
        let mut prev = 0usize;
        for &line_start in &line_starts {
            out.extend_from_slice(&before_bytes[prev..line_start]);
            out.extend_from_slice(&indent);
            prev = line_start;
        }
        out.extend_from_slice(&before_bytes[prev..]);

        let anchor_shift = line_starts
            .iter()
            .filter(|&&pos| pos <= before_selection.anchor.byte_offset)
            .count()
            * tab_width;
        let active_shift = line_starts
            .iter()
            .filter(|&&pos| pos <= before_selection.active.byte_offset)
            .count()
            * tab_width;

        self.tree.replace_all(out)?;
        self.selection.anchor = Cursor::new(before_selection.anchor.byte_offset + anchor_shift);
        self.selection.active = Cursor::new(before_selection.active.byte_offset + active_shift);
        self.selection.clamp_to(self.tree.len());

        let after_bytes = self.tree.read_all()?;
        self.history.record_edit(
            EditGroupKind::Other,
            before_bytes,
            before_selection,
            after_bytes,
            self.selection,
        );
        self.preferred_column = None;
        self.persistence.dirty = true;

        Ok(true)
    }

    pub fn outdent_selection_lines(&mut self, tab_width: usize) -> Result<bool, DocumentError> {
        if self.selection.is_caret() || tab_width == 0 {
            return Ok(false);
        }

        let before_bytes = self.tree.read_all()?;
        let before_selection = self.selection;
        let sel_start = self.selection.start().min(before_bytes.len());
        let sel_end = self.selection.end().min(before_bytes.len());
        let scan_end = if sel_end > sel_start
            && sel_end > 0
            && before_bytes.get(sel_end - 1) == Some(&b'\n')
        {
            sel_end - 1
        } else {
            sel_end
        };
        let first_line_start = self.tree.line_start_offset(sel_start)?;

        let mut line_starts = vec![first_line_start];
        let mut cursor = first_line_start;
        while cursor < scan_end {
            if before_bytes[cursor] == b'\n' && cursor + 1 <= before_bytes.len() {
                line_starts.push(cursor + 1);
            }
            cursor += 1;
        }
        line_starts.sort_unstable();
        line_starts.dedup();

        let mut removals: Vec<(usize, usize)> = Vec::new();
        for &start in &line_starts {
            let mut count = 0usize;
            while count < tab_width
                && start + count < before_bytes.len()
                && before_bytes[start + count] == b' '
            {
                count += 1;
            }
            if count > 0 {
                removals.push((start, count));
            }
        }

        if removals.is_empty() {
            return Ok(false);
        }

        let mut out = Vec::with_capacity(
            before_bytes
                .len()
                .saturating_sub(removals.iter().map(|(_, count)| *count).sum()),
        );
        let mut prev = 0usize;
        for &(start, count) in &removals {
            out.extend_from_slice(&before_bytes[prev..start]);
            prev = start + count;
        }
        out.extend_from_slice(&before_bytes[prev..]);

        fn adjust_offset(offset: usize, removals: &[(usize, usize)]) -> usize {
            let mut adjusted = offset;
            for &(start, count) in removals {
                if adjusted > start + count {
                    adjusted -= count;
                } else if adjusted > start {
                    adjusted = start;
                }
            }
            adjusted
        }

        self.tree.replace_all(out)?;
        self.selection.anchor = Cursor::new(adjust_offset(
            before_selection.anchor.byte_offset,
            &removals,
        ));
        self.selection.active = Cursor::new(adjust_offset(
            before_selection.active.byte_offset,
            &removals,
        ));
        self.selection.clamp_to(self.tree.len());

        let after_bytes = self.tree.read_all()?;
        self.history.record_edit(
            EditGroupKind::Other,
            before_bytes,
            before_selection,
            after_bytes,
            self.selection,
        );
        self.preferred_column = None;
        self.persistence.dirty = true;
        Ok(true)
    }

    pub fn undo(&mut self) -> Result<bool, DocumentError> {
        let Some(step) = self.history.undo() else {
            return Ok(false);
        };
        self.tree.replace_all(step.bytes)?;
        self.selection = step.selection;
        self.selection.clamp_to(self.tree.len());
        self.preferred_column = None;
        self.persistence.dirty = true;
        Ok(true)
    }

    pub fn redo(&mut self) -> Result<bool, DocumentError> {
        let Some(step) = self.history.redo() else {
            return Ok(false);
        };
        self.tree.replace_all(step.bytes)?;
        self.selection = step.selection;
        self.selection.clamp_to(self.tree.len());
        self.preferred_column = None;
        self.persistence.dirty = true;
        Ok(true)
    }

    fn edit_replace_selection(
        &mut self,
        inserted_bytes: Vec<u8>,
        kind: EditGroupKind,
    ) -> Result<(), DocumentError> {
        let before_bytes = self.tree.read_all()?;
        let before_selection = self.selection;
        let start = self.selection.start();
        let delete_len = self.selection.len();
        self.tree
            .replace(start, delete_len, inserted_bytes.clone())?;
        let caret = start + inserted_bytes.len();
        self.selection = Selection::caret(caret);
        let after_bytes = self.tree.read_all()?;
        self.history.record_edit(
            kind,
            before_bytes,
            before_selection,
            after_bytes,
            self.selection,
        );
        self.preferred_column = None;
        self.persistence.dirty = true;
        Ok(())
    }

    fn set_active_cursor(&mut self, byte_offset: usize, extend: bool) {
        let target = byte_offset.min(self.tree.len());
        if extend {
            self.selection.set_active(target);
        } else {
            self.selection = Selection::caret(target);
        }
    }
}

fn previous_word_start_offset(bytes: &[u8], caret: usize) -> usize {
    let caret = caret.min(bytes.len());
    if caret == 0 {
        return 0;
    }

    let Ok(text) = std::str::from_utf8(bytes) else {
        return caret.saturating_sub(1);
    };

    let mut cursor = previous_char_boundary(text, caret);
    while cursor > 0 {
        let Some(ch) = previous_char(text, cursor) else {
            break;
        };
        if !ch.is_whitespace() {
            break;
        }
        cursor -= ch.len_utf8();
    }

    while cursor > 0 {
        let Some(ch) = previous_char(text, cursor) else {
            break;
        };
        if ch.is_whitespace() {
            break;
        }
        cursor -= ch.len_utf8();
    }

    cursor
}

fn previous_char_boundary(text: &str, offset: usize) -> usize {
    let mut cursor = offset.min(text.len());
    while cursor > 0 && !text.is_char_boundary(cursor) {
        cursor -= 1;
    }
    cursor
}

fn previous_char(text: &str, end_offset: usize) -> Option<char> {
    text[..end_offset].chars().next_back()
}
