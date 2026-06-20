use serde_json::{Map, Value, json};
use std::fs;
use std::path::{Path, PathBuf};

use crate::pptx_mutation::{
    pptx_place_image, pptx_place_table_from_xlsx, pptx_replace_images, pptx_replace_text_from_xlsx,
    pptx_shapes_set_bounds, pptx_tables_update_from_xlsx,
};
use crate::{
    CliError, CliResult, XlsxRangeExportOptions, XlsxTableExportOptions, command_arg, has_flag,
    package_mutation_temp_path, parse_i64_flag, parse_string_flag, pptx_shapes_show,
    pptx_tables_show, reject_unknown_flags, validate, validate_xlsx_mutation_output_flags,
    xlsx_range_export_with_options, xlsx_tables_export,
};

#[derive(Clone, Default)]
struct BindingRow {
    source_row: usize,
    id: String,
    op: String,
    slide: u32,
    target: String,
    source_sheet: String,
    source_range: String,
    source_table: String,
    expect_source_range: String,
    formula_mode: String,
    mode: String,
    row_sep: String,
    col_sep: String,
    fit_mode: String,
    image_path: String,
    x: i64,
    y: i64,
    cx: i64,
    cy: i64,
    has_x: bool,
    has_y: bool,
    has_cx: bool,
    has_cy: bool,
    name: String,
    header: bool,
}

struct LoadedSource {
    source: Value,
    values: Vec<Vec<String>>,
}

struct BindingPlan {
    binding_source: Value,
    rows: Vec<BindingRow>,
    operations: Vec<Value>,
}

struct ApplyOptions {
    out: Option<String>,
    backup: Option<String>,
    dry_run: bool,
    in_place: bool,
    no_validate: bool,
}

pub(crate) fn pptx_xlsx_bindings_plan(file: &str, args: &[String]) -> CliResult<Value> {
    reject_unknown_flags(
        args,
        &["--max-cells", "--range", "--sheet", "--table", "--workbook"],
        &[],
    )?;
    let plan = prepare_binding_plan_from_args(file, args)?;
    Ok(json!({
        "file": file,
        "bindingSource": plan.binding_source,
        "operations": plan.operations,
    }))
}

pub(crate) fn pptx_xlsx_bindings_apply(file: &str, args: &[String]) -> CliResult<Value> {
    reject_unknown_flags(
        args,
        &[
            "--backup",
            "--max-cells",
            "--out",
            "--range",
            "--sheet",
            "--table",
            "--workbook",
        ],
        &["--dry-run", "--in-place", "--no-validate"],
    )?;
    let options = ApplyOptions {
        out: parse_string_flag(args, "--out")?,
        backup: parse_string_flag(args, "--backup")?,
        dry_run: has_flag(args, "--dry-run"),
        in_place: has_flag(args, "--in-place"),
        no_validate: has_flag(args, "--no-validate"),
    };
    validate_xlsx_mutation_output_flags(
        options.out.as_deref(),
        options.in_place,
        options.backup.as_deref(),
        options.dry_run,
    )?;
    let workbook = parse_workbook_arg(args)?;
    let max_cells = parse_i64_flag(args, "--max-cells")?.unwrap_or(100_000);
    let plan = prepare_binding_plan_from_args(file, args)?;
    let output = apply_output_path(file, &options);
    let command_target = output.as_deref().unwrap_or("<out.pptx>");
    let mut current = file.to_string();
    let mut temp_paths = Vec::<String>::new();
    let mut operations = Vec::<Value>::new();

    for (row, planned) in plan.rows.iter().zip(plan.operations.iter()) {
        let step_out = package_mutation_temp_path(file, "pptx-xlsx-bindings");
        let leaf = apply_binding_leaf(&current, &workbook, max_cells, row, planned, &step_out)?;
        if current != file {
            let _ = fs::remove_file(&current);
        }
        current = step_out.clone();
        temp_paths.push(step_out);
        operations.push(applied_operation(
            planned,
            &leaf,
            row,
            command_target,
            options.dry_run,
        )?);
    }

    if current != file {
        if !options.no_validate {
            validate(&current, true)?;
        }
        if options.dry_run {
            let _ = fs::remove_file(&current);
        } else {
            commit_apply_output(file, &current, output.as_deref(), &options)?;
        }
    } else if !options.no_validate {
        validate(file, true)?;
    }
    for path in temp_paths {
        if path != current {
            let _ = fs::remove_file(path);
        }
    }

    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    if let Some(output) = output {
        result.insert("output".to_string(), json!(output));
    }
    if options.dry_run {
        result.insert("dryRun".to_string(), json!(true));
    }
    result.insert("bindingSource".to_string(), plan.binding_source);
    result.insert("operations".to_string(), Value::Array(operations));
    Ok(Value::Object(result))
}

