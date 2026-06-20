mod records;

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::xlsx_mutation::{
    XlsxMatrixCell, add_xlsx_range_mutation_commands, parse_xlsx_range_set_matrix,
    rectangularize_xlsx_matrix, resolve_xlsx_ranges_set_values, set_xlsx_range_in_sheet_xml,
    validate_xlsx_null_policy, xlsx_range_destination_json,
};
use crate::xlsx_tables::{
    XlsxTableRef, parse_xlsx_table_part, select_xlsx_table, xlsx_source_command, xlsx_tables,
};
use crate::{
    CliError, CliResult, RangeBounds, WorkbookSheet, add_xlsx_formula_recalc_package_updates,
    col_name, copy_zip_with_part_overrides_and_removals, local_name, normalize_xl_target,
    normalize_xlsx_ranges_set_data_format, parse_range, parse_xlsx_row_spans, range_bounds_ref,
    relationships, render_xml_attrs, validate, validate_xlsx_mutation_output_flags,
    workbook_sheets, xlsx_ranges_set_temp_path, xlsx_sheet_data_span, zip_text,
};
use records::{
    normalize_xlsx_missing_policy, resolve_xlsx_tables_append_records, xlsx_records_to_rows,
};

const XLSX_MAX_ROW: u32 = 1_048_576;

pub(crate) struct XlsxTablesAppendRowsOptions<'a> {
    pub(crate) sheet: Option<&'a str>,
    pub(crate) table: Option<&'a str>,
    pub(crate) values: Option<&'a str>,
    pub(crate) values_file: Option<&'a str>,
    pub(crate) data_format: Option<&'a str>,
    pub(crate) null_policy: Option<&'a str>,
    pub(crate) null_policy_present: bool,
    pub(crate) ragged: Option<&'a str>,
    pub(crate) max_cells: i64,
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
    pub(crate) overwrite_formulas: bool,
}

pub(crate) struct XlsxTablesAppendRecordsOptions<'a> {
    pub(crate) sheet: Option<&'a str>,
    pub(crate) table: Option<&'a str>,
    pub(crate) expect_range: Option<&'a str>,
    pub(crate) records: Option<&'a str>,
    pub(crate) records_file: Option<&'a str>,
    pub(crate) missing: Option<&'a str>,
    pub(crate) null_policy: Option<&'a str>,
    pub(crate) max_cells: i64,
    pub(crate) ignore_extra_fields: bool,
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
    pub(crate) overwrite_formulas: bool,
}

struct XlsxTableAppendTarget {
    table: XlsxTableRef,
    table_part: String,
    sheet_part: String,
    table_xml: String,
    sheet_xml: String,
    table_range: RangeBounds,
}

struct XlsxTableAppendWriteOptions<'a> {
    out: Option<&'a str>,
    backup: Option<&'a str>,
    dry_run: bool,
    no_validate: bool,
    in_place: bool,
    overwrite_formulas: bool,
}

struct TableStartTag {
    start: usize,
    end: usize,
    name: String,
    attrs: BTreeMap<String, String>,
    self_closing: bool,
}

struct TableRefScanState {
    saw_table: bool,
    replacements: Vec<TableStartTag>,
}

