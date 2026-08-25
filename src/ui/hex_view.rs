#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HexViewport {
    pub first_row: usize,
    pub width: usize,
    pub height: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HexRenderOutput {
    pub lines: Vec<String>,
    pub total_rows: usize,
}

#[derive(Debug, Default)]
pub struct HexView;

impl HexView {
    pub const BYTES_PER_ROW: usize = 16;

    pub fn render(bytes: &[u8], viewport: HexViewport) -> HexRenderOutput {
        let total_rows = bytes.len().div_ceil(Self::BYTES_PER_ROW).max(1);
        let first_row = viewport
            .first_row
            .min(total_rows.saturating_sub(1))
            .min(total_rows);
        let mut lines = Vec::with_capacity(viewport.height);

        for row in 0..viewport.height {
            let absolute_row = first_row + row;
            if absolute_row >= total_rows {
                lines.push(fit_to_width("~", viewport.width));
                continue;
            }

            let row_start = absolute_row * Self::BYTES_PER_ROW;
            let row_end = (row_start + Self::BYTES_PER_ROW).min(bytes.len());
            let chunk = &bytes[row_start..row_end];
            lines.push(fit_to_width(
                &format_hex_row(row_start, chunk),
                viewport.width,
            ));
        }

        HexRenderOutput { lines, total_rows }
    }
}

pub fn format_hex_row(offset: usize, bytes: &[u8]) -> String {
    let mut hex_cells = Vec::with_capacity(HexView::BYTES_PER_ROW);
    let mut ascii = String::with_capacity(HexView::BYTES_PER_ROW);

    for index in 0..HexView::BYTES_PER_ROW {
        if let Some(byte) = bytes.get(index).copied() {
            hex_cells.push(format!("{byte:02x}"));
            ascii.push(if byte.is_ascii_graphic() || byte == b' ' {
                char::from(byte)
            } else {
                '.'
            });
        } else {
            hex_cells.push(String::from("  "));
            ascii.push(' ');
        }
    }

    format!("{offset:08x}  {}  |{}|", hex_cells.join(" "), ascii)
}

fn fit_to_width(input: &str, width: usize) -> String {
    if input.len() >= width {
        input.chars().take(width).collect()
    } else {
        format!("{input:<width$}")
    }
}
