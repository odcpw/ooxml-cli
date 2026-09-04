use serde::Serialize;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::{
    BuildCompileError, BuildCompiler, BuildFamily, BuildLength, BuildSpec, CompiledBuildPlan,
    operation_reference,
};

static NEXT_STAGE: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompiledXlsxBuild {
    pub plan: CompiledBuildPlan,
}

pub fn compile_xlsx_spec(spec: &BuildSpec) -> Result<CompiledXlsxBuild, BuildCompileError> {
    if spec.family() != BuildFamily::Xlsx {
        return Err(error(
            "/family",
            None,
            "BUILD_SPEC_FAMILY_MISMATCH",
            "xlsx build requires an xlsx build spec",
        ));
    }
    let document = spec
        .document()
        .as_object()
        .expect("validated xlsx build spec root");
    let mut compiler = BuildCompiler::new(BuildFamily::Xlsx);
    compiler.push_operation(
        "/",
        None,
        "document",
        "xlsx scaffold",
        scaffold_args(document)?,
        "destination",
    )?;

    let mut formula_cells = false;
    for (sheet_index, sheet) in document["sheets"]
        .as_array()
        .expect("validated xlsx sheets")
        .iter()
        .enumerate()
    {
        formula_cells |= compile_sheet(
            sheet_index,
            sheet.as_object().expect("validated xlsx sheet"),
            &mut compiler,
        )?;
    }
    compile_metadata(document.get("metadata"), formula_cells, &mut compiler)?;

    Ok(CompiledXlsxBuild {
        plan: compiler.finish()?,
    })
}

pub(crate) fn xlsx_build(args: &[String]) -> crate::CliResult<Value> {
    crate::reject_unknown_flags(
        args,
        &["--spec", "--out"],
        &["--check", "--dry-run", "--force"],
    )?;
    let spec_path = crate::parse_string_flag(args, "--spec")?
        .ok_or_else(|| crate::CliError::invalid_args("--spec is required"))?;
    let output = crate::parse_string_flag(args, "--out")?
        .ok_or_else(|| crate::CliError::invalid_args("--out is required"))?;
    let dry_run = crate::has_flag(args, "--dry-run");
    let run_check = crate::has_flag(args, "--check");
    if run_check && dry_run {
        return Err(crate::CliError::invalid_args(
            "--check requires a published build; omit --dry-run",
        ));
    }
    validate_output_path(&output, crate::has_flag(args, "--force"))?;

    let (spec, spec_base) = load_xlsx_build_spec(&spec_path)?;
    let compiled = compile_xlsx_spec(&spec).map_err(build_compile_cli_error)?;
    let temp = XlsxBuildTemp::create()?;
    let operations = materialize_operations(
        &compiled.plan.operations,
        spec.document(),
        &spec_base,
        &temp.path,
    )?;
    let ops_path = temp.path.join("operations.json");
    let mut ops_bytes = serde_json::to_vec_pretty(&operations).map_err(|cause| {
        crate::CliError::unexpected(format!("failed to encode build plan: {cause}"))
    })?;
    ops_bytes.push(b'\n');
    fs::write(&ops_path, ops_bytes).map_err(|cause| {
        crate::CliError::unexpected(format!("failed to write build plan: {cause}"))
    })?;

    let virtual_input = if dry_run {
        PathBuf::from(&output)
    } else {
        temp.path.join("new-workbook.xlsx")
    };
    let mut apply_args = vec!["--ops".to_string(), ops_path.to_string_lossy().into_owned()];
    if dry_run {
        apply_args.push("--dry-run".to_string());
    } else {
        apply_args.push("--out".to_string());
        apply_args.push(output.clone());
    }
    let mutation_envelope = crate::apply(&virtual_input.to_string_lossy(), &apply_args)
        .map_err(|error| super::compiler::execution_error_with_spec_path(&compiled.plan, error))?;
    let mutation_envelope = scrub_build_paths(mutation_envelope, &temp.path, &spec_base);

    let outline = if dry_run {
        Value::Null
    } else {
        crate::outline(
            &output,
            crate::OutlineOptions {
                depth: 2,
                text_preview: 240,
                slide: None,
                sheet: None,
                section: None,
            },
        )?
    };
    let check = if run_check {
        crate::check::inspect(&output, &json!({}))?
    } else {
        Value::Null
    };
    let node_map = resolved_node_map(&compiled.plan, &mutation_envelope);
    Ok(json!({
        "schemaVersion": "ooxml-cli.xlsx-build.v1",
        "spec": spec_path,
        "output": if dry_run { Value::Null } else { json!(output) },
        "dryRun": dry_run,
        "validated": mutation_envelope["validated"],
        "mutationEnvelope": mutation_envelope,
        "compiledPlan": compiled.plan,
        "nodeMap": node_map,
        "outline": outline,
        "check": check,
    }))
}

