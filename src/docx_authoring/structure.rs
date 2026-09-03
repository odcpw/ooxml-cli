use serde_json::{Value, json};

use crate::{
    CliError, CliResult, DocxParagraphMutationOptions, docx_body_content_bounds, docx_body_prefix,
    docx_body_tag, ensure_docx_package_kind, find_docx_document_part, word_xml_tag,
    write_docx_mutation_output, zip_entry_names, zip_text,
};

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