fn prepare_binding_plan_from_args(file: &str, args: &[String]) -> CliResult<BindingPlan> {
    let workbook = parse_string_flag(args, "--workbook")?
        .ok_or_else(|| CliError::invalid_args("--workbook is required"))?;
    if !Path::new(&workbook).exists() {
        return Err(CliError::file_not_found(format!(
            "file not found: {workbook}"
        )));
    }
    let sheet = parse_string_flag(args, "--sheet")?.unwrap_or_default();
    let range = parse_string_flag(args, "--range")?.unwrap_or_default();
    let table = parse_string_flag(args, "--table")?.unwrap_or_default();
    let max_cells = parse_i64_flag(args, "--max-cells")?.unwrap_or(100_000);
    let binding_source = load_source(&workbook, &sheet, &range, &table, max_cells, "value")?;
    let rows = parse_binding_rows(&binding_source.values)?;
    let mut operations = Vec::<Value>::new();
    let mut seen_destinations = std::collections::BTreeMap::<String, usize>::new();
    for row in &rows {
        let op = plan_binding_row(file, &workbook, max_cells, row.clone())?;
        if let Some(key) = duplicate_target_key(&op)
            && let Some(previous) = seen_destinations.insert(key.clone(), row.source_row)
        {
            return Err(CliError::invalid_args(format!(
                "row {} duplicates destination target from row {previous}: {key}",
                row.source_row
            )));
        }
        operations.push(op);
    }
    Ok(BindingPlan {
        binding_source: binding_source.source,
        rows,
        operations,
    })
}

fn parse_workbook_arg(args: &[String]) -> CliResult<String> {
    let workbook = parse_string_flag(args, "--workbook")?
        .ok_or_else(|| CliError::invalid_args("--workbook is required"))?;
    if !Path::new(&workbook).exists() {
        return Err(CliError::file_not_found(format!(
            "file not found: {workbook}"
        )));
    }
    Ok(workbook)
}

fn apply_output_path(file: &str, options: &ApplyOptions) -> Option<String> {
    if options.dry_run {
        None
    } else if options.in_place {
        Some(file.to_string())
    } else {
        options
            .out
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(ToString::to_string)
    }
}

fn commit_apply_output(
    file: &str,
    staged_path: &str,
    output: Option<&str>,
    options: &ApplyOptions,
) -> CliResult<()> {
    let target = output.ok_or_else(|| {
        CliError::invalid_args("must specify exactly one of --out, --in-place, or --dry-run")
    })?;
    if (options.in_place || target == file)
        && let Some(backup) = options
            .backup
            .as_deref()
            .filter(|value| !value.trim().is_empty())
    {
        fs::copy(file, backup)
            .map_err(|err| CliError::unexpected(format!("failed to create backup: {err}")))?;
    }
    fs::rename(staged_path, target)
        .or_else(|_| {
            fs::copy(staged_path, target)?;
            fs::remove_file(staged_path)
        })
        .map_err(|err| CliError::unexpected(format!("failed to write output file: {err}")))?;
    Ok(())
}

fn apply_binding_leaf(
    input: &str,
    workbook: &str,
    max_cells: i64,
    row: &BindingRow,
    planned: &Value,
    out: &str,
) -> CliResult<Value> {
    let mut args = Vec::<String>::new();
    match row.op.as_str() {
        "replace-text" => {
            append_source_args(&mut args, workbook, row, planned);
            push_flag(&mut args, "--slide", row.slide.to_string());
            push_flag(&mut args, "--target", row.target.clone());
            if !row.mode.is_empty() {
                push_flag(&mut args, "--mode", row.mode.clone());
            }
            if !row.row_sep.is_empty() {
                push_flag(&mut args, "--row-sep", row.row_sep.clone());
            }
            if !row.col_sep.is_empty() {
                push_flag(&mut args, "--col-sep", row.col_sep.clone());
            }
            append_formula_and_guard_args(&mut args, row, max_cells);
            append_write_args(&mut args, out);
            pptx_replace_text_from_xlsx(input, &args)
        }
        "update-table" => {
            append_source_args(&mut args, workbook, row, planned);
            push_flag(&mut args, "--slide", row.slide.to_string());
            push_flag(&mut args, "--target", row.target.clone());
            append_formula_and_guard_args(&mut args, row, max_cells);
            append_write_args(&mut args, out);
            pptx_tables_update_from_xlsx(input, &args)
        }
        "place-table" => {
            append_source_args(&mut args, workbook, row, planned);
            push_flag(&mut args, "--slide", row.slide.to_string());
            push_flag(&mut args, "--x", row.x.to_string());
            push_flag(&mut args, "--y", row.y.to_string());
            push_flag(&mut args, "--cx", row.cx.to_string());
            if row.cy > 0 {
                push_flag(&mut args, "--cy", row.cy.to_string());
            }
            if !row.name.is_empty() {
                push_flag(&mut args, "--name", row.name.clone());
            }
            if row.header {
                args.push("--header".to_string());
            }
            append_formula_and_guard_args(&mut args, row, max_cells);
            append_write_args(&mut args, out);
            pptx_place_table_from_xlsx(input, &args)
        }
        "place-image" => {
            push_flag(&mut args, "--slide", row.slide.to_string());
            push_flag(&mut args, "--image", resolved_image_arg(planned, row));
            push_flag(&mut args, "--x", row.x.to_string());
            push_flag(&mut args, "--y", row.y.to_string());
            push_flag(&mut args, "--cx", row.cx.to_string());
            push_flag(&mut args, "--cy", row.cy.to_string());
            if !row.fit_mode.is_empty() {
                push_flag(&mut args, "--fit-mode", row.fit_mode.clone());
            }
            if !row.name.is_empty() {
                push_flag(&mut args, "--name", row.name.clone());
            }
            append_write_args(&mut args, out);
            pptx_place_image(input, &args)
        }
        "replace-image" => {
            push_flag(&mut args, "--slide", row.slide.to_string());
            let target = planned
                .get("destination")
                .and_then(|destination| destination.get("primarySelector"))
                .and_then(Value::as_str)
                .unwrap_or(&row.target);
            push_flag(&mut args, "--target", target.to_string());
            push_flag(&mut args, "--image", resolved_image_arg(planned, row));
            if !row.fit_mode.is_empty() {
                push_flag(&mut args, "--fit-mode", row.fit_mode.clone());
            }
            append_write_args(&mut args, out);
            pptx_replace_images(input, &args)
        }
        "set-bounds" => {
            push_flag(&mut args, "--slide", row.slide.to_string());
            let target = planned
                .get("destination")
                .and_then(|destination| destination.get("primarySelector"))
                .and_then(Value::as_str)
                .unwrap_or(&row.target);
            push_flag(&mut args, "--target", target.to_string());
            push_flag(
                &mut args,
                "--bounds",
                format!("{},{},{},{}", row.x, row.y, row.cx, row.cy),
            );
            append_write_args(&mut args, out);
            pptx_shapes_set_bounds(input, &args)
        }
        _ => Err(row_error(
            row,
            "op must be replace-text, update-table, place-table, place-image, replace-image, or set-bounds",
        )),
    }
}

