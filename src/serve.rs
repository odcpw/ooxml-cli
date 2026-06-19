use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, Write};
use std::path::Path;

use crate::{
    CliError, CliResult, DocxCommentEditSpec, DocxHeaderFooterSetTextOptions,
    DocxParagraphMutationOptions, DocxStyleApplyOptions, DocxStyleTarget, EXIT_INVALID_ARGS,
    EXIT_SUCCESS, EXIT_UNEXPECTED, XlsxCellsSetOptions, XlsxRangeExportOptions,
    XlsxRangesSetFormatOptions, XlsxRangesSetOptions, XlsxTableExportOptions,
    XlsxWorkbookMetadataUpdateOptions, current_utc_rfc3339, docx_blocks_delete,
    docx_blocks_insert_after, docx_blocks_replace, docx_blocks_show, docx_comments_add,
    docx_comments_edit, docx_comments_list, docx_comments_remove, docx_fields_insert,
    docx_fields_list, docx_fields_set_result, docx_header_footer_kind,
    docx_header_footer_show_json_args, docx_headers_footers_list, docx_headers_footers_set_text,
    docx_headers_footers_show, docx_images_list, docx_paragraphs_append, docx_paragraphs_clear,
    docx_paragraphs_insert, docx_paragraphs_set, docx_styles_apply, docx_styles_list,
    docx_styles_show, docx_tables_clear_cell, docx_tables_set_cell, docx_tables_show, docx_text,
    json_bool, json_i64, json_optional_serialized, json_optional_string, json_string, json_u32,
    normalize_docx_header_footer_show_type, normalize_docx_style_target, package_type,
    pptx_comments_list, pptx_extract_notes, pptx_extract_text, pptx_extract_text_json_args,
    pptx_layouts_list, pptx_layouts_show, pptx_masters_list, pptx_masters_show, pptx_notes_show,
    pptx_replace_text_in_place, pptx_replace_text_readback, pptx_shapes_show, pptx_slide_selectors,
    pptx_slide_show, pptx_slides_list, pptx_tables_show, require_docx_block_hash,
    require_json_data_format, resolve_required_docx_paragraph_set_text,
    resolve_required_docx_table_text, validate, validate_exit_code, validate_positive_i64,
    xlsx_cells_extract, xlsx_cells_set, xlsx_names_list, xlsx_names_show,
    xlsx_range_export_with_options, xlsx_ranges_set, xlsx_ranges_set_format, xlsx_sheets_list,
    xlsx_sheets_show, xlsx_tables_export, xlsx_tables_list, xlsx_tables_show,
    xlsx_workbook_metadata_inspect, xlsx_workbook_metadata_update,
};
pub(crate) fn run_serve_stdio() -> i32 {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    let mut state = ServeState::default();
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(line) => line,
            Err(err) => {
                let _ = writeln!(std::io::stderr(), "serve read error: {err}");
                return EXIT_UNEXPECTED;
            }
        };
        if line.trim().is_empty() {
            continue;
        }
        let request: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(err) => {
                let _ = writeln!(std::io::stderr(), "serve JSON parse error: {err}");
                return EXIT_INVALID_ARGS;
            }
        };
        let response = state.handle_rpc(request);
        if writeln!(
            stdout,
            "{}",
            serde_json::to_string(&response).expect("serialize rpc response")
        )
        .is_err()
        {
            return EXIT_UNEXPECTED;
        }
        if stdout.flush().is_err() {
            return EXIT_UNEXPECTED;
        }
    }
    EXIT_SUCCESS
}

#[derive(Default)]
pub(crate) struct ServeState {
    next_session: usize,
    sessions: BTreeMap<String, ServeSession>,
}

struct ServeSession {
    file: String,
    out: Option<String>,
    in_place: bool,
    backup: Option<String>,
    no_validate: bool,
    dry_run: bool,
    working: String,
    ops: Vec<ServeOp>,
}

#[derive(Clone)]
enum ServeOp {
    XlsxCellSet {
        command: String,
        plan_flags: Vec<Value>,
        readback_file: String,
        readback: Value,
    },
    PptxReplaceText {
        command: String,
        slide: u32,
        target: String,
        text: String,
    },
    XlsxRangeSet {
        command: String,
        sheet: String,
        range: Option<String>,
        anchor: Option<String>,
        values: Option<String>,
        values_file: Option<String>,
        data_format: Option<String>,
        null_policy: Option<String>,
        ragged: Option<String>,
        max_cells: i64,
        overwrite_formulas: bool,
        readback_file: String,
        readback: Value,
    },
    XlsxRangeSetFormat {
        command: String,
        sheet: String,
        range: String,
        preset: Option<String>,
        format_code: Option<String>,
        decimals: i64,
        currency_symbol: Option<String>,
        max_cells: i64,
        readback_file: String,
        readback: Value,
    },
    XlsxWorkbookMetadataUpdate {
        command: String,
        plan_flags: Vec<Value>,
        readback_file: String,
        readback: Value,
    },
    DocxHeaderFooterSetText {
        command: String,
        plan_flags: Vec<Value>,
        readback_file: String,
        readback: Value,
    },
    DocxFieldsOp {
        command: String,
        plan_flags: Vec<Value>,
        readback_file: String,
        readback: Value,
    },
    DocxBlocksOp {
        command: String,
        plan_flags: Vec<Value>,
        readback_file: String,
        readback: Value,
    },
    DocxParagraphsOp {
        command: String,
        plan_flags: Vec<Value>,
        readback_file: String,
        readback: Value,
    },
    DocxStylesOp {
        command: String,
        plan_flags: Vec<Value>,
        readback_file: String,
        readback: Value,
    },
    DocxTablesOp {
        command: String,
        plan_flags: Vec<Value>,
        readback_file: String,
        readback: Value,
    },
    DocxCommentsOp {
        command: String,
        plan_flags: Vec<Value>,
        readback_file: String,
        readback: Value,
    },
}

fn push_serve_plan_string_flag(flags: &mut Vec<Value>, name: &str, value: Option<&str>) {
    if let Some(value) = value {
        flags.push(json!(name));
        flags.push(json!(value));
    }
}

fn push_serve_plan_bool_flag(flags: &mut Vec<Value>, name: &str, value: Option<bool>) {
    match value {
        Some(true) => flags.push(json!(name)),
        Some(false) => flags.push(json!(format!("{name}=false"))),
        None => {}
    }
}

impl ServeOp {
    fn command(&self) -> &str {
        match self {
            ServeOp::XlsxCellSet { command, .. }
            | ServeOp::PptxReplaceText { command, .. }
            | ServeOp::XlsxRangeSet { command, .. }
            | ServeOp::XlsxRangeSetFormat { command, .. }
            | ServeOp::XlsxWorkbookMetadataUpdate { command, .. }
            | ServeOp::DocxHeaderFooterSetText { command, .. }
            | ServeOp::DocxFieldsOp { command, .. }
            | ServeOp::DocxBlocksOp { command, .. }
            | ServeOp::DocxParagraphsOp { command, .. }
            | ServeOp::DocxStylesOp { command, .. }
            | ServeOp::DocxTablesOp { command, .. }
            | ServeOp::DocxCommentsOp { command, .. } => command,
        }
    }

