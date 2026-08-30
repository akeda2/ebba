use crate::input::KeybindingProfile;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelpMode {
    Text,
    Hex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelpDetail {
    Compact,
    Full,
}

#[derive(Debug, Clone, Copy)]
struct HelpItem {
    label: &'static str,
    bindings: &'static str,
    priority: u8,
}

pub fn startup_help_text(
    profile: KeybindingProfile,
    mode: HelpMode,
    detail: HelpDetail,
    width: usize,
) -> String {
    let width = width.max(1);
    if detail == HelpDetail::Full {
        return format_items_full(&help_items(profile, mode), width).join("\n");
    }

    let mut items = help_items(profile, mode);
    let max_lines = match mode {
        HelpMode::Text => 4,
        HelpMode::Hex => 3,
    };
    let min_items = match mode {
        HelpMode::Text => 4,
        HelpMode::Hex => 3,
    };

    loop {
        let lines = format_items(&items, width);
        if lines.len() <= max_lines || items.len() <= min_items {
            return lines.join("\n");
        }
        let max_priority = items.iter().map(|item| item.priority).max().unwrap_or(0);
        if let Some(index) = items.iter().rposition(|item| item.priority == max_priority) {
            items.remove(index);
        } else {
            return lines.join("\n");
        }
    }
}

pub fn cli_key_bindings_help(profile: KeybindingProfile) -> String {
    let mut lines = String::from("Key bindings:\n");
    for item in help_items(profile, HelpMode::Text) {
        lines.push_str("  ");
        lines.push_str(item.label);
        lines.push_str(": ");
        lines.push_str(item.bindings);
        lines.push('\n');
    }
    lines.push_str(
        "\nExamples:\n  ebba README.md\n  ebba script.sh -w 80 -c -i\n  ebba data.bin -b",
    );
    lines
}

fn help_items(profile: KeybindingProfile, mode: HelpMode) -> Vec<HelpItem> {
    match mode {
        HelpMode::Hex => match profile {
            KeybindingProfile::MacOs => vec![
                item("Quit", "q/⌘Q/Ctrl+Q/F10", 1),
                item("Force quit", "Ctrl+Alt+Q/Ctrl+Shift+Q/Ctrl+G/F12", 1),
                item("Help", "⇧⌘?/Ctrl+H/F1/F9", 1),
                item("Scroll", "↑/↓/PgUp/PgDn/Home/End", 2),
            ],
            KeybindingProfile::Linux => vec![
                item("Quit", "q/Ctrl+Q/Alt+Q/F10", 1),
                item("Force quit", "Ctrl+Alt+Q/Alt+Shift+Q/Ctrl+Shift+Q/Ctrl+G/F12", 1),
                item("Help", "Ctrl+H/Alt+H/F1/F9", 1),
                item("Scroll", "↑/↓/PgUp/PgDn/Home/End", 2),
            ],
            KeybindingProfile::LinuxConsole => vec![
                item("Quit", "q/Ctrl+Q/F10", 1),
                item("Force quit", "Ctrl+Alt+Q/Ctrl+Shift+Q/Ctrl+G/F12", 1),
                item("Help", "Ctrl+H/Alt+H/F1/F9", 1),
                item("Scroll", "↑/↓/PgUp/PgDn/Home/End", 2),
            ],
            KeybindingProfile::Windows => vec![
                item("Quit", "q/Ctrl+Q/F10", 1),
                item("Force quit", "Ctrl+Alt+Q/Ctrl+Shift+Q/Ctrl+G/F12", 1),
                item("Help", "Ctrl+H/F1/F9", 1),
                item("Scroll", "↑/↓/PgUp/PgDn/Home/End", 2),
            ],
        },
        HelpMode::Text => match profile {
            KeybindingProfile::MacOs => vec![
                item("Save", "⌘S/Ctrl+S/F2", 1),
                item("Help", "⇧⌘?/Ctrl+H/F1/F9", 1),
                item("Quit", "⌘Q/Ctrl+Q/F10", 1),
                item("Force quit", "Ctrl+Alt+Q/Ctrl+Shift+Q/Ctrl+G/F12", 1),
                item("Clipboard", "⌘C/X/V, Ctrl+C/X/V, Ctrl+Shift+C/V", 2),
                item("Select", "⌘A, Shift+Arrows/PgUp/PgDn, Shift+⌥←→/⌘←→/⌘↑↓", 2),
                item("Select-mode", "F3/Ctrl+Space", 2),
                item("Undo", "⌘Z/Ctrl+Z", 2),
                item("Redo", "⇧⌘Z/Ctrl+Y", 2),
                item("Toggle BOM", "Ctrl+B/F8", 3),
                item("Tab width", "Ctrl+T/F5", 3),
                item("Hard tabs", "Ctrl+Shift+T/Ctrl+Alt+T/Ctrl+Shift+H/F4", 3),
                item("Toggle wrap", "Ctrl+W/F7", 3),
                item("Toggle invisibles", "Ctrl+K/Ctrl+./Alt+K/F6", 3),
                item("Move", "Arrows/Home/End/⌥←→/⌘←→/⌘↑↓/Ctrl+Home/End/PgUp/PgDn", 4),
                item("Delete", "⌥Backspace/⌘Backspace/Ctrl+Backspace/Ctrl+U", 4),
            ],
            KeybindingProfile::Linux => vec![
                item("Save", "Ctrl+S/F2", 1),
                item("Help", "Ctrl+H/Alt+H/F1/F9", 1),
                item("Quit", "Ctrl+Q/Alt+Q/F10", 1),
                item("Force quit", "Ctrl+Alt+Q/Alt+Shift+Q/Ctrl+Shift+Q/Ctrl+G/F12", 1),
                item("Clipboard", "Ctrl+C/X/V, Ctrl+Shift+C/V (terminal)", 2),
                item("Select", "Ctrl+A, Shift+Arrows/PgUp/PgDn", 2),
                item("Select-mode", "F3/Ctrl+Space/Alt+S", 2),
                item("Undo", "Ctrl+Z", 2),
                item("Redo", "Ctrl+Y/Ctrl+Shift+Z", 2),
                item("Toggle BOM", "Ctrl+B/Alt+B/Ctrl+Shift+B/F8", 3),
                item("Tab width", "Ctrl+T/F5", 3),
                item(
                    "Hard tabs",
                    "Ctrl+Shift+T/Ctrl+Alt+T/Alt+Shift+T/Ctrl+Shift+H/F4",
                    3,
                ),
                item("Toggle wrap", "Ctrl+W/F7", 3),
                item(
                    "Toggle invisibles",
                    "Ctrl+K/Ctrl+./Alt+I/Alt+./Alt+K/F6",
                    3,
                ),
                item("Move", "Arrows/Home/End/Ctrl+←→/Ctrl+Home/End/PgUp/PgDn", 4),
                item("Delete", "Ctrl+Backspace/Ctrl+U", 4),
            ],
            KeybindingProfile::LinuxConsole => vec![
                item("Save", "F2/Ctrl+S", 1),
                item("Help", "Ctrl+H/Alt+H/F1/F9", 1),
                item("Quit", "Ctrl+Q/F10", 1),
                item("Force quit", "Ctrl+Alt+Q/Ctrl+Shift+Q/Ctrl+G/F12", 1),
                item("Clipboard", "Ctrl+C/X/V/Ctrl+A (line fallback on caret)", 2),
                item("Select", "Ctrl+A, Shift+Arrows/PgUp/PgDn", 2),
                item("Select-mode", "F3/Ctrl+Space/Alt+S", 2),
                item("Undo", "Ctrl+Z", 2),
                item("Redo", "Ctrl+Y/Ctrl+Shift+Z", 2),
                item("Toggle BOM", "Ctrl+B/F8", 3),
                item("Tab width", "Ctrl+T/F5", 3),
                item(
                    "Hard tabs",
                    "Ctrl+Shift+T/Ctrl+Alt+T/Alt+Shift+T/Ctrl+Shift+H/F4",
                    3,
                ),
                item("Toggle wrap", "Ctrl+W/F7", 3),
                item(
                    "Toggle invisibles",
                    "Ctrl+K/Ctrl+./Alt+K/Alt+./F6",
                    3,
                ),
                item("Move", "Arrows/Home/End/Ctrl+←→/Ctrl+Home/End/PgUp/PgDn", 4),
                item("Delete", "Ctrl+Backspace/Ctrl+U", 4),
            ],
            KeybindingProfile::Windows => vec![
                item("Save", "Ctrl+S/F2", 1),
                item("Help", "Ctrl+H/F1/F9", 1),
                item("Quit", "Ctrl+Q/F10", 1),
                item("Force quit", "Ctrl+Alt+Q/Ctrl+Shift+Q/Ctrl+G/F12", 1),
                item("Clipboard", "Ctrl+C/X/V, Ctrl+Shift+C/V", 2),
                item("Select", "Ctrl+A, Shift+Arrows/PgUp/PgDn", 2),
                item("Select-mode", "F3/Ctrl+Space/Alt+S", 2),
                item("Undo", "Ctrl+Z", 2),
                item("Redo", "Ctrl+Y/Ctrl+Shift+Z", 2),
                item("Toggle BOM", "Ctrl+B/F8", 3),
                item("Tab width", "Ctrl+T/F5", 3),
                item(
                    "Hard tabs",
                    "Ctrl+Shift+T/Ctrl+Alt+T/Alt+Shift+T/Ctrl+Shift+H/F4",
                    3,
                ),
                item("Toggle wrap", "Ctrl+W/F7", 3),
                item("Toggle invisibles", "Ctrl+K/Ctrl+./Alt+K/F6", 3),
                item("Move", "Arrows/Home/End/Ctrl+←→/Ctrl+Home/End/PgUp/PgDn", 4),
                item("Delete", "Ctrl+Backspace/Ctrl+U", 4),
            ],
        },
    }
}

fn item(label: &'static str, bindings: &'static str, priority: u8) -> HelpItem {
    HelpItem {
        label,
        bindings,
        priority,
    }
}

fn format_items(items: &[HelpItem], width: usize) -> Vec<String> {
    if items.is_empty() {
        return Vec::new();
    }
    let max_width = width.max(1);
    let segments: Vec<String> = items
        .iter()
        .map(|item| format!("{}: {}", item.label, item.bindings))
        .collect();

    for cols in (1..=segments.len()).rev() {
        if let Some(lines) = format_into_columns(&segments, cols, max_width) {
            return lines;
        }
    }
    segments
        .iter()
        .map(|segment| truncate_to_width(segment, max_width))
        .collect()
}

fn format_items_full(items: &[HelpItem], width: usize) -> Vec<String> {
    if items.is_empty() {
        return Vec::new();
    }
    let segments: Vec<String> = items
        .iter()
        .map(|item| format!("{}: {}", item.label, item.bindings))
        .collect();

    for cols in (1..=segments.len()).rev() {
        if let Some(lines) = format_into_columns(&segments, cols, width) {
            return lines;
        }
    }

    let mut lines = Vec::new();
    for segment in &segments {
        lines.extend(wrap_to_width(segment, width));
    }
    lines
}

fn format_into_columns(segments: &[String], cols: usize, width: usize) -> Option<Vec<String>> {
    let rows = segments.len().div_ceil(cols);
    let mut col_widths = vec![0usize; cols];

    for row in 0..rows {
        for (col, col_width) in col_widths.iter_mut().enumerate() {
            let idx = row * cols + col;
            if idx >= segments.len() {
                break;
            }
            *col_width = (*col_width).max(segments[idx].chars().count());
        }
    }

    let total_width: usize = col_widths.iter().sum::<usize>() + (cols.saturating_sub(1) * 2);
    if total_width > width {
        return None;
    }

    let mut lines = Vec::with_capacity(rows);
    for row in 0..rows {
        let mut line = String::new();
        for (col, col_width) in col_widths.iter().enumerate() {
            let idx = row * cols + col;
            if idx >= segments.len() {
                break;
            }
            let segment = &segments[idx];
            line.push_str(segment);
            if col + 1 < cols && (row * cols + col + 1) < segments.len() {
                let padding = col_width.saturating_sub(segment.chars().count()) + 2;
                line.push_str(&" ".repeat(padding));
            }
        }
        lines.push(line);
    }
    Some(lines)
}

fn truncate_to_width(input: &str, width: usize) -> String {
    input.chars().take(width).collect()
}

fn wrap_to_width(input: &str, width: usize) -> Vec<String> {
    let chars: Vec<char> = input.chars().collect();
    if chars.is_empty() {
        return vec![String::new()];
    }
    chars
        .chunks(width.max(1))
        .map(|chunk| chunk.iter().collect())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{HelpDetail, HelpMode, cli_key_bindings_help, startup_help_text};
    use crate::input::KeybindingProfile;

    #[test]
    fn startup_help_overflows_to_multiple_rows_dynamically() {
        let text = startup_help_text(
            KeybindingProfile::Linux,
            HelpMode::Text,
            HelpDetail::Compact,
            50,
        );
        assert!(text.contains('\n'));
        assert!(text.contains("Save: Ctrl+S"));
        assert!(!text.contains(" • "));
    }

    #[test]
    fn startup_help_keeps_essentials_when_width_is_tiny() {
        let text = startup_help_text(
            KeybindingProfile::Linux,
            HelpMode::Text,
            HelpDetail::Compact,
            24,
        );
        assert!(text.contains("Save: Ctrl+S"));
        assert!(text.contains("Quit: Ctrl+Q/Alt+Q/F10"));
        assert!(text.contains("Force quit:"));
    }

    #[test]
    fn full_help_keeps_low_priority_sections_on_narrow_width() {
        let text = startup_help_text(
            KeybindingProfile::Linux,
            HelpMode::Text,
            HelpDetail::Full,
            24,
        );
        assert!(text.contains("Hard tabs:"));
        assert!(text.contains("Delete:"));
    }

    #[test]
    fn cli_help_is_built_from_structured_items() {
        let text = cli_key_bindings_help(KeybindingProfile::Linux);
        assert!(text.contains("Key bindings:"));
        assert!(text.contains("Hard tabs:"));
        assert!(text.contains("Examples:"));
    }
}
