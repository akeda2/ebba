use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use ebba::config::{ConfigError, EditorConfig, LineEnding};

fn unique_path(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_nanos();
    std::env::current_dir()
        .expect("current dir should be available")
        .join("target")
        .join("test-artifacts")
        .join(format!("{name}-{nanos}-{}", std::process::id()))
}

fn write_config(contents: &str) -> PathBuf {
    let file_path = unique_path("config").join("config.yaml");
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent).expect("create test config dir");
    }
    fs::write(&file_path, contents).expect("write config");
    file_path
}

#[test]
fn loads_defaults_when_file_is_missing() {
    let missing_path = unique_path("missing").join("config.yaml");
    let loaded = EditorConfig::load(Some(missing_path)).expect("missing config should use default");
    assert_eq!(loaded, EditorConfig::default());
}

#[test]
fn parses_valid_config() {
    let path = write_config(
        r##"
indentation:
  tab_width: 8
  indent_width: 2
  use_tabs: true
default_line_ending: crlf
theme:
  foreground: "#ffffff"
  background: "#000000"
keybindings:
  save: "ctrl+s"
  quit: "Ctrl+Q"
"##,
    );

    let loaded = EditorConfig::load(Some(path)).expect("config should parse");
    assert_eq!(loaded.indentation.tab_width, 8);
    assert_eq!(loaded.indentation.indent_width, 2);
    assert!(loaded.indentation.use_tabs);
    assert_eq!(loaded.default_line_ending, LineEnding::Crlf);
    assert_eq!(loaded.theme.foreground, "#ffffff");
    assert_eq!(loaded.theme.background, "#000000");
    assert_eq!(
        loaded.keybindings.get("save").expect("save binding"),
        "Ctrl+S"
    );
}

#[test]
fn parses_cr_line_ending() {
    let path = write_config("default_line_ending: cr\n");
    let loaded = EditorConfig::load(Some(path)).expect("config should parse");
    assert_eq!(loaded.default_line_ending, LineEnding::Cr);
}

#[test]
fn malformed_yaml_and_invalid_values_fail() {
    let malformed_path = write_config("indentation: [");
    match EditorConfig::load(Some(malformed_path)) {
        Err(ConfigError::Parse { .. }) => {}
        other => panic!("expected parse error, got {other:?}"),
    }

    let invalid_value_path = write_config(
        r##"
indentation:
  tab_width: 0
theme:
  foreground: "#12FG34"
  background: "#000000"
"##,
    );

    match EditorConfig::load(Some(invalid_value_path)) {
        Err(ConfigError::Validation { field, .. }) => {
            assert_eq!(field, "indentation.tab_width");
        }
        other => panic!("expected validation error, got {other:?}"),
    }
}

#[test]
fn detects_keybinding_conflicts() {
    let path = write_config(
        r##"
keybindings:
  save: "Ctrl+S"
  search: "ctrl+s"
"##,
    );

    match EditorConfig::load(Some(path)) {
        Err(ConfigError::KeybindingConflict {
            keystroke,
            first_command,
            second_command,
        }) => {
            assert_eq!(keystroke, "Ctrl+S");
            assert_eq!(first_command, "save");
            assert_eq!(second_command, "search");
        }
        other => panic!("expected keybinding conflict, got {other:?}"),
    }
}

#[test]
fn detects_invalid_keybinding() {
    let path = write_config(
        r##"
keybindings:
  save: "Ctrl+Nope"
"##,
    );

    match EditorConfig::load(Some(path)) {
        Err(ConfigError::InvalidKeybinding { command, .. }) => {
            assert_eq!(command, "save");
        }
        other => panic!("expected invalid keybinding error, got {other:?}"),
    }
}

#[test]
fn detects_duplicate_keybinding_command() {
    let path = write_config(
        r##"
keybindings:
  save: "Ctrl+S"
  save: "Ctrl+Shift+S"
"##,
    );

    match EditorConfig::load(Some(path)) {
        Err(ConfigError::DuplicateKeybinding { command }) => {
            assert_eq!(command, "save");
        }
        other => panic!("expected duplicate keybinding error, got {other:?}"),
    }
}