    fn plan_argv(&self, source_file: &str) -> Value {
        match self {
            ServeOp::XlsxCellSet { plan_flags, .. } => {
                let mut argv = vec![
                    json!("xlsx"),
                    json!("cells"),
                    json!("set"),
                    json!(source_file),
                ];
                argv.extend(plan_flags.iter().cloned());
                argv.extend([
                    json!("--out"),
                    json!("<temp.0>"),
                    json!("--json"),
                    json!("--no-validate"),
                ]);
                Value::Array(argv)
            }
            ServeOp::XlsxRangeSet {
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
                ..
            } => {
                let mut argv = vec![
                    json!("xlsx"),
                    json!("ranges"),
                    json!("set"),
                    json!(source_file),
                    json!("--sheet"),
                    json!(sheet),
                ];
                if let Some(range) = range {
                    argv.push(json!("--range"));
                    argv.push(json!(range));
                }
                if let Some(anchor) = anchor {
                    argv.push(json!("--anchor"));
                    argv.push(json!(anchor));
                }
                if let Some(values) = values {
                    argv.push(json!("--values"));
                    argv.push(json!(values));
                }
                if let Some(values_file) = values_file {
                    argv.push(json!("--values-file"));
                    argv.push(json!(values_file));
                }
                if let Some(data_format) = data_format {
                    argv.push(json!("--data-format"));
                    argv.push(json!(data_format));
                }
                if let Some(null_policy) = null_policy {
                    argv.push(json!("--null-policy"));
                    argv.push(json!(null_policy));
                }
                if let Some(ragged) = ragged {
                    argv.push(json!("--ragged"));
                    argv.push(json!(ragged));
                }
                if *max_cells != 100000 {
                    argv.push(json!("--max-cells"));
                    argv.push(json!(max_cells.to_string()));
                }
                argv.push(json!("--out"));
                argv.push(json!("<temp.0>"));
                argv.push(json!("--json"));
                argv.push(json!("--no-validate"));
                if *overwrite_formulas {
                    argv.push(json!("--overwrite-formulas"));
                }
                Value::Array(argv)
            }
            ServeOp::XlsxRangeSetFormat {
                sheet,
                range,
                preset,
                format_code,
                decimals,
                currency_symbol,
                max_cells,
                ..
            } => {
                let mut argv = vec![
                    json!("xlsx"),
                    json!("ranges"),
                    json!("set-format"),
                    json!(source_file),
                    json!("--sheet"),
                    json!(sheet),
                    json!("--range"),
                    json!(range),
                ];
                if let Some(preset) = preset {
                    argv.push(json!("--preset"));
                    argv.push(json!(preset));
                }
                if let Some(format_code) = format_code {
                    argv.push(json!("--format-code"));
                    argv.push(json!(format_code));
                }
                if *decimals != 2 {
                    argv.push(json!("--decimals"));
                    argv.push(json!(decimals.to_string()));
                }
                if let Some(currency_symbol) = currency_symbol {
                    argv.push(json!("--currency-symbol"));
                    argv.push(json!(currency_symbol));
                }
                if *max_cells != 100000 {
                    argv.push(json!("--max-cells"));
                    argv.push(json!(max_cells.to_string()));
                }
                argv.extend([
                    json!("--out"),
                    json!("<temp.0>"),
                    json!("--json"),
                    json!("--no-validate"),
                ]);
                Value::Array(argv)
            }
            ServeOp::XlsxWorkbookMetadataUpdate { plan_flags, .. } => {
                let mut argv = vec![
                    json!("xlsx"),
                    json!("workbook"),
                    json!("metadata"),
                    json!("update"),
                    json!(source_file),
                ];
                argv.extend(plan_flags.iter().cloned());
                argv.extend([
                    json!("--out"),
                    json!("<temp.0>"),
                    json!("--json"),
                    json!("--no-validate"),
                ]);
                Value::Array(argv)
            }
            ServeOp::DocxHeaderFooterSetText {
                command,
                plan_flags,
                ..
            } => {
                let parts = command.split_whitespace().collect::<Vec<_>>();
                let group = parts.get(1).copied().unwrap_or("headers");
                let mut argv = vec![
                    json!("docx"),
                    json!(group),
                    json!("set-text"),
                    json!(source_file),
                ];
                argv.extend(plan_flags.iter().cloned());
                argv.extend([
                    json!("--out"),
                    json!("<temp.0>"),
                    json!("--json"),
                    json!("--no-validate"),
                ]);
                Value::Array(argv)
            }
            ServeOp::DocxFieldsOp {
                command,
                plan_flags,
                ..
            } => {
                let verb = command.split_whitespace().nth(2).unwrap_or("set-result");
                let mut argv = vec![
                    json!("docx"),
                    json!("fields"),
                    json!(verb),
                    json!(source_file),
                ];
                argv.extend(plan_flags.iter().cloned());
                argv.extend([
                    json!("--out"),
                    json!("<temp.0>"),
                    json!("--json"),
                    json!("--no-validate"),
                ]);
                Value::Array(argv)
            }
            ServeOp::DocxBlocksOp {
                command,
                plan_flags,
                ..
            } => {
                let verb = command.split_whitespace().nth(2).unwrap_or("replace");
                let mut argv = vec![
                    json!("docx"),
                    json!("blocks"),
                    json!(verb),
                    json!(source_file),
                ];
                argv.extend(plan_flags.iter().cloned());
                argv.extend([
                    json!("--out"),
                    json!("<temp.0>"),
                    json!("--json"),
                    json!("--no-validate"),
                ]);
                Value::Array(argv)
            }
            ServeOp::DocxParagraphsOp {
                command,
                plan_flags,
                ..
            } => {
                let verb = command.split_whitespace().nth(2).unwrap_or("append");
                let mut argv = vec![
                    json!("docx"),
                    json!("paragraphs"),
                    json!(verb),
                    json!(source_file),
                ];
                argv.extend(plan_flags.iter().cloned());
                argv.extend([
                    json!("--out"),
                    json!("<temp.0>"),
                    json!("--json"),
                    json!("--no-validate"),
                ]);
                Value::Array(argv)
            }
            ServeOp::DocxStylesOp {
                command,
                plan_flags,
                ..
            } => {
                let verb = command.split_whitespace().nth(2).unwrap_or("apply");
                let mut argv = vec![
                    json!("docx"),
                    json!("styles"),
                    json!(verb),
                    json!(source_file),
                ];
                argv.extend(plan_flags.iter().cloned());
                argv.extend([json!("--out"), json!("<temp.0>"), json!("--json")]);
                Value::Array(argv)
            }
            ServeOp::DocxTablesOp {
                command,
                plan_flags,
                ..
            } => {
                let verb = command.split_whitespace().nth(2).unwrap_or("set-cell");
                let mut argv = vec![
                    json!("docx"),
                    json!("tables"),
                    json!(verb),
                    json!(source_file),
                ];
                argv.extend(plan_flags.iter().cloned());
                argv.extend([
                    json!("--out"),
                    json!("<temp.0>"),
                    json!("--json"),
                    json!("--no-validate"),
                ]);
                Value::Array(argv)
            }
            ServeOp::DocxCommentsOp {
                command,
                plan_flags,
                ..
            } => {
                let verb = command.split_whitespace().nth(2).unwrap_or("add");
                let mut argv = vec![
                    json!("docx"),
                    json!("comments"),
                    json!(verb),
                    json!(source_file),
                ];
                argv.extend(plan_flags.iter().cloned());
                argv.extend([
                    json!("--out"),
                    json!("<temp.0>"),
                    json!("--json"),
                    json!("--no-validate"),
                ]);
                Value::Array(argv)
            }
            ServeOp::PptxReplaceText {
                slide,
                target,
                text,
                ..
            } => json!([
                "pptx",
                "replace",
                "text",
                source_file,
                "--slide",
                slide.to_string(),
                "--target",
                target,
                "--text",
                text,
                "--out",
                "<temp.0>",
                "--json",
                "--no-validate",
            ]),
        }
    }

    fn readback(&self, file: &str) -> Value {
        match self {
            ServeOp::PptxReplaceText {
                slide,
                target,
                text,
                ..
            } => pptx_replace_text_readback(file, file, *slide, target, text),
            ServeOp::XlsxCellSet {
                readback_file,
                readback,
                ..
            }
            | ServeOp::XlsxRangeSet {
                readback_file,
                readback,
                ..
            } => replace_json_string(readback.clone(), readback_file, file),
            ServeOp::XlsxRangeSetFormat {
                readback_file,
                readback,
                ..
            } => replace_json_string(readback.clone(), readback_file, file),
            ServeOp::XlsxWorkbookMetadataUpdate {
                readback_file,
                readback,
                ..
            }
            | ServeOp::DocxHeaderFooterSetText {
                readback_file,
                readback,
                ..
            }
            | ServeOp::DocxFieldsOp {
                readback_file,
                readback,
                ..
            }
            | ServeOp::DocxBlocksOp {
                readback_file,
                readback,
                ..
            }
            | ServeOp::DocxCommentsOp {
                readback_file,
                readback,
                ..
            }
            | ServeOp::DocxParagraphsOp {
                readback_file,
                readback,
                ..
            }
            | ServeOp::DocxStylesOp {
                readback_file,
                readback,
                ..
            }
            | ServeOp::DocxTablesOp {
                readback_file,
                readback,
                ..
            } => replace_json_string(readback.clone(), readback_file, file),
        }
    }
}

fn replace_json_string(value: Value, from: &str, to: &str) -> Value {
    match value {
        Value::String(text) => Value::String(text.replace(from, to)),
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(|item| replace_json_string(item, from, to))
                .collect(),
        ),
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(key, value)| (key, replace_json_string(value, from, to)))
                .collect(),
        ),
        other => other,
    }
}

