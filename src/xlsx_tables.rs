use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Map, Value, json};

use crate::{
    CliError, CliResult, XlsxRangeExportOptions, add_selector, attr, attr_exact, command_arg,
    local_name, normalize_xl_target, parse_range, relationship_entries, relationships_part_for,
    require_json_data_format, resolve_relationship_target, resolve_sheet, selector_candidates,
    workbook_sheets, xlsx_range_export_with_options, zip_text,
};

#[derive(Clone, Default)]
pub(crate) struct XlsxTableColumn {
    pub(crate) id: u32,
    pub(crate) name: String,
}

#[derive(Clone, Default)]
pub(crate) struct XlsxTableRef {
    pub(crate) number: u32,
    pub(crate) sheet: String,
    pub(crate) sheet_number: u32,
    pub(crate) sheet_part_uri: String,
    pub(crate) relationship_id: String,
    pub(crate) part_uri: String,
    pub(crate) id: u32,
    pub(crate) name: String,
    pub(crate) display_name: String,
    pub(crate) primary_selector: String,
    pub(crate) selectors: Vec<String>,
    pub(crate) range: String,
    pub(crate) rows: u32,
    pub(crate) cols: u32,
    pub(crate) header_row_count: u32,
    pub(crate) data_row_count: u32,
    pub(crate) totals_row_count: u32,
    pub(crate) style_name: String,
    pub(crate) columns: Vec<XlsxTableColumn>,
}

impl XlsxTableRef {
    pub(crate) fn apply_selectors(&mut self) {
        self.primary_selector = if self.id > 0 {
            format!("tableId:{}", self.id)
        } else if self.number > 0 {
            format!("table:{}", self.number)
        } else if !self.display_name.trim().is_empty() {
            format!("table:{}", self.display_name)
        } else {
            String::new()
        };
        let mut selectors = Vec::new();
        add_selector(&mut selectors, self.primary_selector.clone());
        if self.number > 0 {
            add_selector(&mut selectors, format!("table:{}", self.number));
            add_selector(&mut selectors, format!("#{}", self.number));
        }
        if !self.display_name.trim().is_empty() {
            add_selector(&mut selectors, format!("table:{}", self.display_name));
            add_selector(&mut selectors, format!("displayName:{}", self.display_name));
            add_selector(&mut selectors, self.display_name.clone());
        }
        if !self.name.trim().is_empty() {
            add_selector(&mut selectors, format!("name:{}", self.name));
            add_selector(&mut selectors, self.name.clone());
        }
        if self.id > 0 {
            add_selector(&mut selectors, format!("tableId:{}", self.id));
            add_selector(&mut selectors, format!("id:{}", self.id));
        }
        if !self.relationship_id.trim().is_empty() {
            add_selector(&mut selectors, format!("rId:{}", self.relationship_id));
            add_selector(&mut selectors, format!("rid:{}", self.relationship_id));
        }
        if !self.part_uri.trim().is_empty() {
            add_selector(&mut selectors, format!("part:{}", self.part_uri));
        }
        self.selectors = selectors;
    }

    fn to_json_object(&self) -> Map<String, Value> {
        let mut object = Map::new();
        object.insert("number".to_string(), json!(self.number));
        object.insert("sheet".to_string(), json!(self.sheet));
        object.insert("sheetNumber".to_string(), json!(self.sheet_number));
        object.insert("sheetPartUri".to_string(), json!(self.sheet_part_uri));
        object.insert("relationshipId".to_string(), json!(self.relationship_id));
        object.insert("partUri".to_string(), json!(self.part_uri));
        object.insert("id".to_string(), json!(self.id));
        if !self.name.is_empty() {
            object.insert("name".to_string(), json!(self.name));
        }
        object.insert("displayName".to_string(), json!(self.display_name));
        if !self.primary_selector.is_empty() {
            object.insert("primarySelector".to_string(), json!(self.primary_selector));
        }
        if !self.selectors.is_empty() {
            object.insert("selectors".to_string(), json!(self.selectors));
        }
        object.insert("range".to_string(), json!(self.range));
        object.insert("rows".to_string(), json!(self.rows));
        object.insert("cols".to_string(), json!(self.cols));
        object.insert("headerRowCount".to_string(), json!(self.header_row_count));
        object.insert("dataRowCount".to_string(), json!(self.data_row_count));
        object.insert("totalsRowCount".to_string(), json!(self.totals_row_count));
        if !self.style_name.is_empty() {
            object.insert("styleName".to_string(), json!(self.style_name));
        }
        if !self.columns.is_empty() {
            object.insert(
                "columns".to_string(),
                json!(
                    self.columns
                        .iter()
                        .map(|column| json!({"id": column.id, "name": column.name}))
                        .collect::<Vec<_>>()
                ),
            );
        }
        object
    }
}

