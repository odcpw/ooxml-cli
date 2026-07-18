use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use crate::{
    CliError, CliResult, RangeBounds, chrono_like_counter, command_arg, decode_xlsx_raw_cell,
    normalize_xl_target, parse_cli_range, relationships, resolve_sheet, shared_strings_for_indices,
    sheet_raw_cells_in_range, with_zip_entry_reader, workbook_sheets, xlsx_styles, zip_entry_names,
    zip_text,
};
pub(crate) struct XlsxRangeExportOptions<'a> {
    pub(crate) include_types: bool,
    pub(crate) include_formulas: bool,
    pub(crate) include_formats: bool,
    pub(crate) data_out: Option<&'a str>,
    pub(crate) max_cells: i64,
}

pub(crate) fn xlsx_range_export_with_options(
    file: &str,
    sheet_selector: &str,
    range: &str,
    options: XlsxRangeExportOptions<'_>,
) -> CliResult<Value> {
    xlsx_range_export_with_output_limit(file, sheet_selector, range, options, 0)
}

pub(crate) fn xlsx_range_export_with_output_limit(
    file: &str,
    sheet_selector: &str,
    range: &str,
    options: XlsxRangeExportOptions<'_>,
    output_max_cells: i64,
) -> CliResult<Value> {
    let bounds = parse_cli_range(range)?;
    check_range_max_cells(range, bounds, options.max_cells)?;
    if output_max_cells < 0 {
        return Err(CliError::invalid_args("readback cell limit must be >= 0"));
    }
    if let Some(data_out) = options.data_out.filter(|data_out| !data_out.is_empty()) {
        reject_input_as_data_out(file, data_out)?;
    }

    let workbook = zip_text(file, "xl/workbook.xml")?;
    let sheets = workbook_sheets(&workbook)?;
    let sheet = resolve_sheet(&sheets, sheet_selector)?;
    let rels = relationships(file, "xl/_rels/workbook.xml.rels")?;
    let target = rels
        .get(&sheet.rel_id)
        .ok_or_else(|| CliError::unexpected(format!("missing relationship {}", sheet.rel_id)))?;
    let sheet_part = normalize_xl_target(target);
    let total_cells = u64::from(bounds.row_count()).saturating_mul(u64::from(bounds.col_count()));
    let retained_cells = if output_max_cells > 0 {
        total_cells.min(output_max_cells as u64)
    } else {
        total_cells
    };
    let truncated = retained_cells < total_cells;

    let raw_scan = with_zip_entry_reader(file, &sheet_part, |reader| {
        sheet_raw_cells_in_range(reader, bounds, Some(retained_cells))
    })?;
    let wanted_shared_strings = raw_scan
        .cells
        .values()
        .filter(|cell| cell.cell_type == "s")
        .filter_map(|cell| cell.raw_value.parse::<usize>().ok())
        .collect::<BTreeSet<_>>();
    let shared_strings = if wanted_shared_strings.is_empty()
        || !zip_entry_names(file)?
            .iter()
            .any(|entry| entry == "xl/sharedStrings.xml")
    {
        BTreeMap::new()
    } else {
        with_zip_entry_reader(file, "xl/sharedStrings.xml", |reader| {
            shared_strings_for_indices(reader, &wanted_shared_strings)
        })?
    };
    let styles = xlsx_styles(file).unwrap_or_default();
    let style_zero_has_number_format = raw_scan.saw_style_zero
        && styles.first().is_some_and(|style| {
            style.number_format_id.unwrap_or(0) != 0
                || !style
                    .number_format_code
                    .as_deref()
                    .unwrap_or_default()
                    .is_empty()
        });
    let mut has_format_readback = options.include_formats
        && (raw_scan.saw_nonzero_style_index || style_zero_has_number_format);
    let cells = raw_scan
        .cells
        .into_iter()
        .map(|(coordinate, raw)| {
            (
                coordinate,
                decode_xlsx_raw_cell(&raw, &shared_strings, &styles),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut values = Vec::new();
    let mut types = options.include_types.then(Vec::new);
    let mut formulas = options.include_formulas.then(Vec::new);
    let mut style_indexes = options.include_formats.then(Vec::new);
    let mut number_format_ids = options.include_formats.then(Vec::new);
    let mut number_format_codes = options.include_formats.then(Vec::new);
    let mut formula_count = 0;
    let mut emitted_cells = 0_u64;
    for row in bounds.min_row()..=bounds.max_row() {
        if emitted_cells >= retained_cells {
            break;
        }
        let row_cell_count = u64::from(bounds.col_count()).min(retained_cells - emitted_cells);
        let mut row_values = Vec::new();
        let mut row_types = options.include_types.then(Vec::new);
        let mut row_formulas = options.include_formulas.then(Vec::new);
        let mut row_style_indexes = options.include_formats.then(Vec::new);
        let mut row_number_format_ids = options.include_formats.then(Vec::new);
        let mut row_number_format_codes = options.include_formats.then(Vec::new);
        for col_offset in 0..row_cell_count as u32 {
            let col = bounds.min_col() + col_offset;
            if let Some(cell) = cells.get(&(col, row)) {
                if cell.has_formula {
                    formula_count += 1;
                }
                row_values.push(cell.matrix_value.clone());
                if let Some(row_types) = row_types.as_mut() {
                    row_types.push(Value::String(cell.kind.clone()));
                }
                if let Some(row_formulas) = row_formulas.as_mut() {
                    if cell.formula.is_empty() {
                        row_formulas.push(Value::Null);
                    } else {
                        row_formulas.push(Value::String(cell.formula.clone()));
                    }
                }
                if options.include_formats {
                    let style_index = cell.style_index.unwrap_or(0);
                    let number_format_id = cell.number_format_id.unwrap_or(0);
                    let number_format_code = cell.number_format_code.clone().unwrap_or_default();
                    let has_cell_format =
                        style_index != 0 || number_format_id != 0 || !number_format_code.is_empty();
                    if has_cell_format {
                        has_format_readback = true;
                        row_style_indexes
                            .as_mut()
                            .expect("format row")
                            .push(json!(style_index));
                        row_number_format_ids
                            .as_mut()
                            .expect("format row")
                            .push(json!(number_format_id));
                        if number_format_code.is_empty() {
                            row_number_format_codes
                                .as_mut()
                                .expect("format row")
                                .push(Value::Null);
                        } else {
                            row_number_format_codes
                                .as_mut()
                                .expect("format row")
                                .push(Value::String(number_format_code));
                        }
                    } else {
                        row_style_indexes
                            .as_mut()
                            .expect("format row")
                            .push(Value::Null);
                        row_number_format_ids
                            .as_mut()
                            .expect("format row")
                            .push(Value::Null);
                        row_number_format_codes
                            .as_mut()
                            .expect("format row")
                            .push(Value::Null);
                    }
                }
            } else {
                row_values.push(Value::Null);
                if let Some(row_types) = row_types.as_mut() {
                    row_types.push(Value::String("empty".to_string()));
                }
                if let Some(row_formulas) = row_formulas.as_mut() {
                    row_formulas.push(Value::Null);
                }
                if let Some(row_style_indexes) = row_style_indexes.as_mut() {
                    row_style_indexes.push(Value::Null);
                }
                if let Some(row_number_format_ids) = row_number_format_ids.as_mut() {
                    row_number_format_ids.push(Value::Null);
                }
                if let Some(row_number_format_codes) = row_number_format_codes.as_mut() {
                    row_number_format_codes.push(Value::Null);
                }
            }
        }
        values.push(Value::Array(row_values));
        if let (Some(types), Some(row_types)) = (types.as_mut(), row_types) {
            types.push(Value::Array(row_types));
        }
        if let (Some(formulas), Some(row_formulas)) = (formulas.as_mut(), row_formulas) {
            formulas.push(Value::Array(row_formulas));
        }
        if let (Some(style_indexes), Some(row_style_indexes)) =
            (style_indexes.as_mut(), row_style_indexes)
        {
            style_indexes.push(Value::Array(row_style_indexes));
        }
        if let (Some(number_format_ids), Some(row_number_format_ids)) =
            (number_format_ids.as_mut(), row_number_format_ids)
        {
            number_format_ids.push(Value::Array(row_number_format_ids));
        }
        if let (Some(number_format_codes), Some(row_number_format_codes)) =
            (number_format_codes.as_mut(), row_number_format_codes)
        {
            number_format_codes.push(Value::Array(row_number_format_codes));
        }
        emitted_cells = emitted_cells.saturating_add(row_cell_count);
    }
    let rows = bounds.row_count();
    let cols = bounds.col_count();
    let mut output = Map::new();
    output.insert(
        "cellsExtractCommand".to_string(),
        json!(format!(
            "ooxml --json xlsx cells extract {} --sheet {} --range {}",
            command_arg(file),
            command_arg(&sheet.name),
            command_arg(range)
        )),
    );
    output.insert("cols".to_string(), json!(cols));
    output.insert("dataFormat".to_string(), json!("json"));
    output.insert("file".to_string(), json!(file));
    output.insert("formulaCount".to_string(), json!(formula_count));
    output.insert("majorDimension".to_string(), json!("rows"));
    output.insert(
        "pptxPlaceTableCommandTemplate".to_string(),
        json!(format!(
            "ooxml --json pptx place table-from-xlsx deck.pptx --workbook {} --sheet {} --range {} --expect-source-range {} --slide 1 --x 0 --y 0 --cx 4000000 --out out.pptx",
            command_arg(file),
            command_arg(&sheet.name),
            command_arg(range),
            command_arg(range)
        )),
    );
    output.insert(
        "pptxReplaceTextCommandTemplate".to_string(),
        json!(format!(
            "ooxml --json pptx replace text-from-xlsx deck.pptx --workbook {} --sheet {} --range {} --slide 1 --target title --out out.pptx",
            command_arg(file),
            command_arg(&sheet.name),
            command_arg(range)
        )),
    );
    output.insert(
        "pptxUpdateTableCommandTemplate".to_string(),
        json!(format!(
            "ooxml --json pptx tables update-from-xlsx deck.pptx --workbook {} --sheet {} --range {} --expect-source-range {} --slide 1 --target table:1 --out out.pptx",
            command_arg(file),
            command_arg(&sheet.name),
            command_arg(range),
            command_arg(range)
        )),
    );
    output.insert("primarySelector".to_string(), json!(range));
    output.insert("range".to_string(), json!(range));
    output.insert("rows".to_string(), json!(rows));
    output.insert("selectors".to_string(), json!([range]));
    output.insert("sheet".to_string(), json!(sheet.name));
    output.insert("sheetNumber".to_string(), json!(sheet.position));
    output.insert("truncated".to_string(), json!(truncated));
    if let Some(types) = types {
        output.insert("types".to_string(), Value::Array(types));
    }
    if let Some(formulas) = formulas {
        output.insert("formulas".to_string(), Value::Array(formulas));
    }
    if options.include_formats && has_format_readback {
        output.insert(
            "styleIndexes".to_string(),
            Value::Array(style_indexes.expect("format matrix")),
        );
        output.insert(
            "numberFormatIds".to_string(),
            Value::Array(number_format_ids.expect("format matrix")),
        );
        output.insert(
            "numberFormatCodes".to_string(),
            Value::Array(number_format_codes.expect("format matrix")),
        );
    }
    output.insert(
        "validateCommand".to_string(),
        json!(format!("ooxml validate --strict {}", command_arg(file))),
    );
    output.insert("values".to_string(), Value::Array(values));
    if let Some(data_out) = options.data_out.filter(|data_out| !data_out.is_empty()) {
        output.insert("dataOut".to_string(), json!(data_out));
        write_data_out_json(data_out, &output)?;
        output.remove("values");
        output.remove("types");
        output.remove("formulas");
        output.remove("styleIndexes");
        output.remove("numberFormatIds");
        output.remove("numberFormatCodes");
    }
    Ok(Value::Object(output))
}

fn reject_input_as_data_out(input: &str, data_out: &str) -> CliResult<()> {
    let input = resolved_path_for_comparison(Path::new(input)).map_err(|err| {
        CliError::unexpected(format!("failed to resolve input workbook path: {err}"))
    })?;
    let data_out = resolved_path_for_comparison(Path::new(data_out))
        .map_err(|err| CliError::unexpected(format!("failed to resolve --data-out path: {err}")))?;
    if input == data_out {
        return Err(CliError::invalid_args(
            "--data-out must not resolve to the input workbook",
        ));
    }
    Ok(())
}

fn resolved_path_for_comparison(path: &Path) -> std::io::Result<PathBuf> {
    if path.exists() {
        return fs::canonicalize(path);
    }
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let file_name = path.file_name().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "path has no file name")
    })?;
    Ok(fs::canonicalize(parent)?.join(file_name))
}