fn append_source_args(args: &mut Vec<String>, workbook: &str, row: &BindingRow, planned: &Value) {
    push_flag(args, "--workbook", workbook.to_string());
    if !row.source_table.is_empty() && row.op != "replace-text" {
        push_flag(args, "--table", row.source_table.clone());
        if !row.source_sheet.is_empty() {
            push_flag(args, "--sheet", row.source_sheet.clone());
        }
        return;
    }
    let source = planned.get("source").unwrap_or(&Value::Null);
    let sheet = if !row.source_sheet.is_empty() {
        row.source_sheet.as_str()
    } else {
        source.get("sheet").and_then(Value::as_str).unwrap_or("")
    };
    let range = if !row.source_range.is_empty() {
        row.source_range.as_str()
    } else {
        source.get("range").and_then(Value::as_str).unwrap_or("")
    };
    push_flag(args, "--sheet", sheet.to_string());
    push_flag(args, "--range", range.to_string());
}

fn append_formula_and_guard_args(args: &mut Vec<String>, row: &BindingRow, max_cells: i64) {
    if !row.expect_source_range.is_empty() {
        push_flag(
            args,
            "--expect-source-range",
            row.expect_source_range.clone(),
        );
    }
    if !row.formula_mode.is_empty() {
        push_flag(args, "--formula-mode", row.formula_mode.clone());
    }
    if max_cells != 100_000 {
        push_flag(args, "--max-cells", max_cells.to_string());
    }
}

fn append_write_args(args: &mut Vec<String>, out: &str) {
    push_flag(args, "--out", out.to_string());
    args.push("--no-validate".to_string());
}

fn push_flag(args: &mut Vec<String>, name: &str, value: String) {
    args.push(name.to_string());
    args.push(value);
}