pub(crate) fn xlsx_tables_list(file: &str, sheet_selector: Option<&str>) -> CliResult<Value> {
    let tables = xlsx_tables(file, sheet_selector)?;
    Ok(json!({
        "file": file,
        "validateCommand": format!("ooxml validate --strict {}", command_arg(file)),
        "tables": tables.iter().map(|table| xlsx_table_item_json(file, table)).collect::<Vec<_>>(),
    }))
}

pub(crate) fn xlsx_tables_show(
    file: &str,
    sheet_selector: Option<&str>,
    table_selector: Option<&str>,
) -> CliResult<Value> {
    let tables = xlsx_tables(file, sheet_selector)?;
    let table = select_xlsx_table(&tables, table_selector.unwrap_or_default())?;
    Ok(json!({
        "file": file,
        "validateCommand": format!("ooxml validate --strict {}", command_arg(file)),
        "tables": [xlsx_table_item_json(file, &table)],
    }))
}

pub(crate) struct XlsxTableExportOptions<'a> {
    pub(crate) data_format: Option<&'a str>,
    pub(crate) data_out: Option<&'a str>,
    pub(crate) max_cells: i64,
    pub(crate) include_types: bool,
    pub(crate) include_formulas: bool,
}

pub(crate) fn xlsx_tables_export(
    file: &str,
    sheet_selector: Option<&str>,
    table_selector: Option<&str>,
    options: XlsxTableExportOptions<'_>,
) -> CliResult<Value> {
    require_json_data_format(options.data_format)?;
    let tables = xlsx_tables(file, sheet_selector)?;
    let table = select_xlsx_table(&tables, table_selector.unwrap_or_default())?;
    xlsx_range_export_with_options(
        file,
        &table.sheet,
        &table.range,
        XlsxRangeExportOptions {
            include_types: options.include_types,
            include_formulas: options.include_formulas,
            include_formats: false,
            data_out: options.data_out,
            max_cells: options.max_cells,
        },
    )
}

