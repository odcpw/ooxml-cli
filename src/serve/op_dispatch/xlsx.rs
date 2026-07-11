use serde_json::{Value, json};

use crate::command_manifest::{CommandId, XlsxCommandId};
use crate::typed_command_adapter::xlsx_cells_set_by_id;
use crate::{
    CliError, CliResult, XlsxCellsSetOptions, XlsxChartSetSeriesStyleOptions,
    XlsxColWidthsSetOptions, XlsxCommentsAddOptions, XlsxCommentsRemoveOptions,
    XlsxCommentsUpdateOptions, XlsxConditionalFormatMutationOptions, XlsxRangesSetFormatOptions,
    XlsxRangesSetOptions, XlsxRowHeightsSetOptions, XlsxTablesAppendRecordsOptions,
    XlsxTablesAppendRowsOptions, XlsxWorkbookMetadataUpdateOptions, json_bool, json_i64,
    json_optional_serialized, json_optional_string, json_string, xlsx_charts_set_series_style,
    xlsx_colwidths_set, xlsx_comments_add, xlsx_comments_remove, xlsx_comments_update,
    xlsx_conditional_formats_add, xlsx_conditional_formats_delete,
    xlsx_conditional_formats_reorder, xlsx_ranges_set, xlsx_ranges_set_format, xlsx_rowheights_set,
    xlsx_tables_append_records, xlsx_tables_append_rows, xlsx_workbook_metadata_update,
};

use super::super::op::{ServeOp, push_serve_plan_bool_flag, push_serve_plan_string_flag};

