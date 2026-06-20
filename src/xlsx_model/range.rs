use crate::{CliError, CliResult};

pub(crate) fn range_contains_cell(bounds: RangeBounds, col: u32, row: u32) -> bool {
    col >= bounds.min_col()
        && col <= bounds.max_col()
        && row >= bounds.min_row()
        && row <= bounds.max_row()
}

#[derive(Clone, Copy)]
pub(crate) struct RangeBounds {
    pub(crate) start_col: u32,
    pub(crate) start_row: u32,
    pub(crate) end_col: u32,
    pub(crate) end_row: u32,
}

impl RangeBounds {
    pub(crate) fn min_col(self) -> u32 {
        self.start_col.min(self.end_col)
    }

    pub(crate) fn max_col(self) -> u32 {
        self.start_col.max(self.end_col)
    }

    pub(crate) fn min_row(self) -> u32 {
        self.start_row.min(self.end_row)
    }

    pub(crate) fn max_row(self) -> u32 {
        self.start_row.max(self.end_row)
    }

    pub(crate) fn row_count(self) -> u32 {
        self.max_row() - self.min_row() + 1
    }

    pub(crate) fn col_count(self) -> u32 {
        self.max_col() - self.min_col() + 1
    }

    pub(crate) fn normalized(self) -> RangeBounds {
        RangeBounds {
            start_col: self.min_col(),
            start_row: self.min_row(),
            end_col: self.max_col(),
            end_row: self.max_row(),
        }
    }
}

pub(crate) fn parse_range(range: &str) -> CliResult<RangeBounds> {
    let range = range.trim();
    if range.is_empty() {
        return Err(CliError::invalid_args("range reference cannot be empty"));
    }
    let parts = range.split(':').collect::<Vec<_>>();
    if parts.len() > 2 {
        return Err(CliError::invalid_args(format!(
            "invalid range reference {range:?}"
        )));
    }
    let start = parts[0];
    let end = parts.get(1).copied().unwrap_or(start);
    if parts.len() == 2 && end.trim().is_empty() {
        return Err(CliError::invalid_args("range end cannot be empty"));
    }
    let (start_col, start_row) = parse_cell_ref(start)
        .map_err(|err| CliError::invalid_args(format!("invalid range start: {}", err.message)))?;
    let (end_col, end_row) = parse_cell_ref(end)
        .map_err(|err| CliError::invalid_args(format!("invalid range end: {}", err.message)))?;
    Ok(RangeBounds {
        start_col,
        start_row,
        end_col,
        end_row,
    })
}

pub(crate) fn parse_cli_range(range: &str) -> CliResult<RangeBounds> {
    parse_range(range)
        .map_err(|err| CliError::invalid_args(format!("invalid --range: {}", err.message)))
}

pub(crate) fn normalize_xlsx_cell_ref(value: &str, flag: &str) -> CliResult<String> {
    let (col, row) = parse_cell_ref(value)
        .map_err(|err| CliError::invalid_args(format!("invalid {flag}: {}", err.message)))?;
    Ok(format!("{}{}", col_name(col), row))
}

pub(crate) fn parse_cell_ref(cell: &str) -> CliResult<(u32, u32)> {
    let cell = cell.trim();
    if cell.is_empty() {
        return Err(CliError::invalid_args("cell reference cannot be empty"));
    }
    let mut rest = cell;
    if let Some(after_abs_col) = rest.strip_prefix('$') {
        rest = after_abs_col;
        if rest.is_empty() {
            return Err(CliError::invalid_args("missing column in cell reference"));
        }
    }
    let col_len = rest
        .bytes()
        .take_while(|byte| byte.is_ascii_alphabetic())
        .count();
    if col_len == 0 {
        return Err(CliError::invalid_args("missing column in cell reference"));
    }
    let mut col = 0u32;
    for ch in rest[..col_len].chars() {
        col = col * 26 + (ch.to_ascii_uppercase() as u32 - 'A' as u32 + 1);
        if col > 16_384 {
            return Err(CliError::invalid_args(format!(
                "column {:?} out of XLSX bounds A-XFD",
                &rest[..col_len]
            )));
        }
    }
    rest = &rest[col_len..];
    if rest.is_empty() {
        return Err(CliError::invalid_args("missing row in cell reference"));
    }
    if let Some(after_abs_row) = rest.strip_prefix('$') {
        rest = after_abs_row;
        if rest.is_empty() {
            return Err(CliError::invalid_args("missing row in cell reference"));
        }
    }
    if rest.contains('$') {
        return Err(CliError::invalid_args(
            "invalid absolute marker in row reference",
        ));
    }
    if !rest.chars().all(|ch| ch.is_ascii_digit()) {
        return Err(CliError::invalid_args(format!(
            "invalid row {rest:?} in cell reference"
        )));
    }
    let row = rest
        .parse::<u32>()
        .map_err(|err| CliError::invalid_args(format!("invalid row {rest:?}: {err}")))?;
    if row == 0 || row > 1_048_576 {
        return Err(CliError::invalid_args(format!(
            "row {row} out of XLSX bounds 1-1048576"
        )));
    }
    Ok((col, row))
}

pub(crate) fn col_name(mut col: u32) -> String {
    let mut chars = Vec::new();
    while col > 0 {
        col -= 1;
        chars.push((b'A' + (col % 26) as u8) as char);
        col /= 26;
    }
    chars.iter().rev().collect()
}
