#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    pub byte_offset: usize,
}

impl Cursor {
    pub fn new(byte_offset: usize) -> Self {
        Self { byte_offset }
    }

    pub fn move_to(&mut self, byte_offset: usize) {
        self.byte_offset = byte_offset;
    }

    pub fn clamp_to(&mut self, upper_bound: usize) {
        if self.byte_offset > upper_bound {
            self.byte_offset = upper_bound;
        }
    }

    pub fn saturating_add(&mut self, delta: usize) {
        self.byte_offset = self.byte_offset.saturating_add(delta);
    }

    pub fn saturating_sub(&mut self, delta: usize) {
        self.byte_offset = self.byte_offset.saturating_sub(delta);
    }
}