fn write_data_out_json(data_out: &str, output: &Map<String, Value>) -> CliResult<()> {
    let destination = Path::new(data_out);
    let parent = destination
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let file_name = destination
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("range.json");
    let temporary = parent.join(format!(
        ".{file_name}.ooxml-data-out-{}-{}.tmp",
        std::process::id(),
        chrono_like_counter()
    ));
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|err| CliError::unexpected(format!("failed to write --data-out: {err}")))?;
    let mut writer = BufWriter::new(file);
    let write_result = (|| -> CliResult<()> {
        serde_json::to_writer(&mut writer, output)
            .map_err(|err| CliError::unexpected(format!("failed to marshal range JSON: {err}")))?;
        writer
            .write_all(b"\n")
            .map_err(|err| CliError::unexpected(format!("failed to write --data-out: {err}")))?;
        writer
            .flush()
            .map_err(|err| CliError::unexpected(format!("failed to write --data-out: {err}")))?;
        writer
            .get_ref()
            .sync_all()
            .map_err(|err| CliError::unexpected(format!("failed to write --data-out: {err}")))?;
        Ok(())
    })();
    drop(writer);
    if let Err(err) = write_result {
        let _ = fs::remove_file(&temporary);
        return Err(err);
    }
    if let Err(err) = fs::rename(&temporary, destination) {
        let _ = fs::remove_file(&temporary);
        return Err(CliError::unexpected(format!(
            "failed to write --data-out: {err}"
        )));
    }
    Ok(())
}