fn compile_sheet(
    sheet_index: usize,
    sheet: &Map<String, Value>,
    compiler: &mut BuildCompiler,
) -> Result<bool, BuildCompileError> {
    let path = format!("/sheets/{sheet_index}");
    let name = required_string(sheet, "name", &format!("{path}/name"))?;
    let columns = sheet
        .get("columns")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    let column_count = sheet_column_count(sheet, columns, &path)?;
    let (data_op, formula_cells) =
        compile_sheet_data(sheet_index, sheet, columns, name, &path, compiler)?;
    let inline_row_count = sheet.get("rows").and_then(Value::as_array).map(Vec::len);
    if data_op.is_none() {
        compiler.map_node(
            &path,
            sheet.get("id").and_then(Value::as_str),
            "document",
            "destination",
        )?;
    }

    if let Some(color) = sheet.get("tabColor") {
        push_simple(
            compiler,
            format!("{path}/tabColor"),
            format!("sheet-{}-tab-color", sheet_index + 1),
            "xlsx sheets set-tab-color",
            map([("sheet", json!(name)), ("color", color.clone())]),
        )?;
    }
    if let Some(freeze) = sheet.get("freeze").and_then(Value::as_str) {
        let mut args = map([("sheet", json!(name))]);
        let (rows, cols) = parse_freeze(freeze, &format!("{path}/freeze"))?;
        if rows > 0 {
            args.insert("rows".to_string(), json!(rows));
        }
        if cols > 0 {
            args.insert("cols".to_string(), json!(cols));
        }
        push_simple(
            compiler,
            format!("{path}/freeze"),
            format!("sheet-{}-freeze", sheet_index + 1),
            "xlsx freeze set",
            args,
        )?;
    }
    if let Some(preset) = sheet.get("headerStyle") {
        let range = header_range(column_count, &format!("{path}/headerStyle"))?;
        push_simple(
            compiler,
            format!("{path}/headerStyle"),
            format!("sheet-{}-header-style", sheet_index + 1),
            "xlsx ranges set-style",
            map([
                ("sheet", json!(name)),
                ("range", json!(range)),
                ("preset", preset.clone()),
            ]),
        )?;
    }

    compile_columns(
        sheet_index,
        columns,
        name,
        data_op.as_deref(),
        inline_row_count,
        &path,
        compiler,
    )?;
    compile_tables(
        sheet_index,
        sheet,
        name,
        data_op.as_deref(),
        &path,
        compiler,
    )?;
    compile_object_array(
        sheet_index,
        sheet,
        "conditionalFormats",
        "conditional-format",
        "xlsx conditional-formats add",
        name,
        &path,
        compiler,
    )?;
    compile_object_array(
        sheet_index,
        sheet,
        "dataValidations",
        "data-validation",
        "xlsx data-validations create",
        name,
        &path,
        compiler,
    )?;
    compile_names(sheet_index, sheet, name, &path, compiler)?;
    compile_charts(sheet_index, sheet, name, &path, compiler)?;
    compile_object_array(
        sheet_index,
        sheet,
        "hyperlinks",
        "hyperlink",
        "xlsx hyperlinks add",
        name,
        &path,
        compiler,
    )?;
    compile_object_array(
        sheet_index,
        sheet,
        "comments",
        "comment",
        "xlsx comments add",
        name,
        &path,
        compiler,
    )?;
    if let Some(print_setup) = sheet.get("printSetup") {
        let mut args = print_setup
            .as_object()
            .expect("validated printSetup object")
            .clone();
        args.insert("sheet".to_string(), json!(name));
        push_simple(
            compiler,
            format!("{path}/printSetup"),
            format!("sheet-{}-print", sheet_index + 1),
            "xlsx sheets set-print",
            args,
        )?;
    }
    Ok(formula_cells)
}

fn compile_sheet_data(
    sheet_index: usize,
    sheet: &Map<String, Value>,
    columns: &[Value],
    name: &str,
    path: &str,
    compiler: &mut BuildCompiler,
) -> Result<(Option<String>, bool), BuildCompileError> {
    let rows = sheet.get("rows");
    let data_file = sheet.get("dataFile");
    if rows.is_some() && data_file.is_some() {
        return Err(invalid(
            path,
            "specify only one sheet data source: rows or dataFile",
        ));
    }
    let Some(source) = rows.or(data_file) else {
        return Ok((None, columns_have_formulas(columns)));
    };
    let mut args = map([("sheet", json!(name)), ("anchor", json!("A1"))]);
    if rows.is_some() {
        let source_rows = source.as_array().expect("validated rows array");
        if source_rows.is_empty() {
            return Err(invalid(
                format!("{path}/rows"),
                "inline sheet data must contain at least one row",
            ));
        }
        let transformed = transform_rows(source_rows, columns, &format!("{path}/rows"))?;
        args.insert(
            "values".to_string(),
            Value::String(serde_json::to_string(&transformed).map_err(|cause| {
                invalid(path, format!("failed to encode inline rows: {cause}"))
            })?),
        );
        args.insert("dataFormat".to_string(), json!("json"));
        insert_exact_max_cells(&mut args, transformed.len(), columns.len(), path)?;
    } else {
        let source = source.as_object().expect("validated dataFile object");
        args.insert("valuesFile".to_string(), source["path"].clone());
        args.insert("dataFormat".to_string(), source["format"].clone());
    }
    let op_id = format!("sheet-{}-data", sheet_index + 1);
    compiler.push_operation(
        path,
        sheet.get("id").and_then(Value::as_str),
        &op_id,
        "xlsx ranges set",
        args,
        "destination.range",
    )?;
    Ok((Some(op_id), columns_have_formulas(columns)))
}

