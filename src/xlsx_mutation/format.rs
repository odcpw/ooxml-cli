use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use super::{
    add_xlsx_range_mutation_commands, render_empty_xlsx_cell_with_attrs, replace_xlsx_dimension,
    resolve_xlsx_sheet_context, validate_xlsx_mutation_output_flags, xlsx_range_destination_json,
};
use crate::{
    CliError, CliResult, RangeBounds, XlsxCellSpan, allocate_relationship_id, attr,
    builtin_num_format_code, check_range_max_cells, col_name, copy_zip_with_part_overrides,
    ensure_content_type_override, local_name, normalize_xl_target, parse_cli_range,
    parse_xlsx_row_spans, range_bounds_ref, rebuild_xlsx_sheet_data, relationship_entries,
    render_xlsx_row, render_xml_attrs, validate, xlsx_ranges_set_temp_path, xlsx_sheet_data_span,
    xlsx_used_range_from_cell_refs, xml_attr_escape, xml_attrs, zip_text,
};

pub(crate) struct XlsxRangesSetFormatOptions<'a> {
    pub(crate) sheet: &'a str,
    pub(crate) range: &'a str,
    pub(crate) preset: Option<&'a str>,
    pub(crate) format_code: Option<&'a str>,
    pub(crate) decimals: i64,
    pub(crate) currency_symbol: Option<&'a str>,
    pub(crate) max_cells: i64,
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
}

#[derive(Clone)]
struct XlsxNumberFormatSpec {
    preset: String,
    format_code: String,
    number_format_id: u32,
    builtin: bool,
}

#[derive(Default)]
struct XlsxRangeFormatStats {
    updated: usize,
    created: usize,
    created_styles: usize,
    style_indexes: BTreeSet<u32>,
}