pub(super) fn serve_xlsx_op(working: &str, command: &str, args: &Value) -> CliResult<ServeOp> {
    let op = match command {
        "xlsx cells set" => {
            let sheet = json_string(args, "sheet")?;
            let cell = json_string(args, "cell")?;
            let value = json_string(args, "value")?;
            let options = XlsxCellsSetOptions {
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
            };
            let readback =
                xlsx_cells_set_by_id(CommandId::Xlsx(XlsxCommandId::CellsSet), working, options)?;
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
        "xlsx conditional-formats add"
        | "xlsx conditional-formatting add"
        | "xlsx conditional-format add"
        | "xlsx cf add" => {
            let sheet = json_optional_string(args, "sheet");
            let range = json_string(args, "range")?;
            let rule_type = json_optional_string(args, "type");
            let operator = json_optional_string(args, "operator");
            let formula = json_optional_string(args, "formula");
            let formula2 = json_optional_string(args, "formula2");
            let cfvo = json_string_list(args, "cfvo")?;
            let colors = match json_string_list(args, "color")? {
                values if values.is_empty() => json_string_list(args, "colors")?,
                values => values,
            };
            let icon_set = json_optional_string(args, "icon-set")
                .or_else(|| json_optional_string(args, "iconSet"));
            validate_conditional_format_flags(
                rule_type.as_deref(),
                icon_set.as_deref(),
                &cfvo,
                &colors,
            )?;
            let priority = json_i64(args, "priority")?;
            let stop_if_true =
                json_bool(args, "stop-if-true").or_else(|| json_bool(args, "stopIfTrue"));
            let dxf_id = match json_i64(args, "dxf-id")? {
                Some(value) => Some(value),
                None => json_i64(args, "dxfId")?,
            };
            let readback = xlsx_conditional_formats_add(
                working,
                XlsxConditionalFormatMutationOptions {
                    sheet: sheet.as_deref(),
                    range: Some(&range),
                    rule: None,
                    formula: formula.as_deref(),
                    rule_type: rule_type.as_deref(),
                    operator: operator.as_deref(),
                    formula2: formula2.as_deref(),
                    has_formula2: args.get("formula2").is_some(),
                    cfvo: cfvo.clone(),
                    colors: colors.clone(),
                    icon_set: icon_set.as_deref(),
                    priority,
                    stop_if_true: stop_if_true.unwrap_or(false),
                    has_stop_if_true: stop_if_true.is_some(),
                    dxf_id,
                    out: None,
                    backup: None,
                    dry_run: false,
                    no_validate: true,
                    in_place: true,
                },
            )?;
            let mut plan_flags = Vec::new();
            push_serve_plan_string_flag(&mut plan_flags, "--sheet", sheet.as_deref());
            push_serve_plan_string_flag(&mut plan_flags, "--range", Some(&range));
            push_serve_plan_string_flag(&mut plan_flags, "--type", rule_type.as_deref());
            push_serve_plan_string_flag(&mut plan_flags, "--operator", operator.as_deref());
            push_serve_plan_string_flag(&mut plan_flags, "--formula", formula.as_deref());
            push_serve_plan_string_flag(&mut plan_flags, "--formula2", formula2.as_deref());
            push_serve_plan_string_flag(&mut plan_flags, "--icon-set", icon_set.as_deref());
            for value in &cfvo {
                push_serve_plan_string_flag(&mut plan_flags, "--cfvo", Some(value));
            }
            for value in &colors {
                push_serve_plan_string_flag(&mut plan_flags, "--color", Some(value));
            }
            if let Some(priority) = priority {
                plan_flags.push(json!("--priority"));
                plan_flags.push(json!(priority.to_string()));
            }
            push_serve_plan_bool_flag(&mut plan_flags, "--stop-if-true", stop_if_true);
            if let Some(dxf_id) = dxf_id {
                plan_flags.push(json!("--dxf-id"));
                plan_flags.push(json!(dxf_id.to_string()));
            }
            ServeOp::XlsxConditionalFormatsOp {
                command: "xlsx conditional-formats add".to_string(),
                plan_flags,
                readback_file: working.to_string(),
                readback,
            }
        }
        "xlsx conditional-formats delete"
        | "xlsx conditional-formats remove"
        | "xlsx conditional-formatting delete"
        | "xlsx conditional-formatting remove"
        | "xlsx conditional-format delete"
        | "xlsx conditional-format remove"
        | "xlsx cf delete"
        | "xlsx cf remove" => {
            let sheet = json_optional_string(args, "sheet");
            let rule = json_string(args, "rule")?;
            let readback = xlsx_conditional_formats_delete(
                working,
                XlsxConditionalFormatMutationOptions {
                    sheet: sheet.as_deref(),
                    range: None,
                    rule: Some(&rule),
                    formula: None,
                    rule_type: None,
                    operator: None,
                    formula2: None,
                    has_formula2: false,
                    cfvo: Vec::new(),
                    colors: Vec::new(),
                    icon_set: None,
                    priority: None,
                    stop_if_true: false,
                    has_stop_if_true: false,
                    dxf_id: None,
                    out: None,
                    backup: None,
                    dry_run: false,
                    no_validate: true,
                    in_place: true,
                },
            )?;
            let mut plan_flags = Vec::new();
            push_serve_plan_string_flag(&mut plan_flags, "--sheet", sheet.as_deref());
            push_serve_plan_string_flag(&mut plan_flags, "--rule", Some(&rule));
            ServeOp::XlsxConditionalFormatsOp {
                command: "xlsx conditional-formats delete".to_string(),
                plan_flags,
                readback_file: working.to_string(),
                readback,
            }
        }
        "xlsx conditional-formats reorder"
        | "xlsx conditional-formatting reorder"
        | "xlsx conditional-format reorder"
        | "xlsx cf reorder" => {
            let sheet = json_optional_string(args, "sheet");
            let rule = json_string(args, "rule")?;
            let priority = json_i64(args, "priority")?
                .ok_or_else(|| CliError::invalid_args("priority is required"))?;
            let readback = xlsx_conditional_formats_reorder(
                working,
                XlsxConditionalFormatMutationOptions {
                    sheet: sheet.as_deref(),
                    range: None,
                    rule: Some(&rule),
                    formula: None,
                    rule_type: None,
                    operator: None,
                    formula2: None,
                    has_formula2: false,
                    cfvo: Vec::new(),
                    colors: Vec::new(),
                    icon_set: None,
                    priority: Some(priority),
                    stop_if_true: false,
                    has_stop_if_true: false,
                    dxf_id: None,
                    out: None,
                    backup: None,
                    dry_run: false,
                    no_validate: true,
                    in_place: true,
                },
            )?;
            let mut plan_flags = Vec::new();
            push_serve_plan_string_flag(&mut plan_flags, "--sheet", sheet.as_deref());
            push_serve_plan_string_flag(&mut plan_flags, "--rule", Some(&rule));
            plan_flags.push(json!("--priority"));
            plan_flags.push(json!(priority.to_string()));
            ServeOp::XlsxConditionalFormatsOp {
                command: "xlsx conditional-formats reorder".to_string(),
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
        "xlsx charts set-series-style" => {
            let sheet = json_optional_string(args, "sheet");
            let chart = json_optional_string(args, "chart");
            let series = json_i64(args, "series")?.unwrap_or(1);
            let fill_color = json_optional_string(args, "fill-color")
                .or_else(|| json_optional_string(args, "fillColor"));
            let line_color = json_optional_string(args, "line-color")
                .or_else(|| json_optional_string(args, "lineColor"));
            let line_width_text = match json_optional_number_string(args, "line-width-pt")? {
                Some(value) => Some(value),
                None => json_optional_number_string(args, "lineWidthPt")?,
            };
            let line_width_pt = line_width_text
                .as_deref()
                .map(|value| parse_json_f64_arg(value, "line-width-pt"))
                .transpose()?;
            let marker_symbol = json_optional_string(args, "marker-symbol")
                .or_else(|| json_optional_string(args, "markerSymbol"));
            let marker_size = match json_i64(args, "marker-size")? {
                Some(value) => Some(value),
                None => json_i64(args, "markerSize")?,
            };
            let expect_series_count = match json_i64(args, "expect-series-count")? {
                Some(value) => Some(value),
                None => json_i64(args, "expectSeriesCount")?,
            };
            let readback = xlsx_charts_set_series_style(
                working,
                XlsxChartSetSeriesStyleOptions {
                    sheet: sheet.as_deref(),
                    chart: chart.as_deref(),
                    series,
                    fill_color: fill_color.as_deref(),
                    line_color: line_color.as_deref(),
                    line_width_pt,
                    marker_symbol: marker_symbol.as_deref(),
                    marker_size,
                    expect_series_count,
                    out: None,
                    backup: None,
                    dry_run: false,
                    no_validate: true,
                    in_place: true,
                },
            )?;
            let mut plan_flags = Vec::new();
            push_serve_plan_string_flag(&mut plan_flags, "--sheet", sheet.as_deref());
            push_serve_plan_string_flag(&mut plan_flags, "--chart", chart.as_deref());
            plan_flags.push(json!("--series"));
            plan_flags.push(json!(series.to_string()));
            push_serve_plan_string_flag(&mut plan_flags, "--fill-color", fill_color.as_deref());
            push_serve_plan_string_flag(&mut plan_flags, "--line-color", line_color.as_deref());
            push_serve_plan_string_flag(
                &mut plan_flags,
                "--line-width-pt",
                line_width_text.as_deref(),
            );
            push_serve_plan_string_flag(
                &mut plan_flags,
                "--marker-symbol",
                marker_symbol.as_deref(),
            );
            if let Some(marker_size) = marker_size {
                plan_flags.push(json!("--marker-size"));
                plan_flags.push(json!(marker_size.to_string()));
            }
            if let Some(expect_series_count) = expect_series_count {
                plan_flags.push(json!("--expect-series-count"));
                plan_flags.push(json!(expect_series_count.to_string()));
            }
            ServeOp::XlsxChartsOp {
                command: command.to_string(),
                plan_flags,
                readback_file: working.to_string(),
                readback,
            }
        }
        "xlsx colwidths set" => {
            let sheet = json_optional_string(args, "sheet");
            let range = json_string(args, "range")?;
            let width_text = json_number_string(args, "width")?;
            let expect_width_text = match json_optional_number_string(args, "expect-width")? {
                Some(value) => Some(value),
                None => json_optional_number_string(args, "expectWidth")?,
            };
            let width = parse_json_f64_arg(&width_text, "width")?;
            let expect_width = expect_width_text
                .as_deref()
                .map(|value| parse_json_f64_arg(value, "expect-width"))
                .transpose()?;
            let readback = xlsx_colwidths_set(
                working,
                XlsxColWidthsSetOptions {
                    sheet: sheet.as_deref(),
                    range: &range,
                    width: Some(width),
                    expect_width,
                    out: None,
                    backup: None,
                    dry_run: false,
                    no_validate: true,
                    in_place: true,
                },
            )?;
            let mut plan_flags = Vec::new();
            push_serve_plan_string_flag(&mut plan_flags, "--sheet", sheet.as_deref());
            push_serve_plan_string_flag(&mut plan_flags, "--range", Some(&range));
            push_serve_plan_string_flag(&mut plan_flags, "--width", Some(&width_text));
            push_serve_plan_string_flag(
                &mut plan_flags,
                "--expect-width",
                expect_width_text.as_deref(),
            );
            ServeOp::XlsxDimensionsOp {
                command: command.to_string(),
                plan_flags,
                readback_file: working.to_string(),
                readback,
            }
        }
        "xlsx rowheights set" => {
            let sheet = json_optional_string(args, "sheet");
            let range = json_string(args, "range")?;
            let height_text = json_number_string(args, "height")?;
            let expect_height_text = match json_optional_number_string(args, "expect-height")? {
                Some(value) => Some(value),
                None => json_optional_number_string(args, "expectHeight")?,
            };
            let height = parse_json_f64_arg(&height_text, "height")?;
            let expect_height = expect_height_text
                .as_deref()
                .map(|value| parse_json_f64_arg(value, "expect-height"))
                .transpose()?;
            let readback = xlsx_rowheights_set(
                working,
                XlsxRowHeightsSetOptions {
                    sheet: sheet.as_deref(),
                    range: &range,
                    height: Some(height),
                    expect_height,
                    out: None,
                    backup: None,
                    dry_run: false,
                    no_validate: true,
                    in_place: true,
                },
            )?;
            let mut plan_flags = Vec::new();
            push_serve_plan_string_flag(&mut plan_flags, "--sheet", sheet.as_deref());
            push_serve_plan_string_flag(&mut plan_flags, "--range", Some(&range));
            push_serve_plan_string_flag(&mut plan_flags, "--height", Some(&height_text));
            push_serve_plan_string_flag(
                &mut plan_flags,
                "--expect-height",
                expect_height_text.as_deref(),
            );
            ServeOp::XlsxDimensionsOp {
                command: command.to_string(),
                plan_flags,
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

fn json_number_string(args: &Value, field: &str) -> CliResult<String> {
    json_optional_number_string(args, field)?
        .ok_or_else(|| CliError::invalid_args(format!("{field} is required")))
}

fn json_string_list(args: &Value, field: &str) -> CliResult<Vec<String>> {
    let Some(value) = args.get(field) else {
        return Ok(Vec::new());
    };
    match value {
        Value::String(text) => Ok(vec![text.clone()]),
        Value::Array(items) => items
            .iter()
            .map(|item| {
                item.as_str()
                    .map(ToString::to_string)
                    .ok_or_else(|| CliError::invalid_args(format!("{field} must contain strings")))
            })
            .collect(),
        _ => Err(CliError::invalid_args(format!(
            "{field} must be a string or string array"
        ))),
    }
}

fn validate_conditional_format_flags(
    rule_type: Option<&str>,
    icon_set: Option<&str>,
    cfvo: &[String],
    colors: &[String],
) -> CliResult<()> {
    match rule_type.map(str::trim) {
        Some("data-bar" | "dataBar") => {
            if icon_set.is_some() {
                return Err(CliError::invalid_args(
                    "--icon-set is only valid with --type icon-set",
                ));
            }
            if cfvo.len() != 2 {
                return Err(CliError::invalid_args(
                    "--type data-bar requires exactly two --cfvo values",
                ));
            }
            if colors.len() != 1 {
                return Err(CliError::invalid_args(
                    "--type data-bar requires exactly one --color value",
                ));
            }
        }
        Some("icon-set" | "iconSet") => {
            let icon_set = icon_set
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| CliError::invalid_args("--type icon-set requires --icon-set"))?;
            if !colors.is_empty() {
                return Err(CliError::invalid_args(
                    "--type icon-set does not accept --color",
                ));
            }
            let expected = expected_icon_set_cfvo_count(icon_set)?;
            if cfvo.len() != expected {
                return Err(CliError::invalid_args(format!(
                    "--type icon-set with {icon_set} requires exactly {expected} --cfvo values",
                )));
            }
        }
        _ => {
            if icon_set.is_some() {
                return Err(CliError::invalid_args(
                    "--icon-set is only valid with --type icon-set",
                ));
            }
        }
    }
    Ok(())
}

fn expected_icon_set_cfvo_count(icon_set: &str) -> CliResult<usize> {
    match icon_set.as_bytes().first().copied() {
        Some(b'3' | b'4' | b'5') => Ok((icon_set.as_bytes()[0] - b'0') as usize),
        _ => Err(CliError::invalid_args(
            "icon-set must begin with 3, 4, or 5",
        )),
    }
}

fn json_optional_number_string(args: &Value, field: &str) -> CliResult<Option<String>> {
    let Some(value) = args.get(field) else {
        return Ok(None);
    };
    match value {
        Value::String(text) => Ok(Some(text.clone())),
        Value::Number(number) => Ok(Some(number.to_string())),
        _ => Err(CliError::invalid_args(format!("{field} must be a number"))),
    }
}

fn parse_json_f64_arg(value: &str, field: &str) -> CliResult<f64> {
    value
        .parse::<f64>()
        .map_err(|_| CliError::invalid_args(format!("{field} must be a number")))
}