pub(crate) fn require_json_data_format(data_format: Option<&str>) -> CliResult<()> {
    let data_format = data_format.unwrap_or("json").trim().to_ascii_lowercase();
    if data_format.is_empty() || data_format == "json" {
        Ok(())
    } else {
        Err(CliError::invalid_args(format!(
            "unsupported Rust-port data format {data_format:?}; only json is implemented"
        )))
    }
}

pub(crate) fn normalize_xlsx_ranges_set_data_format(
    data_format: Option<&str>,
) -> CliResult<String> {
    let normalized = data_format.unwrap_or("json").trim().to_ascii_lowercase();
    match normalized.as_str() {
        "" | "json" => Ok("json".to_string()),
        "csv" => Ok("csv".to_string()),
        "tsv" => Ok("tsv".to_string()),
        _ => Err(CliError::invalid_args(format!(
            "invalid data format {data_format:?} (must be json, csv, or tsv)",
        ))),
    }
}

pub(crate) fn check_range_max_cells(
    range: &str,
    bounds: RangeBounds,
    max_cells: i64,
) -> CliResult<()> {
    if max_cells < 0 {
        return Err(CliError::invalid_args("--max-cells must be >= 0"));
    }
    let rows = i64::from(bounds.row_count());
    let cols = i64::from(bounds.col_count());
    let cell_count = rows.saturating_mul(cols);
    if max_cells > 0 && cell_count > max_cells {
        return Err(CliError::invalid_args(format!(
            "range {range} contains {cell_count} cells, above --max-cells {max_cells}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_out_rejects_the_input_path_and_symlink() {
        let temp_dir = std::env::temp_dir().join(format!(
            "ooxml-data-out-safety-{}-{}",
            std::process::id(),
            chrono_like_counter()
        ));
        fs::create_dir_all(&temp_dir).expect("temp directory");
        let input = temp_dir.join("book.xlsx");
        fs::write(&input, b"workbook").expect("input");
        let input_text = input.to_string_lossy();
        let err = reject_input_as_data_out(&input_text, &input_text)
            .expect_err("same path must be rejected");
        assert_eq!(err.code, "invalid_args");

        #[cfg(unix)]
        {
            let alias = temp_dir.join("alias.xlsx");
            std::os::unix::fs::symlink(&input, &alias).expect("symlink");
            let err = reject_input_as_data_out(&input_text, &alias.to_string_lossy())
                .expect_err("symlink to input must be rejected");
            assert_eq!(err.code, "invalid_args");
        }
        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn data_out_write_uses_sibling_temp_then_rename() {
        let temp_dir = std::env::temp_dir().join(format!(
            "ooxml-data-out-write-{}-{}",
            std::process::id(),
            chrono_like_counter()
        ));
        fs::create_dir_all(&temp_dir).expect("temp directory");
        let destination = temp_dir.join("range.json");
        let output = Map::from_iter([("values".to_string(), json!([[1, 2]]))]);
        write_data_out_json(&destination.to_string_lossy(), &output).expect("write data out");
        assert_eq!(
            fs::read_to_string(&destination).expect("data out"),
            "{\"values\":[[1,2]]}\n"
        );
        assert_eq!(fs::read_dir(&temp_dir).expect("directory").count(), 1);
        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn data_out_write_cleans_up_staging_file_when_rename_fails() {
        let temp_dir = std::env::temp_dir().join(format!(
            "ooxml-data-out-cleanup-{}-{}",
            std::process::id(),
            chrono_like_counter()
        ));
        let destination = temp_dir.join("occupied");
        fs::create_dir_all(&destination).expect("destination directory");
        let output = Map::from_iter([("values".to_string(), json!([[1]]))]);
        write_data_out_json(&destination.to_string_lossy(), &output)
            .expect_err("rename over a directory must fail");
        assert_eq!(
            fs::read_dir(&temp_dir).expect("directory").count(),
            1,
            "staging file should be removed"
        );
        let _ = fs::remove_dir_all(temp_dir);
    }
}