impl ServeState {
    fn handle_rpc(&mut self, request: Value) -> Value {
        let id = request.get("id").cloned().unwrap_or(Value::Null);
        let method = request
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
        match self.handle_method(method, &params) {
            Ok(result) => json!({"id": id, "jsonrpc": "2.0", "result": result}),
            Err(err) => json!({
                "id": id,
                "jsonrpc": "2.0",
                "error": {
                    "code": err.exit_code,
                    "message": err.message,
                    "data": {"type": err.code, "exitCode": err.exit_code},
                },
            }),
        }
    }

    pub(crate) fn handle_method(&mut self, method: &str, params: &Value) -> CliResult<Value> {
        match method {
            "open" => self.serve_open(params),
            "op" => self.serve_op(params),
            "inspect" => self.serve_inspect(params),
            "validate" => self.serve_validate(params),
            "plan" => self.serve_plan(params),
            "commit" => self.serve_commit(params),
            "abort" => self.serve_abort(params),
            _ => Err(CliError::invalid_args(format!(
                "unsupported serve method: {method}"
            ))),
        }
    }

    fn serve_open(&mut self, params: &Value) -> CliResult<Value> {
        let file = json_string(params, "file")?;
        let out = json_optional_string(params, "out");
        let in_place = json_bool(params, "inPlace").unwrap_or(false);
        let backup = json_optional_string(params, "backup");
        let no_validate = json_bool(params, "noValidate").unwrap_or(false);
        let dry_run = params
            .get("dryRun")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if out.is_some() && in_place {
            return Err(CliError::invalid_args(
                "cannot specify both out and inPlace",
            ));
        }
        if backup
            .as_ref()
            .is_some_and(|value| !value.trim().is_empty())
            && !in_place
        {
            return Err(CliError::invalid_args(
                "backup can only be used with inPlace",
            ));
        }
        self.next_session += 1;
        let session_id = format!("rust-session-{}", self.next_session);
        let working = make_working_copy(&file, self.next_session)?;
        self.sessions.insert(
            session_id.clone(),
            ServeSession {
                file: file.clone(),
                out,
                in_place,
                backup,
                no_validate,
                dry_run,
                working,
                ops: Vec::new(),
            },
        );
        Ok(json!({"sessionId": session_id, "type": package_type(&file)?}))
    }

