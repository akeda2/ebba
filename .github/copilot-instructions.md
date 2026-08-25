# Copilot instructions for `ebba`

## Build, test, and lint commands

```bash
# Build/type-check
cargo check --quiet

# Full test suite
cargo test --quiet

# Run one integration test file
cargo test --quiet --test input
cargo test --quiet --test editing
cargo test --quiet --test terminal_ui

# Run one specific test by name (substring filter)
cargo test --quiet tab_with_selection_indents_lines_instead_of_replacing_selection
```

`ebba` currently has no separate lint command configured (no clippy target wired in project docs/scripts). Use `cargo check --quiet` plus targeted/full tests.

## High-level architecture

- **Entry/runtime loop**: `src/main.rs` calls `app::run()` in `src/app.rs`.  
  `app::run` is the integration point for CLI parsing, startup mode detection, document initialization, terminal mode setup, event polling, command execution, and rendering.

- **Command pipeline**:
  1. `src/input.rs` maps terminal events (`crossterm`) to `Command` values.
  2. `src/command.rs` defines the command model (`Move { direction, extend }`, edit commands, tab-width control, etc.).
  3. `AppState::execute_command` in `src/app.rs` applies commands to `Document`.

- **Document/editing core**: `src/document/mod.rs` wraps the piece-tree model and editing state:
  - text mutations, cursor/selection movement, undo/redo, clipboard,
  - selection-aware indent/outdent helpers,
  - save metadata + persistence via `document::save`.

- **Storage model**: `src/document/piece_tree.rs` is the byte-oriented text store used by `Document`.

- **Rendering path**:
  - `src/ui/text_view.rs` builds line-gutter text rows and cursor position,
  - `src/ui/hex_view.rs` builds read-only hex rows,
  - `src/ui/renderer.rs` composes status + body rows and flushes ANSI output.

## Key conventions in this codebase

- **Keybinding UX uses concrete shortcuts in user messages**, not internal command names.  
  Example: unsaved-quit warnings should mention `Ctrl+S` / `Ctrl+Alt+Q`.

- **Selection semantics are explicit in commands**: movement carries `extend` (`Command::Move { direction, extend }`).  
  Shift-modified movement is how selection is extended.

- **Tab behavior is runtime-configurable in-app**:
  - default tab width is **2 spaces**,
  - `Ctrl+T` cycles `2 -> 4 -> 8 -> 2`,
  - `Tab` inserts spaces using current tab width,
  - `Tab` with selection indents selected lines,
  - `Shift+Tab` (`BackTab`) outdents selected lines.

- **Clipboard is internal to the editor process** (copy/cut/paste currently do not integrate with OS clipboard).

- **Rendering uses ANSI control directly** (cursor hide/show, row addressing, per-line clear).  
  When changing rendering behavior, keep cursor placement, status-row offset, and selection highlighting aligned with `tests/terminal_ui.rs`.

- **Startup mode handling is centralized in `app::run` + `document::encoding`**: decoded text, byte-preserving fallback text, and hex read-only mode are selected before entering the main loop.

- **Tests are split by behavior area** (`tests/input.rs`, `tests/editing.rs`, `tests/terminal_ui.rs`, etc.).  
  Prefer targeted runs for changed areas plus a final `cargo test --quiet` when changes span subsystems.
