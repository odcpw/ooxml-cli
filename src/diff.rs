use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::pptx_render::pptx_render;
use crate::{
    CliError, CliResult, DocxRichBlockReport, EXIT_DIFF_THRESHOLD, EXIT_PARTIAL_SUCCESS,
    EXIT_RENDER_FAILED, EXIT_SUCCESS, EXIT_UNEXPECTED, GlobalFlags, InspectPackageKind,
    WorkbookSheet, XlsxTableRef, append_xml_text_event, attr, content_type_for_part,
    detect_inspect_package_type, docx_rich_block_reports, find_docx_document_part,
    find_xlsx_workbook_part, is_docx_comments_part, is_docx_endnotes_part, is_docx_footer_part,
    is_docx_footnotes_part, is_docx_header_part, is_docx_media_part, is_docx_numbering_part,
    is_docx_styles_part, is_xml_text_event, local_name, normalize_xl_target, package_type,
    parse_cell_ref, pptx_diff, relationship_entries, relationships_part_for, shared_strings,
    sheet_cells, workbook_sheets, xlsx_styles, xlsx_tables, zip_bytes, zip_entry_names, zip_text,
};

pub(crate) struct DiffCommandOutput {
    pub(crate) value: Value,
    pub(crate) exit_code: i32,
}

pub(crate) fn diff_command(
    flags: &GlobalFlags,
    baseline: &str,
    candidate: &str,
    args: &[String],
) -> CliResult<DiffCommandOutput> {
    let options = parse_diff_options(args)?;

    let baseline_type = package_type(baseline)?;
    let candidate_type = package_type(candidate)?;
    if baseline_type != candidate_type {
        return Err(CliError::unsupported_type(format!(
            "cannot diff different package types (baseline: {baseline_type}, candidate: {candidate_type})"
        )));
    }

    let mut value = match baseline_type {
        "pptx" => pptx_diff(baseline, candidate),
        "xlsx" => xlsx_diff(baseline, candidate),
        "docx" => docx_diff(baseline, candidate),
        other => Err(CliError::unsupported_type(format!(
            "unsupported package type for diff: {other}"
        ))),
    }?;
    let mut exit_code = EXIT_SUCCESS;
    if baseline_type == "pptx" {
        let visual = if options.render {
            let outcome = render_visual_diff(flags, baseline, candidate, &options);
            exit_code = outcome.exit_code;
            outcome.value
        } else {
            visual_disabled()
        };
        if let Some(map) = value.as_object_mut() {
            map.insert("visual".to_string(), visual);
        }
    }
    Ok(DiffCommandOutput { value, exit_code })
}

pub(crate) fn pptx_diff_command(
    baseline: &str,
    candidate: &str,
    args: &[String],
) -> CliResult<Value> {
    pptx_diff_dispatch(&GlobalFlags::default(), baseline, candidate, args)
        .map(|output| output.value)
}

pub(crate) fn pptx_diff_dispatch(
    flags: &GlobalFlags,
    baseline: &str,
    candidate: &str,
    args: &[String],
) -> CliResult<DiffCommandOutput> {
    let options = parse_diff_options(args)?;

    let baseline_type = package_type(baseline)?;
    let candidate_type = package_type(candidate)?;
    if baseline_type != "pptx" || candidate_type != "pptx" {
        return Err(CliError::unsupported_type(format!(
            "pptx diff requires PPTX inputs (baseline: {baseline_type}, candidate: {candidate_type})"
        )));
    }

    let mut result = pptx_diff(baseline, candidate)?;
    if let Some(map) = result.as_object_mut() {
        map.remove("schemaVersion");
        map.remove("type");
        let visual = if options.render {
            let outcome = render_visual_diff(flags, baseline, candidate, &options);
            map.insert("visual".to_string(), outcome.value);
            return Ok(DiffCommandOutput {
                value: result,
                exit_code: outcome.exit_code,
            });
        } else {
            visual_disabled()
        };
        map.insert("visual".to_string(), visual);
    }
    Ok(DiffCommandOutput {
        value: result,
        exit_code: EXIT_SUCCESS,
    })
}

#[derive(Default)]
struct DiffOptions {
    render: bool,
    threshold: f64,
    out: Option<String>,
}

fn parse_diff_options(args: &[String]) -> CliResult<DiffOptions> {
    let mut options = DiffOptions {
        threshold: 0.01,
        ..DiffOptions::default()
    };
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        match arg.as_str() {
            "--render" => {
                options.render = true;
                i += 1;
            }
            "--threshold" | "--out" | "--format" | "-f" => {
                let Some(value) = args.get(i + 1) else {
                    return Err(CliError::invalid_args(format!("{arg} requires a value")));
                };
                apply_diff_value_flag(&mut options, arg, value)?;
                i += 2;
            }
            "--json" => {
                i += 1;
            }
            _ if arg.starts_with("--render=") => {
                options.render = parse_diff_bool_value("--render", &arg["--render=".len()..])?;
                i += 1;
            }
            _ if arg.starts_with("--threshold=") => {
                options.threshold = parse_threshold_value(&arg["--threshold=".len()..])?;
                i += 1;
            }
            _ if arg.starts_with("--out=") => {
                options.out = Some(arg["--out=".len()..].to_string());
                i += 1;
            }
            _ if arg.starts_with("--format=") => {
                validate_json_format(&arg["--format=".len()..])?;
                i += 1;
            }
            _ if arg.starts_with("--") => {
                return Err(CliError::invalid_args(format!("unknown flag: {arg}")));
            }
            _ => {
                return Err(CliError::invalid_args(
                    "diff accepts exactly two file arguments",
                ));
            }
        }
    }
    Ok(options)
}

