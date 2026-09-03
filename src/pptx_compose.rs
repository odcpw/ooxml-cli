use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::cli_dispatch::pptx_slots::{SlotBounds, body_bounds};
use crate::cli_dispatch::units::{inches, parse_length};
use crate::pptx_mutation::{
    pptx_add_textbox, pptx_charts_create, pptx_place_image, pptx_place_table,
};
use crate::{
    CliError, CliResult, command_arg, finish_mutation_output, has_flag, mutation_staging_path,
    package_type, parse_i64_flag, parse_string_flag, validate_owned_mutation_output,
    validate_xlsx_mutation_output_flags,
};

const MAX_COMPOSE_ITEMS: usize = 64;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ComposeItem {
    kind: String,
    #[serde(default = "default_grow")]
    grow: f64,
    #[serde(default)]
    aspect: Option<f64>,
    #[serde(default)]
    cell: Option<usize>,
    #[serde(flatten)]
    payload: BTreeMap<String, Value>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Arrangement {
    Row,
    Column,
    Grid { rows: usize, cols: usize },
}

#[derive(Clone, Debug)]
struct PlannedItem {
    index: usize,
    kind: String,
    grow: f64,
    aspect: Option<f64>,
    cell: Option<usize>,
    bounds: SlotBounds,
    command: &'static str,
    args: BTreeMap<String, Value>,
}

struct ComposeResultContext<'a> {
    file: &'a str,
    output: Option<&'a str>,
    slide: u32,
    body: SlotBounds,
    arrangement: Arrangement,
    gutter: i64,
    padding: i64,
    dry_run: bool,
    no_validate: bool,
}

fn default_grow() -> f64 {
    1.0
}

pub(crate) fn pptx_slides_compose(file: &str, args: &[String]) -> CliResult<Value> {
    if package_type(file)? != "pptx" {
        return Err(CliError::unsupported_type("file is not a PPTX document"));
    }
    let slide = parse_i64_flag(args, "--slide")?.unwrap_or(0);
    if slide < 1 {
        return Err(CliError::invalid_args("--slide must be >= 1"));
    }
    let items_path = parse_string_flag(args, "--items")?
        .filter(|path| !path.trim().is_empty())
        .ok_or_else(|| CliError::invalid_args("--items is required"))?;
    let arrangement = parse_arrangement(
        parse_string_flag(args, "--arrangement")?
            .as_deref()
            .unwrap_or("row"),
    )?;
    let body = body_bounds(file, slide as u32)?;
    let reference = body.cx.min(body.cy);
    let gutter = parse_spacing(args, "--gutter", reference)?.unwrap_or(0);
    let padding = parse_spacing(args, "--padding", reference)?.unwrap_or(0);
    let items = read_items(&items_path)?;
    let planned = plan_items(slide as u32, body, arrangement, gutter, padding, items)?;

    let out = parse_string_flag(args, "--out")?;
    let backup = parse_string_flag(args, "--backup")?;
    let dry_run = has_flag(args, "--dry-run");
    let in_place = has_flag(args, "--in-place");
    let no_validate = has_flag(args, "--no-validate");
    validate_xlsx_mutation_output_flags(out.as_deref(), in_place, backup.as_deref(), dry_run)?;

    if dry_run {
        validate_dry_run(file, &planned, no_validate)?;
    } else {
        execute_operation_batch(
            file,
            &planned,
            out.as_deref(),
            in_place,
            backup.as_deref(),
            no_validate,
        )?;
    }
    let output = if dry_run {
        None
    } else if in_place {
        Some(file)
    } else {
        out.as_deref()
    };
    Ok(compose_result(
        ComposeResultContext {
            file,
            output,
            slide: slide as u32,
            body,
            arrangement,
            gutter,
            padding,
            dry_run,
            no_validate,
        },
        &planned,
    ))
}

fn parse_spacing(args: &[String], flag: &str, reference: i64) -> CliResult<Option<i64>> {
    let Some(raw) = parse_string_flag(args, flag)? else {
        return Ok(None);
    };
    let value = parse_length(&raw, Some(reference))?;
    if value < 0 {
        return Err(CliError::invalid_args(format!(
            "{flag} must be non-negative"
        )));
    }
    Ok(Some(value))
}

