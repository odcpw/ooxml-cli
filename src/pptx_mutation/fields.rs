use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::fs;

use crate::cli_args::{parse_bool_flag, value_flag_present};
use crate::{
    CliError, CliResult, append_xml_text_event, attr, attr_exact, command_arg,
    copy_zip_with_part_overrides, is_xml_text_event, local_name, package_mutation_temp_path,
    package_type, parse_string_flag, relationship_entries_from_xml, resolve_relationship_target,
    validate, validate_xlsx_mutation_output_flags, xml_attr_escape, xml_direct_child_ranges,
    xml_escape, xml_fragment_bounds, xml_token_name, zip_text,
};

const SLIDE_REL_TYPE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide";
const MASTER_REL_TYPE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster";

pub(crate) fn pptx_fields_set(file: &str, args: &[String]) -> CliResult<Value> {
    let options = parse_fields_set_options(args)?;
    let detected = package_type(file)?;
    if detected != "pptx" {
        return Err(CliError::unsupported_type(format!(
            "file is not a PPTX document (detected: {detected})"
        )));
    }
    let mutation = build_fields_mutation(file, &options)?;
    write_fields_mutation(file, &mutation.overrides, &options)?;
    Ok(fields_set_result_json(file, &mutation.result, &options))
}

#[derive(Clone)]
struct FieldsSetOptions {
    footer: Option<String>,
    show_footer: Option<bool>,
    show_slide_number: Option<bool>,
    show_date: Option<bool>,
    date_format: Option<String>,
    out: Option<String>,
    backup: Option<String>,
    dry_run: bool,
    in_place: bool,
    no_validate: bool,
}

fn parse_fields_set_options(args: &[String]) -> CliResult<FieldsSetOptions> {
    let footer = if value_flag_present(args, "--footer") {
        Some(parse_string_flag(args, "--footer")?.unwrap_or_default())
    } else {
        None
    };
    let show_footer = parse_bool_flag(args, "--show-footer")?;
    let show_slide_number = parse_bool_flag(args, "--show-slide-number")?;
    let show_date = parse_bool_flag(args, "--show-date")?;
    let date_format = parse_string_flag(args, "--date-format")?;
    if footer.is_none()
        && show_footer.is_none()
        && show_slide_number.is_none()
        && show_date.is_none()
        && date_format.is_none()
    {
        return Err(CliError::invalid_args(
            "no field flags provided; specify at least one of --footer/--show-footer/--show-slide-number/--show-date/--date-format",
        ));
    }
    if let Some(format) = date_format.as_deref()
        && !matches!(format, "auto" | "datetime" | "date-only")
    {
        return Err(CliError::invalid_args(format!(
            "invalid --date-format {format:?} (expected one of: auto, datetime, date-only)"
        )));
    }

    let out = parse_string_flag(args, "--out")?;
    let backup = parse_string_flag(args, "--backup")?;
    let dry_run = parse_bool_flag(args, "--dry-run")?.unwrap_or(false);
    let in_place = parse_bool_flag(args, "--in-place")?.unwrap_or(false);
    let no_validate = parse_bool_flag(args, "--no-validate")?.unwrap_or(false);
    validate_xlsx_mutation_output_flags(out.as_deref(), in_place, backup.as_deref(), dry_run)?;
    Ok(FieldsSetOptions {
        footer,
        show_footer,
        show_slide_number,
        show_date,
        date_format,
        out,
        backup,
        dry_run,
        in_place,
        no_validate,
    })
}

struct FieldsMutation {
    result: FieldsSetResult,
    overrides: BTreeMap<String, String>,
}

struct FieldsSetResult {
    masters_updated: Vec<String>,
    created_header_footer: bool,
    footer_placeholders_updated: usize,
    footer_placeholders_created: usize,
    date_placeholders_updated: usize,
    slides_without_footer_placeholder: Vec<usize>,
    slides_without_date_placeholder: Vec<usize>,
    slides_with_date_placeholder_but_no_field: Vec<usize>,
}

#[derive(Clone)]
struct PresentationParts {
    slides: Vec<String>,
    masters: Vec<String>,
}

