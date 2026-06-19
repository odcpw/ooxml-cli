use serde_json::{Map, Value, json};

use crate::{
    CliError, CliResult, WorkbookSheet, XlsxCellEntry, build_dense_xlsx_rows,
    build_sparse_xlsx_rows, command_arg, normalize_xl_target, parse_cli_range, relationships,
    resolve_sheet, shared_strings, sheet_cells, sorted_xlsx_cells, used_range_for_cells,
    used_range_json, used_range_ref, workbook_sheets, xlsx_dimension_declared,
    xlsx_merged_cell_count, xlsx_sheet_selectors, xlsx_styles, zip_text,
};
pub(crate) fn xlsx_cells_extract(
    file: &str,
    sheet_selector: &str,
    range: Option<&str>,
    max_rows: u32,
    max_cells: u32,
    include_empty: bool,
) -> CliResult<Value> {
    let workbook = zip_text(file, "xl/workbook.xml")?;
    let sheets = workbook_sheets(&workbook)?;
    let sheet = resolve_sheet(&sheets, sheet_selector)?;
    let rels = relationships(file, "xl/_rels/workbook.xml.rels")?;
    let target = rels
        .get(&sheet.rel_id)
        .ok_or_else(|| CliError::unexpected(format!("missing relationship {}", sheet.rel_id)))?;
    let sheet_part = normalize_xl_target(target);
    let part_uri = format!("/{sheet_part}");
    let shared_strings = shared_strings(file).unwrap_or_default();
    let styles = xlsx_styles(file).unwrap_or_default();
    let sheet_xml = zip_text(file, &sheet_part)?;
    let dimension_declared = xlsx_dimension_declared(&sheet_xml);
    let merged_cell_count = xlsx_merged_cell_count(&sheet_xml);
    let all_cells = sheet_cells(&sheet_xml, &shared_strings, &styles);
    let range_bounds = range.map(parse_cli_range).transpose()?;
    let cells = sorted_xlsx_cells(&all_cells, range_bounds);
    let used_range = used_range_for_cells(&cells);
    let row_count = cells
        .iter()
        .map(|cell| cell.row)
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    let (rows, truncated) = if include_empty {
        build_dense_xlsx_rows(
            &cells,
            range_bounds,
            used_range,
            max_rows,
            max_cells,
            &sheet,
        )
    } else {
        build_sparse_xlsx_rows(&cells, max_rows, max_cells, &sheet)
    };

    let mut sheet_obj = Map::new();
    sheet_obj.insert("number".to_string(), json!(sheet.position));
    sheet_obj.insert("name".to_string(), json!(sheet.name));
    sheet_obj.insert("sheetId".to_string(), json!(sheet.sheet_id.to_string()));
    sheet_obj.insert("state".to_string(), json!("visible"));
    sheet_obj.insert("partUri".to_string(), json!(part_uri));
    sheet_obj.insert(
        "primarySelector".to_string(),
        json!(format!("sheetId:{}", sheet.sheet_id)),
    );
    sheet_obj.insert(
        "selectors".to_string(),
        json!(xlsx_sheet_selectors(
            &sheet.name,
            sheet.sheet_id,
            sheet.position,
            &sheet.rel_id,
            &format!("/{sheet_part}")
        )),
    );
    if let Some(dimension_declared) = dimension_declared.filter(|value| !value.is_empty()) {
        sheet_obj.insert("dimensionDeclared".to_string(), json!(dimension_declared));
    }
    sheet_obj.insert("usedRange".to_string(), used_range_json(used_range));
    sheet_obj.insert("rowCount".to_string(), json!(row_count));
    sheet_obj.insert("cellCount".to_string(), json!(cells.len()));
    sheet_obj.insert("mergedCellCount".to_string(), json!(merged_cell_count));
    if !rows.is_empty() {
        sheet_obj.insert("rows".to_string(), Value::Array(rows));
    }
    if truncated {
        sheet_obj.insert("truncated".to_string(), json!(true));
    }

    Ok(json!({
        "file": file,
        "sheet": Value::Object(sheet_obj),
    }))
}

pub(crate) fn xlsx_sheets_show(file: &str, sheet_selector: Option<&str>) -> CliResult<Value> {
    let workbook = zip_text(file, "xl/workbook.xml")?;
    let sheets = workbook_sheets(&workbook)?;
    let rels = relationships(file, "xl/_rels/workbook.xml.rels")?;
    let selected = if let Some(selector) = sheet_selector.filter(|selector| !selector.is_empty()) {
        vec![resolve_sheet(&sheets, selector)?]
    } else {
        sheets
    };
    if selected.is_empty() {
        return Err(CliError::invalid_args("workbook has no worksheet sheets"));
    }
    let shared_strings = shared_strings(file).unwrap_or_default();
    let styles = xlsx_styles(file).unwrap_or_default();
    let mut reports = Vec::new();
    for sheet in selected {
        let target = rels.get(&sheet.rel_id).ok_or_else(|| {
            CliError::unexpected(format!("missing relationship {}", sheet.rel_id))
        })?;
        let sheet_part = normalize_xl_target(target);
        let sheet_xml = zip_text(file, &sheet_part)?;
        let cells = sorted_xlsx_cells(&sheet_cells(&sheet_xml, &shared_strings, &styles), None);
        reports.push(xlsx_sheet_show_item(
            file,
            &sheet,
            &sheet_part,
            &sheet_xml,
            &cells,
        ));
    }
    Ok(json!({
        "file": file,
        "validateCommand": format!("ooxml validate --strict {}", command_arg(file)),
        "sheets": reports,
    }))
}