fn read_items(path: &str) -> CliResult<Vec<ComposeItem>> {
    let data =
        fs::read(path).map_err(|_| CliError::file_not_found(format!("file not found: {path}")))?;
    let items: Vec<ComposeItem> = serde_json::from_slice(&data)
        .map_err(|err| CliError::invalid_args(format!("invalid --items JSON: {err}")))?;
    if items.is_empty() {
        return Err(CliError::invalid_args(
            "--items JSON must contain at least one item",
        ));
    }
    if items.len() > MAX_COMPOSE_ITEMS {
        return Err(CliError::invalid_args(format!(
            "--items JSON contains {} items; maximum is {MAX_COMPOSE_ITEMS}",
            items.len()
        )));
    }
    Ok(items)
}

fn parse_arrangement(raw: &str) -> CliResult<Arrangement> {
    let value = raw.trim().to_ascii_lowercase();
    match value.as_str() {
        "row" => Ok(Arrangement::Row),
        "column" => Ok(Arrangement::Column),
        _ if value.starts_with("grid:") => {
            let dimensions = value.trim_start_matches("grid:");
            let (rows, cols) = dimensions.split_once('x').ok_or_else(|| {
                CliError::invalid_args(
                    "invalid --arrangement; accepted values: row, column, grid:RxC",
                )
            })?;
            let rows = positive_grid_dimension(rows, "rows")?;
            let cols = positive_grid_dimension(cols, "columns")?;
            rows.checked_mul(cols)
                .filter(|cells| *cells <= MAX_COMPOSE_ITEMS)
                .ok_or_else(|| {
                    CliError::invalid_args(format!(
                        "grid may contain at most {MAX_COMPOSE_ITEMS} cells"
                    ))
                })?;
            Ok(Arrangement::Grid { rows, cols })
        }
        _ => Err(CliError::invalid_args(
            "invalid --arrangement; accepted values: row, column, grid:RxC",
        )),
    }
}

fn positive_grid_dimension(raw: &str, name: &str) -> CliResult<usize> {
    raw.parse::<usize>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| CliError::invalid_args(format!("grid {name} must be positive")))
}

fn plan_items(
    slide: u32,
    body: SlotBounds,
    arrangement: Arrangement,
    gutter: i64,
    padding: i64,
    items: Vec<ComposeItem>,
) -> CliResult<Vec<PlannedItem>> {
    validate_item_metrics(&items)?;
    let inner = inset_bounds(body, padding)?;
    let allocations = match arrangement {
        Arrangement::Row => {
            reject_cells(&items, "row")?;
            weighted_row(inner, gutter, &items)?
                .into_iter()
                .map(|bounds| (None, bounds))
                .collect()
        }
        Arrangement::Column => {
            reject_cells(&items, "column")?;
            weighted_column(inner, gutter, &items)?
                .into_iter()
                .map(|bounds| (None, bounds))
                .collect()
        }
        Arrangement::Grid { rows, cols } => sparse_grid(inner, gutter, rows, cols, &items)?,
    };
    items
        .into_iter()
        .zip(allocations)
        .enumerate()
        .map(|(index, (item, (cell, allocation)))| {
            let bounds = item
                .aspect
                .map(|aspect| fit_aspect(allocation, aspect))
                .unwrap_or(allocation);
            build_planned_item(index, slide, item, cell, bounds)
        })
        .collect()
}

fn validate_item_metrics(items: &[ComposeItem]) -> CliResult<()> {
    for (index, item) in items.iter().enumerate() {
        if !item.grow.is_finite() || item.grow <= 0.0 {
            return Err(CliError::invalid_args(format!(
                "item {index} grow must be a finite number greater than zero"
            )));
        }
        if item
            .aspect
            .is_some_and(|aspect| !aspect.is_finite() || aspect <= 0.0)
        {
            return Err(CliError::invalid_args(format!(
                "item {index} aspect must be a finite width/height ratio greater than zero"
            )));
        }
    }
    Ok(())
}

fn reject_cells(items: &[ComposeItem], arrangement: &str) -> CliResult<()> {
    if let Some(index) = items.iter().position(|item| item.cell.is_some()) {
        return Err(CliError::invalid_args(format!(
            "item {index} cell is only valid with grid:RxC, not {arrangement}"
        )));
    }
    Ok(())
}

