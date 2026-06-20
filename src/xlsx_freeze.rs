use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::{
    CliError, CliResult, WorkbookSheet, col_name, command_arg, copy_zip_with_part_override,
    local_name, normalize_xl_target, relationships, remove_xml_span, render_xml_attrs,
    replace_xml_span, resolve_sheet, validate, validate_xlsx_mutation_output_flags,
    workbook_sheets, xlsx_ranges_set_temp_path, xml_attrs, xml_direct_child_ranges,
    xml_fragment_bounds, xml_open_tag_from_start, xml_tag_prefix, zip_text,
};

const XLSX_MAX_ROW: i64 = 1_048_576;
const XLSX_MAX_COL: i64 = 16_384;

#[derive(Clone)]
struct WorksheetRootBounds {
    start: usize,
    open_end: usize,
    close_start: usize,
    end: usize,
    tag_name: String,
    self_closing: bool,
}

#[derive(Clone)]
struct XlsxFreezeState {
    rows: i64,
    cols: i64,
    top_left_cell: String,
    frozen: bool,
}

type FreezeMutationResult = (String, Option<XlsxFreezeState>);
type FreezeMutationApply =
    fn(&str, &XlsxFreezeMutationOptions<'_>) -> CliResult<FreezeMutationResult>;

pub(crate) struct XlsxFreezeMutationOptions<'a> {
    pub(crate) sheet: Option<&'a str>,
    pub(crate) rows: i64,
    pub(crate) cols: i64,
    pub(crate) expect_state: Option<&'a str>,
    pub(crate) expect_state_present: bool,
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
}

pub(crate) fn xlsx_freeze_show(file: &str, sheet_selector: Option<&str>) -> CliResult<Value> {
    let (sheet, sheet_part) = resolve_freeze_sheet(file, sheet_selector)?;
    let sheet_xml = zip_text(file, &sheet_part)?;
    let state = read_freeze_state(&sheet_xml)?;
    let selector = freeze_sheet_selector(&sheet);
    Ok(json!({
        "file": file,
        "sheet": sheet.name,
        "sheetNumber": sheet.position,
        "state": freeze_state_json(state.as_ref()),
        "setCommand": format!("ooxml xlsx freeze set {} --sheet {} --rows 1 --cols 1 --in-place", command_arg(file), command_arg(&selector)),
        "clearCommand": format!("ooxml xlsx freeze clear {} --sheet {} --in-place", command_arg(file), command_arg(&selector)),
        "showCommand": freeze_show_command(file, &sheet),
    }))
}

pub(crate) fn xlsx_freeze_set(
    file: &str,
    options: XlsxFreezeMutationOptions<'_>,
) -> CliResult<Value> {
    if options.rows < 0 || options.cols < 0 {
        return Err(map_freeze_error(
            "set",
            "--rows and --cols must be >= 0".to_string(),
        ));
    }
    if options.rows == 0 && options.cols == 0 {
        return Err(map_freeze_error(
            "set",
            "provide at least one of --rows or --cols (>= 1)".to_string(),
        ));
    }
    if options.rows > XLSX_MAX_ROW - 1 {
        return Err(map_freeze_error(
            "set",
            format!(
                "--rows {} exceeds the maximum freezable rows ({})",
                options.rows,
                XLSX_MAX_ROW - 1
            ),
        ));
    }
    if options.cols > XLSX_MAX_COL - 1 {
        return Err(map_freeze_error(
            "set",
            format!(
                "--cols {} exceeds the maximum freezable columns ({})",
                options.cols,
                XLSX_MAX_COL - 1
            ),
        ));
    }
    run_freeze_mutation(file, "set", options, |xml, options| {
        guard_expect_state(xml, options.expect_state_present, options.expect_state)?;
        apply_freeze(xml, options.rows, options.cols)
    })
}

pub(crate) fn xlsx_freeze_clear(
    file: &str,
    options: XlsxFreezeMutationOptions<'_>,
) -> CliResult<Value> {
    run_freeze_mutation(file, "clear", options, |xml, options| {
        guard_expect_state(xml, options.expect_state_present, options.expect_state)?;
        clear_freeze(xml)
    })
}