    fn serve_op(&mut self, params: &Value) -> CliResult<Value> {
        let session_id = json_string(params, "session")?;
        let command = json_string(params, "command")?;
        let args = params
            .get("args")
            .ok_or_else(|| CliError::invalid_args("op args are required"))?;
        let session = self.session_mut(&session_id)?;
        let op = match command.as_str() {
            "xlsx cells set" => {
                let sheet = json_string(args, "sheet")?;
                let cell = json_string(args, "cell")?;
                let value = json_string(args, "value")?;
                let readback = xlsx_cells_set(
                    &session.working,
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
                    command: command.clone(),
                    plan_flags,
                    readback_file: session.working.clone(),
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
                    &session.working,
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
                    command: command.clone(),
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
                    readback_file: session.working.clone(),
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
                    &session.working,
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
                    command: command.clone(),
                    sheet,
                    range,
                    preset,
                    format_code,
                    decimals,
                    currency_symbol,
                    max_cells,
                    readback_file: session.working.clone(),
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
                let full_calc_on_load = json_bool(args, "full-calc-on-load")
                    .or_else(|| json_bool(args, "fullCalcOnLoad"));
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
                    &session.working,
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
                push_serve_plan_string_flag(
                    &mut plan_flags,
                    "--description",
                    description.as_deref(),
                );
                push_serve_plan_string_flag(
                    &mut plan_flags,
                    "--last-modified-by",
                    last_modified_by.as_deref(),
                );
                push_serve_plan_string_flag(&mut plan_flags, "--category", category.as_deref());
                push_serve_plan_string_flag(&mut plan_flags, "--company", company.as_deref());
                push_serve_plan_string_flag(&mut plan_flags, "--manager", manager.as_deref());
                push_serve_plan_string_flag(&mut plan_flags, "--calc-mode", calc_mode.as_deref());
                push_serve_plan_bool_flag(
                    &mut plan_flags,
                    "--full-calc-on-load",
                    full_calc_on_load,
                );
                push_serve_plan_string_flag(
                    &mut plan_flags,
                    "--expect-title",
                    expect_title.as_deref(),
                );
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
                    command: command.clone(),
                    plan_flags,
                    readback_file: session.working.clone(),
                    readback,
                }
            }
            "docx headers set-text" | "docx footers set-text" => {
                let kind = if command.contains("footers") {
                    "footer"
                } else {
                    "header"
                };
                let id = json_optional_string(args, "id").unwrap_or_default();
                let ref_type =
                    json_optional_string(args, "type").unwrap_or_else(|| "default".to_string());
                let ref_type = normalize_docx_header_footer_show_type(&ref_type)?;
                let section_value = json_i64(args, "section")?;
                let section = section_value.unwrap_or(0);
                let index_value = json_i64(args, "index")?;
                let index = index_value.unwrap_or(1);
                let selector = json_optional_string(args, "selector");
                let text = json_optional_string(args, "text");
                let text_file = json_optional_string(args, "text-file")
                    .or_else(|| json_optional_string(args, "textFile"));
                let text_set = args.get("text").is_some();
                let text_file_set =
                    args.get("text-file").is_some() || args.get("textFile").is_some();
                let text = resolve_required_docx_table_text(
                    text.as_deref(),
                    text_file.as_deref(),
                    text_set,
                    text_file_set,
                )?;
                let readback = docx_headers_footers_set_text(
                    &session.working,
                    kind,
                    DocxHeaderFooterSetTextOptions {
                        id: &id,
                        ref_type: &ref_type,
                        section,
                        index,
                        selector: selector.as_deref(),
                        selector_given: selector.is_some(),
                        index_given: index_value.is_some(),
                        text: &text,
                        out: None,
                        backup: None,
                        dry_run: false,
                        in_place: true,
                        no_validate: true,
                    },
                )?;
                let mut plan_flags = Vec::new();
                push_serve_plan_string_flag(
                    &mut plan_flags,
                    "--id",
                    (!id.is_empty()).then_some(id.as_str()),
                );
                if args.get("type").is_some() {
                    push_serve_plan_string_flag(&mut plan_flags, "--type", Some(ref_type.as_str()));
                }
                if let Some(section) = section_value {
                    plan_flags.push(json!("--section"));
                    plan_flags.push(json!(section.to_string()));
                }
                if let Some(index) = index_value {
                    plan_flags.push(json!("--index"));
                    plan_flags.push(json!(index.to_string()));
                }
                push_serve_plan_string_flag(&mut plan_flags, "--selector", selector.as_deref());
                if text_set {
                    push_serve_plan_string_flag(&mut plan_flags, "--text", Some(text.as_str()));
                }
                if text_file_set {
                    push_serve_plan_string_flag(
                        &mut plan_flags,
                        "--text-file",
                        text_file.as_deref(),
                    );
                }
                ServeOp::DocxHeaderFooterSetText {
                    command: command.clone(),
                    plan_flags,
                    readback_file: session.working.clone(),
                    readback,
                }
            }
            "docx fields insert" => {
                let location = json_string(args, "location")?;
                let field_code = json_optional_string(args, "field-code")
                    .or_else(|| json_optional_string(args, "fieldCode"))
                    .ok_or_else(|| CliError::invalid_args("field-code is required"))?;
                let result = json_optional_string(args, "result").unwrap_or_default();
                let result_set = args.get("result").is_some();
                let readback = docx_fields_insert(
                    &session.working,
                    &location,
                    &field_code,
                    &result,
                    DocxParagraphMutationOptions {
                        text: None,
                        text_file: None,
                        style: "",
                        out: None,
                        backup: None,
                        dry_run: false,
                        in_place: true,
                        no_validate: true,
                    },
                )?;
                let mut plan_flags = Vec::new();
                push_serve_plan_string_flag(&mut plan_flags, "--location", Some(&location));
                push_serve_plan_string_flag(&mut plan_flags, "--field-code", Some(&field_code));
                if result_set {
                    push_serve_plan_string_flag(&mut plan_flags, "--result", Some(&result));
                }
                ServeOp::DocxFieldsOp {
                    command: command.clone(),
                    plan_flags,
                    readback_file: session.working.clone(),
                    readback,
                }
            }
            "docx fields set-result" => {
                let selector = json_string(args, "selector")?;
                if args.get("result").is_none() {
                    return Err(CliError::invalid_args("result is required"));
                }
                let result = json_optional_string(args, "result").unwrap_or_default();
                let expect_hash = json_optional_string(args, "expect-hash")
                    .or_else(|| json_optional_string(args, "expectHash"))
                    .unwrap_or_default();
                let readback = docx_fields_set_result(
                    &session.working,
                    &selector,
                    &result,
                    &expect_hash,
                    DocxParagraphMutationOptions {
                        text: None,
                        text_file: None,
                        style: "",
                        out: None,
                        backup: None,
                        dry_run: false,
                        in_place: true,
                        no_validate: true,
                    },
                )?;
                let mut plan_flags = Vec::new();
                push_serve_plan_string_flag(&mut plan_flags, "--selector", Some(&selector));
                push_serve_plan_string_flag(&mut plan_flags, "--result", Some(&result));
                push_serve_plan_string_flag(
                    &mut plan_flags,
                    "--expect-hash",
                    (!expect_hash.is_empty()).then_some(expect_hash.as_str()),
                );
                ServeOp::DocxFieldsOp {
                    command: command.clone(),
                    plan_flags,
                    readback_file: session.working.clone(),
                    readback,
                }
            }
            "docx paragraphs append" => {
                let text = json_optional_string(args, "text");
                let text_file = json_optional_string(args, "text-file")
                    .or_else(|| json_optional_string(args, "textFile"));
                let style = json_optional_string(args, "style").unwrap_or_default();
                let readback = docx_paragraphs_append(
                    &session.working,
                    DocxParagraphMutationOptions {
                        text: text.as_deref(),
                        text_file: text_file.as_deref(),
                        style: &style,
                        out: None,
                        backup: None,
                        dry_run: false,
                        in_place: true,
                        no_validate: true,
                    },
                )?;
                let mut plan_flags = Vec::new();
                push_serve_plan_string_flag(&mut plan_flags, "--text", text.as_deref());
                push_serve_plan_string_flag(&mut plan_flags, "--text-file", text_file.as_deref());
                push_serve_plan_string_flag(
                    &mut plan_flags,
                    "--style",
                    (!style.is_empty()).then_some(style.as_str()),
                );
                ServeOp::DocxParagraphsOp {
                    command: command.clone(),
                    plan_flags,
                    readback_file: session.working.clone(),
                    readback,
                }
            }
            "docx paragraphs insert" => {
                let insert_after = match json_i64(args, "insert-after")? {
                    Some(value) => value,
                    None => json_i64(args, "insertAfter")?.unwrap_or(0),
                };
                if insert_after < 0 {
                    return Err(CliError::invalid_args("--insert-after must be >= 0"));
                }
                let text = json_optional_string(args, "text");
                let text_file = json_optional_string(args, "text-file")
                    .or_else(|| json_optional_string(args, "textFile"));
                let style = json_optional_string(args, "style").unwrap_or_default();
                let readback = docx_paragraphs_insert(
                    &session.working,
                    insert_after,
                    DocxParagraphMutationOptions {
                        text: text.as_deref(),
                        text_file: text_file.as_deref(),
                        style: &style,
                        out: None,
                        backup: None,
                        dry_run: false,
                        in_place: true,
                        no_validate: true,
                    },
                )?;
                let mut plan_flags = vec![json!("--insert-after"), json!(insert_after.to_string())];
                push_serve_plan_string_flag(&mut plan_flags, "--text", text.as_deref());
                push_serve_plan_string_flag(&mut plan_flags, "--text-file", text_file.as_deref());
                push_serve_plan_string_flag(
                    &mut plan_flags,
                    "--style",
                    (!style.is_empty()).then_some(style.as_str()),
                );
                ServeOp::DocxParagraphsOp {
                    command: command.clone(),
                    plan_flags,
                    readback_file: session.working.clone(),
                    readback,
                }
            }
            "docx paragraphs set" => {
                let handle_set = args.get("handle").is_some();
                let index_set = args.get("index").is_some();
                if handle_set && index_set {
                    return Err(CliError::invalid_args(
                        "cannot specify both --index and --handle",
                    ));
                }
                let index = json_i64(args, "index")?.unwrap_or(0);
                if !handle_set && index < 1 {
                    return Err(CliError::invalid_args(
                        "--index must be >= 1 (or pass --handle)",
                    ));
                }
                let text_set = args.get("text").is_some();
                let text_file_set =
                    args.get("text-file").is_some() || args.get("textFile").is_some();
                let text = json_optional_string(args, "text");
                let text_file = json_optional_string(args, "text-file")
                    .or_else(|| json_optional_string(args, "textFile"));
                let resolved_text = resolve_required_docx_paragraph_set_text(
                    text.as_deref(),
                    text_file.as_deref(),
                    text_set,
                    text_file_set,
                )?;
                let handle = json_optional_string(args, "handle");
                let readback = docx_paragraphs_set(
                    &session.working,
                    index,
                    handle.as_deref(),
                    &resolved_text,
                    DocxParagraphMutationOptions {
                        text: None,
                        text_file: None,
                        style: "",
                        out: None,
                        backup: None,
                        dry_run: false,
                        in_place: true,
                        no_validate: true,
                    },
                )?;
                let mut plan_flags = Vec::new();
                if handle_set {
                    push_serve_plan_string_flag(&mut plan_flags, "--handle", handle.as_deref());
                } else {
                    plan_flags.push(json!("--index"));
                    plan_flags.push(json!(index.to_string()));
                }
                if text_set {
                    push_serve_plan_string_flag(&mut plan_flags, "--text", text.as_deref());
                }
                if text_file_set {
                    push_serve_plan_string_flag(
                        &mut plan_flags,
                        "--text-file",
                        text_file.as_deref(),
                    );
                }
                ServeOp::DocxParagraphsOp {
                    command: command.clone(),
                    plan_flags,
                    readback_file: session.working.clone(),
                    readback,
                }
            }
            "docx paragraphs clear" => {
                let handle_set = args.get("handle").is_some();
                let index_set = args.get("index").is_some();
                if handle_set && index_set {
                    return Err(CliError::invalid_args(
                        "cannot specify both --index and --handle",
                    ));
                }
                let index = json_i64(args, "index")?.unwrap_or(0);
                if !handle_set && index < 1 {
                    return Err(CliError::invalid_args(
                        "--index must be >= 1 (or pass --handle)",
                    ));
                }
                let handle = json_optional_string(args, "handle");
                let readback = docx_paragraphs_clear(
                    &session.working,
                    index,
                    handle.as_deref(),
                    DocxParagraphMutationOptions {
                        text: None,
                        text_file: None,
                        style: "",
                        out: None,
                        backup: None,
                        dry_run: false,
                        in_place: true,
                        no_validate: true,
                    },
                )?;
                let mut plan_flags = Vec::new();
                if handle_set {
                    push_serve_plan_string_flag(&mut plan_flags, "--handle", handle.as_deref());
                } else {
                    plan_flags.push(json!("--index"));
                    plan_flags.push(json!(index.to_string()));
                }
                ServeOp::DocxParagraphsOp {
                    command: command.clone(),
                    plan_flags,
                    readback_file: session.working.clone(),
                    readback,
                }
            }
            "docx styles apply" => {
                let handle_set = args.get("handle").is_some();
                let index_set = args.get("index").is_some();
                if handle_set && index_set {
                    return Err(CliError::invalid_args(
                        "cannot specify both --index and --handle",
                    ));
                }
                let index = json_i64(args, "index")?.unwrap_or(0);
                if !handle_set && index < 1 {
                    return Err(CliError::invalid_args(
                        "--index must be >= 1 (or pass --handle)",
                    ));
                }
                let handle = json_optional_string(args, "handle");
                let target_arg = json_optional_string(args, "target").unwrap_or_default();
                let target = normalize_docx_style_target(&target_arg)?;
                if handle_set && target == DocxStyleTarget::Table {
                    return Err(CliError::invalid_args(
                        "--handle is a paragraph handle; use --index with --target table",
                    ));
                }
                let style = json_optional_string(args, "style").unwrap_or_default();
                if style.trim().is_empty() {
                    return Err(CliError::invalid_args("--style is required"));
                }
                let expect_hash = json_optional_string(args, "expect-hash")
                    .or_else(|| json_optional_string(args, "expectHash"))
                    .unwrap_or_default();
                if !expect_hash.is_empty() {
                    require_docx_block_hash(&expect_hash)?;
                }
                let skip_style_validation = json_bool(args, "no-validate")
                    .or_else(|| json_bool(args, "noValidate"))
                    .unwrap_or(false);
                let readback = docx_styles_apply(
                    &session.working,
                    DocxStyleApplyOptions {
                        index,
                        handle: handle.as_deref(),
                        target,
                        style: &style,
                        expected_hash: &expect_hash,
                        validate_style: !skip_style_validation,
                        mutation: DocxParagraphMutationOptions {
                            text: None,
                            text_file: None,
                            style: "",
                            out: None,
                            backup: None,
                            dry_run: false,
                            in_place: true,
                            no_validate: true,
                        },
                    },
                )?;
                let mut plan_flags = Vec::new();
                if handle_set {
                    push_serve_plan_string_flag(&mut plan_flags, "--handle", handle.as_deref());
                } else {
                    plan_flags.push(json!("--index"));
                    plan_flags.push(json!(index.to_string()));
                }
                push_serve_plan_string_flag(&mut plan_flags, "--target", Some(target.as_str()));
                push_serve_plan_string_flag(&mut plan_flags, "--style", Some(style.as_str()));
                push_serve_plan_string_flag(
                    &mut plan_flags,
                    "--expect-hash",
                    (!expect_hash.is_empty()).then_some(expect_hash.as_str()),
                );
                push_serve_plan_bool_flag(
                    &mut plan_flags,
                    "--no-validate",
                    skip_style_validation.then_some(true),
                );
                ServeOp::DocxStylesOp {
                    command: command.clone(),
                    plan_flags,
                    readback_file: session.working.clone(),
                    readback,
                }
            }
            "docx blocks replace" => {
                let block = json_i64(args, "block")?
                    .ok_or_else(|| CliError::invalid_args("block is required"))?;
                if block < 1 {
                    return Err(CliError::invalid_args("--block must be >= 1"));
                }
                let expect_hash = json_optional_string(args, "expect-hash")
                    .or_else(|| json_optional_string(args, "expectHash"))
                    .unwrap_or_default();
                require_docx_block_hash(&expect_hash)?;
                let text = json_optional_string(args, "text");
                let text_file = json_optional_string(args, "text-file")
                    .or_else(|| json_optional_string(args, "textFile"));
                let style = json_optional_string(args, "style").unwrap_or_default();
                let readback = docx_blocks_replace(
                    &session.working,
                    block as usize,
                    &expect_hash,
                    DocxParagraphMutationOptions {
                        text: text.as_deref(),
                        text_file: text_file.as_deref(),
                        style: &style,
                        out: None,
                        backup: None,
                        dry_run: false,
                        in_place: true,
                        no_validate: true,
                    },
                )?;
                let mut plan_flags = Vec::new();
                plan_flags.push(json!("--block"));
                plan_flags.push(json!(block.to_string()));
                push_serve_plan_string_flag(
                    &mut plan_flags,
                    "--expect-hash",
                    Some(expect_hash.as_str()),
                );
                push_serve_plan_string_flag(&mut plan_flags, "--text", text.as_deref());
                push_serve_plan_string_flag(&mut plan_flags, "--text-file", text_file.as_deref());
                push_serve_plan_string_flag(
                    &mut plan_flags,
                    "--style",
                    (!style.is_empty()).then_some(style.as_str()),
                );
                ServeOp::DocxBlocksOp {
                    command: command.clone(),
                    plan_flags,
                    readback_file: session.working.clone(),
                    readback,
                }
            }
            "docx blocks delete" => {
                let block = json_i64(args, "block")?
                    .ok_or_else(|| CliError::invalid_args("block is required"))?;
                if block < 1 {
                    return Err(CliError::invalid_args("--block must be >= 1"));
                }
                let expect_hash = json_optional_string(args, "expect-hash")
                    .or_else(|| json_optional_string(args, "expectHash"))
                    .unwrap_or_default();
                require_docx_block_hash(&expect_hash)?;
                let readback = docx_blocks_delete(
                    &session.working,
                    block as usize,
                    &expect_hash,
                    DocxParagraphMutationOptions {
                        text: None,
                        text_file: None,
                        style: "",
                        out: None,
                        backup: None,
                        dry_run: false,
                        in_place: true,
                        no_validate: true,
                    },
                )?;
                let mut plan_flags = Vec::new();
                plan_flags.push(json!("--block"));
                plan_flags.push(json!(block.to_string()));
                push_serve_plan_string_flag(
                    &mut plan_flags,
                    "--expect-hash",
                    Some(expect_hash.as_str()),
                );
                ServeOp::DocxBlocksOp {
                    command: command.clone(),
                    plan_flags,
                    readback_file: session.working.clone(),
                    readback,
                }
            }
            "docx blocks insert-after" => {
                let block = json_i64(args, "block")?.unwrap_or(0);
                if block < 0 {
                    return Err(CliError::invalid_args("--block must be >= 0"));
                }
                let expect_hash_set =
                    args.get("expect-hash").is_some() || args.get("expectHash").is_some();
                let expect_hash = json_optional_string(args, "expect-hash")
                    .or_else(|| json_optional_string(args, "expectHash"))
                    .unwrap_or_default();
                if block > 0 {
                    require_docx_block_hash(&expect_hash)?;
                } else if expect_hash_set {
                    return Err(CliError::invalid_args(
                        "--expect-hash cannot be used with --block 0",
                    ));
                }
                let text = json_optional_string(args, "text");
                let text_file = json_optional_string(args, "text-file")
                    .or_else(|| json_optional_string(args, "textFile"));
                let style = json_optional_string(args, "style").unwrap_or_default();
                let readback = docx_blocks_insert_after(
                    &session.working,
                    block as usize,
                    &expect_hash,
                    DocxParagraphMutationOptions {
                        text: text.as_deref(),
                        text_file: text_file.as_deref(),
                        style: &style,
                        out: None,
                        backup: None,
                        dry_run: false,
                        in_place: true,
                        no_validate: true,
                    },
                )?;
                let mut plan_flags = Vec::new();
                plan_flags.push(json!("--block"));
                plan_flags.push(json!(block.to_string()));
                push_serve_plan_string_flag(
                    &mut plan_flags,
                    "--expect-hash",
                    (!expect_hash.is_empty()).then_some(expect_hash.as_str()),
                );
                push_serve_plan_string_flag(&mut plan_flags, "--text", text.as_deref());
                push_serve_plan_string_flag(&mut plan_flags, "--text-file", text_file.as_deref());
                push_serve_plan_string_flag(
                    &mut plan_flags,
                    "--style",
                    (!style.is_empty()).then_some(style.as_str()),
                );
                ServeOp::DocxBlocksOp {
                    command: command.clone(),
                    plan_flags,
                    readback_file: session.working.clone(),
                    readback,
                }
            }
            "docx comments add" => {
                let anchor_block = match json_i64(args, "anchor-block")? {
                    Some(value) => value,
                    None => json_i64(args, "anchorBlock")?.unwrap_or(0),
                };
                if (args.get("anchor-block").is_some() || args.get("anchorBlock").is_some())
                    && anchor_block < 1
                {
                    return Err(CliError::invalid_args("--anchor-block must be >= 1"));
                }
                let author = json_optional_string(args, "author").unwrap_or_default();
                if author.is_empty() {
                    return Err(CliError::invalid_args("--author is required"));
                }
                let initials = json_optional_string(args, "initials").unwrap_or_default();
                let date = json_optional_string(args, "date").unwrap_or_else(current_utc_rfc3339);
                let text = json_optional_string(args, "text");
                let text_file = json_optional_string(args, "text-file")
                    .or_else(|| json_optional_string(args, "textFile"));
                let readback = docx_comments_add(
                    &session.working,
                    anchor_block,
                    &author,
                    &initials,
                    &date,
                    DocxParagraphMutationOptions {
                        text: text.as_deref(),
                        text_file: text_file.as_deref(),
                        style: "",
                        out: None,
                        backup: None,
                        dry_run: false,
                        in_place: true,
                        no_validate: true,
                    },
                )?;
                let mut plan_flags = Vec::new();
                if anchor_block > 0 {
                    plan_flags.push(json!("--anchor-block"));
                    plan_flags.push(json!(anchor_block.to_string()));
                }
                push_serve_plan_string_flag(&mut plan_flags, "--author", Some(&author));
                push_serve_plan_string_flag(
                    &mut plan_flags,
                    "--initials",
                    (!initials.is_empty()).then_some(initials.as_str()),
                );
                push_serve_plan_string_flag(&mut plan_flags, "--date", Some(&date));
                push_serve_plan_string_flag(&mut plan_flags, "--text", text.as_deref());
                push_serve_plan_string_flag(&mut plan_flags, "--text-file", text_file.as_deref());
                ServeOp::DocxCommentsOp {
                    command: command.clone(),
                    plan_flags,
                    readback_file: session.working.clone(),
                    readback,
                }
            }
            "docx comments edit" => {
                let comment_id_set =
                    args.get("comment-id").is_some() || args.get("commentId").is_some();
                let handle_set = args.get("handle").is_some();
                if handle_set && comment_id_set {
                    return Err(CliError::invalid_args(
                        "cannot specify both --comment-id and --handle",
                    ));
                }
                if !handle_set && !comment_id_set {
                    return Err(CliError::invalid_args(
                        "--comment-id is required (or pass --handle)",
                    ));
                }
                let comment_id = match json_i64(args, "comment-id")? {
                    Some(value) => value,
                    None => json_i64(args, "commentId")?.unwrap_or(0),
                };
                if !handle_set && comment_id < 0 {
                    return Err(CliError::invalid_args("--comment-id must be >= 0"));
                }
                let text_set = args.get("text").is_some();
                let text_file_set =
                    args.get("text-file").is_some() || args.get("textFile").is_some();
                let author_set = args.get("author").is_some();
                let date_set = args.get("date").is_some();
                if text_set && text_file_set {
                    return Err(CliError::invalid_args(
                        "cannot specify both --text and --text-file",
                    ));
                }
                if !text_set && !text_file_set && !author_set && !date_set {
                    return Err(CliError::invalid_args(
                        "specify at least one of --text, --text-file, --author, or --date",
                    ));
                }
                let text = json_optional_string(args, "text");
                let text_file = json_optional_string(args, "text-file")
                    .or_else(|| json_optional_string(args, "textFile"));
                let resolved_text = if text_file_set {
                    let path = text_file.as_deref().unwrap_or_default();
                    fs::read(path)
                        .map(|data| String::from_utf8_lossy(&data).to_string())
                        .map_err(|_| CliError::file_not_found(format!("file not found: {path}")))?
                } else {
                    text.clone().unwrap_or_default()
                };
                let handle = json_optional_string(args, "handle");
                let author = json_optional_string(args, "author").unwrap_or_default();
                let date = json_optional_string(args, "date").unwrap_or_default();
                let expect_hash = json_optional_string(args, "expect-hash")
                    .or_else(|| json_optional_string(args, "expectHash"))
                    .unwrap_or_default();
                let readback = docx_comments_edit(
                    &session.working,
                    comment_id,
                    handle.as_deref(),
                    DocxCommentEditSpec {
                        expect_hash: &expect_hash,
                        text: &resolved_text,
                        text_set: text_set || text_file_set,
                        author: &author,
                        author_set,
                        date: &date,
                        date_set,
                    },
                    DocxParagraphMutationOptions {
                        text: None,
                        text_file: None,
                        style: "",
                        out: None,
                        backup: None,
                        dry_run: false,
                        in_place: true,
                        no_validate: true,
                    },
                )?;
                let mut plan_flags = Vec::new();
                if handle_set {
                    push_serve_plan_string_flag(&mut plan_flags, "--handle", handle.as_deref());
                } else {
                    plan_flags.push(json!("--comment-id"));
                    plan_flags.push(json!(comment_id.to_string()));
                }
                if text_set {
                    push_serve_plan_string_flag(&mut plan_flags, "--text", text.as_deref());
                }
                if text_file_set {
                    push_serve_plan_string_flag(
                        &mut plan_flags,
                        "--text-file",
                        text_file.as_deref(),
                    );
                }
                push_serve_plan_string_flag(
                    &mut plan_flags,
                    "--author",
                    author_set.then_some(author.as_str()),
                );
                push_serve_plan_string_flag(
                    &mut plan_flags,
                    "--date",
                    date_set.then_some(date.as_str()),
                );
                push_serve_plan_string_flag(
                    &mut plan_flags,
                    "--expect-hash",
                    (!expect_hash.is_empty()).then_some(expect_hash.as_str()),
                );
                ServeOp::DocxCommentsOp {
                    command: command.clone(),
                    plan_flags,
                    readback_file: session.working.clone(),
                    readback,
                }
            }
            "docx comments remove" => {
                let comment_id_set =
                    args.get("comment-id").is_some() || args.get("commentId").is_some();
                let handle_set = args.get("handle").is_some();
                if handle_set && comment_id_set {
                    return Err(CliError::invalid_args(
                        "cannot specify both --comment-id and --handle",
                    ));
                }
                if !handle_set && !comment_id_set {
                    return Err(CliError::invalid_args(
                        "--comment-id is required (or pass --handle)",
                    ));
                }
                let comment_id = match json_i64(args, "comment-id")? {
                    Some(value) => value,
                    None => json_i64(args, "commentId")?.unwrap_or(0),
                };
                if !handle_set && comment_id < 0 {
                    return Err(CliError::invalid_args("--comment-id must be >= 0"));
                }
                let handle = json_optional_string(args, "handle");
                let expect_hash = json_optional_string(args, "expect-hash")
                    .or_else(|| json_optional_string(args, "expectHash"))
                    .unwrap_or_default();
                let readback = docx_comments_remove(
                    &session.working,
                    comment_id,
                    handle.as_deref(),
                    &expect_hash,
                    DocxParagraphMutationOptions {
                        text: None,
                        text_file: None,
                        style: "",
                        out: None,
                        backup: None,
                        dry_run: false,
                        in_place: true,
                        no_validate: true,
                    },
                )?;
                let mut plan_flags = Vec::new();
                if handle_set {
                    push_serve_plan_string_flag(&mut plan_flags, "--handle", handle.as_deref());
                } else {
                    plan_flags.push(json!("--comment-id"));
                    plan_flags.push(json!(comment_id.to_string()));
                }
                push_serve_plan_string_flag(
                    &mut plan_flags,
                    "--expect-hash",
                    (!expect_hash.is_empty()).then_some(expect_hash.as_str()),
                );
                ServeOp::DocxCommentsOp {
                    command: command.clone(),
                    plan_flags,
                    readback_file: session.working.clone(),
                    readback,
                }
            }
            "docx tables set-cell" => {
                let table = json_i64(args, "table")?
                    .ok_or_else(|| CliError::invalid_args("table is required"))?;
                let row = json_i64(args, "row")?
                    .ok_or_else(|| CliError::invalid_args("row is required"))?;
                let col = json_i64(args, "col")?
                    .ok_or_else(|| CliError::invalid_args("col is required"))?;
                validate_positive_i64(table, "--table")?;
                validate_positive_i64(row, "--row")?;
                validate_positive_i64(col, "--col")?;
                let expect_hash = json_optional_string(args, "expect-hash")
                    .or_else(|| json_optional_string(args, "expectHash"))
                    .unwrap_or_default();
                require_docx_block_hash(&expect_hash)?;
                let text_changed = args.get("text").is_some();
                let text_file_changed =
                    args.get("text-file").is_some() || args.get("textFile").is_some();
                let text = json_optional_string(args, "text");
                let text_file = json_optional_string(args, "text-file")
                    .or_else(|| json_optional_string(args, "textFile"));
                let resolved_text = resolve_required_docx_table_text(
                    text.as_deref(),
                    text_file.as_deref(),
                    text_changed,
                    text_file_changed,
                )?;
                let readback = docx_tables_set_cell(
                    &session.working,
                    table as usize,
                    row as usize,
                    col as usize,
                    &expect_hash,
                    &resolved_text,
                    DocxParagraphMutationOptions {
                        text: None,
                        text_file: None,
                        style: "",
                        out: None,
                        backup: None,
                        dry_run: false,
                        in_place: true,
                        no_validate: true,
                    },
                )?;
                let mut plan_flags = vec![
                    json!("--table"),
                    json!(table.to_string()),
                    json!("--row"),
                    json!(row.to_string()),
                    json!("--col"),
                    json!(col.to_string()),
                ];
                push_serve_plan_string_flag(
                    &mut plan_flags,
                    "--expect-hash",
                    Some(expect_hash.as_str()),
                );
                if text_changed {
                    push_serve_plan_string_flag(
                        &mut plan_flags,
                        "--text",
                        Some(resolved_text.as_str()),
                    );
                }
                if text_file_changed {
                    push_serve_plan_string_flag(
                        &mut plan_flags,
                        "--text-file",
                        text_file.as_deref(),
                    );
                }
                ServeOp::DocxTablesOp {
                    command: command.clone(),
                    plan_flags,
                    readback_file: session.working.clone(),
                    readback,
                }
            }
            "docx tables clear-cell" => {
                let table = json_i64(args, "table")?
                    .ok_or_else(|| CliError::invalid_args("table is required"))?;
                let row = json_i64(args, "row")?
                    .ok_or_else(|| CliError::invalid_args("row is required"))?;
                let col = json_i64(args, "col")?
                    .ok_or_else(|| CliError::invalid_args("col is required"))?;
                validate_positive_i64(table, "--table")?;
                validate_positive_i64(row, "--row")?;
                validate_positive_i64(col, "--col")?;
                let expect_hash = json_optional_string(args, "expect-hash")
                    .or_else(|| json_optional_string(args, "expectHash"))
                    .unwrap_or_default();
                require_docx_block_hash(&expect_hash)?;
                let readback = docx_tables_clear_cell(
                    &session.working,
                    table as usize,
                    row as usize,
                    col as usize,
                    &expect_hash,
                    DocxParagraphMutationOptions {
                        text: None,
                        text_file: None,
                        style: "",
                        out: None,
                        backup: None,
                        dry_run: false,
                        in_place: true,
                        no_validate: true,
                    },
                )?;
                let mut plan_flags = vec![
                    json!("--table"),
                    json!(table.to_string()),
                    json!("--row"),
                    json!(row.to_string()),
                    json!("--col"),
                    json!(col.to_string()),
                ];
                push_serve_plan_string_flag(
                    &mut plan_flags,
                    "--expect-hash",
                    Some(expect_hash.as_str()),
                );
                ServeOp::DocxTablesOp {
                    command: command.clone(),
                    plan_flags,
                    readback_file: session.working.clone(),
                    readback,
                }
            }
            "pptx replace text" => {
                let slide = json_u32(args, "slide")?.unwrap_or(1);
                let target = json_string(args, "target")?;
                let text = json_string(args, "text")?;
                pptx_replace_text_in_place(&session.working, slide, &target, &text)?;
                ServeOp::PptxReplaceText {
                    command: command.clone(),
                    slide,
                    target,
                    text,
                }
            }
            _ => {
                return Err(CliError::invalid_args(format!(
                    "unsupported serve op command: {command}"
                )));
            }
        };
        let readback = op.readback(&session.working);
        let index = session.ops.len();
        session.ops.push(op);
        Ok(json!({"command": command, "index": index, "readback": readback}))
    }

