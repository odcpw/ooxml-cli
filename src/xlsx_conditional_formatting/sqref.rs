use crate::{CliError, CliResult};

#[derive(Clone, Copy)]
struct SqrefCell {
    col: u32,
    row: u32,
    abs_col: bool,
    abs_row: bool,
}

pub(super) fn normalize_sqref(value: &str) -> CliResult<String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(CliError::invalid_args("range cannot be empty"));
    }
    value
        .split_whitespace()
        .map(normalize_sqref_part)
        .collect::<CliResult<Vec<_>>>()
        .map(|parts| parts.join(" "))
}

fn normalize_sqref_part(value: &str) -> CliResult<String> {
    if value.contains(':') {
        let range = parse_sqref_range(value)?;
        if range.0.render() == range.1.render() {
            Ok(range.0.render())
        } else {
            Ok(format!("{}:{}", range.0.render(), range.1.render()))
        }
    } else {
        parse_sqref_cell(value).map(|cell| cell.render())
    }
}

fn parse_sqref_range(value: &str) -> CliResult<(SqrefCell, SqrefCell)> {
    let value = value.trim();
    if value.is_empty() {
        return Err(CliError::invalid_args("range reference cannot be empty"));
    }
    let parts = value.split(':').collect::<Vec<_>>();
    if parts.len() > 2 {
        return Err(CliError::invalid_args(format!(
            "invalid range reference {value:?}"
        )));
    }
    let start = parse_sqref_cell(parts[0])
        .map_err(|err| CliError::invalid_args(format!("invalid range start: {}", err.message)))?;
    let end = if let Some(end) = parts.get(1) {
        if end.trim().is_empty() {
            return Err(CliError::invalid_args("range end cannot be empty"));
        }
        parse_sqref_cell(end)
            .map_err(|err| CliError::invalid_args(format!("invalid range end: {}", err.message)))?
    } else {
        start
    };
    Ok((start, end))
}

fn parse_sqref_cell(value: &str) -> CliResult<SqrefCell> {
    let mut rest = value.trim();
    if rest.is_empty() {
        return Err(CliError::invalid_args("cell reference cannot be empty"));
    }
    let abs_col = rest.starts_with('$');
    if abs_col {
        rest = &rest[1..];
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
    let letters = &rest[..col_len];
    let mut col = 0u32;
    for ch in letters.chars() {
        col = col * 26 + (ch.to_ascii_uppercase() as u32 - 'A' as u32 + 1);
        if col > 16_384 {
            return Err(CliError::invalid_args(format!(
                "column {letters:?} out of XLSX bounds A-XFD"
            )));
        }
    }
    rest = &rest[col_len..];
    if rest.is_empty() {
        return Err(CliError::invalid_args("missing row in cell reference"));
    }
    let abs_row = rest.starts_with('$');
    if abs_row {
        rest = &rest[1..];
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
    Ok(SqrefCell {
        col,
        row,
        abs_col,
        abs_row,
    })
}

impl SqrefCell {
    fn render(self) -> String {
        let mut out = String::new();
        if self.abs_col {
            out.push('$');
        }
        out.push_str(&sqref_col_name(self.col));
        if self.abs_row {
            out.push('$');
        }
        out.push_str(&self.row.to_string());
        out
    }
}

fn sqref_col_name(mut col: u32) -> String {
    let mut chars = Vec::new();
    while col > 0 {
        col -= 1;
        chars.push((b'A' + (col % 26) as u8) as char);
        col /= 26;
    }
    chars.iter().rev().collect()
}

pub(super) fn sqref_cell_count(sqref: &str) -> i64 {
    let mut total = 0i64;
    for part in sqref.split_whitespace() {
        if part.contains(':')
            && let Ok((start, end)) = parse_sqref_range(part)
        {
            let cols = end.col as i64 - start.col as i64 + 1;
            let rows = end.row as i64 - start.row as i64 + 1;
            if cols > 0 && rows > 0 {
                total += cols * rows;
            }
            continue;
        }
        total += 1;
    }
    total
}