fn inset_bounds(bounds: SlotBounds, padding: i64) -> CliResult<SlotBounds> {
    if padding.saturating_mul(2) >= bounds.cx || padding.saturating_mul(2) >= bounds.cy {
        return Err(CliError::invalid_args(
            "--padding must be smaller than half the body width and height",
        ));
    }
    Ok(SlotBounds {
        x: bounds.x + padding,
        y: bounds.y + padding,
        cx: bounds.cx - padding * 2,
        cy: bounds.cy - padding * 2,
    })
}

fn weighted_row(
    bounds: SlotBounds,
    gutter: i64,
    items: &[ComposeItem],
) -> CliResult<Vec<SlotBounds>> {
    let widths = weighted_lengths(bounds.cx, gutter, items)?;
    let mut x = bounds.x;
    Ok(widths
        .into_iter()
        .map(|cx| {
            let item = SlotBounds {
                x,
                y: bounds.y,
                cx,
                cy: bounds.cy,
            };
            x += cx + gutter;
            item
        })
        .collect())
}

fn weighted_column(
    bounds: SlotBounds,
    gutter: i64,
    items: &[ComposeItem],
) -> CliResult<Vec<SlotBounds>> {
    let heights = weighted_lengths(bounds.cy, gutter, items)?;
    let mut y = bounds.y;
    Ok(heights
        .into_iter()
        .map(|cy| {
            let item = SlotBounds {
                x: bounds.x,
                y,
                cx: bounds.cx,
                cy,
            };
            y += cy + gutter;
            item
        })
        .collect())
}

fn weighted_lengths(total: i64, gutter: i64, items: &[ComposeItem]) -> CliResult<Vec<i64>> {
    let gaps = i64::try_from(items.len().saturating_sub(1)).unwrap_or(i64::MAX);
    let available = total.saturating_sub(gutter.saturating_mul(gaps));
    if available < i64::try_from(items.len()).unwrap_or(i64::MAX) {
        return Err(CliError::invalid_args(
            "--gutter leaves no positive space for every compose item",
        ));
    }
    let weight_sum = items.iter().map(|item| item.grow).sum::<f64>();
    let mut previous = 0_i64;
    let mut cumulative = 0.0;
    let mut lengths = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        cumulative += item.grow;
        let edge = if index + 1 == items.len() {
            available
        } else {
            (available as f64 * cumulative / weight_sum).round() as i64
        };
        lengths.push(edge - previous);
        previous = edge;
    }
    if lengths.iter().any(|length| *length <= 0) {
        return Err(CliError::invalid_args(
            "grow weights and gutter must leave positive space for every compose item",
        ));
    }
    Ok(lengths)
}

fn sparse_grid(
    bounds: SlotBounds,
    gutter: i64,
    rows: usize,
    cols: usize,
    items: &[ComposeItem],
) -> CliResult<Vec<(Option<usize>, SlotBounds)>> {
    let cells = rows * cols;
    if items.len() > cells {
        return Err(CliError::invalid_args(format!(
            "grid:{rows}x{cols} has {cells} cells but {} items were supplied",
            items.len()
        )));
    }
    let col_widths = equal_lengths(bounds.cx, gutter, cols)?;
    let row_heights = equal_lengths(bounds.cy, gutter, rows)?;
    let mut used = BTreeSet::new();
    let mut next = 1;
    let mut out = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let cell = if let Some(cell) = item.cell {
            if cell == 0 || cell > cells {
                return Err(CliError::invalid_args(format!(
                    "item {index} grid cell must be 1..={cells}"
                )));
            }
            cell
        } else {
            while used.contains(&next) {
                next += 1;
            }
            next
        };
        if !used.insert(cell) {
            return Err(CliError::invalid_args(format!(
                "item {index} duplicates grid cell {cell}"
            )));
        }
        let zero = cell - 1;
        let row = zero / cols;
        let col = zero % cols;
        let x = bounds.x
            + col_widths.iter().take(col).sum::<i64>()
            + gutter * i64::try_from(col).unwrap_or(i64::MAX);
        let y = bounds.y
            + row_heights.iter().take(row).sum::<i64>()
            + gutter * i64::try_from(row).unwrap_or(i64::MAX);
        out.push((
            Some(cell),
            SlotBounds {
                x,
                y,
                cx: col_widths[col],
                cy: row_heights[row],
            },
        ));
    }
    Ok(out)
}

