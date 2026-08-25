use std::fs;
use std::io::stdout;

use crate::{
    cli::Cli,
    command::{AppError, AppResult, Command, MoveCommand},
    config::{EditorConfig, LineEnding as ConfigLineEnding},
    document::{
        Document,
        encoding::{DetectionOptions, StartupDecision, StartupPayload, detect_startup_mode},
        format::LineEndingMode,
        save::{SaveEncoding, SaveError, SaveOverrides},
    },
    input::EventLoop,
    terminal::{Terminal, TerminalModeGuard},
    ui::{
        renderer::{RenderMode, RenderRequest, RenderState, Renderer, TerminalFlush, WriterFlush},
        status::StatusLine,
    },
};

const STARTUP_CONFIRMATION_THRESHOLD_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandDisposition {
    Continue,
    Exit,
}

#[derive(Debug)]
pub struct AppState {
    document: Document,
    save_overrides: SaveOverrides,
    read_only: bool,
    hex_mode: bool,
    tab_width: usize,
    viewport_rows: usize,
    wrap_enabled: bool,
}

impl AppState {
    pub fn new(document: Document) -> Self {
        Self::with_save_overrides(document, SaveOverrides::default())
    }

    pub fn with_save_overrides(document: Document, save_overrides: SaveOverrides) -> Self {
        Self {
            document,
            save_overrides,
            read_only: false,
            hex_mode: false,
            tab_width: 2,
            viewport_rows: 20,
            wrap_enabled: false,
        }
    }

    pub fn document(&self) -> &Document {
        &self.document
    }

    pub fn set_read_only(&mut self, read_only: bool) {
        self.read_only = read_only;
    }

    pub fn set_hex_mode(&mut self, hex_mode: bool) {
        self.hex_mode = hex_mode;
    }

    pub fn tab_width(&self) -> usize {
        self.tab_width
    }

    pub fn is_hex_mode(&self) -> bool {
        self.hex_mode
    }

    pub fn is_wrap_enabled(&self) -> bool {
        self.wrap_enabled
    }

    pub fn set_wrap_enabled(&mut self, enabled: bool) {
        self.wrap_enabled = enabled;
    }

    pub fn set_viewport_rows(&mut self, rows: usize) {
        self.viewport_rows = rows.max(1);
    }

