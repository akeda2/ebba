use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
#[cfg(unix)]
use std::{fs::Permissions, os::unix::fs::PermissionsExt};

use ebba::document::Document;
use ebba::document::encoding::DetectedEncoding;
use ebba::document::format::{LineEndingMode, analyze_line_endings};
use ebba::document::piece_tree::PieceTree;
use ebba::document::save::{SaveEncoding, SaveError, SaveOverrides, save_piece_tree_atomic};
use ebba::document::source::FileSourceRange;

fn fixture_path(name: &str) -> PathBuf {
    let root = PathBuf::from("target/test-fixtures");
    fs::create_dir_all(&root).expect("fixture directory should exist");
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time should be monotonic")
        .as_nanos();
    root.join(format!("{name}-{stamp}.bin"))
}

fn matching_temp_files(path: &PathBuf) -> Vec<String> {
    let parent = path.parent().expect("fixture path should have parent");
    let hint = path
        .file_name()
        .expect("fixture path should have file name")
        .to_string_lossy();
    let prefix = format!(".{hint}.ebba-save-");

    let mut files = Vec::new();
    for entry in fs::read_dir(parent).expect("fixture dir should be listable") {
        let entry = entry.expect("fixture dir entry should be readable");
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with(&prefix) {
            files.push(name);
        }
    }
    files.sort();
    files
}

#[test]
fn unchanged_round_trip_preserves_bytes() {
    let input_path = fixture_path("save-roundtrip-input");
    let output_path = fixture_path("save-roundtrip-output");
    let bytes = b"line1\r\nline2\n\xff\x00tail".to_vec();
    fs::write(&input_path, &bytes).expect("fixture write should succeed");

    let tree =
        PieceTree::from_file_ranges([FileSourceRange::new(&input_path, 0, bytes.len(), None)]);

    save_piece_tree_atomic(
        &tree,
        &output_path,
        DetectedEncoding::Unknown8Bit,
        LineEndingMode::Preserve,
        SaveOverrides::default(),
    )
    .expect("save should succeed");

    let written = fs::read(&output_path).expect("saved file should exist");
    assert_eq!(written, bytes);

    fs::remove_file(input_path).expect("fixture cleanup should succeed");
    fs::remove_file(output_path).expect("fixture cleanup should succeed");
}

#[test]
fn save_applies_conversion_overrides() {
    let output_path = fixture_path("save-convert");
    let mut document = Document::from_bytes(b"a\nb\n".to_vec());
    document.set_path(output_path.clone());
    document.configure_save_metadata(
        DetectedEncoding::Utf8,
        analyze_line_endings(b"a\nb\n", LineEndingMode::Preserve),
    );

    document
        .save(SaveOverrides {
            encoding: Some(SaveEncoding::Utf16LeBom),
            line_ending_mode: Some(LineEndingMode::Crlf),
        })
        .expect("save with override should succeed");

    let written = fs::read(&output_path).expect("saved file should exist");
    assert!(written.starts_with(&[0xFF, 0xFE]));
    let body = &written[2..];
    let code_units: Vec<u16> = body
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect();
    let text = String::from_utf16(&code_units).expect("utf-16 should decode");
    assert_eq!(text, "a\r\nb\r\n");

    fs::remove_file(output_path).expect("fixture cleanup should succeed");
}

#[test]
fn conversion_failure_does_not_clobber_destination() {
    let output_path = fixture_path("save-conversion-failure");
    fs::write(&output_path, b"ORIGINAL").expect("fixture write should succeed");

    let mut document = Document::from_bytes(vec![0xFF, 0xFE, 0xFF]);
    document.set_path(output_path.clone());
    document.configure_save_metadata(
        DetectedEncoding::Unknown8Bit,
        analyze_line_endings(&[], LineEndingMode::Preserve),
    );
    document.insert_text("x").expect("edit should succeed");

    let result = document.save(SaveOverrides {
        encoding: Some(SaveEncoding::Utf16LeBom),
        line_ending_mode: None,
    });
    assert!(matches!(
        result,
        Err(ebba::document::DocumentError::Save(
            SaveError::InvalidUtf8ForConversion { .. }
        ))
    ));

    let after = fs::read(&output_path).expect("destination should still exist");
    assert_eq!(after, b"ORIGINAL");

    fs::remove_file(output_path).expect("fixture cleanup should succeed");
}

#[test]
fn dirty_state_clears_after_successful_save() {
    let output_path = fixture_path("save-dirty-state");
    fs::write(&output_path, b"seed").expect("fixture write should succeed");

    let mut document = Document::from_bytes(b"seed".to_vec());
    document.set_path(output_path.clone());
    assert!(!document.is_dirty());

    document.insert_text("x").expect("edit should succeed");
    assert!(document.is_dirty());

    document
        .save(SaveOverrides::default())
        .expect("save should succeed");
    assert!(!document.is_dirty());

    fs::remove_file(output_path).expect("fixture cleanup should succeed");
}

#[test]
fn failed_save_keeps_dirty_and_removes_temporary_file() {
    let output_path = fixture_path("save-temp-cleanup");
    fs::write(&output_path, b"ORIGINAL").expect("fixture write should succeed");

    let mut document = Document::from_bytes(vec![0xFF, 0xFE, 0xFF]);
    document.set_path(output_path.clone());
    document.configure_save_metadata(
        DetectedEncoding::Unknown8Bit,
        analyze_line_endings(&[], LineEndingMode::Preserve),
    );
    document.insert_text("x").expect("edit should succeed");
    assert!(document.is_dirty());

    let before_temps = matching_temp_files(&output_path);
    let result = document.save(SaveOverrides {
        encoding: Some(SaveEncoding::Utf16LeBom),
        line_ending_mode: None,
    });
    assert!(matches!(
        result,
        Err(ebba::document::DocumentError::Save(
            SaveError::InvalidUtf8ForConversion { .. }
        ))
    ));
    assert!(document.is_dirty());

    let after_temps = matching_temp_files(&output_path);
    assert_eq!(after_temps, before_temps);
    assert_eq!(
        fs::read(&output_path).expect("destination should still exist"),
        b"ORIGINAL"
    );

    fs::remove_file(output_path).expect("fixture cleanup should succeed");
}

#[cfg(unix)]
#[test]
fn save_preserves_executable_permission_bits() {
    let output_path = fixture_path("save-preserve-executable");
    fs::write(&output_path, b"#!/bin/sh\necho hi\n").expect("fixture write should succeed");
    fs::set_permissions(&output_path, Permissions::from_mode(0o755))
        .expect("permissions should be set");

    let mut document = Document::from_bytes(b"#!/bin/sh\necho hi\n".to_vec());
    document.set_path(output_path.clone());
    document.insert_text("#").expect("edit should succeed");

    document
        .save(SaveOverrides::default())
        .expect("save should succeed");

    let mode = fs::metadata(&output_path)
        .expect("metadata should be readable")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o755);

    fs::remove_file(output_path).expect("fixture cleanup should succeed");
}
