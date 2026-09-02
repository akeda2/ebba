use std::io::Write;

use crate::document::Document;
use crate::ui::hex_view::{HexRenderOutput, HexView, HexViewport};
use crate::ui::status::StatusLine;
use crate::ui::text_view::{TextRenderOutput, TextView, TextViewport};

const BASE_CHROME_ROWS: usize = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BackgroundColor {
    #[default]
    DarkGray,
    LightGray,
    Blue,
    Black,
}

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
    pub body_start_row: usize,
    pub background_color: BackgroundColor,
    pub body_background_spans: Vec<Option<(usize, usize)>>,
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
    pub background_color: BackgroundColor,
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
            let body_row = row.saturating_sub(frame.body_start_row);
            let bg_span = if row >= frame.body_start_row {
                frame.body_background_spans.get(body_row).copied().flatten()
            } else {
                None
            };
            let rendered = if bg_span.is_some() {
                line.as_str()
            } else {
                line.trim_end_matches(' ')
            };
            let write = if row >= frame.body_start_row {
                let (bg_sgr, fg_sgr) = match frame.background_color {
                    BackgroundColor::DarkGray => ("\x1b[48;5;235m", ""),
                    BackgroundColor::LightGray => ("\x1b[48;5;248m", "\x1b[30m"),
                    BackgroundColor::Blue => ("\x1b[48;5;18m", "\x1b[97m"),
                    BackgroundColor::Black => ("\x1b[40m\x1b[48;2;0;0;0m", ""),
                };
                if let Some((start_col, width)) = bg_span {
                    let (prefix, middle, suffix) =
                        split_by_display_columns_with_ansi(rendered, start_col, width);
                    format!(
                        "\x1b[{};1H\x1b[2K{}{}{}{}\x1b[49m\x1b[39m{}",
                        row_1_based, prefix, bg_sgr, fg_sgr, middle, suffix
                    )
                } else {
                    format!("\x1b[{};1H\x1b[2K{}", row_1_based, rendered)
                }
            } else {
                format!("\x1b[{};1H\x1b[2K{}", row_1_based, rendered)
            };
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