fn apply_diff_value_flag(options: &mut DiffOptions, flag: &str, value: &str) -> CliResult<()> {
    match flag {
        "--threshold" => {
            options.threshold = parse_threshold_value(value)?;
            Ok(())
        }
        "--format" | "-f" => validate_json_format(value),
        "--out" => {
            options.out = Some(value.to_string());
            Ok(())
        }
        _ => Ok(()),
    }
}

fn parse_threshold_value(value: &str) -> CliResult<f64> {
    value
        .parse::<f64>()
        .map_err(|_| CliError::invalid_args("--threshold must be a number"))
}

fn visual_disabled() -> Value {
    json!({
        "enabled": false,
        "status": "disabled",
    })
}

struct VisualOutcome {
    value: Value,
    exit_code: i32,
}

fn render_visual_diff(
    flags: &GlobalFlags,
    baseline: &str,
    candidate: &str,
    options: &DiffOptions,
) -> VisualOutcome {
    match try_render_visual_diff(baseline, candidate, options) {
        Ok(value) => {
            let pass = value["pass"].as_bool().unwrap_or(true);
            let exit_code = if pass {
                EXIT_SUCCESS
            } else {
                EXIT_DIFF_THRESHOLD
            };
            VisualOutcome { value, exit_code }
        }
        Err(err) if is_render_tool_issue(&err) => VisualOutcome {
            value: json!({
                "enabled": true,
                "status": "unavailable",
                "threshold": options.threshold,
            }),
            exit_code: if flags.strict {
                EXIT_RENDER_FAILED
            } else {
                EXIT_PARTIAL_SUCCESS
            },
        },
        Err(_) => VisualOutcome {
            value: json!({
                "enabled": true,
                "status": "error",
                "threshold": options.threshold,
            }),
            exit_code: EXIT_UNEXPECTED,
        },
    }
}

fn try_render_visual_diff(
    baseline: &str,
    candidate: &str,
    options: &DiffOptions,
) -> CliResult<Value> {
    let workspace = DiffRenderWorkspace::new(options.out.as_deref())?;
    let base_dir = workspace.path.join("baseline");
    let candidate_dir = workspace.path.join("candidate");
    let diff_dir = workspace.path.join("diff");
    fs::create_dir_all(&diff_dir).map_err(|err| CliError::unexpected(err.to_string()))?;

    let base_images = render_pptx_images(baseline, &base_dir)?;
    let candidate_images = render_pptx_images(candidate, &candidate_dir)?;
    let max_slides = base_images.len().max(candidate_images.len());
    let mut pass = true;
    let mut slides = Vec::with_capacity(max_slides);

    for index in 0..max_slides {
        let slide = index + 1;
        let mut entry = Map::new();
        entry.insert("slide".to_string(), json!(slide));
        match (base_images.get(index), candidate_images.get(index)) {
            (Some(base_image), Some(candidate_image)) => {
                let diff_image = diff_dir.join(format!("slide-{slide}-diff.png"));
                let difference = visual_image_diff(base_image, candidate_image, &diff_image)?;
                let slide_pass = difference <= options.threshold;
                if !slide_pass {
                    pass = false;
                }
                entry.insert("difference".to_string(), json!(difference));
                entry.insert("pass".to_string(), json!(slide_pass));
                if diff_image.exists() {
                    entry.insert(
                        "diffImage".to_string(),
                        json!(diff_image.to_string_lossy().to_string()),
                    );
                }
            }
            _ => {
                pass = false;
                entry.insert("difference".to_string(), json!(1.0));
                entry.insert("pass".to_string(), json!(false));
            }
        }
        slides.push(Value::Object(entry));
    }

    Ok(json!({
        "enabled": true,
        "status": "ok",
        "threshold": options.threshold,
        "pass": pass,
        "slides": slides,
    }))
}

struct DiffRenderWorkspace {
    path: PathBuf,
    cleanup: bool,
}

impl DiffRenderWorkspace {
    fn new(out: Option<&str>) -> CliResult<Self> {
        if let Some(out) = out
            && !out.is_empty()
        {
            let path = PathBuf::from(out);
            fs::create_dir_all(&path).map_err(|err| CliError::unexpected(err.to_string()))?;
            return Ok(Self {
                path,
                cleanup: false,
            });
        }

        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("ooxml-diff-{}-{suffix}", std::process::id()));
        fs::create_dir_all(&path).map_err(|err| CliError::unexpected(err.to_string()))?;
        Ok(Self {
            path,
            cleanup: true,
        })
    }
}

