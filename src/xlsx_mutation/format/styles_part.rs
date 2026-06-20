use crate::{
    CliResult, allocate_relationship_id, normalize_xl_target, relationship_entries, zip_text,
};

pub(super) fn resolve_or_add_xlsx_styles_part(file: &str) -> CliResult<(String, Option<String>)> {
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

pub(super) fn default_xlsx_styles_xml() -> String {
    r#"<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><fonts count="1"><font/></fonts><fills count="2"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill></fills><borders count="1"><border/></borders><cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs><cellXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/></cellXfs><cellStyles count="1"><cellStyle name="Normal" xfId="0" builtinId="0"/></cellStyles></styleSheet>"#.to_string()
}