#[derive(Debug, Clone, Copy)]
struct TextRenderOptions<'a> {
    document: &'a Document,
    body_width: usize,
    body_height: usize,
    wrap: bool,
    wrap_column: Option<usize>,
    center_wrapped_text: bool,
    show_invisibles: bool,
    tab_width: usize,
}

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
                    TextRenderOptions {
                        document,
                        body_width,
                        body_height,
                        wrap,
                        wrap_column,
                        center_wrapped_text,
                        show_invisibles,
                        tab_width,
                    },
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
                    body_start_row: chrome_rows,
                    background_color: request.background_color,
                    body_background_spans: text.background_spans,
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
                    body_start_row: chrome_rows,
                    background_color: request.background_color,
                    body_background_spans: vec![Some((0, body_width)); hex.lines.len()],
                }
            }
        }
    }

    fn render_text(
        &self,
        state: &mut RenderState,
        options: TextRenderOptions<'_>,
    ) -> TextRenderOutput {
        let previous_scroll = state.scroll_row;
        let TextRenderOptions {
            document,
            body_width,
            body_height,
            wrap,
            wrap_column,
            center_wrapped_text,
            show_invisibles,
            tab_width,
        } = options;
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

fn split_by_display_columns_with_ansi(
    input: &str,
    start_col: usize,
    width: usize,
) -> (String, String, String) {
    if width == 0 {
        return (input.to_string(), String::new(), String::new());
    }
    let end_col = start_col.saturating_add(width);
    let mut prefix = String::new();
    let mut middle = String::new();
    let mut suffix = String::new();
    let mut col = 0usize;
    let mut idx = 0usize;
    let bytes = input.as_bytes();

    while idx < input.len() {
        if bytes[idx] == 0x1b {
            let seq_end = parse_ansi_escape_end(bytes, idx).unwrap_or(input.len());
            let seq = &input[idx..seq_end];
            if col < start_col {
                prefix.push_str(seq);
            } else if col < end_col {
                middle.push_str(seq);
            } else {
                suffix.push_str(seq);
            }
            idx = seq_end;
            continue;
        }

        let mut chars = input[idx..].chars();
        let Some(ch) = chars.next() else {
            break;
        };
        let ch_len = ch.len_utf8();
        let ch_width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        let target = if col < start_col {
            &mut prefix
        } else if col < end_col {
            &mut middle
        } else {
            &mut suffix
        };
        target.push(ch);
        idx += ch_len;
        col = col.saturating_add(ch_width);
    }

    (prefix, middle, suffix)
}

fn parse_ansi_escape_end(bytes: &[u8], start: usize) -> Option<usize> {
    if bytes.get(start) != Some(&0x1b) {
        return None;
    }
    if bytes.get(start + 1) != Some(&b'[') {
        return Some((start + 1).min(bytes.len()));
    }
    let mut idx = start + 2;
    while idx < bytes.len() {
        let b = bytes[idx];
        if (0x40..=0x7e).contains(&b) {
            return Some(idx + 1);
        }
        idx += 1;
    }
    Some(bytes.len())
}

#[cfg(test)]
mod tests {
    use super::{BackgroundColor, RenderFrame, TerminalFlush, WriterFlush};

    #[test]
    fn writer_flush_does_not_emit_joined_newlines() {
        let mut sink = Vec::<u8>::new();
        let frame = RenderFrame {
            lines: vec!["a".to_string(), "b".to_string()],
            cursor: Some((1, 1)),
            body_start_row: 1,
            background_color: BackgroundColor::DarkGray,
            body_background_spans: vec![Some((0, 1))],
        };
        WriterFlush::new(&mut sink)
            .flush(&frame)
            .expect("flush should succeed");
        let rendered = String::from_utf8_lossy(&sink);
        assert!(!rendered.contains("\n\n"));
    }

    #[test]
    fn writer_flush_applies_background_only_to_body_rows() {
        let mut sink = Vec::<u8>::new();
        let frame = RenderFrame {
            lines: vec![
                "header".to_string(),
                "status".to_string(),
                "1234 body".to_string(),
            ],
            cursor: None,
            body_start_row: 2,
            background_color: BackgroundColor::DarkGray,
            body_background_spans: vec![Some((5, 4))],
        };
        WriterFlush::new(&mut sink)
            .flush(&frame)
            .expect("flush should succeed");
        let rendered = String::from_utf8_lossy(&sink);
        assert!(rendered.contains("\x1b[3;1H\x1b[2K1234 \x1b[48;5;235mbody\x1b[49m\x1b[39m"));
        assert!(!rendered.contains("\x1b[3;1H\x1b[2K\x1b[48;5;235m1234"));
    }

    #[test]
    fn writer_flush_uses_black_text_on_light_gray_background() {
        let mut sink = Vec::<u8>::new();
        let frame = RenderFrame {
            lines: vec!["header".to_string(), "body".to_string()],
            cursor: None,
            body_start_row: 1,
            background_color: BackgroundColor::LightGray,
            body_background_spans: vec![Some((0, 4))],
        };
        WriterFlush::new(&mut sink)
            .flush(&frame)
            .expect("flush should succeed");
        let rendered = String::from_utf8_lossy(&sink);
        assert!(rendered.contains("\x1b[2;1H\x1b[2K\x1b[48;5;248m\x1b[30mbody\x1b[49m\x1b[39m"));
    }

    #[test]
    fn writer_flush_uses_white_text_on_blue_background() {
        let mut sink = Vec::<u8>::new();
        let frame = RenderFrame {
            lines: vec!["header".to_string(), "body".to_string()],
            cursor: None,
            body_start_row: 1,
            background_color: BackgroundColor::Blue,
            body_background_spans: vec![Some((0, 4))],
        };
        WriterFlush::new(&mut sink)
            .flush(&frame)
            .expect("flush should succeed");
        let rendered = String::from_utf8_lossy(&sink);
        assert!(rendered.contains("\x1b[2;1H\x1b[2K\x1b[48;5;18m\x1b[97mbody\x1b[49m\x1b[39m"));
    }
}
