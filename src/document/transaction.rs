use crate::document::selection::Selection;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditGroupKind {
    Typing,
    Backspace,
    DeleteForward,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditSnapshot {
    pub kind: EditGroupKind,
    pub before_bytes: Vec<u8>,
    pub after_bytes: Vec<u8>,
    pub before_selection: Selection,
    pub after_selection: Selection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UndoStep {
    pub bytes: Vec<u8>,
    pub selection: Selection,
}

#[derive(Debug, Default)]
pub struct TransactionLog {
    pending: Option<EditSnapshot>,
    undo_stack: Vec<EditSnapshot>,
    redo_stack: Vec<EditSnapshot>,
}

impl TransactionLog {
    pub fn record_edit(
        &mut self,
        kind: EditGroupKind,
        before_bytes: Vec<u8>,
        before_selection: Selection,
        after_bytes: Vec<u8>,
        after_selection: Selection,
    ) {
        let next = EditSnapshot {
            kind,
            before_bytes,
            after_bytes,
            before_selection,
            after_selection,
        };

        if self
            .pending
            .as_ref()
            .is_some_and(|pending| Self::can_merge(pending, &next))
        {
            if let Some(pending) = &mut self.pending {
                pending.after_bytes = next.after_bytes;
                pending.after_selection = next.after_selection;
            }
            return;
        }

        self.flush_pending();
        self.pending = Some(next);
        self.redo_stack.clear();
    }

    pub fn flush_pending(&mut self) {
        if let Some(pending) = self.pending.take() {
            self.undo_stack.push(pending);
        }
    }

    pub fn undo(&mut self) -> Option<UndoStep> {
        self.flush_pending();
        let snapshot = self.undo_stack.pop()?;
        let step = UndoStep {
            bytes: snapshot.before_bytes.clone(),
            selection: snapshot.before_selection,
        };
        self.redo_stack.push(snapshot);
        Some(step)
    }

    pub fn redo(&mut self) -> Option<UndoStep> {
        let snapshot = self.redo_stack.pop()?;
        let step = UndoStep {
            bytes: snapshot.after_bytes.clone(),
            selection: snapshot.after_selection,
        };
        self.undo_stack.push(snapshot);
        Some(step)
    }

    pub fn can_undo(&self) -> bool {
        self.pending.is_some() || !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    fn can_merge(current: &EditSnapshot, next: &EditSnapshot) -> bool {
        if current.kind != next.kind {
            return false;
        }
        if !current.after_selection.is_caret() || !next.before_selection.is_caret() {
            return false;
        }

        let expected = current.after_selection.active.byte_offset;
        let before = next.before_selection.active.byte_offset;
        match current.kind {
            EditGroupKind::Typing | EditGroupKind::Backspace | EditGroupKind::DeleteForward => {
                expected == before
            }
            EditGroupKind::Other => false,
        }
    }
}