impl Drop for DiffRenderWorkspace {
    fn drop(&mut self) {
        if self.cleanup {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

fn render_pptx_images(file: &str, out_dir: &Path) -> CliResult<Vec<PathBuf>> {
    let args = vec![
        "--out".to_string(),
        out_dir.to_string_lossy().to_string(),
        "--format".to_string(),
        "json".to_string(),
    ];
    let value = pptx_render(file, &args)?;
    let slides = value["slides"]
        .as_array()
        .ok_or_else(|| CliError::unexpected("render manifest missing slides"))?;
    let mut images = Vec::with_capacity(slides.len());
    for slide in slides {
        let image = slide["imagePath"]
            .as_str()
            .ok_or_else(|| CliError::unexpected("render manifest slide missing imagePath"))?;
        images.push(PathBuf::from(image));
    }
    Ok(images)
}

fn visual_image_diff(
    base_image: &Path,
    candidate_image: &Path,
    diff_image: &Path,
) -> CliResult<f64> {
    if std::env::var_os("OOXML_RUST_MOCK_RENDER").is_some() {
        return mock_visual_image_diff(base_image, candidate_image, diff_image);
    }
    let (program, args) = visual_diff_command(base_image, candidate_image, diff_image)?;
    let output = Command::new(&program)
        .args(args)
        .output()
        .map_err(|err| CliError::unexpected(format!("{program} failed: {err}")))?;
    if let Some(metric) = parse_visual_diff_metric(&output.stderr)
        .or_else(|| parse_visual_diff_metric(&output.stdout))
    {
        return Ok(metric);
    }
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if stderr.is_empty() {
            return Err(CliError::unexpected(format!(
                "{program} failed: {}",
                output.status
            )));
        }
        return Err(CliError::unexpected(format!("{program} failed: {stderr}")));
    }
    Err(CliError::unexpected(
        "could not parse visual diff metric output",
    ))
}

fn mock_visual_image_diff(
    base_image: &Path,
    candidate_image: &Path,
    diff_image: &Path,
) -> CliResult<f64> {
    if let Some(parent) = diff_image.parent() {
        fs::create_dir_all(parent).map_err(|err| CliError::unexpected(err.to_string()))?;
    }
    let base = fs::read(base_image).map_err(|err| CliError::unexpected(err.to_string()))?;
    let candidate =
        fs::read(candidate_image).map_err(|err| CliError::unexpected(err.to_string()))?;
    fs::write(diff_image, b"png").map_err(|err| CliError::unexpected(err.to_string()))?;
    Ok(if base == candidate { 0.0 } else { 1.0 })
}

fn visual_diff_command(
    base_image: &Path,
    candidate_image: &Path,
    diff_image: &Path,
) -> CliResult<(String, Vec<String>)> {
    if command_exists("compare") {
        return Ok((
            "compare".to_string(),
            vec![
                "-metric".to_string(),
                "RMSE".to_string(),
                base_image.to_string_lossy().to_string(),
                candidate_image.to_string_lossy().to_string(),
                diff_image.to_string_lossy().to_string(),
            ],
        ));
    }
    if command_exists("magick") {
        return Ok((
            "magick".to_string(),
            vec![
                "compare".to_string(),
                "-metric".to_string(),
                "RMSE".to_string(),
                base_image.to_string_lossy().to_string(),
                candidate_image.to_string_lossy().to_string(),
                diff_image.to_string_lossy().to_string(),
            ],
        ));
    }
    Err(CliError::unexpected(
        "required render tool not available: compare",
    ))
}

fn command_exists(name: &str) -> bool {
    Command::new(name).arg("--version").output().is_ok()
}

fn parse_visual_diff_metric(bytes: &[u8]) -> Option<f64> {
    let text = String::from_utf8_lossy(bytes);
    if let Some(start) = text.find('(')
        && let Some(end) = text[start + 1..].find(')')
    {
        let metric = &text[start + 1..start + 1 + end];
        if let Ok(value) = metric.trim().parse::<f64>() {
            return Some(value);
        }
    }
    for token in text.split_whitespace() {
        let token = token.trim_matches(|ch| ch == '(' || ch == ')' || ch == ',');
        if let Ok(value) = token.parse::<f64>() {
            return Some(value);
        }
    }
    None
}

fn is_render_tool_issue(err: &CliError) -> bool {
    let message = err.message.as_str();
    message.starts_with("required render tool not available:")
        || message.starts_with("soffice failed:")
        || message.starts_with("libreoffice failed:")
        || message.starts_with("pdftoppm failed:")
        || message.starts_with("compare failed:")
        || message.starts_with("magick failed:")
        || message == "soffice render failed"
        || message == "pdftoppm rasterize failed"
}

fn validate_json_format(value: &str) -> CliResult<()> {
    if value == "json" {
        Ok(())
    } else {
        Err(CliError::invalid_args(format!(
            "invalid format: {value} (expected 'text' or 'json')"
        )))
    }
}

fn parse_diff_bool_value(flag: &str, value: &str) -> CliResult<bool> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(CliError::invalid_args(format!(
            "{flag} must be true or false"
        ))),
    }
}

fn xlsx_diff(baseline: &str, candidate: &str) -> CliResult<Value> {
    let before = read_xlsx_snapshot(baseline)?;
    let after = read_xlsx_snapshot(candidate)?;

    let mut changed_sheets = BTreeSet::<String>::new();
    let mut sheet_diffs = Vec::<Value>::new();
    let mut cell_diffs = Vec::<Value>::new();

    for pair in align_xlsx_sheets(&before.sheets, &after.sheets) {
        match (pair.before, pair.after) {
            (Some(left), Some(right)) => {
                let sheet_name = xlsx_sheet_diff_name(left, right);
                if left.name != right.name {
                    sheet_diffs.push(json!({
                        "sheet": sheet_name,
                        "change": "renamed",
                        "before": left.name,
                        "after": right.name,
                        "identity": xlsx_sheet_identity_json(left, right),
                    }));
                    changed_sheets.insert(sheet_name.clone());
                }
                for diff in compare_xlsx_cells(&sheet_name, left, right) {
                    cell_diffs.push(diff);
                    changed_sheets.insert(sheet_name.clone());
                }
            }
            (Some(left), None) => {
                sheet_diffs.push(json!({"sheet": left.name, "change": "removed"}));
                changed_sheets.insert(left.name.clone());
            }
            (None, Some(right)) => {
                sheet_diffs.push(json!({"sheet": right.name, "change": "added"}));
                changed_sheets.insert(right.name.clone());
            }
            (None, None) => {}
        }
    }

    let defined_name_diffs =
        compare_xlsx_defined_names(&before.defined_names, &after.defined_names);
    let table_diffs = compare_xlsx_tables(&before.tables, &after.tables, &mut changed_sheets);

    Ok(json!({
        "schemaVersion": "1.0",
        "type": "xlsx",
        "semantic": {
            "schemaVersion": "1.0",
            "sheetCountA": before.sheet_count,
            "sheetCountB": after.sheet_count,
            "sheetCountEqual": before.sheet_count == after.sheet_count,
            "changedSheets": changed_sheets.into_iter().collect::<Vec<_>>(),
            "sheets": sheet_diffs,
            "cellDiffs": cell_diffs,
            "definedNameDiffs": defined_name_diffs,
            "tableDiffs": table_diffs,
        },
    }))
}

