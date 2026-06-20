use super::*;

pub(super) struct XlsxFiltersSortsTableTarget {
    pub(super) table: XlsxTableRef,
    pub(super) table_part: String,
    pub(super) sheet_xml: String,
    pub(super) table_xml: String,
}

pub(super) struct XlsxFiltersSortsMutationTarget {
    pub(super) sheet_name: String,
    pub(super) sheet_number: u32,
    pub(super) sheet_id: u32,
    pub(super) table_name: Option<String>,
    pub(super) part: String,
    pub(super) updated_xml: String,
    pub(super) ref_text: Option<String>,
    pub(super) auto_filter: Option<AutoFilterState>,
    pub(super) sort_state: Option<SortState>,
}

pub(super) fn write_filters_sorts_mutation_result(
    file: &str,
    action: &str,
    mutation: XlsxFiltersSortsMutationTarget,
    options: XlsxFiltersSortsOutputOptions<'_>,
) -> CliResult<Value> {
    let output_path = options.out.filter(|value| !value.trim().is_empty());
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

    copy_zip_with_part_override(file, &readback_path, &mutation.part, &mutation.updated_xml)?;
    if !options.no_validate {
        validate(&readback_path, true)?;
    }
    if options.dry_run {
        let _ = fs::remove_file(&readback_path);
    } else if options.in_place || output_path == Some(file) {
        if let Some(backup_path) = options.backup.filter(|value| !value.trim().is_empty()) {
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
    result.insert("sheet".to_string(), json!(mutation.sheet_name));
    result.insert("sheetNumber".to_string(), json!(mutation.sheet_number));
    if let Some(table_name) = mutation.table_name.as_deref() {
        result.insert("table".to_string(), json!(table_name));
    }
    result.insert("action".to_string(), json!(action));
    if let Some(ref_text) = mutation
        .ref_text
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        result.insert("ref".to_string(), json!(ref_text));
    }
    result.insert("note".to_string(), json!(FILTERS_SORTS_NOTE));
    if let Some(auto_filter) = mutation.auto_filter.as_ref() {
        result.insert("autoFilter".to_string(), auto_filter_json(auto_filter));
    }
    if let Some(sort_state) = mutation.sort_state.as_ref() {
        result.insert("sortState".to_string(), sort_state_json(sort_state));
    }
    if let Some(commit_path) = commit_path {
        result.insert("output".to_string(), json!(commit_path));
    }
    result.insert("dryRun".to_string(), json!(options.dry_run));
    if let Some(commit_path) = commit_path {
        result.insert(
            "validateCommand".to_string(),
            json!(format!(
                "ooxml validate --strict {}",
                command_arg(commit_path)
            )),
        );
        let show_command = if let Some(table_name) = mutation.table_name.as_deref() {
            filters_sorts_show_command(commit_path, None, Some(table_name))
        } else {
            let sheet = WorkbookSheet {
                name: mutation.sheet_name,
                sheet_id: mutation.sheet_id,
                position: mutation.sheet_number,
                rel_id: String::new(),
                state: String::new(),
            };
            filters_sorts_show_command(commit_path, Some(&sheet), None)
        };
        result.insert("showCommand".to_string(), json!(show_command));
    }
    Ok(Value::Object(result))
}

pub(super) fn resolve_filters_sorts_table(
    file: &str,
    sheet_selector: Option<&str>,
    table_selector: Option<&str>,
) -> CliResult<XlsxFiltersSortsTableTarget> {
    let tables = xlsx_tables(
        file,
        sheet_selector.filter(|value| !value.trim().is_empty()),
    )?;
    let table = select_xlsx_table(&tables, table_selector.unwrap_or_default())?;
    let table_part = table.part_uri.trim_start_matches('/').to_string();
    let sheet_part = table.sheet_part_uri.trim_start_matches('/').to_string();
    let table_xml = zip_text(file, &table_part)?;
    let sheet_xml = zip_text(file, &sheet_part)?;
    Ok(XlsxFiltersSortsTableTarget {
        table,
        table_part,
        sheet_xml,
        table_xml,
    })
}

pub(super) fn resolve_filters_sorts_sheet(
    file: &str,
    sheet_selector: Option<&str>,
) -> CliResult<(WorkbookSheet, String, String)> {
    let workbook = zip_text(file, "xl/workbook.xml")?;
    let sheets = workbook_sheets(&workbook)?;
    if sheets.is_empty() {
        return Err(CliError::invalid_args("workbook has no sheets"));
    }
    let selector = sheet_selector.unwrap_or_default().trim();
    let sheet = if selector.is_empty() {
        sheets[0].clone()
    } else {
        resolve_sheet(&sheets, selector)?
    };
    let rels = relationships(file, "xl/_rels/workbook.xml.rels")?;
    let target = rels
        .get(&sheet.rel_id)
        .ok_or_else(|| CliError::unexpected(format!("missing relationship {}", sheet.rel_id)))?;
    let sheet_part = normalize_xl_target(target);
    if !sheet_part.starts_with("xl/worksheets/") {
        return Err(CliError::invalid_args(format!(
            "sheet {:?} is not a worksheet",
            sheet.name
        )));
    }
    let sheet_xml = zip_text(file, &sheet_part)?;
    Ok((sheet, sheet_part, sheet_xml))
}

pub(super) fn filters_sorts_sheet_selector(sheet: &WorkbookSheet) -> String {
    if sheet.sheet_id > 0 {
        format!("sheetId:{}", sheet.sheet_id)
    } else if !sheet.name.is_empty() {
        sheet.name.clone()
    } else if sheet.position > 0 {
        format!("sheet:{}", sheet.position)
    } else {
        "1".to_string()
    }
}

pub(super) fn filters_sorts_show_command(
    file: &str,
    sheet: Option<&WorkbookSheet>,
    table: Option<&str>,
) -> String {
    if let Some(table) = table {
        return format!(
            "ooxml --json xlsx filters-sorts show {} --table {}",
            command_arg(file),
            command_arg(table)
        );
    }
    let selector = sheet
        .map(filters_sorts_sheet_selector)
        .unwrap_or_else(|| "1".to_string());
    format!(
        "ooxml --json xlsx filters-sorts show {} --sheet {}",
        command_arg(file),
        command_arg(&selector)
    )
}
