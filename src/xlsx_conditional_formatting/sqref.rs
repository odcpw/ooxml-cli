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
    let reference = crate::xlsx_model::parse_a1_cell_ref(value)?;
    Ok(SqrefCell {
        col: reference.column,
        row: reference.row,
        abs_col: reference.absolute_column,
        abs_row: reference.absolute_row,
    })
}

impl SqrefCell {
    fn render(self) -> String {
        let mut out = String::new();
        if self.abs_col {
            out.push('$');
        }
        out.push_str(&crate::col_name(self.col));
        if self.abs_row {
            out.push('$');
        }
        out.push_str(&self.row.to_string());
        out
    }
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