fn run_freeze_mutation(
    file: &str,
    action: &str,
    options: XlsxFreezeMutationOptions<'_>,
    apply: FreezeMutationApply,
) -> CliResult<Value> {
    if !Path::new(file).exists() {
        return Err(CliError::file_not_found(format!("file not found: {file}")));
    }
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;
    let (sheet, sheet_part) = resolve_freeze_sheet(file, options.sheet)?;
    let sheet_xml = zip_text(file, &sheet_part)?;
    let (updated_xml, state) =
        apply(&sheet_xml, &options).map_err(|err| map_freeze_error(action, err.message))?;

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

    copy_zip_with_part_override(file, &readback_path, &sheet_part, &updated_xml)?;
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
    result.insert("sheet".to_string(), json!(sheet.name));
    result.insert("sheetNumber".to_string(), json!(sheet.position));
    result.insert("action".to_string(), json!(action));
    result.insert("state".to_string(), freeze_state_json(state.as_ref()));
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
        result.insert(
            "showCommand".to_string(),
            json!(freeze_show_command(commit_path, &sheet)),
        );
    }
    Ok(Value::Object(result))
}

fn resolve_freeze_sheet(
    file: &str,
    sheet_selector: Option<&str>,
) -> CliResult<(WorkbookSheet, String)> {
    let workbook = zip_text(file, "xl/workbook.xml")?;
    let sheets = workbook_sheets(&workbook)?;
    if sheets.is_empty() {
        return Err(CliError::invalid_args("workbook has no sheets"));
    }
    let selector = sheet_selector.unwrap_or("").trim();
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
    Ok((sheet, sheet_part))
}

fn read_freeze_state(xml: &str) -> CliResult<Option<XlsxFreezeState>> {
    let Some(pane) = find_freeze_pane_range(xml)? else {
        return Ok(None);
    };
    let attrs = first_element_attrs(&xml[pane.start..pane.end])?;
    if attrs.get("state").map(String::as_str) != Some("frozen") {
        return Ok(None);
    }
    Ok(Some(freeze_state_from_attrs(&attrs)))
}

fn freeze_state_from_attrs(attrs: &BTreeMap<String, String>) -> XlsxFreezeState {
    let cols = attrs
        .get("xSplit")
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(0);
    let rows = attrs
        .get("ySplit")
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(0);
    let top_left_cell = attrs
        .get("topLeftCell")
        .filter(|value| !value.trim().is_empty())
        .cloned()
        .unwrap_or_else(|| freeze_top_left_cell(rows, cols));
    XlsxFreezeState {
        rows,
        cols,
        top_left_cell,
        frozen: true,
    }
}

fn apply_freeze(xml: &str, rows: i64, cols: i64) -> CliResult<(String, Option<XlsxFreezeState>)> {
    let state = XlsxFreezeState {
        rows,
        cols,
        top_left_cell: freeze_top_left_cell(rows, cols),
        frozen: true,
    };
    let root = worksheet_root_bounds(xml)?;
    let prefix = xml_tag_prefix(&root.tag_name);
    let pane_xml = render_freeze_pane(&prefix, &state);
    Ok((
        set_freeze_pane(xml, &root, &prefix, &pane_xml)?,
        Some(state),
    ))
}

fn clear_freeze(xml: &str) -> CliResult<(String, Option<XlsxFreezeState>)> {
    let Some(pane) = find_freeze_pane_range(xml)? else {
        return Err(CliError::invalid_args("worksheet has no frozen pane"));
    };
    let attrs = first_element_attrs(&xml[pane.start..pane.end])?;
    if attrs.get("state").map(String::as_str) != Some("frozen") {
        return Err(CliError::invalid_args("worksheet has no frozen pane"));
    }
    Ok((remove_xml_span(xml, pane.start, pane.end), None))
}

