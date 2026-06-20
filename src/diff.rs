use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};

use crate::{
    CliError, CliResult, DocxRichBlockReport, InspectPackageKind, WorkbookSheet, XlsxTableRef,
    attr, decode_xml_text, detect_inspect_package_type, docx_rich_block_reports,
    find_docx_document_part, find_xlsx_workbook_part, local_name, normalize_xl_target,
    package_type, parse_cell_ref, pptx_diff, relationship_entries, relationships_part_for,
    shared_strings, sheet_cells, workbook_sheets, xlsx_styles, xlsx_tables, zip_entry_names,
    zip_text,
};

pub(crate) fn diff(baseline: &str, candidate: &str, args: &[String]) -> CliResult<Value> {
    let options = parse_diff_options(args)?;
    if options.render {
        return Err(CliError::invalid_args(
            "diff --render visual diff is not yet supported by the Rust port; rerun without --render for semantic diff",
        ));
    }

    let baseline_type = package_type(baseline)?;
    let candidate_type = package_type(candidate)?;
    if baseline_type != candidate_type {
        return Err(CliError::unsupported_type(format!(
            "cannot diff different package types (baseline: {baseline_type}, candidate: {candidate_type})"
        )));
    }

    match baseline_type {
        "pptx" => pptx_diff(baseline, candidate),
        "xlsx" => xlsx_diff(baseline, candidate),
        "docx" => docx_diff(baseline, candidate),
        other => Err(CliError::unsupported_type(format!(
            "unsupported package type for diff: {other}"
        ))),
    }
}

pub(crate) fn pptx_diff_command(
    baseline: &str,
    candidate: &str,
    args: &[String],
) -> CliResult<Value> {
    let options = parse_diff_options(args)?;
    if options.render {
        return Err(CliError::invalid_args(
            "pptx diff --render visual diff is not yet supported by the Rust port; rerun without --render for semantic diff",
        ));
    }

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
    }
    Ok(result)
}

#[derive(Default)]
struct DiffOptions {
    render: bool,
}

fn parse_diff_options(args: &[String]) -> CliResult<DiffOptions> {
    let mut options = DiffOptions::default();
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
                validate_diff_value_flag(arg, value)?;
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
                validate_threshold_value(&arg["--threshold=".len()..])?;
                i += 1;
            }
            _ if arg.starts_with("--out=") => {
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

fn validate_diff_value_flag(flag: &str, value: &str) -> CliResult<()> {
    match flag {
        "--threshold" => validate_threshold_value(value),
        "--format" | "-f" => validate_json_format(value),
        "--out" => Ok(()),
        _ => Ok(()),
    }
}

fn validate_threshold_value(value: &str) -> CliResult<()> {
    value
        .parse::<f64>()
        .map(|_| ())
        .map_err(|_| CliError::invalid_args("--threshold must be a number"))
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

    for sheet in before
        .sheets
        .keys()
        .chain(after.sheets.keys())
        .cloned()
        .collect::<BTreeSet<_>>()
    {
        match (before.sheets.get(&sheet), after.sheets.get(&sheet)) {
            (Some(left), Some(right)) => {
                for diff in compare_xlsx_cells(&sheet, left, right) {
                    cell_diffs.push(diff);
                    changed_sheets.insert(sheet.clone());
                }
            }
            (Some(_), None) => {
                sheet_diffs.push(json!({"sheet": sheet, "change": "removed"}));
                changed_sheets.insert(sheet);
            }
            (None, Some(_)) => {
                sheet_diffs.push(json!({"sheet": sheet, "change": "added"}));
                changed_sheets.insert(sheet);
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
    sheets: BTreeMap<String, XlsxSheetSnapshot>,
    defined_names: Vec<XlsxDefinedNameSnapshot>,
    tables: Vec<XlsxTableRef>,
}

struct XlsxSheetSnapshot {
    cells: BTreeMap<String, XlsxCellSnapshot>,
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

    let mut sheet_snapshots = BTreeMap::new();
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
        sheet_snapshots.insert(sheet.name.clone(), XlsxSheetSnapshot { cells });
    }

    Ok(XlsxSnapshot {
        sheet_count: sheets.len(),
        sheets: sheet_snapshots,
        defined_names: parse_xlsx_defined_names(&workbook_xml, &sheets)?,
        tables: xlsx_tables(file, None)?,
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
            Ok(Event::Text(e)) => {
                if current.is_some() {
                    text.push_str(&decode_xml_text(e.as_ref()));
                }
            }
            Ok(Event::GeneralRef(e)) => {
                if current.is_some() {
                    text.push('&');
                    text.push_str(&String::from_utf8_lossy(e.as_ref()));
                    text.push(';');
                }
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
    let before = read_docx_blocks(baseline)?;
    let after = read_docx_blocks(candidate)?;
    let mut diffs = align_docx_blocks(&before, &after);
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

    Ok(json!({
        "schemaVersion": "1.0",
        "type": "docx",
        "semantic": {
            "schemaVersion": "1.0",
            "blockCountA": before.len(),
            "blockCountB": after.len(),
            "blockCountEqual": before.len() == after.len(),
            "changedBlocks": changed_blocks,
            "blocks": blocks,
        },
    }))
}

#[derive(Clone)]
struct DocxBlockSnapshot {
    index: usize,
    kind: String,
    text: String,
    style: String,
    table_shape: String,
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

fn read_docx_blocks(file: &str) -> CliResult<Vec<DocxBlockSnapshot>> {
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
    docx_rich_block_reports(&document_xml, false)
        .map(|blocks| blocks.iter().map(docx_block_snapshot).collect::<Vec<_>>())
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
