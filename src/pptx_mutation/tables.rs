use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::fs;

use crate::{
    CliError, CliResult, XmlNamedRange, attr, attr_exact, command_arg, copy_zip_with_part_override,
    local_name, package_mutation_temp_path, package_type, pptx_tables_show,
    relationship_entries_from_xml, resolve_relationship_target, validate,
    validate_xlsx_mutation_output_flags, xml_direct_child_ranges, zip_text,
};

#[derive(Clone)]
struct PptxTableMutationOptions {
    out: Option<String>,
    backup: Option<String>,
    dry_run: bool,
    in_place: bool,
    no_validate: bool,
}

#[derive(Clone)]
struct PptxSlideRef {
    part: String,
}

#[derive(Clone, Copy)]
struct XmlSpan {
    start: usize,
    end: usize,
}

struct DeleteRowMutation {
    slide_part: String,
    updated_xml: String,
    resolved_table_id: u32,
    cell_count: usize,
}

pub(crate) fn pptx_tables_delete_row(file: &str, args: &[String]) -> CliResult<Value> {
    let slide = crate::parse_i64_flag(args, "--slide")?.unwrap_or(0);
    let row = crate::parse_i64_flag(args, "--row")?.unwrap_or(0);
    if slide < 1 {
        return Err(CliError::invalid_args("--slide must be >= 1"));
    }
    if row < 1 {
        return Err(CliError::invalid_args("--row must be >= 1"));
    }
    let table_id = crate::parse_i64_flag(args, "--table-id")?.unwrap_or(0);
    if table_id < 0 {
        return Err(CliError::invalid_args(
            "--table-id must be a positive integer",
        ));
    }
    let target = crate::parse_string_flag(args, "--target")?;
    if table_id > 0 && target.as_deref().unwrap_or_default().trim() != "" {
        return Err(CliError::invalid_args(
            "specify only one of --target or --table-id",
        ));
    }
    if table_id == 0 && target.as_deref().unwrap_or_default().trim() == "" {
        return Err(CliError::invalid_args(
            "must specify --target or --table-id",
        ));
    }
    let options = parse_table_mutation_options(args)?;
    delete_pptx_table_row(
        file,
        slide as u32,
        table_id as u32,
        target.as_deref(),
        row as usize,
        options,
    )
}

fn parse_table_mutation_options(args: &[String]) -> CliResult<PptxTableMutationOptions> {
    let out = crate::parse_string_flag(args, "--out")?;
    let backup = crate::parse_string_flag(args, "--backup")?;
    let dry_run = crate::has_flag(args, "--dry-run");
    let in_place = crate::has_flag(args, "--in-place");
    let no_validate = crate::has_flag(args, "--no-validate");
    validate_xlsx_mutation_output_flags(out.as_deref(), in_place, backup.as_deref(), dry_run)?;
    Ok(PptxTableMutationOptions {
        out,
        backup,
        dry_run,
        in_place,
        no_validate,
    })
}

fn delete_pptx_table_row(
    file: &str,
    slide: u32,
    table_id: u32,
    target: Option<&str>,
    row: usize,
    options: PptxTableMutationOptions,
) -> CliResult<Value> {
    let detected = package_type(file)?;
    if detected != "pptx" {
        return Err(CliError::unsupported_type(format!(
            "file is not a PPTX document (detected: {detected})"
        )));
    }
    let resolved_table_id = if table_id > 0 {
        table_id
    } else {
        resolve_pptx_table_target_for_mutation(file, slide, target)?
    };
    let mutation = build_delete_row_mutation(file, slide, resolved_table_id, row)?;
    let output_path = table_mutation_output_path(file, &options);
    let staged_path =
        stage_table_mutation(file, &mutation.slide_part, &mutation.updated_xml, &options)?;
    let mut destination = read_table_destination(
        &staged_path,
        slide,
        mutation.resolved_table_id,
        output_path.as_deref(),
    )?;
    let result = delete_row_result_json(
        file,
        slide,
        row,
        &mutation,
        output_path.as_deref(),
        &mut destination,
    );
    finish_table_mutation(file, &staged_path, &options, output_path.as_deref())?;
    Ok(result)
}

