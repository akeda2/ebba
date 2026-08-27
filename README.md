# EBBA

`ebba` is a minimal terminal editor.

Written in rust, Ebba is inspired by **fresh**, but is a minimal implementation that keeps the same navigation and clipboard-style shortcuts.

## Features
- Modern style clipboarding: `ctrl+c/v/x/a`
- Terminal copy/paste: `ctrl+shift+c/v`
- Arrow key/pgup/pgdn/home/end navigation.
- Mandatory line numbering.
- Multi-line tab indentation.
- Multiple keybindings for exiting.
- Hotkey for showing invisible characters and line endings.
- Read only hex mode fallback for binary files.

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

### 4) Install on Windows via PowerShell

```powershell
.\inst.ps1
```

`inst.ps1` installs with `cargo install --path .` and ensures the user Cargo bin directory is present in the user PATH.

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
- `--encoding <ENCODING>`  Save encoding override: `utf-8`, `utf-8-bom`, `utf-16le-bom`, `utf-16be-bom`.
- `--line-ending <lf|crlf>`  Force line endings on save (otherwise preserve mode).
- `--text`  Force text startup mode.
- `--binary`  Force binary fallback mode (read-only hex view).
- `--wrap [COLUMN]`  Enable wrapping; optional fixed wrap column (for example `--wrap 80`).
- `--invisibles`  Show invisible characters (space `·`, LF `␊`, CRLF `␍`).
- `--config <PATH>`  Load YAML config from explicit path.
- `--keymap <auto|mac|linux|linux-console|windows>`  Force keybinding profile at startup (useful for cross-platform keymap testing).

`--text` and `--binary` are mutually exclusive.

## Config file location

Default auto-load path is `~/.config/ebba/config.yaml` on all platforms.  
On Windows, if that file does not exist, ebba falls back to `%APPDATA%\ebba\config.yaml`.

## Keybindings

### Linux profile

- Save: `Ctrl+S`
- Help: `Ctrl+H`, `Alt+H`
- Quit: `Ctrl+Q`, `Alt+Q`, `F10`
- Force quit: `Ctrl+Alt+Q`, `Alt+Shift+Q`, `Ctrl+Shift+Q`, `Ctrl+G`, `F12`
- Copy/Cut/Paste: `Ctrl+C`, `Ctrl+X`, `Ctrl+V`
- Terminal clipboard copy/paste: `Ctrl+Shift+C`, `Ctrl+Shift+V`
- Select all: `Ctrl+A`
- Undo/Redo: `Ctrl+Z`, `Ctrl+Y`, `Ctrl+Shift+Z`
- Word move: `Ctrl+←`, `Ctrl+→`
- Line move: `Home`, `End`
- Document move: `Ctrl+Home`, `Ctrl+End`
- Delete previous word: `Ctrl+Backspace`
- Delete to line start: `Ctrl+U`
- Toggle BOM: `Ctrl+B`, `Alt+B`, `Ctrl+Shift+B`
- Toggle tab width (2 → 4 → 8): `Ctrl+T`
- Toggle wrap: `Ctrl+W`
- Toggle invisibles: `Ctrl+K`, `Alt+I`

### Linux console profile

Auto-selected on Linux virtual consoles (`TERM=linux`).  
Adds function-key fallbacks and avoids desktop-terminal-specific assumptions.

- Save: `F2`, `Ctrl+S`
- Help: `F1`, `Ctrl+H`
- Quit: `F10`, `Ctrl+Q`
- Force quit: `F12`, `Ctrl+Alt+Q`, `Ctrl+Shift+Q`, `Ctrl+G`
- Copy/Cut/Paste: `Ctrl+C`, `Ctrl+X`, `Ctrl+V`
- `Ctrl+C`/`Ctrl+X` on a caret (no selection) copies/cuts the whole current line
- Select all: `Ctrl+A`
- Toggle selection mode: `Ctrl+Space` (while on, move keys extend selection)
- Undo/Redo: `Ctrl+Z`, `Ctrl+Y`, `Ctrl+Shift+Z`
- Word move: `Ctrl+←`, `Ctrl+→`
- Line move: `Home`, `End`
- Document move: `Ctrl+Home`, `Ctrl+End`
- Delete previous word: `Ctrl+Backspace`
- Delete to line start: `Ctrl+U`
- Toggle BOM: `Ctrl+B`
- Toggle tab width (2 → 4 → 8): `Ctrl+T`
- Toggle wrap: `Ctrl+W`
- Toggle invisibles: `Ctrl+K`

