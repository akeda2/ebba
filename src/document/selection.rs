use crate::document::cursor::Cursor;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub anchor: Cursor,
    pub active: Cursor,
}

impl Selection {
    pub fn new(anchor: Cursor, active: Cursor) -> Self {
        Self { anchor, active }
    }

    pub fn caret(at: usize) -> Self {
        let cursor = Cursor::new(at);
        Self {
            anchor: cursor,
            active: cursor,
        }
    }

    pub fn is_caret(&self) -> bool {
        self.anchor == self.active
    }

    pub fn collapse_to_active(&mut self) {
        self.anchor = self.active;
    }

    pub fn collapse_to_start(&mut self) {
        let start = self.start();
        self.anchor = Cursor::new(start);
        self.active = self.anchor;
    }

    pub fn collapse_to_end(&mut self) {
        let end = self.end();
        self.anchor = Cursor::new(end);
        self.active = self.anchor;
    }

    pub fn start(&self) -> usize {
        self.anchor.byte_offset.min(self.active.byte_offset)
    }

    pub fn end(&self) -> usize {
        self.anchor.byte_offset.max(self.active.byte_offset)
    }

    pub fn len(&self) -> usize {
        self.end() - self.start()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn set_active(&mut self, byte_offset: usize) {
        self.active = Cursor::new(byte_offset);
    }

    pub fn clamp_to(&mut self, upper_bound: usize) {
        self.anchor.clamp_to(upper_bound);
        self.active.clamp_to(upper_bound);
    }
}