pub(crate) fn xlsx_tables(
    file: &str,
    sheet_selector: Option<&str>,
) -> CliResult<Vec<XlsxTableRef>> {
    let workbook = zip_text(file, "xl/workbook.xml")?;
    let sheets = workbook_sheets(&workbook)?;
    let workbook_rels = relationship_entries(file, "xl/_rels/workbook.xml.rels")?;
    let selected = if let Some(selector) = sheet_selector.filter(|selector| !selector.is_empty()) {
        vec![resolve_sheet(&sheets, selector)?]
    } else {
        sheets
    };
    let mut tables = Vec::new();
    for sheet in selected {
        let Some(sheet_rel) = workbook_rels.iter().find(|rel| rel.id == sheet.rel_id) else {
            return Err(CliError::unexpected(format!(
                "missing relationship {}",
                sheet.rel_id
            )));
        };
        let sheet_part = normalize_xl_target(&sheet_rel.target);
        if !sheet_rel.rel_type.ends_with("/worksheet") {
            continue;
        }
        let sheet_xml = zip_text(file, &sheet_part)?;
        let table_relationship_ids = xlsx_table_relationship_ids(&sheet_xml)?;
        if table_relationship_ids.is_empty() {
            continue;
        }
        let sheet_rels = relationship_entries(file, &relationships_part_for(&sheet_part))?;
        for relationship_id in table_relationship_ids {
            let Some(table_rel) = sheet_rels.iter().find(|rel| rel.id == relationship_id) else {
                return Err(CliError::unexpected(format!(
                    "worksheet /{sheet_part} table relationship {relationship_id} not found"
                )));
            };
            if table_rel.target_mode == "External" {
                return Err(CliError::unexpected(format!(
                    "worksheet /{sheet_part} table relationship {relationship_id} is external"
                )));
            }
            if !table_rel.rel_type.ends_with("/table") {
                return Err(CliError::unexpected(format!(
                    "worksheet /{sheet_part} relationship {relationship_id} is {}, expected table",
                    table_rel.rel_type
                )));
            }
            let table_part =
                resolve_relationship_target(&format!("/{sheet_part}"), &table_rel.target);
            let table_part = table_part.trim_start_matches('/').to_string();
            let table_xml = zip_text(file, &table_part)?;
            let mut table = parse_xlsx_table_part(&table_xml, &format!("/{table_part}"))?;
            table.number = tables.len() as u32 + 1;
            table.sheet = sheet.name.clone();
            table.sheet_number = sheet.position;
            table.sheet_part_uri = format!("/{sheet_part}");
            table.relationship_id = relationship_id;
            table.part_uri = format!("/{table_part}");
            table.apply_selectors();
            tables.push(table);
        }
    }
    Ok(tables)
}