fn equal_lengths(total: i64, gutter: i64, count: usize) -> CliResult<Vec<i64>> {
    let items = (0..count)
        .map(|_| ComposeItem {
            kind: String::new(),
            grow: 1.0,
            aspect: None,
            cell: None,
            payload: BTreeMap::new(),
        })
        .collect::<Vec<_>>();
    weighted_lengths(total, gutter, &items)
}

fn fit_aspect(bounds: SlotBounds, aspect: f64) -> SlotBounds {
    let current = bounds.cx as f64 / bounds.cy as f64;
    if current > aspect {
        let cx = (bounds.cy as f64 * aspect).round() as i64;
        SlotBounds {
            x: bounds.x + (bounds.cx - cx) / 2,
            cx,
            ..bounds
        }
    } else {
        let cy = (bounds.cx as f64 / aspect).round() as i64;
        SlotBounds {
            y: bounds.y + (bounds.cy - cy) / 2,
            cy,
            ..bounds
        }
    }
}

fn build_planned_item(
    index: usize,
    slide: u32,
    item: ComposeItem,
    cell: Option<usize>,
    bounds: SlotBounds,
) -> CliResult<PlannedItem> {
    let kind = item.kind.trim().to_ascii_lowercase();
    let (command, fields): (&str, &[(&str, &str)]) = match kind.as_str() {
        "text" => (
            "pptx add-textbox",
            &[
                ("text", "text"),
                ("paragraphsFile", "paragraphs-file"),
                ("name", "name"),
                ("fontSize", "font-size"),
                ("font", "font"),
                ("color", "color"),
                ("level", "level"),
                ("align", "align"),
                ("bold", "bold"),
                ("italic", "italic"),
            ],
        ),
        "image" => (
            "pptx place image",
            &[
                ("image", "image"),
                ("name", "name"),
                ("fitMode", "fit-mode"),
            ],
        ),
        "chart" => (
            "pptx charts create",
            &[
                ("type", "type"),
                ("title", "title"),
                ("valuesJson", "values-json"),
                ("valuesFile", "values-file"),
                ("sourceFile", "source-file"),
                ("sourceSheet", "source-sheet"),
                ("sourceRange", "source-range"),
                ("expectSourceRange", "expect-source-range"),
                ("maxCells", "max-cells"),
                ("embedWorkbook", "embed-workbook"),
            ],
        ),
        "table" => (
            "pptx place table",
            &[
                ("data", "data"),
                ("format", "format"),
                ("name", "name"),
                ("header", "header"),
                ("bandedRows", "banded-rows"),
                ("headerColor", "header-color"),
                ("band1Color", "band1-color"),
                ("band2Color", "band2-color"),
                ("fontSize", "font-size"),
                ("borderColor", "border-color"),
                ("borderWidth", "border-width"),
            ],
        ),
        _ => {
            return Err(CliError::invalid_args(format!(
                "item {index} has unknown kind {:?}; accepted kinds: text, image, chart, table",
                item.kind
            )));
        }
    };
    validate_payload(index, &kind, &item.payload, fields)?;
    let mut args = BTreeMap::new();
    args.insert("slide".to_string(), json!(slide));
    args.insert("x".to_string(), json!(bounds.x));
    args.insert("y".to_string(), json!(bounds.y));
    args.insert("cx".to_string(), json!(bounds.cx));
    args.insert("cy".to_string(), json!(bounds.cy));
    for (json_name, flag_name) in fields {
        if let Some(value) = item.payload.get(*json_name) {
            args.insert((*flag_name).to_string(), value.clone());
        }
    }
    Ok(PlannedItem {
        index,
        kind,
        grow: item.grow,
        aspect: item.aspect,
        cell,
        bounds,
        command,
        args,
    })
}

