use unicode_width::UnicodeWidthChar;

use crate::document::Document;

pub const MIN_GUTTER_WIDTH: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextViewport {
    pub first_row: usize,
    pub width: usize,
    pub height: usize,
    pub wrap: bool,
    pub wrap_column: Option<usize>,
    pub center_wrapped_text: bool,
    pub show_invisibles: bool,
    pub tab_width: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionSpan {
    pub start_column: usize,
    pub end_column: usize,
    pub highlight_line_break: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextRenderOutput {
    pub lines: Vec<String>,
    pub cursor: Option<(usize, usize)>,
    pub cursor_line: usize,
    pub cursor_column: usize,
    pub total_lines: usize,
    pub status_line: usize,
    pub status_column: usize,
    pub status_total_lines: usize,
    pub wrapped_segment: Option<(usize, usize)>,
    pub selection_spans: Vec<Option<SelectionSpan>>,
    pub background_spans: Vec<Option<(usize, usize)>>,
}

#[derive(Debug, Default)]
pub struct TextView;

#[derive(Debug, Clone, Copy)]
struct LineRange {
    start: usize,
    end_no_newline: usize,
    end_with_newline: usize,
}

#[derive(Debug, Clone)]
struct WrappedSegment {
    text: String,
    start_column: usize,
    end_column: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LineEndingKind {
    Lf,
    Cr,
    Crlf,
}

impl TextView {
    pub fn render(document: &Document, viewport: TextViewport) -> TextRenderOutput {
        if viewport.wrap {
            render_wrapped(document, viewport)
        } else {
            render_unwrapped(document, viewport)
        }
    }
}

fn render_unwrapped(document: &Document, viewport: TextViewport) -> TextRenderOutput {
    let bytes = document.bytes().unwrap_or_default();
    let ranges = line_ranges(&bytes);
    let total_lines = ranges.len();
    let selection = document.selection();
    let cursor_offset = selection.active.byte_offset.min(bytes.len());
    let (cursor_line, cursor_column) =
        offset_to_line_column(&ranges, &bytes, cursor_offset, viewport.tab_width);
    let selection_start = selection.start().min(bytes.len());
    let selection_end = selection.end().min(bytes.len());
    let gutter_width = gutter_width(total_lines);
    let text_width = viewport.width.saturating_sub(gutter_width + 1);
    let first_line = viewport
        .first_row
        .min(total_lines.saturating_sub(1))
        .min(total_lines);

    let mut lines = Vec::with_capacity(viewport.height);
    let mut selection_spans = Vec::with_capacity(viewport.height);
    let mut background_spans = Vec::with_capacity(viewport.height);
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
            background_spans.push(None);
            continue;
        }

        let range = ranges[line_index];
        let ending = line_ending_kind(range, &bytes);
        let content = String::from_utf8_lossy(&bytes[range.start..line_display_end(range, &bytes)]);
        let decorated = decorate_line_text(
            content.as_ref(),
            viewport.show_invisibles,
            ending,
            viewport.tab_width,
        );
        let line_selection = selection_span_for_line(
            range,
            &bytes,
            selection_start,
            selection_end,
            viewport.tab_width,
        );
        let decorated_text = render_text_with_selection(&decorated, text_width, line_selection);
        let rendered = format!(
            "{:>gutter_width$} {}",
            line_index + 1,
            decorated_text,
            gutter_width = gutter_width
        );
        lines.push(rendered);
        background_spans.push((text_width > 0).then_some((gutter_width + 1, text_width)));

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
        status_line: cursor_line,
        status_column: cursor_column,
        status_total_lines: total_lines,
        wrapped_segment: None,
        selection_spans,
        background_spans,
    }
}

fn render_wrapped(document: &Document, viewport: TextViewport) -> TextRenderOutput {
    let bytes = document.bytes().unwrap_or_default();
    let ranges = line_ranges(&bytes);
    let total_lines = ranges.len();
    let selection = document.selection();
    let cursor_offset = selection.active.byte_offset.min(bytes.len());
    let (cursor_line, cursor_column) =
        offset_to_line_column(&ranges, &bytes, cursor_offset, viewport.tab_width);
    let selection_start = selection.start().min(bytes.len());
    let selection_end = selection.end().min(bytes.len());
    let gutter_width = gutter_width(total_lines);
    let viewport_text_width = viewport.width.saturating_sub(gutter_width + 1).max(1);
    let text_width = viewport
        .wrap_column
        .filter(|column| *column > 0)
        .map(|column| column.min(viewport_text_width))
        .unwrap_or(viewport_text_width);
    let center_padding = if viewport.center_wrapped_text {
        viewport_text_width.saturating_sub(text_width) / 2
    } else {
        0
    };
    let left_padding = " ".repeat(center_padding);

    let mut visual_rows: Vec<(usize, bool, String)> = Vec::new();
    let mut cursor_visual_row = 0usize;
    let mut cursor_col = gutter_width + 1;
    let mut cursor_segment = 1usize;
    let mut cursor_segment_total = 1usize;
    let mut visual_index = 0usize;

    for (line_index, range) in ranges.iter().copied().enumerate() {
        let ending = line_ending_kind(range, &bytes);
        let content = String::from_utf8_lossy(&bytes[range.start..line_display_end(range, &bytes)]);
        let decorated = decorate_line_text(
            content.as_ref(),
            viewport.show_invisibles,
            ending,
            viewport.tab_width,
        );
        let segments = wrap_segments(&decorated, text_width);
        let line_selection = selection_span_for_line(
            range,
            &bytes,
            selection_start,
            selection_end,
            viewport.tab_width,
        );
        let last_segment = segments.len().saturating_sub(1);

        for (segment_index, segment) in segments.iter().enumerate() {
            if line_index == cursor_line {
                let is_target = (cursor_column >= segment.start_column
                    && cursor_column < segment.end_column)
                    || (segment_index == last_segment && cursor_column >= segment.end_column);
                if is_target {
                    cursor_visual_row = visual_index;
                    cursor_segment = segment_index + 1;
                    cursor_segment_total = segments.len();
                    let segment_width = segment.end_column.saturating_sub(segment.start_column);
                    let relative = cursor_column
                        .saturating_sub(segment.start_column)
                        .min(segment_width);
                    cursor_col = gutter_width + 1 + center_padding + relative;
                }
            }

            let selection = line_selection.and_then(|sel| {
                let start = sel.start_column.max(segment.start_column);
                let end = sel.end_column.min(segment.end_column);
                if start >= end && !(sel.highlight_line_break && segment_index == last_segment) {
                    return None;
                }
                Some(SelectionSpan {
                    start_column: start - segment.start_column,
                    end_column: end - segment.start_column,
                    highlight_line_break: sel.highlight_line_break && segment_index == last_segment,
                })
            });

            visual_rows.push((
                line_index,
                segment_index == 0,
                render_text_with_selection(&segment.text, text_width, selection),
            ));
            visual_index += 1;
        }
    }

    if visual_rows.is_empty() {
        visual_rows.push((0, true, " ".repeat(text_width)));
    }

    let total_rows = visual_rows.len();
    let first_row = viewport
        .first_row
        .min(total_rows.saturating_sub(1))
        .min(total_rows);

    let mut lines = Vec::with_capacity(viewport.height);
    let mut selection_spans = Vec::with_capacity(viewport.height);
    let mut background_spans = Vec::with_capacity(viewport.height);
    let mut cursor = None;

    for row in 0..viewport.height {
        let visual_row = first_row + row;
        if visual_row >= total_rows {
            let filler = fit_to_width(
                &format!("{:>width$} ~", "", width = gutter_width),
                viewport.width,
            );
            lines.push(filler);
            selection_spans.push(None);
            background_spans.push(None);
            continue;
        }

        let (line_index, show_number, text) = &visual_rows[visual_row];
        let gutter = if *show_number {
            format!(
                "{:>gutter_width$}",
                line_index + 1,
                gutter_width = gutter_width
            )
        } else {
            " ".repeat(gutter_width)
        };
        lines.push(format!("{gutter} {left_padding}{text}"));
        selection_spans.push(None);
        let background_start = gutter_width + 1 + center_padding;
        let background_width = text_width.min(viewport.width.saturating_sub(background_start));
        background_spans.push((background_width > 0).then_some((background_start, background_width)));

        if visual_row == cursor_visual_row {
            cursor = Some((row, cursor_col));
        }
    }

    TextRenderOutput {
        lines,
        cursor,
        cursor_line: cursor_visual_row,
        cursor_column: cursor_col.saturating_sub(gutter_width + 1),
        total_lines: total_rows,
        status_line: cursor_line,
        status_column: cursor_column,
        status_total_lines: ranges.len(),
        wrapped_segment: if cursor_segment_total > 1 {
            Some((cursor_segment, cursor_segment_total))
        } else {
            None
        },
        selection_spans,
        background_spans,
    }
}

fn line_ranges(bytes: &[u8]) -> Vec<LineRange> {
    let mut ranges = Vec::new();
    let mut start = 0usize;
    let mut index = 0usize;

    while index < bytes.len() {
        match bytes[index] {
            b'\r' if bytes.get(index + 1) == Some(&b'\n') => {
                // Part of a CRLF pair; the `\n` branch below closes the line.
                index += 1;
            }
            b'\n' | b'\r' => {
                ranges.push(LineRange {
                    start,
                    end_no_newline: index,
                    end_with_newline: index + 1,
                });
                index += 1;
                start = index;
            }
            _ => index += 1,
        }
    }

    ranges.push(LineRange {
        start,
        end_no_newline: bytes.len(),
        end_with_newline: bytes.len(),
    });

    ranges
}

fn line_ending_kind(range: LineRange, bytes: &[u8]) -> Option<LineEndingKind> {
    if range.end_with_newline == range.end_no_newline {
        return None;
    }
    if bytes.get(range.end_no_newline) == Some(&b'\r') {
        return Some(LineEndingKind::Cr);
    }
    if range.end_no_newline > range.start && bytes[range.end_no_newline.saturating_sub(1)] == b'\r'
    {
        return Some(LineEndingKind::Crlf);
    }
    Some(LineEndingKind::Lf)
}

fn line_display_end(range: LineRange, bytes: &[u8]) -> usize {
    match line_ending_kind(range, bytes) {
        Some(LineEndingKind::Crlf) => range.end_no_newline.saturating_sub(1),
        _ => range.end_no_newline,
    }
}

fn decorate_line_text(
    content: &str,
    show_invisibles: bool,
    ending: Option<LineEndingKind>,
    tab_width: usize,
) -> String {
    let mut out = String::with_capacity(content.len() + 2);
    let mut column = 0usize;
    for ch in content.chars() {
        match ch {
            ' ' if show_invisibles => out.push('·'),
            '\t' => {
                let advance = tab_advance(column, tab_width);
                if show_invisibles {
                    out.push('→');
                    if advance > 1 {
                        out.push_str(&"·".repeat(advance - 1));
                    }
                } else {
                    out.push_str(&" ".repeat(advance));
                }
                column += advance;
                continue;
            }
            _ => out.push(ch),
        }
        column += UnicodeWidthChar::width(ch).unwrap_or(0);
    }
    if show_invisibles {
        match ending {
            Some(LineEndingKind::Lf) => out.push('␊'),
            Some(LineEndingKind::Cr) => out.push('␍'),
            Some(LineEndingKind::Crlf) => {
                out.push('␍');
                out.push('␊');
            }
            None => {}
        }
    }
    out
}

fn offset_to_line_column(
    ranges: &[LineRange],
    bytes: &[u8],
    offset: usize,
    tab_width: usize,
) -> (usize, usize) {
    for (index, range) in ranges.iter().enumerate() {
        let is_last = index + 1 == ranges.len();
        let in_line = if is_last {
            offset <= range.end_with_newline
        } else {
            offset < range.end_with_newline
        };
        if in_line {
            let line_offset = offset.min(range.end_no_newline).saturating_sub(range.start);
            let width =
                display_width_with_tabs(&bytes[range.start..range.start + line_offset], tab_width);
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
    tab_width: usize,
) -> Option<SelectionSpan> {
    let start = selection_start.max(range.start).min(range.end_no_newline);
    let end = selection_end.max(range.start).min(range.end_no_newline);

    let start_width = display_width_with_tabs(&bytes[range.start..start], tab_width);
    let end_width = display_width_with_tabs(&bytes[range.start..end], tab_width);
    let highlight_line_break = selection_start < range.end_with_newline
        && selection_end > range.end_no_newline
        && range.end_with_newline > range.end_no_newline;
    if start_width >= end_width && !highlight_line_break {
        return None;
    }
    Some(SelectionSpan {
        start_column: start_width,
        end_column: end_width,
        highlight_line_break,
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
    let used = clipped
        .chars()
        .map(|ch| UnicodeWidthChar::width(ch).unwrap_or(0))
        .sum::<usize>();
    if used >= width {
        return clipped;
    }

    let mut padded = clipped;
    padded.push_str(&" ".repeat(width - used));
    padded
}

fn display_width_with_tabs(bytes: &[u8], tab_width: usize) -> usize {
    let text = String::from_utf8_lossy(bytes);
    let mut column = 0usize;
    for ch in text.chars() {
        if ch == '\t' {
            column += tab_advance(column, tab_width);
        } else {
            column += UnicodeWidthChar::width(ch).unwrap_or(0);
        }
    }
    column
}

fn tab_advance(column: usize, tab_width: usize) -> usize {
    let width = tab_width.max(1);
    width - (column % width)
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
    if used < width && selection.is_some_and(|sel| sel.highlight_line_break) {
        out.push_str("\x1b[7m \x1b[27m");
        used += 1;
    }
    if used < width {
        out.push_str(&" ".repeat(width - used));
    }

    out
}

fn wrap_segments(content: &str, width: usize) -> Vec<WrappedSegment> {
    if content.is_empty() {
        return vec![WrappedSegment {
            text: String::new(),
            start_column: 0,
            end_column: 0,
        }];
    }

    let mut segments = Vec::new();
    let mut segment = String::new();
    let mut segment_start = 0usize;
    let mut segment_width = 0usize;
    let mut total_width = 0usize;

    for ch in content.chars() {
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if segment_width > 0 && segment_width + ch_width > width {
            segments.push(WrappedSegment {
                text: segment,
                start_column: segment_start,
                end_column: total_width,
            });
            segment = String::new();
            segment_start = total_width;
            segment_width = 0;
        }

        segment.push(ch);
        segment_width += ch_width;
        total_width += ch_width;
    }

    segments.push(WrappedSegment {
        text: segment,
        start_column: segment_start,
        end_column: total_width,
    });

    segments
}

#[cfg(test)]
mod tests {
    use crate::document::Document;

    use super::{TextView, TextViewport, line_ranges, offset_to_line_column};

    #[test]
    fn offset_at_next_line_start_maps_to_next_line() {
        let bytes = b"abc\n\nx";
        let ranges = line_ranges(bytes);

        assert_eq!(offset_to_line_column(&ranges, bytes, 4, 2), (1, 0));
        assert_eq!(offset_to_line_column(&ranges, bytes, 5, 2), (2, 0));
    }

    #[test]
    fn wraps_single_line_into_visual_rows() {
        let doc = Document::from_bytes(b"abcdef".to_vec());
        let rendered = TextView::render(
            &doc,
            TextViewport {
                first_row: 0,
                width: 9,
                height: 2,
                wrap: true,
                wrap_column: None,
                center_wrapped_text: false,
                show_invisibles: false,
                tab_width: 2,
            },
        );
        assert!(rendered.lines[0].contains("abcd"));
        assert!(rendered.lines[1].contains("ef"));
        assert!(rendered.lines[1].starts_with("     "));
    }

    #[test]
    fn wraps_at_requested_column_without_counting_gutter() {
        let doc = Document::from_bytes(b"abcdefghij".to_vec());
        let rendered = TextView::render(
            &doc,
            TextViewport {
                first_row: 0,
                width: 20,
                height: 3,
                wrap: true,
                wrap_column: Some(4),
                center_wrapped_text: false,
                show_invisibles: false,
                tab_width: 2,
            },
        );
        assert!(rendered.lines[0].contains("abcd"));
        assert!(rendered.lines[1].contains("efgh"));
        assert!(rendered.lines[2].contains("ij"));
    }

    #[test]
    fn invisibles_show_space_and_line_ending_markers() {
        let doc = Document::from_bytes(b"a b\r\nx\n".to_vec());
        let rendered = TextView::render(
            &doc,
            TextViewport {
                first_row: 0,
                width: 20,
                height: 2,
                wrap: false,
                wrap_column: None,
                center_wrapped_text: false,
                show_invisibles: true,
                tab_width: 2,
            },
        );
        assert!(rendered.lines[0].contains("a·b␍␊"));
        assert!(rendered.lines[1].contains("x␊"));
    }

    #[test]
    fn invisibles_show_tab_markers() {
        let doc = Document::from_bytes(b"a\tb\n".to_vec());
        let rendered = TextView::render(
            &doc,
            TextViewport {
                first_row: 0,
                width: 20,
                height: 1,
                wrap: false,
                wrap_column: None,
                center_wrapped_text: false,
                show_invisibles: true,
                tab_width: 4,
            },
        );
        assert!(rendered.lines[0].contains("a→··b␊"));
    }

    #[test]
    fn tabs_count_as_single_visual_column_for_cursor_math() {
        let bytes = b"a\tb";
        let ranges = line_ranges(bytes);
        assert_eq!(offset_to_line_column(&ranges, bytes, 0, 4), (0, 0));
        assert_eq!(offset_to_line_column(&ranges, bytes, 1, 4), (0, 1));
        assert_eq!(offset_to_line_column(&ranges, bytes, 2, 4), (0, 4));
        assert_eq!(offset_to_line_column(&ranges, bytes, 3, 4), (0, 5));
    }

    #[test]
    fn literal_tabs_expand_to_active_tab_stops() {
        let doc = Document::from_bytes(b"a\tb".to_vec());
        let rendered = TextView::render(
            &doc,
            TextViewport {
                first_row: 0,
                width: 20,
                height: 1,
                wrap: false,
                wrap_column: None,
                center_wrapped_text: false,
                show_invisibles: false,
                tab_width: 4,
            },
        );
        assert!(rendered.lines[0].contains("a   b"));
    }

    #[test]
    fn invisibles_tab_marker_expands_to_active_tab_width() {
        let doc = Document::from_bytes(b"\t".to_vec());
        let rendered_2 = TextView::render(
            &doc,
            TextViewport {
                first_row: 0,
                width: 20,
                height: 1,
                wrap: false,
                wrap_column: None,
                center_wrapped_text: false,
                show_invisibles: true,
                tab_width: 2,
            },
        );
        assert!(rendered_2.lines[0].contains("→·"));

        let rendered_4 = TextView::render(
            &doc,
            TextViewport {
                first_row: 0,
                width: 20,
                height: 1,
                wrap: false,
                wrap_column: None,
                center_wrapped_text: false,
                show_invisibles: true,
                tab_width: 4,
            },
        );
        assert!(rendered_4.lines[0].contains("→···"));

        let rendered_8 = TextView::render(
            &doc,
            TextViewport {
                first_row: 0,
                width: 20,
                height: 1,
                wrap: false,
                wrap_column: None,
                center_wrapped_text: false,
                show_invisibles: true,
                tab_width: 8,
            },
        );
        assert!(rendered_8.lines[0].contains("→·······"));
    }

    #[test]
    fn invisibles_show_distinct_marker_for_bare_cr() {
        let doc = Document::from_bytes(b"one\rtwo\n".to_vec());
        let rendered = TextView::render(
            &doc,
            TextViewport {
                first_row: 0,
                width: 20,
                height: 2,
                wrap: false,
                wrap_column: None,
                center_wrapped_text: false,
                show_invisibles: true,
                tab_width: 2,
            },
        );
        // Bare CR renders as a lone `␍`, distinct from CRLF's `␍␊` pair.
        assert!(rendered.lines[0].contains("one␍"));
        assert!(!rendered.lines[0].contains("one␍␊"));
        assert!(rendered.lines[1].contains("two␊"));
    }

    #[test]
    fn bare_cr_and_mixed_endings_split_into_separate_rendered_lines() {
        let doc = Document::from_bytes(b"one\rtwo\r\nthree\nfour".to_vec());
        let rendered = TextView::render(
            &doc,
            TextViewport {
                first_row: 0,
                width: 20,
                height: 4,
                wrap: false,
                wrap_column: None,
                center_wrapped_text: false,
                show_invisibles: false,
                tab_width: 2,
            },
        );
        assert!(rendered.lines[0].contains("one"));
        assert!(rendered.lines[1].contains("two"));
        assert!(rendered.lines[2].contains("three"));
        assert!(rendered.lines[3].contains("four"));
        assert_eq!(rendered.total_lines, 4);
    }

    #[test]
    fn centered_wrap_adds_padding_after_gutter() {
        let doc = Document::from_bytes(b"abcdefgh".to_vec());
        let rendered = TextView::render(
            &doc,
            TextViewport {
                first_row: 0,
                width: 20,
                height: 2,
                wrap: true,
                wrap_column: Some(4),
                center_wrapped_text: true,
                show_invisibles: false,
                tab_width: 2,
            },
        );
        assert_eq!(rendered.lines[0], "   1      abcd");
        assert_eq!(rendered.lines[1], "          efgh");
        assert_eq!(rendered.cursor, Some((0, 10)));
    }

    #[test]
    fn centered_wrap_clamps_to_viewport_text_width() {
        let doc = Document::from_bytes(b"abcd".to_vec());
        let rendered = TextView::render(
            &doc,
            TextViewport {
                first_row: 0,
                width: 8,
                height: 1,
                wrap: true,
                wrap_column: Some(80),
                center_wrapped_text: true,
                show_invisibles: false,
                tab_width: 2,
            },
        );
        assert_eq!(rendered.lines[0], "   1 abc");
    }
}