fn resolved_image_arg(planned: &Value, row: &BindingRow) -> String {
    planned
        .get("image")
        .and_then(|image| image.get("resolvedPath"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(&row.image_path)
        .to_string()
}

fn applied_operation(
    planned: &Value,
    leaf: &Value,
    row: &BindingRow,
    command_target: &str,
    dry_run: bool,
) -> CliResult<Value> {
    let mut operation = planned
        .as_object()
        .cloned()
        .ok_or_else(|| CliError::unexpected("binding plan operation is not an object"))?;
    operation.insert(
        "status".to_string(),
        json!(if dry_run { "dry-run" } else { "applied" }),
    );
    if let Some(source) = leaf.get("source") {
        operation.insert("source".to_string(), source.clone());
    }
    if let Some(text) = leaf.get("text") {
        operation.insert("text".to_string(), text.clone());
    }
    if let Some(update) = leaf.get("update") {
        operation.insert("update".to_string(), update.clone());
    }
    if let Some(bounds) = leaf_bounds(row, leaf) {
        operation.insert("bounds".to_string(), bounds);
    }
    if let Some(image) = applied_image(planned, leaf) {
        operation.insert("image".to_string(), image);
    }
    let mut destination = leaf
        .get("destination")
        .cloned()
        .ok_or_else(|| CliError::unexpected("binding mutation result missing destination"))?;
    if row.op == "set-bounds"
        && let Some(map) = destination.as_object_mut()
    {
        map.remove("textPreview");
    }
    rewrite_destination_file(&mut destination, command_target, dry_run);
    operation.insert("destination".to_string(), destination.clone());
    operation.insert(
        "readbackCommand".to_string(),
        json!(binding_readback_command(row, &destination, command_target)),
    );
    Ok(Value::Object(operation))
}

fn leaf_bounds(row: &BindingRow, leaf: &Value) -> Option<Value> {
    if row.op != "set-bounds" {
        return leaf.get("bounds").cloned();
    }
    Some(json!({
        "x": leaf.get("newX").and_then(Value::as_i64).unwrap_or(row.x),
        "y": leaf.get("newY").and_then(Value::as_i64).unwrap_or(row.y),
        "cx": leaf.get("newCx").and_then(Value::as_i64).unwrap_or(row.cx),
        "cy": leaf.get("newCy").and_then(Value::as_i64).unwrap_or(row.cy),
    }))
}

fn applied_image(planned: &Value, leaf: &Value) -> Option<Value> {
    let mut image = planned.get("image")?.as_object()?.clone();
    for (leaf_key, image_key) in [
        ("relationshipId", "relationshipId"),
        ("targetUri", "targetUri"),
        ("oldTargetUri", "oldTargetUri"),
        ("oldContentType", "oldContentType"),
        ("newTargetUri", "newTargetUri"),
        ("newContentType", "newContentType"),
    ] {
        if let Some(value) = leaf.get(leaf_key)
            && !value.is_null()
        {
            image.insert(image_key.to_string(), value.clone());
        }
    }
    if image.get("targetUri").is_none()
        && let Some(value) = leaf.get("newTargetUri")
    {
        image.insert("targetUri".to_string(), value.clone());
    }
    Some(Value::Object(image))
}

fn rewrite_destination_file(destination: &mut Value, command_target: &str, dry_run: bool) {
    if let Some(map) = destination.as_object_mut() {
        if dry_run {
            map.remove("file");
        } else {
            map.insert("file".to_string(), json!(command_target));
        }
    }
}

fn binding_readback_command(row: &BindingRow, destination: &Value, command_target: &str) -> String {
    let selector = primary_selector(destination);
    match row.op.as_str() {
        "update-table" | "place-table" => format!(
            "ooxml --json pptx tables show {command_target} --slide {} --target {selector}",
            row.slide
        ),
        "replace-text" => format!(
            "ooxml --json pptx shapes get {command_target} --slide {} --target {selector} --include-text",
            row.slide
        ),
        _ => format!(
            "ooxml --json pptx shapes get {command_target} --slide {} --target {selector} --include-bounds",
            row.slide
        ),
    }
}

fn plan_binding_row(
    deck: &str,
    workbook: &str,
    max_cells: i64,
    mut row: BindingRow,
) -> CliResult<Value> {
    row.formula_mode = normalize_formula_mode(&row.formula_mode)?;
    let mut op = Map::new();
    if !row.id.is_empty() {
        op.insert("id".to_string(), json!(row.id));
    }
    op.insert("sourceRow".to_string(), json!(row.source_row));
    op.insert("op".to_string(), json!(row.op));
    op.insert("status".to_string(), json!("planned"));
    match row.op.as_str() {
        "replace-text" => {
            let source = load_source(
                workbook,
                &row.source_sheet,
                &row.source_range,
                &row.source_table,
                max_cells,
                &row.formula_mode,
            )?;
            check_expected_source_range(&row, &source.source)?;
            let mode = normalize_text_mode(&row.mode)?;
            row.mode = mode.clone();
            let row_sep = decode_separator(if row.row_sep.is_empty() {
                "\\n"
            } else {
                &row.row_sep
            })?;
            let col_sep = decode_separator(if row.col_sep.is_empty() {
                "\\t"
            } else {
                &row.col_sep
            })?;
            let text = join_matrix(&source.values, &row_sep, &col_sep);
            let destination = shape_destination(deck, row.slide, &row.target, true, true, "")?;
            op.insert("source".to_string(), source.source);
            op.insert(
                "text".to_string(),
                json!({
                    "mode": mode,
                    "formulaMode": row.formula_mode,
                    "rowSeparator": row_sep,
                    "colSeparator": col_sep,
                    "chars": text.chars().count(),
                    "value": text,
                }),
            );
            op.insert("destination".to_string(), destination.clone());
            op.insert(
                "readbackCommand".to_string(),
                json!(format!(
                    "ooxml --json pptx shapes get <out.pptx> --slide {} --target {} --include-text",
                    row.slide,
                    command_arg(primary_selector(&destination))
                )),
            );
            op.insert(
                "equivalentCommand".to_string(),
                json!(equivalent_command(deck, workbook, &row)),
            );
        }
        "update-table" => {
            let source = load_source(
                workbook,
                &row.source_sheet,
                &row.source_range,
                &row.source_table,
                max_cells,
                &row.formula_mode,
            )?;
            check_expected_source_range(&row, &source.source)?;
            let destination = table_destination(deck, row.slide, &row.target, "")?;
            let rows = source_rows(&source.source);
            let cols = source_cols(&source.source);
            let dest_rows = destination
                .get("rows")
                .and_then(Value::as_i64)
                .unwrap_or_default();
            let dest_cols = destination
                .get("cols")
                .and_then(Value::as_i64)
                .unwrap_or_default();
            if dest_rows != rows || dest_cols != cols {
                return Err(row_error(
                    &row,
                    format!(
                        "source/destination dimension mismatch: source is {rows}x{cols}, destination table is {dest_rows}x{dest_cols}"
                    ),
                ));
            }
            op.insert("source".to_string(), source.source);
            op.insert(
                "update".to_string(),
                json!({"formulaMode": row.formula_mode, "updatedCells": rows * cols}),
            );
            op.insert("destination".to_string(), destination.clone());
            op.insert(
                "readbackCommand".to_string(),
                json!(format!(
                    "ooxml --json pptx tables show <out.pptx> --slide {} --target {}",
                    row.slide,
                    command_arg(primary_selector(&destination))
                )),
            );
            op.insert(
                "equivalentCommand".to_string(),
                json!(equivalent_command(deck, workbook, &row)),
            );
        }
        "place-table" => {
            let source = load_source(
                workbook,
                &row.source_sheet,
                &row.source_range,
                &row.source_table,
                max_cells,
                &row.formula_mode,
            )?;
            check_expected_source_range(&row, &source.source)?;
            if source.values.is_empty() || source.values.first().is_some_and(Vec::is_empty) {
                return Err(row_error(&row, "source range is empty"));
            }
            if row.cx <= 0 {
                return Err(row_error(&row, "cx must be positive for place-table"));
            }
            let rows = source_rows(&source.source);
            let cols = source_cols(&source.source);
            op.insert("source".to_string(), source.source);
            op.insert(
                "destination".to_string(),
                json!({
                    "slide": row.slide,
                    "name": row.name,
                    "rows": rows,
                    "cols": cols,
                    "x": row.x,
                    "y": row.y,
                    "cx": row.cx,
                    "cy": row.cy,
                }),
            );
            op.insert(
                "equivalentCommand".to_string(),
                json!(equivalent_command(deck, workbook, &row)),
            );
        }
        "place-image" => {
            if row.cx <= 0 || row.cy <= 0 {
                return Err(row_error(
                    &row,
                    "cx and cy must be positive for place-image",
                ));
            }
            let image = image_plan(workbook, &row)?;
            op.insert("image".to_string(), image);
            op.insert(
                "destination".to_string(),
                json!({
                    "slide": row.slide,
                    "name": row.name,
                    "x": row.x,
                    "y": row.y,
                    "cx": row.cx,
                    "cy": row.cy,
                }),
            );
            op.insert(
                "equivalentCommand".to_string(),
                json!(equivalent_command(deck, workbook, &row)),
            );
        }
        "replace-image" => {
            if row.target.is_empty() {
                return Err(row_error(&row, "target is required for replace-image"));
            }
            let destination = shape_destination(deck, row.slide, &row.target, false, true, "")?;
            if destination.get("imageRef").is_none() {
                return Err(row_error(
                    &row,
                    format!("target {} is not an image shape", row.target),
                ));
            }
            let image = image_plan(workbook, &row)?;
            row.target = format!(
                "shape:{}",
                destination
                    .get("shapeId")
                    .and_then(Value::as_i64)
                    .unwrap_or_default()
            );
            op.insert("image".to_string(), image);
            op.insert("destination".to_string(), destination.clone());
            op.insert(
                "readbackCommand".to_string(),
                json!(format!(
                    "ooxml --json pptx shapes get <out.pptx> --slide {} --target {} --include-bounds",
                    row.slide,
                    command_arg(primary_selector(&destination))
                )),
            );
            op.insert(
                "equivalentCommand".to_string(),
                json!(equivalent_command(deck, workbook, &row)),
            );
        }
        "set-bounds" => {
            if row.target.is_empty() {
                return Err(row_error(&row, "target is required for set-bounds"));
            }
            if !row.has_x || !row.has_y || !row.has_cx || !row.has_cy {
                return Err(row_error(
                    &row,
                    "x, y, cx, and cy are required for set-bounds",
                ));
            }
            if row.cx <= 0 || row.cy <= 0 {
                return Err(row_error(&row, "cx and cy must be positive for set-bounds"));
            }
            let destination = shape_destination(deck, row.slide, &row.target, false, true, "")?;
            op.insert(
                "bounds".to_string(),
                json!({"x": row.x, "y": row.y, "cx": row.cx, "cy": row.cy}),
            );
            op.insert("destination".to_string(), destination.clone());
            op.insert(
                "readbackCommand".to_string(),
                json!(format!(
                    "ooxml --json pptx shapes get <out.pptx> --slide {} --target {} --include-bounds",
                    row.slide,
                    command_arg(primary_selector(&destination))
                )),
            );
            op.insert(
                "equivalentCommand".to_string(),
                json!(equivalent_command(deck, workbook, &row)),
            );
        }
        _ => {
            return Err(row_error(
                &row,
                "op must be replace-text, update-table, place-table, place-image, replace-image, or set-bounds",
            ));
        }
    }
    Ok(Value::Object(op))
}

fn load_source(
    workbook: &str,
    sheet: &str,
    range: &str,
    table: &str,
    max_cells: i64,
    formula_mode: &str,
) -> CliResult<LoadedSource> {
    if !range.trim().is_empty() && !table.trim().is_empty() {
        return Err(CliError::invalid_args(
            "specify only one of --range or --table",
        ));
    }
    if range.trim().is_empty() && table.trim().is_empty() {
        return Err(CliError::invalid_args("must specify --range or --table"));
    }
    if !range.trim().is_empty() && sheet.trim().is_empty() {
        return Err(CliError::invalid_args(
            "--sheet is required when using --range",
        ));
    }
    let exported = if !table.trim().is_empty() {
        xlsx_tables_export(
            workbook,
            if sheet.trim().is_empty() {
                None
            } else {
                Some(sheet)
            },
            Some(table),
            XlsxTableExportOptions {
                data_format: Some("json"),
                data_out: None,
                max_cells,
                include_types: false,
                include_formulas: true,
            },
        )?
    } else {
        xlsx_range_export_with_options(
            workbook,
            sheet,
            range,
            XlsxRangeExportOptions {
                include_types: false,
                include_formulas: true,
                include_formats: false,
                data_out: None,
                max_cells,
            },
        )?
    };
    let mut source = Map::new();
    source.insert("workbook".to_string(), json!(workbook));
    copy_field(&exported, &mut source, "sheet");
    copy_field(&exported, &mut source, "sheetNumber");
    copy_field(&exported, &mut source, "range");
    if !table.trim().is_empty() {
        source.insert("table".to_string(), json!(table));
    }
    copy_field(&exported, &mut source, "rows");
    copy_field(&exported, &mut source, "cols");
    copy_field(&exported, &mut source, "formulaCount");
    let values = source_matrix(&exported, formula_mode);
    Ok(LoadedSource {
        source: Value::Object(source),
        values,
    })
}

fn parse_binding_rows(values: &[Vec<String>]) -> CliResult<Vec<BindingRow>> {
    if values.len() < 2 {
        return Err(CliError::invalid_args(
            "binding source must include a header row and at least one operation row",
        ));
    }
    let headers = values[0]
        .iter()
        .enumerate()
        .filter_map(|(index, value)| {
            let key = normalize_header(value);
            if key.is_empty() {
                None
            } else {
                Some((key, index))
            }
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    values
        .iter()
        .enumerate()
        .skip(1)
        .map(|(index, row)| parse_binding_row(row, &headers, index + 1))
        .collect()
}

fn parse_binding_row(
    values: &[String],
    columns: &std::collections::BTreeMap<String, usize>,
    source_row: usize,
) -> CliResult<BindingRow> {
    let op = normalize_op(&column_value(values, columns, "op"));
    let mut row = BindingRow {
        source_row,
        id: column_value(values, columns, "id"),
        op,
        target: column_value(values, columns, "target"),
        source_sheet: first_column_value(values, columns, &["sourceSheet", "sheet"]),
        source_range: first_column_value(values, columns, &["sourceRange", "range"]),
        source_table: first_column_value(values, columns, &["sourceTable", "table"]),
        expect_source_range: first_column_value(
            values,
            columns,
            &["expectSourceRange", "expectRange"],
        ),
        formula_mode: first_column_value(values, columns, &["formulaMode", "formula"]),
        mode: column_value(values, columns, "mode"),
        row_sep: first_raw_column_value(values, columns, &["rowSep", "rowSeparator"]),
        col_sep: first_raw_column_value(values, columns, &["colSep", "colSeparator"]),
        fit_mode: first_column_value(values, columns, &["fitMode", "imageFit"]),
        image_path: first_column_value(
            values,
            columns,
            &["imagePath", "image", "imageFile", "path"],
        ),
        name: column_value(values, columns, "name"),
        ..BindingRow::default()
    };
    if row.fit_mode.is_empty() && matches!(row.op.as_str(), "place-image" | "replace-image") {
        row.fit_mode.clone_from(&row.mode);
    }
    row.slide = parse_required_u32(values, columns, "slide", source_row)?;
    (row.x, row.has_x) = parse_optional_i64(values, columns, "x", source_row)?;
    (row.y, row.has_y) = parse_optional_i64(values, columns, "y", source_row)?;
    (row.cx, row.has_cx) = parse_optional_i64(values, columns, "cx", source_row)?;
    (row.cy, row.has_cy) = parse_optional_i64(values, columns, "cy", source_row)?;
    row.header = parse_optional_bool(values, columns, "header", source_row)?;
    if row.op.is_empty() {
        return Err(CliError::invalid_args(format!(
            "row {source_row}: op is required"
        )));
    }
    if row.target.is_empty()
        && matches!(
            row.op.as_str(),
            "replace-text" | "update-table" | "replace-image" | "set-bounds"
        )
    {
        return Err(CliError::invalid_args(format!(
            "row {source_row}: target is required for {}",
            row.op
        )));
    }
    Ok(row)
}

fn shape_destination(
    deck: &str,
    slide: u32,
    target: &str,
    include_text: bool,
    include_bounds: bool,
    destination_file: &str,
) -> CliResult<Value> {
    let shapes = pptx_shapes_show(deck, slide, include_text, include_bounds)?;
    let shape = shapes
        .get("shapes")
        .and_then(Value::as_array)
        .and_then(|shapes| shapes.iter().find(|shape| shape_matches(shape, target)))
        .cloned()
        .ok_or_else(|| CliError::target_not_found(format!("target not found: {target}")))?;
    let mut out = Map::new();
    if !destination_file.is_empty() {
        out.insert("file".to_string(), json!(destination_file));
    }
    out.insert("slide".to_string(), json!(slide));
    out.insert("target".to_string(), json!(target));
    copy_field(&shape, &mut out, "shapeId");
    copy_field(&shape, &mut out, "shapeName");
    copy_field(&shape, &mut out, "targetKind");
    copy_field(&shape, &mut out, "primarySelector");
    copy_field(&shape, &mut out, "handle");
    copy_field(&shape, &mut out, "selectors");
    copy_field(&shape, &mut out, "textPreview");
    copy_field(&shape, &mut out, "bounds");
    copy_field(&shape, &mut out, "geometry");
    copy_field(&shape, &mut out, "imageRef");
    Ok(Value::Object(out))
}

fn table_destination(
    deck: &str,
    slide: u32,
    target: &str,
    destination_file: &str,
) -> CliResult<Value> {
    let tables = pptx_tables_show(deck, slide, 0, Some(target), true)?;
    let table = tables
        .get("tables")
        .and_then(Value::as_array)
        .and_then(|tables| tables.first())
        .cloned()
        .ok_or_else(|| CliError::target_not_found(format!("target not found: {target}")))?;
    let mut out = Map::new();
    if !destination_file.is_empty() {
        out.insert("file".to_string(), json!(destination_file));
    }
    out.insert("slide".to_string(), json!(slide));
    for field in [
        "shapeId",
        "shapeName",
        "targetKind",
        "primarySelector",
        "selectors",
        "rows",
        "cols",
        "cells",
        "bounds",
        "tableInfo",
    ] {
        copy_field(&table, &mut out, field);
    }
    Ok(Value::Object(out))
}

fn image_plan(workbook: &str, row: &BindingRow) -> CliResult<Value> {
    let image_path = row.image_path.trim();
    if image_path.is_empty() {
        return Err(row_error(row, "imagePath is required for image bindings"));
    }
    let resolved = if Path::new(image_path).is_absolute() {
        PathBuf::from(image_path)
    } else {
        Path::new(workbook)
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(image_path)
    };
    let bytes = fs::metadata(&resolved).map_err(|_| {
        CliError::file_not_found(format!("file not found: {}", resolved.to_string_lossy()))
    })?;
    let content_type = match resolved
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "svg" => "image/svg+xml",
        _ => "image/png",
    };
    Ok(json!({
        "path": image_path,
        "resolvedPath": resolved.to_string_lossy(),
        "contentType": content_type,
        "fitMode": if row.fit_mode.is_empty() { "contain" } else { row.fit_mode.as_str() },
        "bytes": bytes.len(),
    }))
}

fn equivalent_command(deck: &str, workbook: &str, row: &BindingRow) -> String {
    let source = if !row.source_table.is_empty() {
        format!(
            "--workbook {} --table {}",
            command_arg(workbook),
            command_arg(&row.source_table)
        )
    } else if !row.source_range.is_empty() {
        format!(
            "--workbook {} --sheet {} --range {}",
            command_arg(workbook),
            command_arg(&row.source_sheet),
            command_arg(&row.source_range)
        )
    } else {
        format!("--workbook {}", command_arg(workbook))
    };
    match row.op.as_str() {
        "replace-text" => {
            let mut args = vec![
                "ooxml".to_string(),
                "--json".to_string(),
                "pptx".to_string(),
                "replace".to_string(),
                "text-from-xlsx".to_string(),
                command_arg(deck),
                source,
                "--slide".to_string(),
                row.slide.to_string(),
                "--target".to_string(),
                command_arg(&row.target),
            ];
            if !row.mode.is_empty() {
                args.push("--mode".to_string());
                args.push(command_arg(&row.mode));
            }
            if !row.row_sep.is_empty() {
                args.push("--row-sep".to_string());
                args.push(command_arg(&row.row_sep));
            }
            if !row.col_sep.is_empty() {
                args.push("--col-sep".to_string());
                args.push(command_arg(&row.col_sep));
            }
            args.push("--out".to_string());
            args.push("<out.pptx>".to_string());
            args.join(" ")
        }
        "update-table" => format!(
            "ooxml --json pptx tables update-from-xlsx {} {} --slide {} --target {} --out <out.pptx>",
            command_arg(deck),
            source,
            row.slide,
            command_arg(&row.target)
        ),
        "place-table" => format!(
            "ooxml --json pptx place table-from-xlsx {} {} --slide {} --x {} --y {} --cx {} --cy {} --out <out.pptx>",
            command_arg(deck),
            source,
            row.slide,
            row.x,
            row.y,
            row.cx,
            row.cy
        ),
        "place-image" => format!(
            "ooxml --json pptx place image {} --slide {} --image {} --x {} --y {} --cx {} --cy {} --out <out.pptx>",
            command_arg(deck),
            row.slide,
            command_arg(&row.image_path),
            row.x,
            row.y,
            row.cx,
            row.cy
        ),
        "replace-image" => format!(
            "ooxml --json pptx replace images {} --slide {} --target {} --image {} --out <out.pptx>",
            command_arg(deck),
            row.slide,
            command_arg(&row.target),
            command_arg(&row.image_path)
        ),
        "set-bounds" => format!(
            "ooxml --json pptx shapes set-bounds {} --slide {} --target {} --bounds {},{},{},{} --out <out.pptx>",
            command_arg(deck),
            row.slide,
            command_arg(&row.target),
            row.x,
            row.y,
            row.cx,
            row.cy
        ),
        _ => String::new(),
    }
}

fn source_matrix(exported: &Value, formula_mode: &str) -> Vec<Vec<String>> {
    let values = exported
        .get("values")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let formulas = exported
        .get("formulas")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    values
        .iter()
        .enumerate()
        .map(|(row_index, row)| {
            row.as_array()
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .enumerate()
                .map(|(col_index, value)| {
                    if formula_mode == "formula"
                        && let Some(formula) = formulas
                            .get(row_index)
                            .and_then(Value::as_array)
                            .and_then(|row| row.get(col_index))
                            .and_then(Value::as_str)
                    {
                        return formula.to_string();
                    }
                    cell_to_string(&value)
                })
                .collect()
        })
        .collect()
}

fn cell_to_string(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(value) => value.clone(),
        Value::Number(value) => value.to_string(),
        Value::Bool(value) => value.to_string(),
        other => other.to_string(),
    }
}

fn normalize_header(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .replace(['_', '-', ' '], "")
}

fn normalize_op(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "replacetext" | "text" => "replace-text",
        "updatetable" | "table-update" => "update-table",
        "placetable" | "table-place" => "place-table",
        "placeimage" | "image-place" => "place-image",
        "replaceimage" | "image-replace" | "image" => "replace-image",
        "setbounds" | "set-shape-bounds" | "shapebounds" | "shape-bounds" | "bounds" => {
            "set-bounds"
        }
        other => other,
    }
    .to_string()
}

fn normalize_formula_mode(value: &str) -> CliResult<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "" | "value" | "values" => Ok("value".to_string()),
        "formula" | "formulas" => Ok("formula".to_string()),
        other => Err(CliError::invalid_args(format!(
            "invalid formulaMode {other:?}; expected value or formula"
        ))),
    }
}