fn validate_payload(
    index: usize,
    kind: &str,
    payload: &BTreeMap<String, Value>,
    fields: &[(&str, &str)],
) -> CliResult<()> {
    let accepted = fields
        .iter()
        .map(|(name, _)| *name)
        .collect::<BTreeSet<_>>();
    if let Some(name) = payload
        .keys()
        .find(|name| !accepted.contains(name.as_str()))
    {
        return Err(CliError::invalid_args(format!(
            "item {index} {kind} has unknown field {name:?}; accepted fields: {}",
            accepted.into_iter().collect::<Vec<_>>().join(", ")
        )));
    }
    match kind {
        "text" => require_exactly_one(index, payload, &["text", "paragraphsFile"]),
        "image" => {
            require_nonempty_string(index, payload, "image")?;
            require_local_file(index, payload, "image")
        }
        "chart" => {
            require_nonempty_string(index, payload, "type")?;
            require_exactly_one(index, payload, &["valuesJson", "valuesFile", "sourceFile"])?;
            if payload.contains_key("sourceFile") && !payload.contains_key("sourceRange") {
                return Err(CliError::invalid_args(format!(
                    "item {index} chart sourceFile requires sourceRange"
                )));
            }
            Ok(())
        }
        "table" => {
            require_nonempty_string(index, payload, "data")?;
            require_local_file(index, payload, "data")
        }
        _ => unreachable!("kind validated before payload"),
    }
}

fn require_exactly_one(
    index: usize,
    payload: &BTreeMap<String, Value>,
    names: &[&str],
) -> CliResult<()> {
    let present = names
        .iter()
        .filter(|name| payload.contains_key(**name))
        .count();
    if present != 1 {
        return Err(CliError::invalid_args(format!(
            "item {index} requires exactly one of {}",
            names.join(", ")
        )));
    }
    let name = names
        .iter()
        .find(|name| payload.contains_key(**name))
        .expect("one payload field is present");
    require_nonempty_string(index, payload, name)
}

fn require_nonempty_string(
    index: usize,
    payload: &BTreeMap<String, Value>,
    name: &str,
) -> CliResult<()> {
    if payload
        .get(name)
        .and_then(Value::as_str)
        .is_some_and(|value| !value.trim().is_empty())
    {
        return Ok(());
    }
    Err(CliError::invalid_args(format!(
        "item {index} field {name} must be a non-empty string"
    )))
}

fn require_local_file(
    index: usize,
    payload: &BTreeMap<String, Value>,
    name: &str,
) -> CliResult<()> {
    let path = payload
        .get(name)
        .and_then(Value::as_str)
        .expect("non-empty string validated before file");
    if Path::new(path).is_file() {
        Ok(())
    } else {
        Err(CliError::file_not_found(format!(
            "item {index} file not found: {path}"
        )))
    }
}

fn execute_operation_batch(
    file: &str,
    planned: &[PlannedItem],
    out: Option<&str>,
    in_place: bool,
    backup: Option<&str>,
    no_validate: bool,
) -> CliResult<()> {
    let mut current = file.to_string();
    let mut owned_current = false;
    for item in planned {
        let next = mutation_staging_path(file, out, &format!("pptx-compose-{}", item.index));
        let mut args = operation_args(item)?;
        args.extend([
            "--out".to_string(),
            next.clone(),
            "--no-validate".to_string(),
        ]);
        let result = run_operation(&current, item, args);
        if let Err(err) = result {
            let _ = fs::remove_file(&next);
            if owned_current {
                let _ = fs::remove_file(&current);
            }
            return Err(err);
        }
        if owned_current {
            let _ = fs::remove_file(&current);
        }
        current = next;
        owned_current = true;
    }
    if !no_validate && let Err(err) = validate_owned_mutation_output(&current) {
        let _ = fs::remove_file(&current);
        return Err(err);
    }
    if let Err(err) = finish_mutation_output(file, &current, out, in_place, backup, false) {
        let _ = fs::remove_file(&current);
        return Err(err);
    }
    Ok(())
}

fn validate_dry_run(file: &str, planned: &[PlannedItem], no_validate: bool) -> CliResult<()> {
    for item in planned {
        let mut args = operation_args(item)?;
        args.push("--dry-run".to_string());
        if no_validate {
            args.push("--no-validate".to_string());
        }
        run_operation(file, item, args)?;
    }
    Ok(())
}

fn run_operation(file: &str, item: &PlannedItem, args: Vec<String>) -> CliResult<Value> {
    match item.command {
        "pptx add-textbox" => pptx_add_textbox(file, &args),
        "pptx place image" => pptx_place_image(file, &args),
        "pptx charts create" => pptx_charts_create(file, &args),
        "pptx place table" => pptx_place_table(file, &args),
        _ => unreachable!("planned commands are exhaustive"),
    }
}