pub(crate) fn xlsx_ranges_set_format(
    file: &str,
    options: XlsxRangesSetFormatOptions<'_>,
) -> CliResult<Value> {
    if !Path::new(file).exists() {
        return Err(CliError::file_not_found(format!("file not found: {file}")));
    }
    let bounds = parse_cli_range(options.range)?;
    let range = range_bounds_ref(bounds);
    check_range_max_cells(&range, bounds, options.max_cells)?;
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;
    let spec = resolve_xlsx_number_format(
        options.preset,
        options.format_code,
        options.decimals,
        options.currency_symbol,
    )?;

    let (sheet, sheet_part) = resolve_xlsx_sheet_context(file, options.sheet)?;
    let sheet_xml = zip_text(file, &sheet_part)?;
    let (styles_part, rels_override) = resolve_or_add_xlsx_styles_part(file)?;
    let styles_xml = zip_text(file, &styles_part).unwrap_or_else(|_| default_xlsx_styles_xml());
    let (styles_xml, number_format_id) = ensure_xlsx_number_format(styles_xml, &spec)?;
    let (updated_sheet_xml, styles_xml, stats) =
        set_xlsx_range_number_format_xml(&sheet_xml, styles_xml, bounds, number_format_id)?;
    let content_types_xml = ensure_content_type_override(
        zip_text(file, "[Content_Types].xml")?,
        &format!("/{styles_part}"),
        "application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml",
    );

    let output_path = options.out.filter(|value| !value.is_empty());
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

    let mut overrides = BTreeMap::new();
    overrides.insert(sheet_part.clone(), updated_sheet_xml);
    overrides.insert(styles_part.clone(), styles_xml);
    overrides.insert("[Content_Types].xml".to_string(), content_types_xml);
    if let Some(rels_xml) = rels_override {
        overrides.insert("xl/_rels/workbook.xml.rels".to_string(), rels_xml);
    }
    copy_zip_with_part_overrides(file, &readback_path, &overrides)?;
    if !options.no_validate {
        validate(&readback_path, true)?;
    }
    let destination =
        xlsx_range_destination_json(&readback_path, commit_path, &sheet, &sheet_part, &range)?;
    if options.dry_run {
        let _ = fs::remove_file(&readback_path);
    } else if options.in_place || output_path == Some(file) {
        if let Some(backup_path) = options.backup.filter(|value| !value.is_empty()) {
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

    let rows = bounds.row_count();
    let cols = bounds.col_count();
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("sheet".to_string(), json!(sheet.name));
    result.insert("sheetNumber".to_string(), json!(sheet.position));
    result.insert("range".to_string(), json!(range));
    result.insert("rows".to_string(), json!(rows));
    result.insert("cols".to_string(), json!(cols));
    if !spec.preset.is_empty() {
        result.insert("preset".to_string(), json!(spec.preset));
    }
    result.insert("formatCode".to_string(), json!(spec.format_code));
    result.insert("numberFormatId".to_string(), json!(number_format_id));
    result.insert("builtin".to_string(), json!(spec.builtin));
    result.insert("updated".to_string(), json!(stats.updated));
    result.insert("created".to_string(), json!(stats.created));
    result.insert("createdStyles".to_string(), json!(stats.created_styles));
    if !stats.style_indexes.is_empty() {
        result.insert(
            "styleIndexes".to_string(),
            json!(stats.style_indexes.into_iter().collect::<Vec<_>>()),
        );
    }
    if let Some(commit_path) = commit_path {
        result.insert("output".to_string(), json!(commit_path));
    }
    result.insert("dryRun".to_string(), json!(options.dry_run));
    result.insert("destination".to_string(), destination);
    add_xlsx_range_mutation_commands(
        &mut result,
        commit_path,
        &format!("sheetId:{}", sheet.sheet_id),
        &range,
    );
    Ok(Value::Object(result))
}

fn resolve_xlsx_number_format(
    preset: Option<&str>,
    format_code: Option<&str>,
    decimals: i64,
    currency_symbol: Option<&str>,
) -> CliResult<XlsxNumberFormatSpec> {
    let preset = preset.unwrap_or_default().trim().to_ascii_lowercase();
    let format_code = format_code.unwrap_or_default().trim();
    if preset.is_empty() == format_code.is_empty() {
        return Err(CliError::invalid_args(
            "specify exactly one of preset or format code",
        ));
    }
    if !(0..=10).contains(&decimals) {
        return Err(CliError::invalid_args("decimals must be between 0 and 10"));
    }
    if !format_code.is_empty() {
        return Ok(XlsxNumberFormatSpec {
            preset: "custom".to_string(),
            format_code: format_code.to_string(),
            number_format_id: 0,
            builtin: false,
        });
    }
    match preset.as_str() {
        "general" => builtin_xlsx_number_format_spec("general", 0),
        "integer" => builtin_xlsx_number_format_spec("integer", 3),
        "number" => {
            let code = fixed_decimal_format("#,##0", decimals);
            match decimals {
                0 => builtin_xlsx_number_format_spec("number", 3),
                2 => builtin_xlsx_number_format_spec("number", 4),
                _ => custom_xlsx_number_format_spec("number", &code),
            }
        }
        "percent" => {
            let code = format!("{}%", fixed_decimal_format("0", decimals));
            match decimals {
                0 => builtin_xlsx_number_format_spec("percent", 9),
                2 => builtin_xlsx_number_format_spec("percent", 10),
                _ => custom_xlsx_number_format_spec("percent", &code),
            }
        }
        "currency" => {
            let symbol = currency_symbol.unwrap_or("$");
            let code = format!(
                "{}{}",
                xlsx_format_literal(symbol),
                fixed_decimal_format("#,##0", decimals)
            );
            custom_xlsx_number_format_spec("currency", &code)
        }
        "date" => custom_xlsx_number_format_spec("date", "yyyy-mm-dd"),
        "datetime" => custom_xlsx_number_format_spec("datetime", "yyyy-mm-dd h:mm"),
        "text" => builtin_xlsx_number_format_spec("text", 49),
        _ => Err(CliError::invalid_args(format!(
            "invalid preset {:?} (must be integer, number, currency, percent, date, datetime, text, or general)",
            preset
        ))),
    }
}

fn builtin_xlsx_number_format_spec(
    preset: &str,
    number_format_id: u32,
) -> CliResult<XlsxNumberFormatSpec> {
    let code = builtin_num_format_code(number_format_id).ok_or_else(|| {
        CliError::unexpected(format!(
            "unknown built-in number format id {number_format_id}"
        ))
    })?;
    Ok(XlsxNumberFormatSpec {
        preset: preset.to_string(),
        format_code: code.to_string(),
        number_format_id,
        builtin: true,
    })
}

fn custom_xlsx_number_format_spec(preset: &str, code: &str) -> CliResult<XlsxNumberFormatSpec> {
    Ok(XlsxNumberFormatSpec {
        preset: preset.to_string(),
        format_code: code.to_string(),
        number_format_id: 0,
        builtin: false,
    })
}

fn fixed_decimal_format(base: &str, decimals: i64) -> String {
    if decimals == 0 {
        base.to_string()
    } else {
        format!("{base}.{}", "0".repeat(decimals as usize))
    }
}

fn xlsx_format_literal(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn resolve_or_add_xlsx_styles_part(file: &str) -> CliResult<(String, Option<String>)> {
    let rels_part = "xl/_rels/workbook.xml.rels";
    let rels_xml = zip_text(file, rels_part)?;
    let rels = relationship_entries(file, rels_part)?;
    for rel in &rels {
        if rel.target_mode == "External" {
            continue;
        }
        if rel.rel_type
            == "http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles"
        {
            return Ok((normalize_xl_target(&rel.target), None));
        }
    }
    let next_id = allocate_relationship_id(&rels);
    let rel = format!(
        r#"<Relationship Id="{next_id}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>"#
    );
    let updated = if let Some(pos) = rels_xml.rfind("</Relationships>") {
        let mut out = String::with_capacity(rels_xml.len() + rel.len());
        out.push_str(&rels_xml[..pos]);
        out.push_str(&rel);
        out.push_str(&rels_xml[pos..]);
        out
    } else {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{rel}</Relationships>"#
        )
    };
    Ok(("xl/styles.xml".to_string(), Some(updated)))
}

fn default_xlsx_styles_xml() -> String {
    r#"<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><fonts count="1"><font/></fonts><fills count="2"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill></fills><borders count="1"><border/></borders><cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs><cellXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/></cellXfs><cellStyles count="1"><cellStyle name="Normal" xfId="0" builtinId="0"/></cellStyles></styleSheet>"#.to_string()
}

fn ensure_xlsx_number_format(
    styles_xml: String,
    spec: &XlsxNumberFormatSpec,
) -> CliResult<(String, u32)> {
    let styles_xml = ensure_xlsx_style_defaults(styles_xml);
    if spec.builtin {
        return Ok((styles_xml, spec.number_format_id));
    }
    for (id, code) in parse_xlsx_num_formats(&styles_xml) {
        if code == spec.format_code {
            return Ok((styles_xml, id));
        }
    }
    let mut next_id = 164u32;
    for (id, _) in parse_xlsx_num_formats(&styles_xml) {
        if id >= next_id {
            next_id = id + 1;
        }
    }
    let num_fmt = format!(
        r#"<numFmt numFmtId="{next_id}" formatCode="{}"/>"#,
        xml_attr_escape(&spec.format_code)
    );
    let updated = if let Some(span) = element_span_by_local_name(&styles_xml, "numFmts") {
        let mut out = String::with_capacity(styles_xml.len() + num_fmt.len());
        out.push_str(&styles_xml[..span.close_start]);
        out.push_str(&num_fmt);
        out.push_str(&styles_xml[span.close_start..]);
        set_collection_count(out, "numFmts", "numFmt")
    } else {
        insert_xlsx_styles_collection(
            &styles_xml,
            "numFmts",
            &format!(r#"<numFmts count="1">{num_fmt}</numFmts>"#),
        )
    };
    Ok((updated, next_id))
}

fn ensure_xlsx_style_defaults(mut styles_xml: String) -> String {
    if !styles_xml.contains("<styleSheet") {
        return default_xlsx_styles_xml();
    }
    let defaults = [
        ("fonts", r#"<fonts count="1"><font/></fonts>"#),
        (
            "fills",
            r#"<fills count="2"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill></fills>"#,
        ),
        ("borders", r#"<borders count="1"><border/></borders>"#),
        (
            "cellStyleXfs",
            r#"<cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs>"#,
        ),
        (
            "cellXfs",
            r#"<cellXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/></cellXfs>"#,
        ),
        (
            "cellStyles",
            r#"<cellStyles count="1"><cellStyle name="Normal" xfId="0" builtinId="0"/></cellStyles>"#,
        ),
    ];
    for (name, block) in defaults {
        if element_span_by_local_name(&styles_xml, name).is_none() {
            styles_xml = insert_xlsx_styles_collection(&styles_xml, name, block);
        }
    }
    styles_xml
}

fn insert_xlsx_styles_collection(styles_xml: &str, name: &str, block: &str) -> String {
    let target_order = xlsx_styles_collection_order(name);
    for candidate in [
        "numFmts",
        "fonts",
        "fills",
        "borders",
        "cellStyleXfs",
        "cellXfs",
        "cellStyles",
        "dxfs",
        "tableStyles",
        "colors",
        "extLst",
    ] {
        if xlsx_styles_collection_order(candidate) > target_order
            && let Some(span) = element_span_by_local_name(styles_xml, candidate)
        {
            let mut out = String::with_capacity(styles_xml.len() + block.len());
            out.push_str(&styles_xml[..span.start]);
            out.push_str(block);
            out.push_str(&styles_xml[span.start..]);
            return out;
        }
    }
    if let Some(pos) = styles_xml.rfind("</styleSheet>") {
        let mut out = String::with_capacity(styles_xml.len() + block.len());
        out.push_str(&styles_xml[..pos]);
        out.push_str(block);
        out.push_str(&styles_xml[pos..]);
        out
    } else {
        styles_xml.to_string()
    }
}

fn xlsx_styles_collection_order(name: &str) -> u32 {
    match name {
        "numFmts" => 10,
        "fonts" => 20,
        "fills" => 30,
        "borders" => 40,
        "cellStyleXfs" => 50,
        "cellXfs" => 60,
        "cellStyles" => 70,
        "dxfs" => 80,
        "tableStyles" => 90,
        "colors" => 100,
        "extLst" => 110,
        _ => 1000,
    }
}

#[derive(Clone, Copy)]
struct XmlElementSpan {
    start: usize,
    open_end: usize,
    close_start: usize,
}

fn element_span_by_local_name(xml: &str, wanted: &str) -> Option<XmlElementSpan> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == wanted => {
                let open_end = reader.buffer_position() as usize;
                let mut depth = 1usize;
                loop {
                    let inner_before = reader.buffer_position() as usize;
                    match reader.read_event() {
                        Ok(Event::Start(e)) if local_name(e.name().as_ref()) == wanted => {
                            depth += 1;
                        }
                        Ok(Event::End(e)) if local_name(e.name().as_ref()) == wanted => {
                            depth -= 1;
                            if depth == 0 {
                                return Some(XmlElementSpan {
                                    start: before,
                                    open_end,
                                    close_start: inner_before,
                                });
                            }
                        }
                        Ok(Event::Eof) | Err(_) => return None,
                        _ => {}
                    }
                }
            }
            Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == wanted => {
                let end = reader.buffer_position() as usize;
                return Some(XmlElementSpan {
                    start: before,
                    open_end: end,
                    close_start: before,
                });
            }
            Ok(Event::Eof) | Err(_) => return None,
            _ => {}
        }
    }
}

fn parse_xlsx_num_formats(styles_xml: &str) -> Vec<(u32, String)> {
    let mut reader = Reader::from_str(styles_xml);
    reader.config_mut().trim_text(false);
    let mut formats = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "numFmt" =>
            {
                if let (Some(id), Some(code)) = (attr(&e, "numFmtId"), attr(&e, "formatCode"))
                    && let Ok(id) = id.parse::<u32>()
                {
                    formats.push((id, code));
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    formats
}

#[derive(Clone)]
struct XlsxXfEntry {
    attrs: BTreeMap<String, String>,
    inner_xml: String,
}

fn parse_xlsx_cell_xfs(styles_xml: &str) -> CliResult<Vec<XlsxXfEntry>> {
    let Some(parent) = element_span_by_local_name(styles_xml, "cellXfs") else {
        return Ok(Vec::new());
    };
    let fragment = &styles_xml[parent.open_end..parent.close_start];
    let base = parent.open_end;
    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(false);
    let mut entries = Vec::new();
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "xf" => {
                let attrs = xml_attrs(&e);
                let open_end = reader.buffer_position() as usize;
                loop {
                    let inner_before = reader.buffer_position() as usize;
                    match reader.read_event() {
                        Ok(Event::End(e)) if local_name(e.name().as_ref()) == "xf" => {
                            entries.push(XlsxXfEntry {
                                attrs,
                                inner_xml: styles_xml[base + open_end..base + inner_before]
                                    .to_string(),
                            });
                            break;
                        }
                        Ok(Event::Eof) => {
                            return Err(CliError::unexpected("xf has no closing tag"));
                        }
                        Err(err) => return Err(CliError::unexpected(err.to_string())),
                        _ => {}
                    }
                }
            }
            Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == "xf" => {
                let _ = before;
                entries.push(XlsxXfEntry {
                    attrs: xml_attrs(&e),
                    inner_xml: String::new(),
                });
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(entries)
}

fn ensure_xlsx_cell_style(
    styles_xml: String,
    base_style_index: u32,
    number_format_id: u32,
) -> CliResult<(String, u32, bool)> {
    let styles_xml = ensure_xlsx_style_defaults(styles_xml);
    let xfs = parse_xlsx_cell_xfs(&styles_xml)?;
    let base_index = if (base_style_index as usize) < xfs.len() {
        base_style_index
    } else {
        0
    };
    let base = xfs
        .get(base_index as usize)
        .cloned()
        .unwrap_or_else(default_xlsx_xf_entry);
    if xlsx_xf_num_fmt_id(&base.attrs) == number_format_id {
        return Ok((styles_xml, base_index, false));
    }
    let mut attrs = base.attrs.clone();
    for (key, value) in [
        ("fontId", "0"),
        ("fillId", "0"),
        ("borderId", "0"),
        ("xfId", "0"),
    ] {
        attrs
            .entry(key.to_string())
            .or_insert_with(|| value.to_string());
    }
    attrs.insert("numFmtId".to_string(), number_format_id.to_string());
    attrs.insert("applyNumberFormat".to_string(), "1".to_string());
    let candidate = XlsxXfEntry {
        attrs,
        inner_xml: base.inner_xml,
    };
    let candidate_sig = render_xlsx_xf(&candidate);
    for (index, xf) in xfs.iter().enumerate() {
        if render_xlsx_xf(xf) == candidate_sig {
            return Ok((styles_xml, index as u32, false));
        }
    }
    let Some(parent) = element_span_by_local_name(&styles_xml, "cellXfs") else {
        return Err(CliError::unexpected("styles cellXfs not found"));
    };
    let mut out = String::with_capacity(styles_xml.len() + candidate_sig.len());
    out.push_str(&styles_xml[..parent.close_start]);
    out.push_str(&candidate_sig);
    out.push_str(&styles_xml[parent.close_start..]);
    let out = set_collection_count(out, "cellXfs", "xf");
    Ok((out, xfs.len() as u32, true))
}

fn default_xlsx_xf_entry() -> XlsxXfEntry {
    let mut attrs = BTreeMap::new();
    attrs.insert("numFmtId".to_string(), "0".to_string());
    attrs.insert("fontId".to_string(), "0".to_string());
    attrs.insert("fillId".to_string(), "0".to_string());
    attrs.insert("borderId".to_string(), "0".to_string());
    attrs.insert("xfId".to_string(), "0".to_string());
    XlsxXfEntry {
        attrs,
        inner_xml: String::new(),
    }
}

fn render_xlsx_xf(xf: &XlsxXfEntry) -> String {
    if xf.inner_xml.is_empty() {
        format!("<xf{}/>", render_xml_attrs(&xf.attrs))
    } else {
        format!("<xf{}>{}</xf>", render_xml_attrs(&xf.attrs), xf.inner_xml)
    }
}

fn xlsx_xf_num_fmt_id(attrs: &BTreeMap<String, String>) -> u32 {
    attrs
        .get("numFmtId")
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(0)
}

fn set_collection_count(xml: String, parent: &str, child: &str) -> String {
    let count = count_children_in_parent(&xml, parent, child);
    let Some(span) = element_span_by_local_name(&xml, parent) else {
        return xml;
    };
    set_start_tag_count_attr(&xml, span, count)
}

fn count_children_in_parent(xml: &str, parent: &str, child: &str) -> usize {
    let Some(span) = element_span_by_local_name(xml, parent) else {
        return 0;
    };
    let fragment = &xml[span.open_end..span.close_start];
    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(false);
    let mut count = 0usize;
    let mut depth = 0usize;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                if depth == 0 && local_name(e.name().as_ref()) == child {
                    count += 1;
                }
                depth += 1;
            }
            Ok(Event::Empty(e)) => {
                if depth == 0 && local_name(e.name().as_ref()) == child {
                    count += 1;
                }
            }
            Ok(Event::End(_)) => {
                depth = depth.saturating_sub(1);
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    count
}

fn set_start_tag_count_attr(xml: &str, span: XmlElementSpan, count: usize) -> String {
    let open = &xml[span.start..span.open_end];
    let replacement = if let Some(pos) = open.find("count=\"") {
        let value_start = pos + "count=\"".len();
        if let Some(value_end_rel) = open[value_start..].find('"') {
            let value_end = value_start + value_end_rel;
            let mut tag = String::new();
            tag.push_str(&open[..value_start]);
            tag.push_str(&count.to_string());
            tag.push_str(&open[value_end..]);
            tag
        } else {
            open.to_string()
        }
    } else if let Some(pos) = open.rfind("/>") {
        format!("{} count=\"{}\"/>", &open[..pos].trim_end(), count)
    } else if let Some(pos) = open.rfind('>') {
        format!("{} count=\"{}\">", &open[..pos].trim_end(), count)
    } else {
        open.to_string()
    };
    let mut out = String::with_capacity(xml.len() + replacement.len());
    out.push_str(&xml[..span.start]);
    out.push_str(&replacement);
    out.push_str(&xml[span.open_end..]);
    out
}

fn set_xlsx_range_number_format_xml(
    sheet_xml: &str,
    mut styles_xml: String,
    bounds: RangeBounds,
    number_format_id: u32,
) -> CliResult<(String, String, XlsxRangeFormatStats)> {
    let sheet_data = xlsx_sheet_data_span(sheet_xml)?;
    let row_spans = parse_xlsx_row_spans(sheet_xml, sheet_data.as_ref())?;
    let mut stats = XlsxRangeFormatStats::default();
    let mut changed_rows = BTreeMap::<u32, String>::new();
    let mut style_by_base = BTreeMap::<u32, u32>::new();
    let write_bounds = bounds.normalized();
    for row_num in write_bounds.start_row..=write_bounds.end_row {
        let existing_row = row_spans.get(&row_num);
        let mut rendered_cells = existing_row
            .map(|span| {
                span.cells
                    .iter()
                    .map(|(col, cell)| (*col, cell.xml.clone()))
                    .collect::<BTreeMap<u32, String>>()
            })
            .unwrap_or_default();
        let mut row_changed = false;
        for col_num in write_bounds.start_col..=write_bounds.end_col {
            let addr = format!("{}{}", col_name(col_num), row_num);
            let existing_cell = existing_row.and_then(|span| span.cells.get(&col_num));
            let base_style = existing_cell
                .and_then(|cell| cell.attrs.get("s"))
                .and_then(|value| value.parse::<u32>().ok())
                .unwrap_or(0);
            let style_index = if let Some(style_index) = style_by_base.get(&base_style).copied() {
                style_index
            } else {
                let (new_styles_xml, style_index, created) =
                    ensure_xlsx_cell_style(styles_xml, base_style, number_format_id)?;
                styles_xml = new_styles_xml;
                if created {
                    stats.created_styles += 1;
                }
                style_by_base.insert(base_style, style_index);
                style_index
            };
            let cell_xml = if let Some(existing_cell) = existing_cell {
                render_xlsx_existing_cell_with_style(&addr, existing_cell, style_index)
            } else {
                let mut attrs = BTreeMap::new();
                attrs.insert("r".to_string(), addr.clone());
                attrs.insert("s".to_string(), style_index.to_string());
                stats.created += 1;
                render_empty_xlsx_cell_with_attrs(&addr, Some(&attrs))
            };
            rendered_cells.insert(col_num, cell_xml);
            stats.updated += 1;
            stats.style_indexes.insert(style_index);
            row_changed = true;
        }
        if row_changed {
            changed_rows.insert(
                row_num,
                render_xlsx_row(row_num, existing_row, rendered_cells),
            );
        }
    }
    let updated =
        rebuild_xlsx_sheet_data(sheet_xml, sheet_data.as_ref(), &row_spans, &changed_rows)?;
    let used_range = xlsx_used_range_from_cell_refs(&updated);
    Ok((
        replace_xlsx_dimension(&updated, used_range.as_deref()),
        styles_xml,
        stats,
    ))
}

fn render_xlsx_existing_cell_with_style(
    addr: &str,
    cell: &XlsxCellSpan,
    style_index: u32,
) -> String {
    let mut attrs = cell.attrs.clone();
    attrs.insert("r".to_string(), addr.to_string());
    attrs.insert("s".to_string(), style_index.to_string());
    if cell.xml.trim_end().ends_with("/>") {
        return render_empty_xlsx_cell_with_attrs(addr, Some(&attrs));
    }
    if let Some(open_end) = cell.xml.find('>') {
        let mut out = format!("<c{}>", render_xml_attrs(&attrs));
        out.push_str(&cell.xml[open_end + 1..]);
        out
    } else {
        render_empty_xlsx_cell_with_attrs(addr, Some(&attrs))
    }
}
