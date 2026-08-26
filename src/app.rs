use std::fs;
use std::io::{Write, stdout};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

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
    input::{EventLoop, KeybindingProfile},
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
    current_save_encoding: SaveEncoding,
    baseline_save_encoding: SaveEncoding,
    format_dirty: bool,
    read_only: bool,
    hex_mode: bool,
    tab_width: usize,
    viewport_rows: usize,
    viewport_columns: usize,
    wrap_enabled: bool,
    wrap_column: Option<usize>,
    show_invisibles: bool,
    keybinding_profile: KeybindingProfile,
}

impl AppState {
    pub fn new(document: Document) -> Self {
        Self::with_save_overrides(document, SaveOverrides::default())
    }

    pub fn with_save_overrides(document: Document, save_overrides: SaveOverrides) -> Self {
        let detected_encoding = SaveEncoding::from_detected(document.detected_encoding());
        let current_save_encoding = save_overrides.encoding.unwrap_or(detected_encoding);
        Self {
            document,
            save_overrides,
            current_save_encoding,
            baseline_save_encoding: current_save_encoding,
            format_dirty: false,
            read_only: false,
            hex_mode: false,
            tab_width: 2,
            viewport_rows: 20,
            viewport_columns: 80,
            wrap_enabled: false,
            wrap_column: None,
            show_invisibles: false,
            keybinding_profile: KeybindingProfile::current(),
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

    pub fn set_wrap_column(&mut self, column: Option<usize>) {
        self.wrap_column = column.filter(|value| *value > 0);
    }

    pub fn wrap_column(&self) -> Option<usize> {
        self.wrap_column
    }

    pub fn set_viewport_rows(&mut self, rows: usize) {
        self.viewport_rows = rows.max(1);
    }

    pub fn set_viewport_columns(&mut self, columns: usize) {
        self.viewport_columns = columns.max(1);
    }

    pub fn set_show_invisibles(&mut self, show: bool) {
        self.show_invisibles = show;
    }

    pub fn show_invisibles(&self) -> bool {
        self.show_invisibles
    }

    pub fn set_keybinding_profile(&mut self, profile: KeybindingProfile) {
        self.keybinding_profile = profile;
    }

    pub fn is_dirty(&self) -> bool {
        self.document.is_dirty() || self.format_dirty
    }

    pub fn execute_command(&mut self, command: Command) -> AppResult<CommandDisposition> {
        match command {
            Command::Quit => {
                if self.document.can_quit() && !self.format_dirty {
                    Ok(CommandDisposition::Exit)
                } else {
                    Err(AppError::Message(unsaved_changes_message(
                        self.keybinding_profile,
                    )))
                }
            }

            Command::ForceQuit => Ok(CommandDisposition::Exit),
            Command::Save => {
                if self.read_only {
                    return Err(AppError::Message("buffer is read-only".to_string()));
                }
                self.document
                    .save(SaveOverrides {
                        encoding: Some(self.current_save_encoding),
                        line_ending_mode: self.save_overrides.line_ending_mode,
                    })
                    .map_err(|error| AppError::Message(error.to_string()))?;
                self.baseline_save_encoding = self.current_save_encoding;
                self.format_dirty = false;
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
            Command::ShowHelp => Ok(CommandDisposition::Continue),
            Command::ToggleBom => {
                if self.read_only {
                    return Err(AppError::Message("buffer is read-only".to_string()));
                }
                if self.document.detected_encoding()
                    == crate::document::encoding::DetectedEncoding::Unknown8Bit
                    && !self
                        .save_overrides
                        .encoding
                        .is_some_and(SaveEncoding::is_utf8_family)
                {
                    return Err(AppError::Message(
                        "BOM toggle is disabled for unknown-8bit files unless --encoding utf-8 or utf-8-bom is set"
                            .to_string(),
                    ));
                }
                let toggled = self.current_save_encoding.toggled_bom().ok_or_else(|| {
                    AppError::Message("BOM toggle is unavailable for this encoding".to_string())
                })?;
                self.current_save_encoding = toggled;
                self.format_dirty = self.current_save_encoding != self.baseline_save_encoding;
                Ok(CommandDisposition::Continue)
            }
            Command::ToggleWrap => {
                self.wrap_enabled = !self.wrap_enabled;
                Ok(CommandDisposition::Continue)
            }
            Command::ToggleInvisibles => {
                self.show_invisibles = !self.show_invisibles;
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
            Command::DeleteWordBackward => {
                if self.read_only {
                    return Err(AppError::Message("buffer is read-only".to_string()));
                }
                self.document
                    .delete_word_backward()
                    .map_err(|error| AppError::Message(error.to_string()))?;
                Ok(CommandDisposition::Continue)
            }
            Command::DeleteToLineStart => {
                if self.read_only {
                    return Err(AppError::Message("buffer is read-only".to_string()));
                }
                self.document
                    .delete_to_line_start()
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
            Command::PasteText(text) => {
                if self.read_only {
                    return Err(AppError::Message("buffer is read-only".to_string()));
                }
                self.document
                    .insert_text(&text)
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
                if self.wrap_enabled
                    && matches!(
                        direction,
                        MoveCommand::Up
                            | MoveCommand::Down
                            | MoveCommand::PageUp
                            | MoveCommand::PageDown
                    )
                    && self.handle_wrapped_vertical_move(direction, extend)?
                {
                    return Ok(CommandDisposition::Continue);
                }
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

    pub fn status_encoding_label(&self) -> &'static str {
        self.current_save_encoding.encoding_label()
    }

    pub fn status_bom_label(&self) -> &'static str {
        if self.current_save_encoding.has_bom() {
            "BOM"
        } else {
            "NO-BOM"
        }
    }

    fn handle_wrapped_vertical_move(
        &mut self,
        direction: MoveCommand,
        extend: bool,
    ) -> AppResult<bool> {
        let delta = match direction {
            MoveCommand::Up => -1isize,
            MoveCommand::Down => 1isize,
            MoveCommand::PageUp => -(self.viewport_rows as isize),
            MoveCommand::PageDown => self.viewport_rows as isize,
            _ => return Ok(false),
        };

        let bytes = self
            .document
            .bytes()
            .map_err(|error| AppError::Message(error.to_string()))?;
        let cursor_offset = self
            .document
            .selection()
            .active
            .byte_offset
            .min(bytes.len());
        let Some(target_offset) = wrapped_vertical_target_offset(
            &bytes,
            cursor_offset,
            self.viewport_columns,
            self.wrap_column,
            delta,
        ) else {
            return Ok(false);
        };
        if target_offset == cursor_offset {
            return Ok(true);
        }
        self.document.move_to_byte_offset(target_offset, extend);
        Ok(true)
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
    let keybinding_profile = args.keybinding_profile();
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
    app_state.set_keybinding_profile(keybinding_profile);
    app_state.set_show_invisibles(args.invisibles);
    match args.wrap {
        None => {
            app_state.set_wrap_enabled(false);
            app_state.set_wrap_column(None);
        }
        Some(None) => {
            app_state.set_wrap_enabled(true);
            app_state.set_wrap_column(None);
        }
        Some(Some(column)) => {
            app_state.set_wrap_enabled(true);
            app_state.set_wrap_column(Some(column));
        }
    }

    let mut terminal = Terminal::new()?;
    let _terminal_modes = TerminalModeGuard::enter()?;
    let event_loop = EventLoop {
        profile: keybinding_profile,
        ..EventLoop::default()
    };
    let renderer = Renderer;
    let mut render_state = RenderState::new(terminal.width, terminal.height);
    let mut flusher = WriterFlush::new(stdout());
    let mut startup_help_message = Some(startup_help_text(
        app_state.is_hex_mode(),
        keybinding_profile,
    ));
    let mut status_message: Option<String> = None;
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
                dirty: app_state.is_dirty(),
                read_only,
                encoding: app_state.status_encoding_label().to_string(),
                line_ending: app_state
                    .document()
                    .line_endings()
                    .indicator()
                    .label()
                    .to_string(),
                bom: app_state.status_bom_label().to_string(),
                tab_width: app_state.tab_width(),
                wrap_column: if app_state.is_wrap_enabled() {
                    Some(app_state.wrap_column().unwrap_or(0))
                } else {
                    None
                },
                show_invisibles: app_state.show_invisibles(),
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
                        header_message: startup_help_message.as_deref(),
                    },
                )
            } else {
                renderer.render(
                    &mut render_state,
                    RenderRequest {
                        mode: RenderMode::Text {
                            document: app_state.document(),
                            wrap: app_state.is_wrap_enabled(),
                            wrap_column: app_state.wrap_column(),
                            show_invisibles: app_state.show_invisibles(),
                        },
                        status,
                        header_message: startup_help_message.as_deref(),
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

        let chrome_rows = 1
            + Renderer::wrapped_header_lines(
                startup_help_message.as_deref(),
                render_state.width as usize,
            )
            .len()
            + usize::from(startup_help_message.is_some());
        app_state.set_viewport_columns(render_state.width as usize);
        app_state.set_viewport_rows(render_state.body_height_for(chrome_rows).max(1));
        for command in commands {
            if matches!(&command, Command::ShowHelp) {
                startup_help_message = Some(startup_help_text(
                    app_state.is_hex_mode(),
                    keybinding_profile,
                ));
                status_message = None;
                needs_render = true;
                continue;
            }
            startup_help_message = None;
            if app_state.is_hex_mode() {
                match command {
                    Command::InsertChar('q') => return Ok(()),
                    Command::InsertChar('Q') => return Ok(()),
                    _ => {}
                }
            }
            if app_state.is_hex_mode()
                && let Command::Move { direction, .. } = &command
            {
                let bytes = app_state
                    .document()
                    .bytes()
                    .map_err(|error| AppError::Message(error.to_string()))?;
                if apply_hex_scroll(&mut render_state, *direction, hex_total_rows(bytes.len())) {
                    needs_render = true;
                }
                status_message = None;
                continue;
            }
            let was_cycle_tab = matches!(&command, Command::CycleTabWidth);
            let was_toggle_bom = matches!(&command, Command::ToggleBom);
            let was_toggle_invisibles = matches!(&command, Command::ToggleInvisibles);
            let was_copy_or_cut = matches!(&command, Command::Copy | Command::Cut);
            match app_state.execute_command(command) {
                Ok(CommandDisposition::Exit) => return Ok(()),
                Ok(CommandDisposition::Continue) => {
                    if was_copy_or_cut {
                        export_terminal_clipboard_osc52(app_state.document().clipboard());
                    }
                    if was_cycle_tab || was_toggle_bom {
                        status_message = None;
                    } else if was_toggle_invisibles {
                        let mode = if app_state.show_invisibles() {
                            "on"
                        } else {
                            "off"
                        };
                        status_message = Some(format!("Invisibles: {mode}"));
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

fn startup_help_text(hex_mode: bool, profile: KeybindingProfile) -> String {
    if hex_mode {
        return match profile {
            KeybindingProfile::MacOs => String::from(
                "Quit: q/⌘Q/Ctrl+Q/F10 • Force quit: Ctrl+Alt+Q/Ctrl+Shift+Q/Ctrl+G/F12 • Help: ⇧⌘?/Ctrl+H • Scroll: ↑/↓/PgUp/PgDn/Home/End",
            ),
            KeybindingProfile::Default => String::from(
                "Quit: q/Alt+Q/F10 • Force quit: Ctrl+Alt+Q/Alt+Shift+Q/Ctrl+Shift+Q/Ctrl+G/F12 • Help: Ctrl+H/Alt+H • Scroll: ↑/↓/PgUp/PgDn/Home/End",
            ),
        };
    }

    match profile {
        KeybindingProfile::MacOs => String::from(
            "Save: ⌘S/Ctrl+S • Help: ⇧⌘?/Ctrl+H • Quit: ⌘Q/Ctrl+Q/F10 • Force quit: Ctrl+Alt+Q/Ctrl+Shift+Q/Ctrl+G/F12 • Clipboard: ⌘C/X/V, Ctrl+C/X/V, Ctrl+Shift+C/V (terminal) • Select: ⌘A, Shift+Arrows/Shift+PgUp/Shift+PgDn/Shift+⌥←→/Shift+⌘←→/Shift+⌘↑↓ • Undo: ⌘Z/Ctrl+Z • Redo: ⇧⌘Z/Ctrl+Y • Toggle BOM: Ctrl+B • Toggle tab: Ctrl+T • Toggle wrap: Ctrl+W • Toggle invisibles: Ctrl+K • Move: Arrows/Home/End/⌥←→/⌘←→/⌘↑↓/Ctrl+Home/Ctrl+End/PgUp/PgDn • Edit: Enter/Backspace/Delete/⌥Backspace/⌘Backspace/Ctrl+Backspace/Ctrl+U/Tab/Shift+Tab",
        ),
        KeybindingProfile::Default => String::from(
            "Save: Ctrl+S • Help: Ctrl+H/Alt+H • Quit: Ctrl+Q/Alt+Q/F10 • Force quit: Ctrl+Alt+Q/Alt+Shift+Q/Ctrl+Shift+Q/Ctrl+G/F12 • Clipboard: Ctrl+C/X/V, Ctrl+Shift+C/V (terminal) • Select: Ctrl+A, Shift+Arrows/Shift+PgUp/Shift+PgDn • Undo: Ctrl+Z • Redo: Ctrl+Y/Ctrl+Shift+Z • Toggle BOM: Ctrl+B/Alt+B/Ctrl+Shift+B • Toggle tab: Ctrl+T • Toggle wrap: Ctrl+W • Toggle invisibles: Ctrl+K/Alt+I • Move: Arrows/Home/End/Ctrl+←→/Ctrl+Home/Ctrl+End/PgUp/PgDn • Edit: Enter/Backspace/Delete/Ctrl+Backspace/Ctrl+U/Tab/Shift+Tab",
        ),
    }
}

fn unsaved_changes_message(profile: KeybindingProfile) -> String {
    match profile {
        KeybindingProfile::MacOs => {
            "unsaved changes: use ⌘S or Ctrl+S to save, or Ctrl+Alt+Q/Ctrl+G/F12 to force quit"
                .to_string()
        }
        KeybindingProfile::Default => {
            "unsaved changes: use Ctrl+S to save or Ctrl+Alt+Q/Alt+Shift+Q/Ctrl+G/F12 to force quit"
                .to_string()
        }
    }
}

fn export_terminal_clipboard_osc52(text: &str) {
    if text.is_empty() {
        return;
    }
    if !std::env::var_os("TERM").is_some_and(|term| term != "dumb") {
        return;
    }
    let encoded = base64_encode(text.as_bytes());
    let sequence = format!("\x1b]52;c;{encoded}\x07");
    let _ = std::io::stdout().write_all(sequence.as_bytes());
    let _ = std::io::stdout().flush();
}

fn base64_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    if input.is_empty() {
        return String::new();
    }

    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    let mut idx = 0usize;
    while idx + 3 <= input.len() {
        let chunk =
            ((input[idx] as u32) << 16) | ((input[idx + 1] as u32) << 8) | input[idx + 2] as u32;
        out.push(TABLE[((chunk >> 18) & 0x3F) as usize] as char);
        out.push(TABLE[((chunk >> 12) & 0x3F) as usize] as char);
        out.push(TABLE[((chunk >> 6) & 0x3F) as usize] as char);
        out.push(TABLE[(chunk & 0x3F) as usize] as char);
        idx += 3;
    }

    match input.len() - idx {
        1 => {
            let chunk = (input[idx] as u32) << 16;
            out.push(TABLE[((chunk >> 18) & 0x3F) as usize] as char);
            out.push(TABLE[((chunk >> 12) & 0x3F) as usize] as char);
            out.push('=');
            out.push('=');
        }
        2 => {
            let chunk = ((input[idx] as u32) << 16) | ((input[idx + 1] as u32) << 8);
            out.push(TABLE[((chunk >> 18) & 0x3F) as usize] as char);
            out.push(TABLE[((chunk >> 12) & 0x3F) as usize] as char);
            out.push(TABLE[((chunk >> 6) & 0x3F) as usize] as char);
            out.push('=');
        }
        _ => {}
    }

    out
}

#[derive(Clone, Copy)]
struct WrapLineRange {
    start: usize,
    end_no_newline: usize,
    end_with_newline: usize,
}

fn wrapped_vertical_target_offset(
    bytes: &[u8],
    cursor_offset: usize,
    viewport_width: usize,
    wrap_column: Option<usize>,
    delta_rows: isize,
) -> Option<usize> {
    let ranges = wrap_line_ranges(bytes);
    if ranges.is_empty() {
        return Some(0);
    }

    let total_lines = ranges.len().max(1);
    let gutter_width = 4usize.max(total_lines.to_string().len());
    let viewport_text_width = viewport_width.saturating_sub(gutter_width + 1).max(1);
    let text_width = wrap_column
        .filter(|column| *column > 0)
        .map(|column| column.min(viewport_text_width))
        .unwrap_or(viewport_text_width);

    let (line_index, column) = offset_to_wrap_line_column(&ranges, bytes, cursor_offset);
    let mut row_prefix = 0usize;
    for range in ranges.iter().take(line_index) {
        row_prefix += visual_rows_for_line(*range, bytes, text_width);
    }
    let visual_in_line = column / text_width;
    let visual_column = column % text_width;
    let current_global_row = row_prefix + visual_in_line;

    let total_rows: usize = ranges
        .iter()
        .map(|range| visual_rows_for_line(*range, bytes, text_width))
        .sum();
    let max_row = total_rows.saturating_sub(1) as isize;
    let target_row = (current_global_row as isize + delta_rows).clamp(0, max_row) as usize;

    let mut running = 0usize;
    for range in ranges {
        let rows = visual_rows_for_line(range, bytes, text_width);
        if target_row < running + rows {
            let row_in_line = target_row - running;
            let line_width = line_display_width(range, bytes);
            let target_column = (row_in_line * text_width + visual_column).min(line_width);
            return Some(line_column_to_offset(range, bytes, target_column));
        }
        running += rows;
    }

    Some(bytes.len())
}

fn wrap_line_ranges(bytes: &[u8]) -> Vec<WrapLineRange> {
    let mut ranges = Vec::new();
    let mut start = 0usize;

    for (index, byte) in bytes.iter().enumerate() {
        if *byte == b'\n' {
            ranges.push(WrapLineRange {
                start,
                end_no_newline: index,
                end_with_newline: index + 1,
            });
            start = index + 1;
        }
    }

    ranges.push(WrapLineRange {
        start,
        end_no_newline: bytes.len(),
        end_with_newline: bytes.len(),
    });
    ranges
}

fn line_display_end(range: WrapLineRange, bytes: &[u8]) -> usize {
    if range.end_with_newline > range.end_no_newline
        && range.end_no_newline > range.start
        && bytes[range.end_no_newline.saturating_sub(1)] == b'\r'
    {
        return range.end_no_newline.saturating_sub(1);
    }
    range.end_no_newline
}

fn line_display_width(range: WrapLineRange, bytes: &[u8]) -> usize {
    let start = range.start;
    let end = line_display_end(range, bytes);
    let slice = &bytes[start..end];
    match std::str::from_utf8(slice) {
        Ok(text) => UnicodeWidthStr::width(text),
        Err(_) => slice.len(),
    }
}

fn visual_rows_for_line(range: WrapLineRange, bytes: &[u8], width: usize) -> usize {
    let line_width = line_display_width(range, bytes);
    if line_width == 0 {
        1
    } else {
        line_width.div_ceil(width)
    }
}

fn offset_to_wrap_line_column(
    ranges: &[WrapLineRange],
    bytes: &[u8],
    offset: usize,
) -> (usize, usize) {
    for (index, range) in ranges.iter().enumerate() {
        let is_last = index + 1 == ranges.len();
        let in_line = if is_last {
            offset <= range.end_with_newline
        } else {
            offset < range.end_with_newline
        };
        if !in_line {
            continue;
        }
        let clamped_end = offset.min(line_display_end(*range, bytes));
        let slice = &bytes[range.start..clamped_end];
        let width = match std::str::from_utf8(slice) {
            Ok(text) => UnicodeWidthStr::width(text),
            Err(_) => slice.len(),
        };
        return (index, width);
    }
    (ranges.len().saturating_sub(1), 0)
}

fn line_column_to_offset(range: WrapLineRange, bytes: &[u8], target_column: usize) -> usize {
    let start = range.start;
    let end = line_display_end(range, bytes);
    let slice = &bytes[start..end];
    let Ok(text) = std::str::from_utf8(slice) else {
        return start + target_column.min(slice.len());
    };

    let mut used = 0usize;
    let mut byte_offset = 0usize;
    for (idx, ch) in text.char_indices() {
        let width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if used + width > target_column {
            break;
        }
        used += width;
        byte_offset = idx + ch.len_utf8();
    }
    start + byte_offset
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use crate::command::{Command, MoveCommand};
    use crate::document::Document;
    use crate::document::encoding::DetectedEncoding;
    use crate::document::format::{LineEndingMode, analyze_line_endings};
    use crate::document::save::{SaveEncoding, SaveOverrides};

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
    fn toggle_invisibles_flips_runtime_state() {
        let document = Document::from_bytes(Vec::new());
        let mut app = AppState::new(document);
        assert!(!app.show_invisibles());
        app.execute_command(Command::ToggleInvisibles)
            .expect("toggle invisibles should succeed");
        assert!(app.show_invisibles());
    }

    #[test]
    fn show_help_command_is_non_destructive() {
        let document = Document::from_bytes(Vec::new());
        let mut app = AppState::new(document);
        let outcome = app
            .execute_command(Command::ShowHelp)
            .expect("show help should succeed");
        assert_eq!(outcome, CommandDisposition::Continue);
    }

    #[test]
    fn paste_text_inserts_terminal_payload() {
        let document = Document::from_bytes(Vec::new());
        let mut app = AppState::new(document);
        app.execute_command(Command::PasteText("hello".to_string()))
            .expect("paste text should succeed");
        assert_eq!(
            app.document().bytes().expect("bytes should be readable"),
            b"hello"
        );
    }

    #[test]
    fn toggle_bom_marks_format_dirty_and_save_persists_it() {
        let path = fixture_path("app-toggle-bom-save.txt");
        fs::write(&path, b"abc\n").expect("fixture write should succeed");

        let mut document = Document::from_bytes(b"abc\n".to_vec());
        document.set_path(path.clone());
        document.configure_save_metadata(
            DetectedEncoding::Utf8,
            analyze_line_endings(b"abc\n", LineEndingMode::Preserve),
        );
        let mut app = AppState::new(document);

        app.execute_command(Command::ToggleBom)
            .expect("bom toggle should succeed");
        assert!(app.is_dirty());
        assert_eq!(app.status_bom_label(), "BOM");
        assert!(app.execute_command(Command::Quit).is_err());

        app.execute_command(Command::Save)
            .expect("save should succeed after bom toggle");
        assert!(!app.is_dirty());
        assert!(
            fs::read(&path)
                .expect("saved file should be readable")
                .starts_with(&[0xEF, 0xBB, 0xBF])
        );

        fs::remove_file(path).expect("fixture cleanup should succeed");
    }

    #[test]
    fn toggle_bom_disabled_for_unknown_without_utf8_override() {
        let mut document = Document::from_bytes(vec![0xFF, 0xFE, 0x61]);
        document.configure_save_metadata(
            DetectedEncoding::Unknown8Bit,
            analyze_line_endings(&[], LineEndingMode::Preserve),
        );
        let mut app = AppState::new(document);
        let result = app.execute_command(Command::ToggleBom);
        assert!(result.is_err());
    }

    #[test]
    fn toggle_bom_allowed_for_unknown_with_utf8_override() {
        let mut document = Document::from_bytes(vec![0x61, 0x62, 0x63]);
        document.configure_save_metadata(
            DetectedEncoding::Unknown8Bit,
            analyze_line_endings(b"abc", LineEndingMode::Preserve),
        );
        let mut app = AppState::with_save_overrides(
            document,
            SaveOverrides {
                encoding: Some(SaveEncoding::Utf8),
                line_ending_mode: None,
            },
        );

        app.execute_command(Command::ToggleBom)
            .expect("bom toggle should be allowed with utf-8 override");
        assert_eq!(app.status_bom_label(), "BOM");
    }

    #[test]
    fn tab_with_selection_indents_lines_instead_of_replacing_selection() {
        let mut document = Document::from_bytes(b"a\nb".to_vec());
        document.select_all();
        let mut app = AppState::new(document);
        app.execute_command(Command::InsertChar('\t'))
            .expect("tab indent should succeed");
        assert_eq!(
            app.document().bytes().expect("bytes should be readable"),
            b"  a\n  b"
        );
    }

    #[test]
    fn shift_tab_outdents_selected_lines() {
        let mut document = Document::from_bytes(b"  a\n  b".to_vec());
        document.select_all();
        let mut app = AppState::new(document);
        app.execute_command(Command::OutdentSelection)
            .expect("outdent should succeed");
        assert_eq!(
            app.document().bytes().expect("bytes should be readable"),
            b"a\nb"
        );
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
    fn wrapped_move_down_advances_within_single_logical_line() {
        let mut app = AppState::new(Document::from_bytes(b"abcdefghijklmnop".to_vec()));
        app.set_wrap_enabled(true);
        app.set_wrap_column(Some(4));
        app.set_viewport_columns(20);

        let start = app.document().selection().active.byte_offset;
        app.execute_command(Command::Move {
            direction: MoveCommand::Down,
            extend: false,
        })
        .expect("wrapped down should succeed");
        let after_down = app.document().selection().active.byte_offset;
        assert!(after_down > start);

        app.execute_command(Command::Move {
            direction: MoveCommand::Up,
            extend: false,
        })
        .expect("wrapped up should succeed");
        let after_up = app.document().selection().active.byte_offset;
        assert_eq!(after_up, start);

        app.execute_command(Command::InsertChar('Z'))
            .expect("insert on wrapped row should succeed");
        assert!(
            app.document()
                .bytes()
                .expect("bytes should be readable")
                .contains(&b'Z')
        );
    }

    #[test]
    fn wrapped_move_down_crosses_visual_rows_before_next_logical_line() {
        let mut app = AppState::new(Document::from_bytes(b"abcdefghij\nxy".to_vec()));
        app.set_wrap_enabled(true);
        app.set_wrap_column(Some(4));
        app.set_viewport_columns(20);

        app.execute_command(Command::Move {
            direction: MoveCommand::Down,
            extend: false,
        })
        .expect("first wrapped down should succeed");
        let first = app.document().selection().active.byte_offset;
        assert_eq!(first, 4);

        app.execute_command(Command::Move {
            direction: MoveCommand::Down,
            extend: false,
        })
        .expect("second wrapped down should succeed");
        let second = app.document().selection().active.byte_offset;
        assert_eq!(second, 8);

        app.execute_command(Command::Move {
            direction: MoveCommand::Down,
            extend: false,
        })
        .expect("third wrapped down should succeed");
        let third = app.document().selection().active.byte_offset;
        assert_eq!(third, 11);
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
