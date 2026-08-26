use ebba::document::Document;
use ebba::ui::hex_view::{HexView, HexViewport, format_hex_row};
use ebba::ui::renderer::{RenderMode, RenderRequest, RenderState, Renderer};
use ebba::ui::status::StatusLine;
use ebba::ui::text_view::{TextView, TextViewport};

#[test]
fn status_line_is_rendered_on_top_row() {
    let doc = Document::from_bytes(b"alpha\nbeta".to_vec());
    let mut state = RenderState::new(80, 4);
    let status = StatusLine {
        filename: "notes.txt".to_string(),
        message: Some("saved".to_string()),
        ..StatusLine::default()
    };
    let frame = Renderer.render(
        &mut state,
        RenderRequest {
            mode: RenderMode::Text {
                document: &doc,
                wrap: false,
                wrap_column: None,
                show_invisibles: false,
            },
            status,
            header_message: None,
        },
    );

    assert!(frame.lines[0].contains("notes.txt"));
    assert!(frame.lines[0].contains("saved"));
    assert!(frame.lines[1].contains("1"));
}

#[test]
fn text_mode_always_includes_line_number_gutter() {
    let doc = Document::from_bytes(b"hello".to_vec());
    let rendered = TextView::render(
        &doc,
        TextViewport {
            first_row: 0,
            width: 20,
            height: 1,
            wrap: false,
            wrap_column: None,
            show_invisibles: false,
        },
    );
    assert!(rendered.lines[0].starts_with("   1 "));
}

#[test]
fn viewport_clips_rows_based_on_scroll() {
    let doc = Document::from_bytes(b"l1\nl2\nl3\nl4\nl5".to_vec());
    let rendered = TextView::render(
        &doc,
        TextViewport {
            first_row: 2,
            width: 20,
            height: 2,
            wrap: false,
            wrap_column: None,
            show_invisibles: false,
        },
    );

    assert!(rendered.lines[0].contains("3"));
    assert!(rendered.lines[0].contains("l3"));
    assert!(rendered.lines[1].contains("4"));
    assert!(rendered.lines[1].contains("l4"));
    assert_eq!(rendered.lines.len(), 2);
}

#[test]
fn hex_rows_use_offset_hex_and_ascii_columns() {
    let row = format_hex_row(0, b"Hello\x00Rust");
    assert!(row.starts_with("00000000"));
    assert!(row.contains("48 65 6c 6c 6f 00"));
    assert!(row.contains("|Hello.Rust"));

    let rendered = HexView::render(
        b"0123456789abcdefz",
        HexViewport {
            first_row: 0,
            width: 80,
            height: 1,
        },
    );
    assert_eq!(rendered.total_rows, 2);
}

#[test]
fn resize_recalculates_scroll_layout() {
    let mut state = RenderState {
        width: 20,
        height: 5,
        scroll_row: 0,
    };

    state.ensure_cursor_visible(5, 6);
    assert_eq!(state.scroll_row, 2);

    let previous_scroll = state.scroll_row;
    state.resize(20, 3, 5, 6);
    assert!(state.scroll_row >= previous_scroll);
    assert_eq!(state.scroll_row, 4);
    assert_eq!(state.body_height(), 2);
}

#[test]
fn renderer_keeps_status_line_when_body_height_is_zero() {
    let doc = Document::from_bytes(b"one\ntwo".to_vec());
    let mut state = RenderState::new(40, 1);
    let frame = Renderer.render(
        &mut state,
        RenderRequest {
            mode: RenderMode::Text {
                document: &doc,
                wrap: false,
                wrap_column: None,
                show_invisibles: false,
            },
            status: StatusLine {
                filename: "tiny.txt".to_string(),
                ..StatusLine::default()
            },
            header_message: None,
        },
    );

    assert_eq!(frame.lines.len(), 1);
    assert!(frame.lines[0].contains("tiny.txt"));
}

#[test]
fn gutter_width_scales_with_total_line_count() {
    let content = (0..120).map(|_| "x").collect::<Vec<_>>().join("\n");
    let doc = Document::from_bytes(content.into_bytes());
    let rendered = TextView::render(
        &doc,
        TextViewport {
            first_row: 118,
            width: 20,
            height: 1,
            wrap: false,
            wrap_column: None,
            show_invisibles: false,
        },
    );

    assert!(rendered.lines[0].starts_with(" 119 "));
}

#[test]
fn short_hex_rows_keep_ascii_padding_column() {
    let row = format_hex_row(0x10, b"A");
    assert!(row.starts_with("00000010"));
    assert!(row.contains("41"));
    assert!(row.ends_with("|A               |"));
}

#[test]
fn text_view_renders_utf8_file_content() {
    let doc = Document::from_bytes("ölkjölkjölkjds\nnästa rad".as_bytes().to_vec());
    let rendered = TextView::render(
        &doc,
        TextViewport {
            first_row: 0,
            width: 40,
            height: 2,
            wrap: false,
            wrap_column: None,
            show_invisibles: false,
        },
    );

    assert!(rendered.lines[0].contains("ölkjölkjölkjds"));
    assert!(rendered.lines[1].contains("nästa rad"));
}

