use serde_json::{Map, Value, json};
use std::path::Path;

use crate::{
    CliError, CliResult, RangeBounds, WorkbookSheet, XlsxRangesSetFormatOptions, command_arg,
    normalize_xl_target, parse_range, range_bounds_ref, relationships, workbook_sheets,
    xlsx_ranges_set_format, xlsx_source_command,
    xlsx_tables::{XlsxTableRef, select_xlsx_table, xlsx_tables},
    zip_text,
};

pub(crate) struct XlsxTablesSetColumnFormatOptions<'a> {
    pub(crate) sheet: Option<&'a str>,
    pub(crate) table: Option<&'a str>,
    pub(crate) column: Option<&'a str>,
    pub(crate) expect_column: Option<&'a str>,
    pub(crate) preset: Option<&'a str>,
    pub(crate) format_code: Option<&'a str>,
    pub(crate) decimals: i64,
    pub(crate) currency_symbol: Option<&'a str>,
    pub(crate) max_cells: i64,
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
}

pub(crate) fn xlsx_tables_set_column_format(
    file: &str,
    options: XlsxTablesSetColumnFormatOptions<'_>,
) -> CliResult<Value> {
    if !Path::new(file).exists() {
        return Err(CliError::file_not_found(format!("file not found: {file}")));
    }
    let column_name = options.column.unwrap_or_default().trim();
    if column_name.is_empty() {
        return Err(CliError::invalid_args("--column is required"));
    }
    if options.max_cells < 0 {
        return Err(CliError::invalid_args("--max-cells must be >= 0"));
    }

    let sheet_selector = options
        .sheet
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let tables = xlsx_tables(file, sheet_selector)?;
    let table = select_xlsx_table(&tables, options.table.unwrap_or_default())?;
    let column_index = table
        .columns
        .iter()
        .position(|column| column.name == column_name)
        .ok_or_else(|| {
            CliError::target_not_found(format!(
                "target not found: column {:?} not found in table {:?}",
                column_name, table.display_name
            ))
        })?;
    let column = table.columns[column_index].name.clone();
    if let Some(expect) = options
        .expect_column
        .map(str::trim)
        .filter(|value| !value.is_empty())
        && expect != column
    {
        return Err(CliError::invalid_args(format!(
            "resolved column {:?} does not match --expect-column {:?}",
            column, expect
        )));
    }

    let range_bounds = resolve_table_column_data_range(&table, column_name, column_index)?;
    let range = range_bounds_ref(range_bounds);
    let sheet = workbook_sheet_for_table(file, &table)?;
    let sheet_selector = format!("sheetId:{}", sheet.sheet_id);
    let range_result = xlsx_ranges_set_format(
        file,
        XlsxRangesSetFormatOptions {
            sheet: &sheet_selector,
            range: &range,
            preset: options.preset,
            format_code: options.format_code,
            decimals: options.decimals,
            currency_symbol: options.currency_symbol,
            max_cells: options.max_cells,
            out: options.out,
            backup: options.backup,
            dry_run: options.dry_run,
            no_validate: options.no_validate,
            in_place: options.in_place,
        },
    )?;
    let Value::Object(range_result) = range_result else {
        return Err(CliError::unexpected(
            "ranges set-format returned non-object JSON",
        ));
    };

    let output_path = range_result.get("output").and_then(Value::as_str);
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("table".to_string(), json!(table.display_name));
    result.insert("tableNumber".to_string(), json!(table.number));
    result.insert("sheet".to_string(), json!(table.sheet));
    result.insert("sheetNumber".to_string(), json!(table.sheet_number));
    result.insert("column".to_string(), json!(column));
    result.insert("columnIndex".to_string(), json!(column_index));
    copy_json_field(&range_result, &mut result, "range");
    result.insert("tableRange".to_string(), json!(table.range));
    copy_json_field(&range_result, &mut result, "rows");
    copy_json_field(&range_result, &mut result, "cols");
    copy_json_field(&range_result, &mut result, "preset");
    copy_json_field(&range_result, &mut result, "formatCode");
    copy_json_field(&range_result, &mut result, "numberFormatId");
    copy_json_field(&range_result, &mut result, "builtin");
    copy_json_field(&range_result, &mut result, "updated");
    copy_json_field(&range_result, &mut result, "created");
    copy_json_field(&range_result, &mut result, "createdStyles");
    copy_json_field(&range_result, &mut result, "styleIndexes");
    copy_json_field(&range_result, &mut result, "output");
    copy_json_field(&range_result, &mut result, "dryRun");
    copy_json_field(&range_result, &mut result, "destination");
    copy_json_field(&range_result, &mut result, "validateCommand");
    copy_json_field(&range_result, &mut result, "cellsExtractCommand");
    copy_json_field(&range_result, &mut result, "rangesExportCommand");
    copy_json_field(&range_result, &mut result, "validateCommandTemplate");
    copy_json_field(&range_result, &mut result, "cellsExtractCommandTemplate");
    copy_json_field(&range_result, &mut result, "rangesExportCommandTemplate");
    add_xlsx_table_format_commands(
        &mut result,
        output_path,
        &sheet_selector,
        &table.primary_selector,
    );
    Ok(Value::Object(result))
}