fn operation_args(item: &PlannedItem) -> CliResult<Vec<String>> {
    let mut args = Vec::new();
    for (name, value) in &item.args {
        let flag = format!("--{name}");
        match value {
            Value::Bool(true) => args.push(flag),
            Value::Bool(false) => {}
            Value::String(value) => args.extend([flag, value.clone()]),
            Value::Number(value) => args.extend([flag, value.to_string()]),
            _ => {
                return Err(CliError::invalid_args(format!(
                    "item {} field {name} must be a string, number, or boolean",
                    item.index
                )));
            }
        }
    }
    Ok(args)
}

fn compose_result(context: ComposeResultContext<'_>, items: &[PlannedItem]) -> Value {
    let ComposeResultContext {
        file,
        output,
        slide,
        body,
        arrangement,
        gutter,
        padding,
        dry_run,
        no_validate,
    } = context;
    let target = output.unwrap_or("<out.pptx>");
    json!({
        "schemaVersion": 1,
        "action": "pptx.slides.compose",
        "file": file,
        "output": output,
        "dryRun": dry_run,
        "slide": slide,
        "arrangement": arrangement_name(arrangement),
        "gutter": length_json(gutter),
        "padding": length_json(padding),
        "bodyBounds": bounds_json(body),
        "itemCount": items.len(),
        "opsCount": items.len(),
        "batch": {
            "atomic": true,
            "validation": if dry_run {
                "dry-run"
            } else if no_validate {
                "skipped"
            } else {
                "strict-once-before-publish"
            },
        },
        "items": items.iter().map(planned_item_json).collect::<Vec<_>>(),
        "operations": items.iter().map(operation_json).collect::<Vec<_>>(),
        "readbackCommand": (!dry_run).then(|| format!(
            "ooxml --json pptx shapes show {} --slide {slide} --include-text --include-bounds",
            command_arg(target),
        )),
        "validateCommand": (!dry_run).then(|| format!(
            "ooxml --json validate --strict {}",
            command_arg(target),
        )),
        "layoutCheckCommand": (!dry_run).then(|| format!(
            "ooxml --json pptx validate-layout {}",
            command_arg(target),
        )),
        "renderCommand": (!dry_run).then(|| format!(
            "ooxml --json pptx render {} --out render",
            command_arg(target),
        )),
    })
}

fn arrangement_name(arrangement: Arrangement) -> String {
    match arrangement {
        Arrangement::Row => "row".to_string(),
        Arrangement::Column => "column".to_string(),
        Arrangement::Grid { rows, cols } => format!("grid:{rows}x{cols}"),
    }
}

fn planned_item_json(item: &PlannedItem) -> Value {
    json!({
        "index": item.index,
        "kind": item.kind,
        "grow": item.grow,
        "aspect": item.aspect,
        "cell": item.cell,
        "bounds": bounds_json(item.bounds),
        "operation": operation_json(item),
    })
}

fn operation_json(item: &PlannedItem) -> Value {
    json!({
        "command": item.command,
        "args": Value::Object(item.args.clone().into_iter().collect::<Map<_, _>>()),
    })
}

fn bounds_json(bounds: SlotBounds) -> Value {
    json!({
        "x": bounds.x,
        "y": bounds.y,
        "cx": bounds.cx,
        "cy": bounds.cy,
        "inches": {
            "x": inches(bounds.x),
            "y": inches(bounds.y),
            "cx": inches(bounds.cx),
            "cy": inches(bounds.cy),
        },
    })
}