fn compile_columns(
    sheet_index: usize,
    columns: &[Value],
    sheet: &str,
    data_op: Option<&str>,
    row_count: Option<usize>,
    path: &str,
    compiler: &mut BuildCompiler,
) -> Result<(), BuildCompileError> {
    for (column_index, column) in columns.iter().enumerate() {
        let column = column.as_object().expect("validated column object");
        let node_path = format!("{path}/columns/{column_index}");
        let column_number = u32::try_from(column_index + 1)
            .map_err(|_| invalid(&node_path, "column index is out of XLSX bounds A-XFD"))?;
        let letter = crate::xlsx_model::checked_col_name(column_number)
            .map_err(|err| invalid(&node_path, err.message))?;
        let mut operation_ids = Vec::new();
        if let Some(width) = column.get("width") {
            let id = format!(
                "sheet-{}-column-{}-width",
                sheet_index + 1,
                column_index + 1
            );
            compiler.push_internal_operation(
                &id,
                "xlsx colwidths set",
                map([
                    ("sheet", json!(sheet)),
                    ("range", json!(format!("{letter}:{letter}"))),
                    (
                        "width",
                        json!(column_width(width, &format!("{node_path}/width"))?),
                    ),
                ]),
            )?;
            operation_ids.push(id);
        }
        if column
            .get("autofit")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            let id = format!(
                "sheet-{}-column-{}-autofit",
                sheet_index + 1,
                column_index + 1
            );
            compiler.push_internal_operation(
                &id,
                "xlsx colwidths autofit",
                map([
                    ("sheet", json!(sheet)),
                    ("range", json!(format!("{letter}:{letter}"))),
                ]),
            )?;
            operation_ids.push(id);
        }
        let format = column_format(column);
        if let Some((key, value)) = format {
            let id = format!(
                "sheet-{}-column-{}-format",
                sheet_index + 1,
                column_index + 1
            );
            let range = row_count
                .map(|rows| format!("{letter}1:{letter}{rows}"))
                .unwrap_or_else(|| format!("{letter}:{letter}"));
            compiler.push_internal_operation(
                &id,
                "xlsx ranges set-format",
                map([
                    ("sheet", json!(sheet)),
                    ("range", json!(range)),
                    (key, value),
                ]),
            )?;
            operation_ids.push(id);
        }
        if let Some(op_id) = operation_ids.first() {
            compiler.map_node(&node_path, None, op_id, "destination.primarySelector")?;
        } else if let Some(data_op) = data_op {
            compiler.map_node(&node_path, None, data_op, "destination.primarySelector")?;
        }
    }
    Ok(())
}