fn set_freeze_pane(
    xml: &str,
    root: &WorksheetRootBounds,
    prefix: &str,
    pane_xml: &str,
) -> CliResult<String> {
    let Some(sheet_views) = direct_child_range(xml, root.open_end, root.close_start, "sheetViews")?
    else {
        let sheet_views_xml = format!(
            "<{0}><{1} workbookViewId=\"0\">{2}</{1}></{0}>",
            element_name(prefix, "sheetViews"),
            element_name(prefix, "sheetView"),
            pane_xml
        );
        return insert_worksheet_child(xml, root, "sheetViews", &sheet_views_xml);
    };

    let sheet_views_fragment = &xml[sheet_views.start..sheet_views.end];
    let updated_sheet_views =
        set_freeze_pane_in_sheet_views(sheet_views_fragment, prefix, pane_xml)?;
    Ok(replace_xml_span(
        xml,
        sheet_views.start,
        sheet_views.end,
        &updated_sheet_views,
    ))
}

fn set_freeze_pane_in_sheet_views(
    fragment: &str,
    prefix: &str,
    pane_xml: &str,
) -> CliResult<String> {
    let (open_end, tag_name, close_start, self_closing) = xml_fragment_bounds(fragment)?;
    if self_closing {
        let start_tag = xml_open_tag_from_start(&fragment[..=open_end]);
        return Ok(format!(
            "{start_tag}<{} workbookViewId=\"0\">{pane_xml}</{}></{tag_name}>",
            element_name(prefix, "sheetView"),
            element_name(prefix, "sheetView"),
        ));
    }
    let Some(sheet_view) = direct_child_range(fragment, open_end + 1, close_start, "sheetView")?
    else {
        let mut updated = String::new();
        updated.push_str(&fragment[..close_start]);
        updated.push_str(&format!(
            "<{} workbookViewId=\"0\">{pane_xml}</{}>",
            element_name(prefix, "sheetView"),
            element_name(prefix, "sheetView")
        ));
        updated.push_str(&fragment[close_start..]);
        return Ok(updated);
    };

    let sheet_view_fragment = &fragment[sheet_view.start..sheet_view.end];
    let updated_sheet_view = set_freeze_pane_in_sheet_view(sheet_view_fragment, pane_xml)?;
    Ok(replace_xml_span(
        fragment,
        sheet_view.start,
        sheet_view.end,
        &updated_sheet_view,
    ))
}

fn set_freeze_pane_in_sheet_view(fragment: &str, pane_xml: &str) -> CliResult<String> {
    let fragment = ensure_workbook_view_id(fragment)?;
    let (open_end, tag_name, close_start, self_closing) = xml_fragment_bounds(&fragment)?;
    if self_closing {
        let start_tag = xml_open_tag_from_start(&fragment[..=open_end]);
        return Ok(format!("{start_tag}{pane_xml}</{tag_name}>"));
    }
    if let Some(pane) = direct_child_range(&fragment, open_end + 1, close_start, "pane")? {
        return Ok(replace_xml_span(&fragment, pane.start, pane.end, pane_xml));
    }
    let insert_at = direct_child_ranges(&fragment, open_end + 1, close_start)?
        .into_iter()
        .find(|child| sheet_view_child_order(&child.kind) > sheet_view_child_order("pane"))
        .map(|child| child.start)
        .unwrap_or(close_start);
    let mut updated = String::new();
    updated.push_str(&fragment[..insert_at]);
    updated.push_str(pane_xml);
    updated.push_str(&fragment[insert_at..]);
    Ok(updated)
}

fn insert_worksheet_child(
    xml: &str,
    root: &WorksheetRootBounds,
    local_name: &str,
    child_xml: &str,
) -> CliResult<String> {
    if root.self_closing {
        let start_tag = xml_open_tag_from_start(&xml[root.start..root.open_end]);
        let mut updated = String::new();
        updated.push_str(&xml[..root.start]);
        updated.push_str(&start_tag);
        updated.push_str(child_xml);
        updated.push_str(&format!("</{}>", root.tag_name));
        updated.push_str(&xml[root.end..]);
        return Ok(updated);
    }
    let target_order = worksheet_child_order(local_name);
    let insert_at = direct_child_ranges(xml, root.open_end, root.close_start)?
        .into_iter()
        .find(|child| worksheet_child_order(&child.kind) > target_order)
        .map(|child| child.start)
        .unwrap_or(root.close_start);
    let mut updated = String::new();
    updated.push_str(&xml[..insert_at]);
    updated.push_str(child_xml);
    updated.push_str(&xml[insert_at..]);
    Ok(updated)
}