fn build_fields_mutation(file: &str, options: &FieldsSetOptions) -> CliResult<FieldsMutation> {
    let parts = presentation_parts(file)?;
    let mut overrides = BTreeMap::new();
    let mut result = FieldsSetResult {
        masters_updated: Vec::new(),
        created_header_footer: false,
        footer_placeholders_updated: 0,
        footer_placeholders_created: 0,
        date_placeholders_updated: 0,
        slides_without_footer_placeholder: Vec::new(),
        slides_without_date_placeholder: Vec::new(),
        slides_with_date_placeholder_but_no_field: Vec::new(),
    };

    if options.show_slide_number.is_some()
        || options.show_footer.is_some()
        || options.show_date.is_some()
    {
        for master in &parts.masters {
            let part = package_part_name(master);
            let xml = zip_text(file, &part)?;
            let update = apply_master_visibility(&xml, options)?;
            if update.changed {
                overrides.insert(part, update.xml);
                result.masters_updated.push(master.clone());
                result.created_header_footer |= update.created_header_footer;
            }
        }
    }

    let date_field_type = options.date_format.as_deref().map(date_format_field_type);
    for (index, slide) in parts.slides.iter().enumerate() {
        let slide_number = index + 1;
        let part = package_part_name(slide);
        let mut xml = zip_text(file, &part)?;
        let mut changed = false;

        if let Some(footer) = options.footer.as_deref() {
            if let Some(span) = find_placeholder_shape_span(&xml, "ftr") {
                let shape_xml = &xml[span.0..span.1];
                let updated = set_footer_text(shape_xml, footer)?;
                if updated != shape_xml {
                    xml = replace_span(&xml, span.0, span.1, &updated);
                    changed = true;
                    result.footer_placeholders_updated += 1;
                }
            } else if !footer.is_empty() && options.show_footer != Some(false) {
                if let Some(updated) = add_footer_placeholder_shape(&xml, footer)? {
                    xml = updated;
                    changed = true;
                    result.footer_placeholders_created += 1;
                    result.footer_placeholders_updated += 1;
                } else {
                    result.slides_without_footer_placeholder.push(slide_number);
                }
            } else {
                result.slides_without_footer_placeholder.push(slide_number);
            }
        }

        if let Some(field_type) = date_field_type {
            if let Some(span) = find_placeholder_shape_span(&xml, "dt") {
                let shape_xml = &xml[span.0..span.1];
                match set_date_field_type(shape_xml, field_type)? {
                    DateFieldUpdate::Changed(updated) => {
                        xml = replace_span(&xml, span.0, span.1, &updated);
                        changed = true;
                        result.date_placeholders_updated += 1;
                    }
                    DateFieldUpdate::NoField => result
                        .slides_with_date_placeholder_but_no_field
                        .push(slide_number),
                    DateFieldUpdate::Unchanged => {}
                }
            } else {
                result.slides_without_date_placeholder.push(slide_number);
            }
        }

        if changed {
            overrides.insert(part, xml);
        }
    }

    Ok(FieldsMutation { result, overrides })
}