fn compile_tables(
    sheet_index: usize,
    sheet: &Map<String, Value>,
    sheet_name: &str,
    data_op: Option<&str>,
    path: &str,
    compiler: &mut BuildCompiler,
) -> Result<(), BuildCompileError> {
    let tables = sheet
        .get("tables")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    for (table_index, table) in tables.iter().enumerate() {
        let table = table.as_object().expect("validated table object");
        let table_path = format!("{path}/tables/{table_index}");
        for unsupported in [
            "rows",
            "csv",
            "json",
            "xlsx",
            "slot",
            "bounds",
            "columnWidths",
        ] {
            if table.contains_key(unsupported) {
                return Err(unsupported_error(
                    format!("{table_path}/{unsupported}"),
                    "XLSX sheet tables describe the sheet data range; put data in rows or dataFile",
                ));
            }
        }
        if table.get("header").and_then(Value::as_bool) == Some(false) {
            return Err(unsupported_error(
                format!("{table_path}/header"),
                "XLSX tables require a header row",
            ));
        }
        if table.get("bandedRows").and_then(Value::as_bool) == Some(false) {
            return Err(unsupported_error(
                format!("{table_path}/bandedRows"),
                "unbanded table style overrides are not supported by tables create",
            ));
        }
        let data_op = data_op.ok_or_else(|| {
            invalid(
                &table_path,
                "a table requires sheet rows or dataFile so its range can be resolved",
            )
        })?;
        let mut args = map([
            ("sheet", json!(sheet_name)),
            (
                "range",
                operation_reference(data_op, "destination.range")
                    .map_err(|message| invalid(&table_path, message))?,
            ),
        ]);
        copy_value(table, "name", "table", &mut args);
        copy_value(table, "style", "style", &mut args);
        copy_value(table, "totalRow", "totalRow", &mut args);
        if let Some(totals) = table.get("totals") {
            args.insert(
                "totals".to_string(),
                json!(totals_string(totals, &table_path)?),
            );
        }
        compiler.push_operation(
            &table_path,
            table.get("id").and_then(Value::as_str),
            format!("sheet-{}-table-{}", sheet_index + 1, table_index + 1),
            "xlsx tables create",
            args,
            "destination.primarySelector",
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn compile_object_array(
    sheet_index: usize,
    sheet: &Map<String, Value>,
    field: &str,
    id_kind: &str,
    command: &str,
    sheet_name: &str,
    path: &str,
    compiler: &mut BuildCompiler,
) -> Result<(), BuildCompileError> {
    let values = sheet
        .get(field)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    for (index, value) in values.iter().enumerate() {
        let mut args = value.as_object().expect("validated build object").clone();
        args.remove("id");
        args.insert("sheet".to_string(), json!(sheet_name));
        compiler.push_operation(
            format!("{path}/{field}/{index}"),
            value.get("id").and_then(Value::as_str),
            format!("sheet-{}-{id_kind}-{}", sheet_index + 1, index + 1),
            command,
            args,
            "destination.primarySelector",
        )?;
    }
    Ok(())
}

fn compile_names(
    sheet_index: usize,
    sheet: &Map<String, Value>,
    sheet_name: &str,
    path: &str,
    compiler: &mut BuildCompiler,
) -> Result<(), BuildCompileError> {
    let values = sheet
        .get("names")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    for (index, value) in values.iter().enumerate() {
        let mut args = value.as_object().expect("validated name object").clone();
        args.remove("id");
        if args.contains_key("range") && !args.contains_key("ref") {
            args.insert("sheet".to_string(), json!(sheet_name));
        }
        compiler.push_operation(
            format!("{path}/names/{index}"),
            value.get("id").and_then(Value::as_str),
            format!("sheet-{}-name-{}", sheet_index + 1, index + 1),
            "xlsx names add",
            args,
            "destination.primarySelector",
        )?;
    }
    Ok(())
}

fn compile_charts(
    sheet_index: usize,
    sheet: &Map<String, Value>,
    sheet_name: &str,
    path: &str,
    compiler: &mut BuildCompiler,
) -> Result<(), BuildCompileError> {
    let charts = sheet
        .get("charts")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    for (index, chart) in charts.iter().enumerate() {
        let chart = chart.as_object().expect("validated chart object");
        let chart_path = format!("{path}/charts/{index}");
        if chart
            .get("categories")
            .and_then(Value::as_array)
            .is_some_and(|values| !values.is_empty())
            || chart
                .get("series")
                .and_then(Value::as_array)
                .is_some_and(|values| !values.is_empty())
        {
            return Err(unsupported_error(
                &chart_path,
                "XLSX charts use an existing sheet range or table source, not inline series",
            ));
        }
        let mut args = Map::new();
        let chart_type = required_string(chart, "type", &format!("{chart_path}/type"))?;
        args.insert(
            "type".to_string(),
            json!(match chart_type {
                "column" => "bar",
                "bar" | "line" | "area" | "pie" | "scatter" => chart_type,
                other => {
                    return Err(unsupported_error(
                        format!("{chart_path}/type"),
                        format!("chart type {other:?} is not supported by xlsx charts create"),
                    ));
                }
            }),
        );
        copy_value(chart, "title", "title", &mut args);
        if let Some(style) = chart.get("style") {
            args.insert(
                "style".to_string(),
                Value::String(match style {
                    Value::String(value) => value.clone(),
                    Value::Number(value) => value.to_string(),
                    _ => unreachable!("validated chart style"),
                }),
            );
        }
        let options = chart
            .get("options")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        for key in options.keys() {
            if !matches!(
                key.as_str(),
                "table" | "anchor" | "dataLabels" | "numberFormat" | "categories"
            ) {
                return Err(unsupported_error(
                    format!("{chart_path}/options/{key}"),
                    "unsupported XLSX chart option",
                ));
            }
        }
        if let Some(table) = options.get("table") {
            args.insert("table".to_string(), table.clone());
        } else {
            let source = chart
                .get("source")
                .and_then(Value::as_object)
                .ok_or_else(|| invalid(&chart_path, "chart requires source or options.table"))?;
            let source_path =
                required_string(source, "path", &format!("{chart_path}/source/path"))?;
            if !matches!(source_path, "." | "self") {
                return Err(unsupported_error(
                    format!("{chart_path}/source/path"),
                    "XLSX build charts can only source the workbook being built; use self",
                ));
            }
            args.insert("sheet".to_string(), source["sheet"].clone());
            args.insert("range".to_string(), source["range"].clone());
        }
        for key in ["anchor", "dataLabels", "numberFormat", "categories"] {
            if let Some(value) = options.get(key) {
                args.insert(key.to_string(), value.clone());
            }
        }
        if !args.contains_key("sheet") && !args.contains_key("table") {
            args.insert("sheet".to_string(), json!(sheet_name));
        }
        compiler.push_operation(
            &chart_path,
            chart.get("id").and_then(Value::as_str),
            format!("sheet-{}-chart-{}", sheet_index + 1, index + 1),
            "xlsx charts create",
            args,
            "destination.primarySelector",
        )?;
    }
    Ok(())
}

fn compile_metadata(
    metadata: Option<&Value>,
    formula_cells: bool,
    compiler: &mut BuildCompiler,
) -> Result<(), BuildCompileError> {
    if metadata.is_none() && !formula_cells {
        return Ok(());
    }
    let mut args = metadata
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    if formula_cells {
        args.insert("fullCalcOnLoad".to_string(), Value::Bool(true));
    }
    if metadata.is_some() {
        compiler.push_operation(
            "/metadata",
            None,
            "workbook-metadata",
            "xlsx workbook metadata update",
            args,
            "destination.primarySelector",
        )
    } else {
        compiler.push_internal_operation(
            "workbook-formula-recalc",
            "xlsx workbook metadata update",
            args,
        )
    }
}

fn scaffold_args(document: &Map<String, Value>) -> Result<Map<String, Value>, BuildCompileError> {
    if document.contains_key("theme") && document.contains_key("themeSeed") {
        return Err(invalid(
            "/",
            "specify only one workbook theme source: theme or themeSeed",
        ));
    }
    let sheets = document["sheets"]
        .as_array()
        .expect("validated sheets")
        .iter()
        .map(|sheet| sheet["name"].clone())
        .collect::<Vec<_>>();
    let mut args = map([("sheet", Value::Array(sheets))]);
    copy_value(document, "theme", "theme", &mut args);
    copy_value(document, "themeSeed", "themeSeed", &mut args);
    if let Some(brand) = document.get("brand") {
        let path = match brand {
            Value::String(path) => path,
            Value::Object(object) => {
                object.get("path").and_then(Value::as_str).ok_or_else(|| {
                    unsupported_error(
                        "/brand/name",
                        "named brand lookup is unavailable; provide brand.path",
                    )
                })?
            }
            _ => unreachable!("validated brand"),
        };
        args.insert("brand".to_string(), json!(path));
    }
    Ok(args)
}

fn sheet_column_count(
    sheet: &Map<String, Value>,
    columns: &[Value],
    path: &str,
) -> Result<usize, BuildCompileError> {
    let row_width = sheet
        .get("rows")
        .and_then(Value::as_array)
        .and_then(|rows| rows.first())
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    if !columns.is_empty() && row_width != 0 && columns.len() != row_width {
        return Err(invalid(
            format!("{path}/columns"),
            format!(
                "column definition count {} does not match row width {row_width}",
                columns.len()
            ),
        ));
    }
    Ok(columns.len().max(row_width))
}

fn transform_rows(
    rows: &[Value],
    columns: &[Value],
    path: &str,
) -> Result<Vec<Value>, BuildCompileError> {
    let width = rows
        .first()
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    let mut output = Vec::with_capacity(rows.len());
    for (row_index, row) in rows.iter().enumerate() {
        let row = row.as_array().expect("validated row");
        if row.len() != width {
            return Err(invalid(
                format!("{path}/{row_index}"),
                format!(
                    "row width {} does not match first row width {width}",
                    row.len()
                ),
            ));
        }
        let cells = row
            .iter()
            .enumerate()
            .map(|(column_index, value)| {
                if row_index == 0 {
                    return Ok(value.clone());
                }
                transform_typed_cell(
                    value,
                    column_type(columns, column_index),
                    &format!("{path}/{row_index}/{column_index}"),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        output.push(Value::Array(cells));
    }
    Ok(output)
}

fn columns_have_formulas(columns: &[Value]) -> bool {
    columns
        .iter()
        .any(|column| column.get("type").and_then(Value::as_str) == Some("formula"))
}

fn column_type(columns: &[Value], index: usize) -> Option<&str> {
    columns
        .get(index)
        .and_then(|column| column.get("type"))
        .and_then(Value::as_str)
}

fn transform_typed_cell(
    value: &Value,
    column_type: Option<&str>,
    path: &str,
) -> Result<Value, BuildCompileError> {
    let Some(column_type) = column_type else {
        return Ok(value.clone());
    };
    if value.is_null() {
        return Ok(Value::Null);
    }
    match column_type {
        "text" => value
            .as_str()
            .map(|text| json!(text))
            .ok_or_else(|| invalid(path, "text column values must be strings")),
        "number" | "currency" => numeric_cell(value, column_type, path),
        "percent" => {
            if let Some(number) = value.as_f64() {
                return finite_json_number(number, path, "percent");
            }
            let text = value
                .as_str()
                .ok_or_else(|| invalid(path, "percent values must be numbers or strings"))?
                .trim();
            let (number, divisor) = text
                .strip_suffix('%')
                .map_or((text, 1.0), |number| (number.trim(), 100.0));
            let parsed = number
                .parse::<f64>()
                .map_err(|_| invalid(path, format!("invalid percent value {text:?}")))?;
            finite_json_number(parsed / divisor, path, "percent")
        }
        "boolean" => {
            if let Some(value) = value.as_bool() {
                return Ok(json!(value));
            }
            match value.as_str().map(str::trim).map(str::to_ascii_lowercase) {
                Some(value) if matches!(value.as_str(), "true" | "1" | "yes") => Ok(json!(true)),
                Some(value) if matches!(value.as_str(), "false" | "0" | "no") => Ok(json!(false)),
                _ => Err(invalid(
                    path,
                    "boolean values must be true/false, yes/no, or 1/0",
                )),
            }
        }
        "formula" => {
            if value.get("formula").and_then(Value::as_str).is_some() {
                return Ok(value.clone());
            }
            let formula = value
                .as_str()
                .and_then(|text| text.trim().strip_prefix('='))
                .filter(|formula| !formula.trim().is_empty())
                .ok_or_else(|| {
                    invalid(
                        path,
                        "formula values must be non-empty strings beginning with =",
                    )
                })?;
            Ok(json!({"formula": formula}))
        }
        "date" => {
            if value.is_number() {
                return Ok(value.clone());
            }
            let text = value.as_str().ok_or_else(|| {
                invalid(
                    path,
                    "date values must be ISO YYYY-MM-DD strings or Excel serial numbers",
                )
            })?;
            finite_json_number(excel_date_serial(text, path)? as f64, path, "date")
        }
        _ => Ok(value.clone()),
    }
}

fn numeric_cell(value: &Value, kind: &str, path: &str) -> Result<Value, BuildCompileError> {
    if let Some(number) = value.as_f64() {
        return finite_json_number(number, path, kind);
    }
    let text = value
        .as_str()
        .ok_or_else(|| {
            invalid(
                path,
                format!("{kind} values must be numbers or numeric strings"),
            )
        })?
        .trim();
    let normalized = text
        .trim_start_matches(['$', '€', '£', '¥'])
        .replace([',', ' '], "");
    let number = normalized
        .parse::<f64>()
        .map_err(|_| invalid(path, format!("invalid {kind} value {text:?}")))?;
    finite_json_number(number, path, kind)
}

fn finite_json_number(value: f64, path: &str, kind: &str) -> Result<Value, BuildCompileError> {
    if !value.is_finite() {
        return Err(invalid(path, format!("{kind} value must be finite")));
    }
    Ok(json!(value))
}

fn excel_date_serial(value: &str, path: &str) -> Result<i64, BuildCompileError> {
    let mut fields = value.split('-');
    let year = fields.next().and_then(|value| value.parse::<i32>().ok());
    let month = fields.next().and_then(|value| value.parse::<u32>().ok());
    let day = fields.next().and_then(|value| value.parse::<u32>().ok());
    if fields.next().is_some() {
        return Err(invalid(path, format!("invalid ISO date {value:?}")));
    }
    let (year, month, day) = match (year, month, day) {
        (Some(year), Some(month), Some(day)) => (year, month, day),
        _ => return Err(invalid(path, format!("invalid ISO date {value:?}"))),
    };
    let max_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if year % 400 == 0 || (year % 4 == 0 && year % 100 != 0) => 29,
        2 => 28,
        _ => 0,
    };
    if year < 1900 || day == 0 || day > max_day {
        return Err(invalid(path, format!("invalid ISO date {value:?}")));
    }
    let year = year - i32::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let shifted_month = month as i32 + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * shifted_month + 2) / 5 + day as i32 - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    let unix_days = era as i64 * 146_097 + day_of_era as i64 - 719_468;
    Ok(unix_days + 25_569)
}

fn insert_exact_max_cells(
    args: &mut Map<String, Value>,
    rows: usize,
    columns: usize,
    path: &str,
) -> Result<(), BuildCompileError> {
    let cells = rows
        .checked_mul(columns)
        .ok_or_else(|| invalid(path, "sheet data cell count is too large"))?;
    if cells > 100_000 {
        let cells = i64::try_from(cells)
            .map_err(|_| invalid(path, "sheet data cell count exceeds the supported range"))?;
        args.insert("maxCells".to_string(), json!(cells));
    }
    Ok(())
}

fn column_format(column: &Map<String, Value>) -> Option<(&'static str, Value)> {
    if let Some(format) = column.get("format") {
        return Some(("formatCode", format.clone()));
    }
    match column.get("type").and_then(Value::as_str) {
        Some("currency") => Some(("preset", json!("currency"))),
        Some("percent") => Some(("preset", json!("percent"))),
        Some("date") => Some(("preset", json!("date"))),
        _ => None,
    }
}

fn column_width(value: &Value, path: &str) -> Result<f64, BuildCompileError> {
    let width: BuildLength = serde_json::from_value(value.clone())
        .map_err(|cause| invalid(path, format!("invalid column width: {cause}")))?;
    let width = match width {
        BuildLength::Emu(value) => value as f64,
        BuildLength::Human(value) => value.parse::<f64>().map_err(|_| {
            invalid(
                path,
                "column width must be a unitless number, not a physical length",
            )
        })?,
    };
    if width <= 0.0 || !width.is_finite() {
        return Err(invalid(path, "column width must be positive and finite"));
    }
    Ok(width)
}

fn parse_freeze(value: &str, path: &str) -> Result<(u32, u32), BuildCompileError> {
    let split = value
        .find(|character: char| character.is_ascii_digit())
        .ok_or_else(|| invalid(path, "freeze must be an A1 cell such as A2 or B2"))?;
    let (column, row) = value.split_at(split);
    if column.is_empty()
        || !column
            .chars()
            .all(|character| character.is_ascii_alphabetic())
        || row.is_empty()
        || !row.chars().all(|character| character.is_ascii_digit())
    {
        return Err(invalid(path, "freeze must be an A1 cell such as A2 or B2"));
    }
    let row = row
        .parse::<u32>()
        .map_err(|_| invalid(path, "freeze row is out of range"))?;
    let column = column.chars().try_fold(0_u32, |value, character| {
        value
            .checked_mul(26)?
            .checked_add(character.to_ascii_uppercase() as u32 - 'A' as u32 + 1)
    });
    let column = column.ok_or_else(|| invalid(path, "freeze column is out of range"))?;
    if row == 0 || column == 0 || (row == 1 && column == 1) {
        return Err(invalid(
            path,
            "freeze must select a cell below or right of A1",
        ));
    }
    Ok((row - 1, column - 1))
}

fn header_range(column_count: usize, path: &str) -> Result<String, BuildCompileError> {
    if column_count == 0 {
        return Err(invalid(
            path,
            "headerStyle requires columns or a non-empty first row",
        ));
    }
    let column_number = u32::try_from(column_count)
        .map_err(|_| invalid(path, "column count is out of XLSX bounds A-XFD"))?;
    let last_column = crate::xlsx_model::checked_col_name(column_number)
        .map_err(|err| invalid(path, err.message))?;
    Ok(format!("A1:{last_column}1"))
}

fn totals_string(value: &Value, path: &str) -> Result<String, BuildCompileError> {
    match value {
        Value::String(value) => Ok(value.clone()),
        Value::Array(values) => values
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(str::to_string)
                    .ok_or_else(|| invalid(format!("{path}/totals"), "totals must be strings"))
            })
            .collect::<Result<Vec<_>, _>>()
            .map(|values| values.join(",")),
        _ => Err(invalid(
            format!("{path}/totals"),
            "totals must be a string or string array",
        )),
    }
}

fn push_simple(
    compiler: &mut BuildCompiler,
    path: String,
    id: String,
    command: &str,
    args: Map<String, Value>,
) -> Result<(), BuildCompileError> {
    compiler.push_operation(path, None, id, command, args, "destination.primarySelector")
}

fn required_string<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    path: &str,
) -> Result<&'a str, BuildCompileError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| invalid(path, format!("{key} must be a non-empty string")))
}