pub(crate) fn xlsx_tables_append_rows(
    file: &str,
    options: XlsxTablesAppendRowsOptions<'_>,
) -> CliResult<Value> {
    if !Path::new(file).exists() {
        return Err(CliError::file_not_found(format!("file not found: {file}")));
    }
    let data_format = normalize_xlsx_ranges_set_data_format(options.data_format)?;
    let data = resolve_xlsx_ranges_set_values(options.values, options.values_file)?;
    let mut matrix = parse_xlsx_range_set_matrix(&data, &data_format)?;
    rectangularize_xlsx_matrix(&mut matrix.rows, options.ragged.unwrap_or("reject"))?;
    let row_count = matrix.rows.len();
    let col_count = matrix.rows.first().map_or(0, Vec::len);
    if row_count < 1 || col_count < 1 {
        return Err(CliError::invalid_args("values matrix cannot be empty"));
    }
    if options.max_cells < 0 {
        return Err(CliError::invalid_args("--max-cells must be >= 0"));
    }
    let cell_count = i64::try_from(row_count.saturating_mul(col_count)).unwrap_or(i64::MAX);
    if options.max_cells > 0 && cell_count > options.max_cells {
        return Err(CliError::invalid_args(format!(
            "values matrix contains {cell_count} cells, above --max-cells {}",
            options.max_cells
        )));
    }
    let null_policy = normalize_xlsx_append_null_policy(
        options.null_policy,
        options.null_policy_present,
        matrix.null_policy.as_deref(),
    )?;
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;

    let target = resolve_xlsx_table_append_target(file, options.sheet, options.table)?;
    let mut result = xlsx_table_append_matrix(
        file,
        target,
        &matrix.rows,
        &null_policy,
        XlsxTableAppendWriteOptions {
            out: options.out,
            backup: options.backup,
            dry_run: options.dry_run,
            no_validate: options.no_validate,
            in_place: options.in_place,
            overwrite_formulas: options.overwrite_formulas,
        },
    )?;
    result.insert("dataFormat".to_string(), json!(data_format));
    result.insert("majorDimension".to_string(), json!(matrix.major_dimension));
    Ok(Value::Object(result))
}

pub(crate) fn xlsx_tables_append_records(
    file: &str,
    options: XlsxTablesAppendRecordsOptions<'_>,
) -> CliResult<Value> {
    if !Path::new(file).exists() {
        return Err(CliError::file_not_found(format!("file not found: {file}")));
    }
    let table_selector = options.table.unwrap_or_default();
    if table_selector.is_empty() {
        return Err(CliError::invalid_args("--table is required"));
    }
    let expect_range = options.expect_range.unwrap_or_default();
    if expect_range.is_empty() {
        return Err(CliError::invalid_args("--expect-range is required"));
    }
    if options.max_cells < 0 {
        return Err(CliError::invalid_args("--max-cells must be >= 0"));
    }
    let missing_policy = normalize_xlsx_missing_policy(options.missing)?;
    let null_policy = normalize_xlsx_append_null_policy(options.null_policy, true, None)?;
    let records = resolve_xlsx_tables_append_records(options.records, options.records_file)?;
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;

    let target = resolve_xlsx_table_append_target(file, options.sheet, Some(table_selector))?;
    if target.table.range != expect_range {
        return Err(CliError::invalid_args(format!(
            "table range mismatch: expected {expect_range} but found {}",
            target.table.range
        )));
    }
    let columns = xlsx_table_column_names(&target.table);
    let rows = xlsx_records_to_rows(
        &records,
        &columns,
        &missing_policy,
        options.ignore_extra_fields,
    )?;
    let cell_count = i64::try_from(rows.len().saturating_mul(columns.len())).unwrap_or(i64::MAX);
    if options.max_cells > 0 && cell_count > options.max_cells {
        return Err(CliError::invalid_args(format!(
            "records contain {cell_count} cells, above --max-cells {}",
            options.max_cells
        )));
    }

    let mut result = xlsx_table_append_matrix(
        file,
        target,
        &rows,
        &null_policy,
        XlsxTableAppendWriteOptions {
            out: options.out,
            backup: options.backup,
            dry_run: options.dry_run,
            no_validate: options.no_validate,
            in_place: options.in_place,
            overwrite_formulas: options.overwrite_formulas,
        },
    )?;
    result.insert("dataFormat".to_string(), json!("json"));
    result.insert("missingPolicy".to_string(), json!(missing_policy));
    result.insert(
        "ignoredExtraFields".to_string(),
        json!(options.ignore_extra_fields),
    );
    result.insert("columns".to_string(), json!(columns));
    Ok(Value::Object(result))
}

fn resolve_xlsx_table_append_target(
    file: &str,
    sheet: Option<&str>,
    table_selector: Option<&str>,
) -> CliResult<XlsxTableAppendTarget> {
    let tables = xlsx_tables(file, sheet)?;
    let table = select_xlsx_table(&tables, table_selector.unwrap_or_default())?;
    let table_part = table.part_uri.trim_start_matches('/').to_string();
    let sheet_part = table.sheet_part_uri.trim_start_matches('/').to_string();
    let table_xml = zip_text(file, &table_part)?;
    let sheet_xml = zip_text(file, &sheet_part)?;
    let table_range = validate_xlsx_table_append_xml(&table_xml, &table.part_uri)?;
    Ok(XlsxTableAppendTarget {
        table,
        table_part,
        sheet_part,
        table_xml,
        sheet_xml,
        table_range,
    })
}

