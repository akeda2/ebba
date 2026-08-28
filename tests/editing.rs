use ebba::document::Document;
use ebba::document::cursor::Cursor;
use ebba::document::line_index::LineIndex;
use ebba::document::piece_tree::PieceTree;
use ebba::document::selection::Selection;

#[test]
fn edits_produce_expected_bytes() {
    let mut tree = PieceTree::from_bytes(b"hello world".to_vec());
    tree.insert(5, b",".to_vec()).unwrap();
    tree.replace(7, 5, b"Rust".to_vec()).unwrap();
    tree.delete(0, 1).unwrap();

    let actual = tree.read_range(0, tree.len()).unwrap();
    assert_eq!(actual, b"ello, Rust");
}

#[test]
fn range_extraction_is_correct() {
    let mut tree = PieceTree::from_bytes(b"0123456789".to_vec());
    tree.insert(5, b"abc".to_vec()).unwrap();

    assert_eq!(tree.read_range(4, 5).unwrap(), b"4abc5");
    assert_eq!(tree.read_range(0, 3).unwrap(), b"012");
}

#[test]
fn line_feed_metadata_updates_on_edits() {
    let mut tree = PieceTree::from_bytes(b"a\nb\n".to_vec());
    assert_eq!(tree.line_feed_count_exact(), Some(2));

    tree.insert(tree.len(), b"\n".to_vec()).unwrap();
    assert_eq!(tree.line_feed_count_exact(), Some(3));

    tree.delete(1, 1).unwrap();
    assert_eq!(tree.line_feed_count_exact(), Some(2));

    let index = LineIndex::from_piece_tree(&tree);
    assert_eq!(index.exact_line_feeds(), Some(2));
}

#[test]
fn cursor_and_selection_use_byte_offsets() {
    let mut cursor = Cursor::new(10);
    cursor.saturating_sub(4);
    cursor.saturating_add(2);
    assert_eq!(cursor.byte_offset, 8);

    let selection = Selection::new(Cursor::new(9), Cursor::new(3));
    assert_eq!(selection.start(), 3);
    assert_eq!(selection.end(), 9);
    assert!(!selection.is_caret());
}

#[test]
fn select_copy_cut_paste_roundtrip() {
    let mut doc = Document::from_bytes(b"hello world".to_vec());
    for _ in 0..5 {
        doc.move_right(false).unwrap();
    }
    doc.move_right(true).unwrap();
    doc.move_right(true).unwrap();
    assert_eq!(doc.selection().start(), 5);
    assert_eq!(doc.selection().end(), 7);

    assert!(doc.copy_selection().unwrap());
    assert_eq!(doc.clipboard(), " w");

    assert!(doc.cut_selection().unwrap());
    assert_eq!(doc.bytes().unwrap(), b"helloorld");

    assert!(doc.paste_clipboard().unwrap());
    assert_eq!(doc.bytes().unwrap(), b"hello world");
}

#[test]
fn grouped_typing_and_backspace_undo_redo() {
    let mut doc = Document::default();
    doc.insert_text("h").unwrap();
    doc.insert_text("e").unwrap();
    doc.insert_text("y").unwrap();
    assert_eq!(doc.bytes().unwrap(), b"hey");

    assert!(doc.undo().unwrap());
    assert_eq!(doc.bytes().unwrap(), b"");
    assert!(doc.redo().unwrap());
    assert_eq!(doc.bytes().unwrap(), b"hey");

    doc.delete_backward().unwrap();
    doc.delete_backward().unwrap();
    assert_eq!(doc.bytes().unwrap(), b"h");
    assert!(doc.undo().unwrap());
    assert_eq!(doc.bytes().unwrap(), b"hey");
    assert!(doc.redo().unwrap());
    assert_eq!(doc.bytes().unwrap(), b"h");
}

#[test]
fn new_edit_clears_redo_history() {
    let mut doc = Document::default();
    doc.insert_text("a").unwrap();
    doc.insert_text("b").unwrap();
    assert_eq!(doc.bytes().unwrap(), b"ab");

    assert!(doc.undo().unwrap());
    assert_eq!(doc.bytes().unwrap(), b"");
    assert!(doc.can_redo());

    doc.insert_text("x").unwrap();
    assert_eq!(doc.bytes().unwrap(), b"x");
    assert!(!doc.can_redo());

    assert!(doc.undo().unwrap());
    assert_eq!(doc.bytes().unwrap(), b"");
}