fn copy_value(
    source: &Map<String, Value>,
    source_name: &str,
    target_name: &str,
    target: &mut Map<String, Value>,
) {
    if let Some(value) = source.get(source_name) {
        target.insert(target_name.to_string(), value.clone());
    }
}

fn map<const N: usize>(entries: [(&str, Value); N]) -> Map<String, Value> {
    entries
        .into_iter()
        .map(|(key, value)| (key.to_string(), value))
        .collect()
}

fn validate_output_path(output: &str, force: bool) -> crate::CliResult<()> {
    let path = Path::new(output);
    if path.extension().and_then(|value| value.to_str()) != Some("xlsx") {
        return Err(crate::CliError::invalid_args(
            "--out must use the .xlsx extension",
        ));
    }
    if path.exists() && !force {
        return Err(crate::CliError::invalid_args(
            "output file already exists; pass --force to replace it",
        ));
    }
    Ok(())
}

fn load_xlsx_build_spec(path: &str) -> crate::CliResult<(BuildSpec, PathBuf)> {
    if path == "-" {
        let mut source = Vec::new();
        std::io::stdin().read_to_end(&mut source).map_err(|cause| {
            crate::CliError::unexpected(format!("failed to read spec stdin: {cause}"))
        })?;
        let spec =
            super::load_spec_bytes(BuildFamily::Xlsx, &source).map_err(build_spec_cli_error)?;
        return Ok((
            spec,
            std::env::current_dir().map_err(|cause| {
                crate::CliError::unexpected(format!("failed to resolve current directory: {cause}"))
            })?,
        ));
    }
    let spec = super::load_spec_file(BuildFamily::Xlsx, path).map_err(build_spec_cli_error)?;
    let base = Path::new(path)
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .canonicalize()
        .map_err(|cause| {
            crate::CliError::unexpected(format!(
                "failed to resolve spec directory for {path}: {cause}"
            ))
        })?;
    Ok((spec, base))
}