fn presentation_parts(file: &str) -> CliResult<PresentationParts> {
    let presentation = zip_text(file, "ppt/presentation.xml")?;
    let rels = relationship_entries_from_xml(&zip_text(file, "ppt/_rels/presentation.xml.rels")?);
    let mut slides = Vec::new();
    let mut masters = Vec::new();
    let mut reader = Reader::from_str(&presentation);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if name != "sldId" && name != "sldMasterId" {
                    continue;
                }
                let Some(rel_id) = attr_exact(&e, "r:id") else {
                    continue;
                };
                let expected = if name == "sldId" {
                    SLIDE_REL_TYPE
                } else {
                    MASTER_REL_TYPE
                };
                let rel = rels.iter().find(|rel| rel.id == rel_id).ok_or_else(|| {
                    CliError::unexpected(format!("missing relationship {rel_id}"))
                })?;
                if rel.rel_type != expected {
                    continue;
                }
                let part = resolve_relationship_target("/ppt/presentation.xml", &rel.target);
                if name == "sldId" {
                    slides.push(part);
                } else {
                    masters.push(part);
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(PresentationParts { slides, masters })
}

struct MasterVisibilityUpdate {
    xml: String,
    changed: bool,
    created_header_footer: bool,
}

fn apply_master_visibility(
    xml: &str,
    options: &FieldsSetOptions,
) -> CliResult<MasterVisibilityUpdate> {
    if let Some(hf_span) = root_direct_child_span(xml, "hf") {
        let mut updated = xml.to_string();
        for (name, value) in [
            ("sldNum", options.show_slide_number),
            ("ftr", options.show_footer),
            ("dt", options.show_date),
        ] {
            if let Some(value) = value {
                updated = set_attr_on_element(&updated, hf_span.0, name, bool_attr_value(value))?;
            }
        }
        Ok(MasterVisibilityUpdate {
            changed: updated != xml,
            xml: updated,
            created_header_footer: false,
        })
    } else {
        let (open_end, tag_name, close_start, self_closing) = document_root_bounds(xml)?;
        if self_closing {
            return Ok(MasterVisibilityUpdate {
                xml: xml.to_string(),
                changed: false,
                created_header_footer: false,
            });
        }
        let prefix = tag_prefix(&tag_name);
        let mut hf = String::new();
        hf.push('<');
        hf.push_str(&qualified_name(&prefix, "hf"));
        for (name, value) in [
            ("sldNum", options.show_slide_number),
            ("ftr", options.show_footer),
            ("dt", options.show_date),
        ] {
            if let Some(value) = value {
                hf.push(' ');
                hf.push_str(name);
                hf.push_str("=\"");
                hf.push_str(bool_attr_value(value));
                hf.push('"');
            }
        }
        hf.push_str("/>");
        let insert_at = master_header_footer_insert_position(xml, open_end + 1, close_start)?;
        let updated = insert_at_index(xml, insert_at, &hf);
        Ok(MasterVisibilityUpdate {
            xml: updated,
            changed: true,
            created_header_footer: true,
        })
    }
}

fn master_header_footer_insert_position(
    xml: &str,
    content_start: usize,
    content_end: usize,
) -> CliResult<usize> {
    let children = xml_direct_child_ranges(xml, content_start, content_end)?;
    Ok(children
        .iter()
        .find(|child| master_child_rank(&child.kind) > master_child_rank("hf"))
        .map(|child| child.start)
        .unwrap_or(content_end))
}

fn master_child_rank(kind: &str) -> usize {
    match kind {
        "cSld" => 10,
        "clrMap" => 20,
        "sldLayoutIdLst" => 30,
        "transition" => 40,
        "timing" => 50,
        "hf" => 60,
        "txStyles" => 70,
        "extLst" => 80,
        _ => 90,
    }
}

fn root_direct_child_span(xml: &str, wanted: &str) -> Option<(usize, usize)> {
    let (open_end, _, close_start, self_closing) = document_root_bounds(xml).ok()?;
    if self_closing {
        return None;
    }
    xml_direct_child_ranges(xml, open_end + 1, close_start)
        .ok()?
        .into_iter()
        .find(|child| child.kind == wanted)
        .map(|child| (child.start, child.end))
}

fn document_root_bounds(xml: &str) -> CliResult<(usize, String, usize, bool)> {
    let mut cursor = 0usize;
    while cursor < xml.len() {
        let relative_start = xml[cursor..]
            .find('<')
            .ok_or_else(|| CliError::unexpected("invalid XML document"))?;
        let tag_start = cursor + relative_start;
        let relative_end = xml[tag_start..]
            .find('>')
            .ok_or_else(|| CliError::unexpected("invalid XML document"))?;
        let tag_end = tag_start + relative_end;
        let token = xml[tag_start + 1..tag_end].trim_start();
        if token.starts_with('?') || token.starts_with('!') || token.starts_with('/') {
            cursor = tag_end + 1;
            continue;
        }
        let end = if token.trim_end().ends_with('/') {
            tag_end + 1
        } else {
            let name = xml_token_name(token)
                .ok_or_else(|| CliError::unexpected("invalid XML document"))?;
            find_matching_element_end(xml, local_name(name.as_bytes()), tag_end + 1, xml.len())
                .ok_or_else(|| CliError::unexpected("invalid XML document"))?
        };
        let (open_end, tag_name, close_start, self_closing) =
            xml_fragment_bounds(&xml[tag_start..end])?;
        return Ok((
            tag_start + open_end,
            tag_name,
            tag_start + close_start,
            self_closing,
        ));
    }
    Err(CliError::unexpected("invalid XML document"))
}

fn set_footer_text(shape_xml: &str, text: &str) -> CliResult<String> {
    let Some(tx_body) = first_element_span(shape_xml, "txBody", 0, shape_xml.len()) else {
        return Ok(shape_xml.to_string());
    };
    let body_start = content_start(shape_xml, tx_body).unwrap_or(tx_body.0);
    let body_end = content_end(shape_xml, tx_body).unwrap_or(tx_body.1);
    let paragraphs = xml_direct_child_ranges(shape_xml, body_start, body_end)?;
    if let Some(paragraph) = paragraphs.into_iter().find(|child| child.kind == "p") {
        let current = paragraph_run_text(shape_xml, paragraph.start, paragraph.end);
        if current == text {
            return Ok(shape_xml.to_string());
        }
        let paragraph_xml = &shape_xml[paragraph.start..paragraph.end];
        let updated = rewrite_footer_paragraph(paragraph_xml, text)?;
        Ok(replace_span(
            shape_xml,
            paragraph.start,
            paragraph.end,
            &updated,
        ))
    } else {
        let prefix = "a";
        let mut paragraph = String::new();
        paragraph.push('<');
        paragraph.push_str(&qualified_name(prefix, "p"));
        paragraph.push('>');
        if !text.is_empty() {
            paragraph.push_str(&text_run_xml(prefix, text));
        }
        paragraph.push_str("</");
        paragraph.push_str(&qualified_name(prefix, "p"));
        paragraph.push('>');
        Ok(insert_at_index(shape_xml, body_end, &paragraph))
    }
}

fn rewrite_footer_paragraph(paragraph_xml: &str, text: &str) -> CliResult<String> {
    let (open_end, tag_name, close_start, self_closing) = xml_fragment_bounds(paragraph_xml)?;
    if self_closing {
        return Ok(paragraph_xml.to_string());
    }
    let prefix = tag_prefix(&tag_name);
    let children = xml_direct_child_ranges(paragraph_xml, open_end + 1, close_start)?;
    let mut out = String::new();
    out.push_str(&paragraph_xml[..=open_end]);
    for child in children.iter().filter(|child| child.kind == "pPr") {
        out.push_str(&paragraph_xml[child.start..child.end]);
    }
    if !text.is_empty() {
        out.push_str(&text_run_xml(&prefix, text));
    }
    for child in children.iter().filter(|child| child.kind == "endParaRPr") {
        out.push_str(&paragraph_xml[child.start..child.end]);
    }
    out.push_str(&paragraph_xml[close_start..]);
    Ok(out)
}

fn text_run_xml(prefix: &str, text: &str) -> String {
    let r = qualified_name(prefix, "r");
    let t = qualified_name(prefix, "t");
    format!("<{r}><{t}>{}</{t}></{r}>", xml_escape(text))
}

enum DateFieldUpdate {
    Changed(String),
    Unchanged,
    NoField,
}

fn set_date_field_type(shape_xml: &str, field_type: &str) -> CliResult<DateFieldUpdate> {
    let Some(tx_body) = first_element_span(shape_xml, "txBody", 0, shape_xml.len()) else {
        return Ok(DateFieldUpdate::NoField);
    };
    let body_start = content_start(shape_xml, tx_body).unwrap_or(tx_body.0);
    let body_end = content_end(shape_xml, tx_body).unwrap_or(tx_body.1);
    let paragraphs = xml_direct_child_ranges(shape_xml, body_start, body_end)?;
    for paragraph in paragraphs.into_iter().filter(|child| child.kind == "p") {
        let p_start =
            content_start(shape_xml, (paragraph.start, paragraph.end)).unwrap_or(paragraph.start);
        let p_end =
            content_end(shape_xml, (paragraph.start, paragraph.end)).unwrap_or(paragraph.end);
        let children = xml_direct_child_ranges(shape_xml, p_start, p_end)?;
        if let Some(field) = children.into_iter().find(|child| child.kind == "fld") {
            let field_xml = &shape_xml[field.start..field.end];
            let current = attr_from_start_tag(
                &field_xml[..start_tag_end(field_xml, 0, field_xml.len())],
                "type",
            )
            .unwrap_or_default();
            if current == field_type {
                return Ok(DateFieldUpdate::Unchanged);
            }
            let updated = set_attr_on_element(shape_xml, field.start, "type", field_type)?;
            return Ok(DateFieldUpdate::Changed(updated));
        }
    }
    Ok(DateFieldUpdate::NoField)
}

fn add_footer_placeholder_shape(xml: &str, text: &str) -> CliResult<Option<String>> {
    let Some(c_sld) = first_element_span(xml, "cSld", 0, xml.len()) else {
        return Ok(None);
    };
    let Some(sp_tree) = first_element_span(xml, "spTree", c_sld.0, c_sld.1) else {
        return Ok(None);
    };
    let Some(content_start) = content_start(xml, sp_tree) else {
        return Ok(None);
    };
    let Some(content_end) = content_end(xml, sp_tree) else {
        return Ok(None);
    };
    let children = xml_direct_child_ranges(xml, content_start, content_end)?;
    let insert_at = children
        .iter()
        .find(|child| child.kind == "extLst")
        .map(|child| child.start)
        .unwrap_or(content_end);
    let id = next_sp_tree_shape_id(&xml[sp_tree.0..sp_tree.1]);
    let prefix = xml_fragment_bounds(&xml[sp_tree.0..sp_tree.1])
        .map(|(_, tag_name, _, _)| tag_prefix(&tag_name))
        .unwrap_or_else(|_| "p".to_string());
    let drawing_prefix = drawingml_prefix(xml);
    let shape = footer_placeholder_shape_xml(&prefix, &drawing_prefix, id, text);
    Ok(Some(insert_at_index(xml, insert_at, &shape)))
}

fn next_sp_tree_shape_id(sp_tree_xml: &str) -> u32 {
    let mut reader = Reader::from_str(sp_tree_xml);
    reader.config_mut().trim_text(true);
    let mut max_id = 1_u32;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "cNvPr" =>
            {
                if let Some(id) = attr(&e, "id").and_then(|value| value.parse::<u32>().ok()) {
                    max_id = max_id.max(id);
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    max_id.saturating_add(1)
}

fn drawingml_prefix(xml: &str) -> String {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let qname = e.name();
                let name = local_name(qname.as_ref());
                if matches!(
                    name,
                    "bodyPr"
                        | "lstStyle"
                        | "p"
                        | "r"
                        | "t"
                        | "spLocks"
                        | "xfrm"
                        | "off"
                        | "ext"
                        | "prstGeom"
                        | "avLst"
                ) {
                    let raw = String::from_utf8_lossy(qname.as_ref());
                    if let Some((prefix, _)) = raw.split_once(':') {
                        return prefix.to_string();
                    }
                    return String::new();
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    "a".to_string()
}

fn footer_placeholder_shape_xml(prefix: &str, drawing_prefix: &str, id: u32, text: &str) -> String {
    let p_sp = qualified_name(prefix, "sp");
    let p_nv_sp_pr = qualified_name(prefix, "nvSpPr");
    let p_c_nv_pr = qualified_name(prefix, "cNvPr");
    let p_c_nv_sp_pr = qualified_name(prefix, "cNvSpPr");
    let p_nv_pr = qualified_name(prefix, "nvPr");
    let p_ph = qualified_name(prefix, "ph");
    let p_sp_pr = qualified_name(prefix, "spPr");
    let p_tx_body = qualified_name(prefix, "txBody");
    let a_sp_locks = qualified_name(drawing_prefix, "spLocks");
    let a_xfrm = qualified_name(drawing_prefix, "xfrm");
    let a_off = qualified_name(drawing_prefix, "off");
    let a_ext = qualified_name(drawing_prefix, "ext");
    let a_prst_geom = qualified_name(drawing_prefix, "prstGeom");
    let a_av_lst = qualified_name(drawing_prefix, "avLst");
    let a_body_pr = qualified_name(drawing_prefix, "bodyPr");
    let a_lst_style = qualified_name(drawing_prefix, "lstStyle");
    let a_lvl1p_pr = qualified_name(drawing_prefix, "lvl1pPr");
    let a_def_r_pr = qualified_name(drawing_prefix, "defRPr");
    let a_p = qualified_name(drawing_prefix, "p");
    let a_end_para_r_pr = qualified_name(drawing_prefix, "endParaRPr");
    format!(
        concat!(
            "<{p_sp}>",
            "<{p_nv_sp_pr}>",
            "<{p_c_nv_pr} id=\"{id}\" name=\"Footer Placeholder {id}\"/>",
            "<{p_c_nv_sp_pr}><{a_sp_locks} noGrp=\"1\"/></{p_c_nv_sp_pr}>",
            "<{p_nv_pr}><{p_ph} type=\"ftr\" sz=\"quarter\" idx=\"11\"/></{p_nv_pr}>",
            "</{p_nv_sp_pr}>",
            "<{p_sp_pr}>",
            "<{a_xfrm}><{a_off} x=\"3124200\" y=\"6356350\"/><{a_ext} cx=\"2895600\" cy=\"365125\"/></{a_xfrm}>",
            "<{a_prst_geom} prst=\"rect\"><{a_av_lst}/></{a_prst_geom}>",
            "</{p_sp_pr}>",
            "<{p_tx_body}>",
            "<{a_body_pr} vert=\"horz\" lIns=\"91440\" tIns=\"45720\" rIns=\"91440\" bIns=\"45720\" rtlCol=\"0\" anchor=\"ctr\"/>",
            "<{a_lst_style}><{a_lvl1p_pr} algn=\"ctr\"><{a_def_r_pr} sz=\"1200\"/></{a_lvl1p_pr}></{a_lst_style}>",
            "<{a_p}>{}<{a_end_para_r_pr} lang=\"en-US\"/></{a_p}>",
            "</{p_tx_body}>",
            "</{p_sp}>"
        ),
        text_run_xml(drawing_prefix, text),
        a_av_lst = a_av_lst,
        a_body_pr = a_body_pr,
        a_def_r_pr = a_def_r_pr,
        a_end_para_r_pr = a_end_para_r_pr,
        a_ext = a_ext,
        a_lst_style = a_lst_style,
        a_lvl1p_pr = a_lvl1p_pr,
        a_off = a_off,
        a_p = a_p,
        a_prst_geom = a_prst_geom,
        a_sp_locks = a_sp_locks,
        a_xfrm = a_xfrm,
        id = id,
        p_c_nv_pr = p_c_nv_pr,
        p_c_nv_sp_pr = p_c_nv_sp_pr,
        p_nv_pr = p_nv_pr,
        p_nv_sp_pr = p_nv_sp_pr,
        p_ph = p_ph,
        p_sp = p_sp,
        p_sp_pr = p_sp_pr,
        p_tx_body = p_tx_body,
    )
}

fn find_placeholder_shape_span(xml: &str, placeholder_type: &str) -> Option<(usize, usize)> {
    let c_sld = first_element_span(xml, "cSld", 0, xml.len())?;
    let sp_tree = first_element_span(xml, "spTree", c_sld.0, c_sld.1)?;
    let children = xml_direct_child_ranges(
        xml,
        content_start(xml, sp_tree)?,
        content_end(xml, sp_tree)?,
    )
    .ok()?;
    children.into_iter().find_map(|child| {
        if child.kind != "sp" {
            return None;
        }
        let shape_xml = &xml[child.start..child.end];
        if placeholder_type_of_shape(shape_xml).as_deref() == Some(placeholder_type) {
            Some((child.start, child.end))
        } else {
            None
        }
    })
}

fn placeholder_type_of_shape(shape_xml: &str) -> Option<String> {
    let mut reader = Reader::from_str(shape_xml);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == "ph" => {
                return attr(&e, "type");
            }
            Ok(Event::Eof) => return None,
            Err(_) => return None,
            _ => {}
        }
    }
}

fn paragraph_run_text(xml: &str, start: usize, end: usize) -> String {
    let Some(content_start) = content_start(xml, (start, end)) else {
        return String::new();
    };
    let Some(content_end) = content_end(xml, (start, end)) else {
        return String::new();
    };
    let Ok(children) = xml_direct_child_ranges(xml, content_start, content_end) else {
        return String::new();
    };
    let mut text = String::new();
    for child in children.into_iter().filter(|child| child.kind == "r") {
        text.push_str(&text_descendants(&xml[child.start..child.end]));
    }
    text
}

fn text_descendants(fragment: &str) -> String {
    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(false);
    let mut in_text = false;
    let mut text = String::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "t" => in_text = true,
            Ok(Event::End(e)) if local_name(e.name().as_ref()) == "t" => in_text = false,
            Ok(event) if in_text && is_xml_text_event(&event) => {
                append_xml_text_event(&mut text, &event);
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    text
}

fn date_format_field_type(format: &str) -> &'static str {
    match format {
        "auto" => "datetimeFigureOut",
        "datetime" => "datetime",
        "date-only" => "datetime1",
        _ => "datetimeFigureOut",
    }
}

fn write_fields_mutation(
    file: &str,
    overrides: &BTreeMap<String, String>,
    options: &FieldsSetOptions,
) -> CliResult<()> {
    let output_path = options
        .out
        .as_deref()
        .filter(|value| !value.trim().is_empty());
    let write_path = if options.dry_run || options.in_place || output_path == Some(file) {
        package_mutation_temp_path(file, "pptx-fields")
    } else {
        output_path
            .ok_or_else(|| {
                CliError::invalid_args(
                    "must specify exactly one of --out, --in-place, or --dry-run",
                )
            })?
            .to_string()
    };

    copy_zip_with_part_overrides(file, &write_path, overrides)?;
    if !options.no_validate {
        validate(&write_path, true)?;
    }
    if options.dry_run {
        let _ = fs::remove_file(&write_path);
    } else if options.in_place || output_path == Some(file) {
        if let Some(backup_path) = options
            .backup
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            fs::copy(file, backup_path)
                .map_err(|err| CliError::unexpected(format!("failed to create backup: {err}")))?;
        }
        fs::rename(&write_path, file)
            .or_else(|_| {
                fs::copy(&write_path, file)?;
                fs::remove_file(&write_path)
            })
            .map_err(|err| CliError::unexpected(format!("failed to write output file: {err}")))?;
    }
    Ok(())
}

fn fields_set_result_json(
    file: &str,
    result: &FieldsSetResult,
    options: &FieldsSetOptions,
) -> Value {
    let output_path = if options.in_place {
        Some(file)
    } else {
        options
            .out
            .as_deref()
            .filter(|value| !value.trim().is_empty())
    };
    let command_target = if options.dry_run {
        "<out.pptx>"
    } else {
        output_path.unwrap_or(file)
    };
    let command_suffix = if options.dry_run { "Template" } else { "" };
    let mut out = Map::new();
    out.insert("file".to_string(), json!(file));
    if !options.dry_run
        && let Some(output_path) = output_path
    {
        out.insert("output".to_string(), json!(output_path));
    }
    out.insert("dryRun".to_string(), json!(options.dry_run));
    out.insert(
        format!("readbackCommand{command_suffix}"),
        json!(format!(
            "ooxml --json pptx fields inspect {}",
            command_arg(command_target)
        )),
    );
    out.insert(
        format!("validateCommand{command_suffix}"),
        json!(format!(
            "ooxml validate --strict {}",
            command_arg(command_target)
        )),
    );
    out.insert(
        format!("renderCommand{command_suffix}"),
        json!(format!(
            "ooxml pptx render {} --out render-check",
            command_arg(command_target)
        )),
    );
    if let Some(footer) = options.footer.as_deref()
        && !footer.is_empty()
    {
        out.insert("footerText".to_string(), json!(footer));
    }
    if let Some(show_slide_number) = options.show_slide_number {
        out.insert("showSlideNumber".to_string(), json!(show_slide_number));
    }
    if let Some(show_footer) = options.show_footer {
        out.insert("showFooter".to_string(), json!(show_footer));
    }
    if let Some(show_date) = options.show_date {
        out.insert("showDate".to_string(), json!(show_date));
    }
    if let Some(format) = options.date_format.as_deref()
        && !format.is_empty()
    {
        out.insert("dateFormat".to_string(), json!(format));
    }
    out.insert("mastersUpdated".to_string(), json!(result.masters_updated));
    out.insert(
        "createdHeaderFooter".to_string(),
        json!(result.created_header_footer),
    );
    out.insert(
        "footerPlaceholdersUpdated".to_string(),
        json!(result.footer_placeholders_updated),
    );
    if result.footer_placeholders_created > 0 {
        out.insert(
            "footerPlaceholdersCreated".to_string(),
            json!(result.footer_placeholders_created),
        );
    }
    out.insert(
        "datePlaceholdersUpdated".to_string(),
        json!(result.date_placeholders_updated),
    );
    if !result.slides_without_footer_placeholder.is_empty() {
        out.insert(
            "slidesWithoutFooterPlaceholder".to_string(),
            json!(result.slides_without_footer_placeholder),
        );
    }
    if !result.slides_without_date_placeholder.is_empty() {
        out.insert(
            "slidesWithoutDatePlaceholder".to_string(),
            json!(result.slides_without_date_placeholder),
        );
    }
    if !result.slides_with_date_placeholder_but_no_field.is_empty() {
        out.insert(
            "slidesWithDatePlaceholderButNoField".to_string(),
            json!(result.slides_with_date_placeholder_but_no_field),
        );
    }
    Value::Object(out)
}

fn first_element_span(
    xml: &str,
    wanted: &str,
    range_start: usize,
    range_end: usize,
) -> Option<(usize, usize)> {
    let mut cursor = range_start;
    while cursor < range_end {
        let relative_start = xml[cursor..range_end].find('<')?;
        let tag_start = cursor + relative_start;
        let relative_end = xml[tag_start..range_end].find('>')?;
        let tag_end = tag_start + relative_end;
        let token = xml[tag_start + 1..tag_end].trim_start();
        if token.starts_with('/') || token.starts_with('?') || token.starts_with('!') {
            cursor = tag_end + 1;
            continue;
        }
        let name = xml_token_name(token)?;
        let self_closing = token.trim_end().ends_with('/');
        if local_name(name.as_bytes()) == wanted {
            if self_closing {
                return Some((tag_start, tag_end + 1));
            }
            return find_matching_element_end(xml, wanted, tag_end + 1, range_end)
                .map(|end| (tag_start, end));
        }
        cursor = tag_end + 1;
    }
    None
}

fn find_matching_element_end(
    xml: &str,
    wanted: &str,
    range_start: usize,
    range_end: usize,
) -> Option<usize> {
    let mut depth = 1usize;
    let mut cursor = range_start;
    while cursor < range_end {
        let relative_start = xml[cursor..range_end].find('<')?;
        let tag_start = cursor + relative_start;
        let relative_end = xml[tag_start..range_end].find('>')?;
        let tag_end = tag_start + relative_end;
        let token = xml[tag_start + 1..tag_end].trim_start();
        if token.starts_with('?') || token.starts_with('!') {
            cursor = tag_end + 1;
            continue;
        }
        if let Some(name) = xml_token_name(token)
            && local_name(name.as_bytes()) == wanted
        {
            if token.starts_with('/') {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(tag_end + 1);
                }
            } else if !token.trim_end().ends_with('/') {
                depth += 1;
            }
        }
        cursor = tag_end + 1;
    }
    None
}