    fn serve_inspect(&mut self, params: &Value) -> CliResult<Value> {
        let session_id = json_string(params, "session")?;
        let command = json_string(params, "command")?;
        let args = params
            .get("args")
            .ok_or_else(|| CliError::invalid_args("inspect args are required"))?;
        let session = self.session(&session_id)?;
        match command.as_str() {
            "xlsx ranges export" => {
                let sheet = json_string(args, "sheet")?;
                let range = json_string(args, "range")?;
                let data_format = json_optional_string(args, "data-format")
                    .or_else(|| json_optional_string(args, "dataFormat"));
                require_json_data_format(data_format.as_deref())?;
                let data_out = json_optional_string(args, "data-out")
                    .or_else(|| json_optional_string(args, "dataOut"));
                let max_cells = json_i64(args, "max-cells")?
                    .or(json_i64(args, "maxCells")?)
                    .unwrap_or(100000);
                let include_types = json_bool(args, "include-types")
                    .or_else(|| json_bool(args, "includeTypes"))
                    .unwrap_or(false);
                let include_formulas = json_bool(args, "include-formulas")
                    .or_else(|| json_bool(args, "includeFormulas"))
                    .unwrap_or(false);
                let include_formats = json_bool(args, "include-formats")
                    .or_else(|| json_bool(args, "includeFormats"))
                    .unwrap_or(false);
                xlsx_range_export_with_options(
                    &session.working,
                    &sheet,
                    &range,
                    XlsxRangeExportOptions {
                        include_types,
                        include_formulas,
                        include_formats,
                        data_out: data_out.as_deref(),
                        max_cells,
                    },
                )
            }
            "xlsx cells extract" => {
                let sheet = json_optional_string(args, "sheet").unwrap_or_else(|| "1".to_string());
                let range = json_optional_string(args, "range");
                let max_rows = json_u32(args, "max-rows")?
                    .or(json_u32(args, "maxRows")?)
                    .unwrap_or(1000);
                let max_cells = json_u32(args, "max-cells")?
                    .or(json_u32(args, "maxCells")?)
                    .unwrap_or(0);
                let include_empty = json_bool(args, "include-empty")
                    .or_else(|| json_bool(args, "includeEmpty"))
                    .unwrap_or(false);
                xlsx_cells_extract(
                    &session.working,
                    &sheet,
                    range.as_deref(),
                    max_rows,
                    max_cells,
                    include_empty,
                )
            }
            "xlsx sheets list" => xlsx_sheets_list(&session.working),
            "xlsx sheets show" => {
                let sheet = json_optional_string(args, "sheet");
                xlsx_sheets_show(&session.working, sheet.as_deref())
            }
            "xlsx names list" => {
                let scope_sheet = json_optional_string(args, "scope-sheet")
                    .or_else(|| json_optional_string(args, "scopeSheet"));
                xlsx_names_list(&session.working, scope_sheet.as_deref())
            }
            "xlsx names show" => {
                let name = json_string(args, "name")?;
                let scope_sheet = json_optional_string(args, "scope-sheet")
                    .or_else(|| json_optional_string(args, "scopeSheet"));
                xlsx_names_show(&session.working, &name, scope_sheet.as_deref())
            }
            "xlsx tables list" => {
                let sheet = json_optional_string(args, "sheet");
                xlsx_tables_list(&session.working, sheet.as_deref())
            }
            "xlsx tables show" => {
                let sheet = json_optional_string(args, "sheet");
                let table = json_optional_string(args, "table");
                xlsx_tables_show(&session.working, sheet.as_deref(), table.as_deref())
            }
            "xlsx tables export" => {
                let sheet = json_optional_string(args, "sheet");
                let table = json_optional_string(args, "table");
                let data_format = json_optional_string(args, "data-format")
                    .or_else(|| json_optional_string(args, "dataFormat"));
                let data_out = json_optional_string(args, "data-out")
                    .or_else(|| json_optional_string(args, "dataOut"));
                let max_cells = json_i64(args, "max-cells")?
                    .or(json_i64(args, "maxCells")?)
                    .unwrap_or(100000);
                let include_types = json_bool(args, "include-types")
                    .or_else(|| json_bool(args, "includeTypes"))
                    .unwrap_or(false);
                let include_formulas = json_bool(args, "include-formulas")
                    .or_else(|| json_bool(args, "includeFormulas"))
                    .unwrap_or(false);
                xlsx_tables_export(
                    &session.working,
                    sheet.as_deref(),
                    table.as_deref(),
                    XlsxTableExportOptions {
                        data_format: data_format.as_deref(),
                        data_out: data_out.as_deref(),
                        max_cells,
                        include_types,
                        include_formulas,
                    },
                )
            }
            "xlsx workbook metadata inspect" => xlsx_workbook_metadata_inspect(&session.working),
            "docx text" => docx_text(&session.working),
            "docx fields list" => {
                let field_type = json_optional_string(args, "type");
                docx_fields_list(&session.working, field_type.as_deref())
            }
            "docx headers list" | "docx footers list" => {
                docx_headers_footers_list(&session.working)
            }
            "docx headers show" | "docx footers show" => {
                let group = if command.starts_with("docx footers") {
                    "footers"
                } else {
                    "headers"
                };
                let rest = docx_header_footer_show_json_args(args)?;
                docx_headers_footers_show(&session.working, docx_header_footer_kind(group), &rest)
            }
            "docx images list" => docx_images_list(&session.working),
            "docx comments list" => {
                let comment_id = match json_i64(args, "comment-id")? {
                    Some(value) => Some(value),
                    None => json_i64(args, "commentId")?,
                };
                if let Some(comment_id) = comment_id
                    && comment_id < 0
                {
                    return Err(CliError::invalid_args("--comment-id must be >= 0"));
                }
                docx_comments_list(&session.working, comment_id)
            }
            "docx blocks" => {
                let block = json_i64(args, "block")?.unwrap_or(0);
                if block < 0 {
                    return Err(CliError::invalid_args("--block must be >= 0"));
                }
                let include_runs = json_bool(args, "include-runs")
                    .or_else(|| json_bool(args, "includeRuns"))
                    .unwrap_or(false);
                docx_blocks_show(&session.working, block as usize, include_runs)
            }
            "docx styles list" => {
                let style_type = json_optional_string(args, "type");
                docx_styles_list(&session.working, style_type.as_deref())
            }
            "docx styles show" => {
                let style_id = json_string(args, "style")?;
                docx_styles_show(&session.working, &style_id)
            }
            "docx tables show" => {
                let table = json_i64(args, "table")?.unwrap_or(0);
                if table < 0 {
                    return Err(CliError::invalid_args("--table must be >= 0"));
                }
                let details = json_bool(args, "details")
                    .or_else(|| json_bool(args, "includeDetails"))
                    .unwrap_or(false);
                docx_tables_show(&session.working, table as usize, details)
            }
            "pptx slides list" => pptx_slides_list(&session.working),
            "pptx slides selectors" => {
                let slide = json_u32(args, "slide")?
                    .ok_or_else(|| CliError::invalid_args("slide is required"))?;
                pptx_slide_selectors(&session.working, slide)
            }
            "pptx slides show" => {
                let slide = json_u32(args, "slide")?.unwrap_or(1);
                pptx_slide_show(&session.working, slide)
            }
            "pptx extract text" => {
                let rest = pptx_extract_text_json_args(args)?;
                pptx_extract_text(&session.working, &rest)
            }
            "pptx extract notes" => {
                let rest = pptx_extract_text_json_args(args)?;
                pptx_extract_notes(&session.working, &rest)
            }
            "pptx notes show" => {
                let slide = json_u32(args, "slide")?
                    .ok_or_else(|| CliError::invalid_args("slide is required"))?;
                pptx_notes_show(&session.working, slide)
            }
            "pptx comments list" => {
                let slide = json_u32(args, "slide")?;
                let comment_id = match json_i64(args, "comment-id")? {
                    Some(value) => Some(value),
                    None => json_i64(args, "commentId")?,
                };
                if comment_id.is_some() && slide.is_none() {
                    return Err(CliError::invalid_args("--comment-id requires --slide"));
                }
                pptx_comments_list(&session.working, slide, comment_id)
            }
            "pptx masters list" => pptx_masters_list(&session.working),
            "pptx masters show" => {
                let master = json_u32(args, "master")?.unwrap_or(1) as i64;
                pptx_masters_show(&session.working, master)
            }
            "pptx layouts list" => {
                let master = json_u32(args, "master")?;
                pptx_layouts_list(&session.working, master)
            }
            "pptx layouts show" => {
                let layout = json_string(args, "layout")?;
                pptx_layouts_show(&session.working, &layout)
            }
            "pptx tables show" => {
                let slide = json_u32(args, "slide")?
                    .ok_or_else(|| CliError::invalid_args("slide is required"))?;
                let table_id = json_u32(args, "table-id")?
                    .or(json_u32(args, "tableId")?)
                    .unwrap_or(0);
                let target = json_optional_string(args, "target");
                let details = json_bool(args, "details").unwrap_or(false);
                if table_id > 0 && target.as_deref().unwrap_or_default() != "" {
                    return Err(CliError::invalid_args(
                        "specify only one of --target or --table-id",
                    ));
                }
                pptx_tables_show(
                    &session.working,
                    slide,
                    table_id,
                    target.as_deref(),
                    details,
                )
            }
            "pptx shapes show" => {
                let slide = json_u32(args, "slide")?
                    .ok_or_else(|| CliError::invalid_args("slide is required"))?;
                let include_text = json_bool(args, "include-text")
                    .or_else(|| json_bool(args, "includeText"))
                    .unwrap_or(false);
                let include_bounds = json_bool(args, "include-bounds")
                    .or_else(|| json_bool(args, "includeBounds"))
                    .unwrap_or(false);
                pptx_shapes_show(&session.working, slide, include_text, include_bounds)
            }
            _ => Err(CliError::invalid_args(format!(
                "unsupported serve inspect command: {command}"
            ))),
        }
    }