fn materialize_operations(
    operations: &[super::BuildOperation],
    document: &Value,
    spec_base: &Path,
    temp: &Path,
) -> crate::CliResult<Vec<super::BuildOperation>> {
    let column_types = xlsx_column_types(document);
    let mut row_counts = BTreeMap::new();
    let mut materialized = Vec::with_capacity(operations.len());
    for (operation_index, operation) in operations.iter().enumerate() {
        let mut operation = operation.clone();
        if let Some(Value::String(brand)) = operation.args.get_mut("brand") {
            *brand = stage_build_source(brand, spec_base, temp, operation_index, "brand")?;
        }
        if operation.command == "xlsx ranges set"
            && let (Some(sheet), Some(values_file), Some(data_format)) = (
                operation
                    .args
                    .get("sheet")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                operation
                    .args
                    .get("valuesFile")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                operation
                    .args
                    .get("dataFormat")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            )
        {
            let values_file = resolve_source_path(&values_file, spec_base);
            let data = fs::read_to_string(&values_file).map_err(|cause| {
                crate::CliError::unexpected(format!(
                    "failed to read XLSX build data file {}: {cause}",
                    values_file.display()
                ))
            })?;
            let matrix = crate::xlsx_mutation::parse_xlsx_range_set_matrix(&data, &data_format)?;
            if matrix.rows.is_empty() {
                return Err(crate::CliError::invalid_args(format!(
                    "XLSX build data file for sheet {sheet:?} must contain at least one row"
                )));
            }
            row_counts.insert(sheet.clone(), matrix.rows.len());
            let types = column_types
                .get(&sheet)
                .map(Vec::as_slice)
                .unwrap_or_default();
            if !types.is_empty() {
                // The range importer deliberately treats CSV/TSV cells as text. Build specs
                // promise typed columns, so normalize the external matrix once into a staged
                // JSON values file before the atomic apply session consumes it.
                for (row_index, row) in matrix.rows.iter().enumerate() {
                    if row.len() != types.len() {
                        return Err(crate::CliError::invalid_args(format!(
                            "XLSX build data for sheet {sheet:?} row {} has {} columns; the spec declares {} typed columns",
                            row_index + 1,
                            row.len(),
                            types.len()
                        )));
                    }
                }
                let typed = matrix
                    .rows
                    .iter()
                    .enumerate()
                    .map(|(row_index, row)| {
                        row.iter()
                            .enumerate()
                            .map(|(column_index, cell)| {
                                let value = if cell.null {
                                    Value::Null
                                } else if !cell.formula.is_empty() {
                                    json!({"formula": cell.formula})
                                } else {
                                    Value::String(cell.value.clone())
                                };
                                if row_index == 0 {
                                    Ok(value)
                                } else {
                                    transform_typed_cell(
                                        &value,
                                        types.get(column_index).map(String::as_str),
                                        &format!(
                                            "sheet {sheet:?}, row {}, column {}",
                                            row_index + 1,
                                            column_index + 1
                                        ),
                                    )
                                    .map_err(build_compile_cli_error)
                                }
                            })
                            .collect::<crate::CliResult<Vec<_>>>()
                            .map(Value::Array)
                    })
                    .collect::<crate::CliResult<Vec<_>>>()?;
                let typed_path =
                    temp.join(format!("sheet-{}-typed-data.json", materialized.len() + 1));
                let mut bytes = serde_json::to_vec(&typed).map_err(|cause| {
                    crate::CliError::unexpected(format!(
                        "failed to encode typed XLSX build data: {cause}"
                    ))
                })?;
                bytes.push(b'\n');
                fs::write(&typed_path, bytes).map_err(|cause| {
                    crate::CliError::unexpected(format!(
                        "failed to write typed XLSX build data: {cause}"
                    ))
                })?;
                operation.args.insert(
                    "valuesFile".to_string(),
                    Value::String(
                        typed_path
                            .file_name()
                            .expect("typed build data filename")
                            .to_string_lossy()
                            .into_owned(),
                    ),
                );
                operation
                    .args
                    .insert("dataFormat".to_string(), json!("json"));
            } else {
                operation.args.insert(
                    "valuesFile".to_string(),
                    Value::String(stage_resolved_build_source(
                        &values_file,
                        temp,
                        operation_index,
                        "valuesFile",
                    )?),
                );
            }
            let column_count = matrix.rows.iter().map(Vec::len).max().unwrap_or(0);
            insert_exact_max_cells(
                &mut operation.args,
                matrix.rows.len(),
                column_count,
                &format!("sheet {sheet:?}"),
            )
            .map_err(build_compile_cli_error)?;
        }
        if operation.command == "xlsx ranges set-format"
            && let (Some(sheet), Some(range)) = (
                operation.args.get("sheet").and_then(Value::as_str),
                operation.args.get("range").and_then(Value::as_str),
            )
            && let Some(column) = whole_column_range(range)
        {
            let rows = row_counts.get(sheet).copied().ok_or_else(|| {
                crate::CliError::unexpected(format!(
                    "could not resolve row count for formatted data sheet {sheet:?}"
                ))
            })?;
            operation.args.insert(
                "range".to_string(),
                json!(format!("{column}1:{column}{rows}")),
            );
        }
        materialized.push(operation);
    }
    Ok(materialized)
}