fn normalize_text_mode(value: &str) -> CliResult<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "" | "preserve-format" | "preserve" => Ok("preserve-format".to_string()),
        "plain" | "plain-text" => Ok("plain".to_string()),
        other => Err(CliError::invalid_args(format!(
            "invalid mode {other:?}; expected preserve-format or plain"
        ))),
    }
}

fn decode_separator(value: &str) -> CliResult<String> {
    let mut out = String::new();
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('\\') => out.push('\\'),
            Some(other) => {
                return Err(CliError::invalid_args(format!(
                    "unsupported escape sequence \\{other}"
                )));
            }
            None => out.push('\\'),
        }
    }
    Ok(out)
}

fn join_matrix(values: &[Vec<String>], row_sep: &str, col_sep: &str) -> String {
    values
        .iter()
        .map(|row| row.join(col_sep))
        .collect::<Vec<_>>()
        .join(row_sep)
}

fn copy_field(source: &Value, dest: &mut Map<String, Value>, field: &str) {
    if let Some(value) = source.get(field)
        && !value.is_null()
    {
        dest.insert(field.to_string(), value.clone());
    }
}

fn source_rows(source: &Value) -> i64 {
    source
        .get("rows")
        .and_then(Value::as_i64)
        .unwrap_or_default()
}

