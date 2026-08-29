use std::io::Write;

use crate::document::Document;
use crate::ui::hex_view::{HexRenderOutput, HexView, HexViewport};
use crate::ui::status::StatusLine;
use crate::ui::text_view::{TextRenderOutput, TextView, TextViewport};

const BASE_CHROME_ROWS: usize = 1;

#[derive(Debug, Clone, Copy)]
pub enum RenderMode<'a> {
    Text {
        document: &'a Document,
        wrap: bool,
        wrap_column: Option<usize>,
        center_wrapped_text: bool,
        show_invisibles: bool,
        tab_width: usize,
    },
    Hex {
        bytes: &'a [u8],
    },
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
        self.body_height_for(BASE_CHROME_ROWS)
    }

    pub fn body_height_for(&self, chrome_rows: usize) -> usize {
        self.height.saturating_sub(chrome_rows as u16) as usize
    }

    pub fn resize(&mut self, width: u16, height: u16, cursor_row: usize, total_rows: usize) {
        self.width = width;
        self.height = height;
        self.ensure_cursor_visible(cursor_row, total_rows);
    }

    pub fn ensure_cursor_visible(&mut self, cursor_row: usize, total_rows: usize) {
        self.ensure_cursor_visible_for_body_height(
            cursor_row,
            total_rows,
            self.body_height().max(1),
        );
    }

    pub fn clamp_scroll(&mut self, total_rows: usize) {
        self.clamp_scroll_for_body_height(total_rows, self.body_height().max(1));
    }

    fn ensure_cursor_visible_for_body_height(
        &mut self,
        cursor_row: usize,
        total_rows: usize,
        body_height: usize,
    ) {
        if cursor_row < self.scroll_row {
            self.scroll_row = cursor_row;
        } else if cursor_row >= self.scroll_row + body_height {
            self.scroll_row = cursor_row + 1 - body_height;
        }
        self.clamp_scroll_for_body_height(total_rows, body_height);
    }

    fn clamp_scroll_for_body_height(&mut self, total_rows: usize, body_height: usize) {
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
    pub header_message: Option<&'a str>,
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

const HEADER_SEPARATOR_RIGHT_MARGIN: usize = 3;

impl Renderer {
    pub fn wrapped_header_lines(header_message: Option<&str>, width: usize) -> Vec<String> {
        let Some(message) = header_message else {
            return Vec::new();
        };
        if width == 0 {
            return Vec::new();
        }
        let mut lines = Vec::new();
        for segment in message.split('\n') {
            if segment.contains(" • ") {
                lines.extend(columnize_segment(segment, width));
            } else {
                lines.extend(wrap_segment(segment, width));
            }
        }
        lines
    }

    pub fn render(&self, state: &mut RenderState, request: RenderRequest<'_>) -> RenderFrame {
        let body_width = state.width as usize;
        let header_lines = Self::wrapped_header_lines(request.header_message, body_width);
        let show_separator = !header_lines.is_empty();
        let chrome_rows = 1 + header_lines.len() + usize::from(show_separator);
        let body_height = state.body_height_for(chrome_rows);
        let mut status = request.status;

        match request.mode {
            RenderMode::Text {
                document,
                wrap,
                wrap_column,
                center_wrapped_text,
                show_invisibles,
                tab_width,
            } => {
                let mut text = self.render_text(
                    state,
                    document,
                    body_width,
                    body_height,
                    wrap,
                    wrap_column,
                    center_wrapped_text,
                    show_invisibles,
                    tab_width,
                );
                status = status.with_position(
                    text.status_line + 1,
                    text.status_column + 1,
                    text.status_total_lines,
                );
                status.wrapped_segment = text.wrapped_segment;
                let mut lines = header_lines.clone();
                if show_separator {
                    lines.push(header_separator_line(state.width as usize));
                }
                lines.push(status.render(state.width as usize));
                lines.append(&mut text.lines);
                RenderFrame {
                    lines,
                    cursor: text
                        .cursor
                        .map(|(row, col)| ((row + chrome_rows) as u16, col as u16)),
                }
            }

            RenderMode::Hex { bytes } => {
                let mut hex = self.render_hex(state, bytes, body_width, body_height);
                status = status.with_position(state.scroll_row + 1, 1, hex.total_rows);
                let mut lines = header_lines.clone();
                if show_separator {
                    lines.push(header_separator_line(state.width as usize));
                }
                lines.push(status.render(state.width as usize));
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
        wrap: bool,
        wrap_column: Option<usize>,
        center_wrapped_text: bool,
        show_invisibles: bool,
        tab_width: usize,
    ) -> TextRenderOutput {
        let previous_scroll = state.scroll_row;
        let mut rendered = TextView::render(
            document,
            TextViewport {
                first_row: state.scroll_row,
                width: body_width,
                height: body_height,
                wrap,
                wrap_column,
                center_wrapped_text,
                show_invisibles,
                tab_width,
            },
        );
        state.ensure_cursor_visible_for_body_height(
            rendered.cursor_line,
            rendered.total_lines,
            body_height.max(1),
        );
        if (state.scroll_row != previous_scroll || rendered.cursor.is_none()) && body_height > 0 {
            rendered = TextView::render(
                document,
                TextViewport {
                    first_row: state.scroll_row,
                    width: body_width,
                    height: body_height,
                    wrap,
                    wrap_column,
                    center_wrapped_text,
                    show_invisibles,
                    tab_width,
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
        state.clamp_scroll_for_body_height(rendered.total_rows, body_height.max(1));
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

fn wrap_segment(segment: &str, width: usize) -> Vec<String> {
    let chars: Vec<char> = segment.chars().collect();
    if chars.is_empty() {
        return vec![String::new()];
    }
    chars
        .chunks(width)
        .map(|chunk| chunk.iter().collect())
        .collect()
}

fn truncate_to_width(input: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    input.chars().take(width).collect()
}

fn single_column_segment(items: &[String], width: usize) -> Vec<String> {
    items
        .iter()
        .map(|item| truncate_to_width(item, width))
        .collect()
}

fn columnize_segment(segment: &str, width: usize) -> Vec<String> {
    let items: Vec<String> = segment
        .split(" • ")
        .map(str::trim_end)
        .filter(|s| !s.trim().is_empty())
        .map(ToOwned::to_owned)
        .collect();
    if items.is_empty() {
        return wrap_segment(segment, width);
    }
    let max_item_width = items
        .iter()
        .map(|item| item.chars().count())
        .max()
        .unwrap_or(0);
    if max_item_width == 0 || max_item_width >= width {
        return single_column_segment(&items, width);
    }

    let col_width = max_item_width + 2;
    let cols = (width + 2) / col_width;
    if cols <= 1 {
        return single_column_segment(&items, width);
    }

    let rows = items.len().div_ceil(cols);
    let mut lines = Vec::with_capacity(rows);
    for row in 0..rows {
        let mut line = String::new();
        for col in 0..cols {
            let idx = col * rows + row;
            if idx >= items.len() {
                break;
            }
            let item = &items[idx];
            let item_width = item.chars().count();
            line.push_str(item);
            if col + 1 < cols && idx + 1 < items.len() {
                let pad = col_width.saturating_sub(item_width);
                line.push_str(&" ".repeat(pad));
            }
        }
        lines.push(line);
    }
    lines
}

fn header_separator_line(width: usize) -> String {
    if width <= HEADER_SEPARATOR_RIGHT_MARGIN {
        return String::new();
    }
    "-".repeat(width - HEADER_SEPARATOR_RIGHT_MARGIN)
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