fn resolve_pptx_table_target_for_mutation(
    file: &str,
    slide: u32,
    target: Option<&str>,
) -> CliResult<u32> {
    let show = pptx_tables_show(file, slide, 0, target, false)?;
    let tables = show
        .get("tables")
        .and_then(Value::as_array)
        .ok_or_else(|| CliError::unexpected("table readback missing tables array"))?;
    let table = match tables.as_slice() {
        [table] => table,
        [] => {
            return Err(CliError::target_not_found("target not found: table"));
        }
        _ => {
            return Err(CliError::invalid_args(
                "--target must resolve to exactly one table",
            ));
        }
    };
    table
        .get("shapeId")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| CliError::unexpected("table readback missing shapeId"))
}

fn build_delete_row_mutation(
    file: &str,
    slide: u32,
    table_id: u32,
    row: usize,
) -> CliResult<DeleteRowMutation> {
    let slides = pptx_slide_refs_for_table_mutation(file)?;
    let slide_ref = slides.get(slide as usize - 1).ok_or_else(|| {
        CliError::invalid_args(format!(
            "slide number {slide} out of range (1-{})",
            slides.len()
        ))
    })?;
    let slide_xml = zip_text(file, &slide_ref.part)?;
    let table_span = find_table_span_for_shape(&slide_xml, table_id)?.ok_or_else(|| {
        CliError::target_not_found(format!(
            "target not found: table with ID {table_id} not found"
        ))
    })?;
    let (updated_xml, cell_count) = delete_table_row_from_slide_xml(&slide_xml, table_span, row)?;
    Ok(DeleteRowMutation {
        slide_part: slide_ref.part.clone(),
        updated_xml,
        resolved_table_id: table_id,
        cell_count,
    })
}

fn delete_table_row_from_slide_xml(
    slide_xml: &str,
    table_span: XmlSpan,
    row: usize,
) -> CliResult<(String, usize)> {
    let table_fragment = &slide_xml[table_span.start..table_span.end];
    let (content_start, content_end) = element_content_bounds(table_fragment)?;
    let rows: Vec<XmlNamedRange> =
        xml_direct_child_ranges(table_fragment, content_start, content_end)?
            .into_iter()
            .filter(|child| child.kind == "tr")
            .collect();
    let row_range = rows
        .get(row - 1)
        .ok_or_else(|| CliError::target_not_found("target not found: row index out of range"))?;
    if rows.len() <= 1 {
        return Err(CliError::invalid_args("cannot delete last row"));
    }

    let row_fragment = &table_fragment[row_range.start..row_range.end];
    let (row_content_start, row_content_end) = element_content_bounds(row_fragment)?;
    let cells: Vec<XmlNamedRange> =
        xml_direct_child_ranges(row_fragment, row_content_start, row_content_end)?
            .into_iter()
            .filter(|child| child.kind == "tc")
            .collect();
    for cell in &cells {
        reject_unsafe_row_delete_cell(&row_fragment[cell.start..cell.end], row - 1)?;
    }

    let global_start = table_span.start + row_range.start;
    let global_end = table_span.start + row_range.end;
    let mut updated =
        String::with_capacity(slide_xml.len().saturating_sub(global_end - global_start));
    updated.push_str(&slide_xml[..global_start]);
    updated.push_str(&slide_xml[global_end..]);
    Ok((updated, cells.len()))
}

fn reject_unsafe_row_delete_cell(cell_fragment: &str, row_index: usize) -> CliResult<()> {
    let attrs = first_element_attrs(cell_fragment)?;
    if let Some(row_span) = attrs
        .get("rowSpan")
        .and_then(|value| value.parse::<u32>().ok())
        && row_span > 1
    {
        return Err(CliError::invalid_args(format!(
            "cannot delete row {row_index}: cell contains vertical merge extending into row(s) below"
        )));
    }
    if attrs.get("vMerge").map(String::as_str) == Some("1") {
        return Err(CliError::invalid_args(format!(
            "cannot delete row {row_index}: cell is part of a vertical merge extending from above"
        )));
    }
    Ok(())
}

fn read_table_destination(
    readback_path: &str,
    slide: u32,
    table_id: u32,
    output_path: Option<&str>,
) -> CliResult<Value> {
    let show = pptx_tables_show(readback_path, slide, table_id, None, false)?;
    let mut table = show
        .get("tables")
        .and_then(Value::as_array)
        .and_then(|tables| tables.first())
        .cloned()
        .ok_or_else(|| CliError::unexpected("updated table readback missing"))?;
    if let Some(output_path) = output_path
        && let Some(map) = table.as_object_mut()
    {
        map.insert("file".to_string(), json!(output_path));
    }
    Ok(table)
}

