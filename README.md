# EBBA

`ebba` is a minimal terminal editor.

Ebba is inspired by **fresh**, but is a minimal implementation that keeps the same navigation and clipboard-style shortcuts.

## Installation

### 1) Build a release binary

```bash
cargo build --release
```

Binary path:

```text
target/release/ebba
```

### 2) Install with Cargo

```bash
cargo install --path .
```

### 3) Install via project script

```bash
./inst.sh
```

`inst.sh` currently runs `cargo install --path .`, sudo-copies `target/release/ebba` to `/usr/local/bin/ebba` for all users, and does optional `gb` integration if available.

## Usage

```bash
ebba FILE [OPTIONS]
```

Example:

```bash
ebba README.md --wrap 80 --invisibles
```

## Startup options

- `FILE`  
  Path to the file to open.
- `--encoding <ENCODING>`  
  Save encoding override: `utf-8`, `utf-8-bom`, `utf-16le-bom`, `utf-16be-bom`.
- `--line-ending <lf|crlf>`  
  Force line endings on save (otherwise preserve mode).
- `--text`  
  Force text startup mode.
- `--binary`  
  Force binary fallback mode (read-only hex view).
- `--wrap [COLUMN]`  
  Enable wrapping; optional fixed wrap column (for example `--wrap 80`).
- `--invisibles`  
  Show invisible characters (space `·`, LF `␊`, CRLF `␍`).
- `--config <PATH>`  
  Load YAML config from explicit path.

`--text` and `--binary` are mutually exclusive.

## Keybindings

### Core

- Save: `Ctrl+S`
- Help: `Ctrl+H`, `Alt+H`
- Quit: `Ctrl+Q`, `Alt+Q`, `F10`
- Force quit: `Ctrl+Alt+Q`, `Alt+Shift+Q`, `Ctrl+Shift+Q`, `Ctrl+G`, `F12`

### Editing

- Insert newline: `Enter`
- Backspace: `Backspace`
- Delete forward: `Delete`
- Insert tab / indent selection: `Tab`
- Outdent selection: `Shift+Tab`

### Clipboard and selection

- Copy: `Ctrl+C`
- Cut: `Ctrl+X`
- Paste: `Ctrl+V`
- Select all: `Ctrl+A`

### Undo/redo

- Undo: `Ctrl+Z`
- Redo: `Ctrl+Y`, `Ctrl+Shift+Z`

### Toggles

- Toggle BOM: `Ctrl+B`, `Alt+B`, `Ctrl+Shift+B`
- Toggle tab width (2 → 4 → 8): `Ctrl+T`
- Toggle wrap: `Ctrl+W`
- Toggle invisibles: `Ctrl+K`, `Alt+I`

### Navigation

- Cursor move: `←`, `→`, `↑`, `↓`
- Word move: `Ctrl+←`, `Ctrl+→`
- Line boundaries: `Home`, `End`
- Document boundaries: `Ctrl+Home`, `Ctrl+End`
- Page move: `PageUp`, `PageDown`

### Extend selection

- Extend with arrows: `Shift+←`, `Shift+→`, `Shift+↑`, `Shift+↓`
- Extend by page: `Shift+PageUp`, `Shift+PageDown`
- Extend with document boundaries: `Ctrl+Shift+Home`, `Ctrl+Shift+End`

### Hex mode notes

- Opened via binary detection or `--binary`.
- Read-only.
- Scroll uses: `↑`, `↓`, `PageUp`, `PageDown`, `Home`, `End`.
- Quit also accepts plain `q`/`Q` in addition to global quit keys.

## License

GPL-2.0-only. See `LICENSE`.