    pub fn execute_command(&mut self, command: Command) -> AppResult<CommandDisposition> {
        match command {
            Command::Quit => {
                if self.document.can_quit() {
                    Ok(CommandDisposition::Exit)
                } else {
                    Err(AppError::Message(
                        "unsaved changes: use Ctrl+S to save or Ctrl+Alt+Q to force quit"
                            .to_string(),
                    ))
                }
            }

            Command::ForceQuit => Ok(CommandDisposition::Exit),
            Command::Save => {
                if self.read_only {
                    return Err(AppError::Message("buffer is read-only".to_string()));
                }
                self.document
                    .save(self.save_overrides)
                    .map_err(|error| AppError::Message(error.to_string()))?;
                Ok(CommandDisposition::Continue)
            }
            Command::InsertChar(ch) => {
                if self.read_only {
                    return Err(AppError::Message("buffer is read-only".to_string()));
                }
                if ch == '\t' && !self.document.selection().is_caret() {
                    self.document
                        .indent_selection_lines(self.tab_width)
                        .map_err(|error| AppError::Message(error.to_string()))?;
                } else {
                    let inserted = if ch == '\t' {
                        " ".repeat(self.tab_width)
                    } else {
                        ch.to_string()
                    };
                    self.document
                        .insert_text(&inserted)
                        .map_err(|error| AppError::Message(error.to_string()))?;
                }
                Ok(CommandDisposition::Continue)
            }
            Command::CycleTabWidth => {
                self.tab_width = match self.tab_width {
                    2 => 4,
                    4 => 8,
                    _ => 2,
                };
                Ok(CommandDisposition::Continue)
            }
            Command::ToggleWrap => {
                self.wrap_enabled = !self.wrap_enabled;
                Ok(CommandDisposition::Continue)
            }
            Command::OutdentSelection => {
                if self.read_only {
                    return Err(AppError::Message("buffer is read-only".to_string()));
                }
                self.document
                    .outdent_selection_lines(self.tab_width)
                    .map_err(|error| AppError::Message(error.to_string()))?;
                Ok(CommandDisposition::Continue)
            }
            Command::NewLine => {
                if self.read_only {
                    return Err(AppError::Message("buffer is read-only".to_string()));
                }
                self.document
                    .insert_text("\n")
                    .map_err(|error| AppError::Message(error.to_string()))?;
                Ok(CommandDisposition::Continue)
            }
            Command::Backspace => {
                if self.read_only {
                    return Err(AppError::Message("buffer is read-only".to_string()));
                }
                self.document
                    .delete_backward()
                    .map_err(|error| AppError::Message(error.to_string()))?;
                Ok(CommandDisposition::Continue)
            }
            Command::Delete => {
                if self.read_only {
                    return Err(AppError::Message("buffer is read-only".to_string()));
                }
                self.document
                    .delete_forward()
                    .map_err(|error| AppError::Message(error.to_string()))?;
                Ok(CommandDisposition::Continue)
            }
            Command::Copy => {
                self.document
                    .copy_selection()
                    .map_err(|error| AppError::Message(error.to_string()))?;
                Ok(CommandDisposition::Continue)
            }
            Command::Cut => {
                if self.read_only {
                    return Err(AppError::Message("buffer is read-only".to_string()));
                }
                self.document
                    .cut_selection()
                    .map_err(|error| AppError::Message(error.to_string()))?;
                Ok(CommandDisposition::Continue)
            }
            Command::Paste => {
                if self.read_only {
                    return Err(AppError::Message("buffer is read-only".to_string()));
                }
                self.document
                    .paste_clipboard()
                    .map_err(|error| AppError::Message(error.to_string()))?;
                Ok(CommandDisposition::Continue)
            }
            Command::SelectAll => {
                self.document.select_all();
                Ok(CommandDisposition::Continue)
            }
            Command::Undo => {
                if self.read_only {
                    return Err(AppError::Message("buffer is read-only".to_string()));
                }
                self.document
                    .undo()
                    .map_err(|error| AppError::Message(error.to_string()))?;
                Ok(CommandDisposition::Continue)
            }
            Command::Redo => {
                if self.read_only {
                    return Err(AppError::Message("buffer is read-only".to_string()));
                }
                self.document
                    .redo()
                    .map_err(|error| AppError::Message(error.to_string()))?;
                Ok(CommandDisposition::Continue)
            }
            Command::Move { direction, extend } => {
                match direction {
                    MoveCommand::Left | MoveCommand::WordLeft => self
                        .document
                        .move_left(extend)
                        .map_err(|error| AppError::Message(error.to_string()))?,
                    MoveCommand::Right | MoveCommand::WordRight => self
                        .document
                        .move_right(extend)
                        .map_err(|error| AppError::Message(error.to_string()))?,
                    MoveCommand::Up => self
                        .document
                        .move_up(extend)
                        .map_err(|error| AppError::Message(error.to_string()))?,
                    MoveCommand::Down => self
                        .document
                        .move_down(extend)
                        .map_err(|error| AppError::Message(error.to_string()))?,
                    MoveCommand::PageUp => self
                        .document
                        .move_page_up(self.viewport_rows, extend)
                        .map_err(|error| AppError::Message(error.to_string()))?,
                    MoveCommand::PageDown => self
                        .document
                        .move_page_down(self.viewport_rows, extend)
                        .map_err(|error| AppError::Message(error.to_string()))?,
                    MoveCommand::LineStart => self
                        .document
                        .move_line_start(extend)
                        .map_err(|error| AppError::Message(error.to_string()))?,
                    MoveCommand::LineEnd => self
                        .document
                        .move_line_end(extend)
                        .map_err(|error| AppError::Message(error.to_string()))?,
                    MoveCommand::DocumentStart => self.document.move_document_start(extend),
                    MoveCommand::DocumentEnd => self.document.move_document_end(extend),
                };
                Ok(CommandDisposition::Continue)
            }
        }
    }
}

fn hex_total_rows(byte_len: usize) -> usize {
    ((byte_len + 15) / 16).max(1)
}

