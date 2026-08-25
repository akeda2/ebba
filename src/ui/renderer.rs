use std::io::Write;

use crate::document::Document;
use crate::ui::hex_view::{HexRenderOutput, HexView, HexViewport};
use crate::ui::status::StatusLine;
use crate::ui::text_view::{TextRenderOutput, TextView, TextViewport};

const STATUS_ROWS: usize = 1;

#[derive(Debug, Clone, Copy)]
pub enum RenderMode<'a> {
    Text { document: &'a Document },
    Hex { bytes: &'a [u8] },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderState {
    pub width: u16,
    pub height: u16,
    pub scroll_row: usize,
}

impl RenderState {
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            scroll_row: 0,
        }
    }

    pub fn body_height(&self) -> usize {
        self.height.saturating_sub(STATUS_ROWS as u16) as usize
    }

    pub fn resize(&mut self, width: u16, height: u16, cursor_row: usize, total_rows: usize) {
        self.width = width;
        self.height = height;
        self.ensure_cursor_visible(cursor_row, total_rows);
    }

    pub fn ensure_cursor_visible(&mut self, cursor_row: usize, total_rows: usize) {
        let body_height = self.body_height().max(1);
        if cursor_row < self.scroll_row {
            self.scroll_row = cursor_row;
        } else if cursor_row >= self.scroll_row + body_height {
            self.scroll_row = cursor_row + 1 - body_height;
        }
        self.clamp_scroll(total_rows);
    }

    pub fn clamp_scroll(&mut self, total_rows: usize) {
        let body_height = self.body_height().max(1);
        let max_scroll = total_rows.saturating_sub(body_height);
        self.scroll_row = self.scroll_row.min(max_scroll);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderFrame {
    pub lines: Vec<String>,
    pub cursor: Option<(u16, u16)>,
}

impl RenderFrame {
    pub fn to_string_frame(&self) -> String {
        self.lines.join("\n")
    }
}

#[derive(Debug, Clone)]
pub struct RenderRequest<'a> {
    pub mode: RenderMode<'a>,
    pub status: StatusLine,
}

pub trait TerminalFlush {
    fn flush(&mut self, frame: &RenderFrame) -> std::io::Result<()>;
}

pub struct WriterFlush<W: Write> {
    writer: W,
}

impl<W: Write> WriterFlush<W> {
    pub fn new(writer: W) -> Self {
        Self { writer }
    }
}

impl<W: Write> TerminalFlush for WriterFlush<W> {
    fn flush(&mut self, frame: &RenderFrame) -> std::io::Result<()> {
        self.writer.write_all(b"\x1b[?25l\x1b[H")?;
        for (row, line) in frame.lines.iter().enumerate() {
            let row_1_based = row + 1;
            let rendered = line.trim_end_matches(' ');
            let write = format!("\x1b[{};1H\x1b[2K{}", row_1_based, rendered);
            self.writer.write_all(write.as_bytes())?;
        }
        if let Some((row, col)) = frame.cursor {
            let row_1_based = row.saturating_add(1);
            let col_1_based = col.saturating_add(1);
            let position = format!("\x1b[?25h\x1b[{};{}H", row_1_based, col_1_based);
            self.writer.write_all(position.as_bytes())?;
        } else {
            self.writer.write_all(b"\x1b[?25l")?;
        }
        self.writer.flush()
    }
}

#[derive(Debug, Default)]
pub struct Renderer;

impl Renderer {
    pub fn render(&self, state: &mut RenderState, request: RenderRequest<'_>) -> RenderFrame {
        let body_width = state.width as usize;
        let body_height = state.body_height();
        let mut status = request.status;

        match request.mode {
            RenderMode::Text { document } => {
                let mut text = self.render_text(state, document, body_width, body_height);
                status = status.with_position(
                    text.cursor_line + 1,
                    text.cursor_column + 1,
                    text.total_lines,
                );
                let mut lines = vec![status.render(state.width as usize)];
                lines.append(&mut text.lines);
                RenderFrame {
                    lines,
                    cursor: text.cursor.map(|(row, col)| ((row + 1) as u16, col as u16)),
                }
            }

            RenderMode::Hex { bytes } => {
                let mut hex = self.render_hex(state, bytes, body_width, body_height);
                status = status.with_position(state.scroll_row + 1, 1, hex.total_rows);
                let mut lines = vec![status.render(state.width as usize)];
                lines.append(&mut hex.lines);
                RenderFrame {
                    lines,
                    cursor: None,
                }
            }

        }
    }

    fn render_text(
        &self,
        state: &mut RenderState,
        document: &Document,
        body_width: usize,
        body_height: usize,
    ) -> TextRenderOutput {
        let previous_scroll = state.scroll_row;
        let mut rendered = TextView::render(
            document,
            TextViewport {
                first_line: state.scroll_row,
                width: body_width,
                height: body_height,
            },
        );
        state.ensure_cursor_visible(rendered.cursor_line, rendered.total_lines);
        if (state.scroll_row != previous_scroll || rendered.cursor.is_none()) && body_height > 0 {
            rendered = TextView::render(
                document,
                TextViewport {
                    first_line: state.scroll_row,
                    width: body_width,
                    height: body_height,
                },
            );
        }
        rendered
    }

    fn render_hex(
        &self,
        state: &mut RenderState,
        bytes: &[u8],
        body_width: usize,
        body_height: usize,
    ) -> HexRenderOutput {
        let previous_scroll = state.scroll_row;
        let mut rendered = HexView::render(
            bytes,
            HexViewport {
                first_row: state.scroll_row,
                width: body_width,
                height: body_height,
            },
        );
        state.clamp_scroll(rendered.total_rows);
        if body_height > 0 && state.scroll_row != previous_scroll {
            rendered = HexView::render(
                bytes,
                HexViewport {
                    first_row: state.scroll_row,
                    width: body_width,
                    height: body_height,
                },
            );
        }
        rendered
    }
}

#[cfg(test)]
mod tests {
    use super::{RenderFrame, TerminalFlush, WriterFlush};

    #[test]
    fn writer_flush_does_not_emit_joined_newlines() {
        let mut sink = Vec::<u8>::new();
        let frame = RenderFrame {
            lines: vec!["a".to_string(), "b".to_string()],
            cursor: Some((1, 1)),
        };
        WriterFlush::new(&mut sink)
            .flush(&frame)
            .expect("flush should succeed");
        let rendered = String::from_utf8_lossy(&sink);
        assert!(!rendered.contains("\n\n"));
    }
}