fn xlsx_table_relationship_ids(sheet_xml: &str) -> CliResult<Vec<String>> {
    let mut reader = Reader::from_str(sheet_xml);
    reader.config_mut().trim_text(true);
    let mut in_table_parts = false;
    let mut ids = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "tableParts" => {
                in_table_parts = true;
            }
            Ok(Event::Empty(e))
                if in_table_parts && local_name(e.name().as_ref()) == "tablePart" =>
            {
                if let Some(id) = attr_exact(&e, "r:id") {
                    ids.push(id);
                }
            }
            Ok(Event::Start(e))
                if in_table_parts && local_name(e.name().as_ref()) == "tablePart" =>
            {
                if let Some(id) = attr_exact(&e, "r:id") {
                    ids.push(id);
                }
            }
            Ok(Event::End(e)) if local_name(e.name().as_ref()) == "tableParts" => {
                in_table_parts = false;
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(ids)
}

pub(crate) fn parse_xlsx_table_part(xml: &str, part_uri: &str) -> CliResult<XlsxTableRef> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut table = XlsxTableRef::default();
    let mut saw_table = false;
    let mut in_table_columns = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "table" =>
            {
                saw_table = true;
                table.id = parse_optional_u32(attr(&e, "id").as_deref(), 0);
                table.name = attr(&e, "name").unwrap_or_default();
                table.display_name = attr(&e, "displayName").unwrap_or_else(|| table.name.clone());
                table.range = attr(&e, "ref").unwrap_or_default();
                let bounds = parse_range(&table.range).map_err(|err| {
                    CliError::unexpected(format!(
                        "invalid table ref {:?} in {part_uri}: {}",
                        table.range, err.message
                    ))
                })?;
                table.rows = bounds.row_count();
                table.cols = bounds.col_count();
                table.header_row_count =
                    parse_optional_u32(attr(&e, "headerRowCount").as_deref(), 1);
                table.totals_row_count =
                    parse_optional_u32(attr(&e, "totalsRowCount").as_deref(), 0);
                table.data_row_count = table
                    .rows
                    .saturating_sub(table.header_row_count)
                    .saturating_sub(table.totals_row_count);
            }
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "tableColumns" => {
                in_table_columns = true;
            }
            Ok(Event::End(e)) if local_name(e.name().as_ref()) == "tableColumns" => {
                in_table_columns = false;
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if in_table_columns && local_name(e.name().as_ref()) == "tableColumn" =>
            {
                table.columns.push(XlsxTableColumn {
                    id: parse_optional_u32(attr(&e, "id").as_deref(), 0),
                    name: attr(&e, "name").unwrap_or_default(),
                });
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "tableStyleInfo" =>
            {
                table.style_name = attr(&e, "name").unwrap_or_default();
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    if !saw_table {
        return Err(CliError::unexpected(format!(
            "table part {part_uri} root element not found"
        )));
    }
    if table.display_name.is_empty() {
        table.display_name = table.name.clone();
    }
    table.part_uri = part_uri.to_string();
    table.apply_selectors();
    Ok(table)
}

fn xlsx_table_item_json(file: &str, table: &XlsxTableRef) -> Value {
    let mut object = table.to_json_object();
    let table_selector = xlsx_table_selector(table);
    let sheet_selector = xlsx_table_sheet_selector(table);
    object.insert(
        "showCommand".to_string(),
        json!(xlsx_table_show_command(
            file,
            &sheet_selector,
            &table_selector
        )),
    );
    object.insert(
        "exportCommand".to_string(),
        json!(xlsx_table_export_command(
            file,
            &sheet_selector,
            &table_selector
        )),
    );
    object.insert(
        "appendRowsCommandTemplate".to_string(),
        json!(xlsx_table_append_rows_command_template(
            file,
            &sheet_selector,
            &table_selector
        )),
    );
    object.insert(
        "appendRecordsCommandTemplate".to_string(),
        json!(xlsx_table_append_records_command_template(
            file,
            &sheet_selector,
            &table_selector,
            &table.range
        )),
    );
    object.insert(
        "pptxUpdateTableCommandTemplate".to_string(),
        json!(xlsx_pptx_update_table_from_table_template(
            file,
            &sheet_selector,
            &table_selector,
            &table.range
        )),
    );
    object.insert(
        "pptxPlaceTableCommandTemplate".to_string(),
        json!(xlsx_pptx_place_table_from_table_template(
            file,
            &sheet_selector,
            &table_selector,
            &table.range
        )),
    );
    if !table.sheet.is_empty() && !table.range.is_empty() {
        object.insert(
            "pptxReplaceTextCommandTemplate".to_string(),
            json!(xlsx_pptx_replace_text_from_range_template(
                file,
                &table.sheet,
                &table.range
            )),
        );
    }
    Value::Object(object)
}

pub(crate) fn select_xlsx_table(
    tables: &[XlsxTableRef],
    selector: &str,
) -> CliResult<XlsxTableRef> {
    if tables.is_empty() {
        return Err(CliError::invalid_args("workbook has no tables"));
    }
    let selector = selector.trim();
    if selector.is_empty() {
        if tables.len() == 1 {
            return Ok(tables[0].clone());
        }
        return Err(CliError::invalid_args(
            "--table is required when workbook has multiple tables",
        ));
    }
    for table in tables {
        if table
            .selectors
            .iter()
            .any(|candidate| candidate == selector)
        {
            return Ok(table.clone());
        }
    }
    if let Ok(number) = selector.parse::<u32>() {
        if number >= 1 && (number as usize) <= tables.len() {
            return Ok(tables[number as usize - 1].clone());
        }
        return Err(CliError::target_not_found(format!(
            "table {number} is out of range (1-{})",
            tables.len()
        )));
    }
    let candidates = selector_candidates(
        &tables
            .iter()
            .map(|table| (table.primary_selector.as_str(), table.selectors.as_slice()))
            .collect::<Vec<_>>(),
        selector,
        3,
    );
    let mut message = format!("table not found: {selector}");
    if !candidates.is_empty() {
        message.push_str(&format!("; did you mean: {}", candidates.join(", ")));
    }
    message.push_str("; discover with `ooxml --json xlsx tables list <file>`");
    Err(CliError::target_not_found(message))
}

fn xlsx_table_selector(table: &XlsxTableRef) -> String {
    if !table.primary_selector.is_empty() {
        table.primary_selector.clone()
    } else if !table.display_name.is_empty() {
        table.display_name.clone()
    } else if table.number > 0 {
        format!("table:{}", table.number)
    } else {
        "1".to_string()
    }
}

fn xlsx_table_sheet_selector(table: &XlsxTableRef) -> String {
    if !table.sheet.is_empty() {
        table.sheet.clone()
    } else if table.sheet_number > 0 {
        format!("sheet:{}", table.sheet_number)
    } else {
        String::new()
    }
}

fn xlsx_table_show_command(file: &str, sheet_selector: &str, table_selector: &str) -> String {
    xlsx_source_command(
        vec!["ooxml", "--json", "xlsx", "tables", "show", file],
        &[("--sheet", sheet_selector), ("--table", table_selector)],
    )
}

fn xlsx_table_export_command(file: &str, sheet_selector: &str, table_selector: &str) -> String {
    let mut command = xlsx_source_command(
        vec!["ooxml", "--json", "xlsx", "tables", "export", file],
        &[("--sheet", sheet_selector), ("--table", table_selector)],
    );
    command.push_str(" --include-types");
    command
}

fn xlsx_table_append_rows_command_template(
    file: &str,
    sheet_selector: &str,
    table_selector: &str,
) -> String {
    let mut command = xlsx_source_command(
        vec!["ooxml", "--json", "xlsx", "tables", "append-rows", file],
        &[("--sheet", sheet_selector), ("--table", table_selector)],
    );
    command.push_str(" --values-file rows.json --out out.xlsx");
    command
}

fn xlsx_table_append_records_command_template(
    file: &str,
    sheet_selector: &str,
    table_selector: &str,
    expect_range: &str,
) -> String {
    let mut command = xlsx_source_command(
        vec!["ooxml", "--json", "xlsx", "tables", "append-records", file],
        &[
            ("--sheet", sheet_selector),
            ("--table", table_selector),
            ("--expect-range", expect_range),
        ],
    );
    command.push_str(" --records-file records.json --out out.xlsx");
    command
}

fn xlsx_pptx_update_table_from_table_template(
    file: &str,
    sheet_selector: &str,
    table_selector: &str,
    expect_range: &str,
) -> String {
    let mut command = xlsx_source_command(
        vec![
            "ooxml",
            "--json",
            "pptx",
            "tables",
            "update-from-xlsx",
            "deck.pptx",
            "--workbook",
            file,
        ],
        &[
            ("--sheet", sheet_selector),
            ("--table", table_selector),
            ("--expect-source-range", expect_range),
        ],
    );
    command.push_str(" --slide 1 --target table:1 --out out.pptx");
    command
}

fn xlsx_pptx_place_table_from_table_template(
    file: &str,
    sheet_selector: &str,
    table_selector: &str,
    expect_range: &str,
) -> String {
    let mut command = xlsx_source_command(
        vec![
            "ooxml",
            "--json",
            "pptx",
            "place",
            "table-from-xlsx",
            "deck.pptx",
            "--workbook",
            file,
        ],
        &[
            ("--sheet", sheet_selector),
            ("--table", table_selector),
            ("--expect-source-range", expect_range),
        ],
    );
    command.push_str(" --slide 1 --x 0 --y 0 --cx 4000000 --out out.pptx");
    command
}

fn xlsx_pptx_replace_text_from_range_template(
    file: &str,
    sheet_selector: &str,
    range: &str,
) -> String {
    let mut command = xlsx_source_command(
        vec![
            "ooxml",
            "--json",
            "pptx",
            "replace",
            "text-from-xlsx",
            "deck.pptx",
            "--workbook",
            file,
        ],
        &[("--sheet", sheet_selector), ("--range", range)],
    );
    command.push_str(" --slide 1 --target title --out out.pptx");
    command
}

pub(crate) fn xlsx_source_command(args: Vec<&str>, flags: &[(&str, &str)]) -> String {
    let mut args = args.into_iter().map(command_arg).collect::<Vec<_>>();
    for (name, value) in flags {
        if !value.trim().is_empty() {
            args.push((*name).to_string());
            args.push(command_arg(value));
        }
    }
    args.join(" ")
}

fn parse_optional_u32(value: Option<&str>, fallback: u32) -> u32 {
    value
        .filter(|value| !value.trim().is_empty())
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(fallback)
}