fn apply_hex_scroll(state: &mut RenderState, direction: MoveCommand, total_rows: usize) -> bool {
    let before = state.scroll_row;
    let page = state.body_height().max(1);
    let max_scroll = total_rows.saturating_sub(page);

    state.scroll_row = match direction {
        MoveCommand::Up => state.scroll_row.saturating_sub(1),
        MoveCommand::Down => (state.scroll_row + 1).min(max_scroll),
        MoveCommand::PageUp => state.scroll_row.saturating_sub(page),
        MoveCommand::PageDown => state.scroll_row.saturating_add(page).min(max_scroll),
        MoveCommand::DocumentStart => 0,
        MoveCommand::DocumentEnd => max_scroll,
        MoveCommand::Left
        | MoveCommand::Right
        | MoveCommand::LineStart
        | MoveCommand::LineEnd
        | MoveCommand::WordLeft
        | MoveCommand::WordRight => state.scroll_row,
    };
    state.clamp_scroll(total_rows);
    state.scroll_row != before
}

pub fn run() -> AppResult<()> {
    let args = Cli::parse_args();
    let config_line_ending = if args.config.is_some() {
        let config = EditorConfig::load(args.config.clone())
            .map_err(|error| AppError::Message(error.to_string()))?;
        Some(match config.default_line_ending {
            ConfigLineEnding::Lf => LineEndingMode::Lf,
            ConfigLineEnding::Crlf => LineEndingMode::Crlf,
        })
    } else {
        None
    };

    let line_ending_override = if args.line_ending.is_some() {
        Some(args.line_ending_mode())
    } else {
        config_line_ending
    };
    let encoding_override = args
        .encoding
        .as_deref()
        .map(|label| {
            SaveEncoding::parse(label).ok_or_else(|| SaveError::UnsupportedEncoding {
                label: label.to_string(),
            })
        })
        .transpose()
        .map_err(|error| AppError::Message(error.to_string()))?;

    let mut document: Document;
    let mut hex_mode = false;
    let mut read_only = false;
    let path = args.file.clone();
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(error.into()),
    };
    let options = DetectionOptions {
        content_override: args.content_override(),
        line_ending_mode: line_ending_override.unwrap_or(LineEndingMode::Preserve),
        large_file_threshold_bytes: Some(STARTUP_CONFIRMATION_THRESHOLD_BYTES),
        ..DetectionOptions::default()
    };
    let startup = detect_startup_mode(&bytes, options);
    match startup {
        StartupDecision::Ready(plan) => {
            document = match plan.payload {
                StartupPayload::DecodedText { text, .. } => Document::from_bytes(text.into_bytes()),
                StartupPayload::BytePreservingText { bytes } => Document::from_bytes(bytes),
                StartupPayload::HexReadOnly { bytes } => {
                    hex_mode = true;
                    read_only = true;
                    Document::from_bytes(bytes)
                }
            };
            document.set_path(path.clone());
            document.configure_save_metadata(plan.encoding, plan.line_endings.clone());
        }
        StartupDecision::RequiresConfirmation(pending) => {
            document = match pending.proposed.payload {
                StartupPayload::DecodedText { text, .. } => Document::from_bytes(text.into_bytes()),
                StartupPayload::BytePreservingText { bytes } => Document::from_bytes(bytes),
                StartupPayload::HexReadOnly { bytes } => {
                    hex_mode = true;
                    read_only = true;
                    Document::from_bytes(bytes)
                }
            };
            document.set_path(path.clone());
            document.configure_save_metadata(
                pending.proposed.encoding,
                pending.proposed.line_endings.clone(),
            );
        }
    }

    let mut app_state = AppState::with_save_overrides(
        document,
        SaveOverrides {
            encoding: encoding_override,
            line_ending_mode: line_ending_override,
        },
    );
    app_state.set_hex_mode(hex_mode);
    app_state.set_read_only(read_only);
    app_state.set_wrap_enabled(args.wrap);

    let mut terminal = Terminal::new()?;
    let _terminal_modes = TerminalModeGuard::enter()?;
    let event_loop = EventLoop::default();
    let renderer = Renderer;
    let mut render_state = RenderState::new(terminal.width, terminal.height);
    let mut flusher = WriterFlush::new(stdout());
    let mut status_message = if app_state.is_hex_mode() {
        Some(String::from(
            "q quit • Esc force quit • ↑/↓/PgUp/PgDn/Home/End scroll",
        ))
    } else {
        Some(String::from(
            "Ctrl+Q quit • Ctrl+Alt+Q force quit • Ctrl+T tab width • Ctrl+W wrap",
        ))
    };
    let mut needs_render = true;

    loop {
        terminal = Terminal::new()?;
        if terminal.width != render_state.width || terminal.height != render_state.height {
            render_state.width = terminal.width;
            render_state.height = terminal.height;
            needs_render = true;
        }

        if needs_render {
            let filename = app_state
                .document()
                .path()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| String::from("[No Name]"));
            let status = StatusLine {
                filename,
                dirty: app_state.document().is_dirty(),
                read_only,
                encoding: encoding_override
                    .map(|enc| enc.to_string())
                    .unwrap_or_else(|| "utf-8".to_string()),
                line_ending: line_ending_override
                    .map(|mode| match mode {
                        LineEndingMode::Preserve => "PRESERVE",
                        LineEndingMode::Lf => "LF",
                        LineEndingMode::Crlf => "CRLF",
                    })
                    .map(str::to_string)
                    .unwrap_or_else(|| "PRESERVE".to_string()),
                wrap_enabled: app_state.is_wrap_enabled(),
                message: status_message.clone(),
                ..StatusLine::default()
            };

            let frame = if app_state.hex_mode {
                let bytes = app_state
                    .document()
                    .bytes()
                    .map_err(|error| AppError::Message(error.to_string()))?;
                renderer.render(
                    &mut render_state,
                    RenderRequest {
                        mode: RenderMode::Hex { bytes: &bytes },
                        status,
                    },
                )
            } else {
                renderer.render(
                    &mut render_state,
                    RenderRequest {
                        mode: RenderMode::Text {
                            document: app_state.document(),
                            wrap: app_state.is_wrap_enabled(),
                        },
                        status,
                    },
                )
            };
            flusher
                .flush(&frame)
                .map_err(|error| AppError::Message(error.to_string()))?;
            needs_render = false;
        }

        let commands = event_loop.tick()?;
        if commands.is_empty() {
            continue;
        }

        app_state.set_viewport_rows(render_state.body_height().max(1));
        for command in commands {
            if app_state.is_hex_mode() {
                match command {
                    Command::InsertChar('q') => return Ok(()),
                    Command::InsertChar('Q') => return Ok(()),
                    _ => {}
                }
            }
            if app_state.is_hex_mode()
                && let Command::Move { direction, .. } = command
            {
                let bytes = app_state
                    .document()
                    .bytes()
                    .map_err(|error| AppError::Message(error.to_string()))?;
                if apply_hex_scroll(&mut render_state, direction, hex_total_rows(bytes.len())) {
                    needs_render = true;
                }
                status_message = None;
                continue;
            }
            let was_cycle_tab = matches!(command, Command::CycleTabWidth);
            let was_toggle_wrap = matches!(command, Command::ToggleWrap);
            match app_state.execute_command(command) {
                Ok(CommandDisposition::Exit) => return Ok(()),
                Ok(CommandDisposition::Continue) => {
                    if was_cycle_tab {
                        status_message = Some(format!("Tab width: {}", app_state.tab_width()));
                    } else if was_toggle_wrap {
                        let mode = if app_state.is_wrap_enabled() { "on" } else { "off" };
                        status_message = Some(format!("Wrap: {mode}"));
                    } else {
                        status_message = None;
                    }
                    needs_render = true;
                }
                Err(error) => {
                    status_message = Some(error.to_string());
                    needs_render = true;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use crate::command::{Command, MoveCommand};
    use crate::document::Document;

    use crate::ui::renderer::RenderState;

    use super::{AppState, CommandDisposition, apply_hex_scroll};

    fn fixture_path(name: &str) -> PathBuf {
        let path = PathBuf::from("target/test-fixtures");
        fs::create_dir_all(&path).expect("fixture dir should exist");
        path.join(name)
    }

    #[test]
    fn quit_refuses_dirty_and_force_quit_exits() {
        let mut document = Document::from_bytes(b"abc".to_vec());
        document.insert_text("z").expect("insert should succeed");
        let mut app = AppState::new(document);

        let quit = app.execute_command(Command::Quit);
        assert!(quit.is_err());

        let force_quit = app
            .execute_command(Command::ForceQuit)
            .expect("force quit should work");
        assert_eq!(force_quit, CommandDisposition::Exit);
    }

    #[test]
    fn save_command_writes_and_clears_dirty() {
        let path = fixture_path("app-save-command.txt");
        fs::write(&path, b"abc").expect("fixture write should succeed");

        let mut document = Document::from_bytes(b"abc".to_vec());
        document.set_path(path.clone());
        document.insert_text("z").expect("insert should succeed");
        assert!(document.is_dirty());

        let mut app = AppState::new(document);
        let outcome = app
            .execute_command(Command::Save)
            .expect("save should succeed");
        assert_eq!(outcome, CommandDisposition::Continue);
        assert!(!app.document().is_dirty());
        assert_eq!(
            fs::read(&path).expect("saved file should be readable"),
            b"zabc"
        );

        fs::remove_file(path).expect("fixture cleanup should succeed");
    }

    #[test]
    fn tab_inserts_spaces() {
        let document = Document::from_bytes(Vec::new());
        let mut app = AppState::new(document);
        app.execute_command(Command::InsertChar('\t'))
            .expect("tab insert should succeed");
        assert_eq!(
            app.document().bytes().expect("bytes should be readable"),
            b"  "
        );
    }

    #[test]
    fn cycle_tab_width_changes_inserted_tab_size() {
        let document = Document::from_bytes(Vec::new());
        let mut app = AppState::new(document);
        assert_eq!(app.tab_width(), 2);

        app.execute_command(Command::CycleTabWidth)
            .expect("cycle should succeed");
        assert_eq!(app.tab_width(), 4);

        app.execute_command(Command::InsertChar('\t'))
            .expect("tab insert should succeed");
        assert_eq!(
            app.document().bytes().expect("bytes should be readable"),
            b"    "
        );
    }

    #[test]
    fn toggle_wrap_flips_runtime_wrap_state() {
        let document = Document::from_bytes(Vec::new());
        let mut app = AppState::new(document);
        assert!(!app.is_wrap_enabled());
        app.execute_command(Command::ToggleWrap)
            .expect("toggle wrap should succeed");
        assert!(app.is_wrap_enabled());
    }

    #[test]
    fn tab_with_selection_indents_lines_instead_of_replacing_selection() {
        let mut document = Document::from_bytes(b"a\nb".to_vec());
        document.select_all();
        let mut app = AppState::new(document);
        app.execute_command(Command::InsertChar('\t'))
            .expect("tab indent should succeed");
        assert_eq!(app.document().bytes().expect("bytes should be readable"), b"  a\n  b");
    }

    #[test]
    fn shift_tab_outdents_selected_lines() {
        let mut document = Document::from_bytes(b"  a\n  b".to_vec());
        document.select_all();
        let mut app = AppState::new(document);
        app.execute_command(Command::OutdentSelection)
            .expect("outdent should succeed");
        assert_eq!(app.document().bytes().expect("bytes should be readable"), b"a\nb");
    }

    #[test]
    fn page_down_uses_viewport_height() {
        let content = (0..30)
            .map(|i| format!("line{i}"))
            .collect::<Vec<_>>()
            .join("\n")
            .into_bytes();
        let mut app = AppState::new(Document::from_bytes(content.clone()));
        app.set_viewport_rows(6);
        app.execute_command(Command::Move {
            direction: MoveCommand::PageDown,
            extend: false,
        })
        .expect("page down should succeed");
        let page_offset = app.document().selection().active.byte_offset;

        let mut one_line = AppState::new(Document::from_bytes(content));
        one_line
            .execute_command(Command::Move {
                direction: MoveCommand::Down,
                extend: false,
            })
            .expect("line down should succeed");
        let line_offset = one_line.document().selection().active.byte_offset;

        assert!(page_offset > line_offset);
    }

    #[test]
    fn hex_page_down_scrolls_by_visible_rows() {
        let mut state = RenderState::new(80, 21);
        let changed = apply_hex_scroll(&mut state, MoveCommand::PageDown, 100);
        assert!(changed);
        assert_eq!(state.scroll_row, 20);
    }

    #[test]
    fn hex_left_right_do_not_change_scroll() {
        let mut state = RenderState::new(80, 21);
        state.scroll_row = 5;
        let left_changed = apply_hex_scroll(&mut state, MoveCommand::Left, 100);
        assert!(!left_changed);
        assert_eq!(state.scroll_row, 5);
        let right_changed = apply_hex_scroll(&mut state, MoveCommand::Right, 100);
        assert!(!right_changed);
        assert_eq!(state.scroll_row, 5);
    }
}