struct XlsxSnapshot {
    sheet_count: usize,
    sheets: Vec<XlsxSheetSnapshot>,
    defined_names: Vec<XlsxDefinedNameSnapshot>,
    tables: Vec<XlsxTableRef>,
}

struct XlsxSheetSnapshot {
    name: String,
    sheet_id: u32,
    rel_id: String,
    part_uri: String,
    cells: BTreeMap<String, XlsxCellSnapshot>,
}

#[derive(Clone, Copy)]
struct XlsxSheetPair<'a> {
    before: Option<&'a XlsxSheetSnapshot>,
    after: Option<&'a XlsxSheetSnapshot>,
}

#[derive(Clone, Default)]
struct XlsxCellSnapshot {
    row: u32,
    col: u32,
    value: String,
    formula: String,
}

#[derive(Clone)]
struct XlsxDefinedNameSnapshot {
    name: String,
    scope: String,
    sheet_name: String,
    reference: String,
}

fn read_xlsx_snapshot(file: &str) -> CliResult<XlsxSnapshot> {
    let entries = zip_entry_names(file)?;
    let kind = detect_inspect_package_type(file, &entries);
    if kind != InspectPackageKind::Xlsx {
        return Err(CliError::unsupported_type(format!(
            "unsupported package type for diff: {}",
            package_type(file)?
        )));
    }

    let workbook_part = find_xlsx_workbook_part(file, &entries)?;
    let workbook_xml = zip_text(file, &workbook_part)?;
    let sheets = workbook_sheets(&workbook_xml)?;
    let rels = relationship_entries(file, &relationships_part_for(&workbook_part))?;
    let shared_strings = shared_strings(file).unwrap_or_default();
    let styles = xlsx_styles(file).unwrap_or_default();

    let mut sheet_snapshots = Vec::new();
    for sheet in &sheets {
        let Some(rel) = rels.iter().find(|rel| rel.id == sheet.rel_id) else {
            return Err(CliError::unexpected(format!(
                "missing relationship {}",
                sheet.rel_id
            )));
        };
        if rel.target_mode == "External" || !rel.rel_type.ends_with("/worksheet") {
            continue;
        }
        let sheet_part = normalize_xl_target(&rel.target);
        let sheet_xml = zip_text(file, &sheet_part)?;
        let mut cells = BTreeMap::new();
        for (cell_ref, value) in sheet_cells(&sheet_xml, &shared_strings, &styles) {
            let (col, row) = parse_cell_ref(&cell_ref).unwrap_or((0, 0));
            cells.insert(
                cell_ref,
                XlsxCellSnapshot {
                    row,
                    col,
                    value: value.display_value,
                    formula: value.formula,
                },
            );
        }
        sheet_snapshots.push(XlsxSheetSnapshot {
            name: sheet.name.clone(),
            sheet_id: sheet.sheet_id,
            rel_id: sheet.rel_id.clone(),
            part_uri: sheet_part,
            cells,
        });
    }

    Ok(XlsxSnapshot {
        sheet_count: sheets.len(),
        sheets: sheet_snapshots,
        defined_names: parse_xlsx_defined_names(&workbook_xml, &sheets)?,
        tables: xlsx_tables(file, None)?,
    })
}

fn align_xlsx_sheets<'a>(
    before: &'a [XlsxSheetSnapshot],
    after: &'a [XlsxSheetSnapshot],
) -> Vec<XlsxSheetPair<'a>> {
    let mut matched_before = BTreeSet::new();
    let mut matched_after = BTreeSet::new();
    let mut pairs = Vec::new();

    match_xlsx_sheets_by(
        before,
        after,
        &mut matched_before,
        &mut matched_after,
        &mut pairs,
        |sheet| sheet.part_uri.clone(),
    );
    match_xlsx_sheets_by(
        before,
        after,
        &mut matched_before,
        &mut matched_after,
        &mut pairs,
        |sheet| format!("sheetId:{}", sheet.sheet_id),
    );
    match_xlsx_sheets_by(
        before,
        after,
        &mut matched_before,
        &mut matched_after,
        &mut pairs,
        |sheet| sheet.rel_id.clone(),
    );
    match_xlsx_sheets_by(
        before,
        after,
        &mut matched_before,
        &mut matched_after,
        &mut pairs,
        |sheet| sheet.name.clone(),
    );

    for (index, sheet) in before.iter().enumerate() {
        if !matched_before.contains(&index) {
            pairs.push(XlsxSheetPair {
                before: Some(sheet),
                after: None,
            });
        }
    }
    for (index, sheet) in after.iter().enumerate() {
        if !matched_after.contains(&index) {
            pairs.push(XlsxSheetPair {
                before: None,
                after: Some(sheet),
            });
        }
    }

    pairs.sort_by_key(xlsx_sheet_pair_sort_key);
    pairs
}