fn source_cols(source: &Value) -> i64 {
    source
        .get("cols")
        .and_then(Value::as_i64)
        .unwrap_or_default()
}

fn check_expected_source_range(row: &BindingRow, source: &Value) -> CliResult<()> {
    if row.expect_source_range.trim().is_empty() {
        return Ok(());
    }
    let actual = source.get("range").and_then(Value::as_str).unwrap_or("");
    if actual != row.expect_source_range {
        return Err(row_error(
            row,
            format!(
                "source range mismatch: expected {}, got {actual}",
                row.expect_source_range
            ),
        ));
    }
    Ok(())
}

fn primary_selector(value: &Value) -> &str {
    value
        .get("primarySelector")
        .and_then(Value::as_str)
        .unwrap_or("")
}

fn duplicate_target_key(operation: &Value) -> Option<String> {
    let dest = operation.get("destination")?;
    let slide = dest.get("slide").and_then(Value::as_i64)?;
    let selector = dest.get("primarySelector").and_then(Value::as_str)?;
    Some(format!("slide:{slide}:{selector}"))
}

fn shape_matches(shape: &Value, target: &str) -> bool {
    shape.get("primarySelector").and_then(Value::as_str) == Some(target)
        || shape
            .get("selectors")
            .and_then(Value::as_array)
            .is_some_and(|selectors| {
                selectors
                    .iter()
                    .any(|selector| selector.as_str() == Some(target))
            })
        || target
            .strip_prefix("shape:")
            .and_then(|value| value.parse::<i64>().ok())
            == shape.get("shapeId").and_then(Value::as_i64)
}

