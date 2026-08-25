use std::fs;
use std::path::PathBuf;

use ebba::document::piece_tree::PieceTree;
use ebba::document::source::FileSourceRange;

fn fixture_path(name: &str) -> PathBuf {
    let path = PathBuf::from("target/test-fixtures");
    fs::create_dir_all(&path).unwrap();
    path.join(name)
}

#[test]
fn file_backed_edits_do_not_require_full_materialization() {
    let file_path = fixture_path("large-piece-tree.bin");
    let mut bytes = vec![b'x'; 2_000_000];
    bytes[100] = b'\n';
    bytes[1_000_000] = b'\n';
    bytes[1_999_999] = b'\n';
    fs::write(&file_path, &bytes).unwrap();

    let range = FileSourceRange::new(&file_path, 0, bytes.len(), None);
    let mut tree = PieceTree::from_file_ranges([range]);
    assert_eq!(tree.line_feed_count_exact(), None);

    let source_reads_before_edit = tree.source(0).unwrap().bytes_read().unwrap();
    tree.insert(1_000_000, b"ABC".to_vec()).unwrap();
    tree.delete(1_500_000, 3).unwrap();
    let source_reads_after_edit = tree.source(0).unwrap().bytes_read().unwrap();
    assert_eq!(source_reads_before_edit, source_reads_after_edit);

    let snippet = tree.read_range(999_998, 8).unwrap();
    assert_eq!(&snippet, b"xxABC\nxx");

    let source_reads_after_small_read = tree.source(0).unwrap().bytes_read().unwrap();
    assert!(source_reads_after_small_read < 1024);

    fs::remove_file(file_path).unwrap();
}
