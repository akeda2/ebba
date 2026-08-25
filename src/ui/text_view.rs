use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::document::Document;

pub const MIN_GUTTER_WIDTH: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextViewport {
    pub first_line: usize,
    pub width: usize,
    pub height: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionSpan {
    pub start_column: usize,
    pub end_column: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextRenderOutput {
    pub lines: Vec<String>,
    pub cursor: Option<(usize, usize)>,
    pub cursor_line: usize,
    pub cursor_column: usize,
    pub total_lines: usize,
    pub selection_spans: Vec<Option<SelectionSpan>>,
}

#[derive(Debug, Default)]
pub struct TextView;

#[derive(Debug, Clone, Copy)]
struct LineRange {
    start: usize,
    end_no_newline: usize,
    end_with_newline: usize,
}

impl TextView {
    pub fn render(document: &Document, viewport: TextViewport) -> TextRenderOutput {
        let bytes = document.bytes().unwrap_or_default();
        let ranges = line_ranges(&bytes);
        let total_lines = ranges.len();
        let selection = document.selection();
        let cursor_offset = selection.active.byte_offset.min(bytes.len());
        let (cursor_line, cursor_column) = offset_to_line_column(&ranges, &bytes, cursor_offset);
        let selection_start = selection.start().min(bytes.len());
        let selection_end = selection.end().min(bytes.len());
        let gutter_width = gutter_width(total_lines);
        let text_width = viewport.width.saturating_sub(gutter_width + 1);
        let first_line = viewport
            .first_line
            .min(total_lines.saturating_sub(1))
            .min(total_lines);

        let mut lines = Vec::with_capacity(viewport.height);
        let mut selection_spans = Vec::with_capacity(viewport.height);
        let mut cursor = None;

        for row in 0..viewport.height {
            let line_index = first_line + row;
            if line_index >= total_lines {
                let filler = fit_to_width(
                    &format!("{:>width$} ~", "", width = gutter_width),
                    viewport.width,
                );
                lines.push(filler);
                selection_spans.push(None);
                continue;
            }

            let range = ranges[line_index];
            let content = String::from_utf8_lossy(&bytes[range.start..range.end_no_newline]);
            let line_selection = selection_span_for_line(range, &bytes, selection_start, selection_end);
            let decorated_text = render_text_with_selection(content.as_ref(), text_width, line_selection);
            let rendered = format!(
                "{:>gutter_width$} {}",
                line_index + 1,
                decorated_text,
                gutter_width = gutter_width
            );
            lines.push(rendered);

            if line_index == cursor_line {
                let max_cursor_col = text_width.saturating_sub(1);
                let visual_col = if text_width == 0 {
                    0
                } else {
                    cursor_column.min(max_cursor_col)
                };
                cursor = Some((row, gutter_width + 1 + visual_col));
            }

            selection_spans.push(line_selection);
        }

        TextRenderOutput {
            lines,
            cursor,
            cursor_line,
            cursor_column,
            total_lines,
            selection_spans,
        }
    }
}

fn line_ranges(bytes: &[u8]) -> Vec<LineRange> {
    let mut ranges = Vec::new();
    let mut start = 0usize;

    for (index, byte) in bytes.iter().enumerate() {
        if *byte == b'\n' {
            ranges.push(LineRange {
                start,
                end_no_newline: index,
                end_with_newline: index + 1,
            });
            start = index + 1;
        }
    }

    ranges.push(LineRange {
        start,
        end_no_newline: bytes.len(),
        end_with_newline: bytes.len(),
    });

    ranges
}

fn offset_to_line_column(ranges: &[LineRange], bytes: &[u8], offset: usize) -> (usize, usize) {
    for (index, range) in ranges.iter().enumerate() {
        let is_last = index + 1 == ranges.len();
        let in_line = if is_last {
            offset <= range.end_with_newline
        } else {
            offset < range.end_with_newline
        };
        if in_line {
            let line_offset = offset.min(range.end_no_newline).saturating_sub(range.start);
            let width = UnicodeWidthStr::width(
                String::from_utf8_lossy(&bytes[range.start..range.start + line_offset]).as_ref(),
            );
            return (index, width);
        }
    }

    let last = ranges.len().saturating_sub(1);
    (last, 0)
}

fn selection_span_for_line(
    range: LineRange,
    bytes: &[u8],
    selection_start: usize,
    selection_end: usize,
) -> Option<SelectionSpan> {
    if selection_start == selection_end {
        return None;
    }
    let start = selection_start.max(range.start).min(range.end_no_newline);
    let end = selection_end.max(range.start).min(range.end_no_newline);
    if start >= end {
        return None;
    }

    let start_width =
        UnicodeWidthStr::width(String::from_utf8_lossy(&bytes[range.start..start]).as_ref());
    let end_width =
        UnicodeWidthStr::width(String::from_utf8_lossy(&bytes[range.start..end]).as_ref());
    Some(SelectionSpan {
        start_column: start_width,
        end_column: end_width,
    })
}

fn gutter_width(total_lines: usize) -> usize {
    MIN_GUTTER_WIDTH.max(total_lines.max(1).to_string().len())
}

fn clip_to_width(input: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }

    let mut used = 0usize;
    let mut out = String::new();
    for ch in input.chars() {
        let width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if used + width > max_width {
            break;
        }
        used += width;
        out.push(ch);
    }
    out
}

fn fit_to_width(input: &str, width: usize) -> String {
    let clipped = clip_to_width(input, width);
    let used = UnicodeWidthStr::width(clipped.as_str());
    if used >= width {
        return clipped;
    }

    let mut padded = clipped;
    padded.push_str(&" ".repeat(width - used));
    padded
}

fn render_text_with_selection(
    content: &str,
    width: usize,
    selection: Option<SelectionSpan>,
) -> String {
    if width == 0 {
        return String::new();
    }

    let mut out = String::new();
    let mut used = 0usize;
    let mut inverse_on = false;

    for ch in content.chars() {
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if ch_width > 0 && used + ch_width > width {
            break;
        }

        let should_inverse = if let Some(sel) = selection {
            ch_width > 0 && used >= sel.start_column && used < sel.end_column
        } else {
            false
        };

        if should_inverse && !inverse_on {
            out.push_str("\x1b[7m");
            inverse_on = true;
        } else if !should_inverse && inverse_on {
            out.push_str("\x1b[27m");
            inverse_on = false;
        }

        out.push(ch);
        used += ch_width;
    }

    if inverse_on {
        out.push_str("\x1b[27m");
    }
    if used < width {
        out.push_str(&" ".repeat(width - used));
    }

    out
}

#[cfg(test)]
mod tests {
    use super::{line_ranges, offset_to_line_column};

    #[test]
    fn offset_at_next_line_start_maps_to_next_line() {
        let bytes = b"abc\n\nx";
        let ranges = line_ranges(bytes);

        assert_eq!(offset_to_line_column(&ranges, bytes, 4), (1, 0));
        assert_eq!(offset_to_line_column(&ranges, bytes, 5), (2, 0));
    }
}