    fn serve_validate(&self, params: &Value) -> CliResult<Value> {
        let session_id = json_string(params, "session")?;
        let session = self.session(&session_id)?;
        let report = validate(&session.working, true)?;
        Ok(json!({
            "diagnostics": report
                .get("diagnostics")
                .cloned()
                .unwrap_or_else(|| Value::Array(Vec::new())),
        }))
    }

    fn serve_plan(&self, params: &Value) -> CliResult<Value> {
        let session_id = json_string(params, "session")?;
        let session = self.session(&session_id)?;
        let plan: Vec<Value> = session
            .ops
            .iter()
            .enumerate()
            .map(|(index, op)| {
                json!({
                    "argv": op.plan_argv(&session.file),
                    "command": op.command(),
                    "index": index,
                })
            })
            .collect();
        Ok(json!({
            "dryRun": session.dry_run,
            "file": session.file,
            "opsCount": session.ops.len(),
            "plan": plan,
            "schemaVersion": 1,
        }))
    }

    fn serve_commit(&mut self, params: &Value) -> CliResult<Value> {
        let session_id = json_string(params, "session")?;
        let session = self.session(&session_id)?;
        let output = if session.in_place {
            session.file.clone()
        } else {
            session
                .out
                .clone()
                .ok_or_else(|| CliError::invalid_args("commit requires an output path"))?
        };
        if !session.dry_run {
            if !session.no_validate {
                let validation = validate(&session.working, true)?;
                if validate_exit_code(&validation, true) != EXIT_SUCCESS {
                    return Err(CliError::validation_failed(format!(
                        "validation failed for working copy: {}",
                        serde_json::to_string(&validation).expect("serialize validation")
                    )));
                }
            }
            if session.in_place
                && let Some(backup_path) = session
                    .backup
                    .as_ref()
                    .filter(|value| !value.trim().is_empty())
            {
                if let Some(parent) = Path::new(backup_path).parent() {
                    fs::create_dir_all(parent)
                        .map_err(|err| CliError::unexpected(err.to_string()))?;
                }
                fs::copy(&session.file, backup_path).map_err(|err| {
                    CliError::unexpected(format!("failed to create backup: {err}"))
                })?;
            }
            if let Some(parent) = Path::new(&output).parent() {
                fs::create_dir_all(parent).map_err(|err| CliError::unexpected(err.to_string()))?;
            }
            fs::copy(&session.working, &output)
                .map_err(|err| CliError::unexpected(err.to_string()))?;
        }
        let readback_file = if session.dry_run {
            &session.working
        } else {
            &output
        };
        let applied: Vec<Value> = session
            .ops
            .iter()
            .enumerate()
            .map(|(index, op)| {
                json!({
                    "command": op.command(),
                    "index": index,
                    "readback": op.readback(readback_file),
                })
            })
            .collect();
        let mut result = json!({
            "applied": applied,
            "dryRun": session.dry_run,
            "file": session.file,
            "opsCount": session.ops.len(),
            "output": if session.dry_run { Value::Null } else { json!(output.clone()) },
            "schemaVersion": 1,
            "validateCommand": if session.dry_run {
                Value::Null
            } else {
                json!(format!("ooxml validate --strict {output}"))
            },
        });
        if session.dry_run
            && let Value::Object(ref mut object) = result
        {
            object.insert("committed".to_string(), json!(false));
            object.insert("plannedOutput".to_string(), json!(output));
        }
        Ok(result)
    }