fn xlsx_table_append_matrix(
    file: &str,
    target: XlsxTableAppendTarget,
    rows: &[Vec<XlsxMatrixCell>],
    null_policy: &str,
    options: XlsxTableAppendWriteOptions<'_>,
) -> CliResult<Map<String, Value>> {
    let row_count = rows.len();
    let col_count = rows.first().map_or(0, Vec::len);
    if row_count < 1 || col_count < 1 {
        return Err(CliError::invalid_args("values matrix cannot be empty"));
    }
    let table_bounds = target.table_range.normalized();
    if col_count as u32 != table_bounds.col_count() {
        return Err(CliError::invalid_args(format!(
            "table column count mismatch: row 1 has {col_count} columns, want {}",
            table_bounds.col_count()
        )));
    }
    if table_bounds.max_row().saturating_add(row_count as u32) > XLSX_MAX_ROW {
        return Err(CliError::invalid_args(format!(
            "table append exceeds XLSX max row {XLSX_MAX_ROW}"
        )));
    }

    let append_bounds = RangeBounds {
        start_col: table_bounds.min_col(),
        start_row: table_bounds.max_row() + 1,
        end_col: table_bounds.max_col(),
        end_row: table_bounds.max_row() + row_count as u32,
    };
    reject_xlsx_table_append_overwrite(&target.sheet_xml, append_bounds)?;

    let (updated_sheet_xml, stats) = set_xlsx_range_in_sheet_xml(
        &target.sheet_xml,
        append_bounds,
        rows,
        null_policy,
        options.overwrite_formulas,
    )?;
    let new_bounds = RangeBounds {
        start_col: table_bounds.min_col(),
        start_row: table_bounds.min_row(),
        end_col: table_bounds.max_col(),
        end_row: table_bounds.max_row() + row_count as u32,
    };
    let previous_range = range_bounds_ref(target.table_range);
    let new_range = range_bounds_ref(new_bounds);
    let append_range = range_bounds_ref(append_bounds);
    let updated_table_xml = update_xlsx_table_refs(&target.table_xml, &new_range)?;

    let output_path = options.out.filter(|value| !value.is_empty());
    let commit_path = if options.in_place {
        Some(file)
    } else {
        output_path
    };
    let readback_path = if options.dry_run || options.in_place || output_path == Some(file) {
        xlsx_ranges_set_temp_path(file)
    } else {
        output_path
            .ok_or_else(|| {
                CliError::invalid_args(
                    "must specify exactly one of --out, --in-place, or --dry-run",
                )
            })?
            .to_string()
    };
    let mut overrides = BTreeMap::new();
    let mut removals = BTreeSet::new();
    overrides.insert(target.sheet_part.clone(), updated_sheet_xml);
    overrides.insert(target.table_part.clone(), updated_table_xml.clone());
    add_xlsx_formula_recalc_package_updates(
        file,
        stats.formula_seen,
        stats.formula_invalidated,
        &mut overrides,
        &mut removals,
    )?;
    copy_zip_with_part_overrides_and_removals(file, &readback_path, &overrides, &removals)?;
    if !options.no_validate {
        validate(&readback_path, true)?;
    }

    let sheet = workbook_sheet_for_table(file, &target.table)?;
    let destination = xlsx_table_append_destination_json(
        &readback_path,
        commit_path,
        &sheet,
        &target.table,
        &updated_table_xml,
        &previous_range,
        &append_range,
    )?;

    if options.dry_run {
        let _ = fs::remove_file(&readback_path);
    } else if options.in_place || output_path == Some(file) {
        if let Some(backup_path) = options.backup.filter(|value| !value.is_empty()) {
            fs::copy(file, backup_path)
                .map_err(|err| CliError::unexpected(format!("failed to create backup: {err}")))?;
        }
        fs::rename(&readback_path, file)
            .or_else(|_| {
                fs::copy(&readback_path, file)?;
                fs::remove_file(&readback_path)
            })
            .map_err(|err| CliError::unexpected(format!("failed to write output file: {err}")))?;
    }

    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("table".to_string(), json!(target.table.display_name));
    result.insert("sheet".to_string(), json!(target.table.sheet));
    result.insert("sheetNumber".to_string(), json!(target.table.sheet_number));
    result.insert("previousRange".to_string(), json!(previous_range));
    result.insert("range".to_string(), json!(new_range));
    result.insert("appendRange".to_string(), json!(append_range));
    result.insert("rowsAppended".to_string(), json!(row_count));
    result.insert("updated".to_string(), json!(stats.updated));
    result.insert("created".to_string(), json!(stats.created));
    result.insert("cleared".to_string(), json!(stats.cleared));
    result.insert("skipped".to_string(), json!(stats.skipped));
    result.insert("formulaCount".to_string(), json!(stats.formula_count));
    result.insert("nullPolicy".to_string(), json!(null_policy));
    if let Some(commit_path) = commit_path {
        result.insert("output".to_string(), json!(commit_path));
    }
    result.insert("dryRun".to_string(), json!(options.dry_run));
    result.insert("destination".to_string(), destination);
    add_xlsx_range_mutation_commands(
        &mut result,
        commit_path,
        &format!("sheetId:{}", sheet.sheet_id),
        &append_range,
    );
    add_xlsx_table_append_commands(
        &mut result,
        commit_path,
        &format!("sheetId:{}", sheet.sheet_id),
        &target.table.primary_selector,
    );
    Ok(result)
}