### Windows profile

Targeted for modern VT-capable terminals (for example Windows Terminal).  
Terminal-level copy/paste (`Ctrl+Shift+C/V`) depends on terminal settings.

- Save: `Ctrl+S`
- Help: `F1`, `Ctrl+H`
- Quit: `Ctrl+Q`, `F10`
- Force quit: `Ctrl+Alt+Q`, `Ctrl+Shift+Q`, `Ctrl+G`, `F12`
- Copy/Cut/Paste: `Ctrl+C`, `Ctrl+X`, `Ctrl+V`
- Terminal clipboard copy/paste: `Ctrl+Shift+C`, `Ctrl+Shift+V`
- Select all: `Ctrl+A`
- Undo/Redo: `Ctrl+Z`, `Ctrl+Y`, `Ctrl+Shift+Z`
- Word move: `Ctrl+←`, `Ctrl+→`
- Line move: `Home`, `End`
- Document move: `Ctrl+Home`, `Ctrl+End`
- Delete previous word: `Ctrl+Backspace`
- Delete to line start: `Ctrl+U`
- Toggle BOM: `Ctrl+B`
- Toggle tab width (2 → 4 → 8): `Ctrl+T`
- Toggle wrap: `Ctrl+W`
- Toggle invisibles: `Ctrl+K`

### macOS profile

`ebba` auto-detects macOS and switches to a macOS-oriented profile.
Terminal apps on macOS may intercept some `⌘` keys before `ebba` receives them. Control fallbacks remain available for all core actions.

- Save: `⌘S`, fallback `Ctrl+S`
- Help: `⇧⌘?`, fallback `Ctrl+H`
- Quit: `⌘Q`, fallback `Ctrl+Q`, `F10`
- Force quit: `Ctrl+Alt+Q`, `Ctrl+Shift+Q`, `Ctrl+G`, `F12`
- Copy/Cut/Paste: `⌘C`, `⌘X`, `⌘V` (plus `Ctrl+C/X/V`)
- Select all: `⌘A` (plus `Ctrl+A`)
- Undo/Redo: `⌘Z`, `⇧⌘Z` (plus `Ctrl+Y`)
- Word move: `⌥←`, `⌥→` (plus `Ctrl+←`, `Ctrl+→`)
- Line move: `⌘←`, `⌘→`
- Document move: `⌘↑`, `⌘↓` (plus `Ctrl+Home`, `Ctrl+End`)
- Delete previous word: `⌥Backspace` (plus `Ctrl+Backspace`)
- Delete to line start: `⌘Backspace` (plus `Ctrl+U`)
- Editor toggles (editor-specific): `Ctrl+B` BOM, `Ctrl+T` tab width, `Ctrl+W` wrap, `Ctrl+K` invisibles


### Shared editing and selection

- Insert newline: `Enter`
- Backspace: `Backspace`
- Delete forward: `Delete` (`fn+Backspace` on many Mac keyboards)
- Insert tab / indent selection: `Tab`
- Outdent selection: `Shift+Tab`
- Extend selection: add `Shift` to movement keys (arrows, page keys, word/document jumps)
- Linux console fallback: toggle selection mode with `Ctrl+Space`, then move with arrows/page/home/end

### Hex mode notes

- Opened via binary detection or `--binary`.
- Read-only.
- Scroll uses: `↑`, `↓`, `PageUp`, `PageDown`, `Home`, `End`.
- Quit also accepts plain `q`/`Q` in addition to global quit keys.

## License

GPL-2.0-only. See `LICENSE`.