fn resolve_source_path(path: &str, spec_base: &Path) -> PathBuf {
    if Path::new(path).is_absolute() {
        PathBuf::from(path)
    } else {
        spec_base.join(path)
    }
}

fn stage_build_source(
    value: &str,
    spec_base: &Path,
    temp: &Path,
    operation_index: usize,
    key: &str,
) -> crate::CliResult<String> {
    let source = resolve_source_path(value, spec_base);
    stage_resolved_build_source(&source, temp, operation_index, key)
}

fn stage_resolved_build_source(
    source: &Path,
    temp: &Path,
    operation_index: usize,
    key: &str,
) -> crate::CliResult<String> {
    let extension = source
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| format!(".{value}"))
        .unwrap_or_default();
    let relative =
        PathBuf::from("external").join(format!("op-{}-{key}{extension}", operation_index + 1));
    let destination = temp.join(&relative);
    fs::create_dir_all(destination.parent().expect("staged source parent")).map_err(|cause| {
        crate::CliError::unexpected(format!("failed to create build source stage: {cause}"))
    })?;
    fs::copy(source, &destination).map_err(|cause| {
        crate::CliError::invalid_args(format!(
            "failed to stage XLSX build source {}: {cause}",
            source.display()
        ))
    })?;
    Ok(relative.to_string_lossy().into_owned())
}