fn normalize_xlsx_append_null_policy(
    flag_value: Option<&str>,
    flag_present: bool,
    matrix_value: Option<&str>,
) -> CliResult<String> {
    let raw = if flag_present {
        flag_value.unwrap_or_default()
    } else {
        matrix_value.unwrap_or_else(|| flag_value.unwrap_or("skip"))
    };
    let normalized = raw.trim().to_ascii_lowercase();
    let normalized = match normalized.as_str() {
        "" | "skip" => "skip",
        "clear" => "clear",
        "empty-string" => "empty-string",
        _ => {
            validate_xlsx_null_policy(raw)?;
            unreachable!("validate_xlsx_null_policy rejects invalid policies")
        }
    };
    Ok(normalized.to_string())
}

fn xlsx_table_column_names(table: &XlsxTableRef) -> Vec<String> {
    table
        .columns
        .iter()
        .map(|column| column.name.clone())
        .collect()
}

fn validate_xlsx_table_append_xml(xml: &str, part_uri: &str) -> CliResult<RangeBounds> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut stack = Vec::<String>::new();
    let mut table_range = None;
    let mut saw_table = false;
    let mut saw_table_columns = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                validate_xlsx_table_append_element(
                    &e,
                    &name,
                    &stack,
                    part_uri,
                    &mut saw_table,
                    &mut saw_table_columns,
                    &mut table_range,
                )?;
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                validate_xlsx_table_append_element(
                    &e,
                    &name,
                    &stack,
                    part_uri,
                    &mut saw_table,
                    &mut saw_table_columns,
                    &mut table_range,
                )?;
            }
            Ok(Event::End(_)) => {
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    if !saw_table {
        return Err(CliError::unexpected(format!(
            "table part {part_uri} root element not found"
        )));
    }
    if !saw_table_columns {
        return Err(CliError::invalid_args(
            "table has unsupported features: missing tableColumns",
        ));
    }
    table_range.ok_or_else(|| CliError::unexpected(format!("table {part_uri} has no ref")))
}