#[test]
fn renderer_places_cursor_on_first_text_row_not_status_row() {
    let doc = Document::from_bytes(b"abc".to_vec());
    let mut state = RenderState::new(40, 4);
    let frame = Renderer.render(
        &mut state,
        RenderRequest {
            mode: RenderMode::Text {
                document: &doc,
                wrap: false,
                wrap_column: None,
                show_invisibles: false,
            },
            status: StatusLine::default(),
            header_message: None,
        },
    );

    assert_eq!(frame.cursor, Some((1, 5)));
}

#[test]
fn renderer_places_transient_header_above_status() {
    let doc = Document::from_bytes(b"abc".to_vec());
    let mut state = RenderState::new(40, 4);
    let frame = Renderer.render(
        &mut state,
        RenderRequest {
            mode: RenderMode::Text {
                document: &doc,
                wrap: false,
                wrap_column: None,
                show_invisibles: false,
            },
            status: StatusLine {
                filename: "notes.txt".to_string(),
                ..StatusLine::default()
            },
            header_message: Some("Ctrl+Q quit"),
        },
    );

    assert!(frame.lines[0].contains("Ctrl+Q quit"));
    assert_eq!(frame.lines[1], "-".repeat(37));
    assert!(frame.lines[2].contains("notes.txt"));
    assert_eq!(frame.cursor, Some((3, 5)));
}

#[test]
fn renderer_wraps_transient_header_by_width() {
    let doc = Document::from_bytes(b"abc".to_vec());
    let mut state = RenderState::new(10, 6);
    let frame = Renderer.render(
        &mut state,
        RenderRequest {
            mode: RenderMode::Text {
                document: &doc,
                wrap: false,
                wrap_column: None,
                show_invisibles: false,
            },
            status: StatusLine::default(),
            header_message: Some("1234567890AB"),
        },
    );

    assert_eq!(frame.lines[0], "1234567890");
    assert_eq!(frame.lines[1], "AB");
    assert_eq!(frame.lines[2], "-".repeat(7));
    assert!(!frame.lines[3].is_empty());
    assert_eq!(frame.cursor, Some((4, 5)));
}

#[test]
fn renderer_columnizes_bulleted_header_segments() {
    let lines = Renderer::wrapped_header_lines(Some("A • BB • CCC • DDDD"), 14);
    assert_eq!(lines.len(), 2);
    assert!(lines[0].contains("A"));
    assert!(lines[0].contains("CCC"));
    assert!(lines[1].contains("BB"));
    assert!(lines[1].contains("DDDD"));
}

#[test]
fn renderer_keeps_split_siblings_in_same_column_order() {
    let lines = Renderer::wrapped_header_lines(Some("Force quit: A • Force quit: B • Save: S • Help: H"), 32);
    assert_eq!(lines.len(), 2);
    assert!(lines[0].contains("Force quit: A"));
    assert!(lines[1].contains("Force quit: B"));
}

#[test]
fn renderer_uses_single_column_for_narrow_bulleted_header() {
    let lines = Renderer::wrapped_header_lines(Some("Save: Ctrl+S • Quit: Ctrl+Q"), 12);
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], "Save: Ctrl+S");
    assert_eq!(lines[1], "Quit: Ctrl+Q");
}

#[test]
fn wrapped_status_shows_logical_line_and_wrap_segment() {
    let mut doc = Document::from_bytes(b"abcdefghij\nx".to_vec());
    doc.move_right(false).expect("move should succeed");
    doc.move_right(false).expect("move should succeed");
    doc.move_right(false).expect("move should succeed");
    doc.move_right(false).expect("move should succeed");
    doc.move_right(false).expect("move should succeed");
    let mut state = RenderState::new(80, 4);
    let frame = Renderer.render(
        &mut state,
        RenderRequest {
            mode: RenderMode::Text {
                document: &doc,
                wrap: true,
                wrap_column: Some(4),
                show_invisibles: false,
            },
            status: StatusLine::default(),
            header_message: None,
        },
    );

    assert!(
        frame.lines[0].contains("Ln 1 (2/3)"),
        "status line: {}",
        frame.lines[0]
    );
}

#[test]
fn selection_is_visually_highlighted() {
    let mut doc = Document::from_bytes(b"hello world".to_vec());
    doc.extend_selection_to(2);
    let rendered = TextView::render(
        &doc,
        TextViewport {
            first_row: 0,
            width: 30,
            height: 1,
            wrap: false,
            wrap_column: None,
            show_invisibles: false,
        },
    );

    assert!(rendered.lines[0].contains("\x1b[7mhe\x1b[27m"));
    assert!(!rendered.lines[0].contains("\x1b[7mhello world"));
}
