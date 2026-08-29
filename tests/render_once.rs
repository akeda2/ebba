use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn temp_fixture_path(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be monotonic enough for tests")
        .as_nanos();
    let dir = PathBuf::from("target")
        .join("test-fixtures")
        .join(format!("render-once-{unique}"));
    fs::create_dir_all(&dir).expect("temp fixture directory should be creatable");
    dir.join(name)
}

fn run_ebba(args: &[String], cwd: Option<&Path>) -> String {
    let exe = env!("CARGO_BIN_EXE_ebba");
    let mut cmd = Command::new(exe);
    cmd.args(args);
    if let Some(cwd) = cwd {
        cmd.current_dir(cwd);
    }
    let output = cmd.output().expect("ebba process should run");
    assert!(
        output.status.success(),
        "ebba failed: status={:?}, stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("stdout should be valid UTF-8")
}

fn render_once_args(file: &Path, width: u16, height: u16) -> Vec<String> {
    vec![
        file.to_string_lossy().into_owned(),
        "--render-once".to_string(),
        "--render-width".to_string(),
        width.to_string(),
        "--render-height".to_string(),
        height.to_string(),
    ]
}

#[test]
fn render_once_output_is_deterministic_for_same_fixture() {
    let file = fixture_path("render_once_cases.txt");
    let mut args = render_once_args(&file, 88, 12);
    args.push("--invisibles".to_string());

    let first = run_ebba(&args, None);
    let second = run_ebba(&args, None);
    assert_eq!(first, second);
    assert!(first.contains("# cursor: 2,6"));
}

#[test]
fn render_once_tab_visualization_matrix_matches_tab_stops() {
    let tab_file = temp_fixture_path("tab-matrix.txt");
    fs::write(&tab_file, b"\tX\n").expect("tab fixture should be writable");
    let tab_file_arg = tab_file
        .file_name()
        .expect("tab fixture should have filename")
        .to_string_lossy()
        .into_owned();
    let tab_cwd = tab_file.parent().expect("tab fixture should have parent");

    for width in [2usize, 4, 8] {
        let expected_marker = format!("→{}X", "·".repeat(width.saturating_sub(1)));
        let expected_spaces = format!("{}X", " ".repeat(width));

        for hard_tabs in [false, true] {
            let mut invis_args = vec![
                tab_file_arg.clone(),
                "--render-once".to_string(),
                "--render-width".to_string(),
                "120".to_string(),
                "--render-height".to_string(),
                "4".to_string(),
            ];
            invis_args.push("--invisibles".to_string());
            invis_args.push("--tab-width".to_string());
            invis_args.push(width.to_string());
            if hard_tabs {
                invis_args.push("--hard-tabs".to_string());
            }
            let invis_out = run_ebba(&invis_args, Some(tab_cwd));
            assert!(
                invis_out.contains(&expected_marker),
                "expected `{expected_marker}` for width={width}, hard_tabs={hard_tabs}; output:\n{invis_out}"
            );
            assert!(
                invis_out.contains(if hard_tabs { "TABS:HARD" } else { "TABS:SOFT" }),
                "expected tab mode status for hard_tabs={hard_tabs}; output:\n{invis_out}"
            );

            let mut plain_args = vec![
                tab_file_arg.clone(),
                "--render-once".to_string(),
                "--render-width".to_string(),
                "120".to_string(),
                "--render-height".to_string(),
                "4".to_string(),
            ];
            plain_args.push("--tab-width".to_string());
            plain_args.push(width.to_string());
            if hard_tabs {
                plain_args.push("--hard-tabs".to_string());
            }
            let plain_out = run_ebba(&plain_args, Some(tab_cwd));
            assert!(
                plain_out.contains(&expected_spaces),
                "expected expanded spaces `{expected_spaces}` for width={width}; output:\n{plain_out}"
            );
        }
    }
}

#[test]
fn render_once_honors_explicit_width_and_height_with_filler_rows() {
    let file = temp_fixture_path("short.txt");
    fs::write(&file, b"one\n").expect("short fixture should be writable");
    let out = run_ebba(&render_once_args(&file, 36, 6), None);

    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 7, "6 frame rows + 1 cursor row expected");
    assert!(
        lines.iter().any(|line| line.contains('~')),
        "expected at least one filler row with compact content"
    );
}

#[test]
fn render_once_hex_mode_outputs_expected_columns_and_hidden_cursor() {
    let file = temp_fixture_path("bytes.bin");
    fs::write(&file, [0x00, 0x01, 0x41, 0x42, 0x7F, 0xFF]).expect("binary fixture should write");
    let mut args = render_once_args(&file, 80, 5);
    args.push("--binary".to_string());
    let out = run_ebba(&args, None);

    assert!(out.contains("00000000"));
    assert!(out.contains("00 01 41 42 7f ff"));
    assert!(out.contains("# cursor: hidden"));
}

#[test]
fn render_once_composes_with_runtime_flags_without_terminal_ansi_sequences() {
    let cwd = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut args = vec![
        "tests/fixtures/render_once_cases.txt".to_string(),
        "--render-once".to_string(),
        "--render-width".to_string(),
        "220".to_string(),
        "--render-height".to_string(),
        "10".to_string(),
    ];
    args.extend(
        [
            "--invisibles",
            "--wrap",
            "20",
            "--center",
            "--hard-tabs",
            "--line-ending",
            "crlf",
            "--tab-width",
            "4",
        ]
        .into_iter()
        .map(ToOwned::to_owned),
    );
    let out = run_ebba(&args, Some(&cwd));

    assert!(out.contains("INV:ON"));
    assert!(out.contains("WRAP:20"));
    assert!(out.contains("TABS:HARD"));
    assert!(out.contains("TAB:4"));
    assert!(out.contains("CRLF"));
    assert!(!out.contains("\x1b["));
}

#[test]
fn render_once_snapshot_for_small_file_is_stable() {
    let file = temp_fixture_path("small.txt");
    fs::write(&file, b"A\nB\n").expect("small fixture should write");
    let file_arg = file
        .file_name()
        .expect("small fixture should have filename")
        .to_string_lossy()
        .into_owned();
    let cwd = file.parent().expect("small fixture should have parent");
    let args = vec![
        file_arg,
        "--render-once".to_string(),
        "--render-width".to_string(),
        "20".to_string(),
        "--render-height".to_string(),
        "5".to_string(),
    ];
    let out = run_ebba(&args, Some(cwd));
    let lines: Vec<&str> = out.lines().collect();

    assert!(lines[0].contains("small.txt"));
    assert_eq!(lines[1], "   1 A              ");
    assert_eq!(lines[2], "   2 B              ");
    assert_eq!(lines[5], "# cursor: 2,6");
}