fn match_xlsx_sheets_by<'a, F>(
    before: &'a [XlsxSheetSnapshot],
    after: &'a [XlsxSheetSnapshot],
    matched_before: &mut BTreeSet<usize>,
    matched_after: &mut BTreeSet<usize>,
    pairs: &mut Vec<XlsxSheetPair<'a>>,
    key_for: F,
) where
    F: Fn(&XlsxSheetSnapshot) -> String,
{
    let before_index = unmatched_xlsx_sheet_identity_index(before, matched_before, &key_for);
    let after_index = unmatched_xlsx_sheet_identity_index(after, matched_after, &key_for);
    for key in before_index
        .keys()
        .chain(after_index.keys())
        .cloned()
        .collect::<BTreeSet<_>>()
    {
        let Some(left) = before_index.get(&key).and_then(|items| {
            if items.len() == 1 {
                Some(items[0])
            } else {
                None
            }
        }) else {
            continue;
        };
        let Some(right) = after_index.get(&key).and_then(|items| {
            if items.len() == 1 {
                Some(items[0])
            } else {
                None
            }
        }) else {
            continue;
        };
        if matched_before.insert(left) && matched_after.insert(right) {
            pairs.push(XlsxSheetPair {
                before: Some(&before[left]),
                after: Some(&after[right]),
            });
        }
    }
}

fn unmatched_xlsx_sheet_identity_index<F>(
    sheets: &[XlsxSheetSnapshot],
    matched: &BTreeSet<usize>,
    key_for: F,
) -> BTreeMap<String, Vec<usize>>
where
    F: Fn(&XlsxSheetSnapshot) -> String,
{
    let mut index = BTreeMap::<String, Vec<usize>>::new();
    for (position, sheet) in sheets.iter().enumerate() {
        if matched.contains(&position) {
            continue;
        }
        let key = key_for(sheet);
        if !key.trim().is_empty() {
            index.entry(key).or_default().push(position);
        }
    }
    index
}

fn xlsx_sheet_pair_sort_key(pair: &XlsxSheetPair<'_>) -> String {
    match (pair.before, pair.after) {
        (_, Some(after)) => after.name.clone(),
        (Some(before), None) => before.name.clone(),
        (None, None) => String::new(),
    }
}

fn xlsx_sheet_diff_name(left: &XlsxSheetSnapshot, right: &XlsxSheetSnapshot) -> String {
    if !right.name.is_empty() {
        right.name.clone()
    } else {
        left.name.clone()
    }
}

fn xlsx_sheet_identity_json(left: &XlsxSheetSnapshot, right: &XlsxSheetSnapshot) -> Value {
    json!({
        "sheetIdBefore": left.sheet_id,
        "sheetIdAfter": right.sheet_id,
        "relationshipIdBefore": left.rel_id,
        "relationshipIdAfter": right.rel_id,
        "partUriBefore": left.part_uri,
        "partUriAfter": right.part_uri,
    })
}

fn compare_xlsx_cells(
    sheet: &str,
    before: &XlsxSheetSnapshot,
    after: &XlsxSheetSnapshot,
) -> Vec<Value> {
    let mut refs = before
        .cells
        .keys()
        .chain(after.cells.keys())
        .cloned()
        .collect::<Vec<_>>();
    refs.sort_by_key(|cell_ref| {
        before
            .cells
            .get(cell_ref)
            .or_else(|| after.cells.get(cell_ref))
            .map(|cell| (cell.row, cell.col, cell_ref.clone()))
            .unwrap_or_else(|| (0, 0, cell_ref.clone()))
    });
    refs.dedup();

    let mut diffs = Vec::new();
    for cell_ref in refs {
        let left = before.cells.get(&cell_ref).cloned().unwrap_or_default();
        let right = after.cells.get(&cell_ref).cloned().unwrap_or_default();
        if left.value != right.value {
            diffs.push(json!({
                "sheet": sheet,
                "cell": cell_ref,
                "property": "value",
                "before": left.value,
                "after": right.value,
            }));
        }
        if left.formula != right.formula {
            diffs.push(json!({
                "sheet": sheet,
                "cell": cell_ref,
                "property": "formula",
                "before": left.formula,
                "after": right.formula,
            }));
        }
    }
    diffs
}

fn parse_xlsx_defined_names(
    workbook_xml: &str,
    sheets: &[WorkbookSheet],
) -> CliResult<Vec<XlsxDefinedNameSnapshot>> {
    let mut reader = Reader::from_str(workbook_xml);
    reader.config_mut().trim_text(false);
    let mut in_defined_names = false;
    let mut defined_names_depth = 0_u32;
    let mut current: Option<XlsxDefinedNameSnapshot> = None;
    let mut text = String::new();
    let mut names = Vec::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if name == "definedNames" && current.is_none() {
                    in_defined_names = true;
                    defined_names_depth = 1;
                } else if in_defined_names && defined_names_depth == 1 && name == "definedName" {
                    current = Some(xlsx_defined_name_from_element(&e, sheets));
                    text.clear();
                } else if in_defined_names {
                    defined_names_depth += 1;
                }
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if in_defined_names && defined_names_depth == 1 && name == "definedName" {
                    let mut item = xlsx_defined_name_from_element(&e, sheets);
                    item.reference.clear();
                    names.push(item);
                }
            }
            Ok(event) if current.is_some() && is_xml_text_event(&event) => {
                append_xml_text_event(&mut text, &event);
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if name == "definedName" && current.is_some() {
                    let mut item = current.take().expect("defined name current");
                    item.reference = text.clone();
                    names.push(item);
                    text.clear();
                } else if name == "definedNames" {
                    in_defined_names = false;
                    defined_names_depth = 0;
                } else if in_defined_names && defined_names_depth > 0 {
                    defined_names_depth -= 1;
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }

    Ok(names)
}