#[test]
fn move_without_extend_collapses_existing_selection() {
    let mut doc = Document::from_bytes(b"abc".to_vec());
    doc.move_right(true).unwrap();
    doc.move_right(true).unwrap();
    assert_eq!(doc.selection().start(), 0);
    assert_eq!(doc.selection().end(), 2);

    doc.move_left(false).unwrap();
    assert!(doc.selection().is_caret());
    assert_eq!(doc.selection().active.byte_offset, 0);
}

#[test]
fn movement_and_selection_boundaries_hold() {
    let mut doc = Document::from_bytes("a💡\nxyz".as_bytes().to_vec());

    doc.move_left(false).unwrap();
    assert_eq!(doc.selection().active.byte_offset, 0);
    doc.move_right(false).unwrap();
    assert_eq!(doc.selection().active.byte_offset, 1);
    doc.move_right(false).unwrap();
    assert_eq!(doc.selection().active.byte_offset, "a💡".len());
    doc.move_right(false).unwrap();
    assert_eq!(doc.selection().active.byte_offset, "a💡\n".len());

    doc.move_down(false).unwrap();
    let down_offset = doc.selection().active.byte_offset;
    assert!(down_offset <= doc.len());
    doc.move_up(false).unwrap();
    assert!(doc.selection().active.byte_offset <= doc.len());

    doc.select_all();
    assert_eq!(doc.selection().start(), 0);
    assert_eq!(doc.selection().end(), doc.len());
    doc.clear_selection();
    assert!(doc.selection().is_caret());
    assert!(doc.selection().active.byte_offset <= doc.len());
}

#[test]
fn bare_cr_line_endings_are_navigable_like_lf() {
    // Old Mac-style CR-only line endings should behave the same as `\n` for
    // cursor movement, not merge into one giant line. Use equal-width lines
    // so vertical movement lands at a predictable column each time.
    let mut doc = Document::from_bytes(b"abc\rdef\rghi".to_vec());

    doc.move_document_start(false);
    for _ in 0..9 {
        doc.move_right(false).unwrap();
    }
    assert_eq!(doc.selection().active.byte_offset, 9); // column 1 of "ghi"

    doc.move_up(false).unwrap();
    assert_eq!(doc.selection().active.byte_offset, 5); // column 1 of "def"

    doc.move_up(false).unwrap();
    assert_eq!(doc.selection().active.byte_offset, 1); // column 1 of "abc"
}

#[test]
fn mixed_line_endings_select_current_line_stops_at_any_terminator() {
    let mut doc = Document::from_bytes(b"one\rtwo\r\nthree\nfour".to_vec());

    doc.move_document_start(false);
    for _ in 0.."one\r".len() {
        doc.move_right(false).unwrap();
    }
    assert!(doc.select_current_line(false).unwrap());
    assert_eq!(doc.selection().start(), "one\r".len());
    assert_eq!(doc.selection().end(), "one\rtwo\r".len());
}

#[test]
fn indent_selection_indents_each_selected_line() {
    let mut doc = Document::from_bytes(b"one\ntwo\nthree".to_vec());
    doc.select_all();
    assert!(doc.indent_selection_lines(2).unwrap());
    assert_eq!(doc.bytes().unwrap(), b"  one\n  two\n  three");
}

#[test]
fn indent_selection_treats_mixed_line_endings_as_separate_lines() {
    let mut doc = Document::from_bytes(b"one\rtwo\r\nthree".to_vec());
    doc.select_all();
    assert!(doc.indent_selection_lines(2).unwrap());
    assert_eq!(doc.bytes().unwrap(), b"  one\r  two\r\n  three");
}

#[test]
fn outdent_selection_removes_one_tab_width_from_each_line() {
    let mut doc = Document::from_bytes(b"  one\n  two\n  three".to_vec());
    doc.select_all();
    assert!(doc.outdent_selection_lines(2).unwrap());
    assert_eq!(doc.bytes().unwrap(), b"one\ntwo\nthree");
}

