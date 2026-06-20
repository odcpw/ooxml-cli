use serde_json::{Value, json};

use crate::{
    CliError, CliResult, XlsxCellsSetOptions, XlsxCommentsAddOptions, XlsxCommentsRemoveOptions,
    XlsxCommentsUpdateOptions, XlsxRangesSetFormatOptions, XlsxRangesSetOptions,
    XlsxTablesAppendRecordsOptions, XlsxTablesAppendRowsOptions, XlsxWorkbookMetadataUpdateOptions,
    json_bool, json_i64, json_optional_serialized, json_optional_string, json_string,
    xlsx_cells_set, xlsx_comments_add, xlsx_comments_remove, xlsx_comments_update, xlsx_ranges_set,
    xlsx_ranges_set_format, xlsx_tables_append_records, xlsx_tables_append_rows,
    xlsx_workbook_metadata_update,
};

use super::super::op::{ServeOp, push_serve_plan_bool_flag, push_serve_plan_string_flag};

pub(super) fn serve_xlsx_op(working: &str, command: &str, args: &Value) -> CliResult<ServeOp> {
    let op = match command {
        "xlsx cells set" => {
            let sheet = json_string(args, "sheet")?;
            let cell = json_string(args, "cell")?;
            let value = json_string(args, "value")?;
            let readback = xlsx_cells_set(
                working,
                XlsxCellsSetOptions {
                    sheet: Some(&sheet),
                    cell: Some(&cell),
                    ref_: None,
                    value: Some(&value),
                    formula: None,
                    value_type: None,
                    out: None,
                    backup: None,
                    dry_run: false,
                    no_validate: true,
                    in_place: true,
                },
            )?;
            let plan_flags = vec![
                json!("--cell"),
                json!(cell),
                json!("--sheet"),
                json!(sheet),
                json!("--value"),
                json!(value),
            ];
            ServeOp::XlsxCellSet {
                command: command.to_string(),
                plan_flags,
                readback_file: working.to_string(),
                readback,
            }
        }
        "xlsx comments add" => {
            let sheet = json_optional_string(args, "sheet");
            let cell = json_string(args, "cell")?;
            let author = json_string(args, "author")?;
            let text = json_optional_string(args, "text");
            let text_file = json_optional_string(args, "text-file")
                .or_else(|| json_optional_string(args, "textFile"));
            let readback = xlsx_comments_add(
                working,
                XlsxCommentsAddOptions {
                    sheet: sheet.as_deref(),
                    cell: Some(&cell),
                    author: Some(&author),
                    text: text.as_deref(),
                    text_file: text_file.as_deref(),
                    out: None,
                    backup: None,
                    dry_run: false,
                    no_validate: true,
                    in_place: true,
                },
            )?;
            let mut plan_flags = Vec::new();
            push_serve_plan_string_flag(&mut plan_flags, "--sheet", sheet.as_deref());
            push_serve_plan_string_flag(&mut plan_flags, "--cell", Some(&cell));
            push_serve_plan_string_flag(&mut plan_flags, "--author", Some(&author));
            push_serve_plan_string_flag(&mut plan_flags, "--text", text.as_deref());
            push_serve_plan_string_flag(&mut plan_flags, "--text-file", text_file.as_deref());
            ServeOp::XlsxCommentsOp {
                command: command.to_string(),
                plan_flags,
                readback_file: working.to_string(),
                readback,
            }
        }
        "xlsx comments update" => {
            let sheet = json_optional_string(args, "sheet");
            let comment_id = match json_i64(args, "comment-id")? {
                Some(value) => Some(value),
                None => json_i64(args, "commentId")?,
            };
            if let Some(comment_id) = comment_id
                && comment_id < 0
            {
                return Err(CliError::invalid_args("--comment-id must be >= 0"));
            }
            let handle = json_optional_string(args, "handle");
            let text = json_optional_string(args, "text");
            let text_file = json_optional_string(args, "text-file")
                .or_else(|| json_optional_string(args, "textFile"));
            let author = json_optional_string(args, "author");
            let expect_hash = json_optional_string(args, "expect-hash")
                .or_else(|| json_optional_string(args, "expectHash"));
            let readback = xlsx_comments_update(
                working,
                XlsxCommentsUpdateOptions {
                    sheet: sheet.as_deref(),
                    comment_id,
                    handle: handle.as_deref(),
                    text: text.as_deref(),
                    text_present: args.get("text").is_some(),
                    text_file: text_file.as_deref(),
                    author: author.as_deref(),
                    author_present: args.get("author").is_some(),
                    expect_hash: expect_hash.as_deref(),
                    out: None,
                    backup: None,
                    dry_run: false,
                    no_validate: true,
                    in_place: true,
                },
            )?;
            let mut plan_flags = Vec::new();
            push_serve_plan_string_flag(&mut plan_flags, "--sheet", sheet.as_deref());
            if let Some(comment_id) = comment_id {
                plan_flags.push(json!("--comment-id"));
                plan_flags.push(json!(comment_id.to_string()));
            }
            push_serve_plan_string_flag(&mut plan_flags, "--handle", handle.as_deref());
            push_serve_plan_string_flag(&mut plan_flags, "--text", text.as_deref());
            push_serve_plan_string_flag(&mut plan_flags, "--text-file", text_file.as_deref());
            push_serve_plan_string_flag(&mut plan_flags, "--author", author.as_deref());
            push_serve_plan_string_flag(&mut plan_flags, "--expect-hash", expect_hash.as_deref());
            ServeOp::XlsxCommentsOp {
                command: command.to_string(),
                plan_flags,
                readback_file: working.to_string(),
                readback,
            }
        }
        "xlsx comments remove" | "xlsx comments delete" => {
            let sheet = json_optional_string(args, "sheet");
            let comment_id = match json_i64(args, "comment-id")? {
                Some(value) => Some(value),
                None => json_i64(args, "commentId")?,
            };
            if let Some(comment_id) = comment_id
                && comment_id < 0
            {
                return Err(CliError::invalid_args("--comment-id must be >= 0"));
            }
            let handle = json_optional_string(args, "handle");
            let expect_hash = json_optional_string(args, "expect-hash")
                .or_else(|| json_optional_string(args, "expectHash"));
            let readback = xlsx_comments_remove(
                working,
                XlsxCommentsRemoveOptions {
                    sheet: sheet.as_deref(),
                    comment_id,
                    handle: handle.as_deref(),
                    expect_hash: expect_hash.as_deref(),
                    out: None,
                    backup: None,
                    dry_run: false,
                    no_validate: true,
                    in_place: true,
                },
            )?;
            let mut plan_flags = Vec::new();
            push_serve_plan_string_flag(&mut plan_flags, "--sheet", sheet.as_deref());
            if let Some(comment_id) = comment_id {
                plan_flags.push(json!("--comment-id"));
                plan_flags.push(json!(comment_id.to_string()));
            }
            push_serve_plan_string_flag(&mut plan_flags, "--handle", handle.as_deref());
            push_serve_plan_string_flag(&mut plan_flags, "--expect-hash", expect_hash.as_deref());
            ServeOp::XlsxCommentsOp {
                command: command.to_string(),
                plan_flags,
                readback_file: working.to_string(),
                readback,
            }
        }
        "xlsx ranges set" => {
            let sheet = json_string(args, "sheet")?;
            let range = json_optional_string(args, "range");
            let anchor = json_optional_string(args, "anchor");
            let values = json_optional_serialized(args, "values")?;
            let values_file = json_optional_string(args, "values-file")
                .or_else(|| json_optional_string(args, "valuesFile"));
            let data_format = json_optional_string(args, "data-format")
                .or_else(|| json_optional_string(args, "dataFormat"));
            let null_policy = json_optional_string(args, "null-policy")
                .or_else(|| json_optional_string(args, "nullPolicy"));
            let ragged = json_optional_string(args, "ragged");
            let max_cells = json_i64(args, "max-cells")?
                .or(json_i64(args, "maxCells")?)
                .unwrap_or(100000);
            let overwrite_formulas = json_bool(args, "overwrite-formulas")
                .or_else(|| json_bool(args, "overwriteFormulas"))
                .unwrap_or(false);
            let readback = xlsx_ranges_set(
                working,
                XlsxRangesSetOptions {
                    sheet: &sheet,
                    range: range.as_deref(),
                    anchor: anchor.as_deref(),
                    values: values.as_deref(),
                    values_file: values_file.as_deref(),
                    data_format: data_format.as_deref(),
                    null_policy: null_policy.as_deref(),
                    ragged: ragged.as_deref(),
                    max_cells,
                    out: None,
                    backup: None,
                    dry_run: false,
                    no_validate: true,
                    in_place: true,
                    overwrite_formulas,
                },
            )?;
            ServeOp::XlsxRangeSet {
                command: command.to_string(),
                sheet,
                range,
                anchor,
                values,
                values_file,
                data_format,
                null_policy,
                ragged,
                max_cells,
                overwrite_formulas,
                readback_file: working.to_string(),
                readback,
            }
        }
        "xlsx ranges set-format" => {
            let sheet = json_string(args, "sheet")?;
            let range = json_string(args, "range")?;
            let preset = json_optional_string(args, "preset");
            let format_code = json_optional_string(args, "format-code")
                .or_else(|| json_optional_string(args, "formatCode"));
            let decimals = json_i64(args, "decimals")?.unwrap_or(2);
            let currency_symbol = json_optional_string(args, "currency-symbol")
                .or_else(|| json_optional_string(args, "currencySymbol"));
            let max_cells = json_i64(args, "max-cells")?
                .or(json_i64(args, "maxCells")?)
                .unwrap_or(100000);
            let readback = xlsx_ranges_set_format(
                working,
                XlsxRangesSetFormatOptions {
                    sheet: &sheet,
                    range: &range,
                    preset: preset.as_deref(),
                    format_code: format_code.as_deref(),
                    decimals,
                    currency_symbol: currency_symbol.as_deref(),
                    max_cells,
                    out: None,
                    backup: None,
                    dry_run: false,
                    no_validate: true,
                    in_place: true,
                },
            )?;
            ServeOp::XlsxRangeSetFormat {
                command: command.to_string(),
                sheet,
                range,
                preset,
                format_code,
                decimals,
                currency_symbol,
                max_cells,
                readback_file: working.to_string(),
                readback,
            }
        }
        "xlsx tables append-rows" => {
            let sheet = json_optional_string(args, "sheet");
            let table = json_optional_string(args, "table");
            let values = json_optional_serialized(args, "values")?;
            let values_file = json_optional_string(args, "values-file")
                .or_else(|| json_optional_string(args, "valuesFile"));
            let data_format = json_optional_string(args, "data-format")
                .or_else(|| json_optional_string(args, "dataFormat"));
            let null_policy = json_optional_string(args, "null-policy")
                .or_else(|| json_optional_string(args, "nullPolicy"));
            let ragged = json_optional_string(args, "ragged");
            let max_cells = json_i64(args, "max-cells")?
                .or(json_i64(args, "maxCells")?)
                .unwrap_or(100000);
            let overwrite_formulas = json_bool(args, "overwrite-formulas")
                .or_else(|| json_bool(args, "overwriteFormulas"))
                .unwrap_or(false);
            let readback = xlsx_tables_append_rows(
                working,
                XlsxTablesAppendRowsOptions {
                    sheet: sheet.as_deref(),
                    table: table.as_deref(),
                    values: values.as_deref(),
                    values_file: values_file.as_deref(),
                    data_format: data_format.as_deref(),
                    null_policy: null_policy.as_deref(),
                    null_policy_present: null_policy.is_some(),
                    ragged: ragged.as_deref(),
                    max_cells,
                    out: None,
                    backup: None,
                    dry_run: false,
                    no_validate: true,
                    in_place: true,
                    overwrite_formulas,
                },
            )?;
            let mut plan_flags = Vec::new();
            push_serve_plan_string_flag(&mut plan_flags, "--sheet", sheet.as_deref());
            push_serve_plan_string_flag(&mut plan_flags, "--table", table.as_deref());
            push_serve_plan_string_flag(&mut plan_flags, "--values", values.as_deref());
            push_serve_plan_string_flag(&mut plan_flags, "--values-file", values_file.as_deref());
            push_serve_plan_string_flag(&mut plan_flags, "--data-format", data_format.as_deref());
            push_serve_plan_string_flag(&mut plan_flags, "--null-policy", null_policy.as_deref());
            push_serve_plan_string_flag(&mut plan_flags, "--ragged", ragged.as_deref());
            if max_cells != 100000 {
                plan_flags.push(json!("--max-cells"));
                plan_flags.push(json!(max_cells.to_string()));
            }
            if overwrite_formulas {
                plan_flags.push(json!("--overwrite-formulas"));
            }
            ServeOp::XlsxTablesOp {
                command: command.to_string(),
                plan_flags,
                readback_file: working.to_string(),
                readback,
            }
        }
        "xlsx tables append-records" => {
            let sheet = json_optional_string(args, "sheet");
            let table = json_string(args, "table")?;
            let expect_range = json_optional_string(args, "expect-range")
                .or_else(|| json_optional_string(args, "expectRange"))
                .ok_or_else(|| CliError::invalid_args("expect-range is required"))?;
            let records = json_optional_serialized(args, "records")?;
            let records_file = json_optional_string(args, "records-file")
                .or_else(|| json_optional_string(args, "recordsFile"));
            let missing = json_optional_string(args, "missing");
            let null_policy = json_optional_string(args, "null-policy")
                .or_else(|| json_optional_string(args, "nullPolicy"));
            let max_cells = json_i64(args, "max-cells")?
                .or(json_i64(args, "maxCells")?)
                .unwrap_or(100000);
            let ignore_extra_fields = json_bool(args, "ignore-extra-fields")
                .or_else(|| json_bool(args, "ignoreExtraFields"))
                .unwrap_or(false);
            let overwrite_formulas = json_bool(args, "overwrite-formulas")
                .or_else(|| json_bool(args, "overwriteFormulas"))
                .unwrap_or(false);
            let readback = xlsx_tables_append_records(
                working,
                XlsxTablesAppendRecordsOptions {
                    sheet: sheet.as_deref(),
                    table: Some(&table),
                    expect_range: Some(&expect_range),
                    records: records.as_deref(),
                    records_file: records_file.as_deref(),
                    missing: missing.as_deref(),
                    null_policy: null_policy.as_deref(),
                    max_cells,
                    ignore_extra_fields,
                    out: None,
                    backup: None,
                    dry_run: false,
                    no_validate: true,
                    in_place: true,
                    overwrite_formulas,
                },
            )?;
            let mut plan_flags = Vec::new();
            push_serve_plan_string_flag(&mut plan_flags, "--sheet", sheet.as_deref());
            push_serve_plan_string_flag(&mut plan_flags, "--table", Some(&table));
            push_serve_plan_string_flag(&mut plan_flags, "--expect-range", Some(&expect_range));
            push_serve_plan_string_flag(&mut plan_flags, "--records", records.as_deref());
            push_serve_plan_string_flag(&mut plan_flags, "--records-file", records_file.as_deref());
            push_serve_plan_string_flag(&mut plan_flags, "--missing", missing.as_deref());
            push_serve_plan_string_flag(&mut plan_flags, "--null-policy", null_policy.as_deref());
            if max_cells != 100000 {
                plan_flags.push(json!("--max-cells"));
                plan_flags.push(json!(max_cells.to_string()));
            }
            if ignore_extra_fields {
                plan_flags.push(json!("--ignore-extra-fields"));
            }
            if overwrite_formulas {
                plan_flags.push(json!("--overwrite-formulas"));
            }
            ServeOp::XlsxTablesOp {
                command: command.to_string(),
                plan_flags,
                readback_file: working.to_string(),
                readback,
            }
        }
        "xlsx workbook metadata update" => {
            let title = json_optional_string(args, "title");
            let subject = json_optional_string(args, "subject");
            let creator = json_optional_string(args, "creator");
            let keywords = json_optional_string(args, "keywords");
            let description = json_optional_string(args, "description");
            let last_modified_by = json_optional_string(args, "last-modified-by")
                .or_else(|| json_optional_string(args, "lastModifiedBy"));
            let category = json_optional_string(args, "category");
            let company = json_optional_string(args, "company");
            let manager = json_optional_string(args, "manager");
            let calc_mode = json_optional_string(args, "calc-mode")
                .or_else(|| json_optional_string(args, "calcMode"));
            let full_calc_on_load =
                json_bool(args, "full-calc-on-load").or_else(|| json_bool(args, "fullCalcOnLoad"));
            let expect_title = json_optional_string(args, "expect-title")
                .or_else(|| json_optional_string(args, "expectTitle"));
            let expect_subject = json_optional_string(args, "expect-subject")
                .or_else(|| json_optional_string(args, "expectSubject"));
            let expect_creator = json_optional_string(args, "expect-creator")
                .or_else(|| json_optional_string(args, "expectCreator"));
            let expect_keywords = json_optional_string(args, "expect-keywords")
                .or_else(|| json_optional_string(args, "expectKeywords"));
            let expect_description = json_optional_string(args, "expect-description")
                .or_else(|| json_optional_string(args, "expectDescription"));
            let expect_last_modified_by = json_optional_string(args, "expect-last-modified-by")
                .or_else(|| json_optional_string(args, "expectLastModifiedBy"));
            let expect_category = json_optional_string(args, "expect-category")
                .or_else(|| json_optional_string(args, "expectCategory"));
            let expect_company = json_optional_string(args, "expect-company")
                .or_else(|| json_optional_string(args, "expectCompany"));
            let expect_manager = json_optional_string(args, "expect-manager")
                .or_else(|| json_optional_string(args, "expectManager"));
            let readback = xlsx_workbook_metadata_update(
                working,
                XlsxWorkbookMetadataUpdateOptions {
                    title: title.as_deref(),
                    subject: subject.as_deref(),
                    creator: creator.as_deref(),
                    keywords: keywords.as_deref(),
                    description: description.as_deref(),
                    last_modified_by: last_modified_by.as_deref(),
                    category: category.as_deref(),
                    company: company.as_deref(),
                    manager: manager.as_deref(),
                    calc_mode: calc_mode.as_deref(),
                    full_calc_on_load,
                    expect_title: expect_title.as_deref(),
                    expect_subject: expect_subject.as_deref(),
                    expect_creator: expect_creator.as_deref(),
                    expect_keywords: expect_keywords.as_deref(),
                    expect_description: expect_description.as_deref(),
                    expect_last_modified_by: expect_last_modified_by.as_deref(),
                    expect_category: expect_category.as_deref(),
                    expect_company: expect_company.as_deref(),
                    expect_manager: expect_manager.as_deref(),
                    out: None,
                    backup: None,
                    dry_run: false,
                    no_validate: true,
                    in_place: true,
                },
            )?;
            let mut plan_flags = Vec::new();
            push_serve_plan_string_flag(&mut plan_flags, "--title", title.as_deref());
            push_serve_plan_string_flag(&mut plan_flags, "--subject", subject.as_deref());
            push_serve_plan_string_flag(&mut plan_flags, "--creator", creator.as_deref());
            push_serve_plan_string_flag(&mut plan_flags, "--keywords", keywords.as_deref());
            push_serve_plan_string_flag(&mut plan_flags, "--description", description.as_deref());
            push_serve_plan_string_flag(
                &mut plan_flags,
                "--last-modified-by",
                last_modified_by.as_deref(),
            );
            push_serve_plan_string_flag(&mut plan_flags, "--category", category.as_deref());
            push_serve_plan_string_flag(&mut plan_flags, "--company", company.as_deref());
            push_serve_plan_string_flag(&mut plan_flags, "--manager", manager.as_deref());
            push_serve_plan_string_flag(&mut plan_flags, "--calc-mode", calc_mode.as_deref());
            push_serve_plan_bool_flag(&mut plan_flags, "--full-calc-on-load", full_calc_on_load);
            push_serve_plan_string_flag(&mut plan_flags, "--expect-title", expect_title.as_deref());
            push_serve_plan_string_flag(
                &mut plan_flags,
                "--expect-subject",
                expect_subject.as_deref(),
            );
            push_serve_plan_string_flag(
                &mut plan_flags,
                "--expect-creator",
                expect_creator.as_deref(),
            );
            push_serve_plan_string_flag(
                &mut plan_flags,
                "--expect-keywords",
                expect_keywords.as_deref(),
            );
            push_serve_plan_string_flag(
                &mut plan_flags,
                "--expect-description",
                expect_description.as_deref(),
            );
            push_serve_plan_string_flag(
                &mut plan_flags,
                "--expect-last-modified-by",
                expect_last_modified_by.as_deref(),
            );
            push_serve_plan_string_flag(
                &mut plan_flags,
                "--expect-category",
                expect_category.as_deref(),
            );
            push_serve_plan_string_flag(
                &mut plan_flags,
                "--expect-company",
                expect_company.as_deref(),
            );
            push_serve_plan_string_flag(
                &mut plan_flags,
                "--expect-manager",
                expect_manager.as_deref(),
            );
            ServeOp::XlsxWorkbookMetadataUpdate {
                command: command.to_string(),
                plan_flags,
                readback_file: working.to_string(),
                readback,
            }
        }
        _ => {
            return Err(CliError::invalid_args(format!(
                "unsupported serve op command: {command}"
            )));
        }
    };
    Ok(op)
}