fn xlsx_defined_name_from_element(
    element: &quick_xml::events::BytesStart<'_>,
    sheets: &[WorkbookSheet],
) -> XlsxDefinedNameSnapshot {
    let name = attr(element, "name").unwrap_or_default();
    let local_sheet_id =
        attr(element, "localSheetId").and_then(|value| value.parse::<usize>().ok());
    let sheet_name = local_sheet_id
        .and_then(|index| sheets.get(index))
        .map(|sheet| sheet.name.clone())
        .unwrap_or_default();
    let scope = if local_sheet_id.is_some() {
        "sheet"
    } else {
        "workbook"
    };
    XlsxDefinedNameSnapshot {
        name,
        scope: scope.to_string(),
        sheet_name,
        reference: String::new(),
    }
}

fn compare_xlsx_defined_names(
    before: &[XlsxDefinedNameSnapshot],
    after: &[XlsxDefinedNameSnapshot],
) -> Vec<Value> {
    let before = index_xlsx_defined_names(before);
    let after = index_xlsx_defined_names(after);
    let mut diffs = Vec::new();
    for key in before
        .keys()
        .chain(after.keys())
        .cloned()
        .collect::<BTreeSet<_>>()
    {
        match (before.get(&key), after.get(&key)) {
            (Some(left), Some(right)) if left.reference != right.reference => {
                diffs.push(json!({
                    "name": left.name,
                    "scope": left.scope,
                    "change": "modified",
                    "before": left.reference,
                    "after": right.reference,
                }));
            }
            (Some(left), None) => {
                diffs.push(json!({
                    "name": left.name,
                    "scope": left.scope,
                    "change": "removed",
                    "before": left.reference,
                }));
            }
            (None, Some(right)) => {
                diffs.push(json!({
                    "name": right.name,
                    "scope": right.scope,
                    "change": "added",
                    "after": right.reference,
                }));
            }
            _ => {}
        }
    }
    diffs
}

fn index_xlsx_defined_names(
    names: &[XlsxDefinedNameSnapshot],
) -> BTreeMap<String, XlsxDefinedNameSnapshot> {
    names
        .iter()
        .cloned()
        .map(|name| {
            let key = if name.scope == "sheet" {
                format!("{}\0{}\0{}", name.scope, name.sheet_name, name.name)
            } else {
                format!("{}\0\0{}", name.scope, name.name)
            };
            (key, name)
        })
        .collect()
}

fn compare_xlsx_tables(
    before: &[XlsxTableRef],
    after: &[XlsxTableRef],
    changed_sheets: &mut BTreeSet<String>,
) -> Vec<Value> {
    let before = index_xlsx_tables(before);
    let after = index_xlsx_tables(after);
    let mut diffs = Vec::new();
    for key in before
        .keys()
        .chain(after.keys())
        .cloned()
        .collect::<BTreeSet<_>>()
    {
        match (before.get(&key), after.get(&key)) {
            (Some(left), Some(right)) => {
                if left.range != right.range {
                    changed_sheets.insert(left.sheet.clone());
                    diffs.push(json!({
                        "sheet": left.sheet,
                        "table": xlsx_table_name(left),
                        "property": "range",
                        "change": "modified",
                        "before": left.range,
                        "after": right.range,
                    }));
                }
                let before_cols = xlsx_table_columns(left);
                let after_cols = xlsx_table_columns(right);
                if before_cols != after_cols {
                    changed_sheets.insert(left.sheet.clone());
                    diffs.push(json!({
                        "sheet": left.sheet,
                        "table": xlsx_table_name(left),
                        "property": "columns",
                        "change": "modified",
                        "before": before_cols,
                        "after": after_cols,
                    }));
                }
            }
            (Some(left), None) => {
                changed_sheets.insert(left.sheet.clone());
                diffs.push(json!({
                    "sheet": left.sheet,
                    "table": xlsx_table_name(left),
                    "property": "presence",
                    "change": "removed",
                }));
            }
            (None, Some(right)) => {
                changed_sheets.insert(right.sheet.clone());
                diffs.push(json!({
                    "sheet": right.sheet,
                    "table": xlsx_table_name(right),
                    "property": "presence",
                    "change": "added",
                }));
            }
            (None, None) => {}
        }
    }
    diffs
}

fn index_xlsx_tables(tables: &[XlsxTableRef]) -> BTreeMap<String, XlsxTableRef> {
    tables
        .iter()
        .cloned()
        .map(|table| {
            (
                format!("{}\0{}", table.sheet, xlsx_table_name(&table)),
                table,
            )
        })
        .collect()
}

fn xlsx_table_name(table: &XlsxTableRef) -> String {
    if !table.display_name.is_empty() {
        table.display_name.clone()
    } else if !table.name.is_empty() {
        table.name.clone()
    } else {
        format!("table:{}", table.id)
    }
}

fn xlsx_table_columns(table: &XlsxTableRef) -> String {
    table
        .columns
        .iter()
        .map(|column| column.name.clone())
        .collect::<Vec<_>>()
        .join(", ")
}