fn content_start(xml: &str, span: (usize, usize)) -> Option<usize> {
    let tag_end = xml[span.0..span.1].find('>')?;
    Some(span.0 + tag_end + 1)
}

fn content_end(xml: &str, span: (usize, usize)) -> Option<usize> {
    let fragment = &xml[span.0..span.1];
    let first_end = fragment.find('>')?;
    if fragment[..first_end].trim_end().ends_with('/') {
        return Some(span.0 + first_end);
    }
    let close_start = fragment.rfind("</")?;
    Some(span.0 + close_start)
}

fn set_attr_on_element(
    xml: &str,
    element_start: usize,
    attr_name: &str,
    value: &str,
) -> CliResult<String> {
    let tag_end = xml[element_start..]
        .find('>')
        .map(|offset| element_start + offset + 1)
        .ok_or_else(|| CliError::unexpected("invalid XML start tag"))?;
    let start_tag = &xml[element_start..tag_end];
    let replacement = set_attr_on_start_tag(start_tag, attr_name, value)?;
    Ok(replace_span(xml, element_start, tag_end, &replacement))
}

fn set_attr_on_start_tag(start_tag: &str, attr_name: &str, value: &str) -> CliResult<String> {
    let Some(open_end) = start_tag.find('>') else {
        return Err(CliError::unexpected("invalid XML start tag"));
    };
    let Some(token_name) = xml_token_name(&start_tag[1..open_end]) else {
        return Err(CliError::unexpected("invalid XML start tag"));
    };
    let mut cursor = 1 + token_name.len();
    while cursor < open_end {
        while cursor < open_end && start_tag.as_bytes()[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor >= open_end || start_tag.as_bytes()[cursor] == b'/' {
            break;
        }
        let name_start = cursor;
        while cursor < open_end {
            let byte = start_tag.as_bytes()[cursor];
            if byte == b'=' || byte.is_ascii_whitespace() || byte == b'/' {
                break;
            }
            cursor += 1;
        }
        let name_end = cursor;
        while cursor < open_end && start_tag.as_bytes()[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor >= open_end || start_tag.as_bytes()[cursor] != b'=' {
            continue;
        }
        cursor += 1;
        while cursor < open_end && start_tag.as_bytes()[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor >= open_end {
            break;
        }
        let quote = start_tag.as_bytes()[cursor];
        if quote != b'"' && quote != b'\'' {
            continue;
        }
        cursor += 1;
        let value_start = cursor;
        while cursor < open_end && start_tag.as_bytes()[cursor] != quote {
            cursor += 1;
        }
        if cursor >= open_end {
            break;
        }
        let value_end = cursor;
        cursor += 1;
        let existing_name = &start_tag[name_start..name_end];
        if local_name(existing_name.as_bytes()) == attr_name {
            if &start_tag[value_start..value_end] == value {
                return Ok(start_tag.to_string());
            }
            let mut out = String::with_capacity(start_tag.len() + value.len());
            out.push_str(&start_tag[..value_start]);
            out.push_str(&xml_attr_escape(value));
            out.push_str(&start_tag[value_end..]);
            return Ok(out);
        }
    }

    let insert_at = if start_tag[..open_end].trim_end().ends_with('/') {
        start_tag[..open_end]
            .rfind('/')
            .ok_or_else(|| CliError::unexpected("invalid XML start tag"))?
    } else {
        open_end
    };
    let mut out = String::with_capacity(start_tag.len() + attr_name.len() + value.len() + 4);
    out.push_str(&start_tag[..insert_at]);
    out.push(' ');
    out.push_str(attr_name);
    out.push_str("=\"");
    out.push_str(&xml_attr_escape(value));
    out.push('"');
    out.push_str(&start_tag[insert_at..]);
    Ok(out)
}

fn attr_from_start_tag(start_tag: &str, name: &str) -> Option<String> {
    let mut reader = Reader::from_str(start_tag);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => return attr(&e, name),
            Ok(Event::Eof) => return None,
            Err(_) => return None,
            _ => {}
        }
    }
}

fn start_tag_end(xml: &str, start: usize, end: usize) -> usize {
    xml[start..end]
        .find('>')
        .map(|offset| start + offset + 1)
        .unwrap_or(end)
}

fn replace_span(xml: &str, start: usize, end: usize, replacement: &str) -> String {
    let mut out = String::with_capacity(xml.len() - (end - start) + replacement.len());
    out.push_str(&xml[..start]);
    out.push_str(replacement);
    out.push_str(&xml[end..]);
    out
}

fn insert_at_index(xml: &str, index: usize, insertion: &str) -> String {
    let mut out = String::with_capacity(xml.len() + insertion.len());
    out.push_str(&xml[..index]);
    out.push_str(insertion);
    out.push_str(&xml[index..]);
    out
}

fn bool_attr_value(value: bool) -> &'static str {
    if value { "1" } else { "0" }
}

fn tag_prefix(tag_name: &str) -> String {
    tag_name
        .split_once(':')
        .map(|(prefix, _)| prefix.to_string())
        .unwrap_or_default()
}

fn qualified_name(prefix: &str, local: &str) -> String {
    if prefix.is_empty() {
        local.to_string()
    } else {
        format!("{prefix}:{local}")
    }
}

fn package_part_name(uri: &str) -> String {
    uri.trim_start_matches('/').to_string()
}
