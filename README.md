# ebba

`ebba` is a **minimal terminal editor** for text/binary file workflows.  
Current scope is Linux-first and intentionally small.

## MVP scope and constraints

- Linux-first development/testing target for now.
- One file per process (`FILE` positional argument is singular; no multi-buffer/tabs in this MVP).
- Startup mode selection for text vs binary-oriented views.
- Core editing primitives and rendering components are implemented/tested; app wiring remains intentionally minimal.

## Run

```bash
cargo run -- [FILE] [--encoding ENCODING] [--line-ending <lf|crlf>] [--text|--binary] [--wrap [COLUMN]] [--invisibles] [--config PATH]
```

`--text` and `--binary` are mutually exclusive.

## Implemented CLI options

- `--encoding ENCODING`  
  Save-encoding override. Accepted labels: `utf-8`, `utf-8-bom`, `utf-16le` (`utf-16le-bom`), `utf-16be` (`utf-16be-bom`).
- `--line-ending <lf|crlf>`  
  Save line-ending override.
- `--text`  
  Force text startup path; if bytes are not safely decodable, fallback is byte-preserving text mode.
- `--binary`  
  Force hex read-only startup mode.
- `--config PATH`  
  Load YAML config from an explicit path.
- `--wrap [COLUMN]`  
  Enable line wrapping at startup (default is off).  
  Example: `--wrap 80` wraps at 80 text columns (line-number gutter excluded).
- `--invisibles`  
  Show invisible characters at startup (spaces as `·`, LF as `␊`, CRLF as `␍`).

## Rendering behavior in MVP

- A status line is rendered on the **top row**.
- Text rendering always includes a **mandatory line-number gutter**.

## Shortcuts and quit behavior

Current input mapping includes:

- `Ctrl+S` save
- `Ctrl+Q`, `Alt+Q`, or `F10` quit
- `Ctrl+Alt+Q`, `Alt+Shift+Q`, `Ctrl+Shift+Q`, `Ctrl+G`, or `F12` force-quit
- `Ctrl+W` toggle line wrap on/off
- `Ctrl+K` or `Alt+I` toggle invisible characters on/off
- `Ctrl+C` copy, `Ctrl+X` cut, `Ctrl+V` paste, `Ctrl+A` select-all
- `Ctrl+Z` undo, `Ctrl+Y` / `Ctrl+Shift+Z` redo

Quit refuses to exit on unsaved changes; force-quit exits immediately.

## Encoding/binary behavior

Startup detection supports:

- decoded text (`utf-8`, `utf-8-bom`, `utf-16le`, `utf-16be` when decodable)
- byte-preserving fallback text for uncertain/non-UTF8 data
- hex read-only mode for binary-classified data (or when forced via `--binary`)

When no conversion override is requested, unknown 8-bit content is saved in preserve-bytes mode.

## Large-file and confirmation notes

- Startup policy can return “requires confirmation” for:
  - large files above the MVP threshold (64 MiB in app startup policy)
  - non-resynchronizable encodings (UTF-16 BOM variants)
- Current app startup surfaces this decision via startup messages; interactive confirmation UX is not wired yet.

## Configuration

`EditorConfig::load` resolves config paths in this order:

1. explicit `--config PATH` override
2. `$XDG_CONFIG_HOME/ebba/config.yaml`
3. `~/.config/ebba/config.yaml`

Missing config file => defaults. Invalid YAML/values => typed errors.

> Note: current `app::run` only applies config when `--config` is passed.

YAML structure overview:

```yaml
indentation:
  tab_width: 4
  indent_width: 4
  use_tabs: false
default_line_ending: lf # lf|crlf
theme:
  foreground: "#d4d4d4"
  background: "#1e1e1e"
keybindings:
  save: "Ctrl+S"
  quit: "Ctrl+Q"
```

## License

This project is licensed under **GPL-2.0-only** (`LICENSE`, SPDX: `GPL-2.0-only`).