fn docx_diff(baseline: &str, candidate: &str) -> CliResult<Value> {
    let before = read_docx_snapshot(baseline)?;
    let after = read_docx_snapshot(candidate)?;
    let mut diffs = align_docx_blocks(&before.blocks, &after.blocks);
    diffs.sort_by(|left, right| {
        left.index
            .cmp(&right.index)
            .then_with(|| left.property.cmp(&right.property))
    });

    let changed_blocks = diffs
        .iter()
        .map(|diff| diff.index)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let blocks = diffs
        .into_iter()
        .map(DocxBlockDiff::into_json)
        .collect::<Vec<_>>();
    let part_diffs = compare_docx_parts(&before.parts, &after.parts);
    let changed_parts = part_diffs
        .iter()
        .filter_map(|diff| diff.get("part").and_then(Value::as_str))
        .map(ToString::to_string)
        .collect::<Vec<_>>();

    Ok(json!({
        "schemaVersion": "1.0",
        "type": "docx",
        "semantic": {
            "schemaVersion": "1.0",
            "blockCountA": before.blocks.len(),
            "blockCountB": after.blocks.len(),
            "blockCountEqual": before.blocks.len() == after.blocks.len(),
            "changedBlocks": changed_blocks,
            "blocks": blocks,
            "secondaryPartCountA": before.parts.len(),
            "secondaryPartCountB": after.parts.len(),
            "secondaryPartCountEqual": before.parts.len() == after.parts.len(),
            "changedParts": changed_parts,
            "partDiffs": part_diffs,
        },
    }))
}

struct DocxSnapshot {
    blocks: Vec<DocxBlockSnapshot>,
    parts: Vec<DocxPartSnapshot>,
}

#[derive(Clone)]
struct DocxBlockSnapshot {
    index: usize,
    kind: String,
    text: String,
    style: String,
    table_shape: String,
}

#[derive(Clone)]
struct DocxPartSnapshot {
    part: String,
    kind: String,
    content_type: String,
    sha256: String,
}

struct DocxBlockDiff {
    index: usize,
    kind: String,
    property: String,
    change: String,
    before: Option<String>,
    after: Option<String>,
}

impl DocxBlockDiff {
    fn into_json(self) -> Value {
        let mut object = Map::new();
        object.insert("index".to_string(), json!(self.index));
        object.insert("kind".to_string(), json!(self.kind));
        object.insert("property".to_string(), json!(self.property));
        object.insert("change".to_string(), json!(self.change));
        if let Some(before) = self.before {
            object.insert("before".to_string(), json!(before));
        }
        if let Some(after) = self.after {
            object.insert("after".to_string(), json!(after));
        }
        Value::Object(object)
    }
}

fn read_docx_snapshot(file: &str) -> CliResult<DocxSnapshot> {
    let entries = zip_entry_names(file)?;
    let kind = detect_inspect_package_type(file, &entries);
    if kind != InspectPackageKind::Docx {
        return Err(CliError::unsupported_type(format!(
            "unsupported package type for diff: {}",
            package_type(file)?
        )));
    }
    let document_part = find_docx_document_part(file, &entries)?;
    let document_xml = zip_text(file, &document_part)?;
    let blocks = docx_rich_block_reports(&document_xml, false)?
        .iter()
        .map(docx_block_snapshot)
        .collect::<Vec<_>>();
    let parts = read_docx_secondary_parts(file, &entries)?;
    Ok(DocxSnapshot { blocks, parts })
}

fn docx_block_snapshot(block: &DocxRichBlockReport) -> DocxBlockSnapshot {
    DocxBlockSnapshot {
        index: block.index,
        kind: block.kind.to_string(),
        text: block.text.clone(),
        style: block.style.clone(),
        table_shape: docx_table_shape(&block.table_rows),
    }
}