fn validate_xlsx_table_append_element(
    element: &BytesStart<'_>,
    name: &str,
    stack: &[String],
    part_uri: &str,
    saw_table: &mut bool,
    saw_table_columns: &mut bool,
    table_range: &mut Option<RangeBounds>,
) -> CliResult<()> {
    if !*saw_table && stack.is_empty() && name == "table" {
        *saw_table = true;
        if parse_boolish(
            xlsx_attr(element, "totalsRowShown")
                .as_deref()
                .unwrap_or_default(),
        ) || parse_positive_int(
            xlsx_attr(element, "totalsRowCount")
                .as_deref()
                .unwrap_or_default(),
        ) {
            return Err(CliError::invalid_args("table has totals rows"));
        }
        let table_type = xlsx_attr(element, "tableType").unwrap_or_default();
        if !table_type.is_empty() && table_type != "worksheet" {
            return Err(CliError::invalid_args(format!(
                "table has unsupported features: tableType={table_type}"
            )));
        }
        let range_text = xlsx_attr(element, "ref").unwrap_or_default();
        *table_range = Some(parse_range(&range_text).map_err(|err| {
            CliError::unexpected(format!("invalid table ref {range_text:?}: {}", err.message))
        })?);
        return Ok(());
    }
    if stack.len() == 1 && stack[0] == "table" {
        match name {
            "extLst" => {
                return Err(CliError::invalid_args(
                    "table has unsupported features: extLst",
                ));
            }
            "tableColumns" => {
                *saw_table_columns = true;
            }
            _ => {}
        }
    }
    if name == "sortState" && stack.last().is_some_and(|parent| parent == "autoFilter") {
        return Err(CliError::invalid_args(
            "table has unsupported features: sortState",
        ));
    }
    if name == "calculatedColumnFormula" && stack.iter().any(|part| part == "tableColumn") {
        return Err(CliError::invalid_args("table has calculated columns"));
    }
    if !*saw_table && stack.is_empty() {
        return Err(CliError::unexpected(format!(
            "table part {part_uri} root element not found"
        )));
    }
    Ok(())
}

fn reject_xlsx_table_append_overwrite(xml: &str, target: RangeBounds) -> CliResult<()> {
    let sheet_data = xlsx_sheet_data_span(xml)?;
    let rows = parse_xlsx_row_spans(xml, sheet_data.as_ref())?;
    let target = target.normalized();
    for row_number in target.start_row..=target.end_row {
        let Some(row) = rows.get(&row_number) else {
            continue;
        };
        for col_number in target.start_col..=target.end_col {
            let Some(cell) = row.cells.get(&col_number) else {
                continue;
            };
            if xlsx_cell_xml_has_content(&cell.xml) {
                return Err(CliError::invalid_args(format!(
                    "table append would overwrite existing cells: {}{}",
                    col_name(col_number),
                    row_number
                )));
            }
        }
    }
    Ok(())
}

fn xlsx_cell_xml_has_content(xml: &str) -> bool {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if matches!(local_name(e.name().as_ref()), "v" | "f" | "is") =>
            {
                return true;
            }
            Ok(Event::Eof) => return false,
            Err(_) => return false,
            _ => {}
        }
    }
}

fn update_xlsx_table_refs(xml: &str, new_range: &str) -> CliResult<String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut stack = Vec::<String>::new();
    let mut state = TableRefScanState {
        saw_table: false,
        replacements: Vec::new(),
    };
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                collect_xlsx_table_ref_replacement(
                    &e,
                    before,
                    reader.buffer_position() as usize,
                    false,
                    &name,
                    &stack,
                    &mut state,
                );
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                collect_xlsx_table_ref_replacement(
                    &e,
                    before,
                    reader.buffer_position() as usize,
                    true,
                    &name,
                    &stack,
                    &mut state,
                );
            }
            Ok(Event::End(_)) => {
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }

    let mut out = xml.to_string();
    state
        .replacements
        .sort_by_key(|replacement| replacement.start);
    for mut replacement in state.replacements.into_iter().rev() {
        replacement
            .attrs
            .insert("ref".to_string(), new_range.to_string());
        let tag = if replacement.self_closing {
            format!(
                "<{}{}{}",
                replacement.name,
                render_xml_attrs(&replacement.attrs),
                "/>"
            )
        } else {
            format!(
                "<{}{}>",
                replacement.name,
                render_xml_attrs(&replacement.attrs)
            )
        };
        out.replace_range(replacement.start..replacement.end, &tag);
    }
    Ok(out)
}