fn guard_expect_state(xml: &str, has_expect: bool, expect: Option<&str>) -> CliResult<()> {
    if !has_expect {
        return Ok(());
    }
    let expect = expect.unwrap_or("").trim();
    if !matches!(expect, "none" | "frozen") {
        return Err(CliError::invalid_args(format!(
            "invalid --expect-state {:?} (use none|frozen)",
            expect
        )));
    }
    let current = if read_freeze_state(xml)?.is_some() {
        "frozen"
    } else {
        "none"
    };
    if current != expect {
        return Err(CliError::invalid_args(format!(
            "freeze state mismatch: expected {:?}, found {:?}",
            expect, current
        )));
    }
    Ok(())
}

fn find_freeze_pane_range(xml: &str) -> CliResult<Option<crate::XmlNamedRange>> {
    let root = worksheet_root_bounds(xml)?;
    let Some(sheet_views) = direct_child_range(xml, root.open_end, root.close_start, "sheetViews")?
    else {
        return Ok(None);
    };
    let sheet_views_fragment = &xml[sheet_views.start..sheet_views.end];
    let (open_end, _, close_start, self_closing) = xml_fragment_bounds(sheet_views_fragment)?;
    if self_closing {
        return Ok(None);
    }
    let Some(sheet_view) =
        direct_child_range(sheet_views_fragment, open_end + 1, close_start, "sheetView")?
    else {
        return Ok(None);
    };
    let sheet_view_fragment = &sheet_views_fragment[sheet_view.start..sheet_view.end];
    let (open_end, _, close_start, self_closing) = xml_fragment_bounds(sheet_view_fragment)?;
    if self_closing {
        return Ok(None);
    }
    Ok(
        direct_child_range(sheet_view_fragment, open_end + 1, close_start, "pane")?.map(|pane| {
            crate::XmlNamedRange {
                start: sheet_views.start + sheet_view.start + pane.start,
                end: sheet_views.start + sheet_view.start + pane.end,
                kind: pane.kind,
            }
        }),
    )
}

fn worksheet_root_bounds(xml: &str) -> CliResult<WorksheetRootBounds> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "worksheet" => {
                let open_end = reader.buffer_position() as usize;
                let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let close_tag = format!("</{tag_name}>");
                let close_start = xml
                    .rfind(&close_tag)
                    .ok_or_else(|| CliError::unexpected("worksheet root has no closing tag"))?;
                return Ok(WorksheetRootBounds {
                    start: before,
                    open_end,
                    close_start,
                    end: close_start + close_tag.len(),
                    tag_name,
                    self_closing: false,
                });
            }
            Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == "worksheet" => {
                return Ok(WorksheetRootBounds {
                    start: before,
                    open_end: reader.buffer_position() as usize,
                    close_start: reader.buffer_position() as usize,
                    end: reader.buffer_position() as usize,
                    tag_name: String::from_utf8_lossy(e.name().as_ref()).to_string(),
                    self_closing: true,
                });
            }
            Ok(Event::Eof) => return Err(CliError::unexpected("worksheet root not found")),
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
}

fn direct_child_range(
    xml: &str,
    content_start: usize,
    content_end: usize,
    kind: &str,
) -> CliResult<Option<crate::XmlNamedRange>> {
    Ok(direct_child_ranges(xml, content_start, content_end)?
        .into_iter()
        .find(|child| child.kind == kind))
}

fn direct_child_ranges(
    xml: &str,
    content_start: usize,
    content_end: usize,
) -> CliResult<Vec<crate::XmlNamedRange>> {
    if content_start >= content_end {
        return Ok(Vec::new());
    }
    xml_direct_child_ranges(xml, content_start, content_end)
}

fn first_element_attrs(fragment: &str) -> CliResult<BTreeMap<String, String>> {
    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(false);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => return Ok(xml_attrs(&e)),
            Ok(Event::Eof) => return Err(CliError::unexpected("XML element not found")),
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
}