fn delete_row_result_json(
    file: &str,
    slide: u32,
    row: usize,
    mutation: &DeleteRowMutation,
    output_path: Option<&str>,
    destination: &mut Value,
) -> Value {
    let rows = destination
        .get("rows")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let cols = destination
        .get("cols")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    if let Some(output_path) = output_path {
        result.insert("output".to_string(), json!(output_path));
    }
    result.insert("dryRun".to_string(), json!(output_path.is_none()));
    result.insert("slide".to_string(), json!(slide));
    result.insert("tableId".to_string(), json!(mutation.resolved_table_id));
    result.insert("row".to_string(), json!(row));
    result.insert("rows".to_string(), json!(rows));
    result.insert("cols".to_string(), json!(cols));
    result.insert("cellCount".to_string(), json!(mutation.cell_count));
    let destination_value = destination.take();
    add_pptx_table_readback_commands(&mut result, output_path, slide, &destination_value);
    result.insert("destination".to_string(), destination_value);
    Value::Object(result)
}

fn add_pptx_table_readback_commands(
    result: &mut Map<String, Value>,
    output_path: Option<&str>,
    slide: u32,
    destination: &Value,
) {
    let command_target = output_path.unwrap_or("<out.pptx>");
    let target = destination
        .get("primarySelector")
        .and_then(Value::as_str)
        .unwrap_or("table:1");
    let command_suffix = if output_path.is_some() {
        ""
    } else {
        "Template"
    };
    result.insert(
        format!("readbackCommand{command_suffix}"),
        json!(format!(
            "ooxml --json pptx tables show {} --slide {} --target {}",
            command_arg(command_target),
            slide,
            command_arg(target)
        )),
    );
    result.insert(
        format!("slideReadbackCommand{command_suffix}"),
        json!(format!(
            "ooxml --json pptx slides show {} --slide {} --include-text --include-bounds",
            command_arg(command_target),
            slide
        )),
    );
    result.insert(
        format!("validateCommand{command_suffix}"),
        json!(format!(
            "ooxml validate --strict {}",
            command_arg(command_target)
        )),
    );
    result.insert(
        format!("renderCommand{command_suffix}"),
        json!(format!(
            "ooxml pptx render {} --out render-check",
            command_arg(command_target)
        )),
    );
}

fn table_mutation_output_path(file: &str, options: &PptxTableMutationOptions) -> Option<String> {
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

fn stage_table_mutation(
    file: &str,
    slide_part: &str,
    updated_xml: &str,
    options: &PptxTableMutationOptions,
) -> CliResult<String> {
    let output_path = options
        .out
        .as_deref()
        .filter(|value| !value.trim().is_empty());
    let write_path = if options.dry_run || options.in_place || output_path == Some(file) {
        package_mutation_temp_path(file, "pptx-table")
    } else {
        output_path
            .ok_or_else(|| {
                CliError::invalid_args(
                    "must specify exactly one of --out, --in-place, or --dry-run",
                )
            })?
            .to_string()
    };
    copy_zip_with_part_override(file, &write_path, slide_part, updated_xml)?;
    if !options.no_validate {
        validate(&write_path, true)?;
    }
    Ok(write_path)
}

fn finish_table_mutation(
    file: &str,
    staged_path: &str,
    options: &PptxTableMutationOptions,
    output_path: Option<&str>,
) -> CliResult<()> {
    if options.dry_run {
        let _ = fs::remove_file(staged_path);
    } else if options.in_place || output_path == Some(file) {
        if let Some(backup_path) = options
            .backup
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            fs::copy(file, backup_path)
                .map_err(|err| CliError::unexpected(format!("failed to create backup: {err}")))?;
        }
        fs::rename(staged_path, file)
            .or_else(|_| {
                fs::copy(staged_path, file)?;
                fs::remove_file(staged_path)
            })
            .map_err(|err| CliError::unexpected(format!("failed to write output file: {err}")))?;
    }
    Ok(())
}

