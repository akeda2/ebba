#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusLine {
    pub filename: String,
    pub dirty: bool,
    pub read_only: bool,
    pub line: usize,
    pub column: usize,
    pub total_lines: usize,
    pub encoding: String,
    pub line_ending: String,
    pub wrap_column: Option<usize>,
    pub message: Option<String>,
}

impl Default for StatusLine {
    fn default() -> Self {
        Self {
            filename: String::from("[No Name]"),
            dirty: false,
            read_only: false,
            line: 1,
            column: 1,
            total_lines: 1,
            encoding: String::from("utf-8"),
            line_ending: String::from("LF"),
            wrap_column: None,
            message: Some(String::from("ready")),
        }
    }
}

impl StatusLine {
    pub fn with_position(mut self, line: usize, column: usize, total_lines: usize) -> Self {
        self.line = line.max(1);
        self.column = column.max(1);
        self.total_lines = total_lines.max(1);
        self
    }

    pub fn render(&self, width: usize) -> String {
        let modified = if self.dirty { "*" } else { "-" };
        let access = if self.read_only { "RO" } else { "RW" };
        let wrap = match self.wrap_column {
            None => String::from("WRAP:OFF"),
            Some(0) => String::from("WRAP:ON"),
            Some(column) => format!("WRAP:{column}"),
        };
        let left = format!(
            "{} [{}|{}] Ln {}, Col {} / {} {} {} {}",
            self.filename,
            modified,
            access,
            self.line,
            self.column,
            self.total_lines,
            wrap,
            self.encoding,
            self.line_ending
        );
        let message = self.message.as_deref().unwrap_or("");

        if message.is_empty() {
            return fit_to_width(&left, width);
        }

        let space_for_message = width.saturating_sub(left.len() + 1);
        if space_for_message == 0 {
            return fit_to_width(&left, width);
        }

        let rendered_message = truncate(message, space_for_message);
        let mut line = String::new();
        line.push_str(&left);
        line.push(' ');
        line.push_str(&rendered_message);
        fit_to_width(&line, width)
    }
}

fn truncate(input: &str, width: usize) -> String {
    input.chars().take(width).collect()
}

fn fit_to_width(input: &str, width: usize) -> String {
    let rendered = truncate(input, width);
    if rendered.len() >= width {
        rendered
    } else {
        format!("{rendered:<width$}")
    }
}
