use serde_json::{Value, json};

use crate::{
    CliError, CliResult, XlsxCellsSetOptions, XlsxRangesSetFormatOptions, XlsxRangesSetOptions,
    XlsxWorkbookMetadataUpdateOptions, json_bool, json_i64, json_optional_serialized,
    json_optional_string, json_string, xlsx_cells_set, xlsx_ranges_set, xlsx_ranges_set_format,
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