fn pptx_slide_refs_for_table_mutation(file: &str) -> CliResult<Vec<PptxSlideRef>> {
    let presentation = zip_text(file, "ppt/presentation.xml")?;
    let slide_refs = presentation_slide_refs(&presentation);
    let rels = relationship_entries_from_xml(&zip_text(file, "ppt/_rels/presentation.xml.rels")?);
    slide_refs
        .into_iter()
        .map(|rel_id| {
            let rel = rels
                .iter()
                .find(|candidate| candidate.id == rel_id)
                .ok_or_else(|| CliError::unexpected(format!("missing relationship {rel_id}")))?;
            Ok(PptxSlideRef {
                part: package_part_name(&resolve_relationship_target(
                    "/ppt/presentation.xml",
                    &rel.target,
                )),
            })
        })
        .collect()
}

fn presentation_slide_refs(xml: &str) -> Vec<String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut slides = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "sldId" =>
            {
                if let Some(rel) = attr_exact(&e, "r:id") {
                    slides.push(rel);
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    slides
}

fn find_table_span_for_shape(xml: &str, table_id: u32) -> CliResult<Option<XmlSpan>> {
    let Some(sp_tree) = find_first_element_span(xml, "spTree")? else {
        return Err(CliError::unexpected("shape tree not found in slide"));
    };
    let (content_start, content_end) = element_content_bounds(&xml[sp_tree.start..sp_tree.end])?;
    let shapes = xml_direct_child_ranges(
        xml,
        sp_tree.start + content_start,
        sp_tree.start + content_end,
    )?;
    for shape in shapes
        .into_iter()
        .filter(|shape| shape.kind == "graphicFrame")
    {
        let fragment = &xml[shape.start..shape.end];
        if first_c_nv_pr_id(fragment) != Some(table_id) {
            continue;
        }
        if let Some(table) = find_first_element_span(fragment, "tbl")? {
            return Ok(Some(XmlSpan {
                start: shape.start + table.start,
                end: shape.start + table.end,
            }));
        }
    }
    Ok(None)
}

fn find_first_element_span(xml: &str, wanted_local: &str) -> CliResult<Option<XmlSpan>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut active: Option<(usize, usize)> = None;
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                if let Some((_, depth)) = active.as_mut() {
                    *depth += 1;
                } else if local_name(e.name().as_ref()) == wanted_local {
                    active = Some((before, 1));
                }
            }
            Ok(Event::Empty(e)) => {
                if active.is_none() && local_name(e.name().as_ref()) == wanted_local {
                    return Ok(Some(XmlSpan {
                        start: before,
                        end: reader.buffer_position() as usize,
                    }));
                }
            }
            Ok(Event::End(e)) => {
                if let Some((start, depth)) = active.as_mut() {
                    if *depth == 1 && local_name(e.name().as_ref()) == wanted_local {
                        return Ok(Some(XmlSpan {
                            start: *start,
                            end: reader.buffer_position() as usize,
                        }));
                    }
                    *depth = depth.saturating_sub(1);
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(None)
}

fn element_content_bounds(fragment: &str) -> CliResult<(usize, usize)> {
    let open_end = fragment
        .find('>')
        .ok_or_else(|| CliError::unexpected("invalid PPTX XML"))?;
    if fragment[..=open_end].trim_end().ends_with("/>") {
        return Ok((open_end + 1, open_end + 1));
    }
    let close_start = fragment
        .rfind("</")
        .ok_or_else(|| CliError::unexpected("invalid PPTX XML"))?;
    Ok((open_end + 1, close_start))
}

fn first_c_nv_pr_id(fragment: &str) -> Option<u32> {
    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "cNvPr" =>
            {
                return attr(&e, "id").and_then(|value| value.parse().ok());
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    None
}

fn first_element_attrs(fragment: &str) -> CliResult<BTreeMap<String, String>> {
    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let mut attrs = BTreeMap::new();
                for attr in e.attributes().with_checks(false).flatten() {
                    attrs.insert(
                        local_name(attr.key.as_ref()).to_string(),
                        crate::decode_xml_text(attr.value.as_ref()),
                    );
                }
                return Ok(attrs);
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(BTreeMap::new())
}

fn package_part_name(uri: &str) -> String {
    uri.trim_start_matches('/').to_string()
}