fn collect_xlsx_table_ref_replacement(
    element: &BytesStart<'_>,
    start: usize,
    end: usize,
    self_closing: bool,
    name: &str,
    stack: &[String],
    state: &mut TableRefScanState,
) {
    let is_table_root = !state.saw_table && stack.is_empty() && name == "table";
    let is_direct_auto_filter = stack.len() == 1 && stack[0] == "table" && name == "autoFilter";
    if !is_table_root && !is_direct_auto_filter {
        return;
    }
    if is_table_root {
        state.saw_table = true;
    }
    state.replacements.push(TableStartTag {
        start,
        end,
        name: String::from_utf8_lossy(element.name().as_ref()).to_string(),
        attrs: crate::xml_util::decode_xml_attrs(element),
        self_closing,
    });
}

fn xlsx_table_append_destination_json(
    readback_file: &str,
    destination_file: Option<&str>,
    sheet: &WorkbookSheet,
    table: &XlsxTableRef,
    updated_table_xml: &str,
    previous_range: &str,
    append_range: &str,
) -> CliResult<Value> {
    let mut updated = parse_xlsx_table_part(updated_table_xml, &table.part_uri)?;
    updated.number = table.number;
    updated.sheet = table.sheet.clone();
    updated.sheet_number = table.sheet_number;
    updated.sheet_part_uri = table.sheet_part_uri.clone();
    updated.relationship_id = table.relationship_id.clone();
    updated.part_uri = table.part_uri.clone();
    updated.apply_selectors();
    let sheet_part = table.sheet_part_uri.trim_start_matches('/');
    let appended = xlsx_range_destination_json(
        readback_file,
        destination_file,
        sheet,
        sheet_part,
        append_range,
    )?;

    let mut destination = Map::new();
    if let Some(file) = destination_file {
        destination.insert("file".to_string(), json!(file));
    }
    destination.insert("table".to_string(), json!(updated.display_name));
    destination.insert(
        "tablePrimarySelector".to_string(),
        json!(updated.primary_selector),
    );
    destination.insert("tableSelectors".to_string(), json!(updated.selectors));
    destination.insert("tablePartUri".to_string(), json!(updated.part_uri));
    destination.insert("relationshipId".to_string(), json!(updated.relationship_id));
    destination.insert("sheet".to_string(), json!(updated.sheet));
    destination.insert("sheetNumber".to_string(), json!(updated.sheet_number));
    destination.insert(
        "sheetPrimarySelector".to_string(),
        json!(format!("sheetId:{}", sheet.sheet_id)),
    );
    destination.insert(
        "sheetSelectors".to_string(),
        json!(crate::xlsx_sheet_selectors(
            &sheet.name,
            sheet.sheet_id,
            sheet.position,
            &sheet.rel_id,
            &table.sheet_part_uri,
        )),
    );
    destination.insert("previousRange".to_string(), json!(previous_range));
    destination.insert("range".to_string(), json!(updated.range));
    destination.insert("appendRange".to_string(), json!(append_range));
    destination.insert("rows".to_string(), json!(updated.rows));
    destination.insert("cols".to_string(), json!(updated.cols));
    destination.insert("dataRows".to_string(), json!(updated.data_row_count));
    destination.insert(
        "columns".to_string(),
        json!(
            updated
                .columns
                .iter()
                .map(|column| column.name.clone())
                .collect::<Vec<_>>()
        ),
    );
    destination.insert("appended".to_string(), appended);
    Ok(Value::Object(destination))
}

fn add_xlsx_table_append_commands(
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
    let mut export = xlsx_source_command(
        vec!["ooxml", "--json", "xlsx", "tables", "export", target],
        &[("--sheet", sheet_selector), ("--table", table_selector)],
    );
    export.push_str(" --include-types --include-formulas");
    result.insert(export_key.to_string(), json!(export));
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

fn xlsx_attr(element: &BytesStart<'_>, wanted: &str) -> Option<String> {
    element.attributes().flatten().find_map(|attr| {
        if local_name(attr.key.as_ref()) == wanted {
            Some(crate::xml_util::decode_xml_text(attr.value.as_ref()))
        } else {
            None
        }
    })
}

fn parse_boolish(value: &str) -> bool {
    value == "1" || value == "true"
}

fn parse_positive_int(value: &str) -> bool {
    !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()) && value != "0"
}