fn column_value(
    values: &[String],
    columns: &std::collections::BTreeMap<String, usize>,
    name: &str,
) -> String {
    raw_column_value(values, columns, name).trim().to_string()
}

fn raw_column_value(
    values: &[String],
    columns: &std::collections::BTreeMap<String, usize>,
    name: &str,
) -> String {
    columns
        .get(&normalize_header(name))
        .and_then(|index| values.get(*index))
        .cloned()
        .unwrap_or_default()
}

fn first_column_value(
    values: &[String],
    columns: &std::collections::BTreeMap<String, usize>,
    names: &[&str],
) -> String {
    names
        .iter()
        .map(|name| column_value(values, columns, name))
        .find(|value| !value.is_empty())
        .unwrap_or_default()
}

fn first_raw_column_value(
    values: &[String],
    columns: &std::collections::BTreeMap<String, usize>,
    names: &[&str],
) -> String {
    names
        .iter()
        .map(|name| raw_column_value(values, columns, name))
        .find(|value| !value.is_empty())
        .unwrap_or_default()
}

fn parse_required_u32(
    values: &[String],
    columns: &std::collections::BTreeMap<String, usize>,
    name: &str,
    source_row: usize,
) -> CliResult<u32> {
    let value = column_value(values, columns, name);
    if value.is_empty() {
        return Err(CliError::invalid_args(format!(
            "row {source_row}: {name} is required"
        )));
    }
    let parsed = value.parse::<u32>().map_err(|_| {
        CliError::invalid_args(format!(
            "row {source_row}: {name} must be a positive integer"
        ))
    })?;
    if parsed == 0 {
        return Err(CliError::invalid_args(format!(
            "row {source_row}: {name} must be a positive integer"
        )));
    }
    Ok(parsed)
}

fn parse_optional_i64(
    values: &[String],
    columns: &std::collections::BTreeMap<String, usize>,
    name: &str,
    source_row: usize,
) -> CliResult<(i64, bool)> {
    let value = column_value(values, columns, name);
    if value.is_empty() {
        return Ok((0, false));
    }
    let parsed = value.parse::<i64>().map_err(|_| {
        CliError::invalid_args(format!("row {source_row}: {name} must be an integer"))
    })?;
    Ok((parsed, true))
}

fn parse_optional_bool(
    values: &[String],
    columns: &std::collections::BTreeMap<String, usize>,
    name: &str,
    source_row: usize,
) -> CliResult<bool> {
    match column_value(values, columns, name)
        .to_ascii_lowercase()
        .as_str()
    {
        "" => Ok(false),
        "1" | "true" | "yes" | "y" => Ok(true),
        "0" | "false" | "no" | "n" => Ok(false),
        _ => Err(CliError::invalid_args(format!(
            "row {source_row}: {name} must be true or false"
        ))),
    }
}

fn row_error(row: &BindingRow, message: impl Into<String>) -> CliError {
    CliError::invalid_args(format!("row {}: {}", row.source_row, message.into()))
}