fn docx_table_shape(rows: &[Vec<String>]) -> String {
    if rows.is_empty() {
        return String::new();
    }
    format!(
        "rows={} cols=[{}]",
        rows.len(),
        rows.iter()
            .map(|row| row.len().to_string())
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn read_docx_secondary_parts(file: &str, entries: &[String]) -> CliResult<Vec<DocxPartSnapshot>> {
    let mut parts = Vec::new();
    for entry in entries.iter().filter(|entry| !entry.ends_with('/')) {
        let uri = format!("/{}", entry.trim_start_matches('/'));
        let content_type = content_type_for_part(file, &uri).unwrap_or_default();
        let Some(kind) = docx_secondary_part_kind(&uri, &content_type) else {
            continue;
        };
        let bytes = zip_bytes(file, entry)?;
        parts.push(DocxPartSnapshot {
            part: entry.clone(),
            kind: kind.to_string(),
            content_type,
            sha256: sha256_digest(&bytes),
        });
    }
    parts.sort_by(|left, right| left.part.cmp(&right.part));
    Ok(parts)
}

fn docx_secondary_part_kind(uri: &str, content_type: &str) -> Option<&'static str> {
    if is_docx_header_part(uri, content_type) {
        Some("header")
    } else if is_docx_footer_part(uri, content_type) {
        Some("footer")
    } else if is_docx_footnotes_part(uri, content_type) {
        Some("footnotes")
    } else if is_docx_endnotes_part(uri, content_type) {
        Some("endnotes")
    } else if is_docx_comments_part(uri, content_type) {
        Some("comments")
    } else if is_docx_styles_part(uri, content_type) {
        Some("styles")
    } else if is_docx_numbering_part(uri, content_type) {
        Some("numbering")
    } else if is_docx_media_part(uri) {
        Some("media")
    } else {
        None
    }
}

fn sha256_digest(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("sha256:{:x}", hasher.finalize())
}

fn compare_docx_parts(before: &[DocxPartSnapshot], after: &[DocxPartSnapshot]) -> Vec<Value> {
    let before_by_part = before
        .iter()
        .map(|part| (part.part.as_str(), part))
        .collect::<BTreeMap<_, _>>();
    let after_by_part = after
        .iter()
        .map(|part| (part.part.as_str(), part))
        .collect::<BTreeMap<_, _>>();
    let part_names = before_by_part
        .keys()
        .chain(after_by_part.keys())
        .copied()
        .collect::<BTreeSet<_>>();
    part_names
        .iter()
        .filter_map(
            |part| match (before_by_part.get(part), after_by_part.get(part)) {
                (Some(left), Some(right)) if left.sha256 != right.sha256 => Some(json!({
                    "part": part,
                    "kind": right.kind,
                    "change": "modified",
                    "contentType": right.content_type,
                    "beforeHash": left.sha256,
                    "afterHash": right.sha256,
                })),
                (Some(left), None) => Some(json!({
                    "part": part,
                    "kind": left.kind,
                    "change": "removed",
                    "contentType": left.content_type,
                    "beforeHash": left.sha256,
                })),
                (None, Some(right)) => Some(json!({
                    "part": part,
                    "kind": right.kind,
                    "change": "added",
                    "contentType": right.content_type,
                    "afterHash": right.sha256,
                })),
                _ => None,
            },
        )
        .collect()
}

fn align_docx_blocks(
    before: &[DocxBlockSnapshot],
    after: &[DocxBlockSnapshot],
) -> Vec<DocxBlockDiff> {
    let before_signatures = before.iter().map(docx_block_signature).collect::<Vec<_>>();
    let after_signatures = after.iter().map(docx_block_signature).collect::<Vec<_>>();
    let pairs = lcs_pairs(&before_signatures, &after_signatures);
    let mut diffs = Vec::new();
    let mut before_index = 0;
    let mut after_index = 0;

    for (next_before, next_after) in pairs {
        emit_docx_gap(
            &before[before_index..next_before],
            &after[after_index..next_after],
            &mut diffs,
        );
        before_index = next_before + 1;
        after_index = next_after + 1;
    }
    emit_docx_gap(&before[before_index..], &after[after_index..], &mut diffs);
    diffs
}

fn docx_block_signature(block: &DocxBlockSnapshot) -> String {
    format!(
        "{}\0{}\0{}\0{}",
        block.kind, block.style, block.text, block.table_shape
    )
}

fn emit_docx_gap(
    before: &[DocxBlockSnapshot],
    after: &[DocxBlockSnapshot],
    diffs: &mut Vec<DocxBlockDiff>,
) {
    let paired = before.len().min(after.len());
    for index in 0..paired {
        if before[index].kind == after[index].kind {
            diffs.extend(compare_docx_block(&before[index], &after[index]));
        } else {
            diffs.push(removed_docx_block(&before[index]));
            diffs.push(added_docx_block(&after[index]));
        }
    }
    for block in before.iter().skip(paired) {
        diffs.push(removed_docx_block(block));
    }
    for block in after.iter().skip(paired) {
        diffs.push(added_docx_block(block));
    }
}

fn lcs_pairs(before: &[String], after: &[String]) -> Vec<(usize, usize)> {
    let mut table = vec![vec![0usize; after.len() + 1]; before.len() + 1];
    for i in (0..before.len()).rev() {
        for j in (0..after.len()).rev() {
            if before[i] == after[j] {
                table[i][j] = table[i + 1][j + 1] + 1;
            } else {
                table[i][j] = table[i + 1][j].max(table[i][j + 1]);
            }
        }
    }

    let mut pairs = Vec::new();
    let mut i = 0;
    let mut j = 0;
    while i < before.len() && j < after.len() {
        if before[i] == after[j] {
            pairs.push((i, j));
            i += 1;
            j += 1;
        } else if table[i + 1][j] >= table[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    pairs
}

fn removed_docx_block(block: &DocxBlockSnapshot) -> DocxBlockDiff {
    DocxBlockDiff {
        index: block.index,
        kind: block.kind.clone(),
        property: "presence".to_string(),
        change: "removed".to_string(),
        before: Some(block.text.clone()),
        after: None,
    }
}

fn added_docx_block(block: &DocxBlockSnapshot) -> DocxBlockDiff {
    DocxBlockDiff {
        index: block.index,
        kind: block.kind.clone(),
        property: "presence".to_string(),
        change: "added".to_string(),
        before: None,
        after: Some(block.text.clone()),
    }
}

fn compare_docx_block(before: &DocxBlockSnapshot, after: &DocxBlockSnapshot) -> Vec<DocxBlockDiff> {
    let mut diffs = Vec::new();
    let index = after.index;
    if before.kind != after.kind {
        diffs.push(DocxBlockDiff {
            index,
            kind: after.kind.clone(),
            property: "kind".to_string(),
            change: "modified".to_string(),
            before: Some(before.kind.clone()),
            after: Some(after.kind.clone()),
        });
        return diffs;
    }
    if before.text != after.text {
        diffs.push(DocxBlockDiff {
            index,
            kind: before.kind.clone(),
            property: "text".to_string(),
            change: "modified".to_string(),
            before: Some(before.text.clone()),
            after: Some(after.text.clone()),
        });
    }
    if before.kind == "paragraph" && before.style != after.style {
        diffs.push(DocxBlockDiff {
            index,
            kind: before.kind.clone(),
            property: "style".to_string(),
            change: "modified".to_string(),
            before: Some(before.style.clone()),
            after: Some(after.style.clone()),
        });
    }
    if before.kind == "table" && before.table_shape != after.table_shape {
        diffs.push(DocxBlockDiff {
            index,
            kind: before.kind.clone(),
            property: "table".to_string(),
            change: "modified".to_string(),
            before: Some(before.table_shape.clone()),
            after: Some(after.table_shape.clone()),
        });
    }
    diffs
}
