use serde_json::{Value, json};

use crate::pptx_replace_text_readback;

#[derive(Clone)]
pub(super) enum ServeOp {
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

pub(super) fn push_serve_plan_string_flag(flags: &mut Vec<Value>, name: &str, value: Option<&str>) {
    if let Some(value) = value {
        flags.push(json!(name));
        flags.push(json!(value));
    }
}

pub(super) fn push_serve_plan_bool_flag(flags: &mut Vec<Value>, name: &str, value: Option<bool>) {
    match value {
        Some(true) => flags.push(json!(name)),
        Some(false) => flags.push(json!(format!("{name}=false"))),
        None => {}
    }
}

impl ServeOp {
    pub(super) fn command(&self) -> &str {
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

    pub(super) fn plan_argv(&self, source_file: &str) -> Value {
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

    pub(super) fn readback(&self, file: &str) -> Value {
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