fn xlsx_column_types(document: &Value) -> BTreeMap<String, Vec<String>> {
    document["sheets"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|sheet| {
            let name = sheet.get("name")?.as_str()?.to_string();
            let types = sheet
                .get("columns")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|column| {
                    column
                        .get("type")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .collect::<Vec<_>>();
            Some((name, types))
        })
        .collect()
}

fn whole_column_range(range: &str) -> Option<&str> {
    let (start, end) = range.split_once(':')?;
    (start == end
        && !start.is_empty()
        && start
            .chars()
            .all(|character| character.is_ascii_alphabetic()))
    .then_some(start)
}

struct XlsxBuildTemp {
    path: PathBuf,
}

impl XlsxBuildTemp {
    fn create() -> crate::CliResult<Self> {
        let nonce = NEXT_STAGE.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("ooxml-xlsx-build-{}-{nonce}", std::process::id()));
        fs::create_dir_all(&path).map_err(|cause| {
            crate::CliError::unexpected(format!(
                "failed to create XLSX build staging directory: {cause}"
            ))
        })?;
        Ok(Self { path })
    }
}

impl Drop for XlsxBuildTemp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn scrub_build_paths(value: Value, temp: &Path, spec_base: &Path) -> Value {
    let temp_aliases = super::path_scrub::path_prefix_aliases(temp);
    let spec_base = spec_base.to_string_lossy();
    scrub_build_path_strings(value, &temp_aliases, spec_base.as_ref())
}

fn scrub_build_path_strings(value: Value, temp_aliases: &[String], spec_base: &str) -> Value {
    match value {
        Value::String(text) => {
            let text = super::path_scrub::scrub_path_aliases(&text, temp_aliases, "<build-stage>");
            Value::String(super::path_scrub::scrub_path_string(
                &text,
                spec_base,
                "<spec-dir>",
            ))
        }
        Value::Array(values) => Value::Array(
            values
                .into_iter()
                .map(|value| scrub_build_path_strings(value, temp_aliases, spec_base))
                .collect(),
        ),
        Value::Object(values) => Value::Object(
            values
                .into_iter()
                .map(|(key, value)| {
                    (
                        key,
                        scrub_build_path_strings(value, temp_aliases, spec_base),
                    )
                })
                .collect(),
        ),
        scalar => scalar,
    }
}

fn resolved_node_map(plan: &CompiledBuildPlan, envelope: &Value) -> Value {
    let applied = envelope["applied"].as_array();
    let map = plan
        .node_map
        .iter()
        .map(|(path, node)| {
            let operation = applied.and_then(|items| {
                items
                    .iter()
                    .find(|item| item["id"].as_str() == Some(node.op_id.as_str()))
            });
            let selector = operation
                .and_then(|item| item.pointer("/mutationEnvelope/destination/primarySelector"))
                .cloned()
                .unwrap_or(Value::Null);
            (
                path.clone(),
                json!({
                    "opId": node.op_id,
                    "specId": node.spec_id,
                    "selector": selector,
                }),
            )
        })
        .collect::<BTreeMap<_, _>>();
    serde_json::to_value(map).expect("resolved build node map is serializable")
}

fn build_spec_cli_error(error: super::BuildSpecError) -> crate::CliError {
    crate::CliError::invalid_args(
        serde_json::to_string(&error).unwrap_or_else(|_| error.to_string()),
    )
}

fn build_compile_cli_error(error: BuildCompileError) -> crate::CliError {
    crate::CliError::invalid_args(
        serde_json::to_string(&error).unwrap_or_else(|_| error.to_string()),
    )
}

fn invalid(path: impl Into<String>, message: impl Into<String>) -> BuildCompileError {
    error(path, None, "BUILD_SPEC_VALUE_INVALID", message)
}

fn unsupported_error(path: impl Into<String>, message: impl Into<String>) -> BuildCompileError {
    error(path, None, "BUILD_SPEC_OPERATION_UNAVAILABLE", message)
}

fn error(
    path: impl Into<String>,
    op_id: Option<&str>,
    code: &str,
    message: impl Into<String>,
) -> BuildCompileError {
    BuildCompileError {
        code: code.to_string(),
        path: path.into(),
        op_id: op_id.map(str::to_string),
        message: message.into(),
    }
}