fn ensure_workbook_view_id(fragment: &str) -> CliResult<String> {
    if first_element_attrs(fragment)?.contains_key("workbookViewId") {
        return Ok(fragment.to_string());
    }
    let open_end = fragment
        .find('>')
        .ok_or_else(|| CliError::unexpected("invalid sheetView XML"))?;
    let insert_at = if fragment[..=open_end].trim_end().ends_with("/>") {
        fragment[..open_end]
            .rfind('/')
            .ok_or_else(|| CliError::unexpected("invalid sheetView XML"))?
    } else {
        open_end
    };
    let mut updated = String::new();
    updated.push_str(&fragment[..insert_at]);
    updated.push_str(" workbookViewId=\"0\"");
    updated.push_str(&fragment[insert_at..]);
    Ok(updated)
}

fn render_freeze_pane(prefix: &str, state: &XlsxFreezeState) -> String {
    let mut attrs = BTreeMap::new();
    if state.cols > 0 {
        attrs.insert("xSplit".to_string(), state.cols.to_string());
    }
    if state.rows > 0 {
        attrs.insert("ySplit".to_string(), state.rows.to_string());
    }
    attrs.insert("topLeftCell".to_string(), state.top_left_cell.clone());
    attrs.insert("state".to_string(), "frozen".to_string());
    format!(
        "<{}{} />",
        element_name(prefix, "pane"),
        render_xml_attrs(&attrs)
    )
    .replace(" />", "/>")
}

fn freeze_top_left_cell(rows: i64, cols: i64) -> String {
    format!("{}{}", col_name((cols + 1) as u32), rows + 1)
}

fn freeze_state_json(state: Option<&XlsxFreezeState>) -> Value {
    match state {
        Some(state) => json!({
            "rows": state.rows,
            "cols": state.cols,
            "topLeftCell": state.top_left_cell,
            "frozen": state.frozen,
        }),
        None => Value::Null,
    }
}

fn freeze_show_command(file: &str, sheet: &WorkbookSheet) -> String {
    format!(
        "ooxml --json xlsx freeze show {} --sheet {}",
        command_arg(file),
        command_arg(&freeze_sheet_selector(sheet))
    )
}

fn freeze_sheet_selector(sheet: &WorkbookSheet) -> String {
    format!("sheetId:{}", sheet.sheet_id)
}

fn map_freeze_error(action: &str, message: String) -> CliError {
    CliError::invalid_args(format!("failed to {action} freeze panes: {message}"))
}

fn element_name(prefix: &str, local: &str) -> String {
    if prefix.is_empty() {
        local.to_string()
    } else {
        format!("{prefix}:{local}")
    }
}

fn worksheet_child_order(local_name: &str) -> i32 {
    match local_name {
        "sheetPr" => 10,
        "dimension" => 20,
        "sheetViews" => 30,
        "sheetFormatPr" => 40,
        "cols" => 50,
        "sheetData" => 60,
        "sheetCalcPr" => 70,
        "sheetProtection" => 80,
        "protectedRanges" => 90,
        "scenarios" => 100,
        "autoFilter" => 110,
        "sortState" => 120,
        "dataConsolidate" => 130,
        "customSheetViews" => 140,
        "mergeCells" => 150,
        "phoneticPr" => 160,
        "conditionalFormatting" => 170,
        "dataValidations" => 180,
        "hyperlinks" => 190,
        "printOptions" => 200,
        "pageMargins" => 210,
        "pageSetup" => 220,
        "headerFooter" => 230,
        "rowBreaks" => 240,
        "colBreaks" => 250,
        "customProperties" => 260,
        "cellWatches" => 270,
        "ignoredErrors" => 280,
        "smartTags" => 290,
        "drawing" => 300,
        "legacyDrawing" => 310,
        "legacyDrawingHF" => 320,
        "picture" => 330,
        "oleObjects" => 340,
        "controls" => 350,
        "webPublishItems" => 360,
        "tableParts" => 370,
        "extLst" => 380,
        _ => 1000,
    }
}

fn sheet_view_child_order(local_name: &str) -> i32 {
    match local_name {
        "pane" => 10,
        "selection" => 20,
        "pivotSelection" => 30,
        "extLst" => 40,
        _ => 1000,
    }
}
