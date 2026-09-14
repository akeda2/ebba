# EBBA

`ebba` is a minimal terminal editor.

Ebba is inspired by all the things i like about all sorts of different editors: **fresh**, **ne**, **nano**, **edit**, **eb**, **notepad** etc. It is a minimal implementation of a terminal style editor with modern navigation and clipboard-style shortcuts.

## Features
### Modern style clipboarding 
- Line select mode: `ctrl+c/v/x/a` without selecting copies the whole line.
- Standard select: `shift+arrows`, `ctrl+c/v/x/a etc.
- `Select-mode` - `F3/Ctrl+space/Alt+s`, toggles on/off, `arrows` select, `ctrl+c/v/x` works as usual.
- Terminal copy/paste: `ctrl+shift+c/v`

### Navigation
- Arrow key/pgup/pgdn/home/end navigation.

### Design choices
- Line-wrap, with up/down-navigation.
- Mandatory line numbering.
- Multiple keybindings for each function for compatibility.
- Multi-line tab indentation: `select` then `tab`.
- Multiple keybindings for exiting.
- Hotkey for showing invisible characters and line endings.
- Read only hex mode fallback for binary files.
- Cut/copy-guard. Repeated cut/copy requires extra keypress.
- No dialogs. Will not exit (except force-quit) with unsaved changes.
- Condensed help always visible on startup, `ctrl+h/F1` for full.

### Few colour choices
- Background: Dark gray/Light gray(inverted)/Black/Blue.
- Text: Default/Green/Amber (Light gray background is black text only).

## Installation

## Easy: Build with project script

```bash
./inst.sh
```
## Cargo
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

`inst.sh` currently runs `cargo install --path .` for user install, then sudo-copies `target/release/ebba` to `/usr/local/bin/ebba` for all users (mostly for sudo), and does optional `gb` integration if available.

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
ebba README.md -w 80 -c -i
```

## Startup options

- `FILE`  
  Path to the file to open.
- `-e, --encoding <ENCODING>`  Save encoding override: `utf-8`, `utf-8-bom`, `utf-16le-bom`, `utf-16be-bom`.
- `-l, --line-ending <lf|cr|crlf>`  Force line endings on save (otherwise preserve mode).
- `-t, --text`  Force text startup mode.
- `-b, --binary`  Force binary fallback mode (read-only hex view).
- `-w, --wrap [COLUMN]`  Enable wrapping; optional fixed wrap column (for example `--wrap 80`). Without `COLUMN`, wrap uses the available text width in the viewport.
- `-c, --center`  Center wrapped text after the gutter and enable wrapping. With bare `--center` (or `--wrap --center`), ebba auto-selects a wrap width from the longest line (capped at 140). For empty files it falls back to viewport width (capped at 120), and the final width is clamped to the available viewport text width.
- `-i, --invisibles`  Show invisible characters (space `·`, LF `␊`, CR `␍`, CRLF `␍␊`).
- `-C, --config <PATH>`  Load YAML config from explicit path.
- `-k, --keymap <auto|mac|linux|linux-console|windows>`  Force keybinding profile at startup (useful for cross-platform keymap testing).

`--text` and `--binary` are mutually exclusive.

## Line endings

- **Navigation & rendering**: `\n`, `\r\n`, and bare `\r` (old Mac-style) are all treated as line breaks for cursor movement, selection, indent/outdent, and rendering — including files with mixed endings. The underlying bytes are left untouched (see Preserve below), so this only affects how the buffer is navigated/displayed, not what's stored.
- **Typing/paste normalization**: whenever text you type (`Enter`) or paste contains a newline, it is rewritten to match the file's own dominant line ending before insertion. Pasting `\n`-terminated text into a CRLF file produces `\r\n`; pasting terminal-supplied bare `\r` (a known quirk of some terminals, e.g. Windows Terminal under WSL) into an LF file produces `\n`. This keeps a consistently-ended file from becoming mixed by a paste.
- **Status bar indicator**: shows `LF`, `CRLF`, `CR`, or `MIXED`, recomputed live from the current buffer content (not just a snapshot from when the file was opened), so it always reflects the real state of the document as you edit.
- **Save modes** (`-l, --line-ending <lf|cr|crlf>` or `default_line_ending` in config):
  - Default is **Preserve**: bytes are saved exactly as they are in the buffer.
  - `lf`/`cr`/`crlf` **forces** the whole buffer onto that convention immediately on load (not only at save time), converting any `\r\n`/bare `\r`/`\n` accordingly, so the in-editor buffer and status indicator match the forced mode right away; saving simply persists that already-normalized content.

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
- Toggle selection mode: `F3`, `Ctrl+Space`
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
- Help: `F1`, `Alt+H` (`Ctrl+H` is terminal-dependent in Linux console)
- Quit: `F10`, `Ctrl+Q`
- Force quit: `F12`, `Ctrl+Alt+Q`, `Ctrl+Shift+Q`, `Ctrl+G`
- Copy/Cut/Paste: `Ctrl+C`, `Ctrl+X`, `Ctrl+V`
- `Ctrl+C`/`Ctrl+X` on a caret (no selection) copies/cuts the whole current line
- Select all: `Ctrl+A`
- Toggle selection mode: `F3`, `Ctrl+Space` (while on, move keys extend selection)
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
- Toggle selection mode: `F3`, `Ctrl+Space`
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
- Toggle selection mode: `F3`, `Ctrl+Space` (terminal-dependent on macOS)
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