fn resolve_table_column_data_range(
    table: &XlsxTableRef,
    column_name: &str,
    column_index: usize,
) -> CliResult<RangeBounds> {
    let table_range = parse_range(&table.range).map_err(|err| {
        CliError::invalid_args(format!(
            "failed to resolve table column range: invalid table ref {:?}: {}",
            table.range, err.message
        ))
    })?;
    let table_range = table_range.normalized();
    let abs_col = table_range.min_col() + column_index as u32;
    if abs_col > table_range.max_col() {
        return Err(CliError::target_not_found(format!(
            "target not found: table column not found: {:?} resolves outside table range {}",
            column_name, table.range
        )));
    }
    let start_row = table_range.min_row() + table.header_row_count;
    let end_row = table_range.max_row().saturating_sub(table.totals_row_count);
    if end_row < start_row {
        return Err(CliError::invalid_args(format!(
            "table column has no data rows: column {:?} in table {:?}",
            column_name, table.display_name
        )));
    }
    Ok(RangeBounds {
        start_col: abs_col,
        start_row,
        end_col: abs_col,
        end_row,
    })
}

fn workbook_sheet_for_table(file: &str, table: &XlsxTableRef) -> CliResult<WorkbookSheet> {
    let workbook = zip_text(file, "xl/workbook.xml")?;
    let sheets = workbook_sheets(&workbook)?;
    let rels = relationships(file, "xl/_rels/workbook.xml.rels")?;
    let wanted = table.sheet_part_uri.trim_start_matches('/');
    for sheet in sheets {
        let Some(target) = rels.get(&sheet.rel_id) else {
            continue;
        };
        if normalize_xl_target(target) == wanted {
            return Ok(sheet);
        }
    }
    Ok(WorkbookSheet {
        name: table.sheet.clone(),
        sheet_id: table.sheet_number,
        position: table.sheet_number,
        rel_id: String::new(),
        state: String::new(),
    })
}

fn add_xlsx_table_format_commands(
    result: &mut Map<String, Value>,
    output_path: Option<&str>,
    sheet_selector: &str,
    table_selector: &str,
) {
    let target = output_path.unwrap_or("<out.xlsx>");
    let show_key = if output_path.is_some() {
        "tableShowCommand"
    } else {
        "tableShowCommandTemplate"
    };
    let export_key = if output_path.is_some() {
        "tableExportCommand"
    } else {
        "tableExportCommandTemplate"
    };
    result.insert(
        show_key.to_string(),
        json!(xlsx_source_command(
            vec!["ooxml", "--json", "xlsx", "tables", "show", target],
            &[("--sheet", sheet_selector), ("--table", table_selector)],
        )),
    );
    result.insert(
        export_key.to_string(),
        json!(format!(
            "ooxml --json xlsx tables export {} --sheet {} --table {} --include-types --include-formulas",
            command_arg(target),
            command_arg(sheet_selector),
            command_arg(table_selector)
        )),
    );
}

fn copy_json_field(source: &Map<String, Value>, target: &mut Map<String, Value>, key: &str) {
    if let Some(value) = source.get(key) {
        target.insert(key.to_string(), value.clone());
    }
}
