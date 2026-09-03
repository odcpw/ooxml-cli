use serde_json::{Value, json};

use crate::{
    CliError, CliResult, DocxParagraphMutationOptions, docx_body_content_bounds, docx_body_prefix,
    docx_body_tag, ensure_docx_package_kind, find_docx_document_part, word_xml_tag,
    write_docx_mutation_output, zip_entry_names, zip_text,
};

pub(crate) struct DocxSectionSetupOptions<'a> {
    pub(crate) section: i64,
    pub(crate) orientation: &'a str,
    pub(crate) size: &'a str,
    pub(crate) margins: &'a str,
    pub(crate) mutation: DocxParagraphMutationOptions<'a>,
}

pub(crate) fn docx_break_insert(
    file: &str,
    page: bool,
    section: bool,
    options: DocxParagraphMutationOptions<'_>,
) -> CliResult<Value> {
    if page == section {
        return Err(CliError::invalid_args(
            "specify exactly one of --page or --section",
        ));
    }
    let entries = zip_entry_names(file)?;
    ensure_docx_package_kind(file, &entries)?;
    let document_part = find_docx_document_part(file, &entries)?;
    let xml = zip_text(file, &document_part)?;
    let body_tag = docx_body_tag(&xml)?;
    let prefix = docx_body_prefix(&body_tag);
    let (_, body_end) = docx_body_content_bounds(&xml, &body_tag)?;
    let sect_pr = crate::docx_headers::docx_section_ranges(&xml)?
        .last()
        .filter(|range| range.end == body_end)
        .copied();
    let insert_at = sect_pr.map_or(body_end, |range| range.start);
    let fragment = if page {
        format!(
            "<{p}><{r}><{br} {wtype}=\"page\"/></{r}></{p}>",
            p = word_xml_tag(&prefix, "p"),
            r = word_xml_tag(&prefix, "r"),
            br = word_xml_tag(&prefix, "br"),
            wtype = word_xml_tag(&prefix, "type"),
        )
    } else {
        let properties = sect_pr
            .map(|range| xml[range.start..range.end].to_string())
            .unwrap_or_else(|| format!("<{}/>", word_xml_tag(&prefix, "sectPr")));
        format!(
            "<{p}><{ppr}>{properties}</{ppr}></{p}>",
            p = word_xml_tag(&prefix, "p"),
            ppr = word_xml_tag(&prefix, "pPr"),
        )
    };
    let mut updated = String::with_capacity(xml.len() + fragment.len());
    updated.push_str(&xml[..insert_at]);
    updated.push_str(&fragment);
    updated.push_str(&xml[insert_at..]);
    write_docx_mutation_output(file, &document_part, &updated, options)?;
    Ok(json!({
        "command": "docx breaks insert",
        "file": file,
        "break": if page { "page" } else { "section" },
        "validation": "strict"
    }))
}

pub(crate) fn docx_section_set(
    file: &str,
    options: DocxSectionSetupOptions<'_>,
) -> CliResult<Value> {
    if options.section < 1 {
        return Err(CliError::invalid_args("--section must be >= 1"));
    }
    let orientation = match options.orientation.trim().to_ascii_lowercase().as_str() {
        "portrait" => "portrait",
        "landscape" => "landscape",
        _ => {
            return Err(CliError::invalid_args(
                "--orientation must be portrait or landscape",
            ));
        }
    };
    let (mut width, mut height, size) = match options.size.trim().to_ascii_lowercase().as_str() {
        "a4" => (11_906, 16_838, "A4"),
        "letter" => (12_240, 15_840, "Letter"),
        _ => return Err(CliError::invalid_args("--size must be A4 or Letter")),
    };
    if orientation == "landscape" {
        std::mem::swap(&mut width, &mut height);
    }
    let margins = parse_margins(options.margins)?;
    let entries = zip_entry_names(file)?;
    ensure_docx_package_kind(file, &entries)?;
    let document_part = find_docx_document_part(file, &entries)?;
    let xml = zip_text(file, &document_part)?;
    let sections = crate::docx_headers::docx_section_ranges(&xml)?;
    let range = sections.get(options.section as usize - 1).ok_or_else(|| {
        CliError::target_not_found(format!("section {} not found", options.section))
    })?;
    let prefix = docx_body_prefix(&docx_body_tag(&xml)?);
    let replacement = render_section_setup(
        &xml[range.start..range.end],
        &prefix,
        width,
        height,
        orientation,
        margins,
    )?;
    let mut updated = String::with_capacity(xml.len() + replacement.len());
    updated.push_str(&xml[..range.start]);
    updated.push_str(&replacement);
    updated.push_str(&xml[range.end..]);
    write_docx_mutation_output(file, &document_part, &updated, options.mutation)?;
    Ok(json!({
        "command": "docx sections set", "file": file, "section": options.section,
        "orientation": orientation, "size": size,
        "marginsTwips": {"top": margins[0], "right": margins[1], "bottom": margins[2], "left": margins[3]},
        "validation": "strict"
    }))
}

fn parse_margins(raw: &str) -> CliResult<[i64; 4]> {
    let values = raw.split(',').map(str::trim).collect::<Vec<_>>();
    if values.len() != 4 || values.iter().any(|value| value.is_empty()) {
        return Err(CliError::invalid_args(
            "--margins requires top,right,bottom,left",
        ));
    }
    let mut result = [0; 4];
    for (index, value) in values.into_iter().enumerate() {
        let emu = crate::cli_dispatch::units::parse_length(value, None)?;
        if emu < 0 {
            return Err(CliError::invalid_args(
                "section margins must be non-negative",
            ));
        }
        result[index] = emu / 635;
    }
    Ok(result)
}

fn render_section_setup(
    fragment: &str,
    prefix: &str,
    width: i64,
    height: i64,
    orientation: &str,
    margins: [i64; 4],
) -> CliResult<String> {
    let (open_end, tag, close_start, self_closing) = crate::xml_fragment_bounds(fragment)?;
    let mut before = String::new();
    let mut after = String::new();
    if !self_closing {
        for child in crate::xml_direct_child_ranges(fragment, open_end + 1, close_start)? {
            match child.kind.as_str() {
                "pgSz" | "pgMar" => {}
                "headerReference" | "footerReference" | "footnotePr" | "endnotePr" | "type" => {
                    before.push_str(&fragment[child.start..child.end]);
                }
                _ => after.push_str(&fragment[child.start..child.end]),
            }
        }
    }
    let opening = if self_closing {
        format!("{}>", fragment[..open_end].trim_end_matches('/'))
    } else {
        fragment[..=open_end].to_string()
    };
    let orient = if orientation == "landscape" {
        format!(" {}=\"landscape\"", word_xml_tag(prefix, "orient"))
    } else {
        String::new()
    };
    Ok(format!(
        "{opening}{before}<{pgsz} {w}=\"{width}\" {h}=\"{height}\"{orient}/><{pgmar} {top}=\"{}\" {right}=\"{}\" {bottom}=\"{}\" {left}=\"{}\"/>{after}</{tag}>",
        margins[0],
        margins[1],
        margins[2],
        margins[3],
        pgsz = word_xml_tag(prefix, "pgSz"),
        pgmar = word_xml_tag(prefix, "pgMar"),
        w = word_xml_tag(prefix, "w"),
        h = word_xml_tag(prefix, "h"),
        top = word_xml_tag(prefix, "top"),
        right = word_xml_tag(prefix, "right"),
        bottom = word_xml_tag(prefix, "bottom"),
        left = word_xml_tag(prefix, "left"),
    ))
}