#[test]
fn outdent_then_indent_does_not_spill_to_next_line_when_selection_ends_on_line_boundary() {
    let mut doc = Document::from_bytes(b"  a\n  b\nc".to_vec());
    doc.extend_selection_to(8);

    assert!(doc.outdent_selection_lines(2).unwrap());
    assert_eq!(doc.bytes().unwrap(), b"a\nb\nc");

    assert!(doc.indent_selection_lines(2).unwrap());
    assert_eq!(doc.bytes().unwrap(), b"  a\n  b\nc");
}

#[test]
fn page_movement_moves_more_than_single_line() {
    let content = (0..30)
        .map(|i| format!("line{i}"))
        .collect::<Vec<_>>()
        .join("\n")
        .into_bytes();

    let mut single = Document::from_bytes(content.clone());
    single.move_down(false).unwrap();
    let one_line_offset = single.selection().active.byte_offset;

    let mut page = Document::from_bytes(content);
    page.move_page_down(8, false).unwrap();
    let page_offset = page.selection().active.byte_offset;
    assert!(page_offset > one_line_offset);

    page.move_page_up(8, false).unwrap();
    assert_eq!(page.selection().active.byte_offset, 0);
}

#[test]
fn delete_word_backward_removes_previous_word() {
    let mut doc = Document::from_bytes(b"alpha beta".to_vec());
    doc.move_document_end(false);
    doc.delete_word_backward().unwrap();
    assert_eq!(doc.bytes().unwrap(), b"alpha ");
}

#[test]
fn delete_word_backward_skips_trailing_whitespace() {
    let mut doc = Document::from_bytes(b"alpha beta   ".to_vec());
    doc.move_document_end(false);
    doc.delete_word_backward().unwrap();
    assert_eq!(doc.bytes().unwrap(), b"alpha ");
}

#[test]
fn delete_word_backward_is_unicode_aware() {
    let mut doc = Document::from_bytes("hej värld".as_bytes().to_vec());
    doc.move_document_end(false);
    doc.delete_word_backward().unwrap();
    assert_eq!(doc.bytes().unwrap(), "hej ".as_bytes());
}

#[test]
fn delete_word_backward_deletes_selection() {
    let mut doc = Document::from_bytes(b"hello world".to_vec());
    doc.move_right(true).unwrap();
    doc.move_right(true).unwrap();
    doc.delete_word_backward().unwrap();
    assert_eq!(doc.bytes().unwrap(), b"llo world");
}

#[test]
fn delete_to_line_start_removes_to_logical_line_start() {
    let mut doc = Document::from_bytes(b"first\nsecond".to_vec());
    doc.move_document_end(false);
    doc.delete_to_line_start().unwrap();
    assert_eq!(doc.bytes().unwrap(), b"first\n");
}

#[test]
fn delete_to_line_start_at_boundary_is_noop() {
    let mut doc = Document::from_bytes(b"first\nsecond".to_vec());
    doc.move_line_start(false).unwrap();
    doc.delete_to_line_start().unwrap();
    assert_eq!(doc.bytes().unwrap(), b"first\nsecond");
}

#[test]
fn delete_to_line_start_deletes_selection_when_present() {
    let mut doc = Document::from_bytes(b"hello world".to_vec());
    doc.move_right(true).unwrap();
    doc.move_right(true).unwrap();
    doc.delete_to_line_start().unwrap();
    assert_eq!(doc.bytes().unwrap(), b"llo world");
}

#[test]
fn delete_word_and_line_start_participate_in_undo_redo() {
    let mut doc = Document::from_bytes(b"alpha beta\ngamma".to_vec());
    doc.move_document_end(false);
    doc.delete_word_backward().unwrap();
    doc.delete_to_line_start().unwrap();
    assert_eq!(doc.bytes().unwrap(), b"alpha beta\n");

    assert!(doc.undo().unwrap());
    assert_eq!(doc.bytes().unwrap(), b"alpha beta\ngamma");
    assert!(doc.redo().unwrap());
    assert_eq!(doc.bytes().unwrap(), b"alpha beta\n");
}