fn length_json(value: i64) -> Value {
    json!({"emu": value, "inches": inches(value)})
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn item(grow: f64, aspect: Option<f64>, cell: Option<usize>) -> ComposeItem {
        ComposeItem {
            kind: "text".to_string(),
            grow,
            aspect,
            cell,
            payload: BTreeMap::from([("text".to_string(), json!("x"))]),
        }
    }

    fn body() -> SlotBounds {
        SlotBounds {
            x: 100,
            y: 200,
            cx: 1_000,
            cy: 800,
        }
    }

    fn overlaps(a: SlotBounds, b: SlotBounds) -> bool {
        a.x < b.x + b.cx && b.x < a.x + a.cx && a.y < b.y + b.cy && b.y < a.y + a.cy
    }

    fn assert_plan_is_inside_and_non_overlapping(body: SlotBounds, planned: &[PlannedItem]) {
        for item in planned {
            let bounds = item.bounds;
            assert!(bounds.cx > 0 && bounds.cy > 0, "{bounds:?}");
            assert!(bounds.x >= body.x && bounds.y >= body.y, "{bounds:?}");
            assert!(bounds.x + bounds.cx <= body.x + body.cx, "{bounds:?}");
            assert!(bounds.y + bounds.cy <= body.y + body.cy, "{bounds:?}");
        }
        for left in 0..planned.len() {
            for right in left + 1..planned.len() {
                assert!(
                    !overlaps(planned[left].bounds, planned[right].bounds),
                    "left={:?} right={:?}",
                    planned[left].bounds,
                    planned[right].bounds
                );
            }
        }
    }

    fn temp_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ooxml-pptx-compose-{label}-{}-{}",
            std::process::id(),
            crate::chrono_like_counter()
        ));
        fs::create_dir_all(&dir).expect("create compose test directory");
        dir
    }

    #[test]
    fn weighted_rows_and_columns_consume_the_inner_area_exactly() {
        let items = vec![item(2.0, None, None), item(3.0, None, None)];
        let row = weighted_row(body(), 10, &items).expect("row");
        assert_eq!(row[0].cx + row[1].cx + 10, body().cx);
        assert_eq!(row[0].cx, 396);
        assert_eq!(row[1].cx, 594);
        assert!(!overlaps(row[0], row[1]));

        let column = weighted_column(body(), 20, &items).expect("column");
        assert_eq!(column[0].cy + column[1].cy + 20, body().cy);
        assert_eq!(column[0].cy, 312);
        assert_eq!(column[1].cy, 468);
        assert!(!overlaps(column[0], column[1]));
    }

    #[test]
    fn sparse_grid_preserves_item_order_and_requested_hole() {
        let items = vec![
            item(1.0, None, Some(1)),
            item(1.0, None, Some(3)),
            item(1.0, None, Some(4)),
        ];
        let grid = sparse_grid(body(), 20, 2, 2, &items).expect("grid");
        assert_eq!(
            grid.iter().map(|(cell, _)| *cell).collect::<Vec<_>>(),
            [Some(1), Some(3), Some(4)]
        );
        for left in 0..grid.len() {
            for right in left + 1..grid.len() {
                assert!(!overlaps(grid[left].1, grid[right].1));
            }
        }
    }

    #[test]
    fn aspect_fitting_is_centered_and_never_expands() {
        let wide = fit_aspect(body(), 2.0);
        assert_eq!(wide.cx, 1_000);
        assert_eq!(wide.cy, 500);
        assert_eq!(wide.y, 350);
        let tall = fit_aspect(body(), 0.5);
        assert_eq!(tall.cx, 400);
        assert_eq!(tall.cy, 800);
        assert_eq!(tall.x, 400);
    }

    #[test]
    fn three_image_aspects_stay_ordered_inside_equal_row_tracks() {
        let image = "testdata/test_image.png";
        let items = [0.5, 1.0, 2.0]
            .into_iter()
            .map(|aspect| ComposeItem {
                kind: "image".to_string(),
                grow: 1.0,
                aspect: Some(aspect),
                cell: None,
                payload: BTreeMap::from([("image".to_string(), json!(image))]),
            })
            .collect::<Vec<_>>();
        let planned =
            plan_items(1, body(), Arrangement::Row, 10, 20, items).expect("three-image row plan");
        assert_eq!(planned.len(), 3);
        assert!(
            planned
                .iter()
                .all(|item| item.command == "pptx place image")
        );
        assert!(
            planned
                .windows(2)
                .all(|pair| pair[0].bounds.x < pair[1].bounds.x)
        );
        for (index, item) in planned.iter().enumerate() {
            assert!(item.bounds.x >= body().x + 20, "item {index}");
            assert!(item.bounds.y >= body().y + 20, "item {index}");
            assert!(
                item.bounds.x + item.bounds.cx <= body().x + body().cx - 20,
                "item {index}"
            );
            assert!(
                item.bounds.y + item.bounds.cy <= body().y + body().cy - 20,
                "item {index}"
            );
        }
        for left in 0..planned.len() {
            for right in left + 1..planned.len() {
                assert!(!overlaps(planned[left].bounds, planned[right].bounds));
            }
        }
    }

    #[test]
    fn layout_property_keeps_varied_flex_and_grid_items_disjoint() {
        for count in 1..=16 {
            let varied = (0..count)
                .map(|index| {
                    item(
                        ((index * 7 + count) % 9 + 1) as f64,
                        (index % 3 != 0).then_some([0.5, 1.0, 16.0 / 9.0][index % 3]),
                        None,
                    )
                })
                .collect::<Vec<_>>();
            let bounds = SlotBounds {
                x: 101 + count as i64,
                y: 203,
                cx: 50_000 + count as i64 * 137,
                cy: 30_000 + count as i64 * 83,
            };
            let gutter = 7 + count as i64;
            for arrangement in [Arrangement::Row, Arrangement::Column] {
                let planned = plan_items(1, bounds, arrangement, gutter, 13, varied.clone())
                    .expect("flex property plan");
                assert_plan_is_inside_and_non_overlapping(bounds, &planned);
            }
        }

        for rows in 1..=4 {
            for cols in 1..=4 {
                let count = rows * cols;
                let varied = (0..count)
                    .map(|index| item(1.0, (index % 2 == 0).then_some(4.0 / 3.0), None))
                    .collect::<Vec<_>>();
                let planned = plan_items(1, body(), Arrangement::Grid { rows, cols }, 9, 7, varied)
                    .expect("grid property plan");
                assert_plan_is_inside_and_non_overlapping(body(), &planned);
            }
        }
    }

    #[test]
    fn arrangement_and_grid_errors_name_the_valid_contract() {
        assert_eq!(
            parse_arrangement("GRID:2x3").expect("grid"),
            Arrangement::Grid { rows: 2, cols: 3 }
        );
        let error = parse_arrangement("masonry").expect_err("invalid arrangement");
        assert!(error.message.contains("row, column, grid:RxC"));
        let error =
            sparse_grid(body(), 0, 1, 1, &[item(1.0, None, Some(2))]).expect_err("invalid cell");
        assert!(error.message.contains("1..=1"));
    }

    #[test]
    fn text_and_chart_compose_is_atomic_valid_and_deterministic() {
        let dir = temp_dir("text-chart");
        let items_path = dir.join("items.json");
        fs::write(
            &items_path,
            serde_json::to_vec_pretty(&json!([
                {
                    "kind": "text",
                    "text": "Revenue\n- Stable delivery",
                    "grow": 2,
                    "fontSize": 20
                },
                {
                    "kind": "chart",
                    "type": "bar",
                    "title": "Revenue",
                    "valuesJson": "[[\"Quarter\",\"Revenue\"],[\"Q1\",12],[\"Q2\",18]]",
                    "grow": 3
                }
            ]))
            .expect("serialize items"),
        )
        .expect("write items");
        let fixture = "testdata/pptx/scaffold/eleven-layouts.pptx";
        let first = dir.join("first.pptx");
        let second = dir.join("second.pptx");
        for output in [&first, &second] {
            let result = pptx_slides_compose(
                fixture,
                &[
                    "--slide".to_string(),
                    "7".to_string(),
                    "--items".to_string(),
                    items_path.to_string_lossy().to_string(),
                    "--arrangement".to_string(),
                    "row".to_string(),
                    "--gutter".to_string(),
                    "0.2in".to_string(),
                    "--padding".to_string(),
                    "0.1in".to_string(),
                    "--out".to_string(),
                    output.to_string_lossy().to_string(),
                ],
            )
            .expect("compose text and chart");
            assert_eq!(result["itemCount"], 2);
            assert_eq!(result["operations"][0]["command"], "pptx add-textbox");
            assert_eq!(result["operations"][1]["command"], "pptx charts create");
            crate::validate_owned_mutation_output(output.to_str().expect("output path"))
                .expect("strict validation");
            let qa = crate::pptx_validate_layout(output.to_str().expect("output path"))
                .expect("layout QA");
            assert_eq!(qa["totalCollisions"], 0, "{qa}");
            assert_eq!(qa["totalOffSlide"], 0, "{qa}");
            assert_eq!(qa["totalSafeMarginViolations"], 0, "{qa}");
        }
        assert_eq!(
            fs::read(&first).expect("read first output"),
            fs::read(&second).expect("read second output")
        );
        fs::remove_dir_all(dir).expect("remove compose test directory");
    }
}