fn xlsx_sheet_show_item(
    file: &str,
    sheet: &WorkbookSheet,
    sheet_part: &str,
    sheet_xml: &str,
    cells: &[XlsxCellEntry],
) -> Value {
    let part_uri = format!("/{sheet_part}");
    let used_range = used_range_for_cells(cells);
    let selector = format!("sheetId:{}", sheet.sheet_id);
    let used_range_ref = used_range_ref(used_range);
    let row_count = cells
        .iter()
        .map(|cell| cell.row)
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    let mut item = Map::new();
    item.insert("number".to_string(), json!(sheet.position));
    item.insert("name".to_string(), json!(sheet.name));
    item.insert("sheetId".to_string(), json!(sheet.sheet_id.to_string()));
    item.insert("state".to_string(), json!(sheet.state));
    item.insert("partUri".to_string(), json!(part_uri));
    item.insert("primarySelector".to_string(), json!(selector));
    item.insert(
        "selectors".to_string(),
        json!(xlsx_sheet_selectors(
            &sheet.name,
            sheet.sheet_id,
            sheet.position,
            &sheet.rel_id,
            &format!("/{sheet_part}")
        )),
    );
    if let Some(dimension_declared) =
        xlsx_dimension_declared(sheet_xml).filter(|value| !value.is_empty())
    {
        item.insert("dimensionDeclared".to_string(), json!(dimension_declared));
    }
    item.insert("usedRange".to_string(), used_range_json(used_range));
    item.insert("rowCount".to_string(), json!(row_count));
    item.insert("cellCount".to_string(), json!(cells.len()));
    item.insert(
        "mergedCellCount".to_string(),
        json!(xlsx_merged_cell_count(sheet_xml)),
    );
    item.insert(
        "tablesListCommand".to_string(),
        json!(format!(
            "ooxml --json xlsx tables list {} --sheet {}",
            command_arg(file),
            command_arg(&selector)
        )),
    );
    item.insert(
        "setCellCommandTemplate".to_string(),
        json!(format!(
            "ooxml --json xlsx cells set {} --sheet {} --cell A1 --value VALUE --out out.xlsx",
            command_arg(file),
            command_arg(&selector)
        )),
    );
    if let Some(range_ref) = used_range_ref {
        item.insert(
            "cellsExtractCommand".to_string(),
            json!(format!(
                "ooxml --json xlsx cells extract {} --sheet {} --range {}",
                command_arg(file),
                command_arg(&selector),
                command_arg(&range_ref)
            )),
        );
        item.insert(
            "rangesExportCommand".to_string(),
            json!(format!(
                "ooxml --json xlsx ranges export {} --sheet {} --range {} --include-types",
                command_arg(file),
                command_arg(&selector),
                command_arg(&range_ref)
            )),
        );
        item.insert(
            "setRangeCommandTemplate".to_string(),
            json!(format!(
                "ooxml --json xlsx ranges set {} --sheet {} --range {} --data-format json --values-file values.json --out out.xlsx",
                command_arg(file),
                command_arg(&selector),
                command_arg(&range_ref)
            )),
        );
    }
    Value::Object(item)
}

pub(crate) fn xlsx_sheets_list(file: &str) -> CliResult<Value> {
    let workbook = zip_text(file, "xl/workbook.xml")?;
    let sheets = workbook_sheets(&workbook)?;
    let rels = relationships(file, "xl/_rels/workbook.xml.rels")?;
    let values: Vec<Value> = sheets
        .iter()
        .map(|sheet| {
            let target = rels.get(&sheet.rel_id).cloned().unwrap_or_default();
            let part = normalize_xl_target(&target);
            let part_uri = format!("/{part}");
            let primary_selector = format!("sheetId:{}", sheet.sheet_id);
            json!({
                "number": sheet.position,
                "position": sheet.position,
                "name": sheet.name,
                "sheetId": sheet.sheet_id.to_string(),
                "state": sheet.state,
                "relationshipId": sheet.rel_id,
                "partUri": part_uri,
                "relationshipType": "http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet",
                "primarySelector": primary_selector,
                "selectors": xlsx_sheet_selectors(&sheet.name, sheet.sheet_id, sheet.position, &sheet.rel_id, &part_uri),
                "handle": format!("H:xlsx/ws:{}", sheet.sheet_id),
                "showCommand": format!("ooxml --json xlsx sheets show {} --sheet {}", command_arg(file), command_arg(&primary_selector)),
                "tablesListCommand": format!("ooxml --json xlsx tables list {} --sheet {}", command_arg(file), command_arg(&primary_selector)),
            })
        })
        .collect();
    Ok(json!({
        "file": file,
        "validateCommand": format!("ooxml validate --strict {}", command_arg(file)),
        "sheets": values,
    }))
}