    fn serve_abort(&mut self, params: &Value) -> CliResult<Value> {
        let session_id = json_string(params, "session")?;
        self.sessions
            .remove(&session_id)
            .ok_or_else(|| CliError::invalid_args(format!("session not found: {session_id}")))?;
        Ok(json!({"aborted": true}))
    }

    fn session(&self, session_id: &str) -> CliResult<&ServeSession> {
        self.sessions
            .get(session_id)
            .ok_or_else(|| CliError::invalid_args(format!("session not found: {session_id}")))
    }

    fn session_mut(&mut self, session_id: &str) -> CliResult<&mut ServeSession> {
        self.sessions
            .get_mut(session_id)
            .ok_or_else(|| CliError::invalid_args(format!("session not found: {session_id}")))
    }
}

fn make_working_copy(file: &str, session_number: usize) -> CliResult<String> {
    let dir = std::env::temp_dir().join(format!(
        "ooxml-rust-serve-{}-{session_number}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).map_err(|err| CliError::unexpected(err.to_string()))?;
    let extension = Path::new(file)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("xlsx");
    let working = dir.join(format!("working.{extension}"));
    fs::copy(file, &working).map_err(|err| CliError::unexpected(err.to_string()))?;
    Ok(working.to_string_lossy().to_string())
}
