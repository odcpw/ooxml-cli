pub(crate) mod numbering;
mod properties;
mod settings;
pub(crate) mod styles;
mod theme;

use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::{
    CliError, CliResult, InspectPackageKind, command_arg, copy_zip_with_part_overrides,
    detect_inspect_package_type, docx_body_content_bounds, docx_body_prefix, docx_body_tag,
    ensure_content_type_override, ensure_package_root_relationship_xml, find_docx_document_part,
    package_mutation_temp_path, relationship_entries_from_xml, render_docx_paragraph,
    resolve_optional_docx_paragraph_text, resolve_relationship_target, xml_direct_child_ranges,
    zip_entry_names, zip_text,
};

const DOCUMENT_PART: &str = "word/document.xml";
const DOCUMENT_RELS_PART: &str = "word/_rels/document.xml.rels";
const STYLES_PART: &str = "word/styles.xml";
const NUMBERING_PART: &str = "word/numbering.xml";
const SETTINGS_PART: &str = "word/settings.xml";
const FONT_TABLE_PART: &str = "word/fontTable.xml";
const THEME_PART: &str = "word/theme/theme1.xml";
const CORE_PROPERTIES_PART: &str = "docProps/core.xml";
const APP_PROPERTIES_PART: &str = "docProps/app.xml";

const CORE_PROPERTIES_REL: &str =
    "http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties";
const APP_PROPERTIES_REL: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties";
const CORE_PROPERTIES_CONTENT_TYPE: &str =
    "application/vnd.openxmlformats-package.core-properties+xml";
const APP_PROPERTIES_CONTENT_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.extended-properties+xml";

pub(crate) struct DocxScaffoldOptions<'a> {
    pub(crate) text: Option<&'a str>,
    pub(crate) text_file: Option<&'a str>,
    pub(crate) theme: Option<&'a str>,
    pub(crate) theme_seed: Option<&'a str>,
    pub(crate) template: Option<&'a str>,
    pub(crate) force: bool,
    pub(crate) no_validate: bool,
}

pub(crate) fn docx_scaffold(output: &str, options: DocxScaffoldOptions<'_>) -> CliResult<Value> {
    validate_scaffold_output(output, options.force)?;
    if options.template.is_some() && (options.theme.is_some() || options.theme_seed.is_some()) {
        return Err(CliError::invalid_args(
            "--template cannot be combined with --theme or --theme-seed; the template theme is inherited",
        ));
    }

    let text = resolve_optional_docx_paragraph_text(options.text, options.text_file)?;
    let temp_path = package_mutation_temp_path(output, "docx-scaffold");
    let (theme_name, theme_seed) = if let Some(template) = options.template {
        write_docx_template_scaffold_package(template, &temp_path, &text)?;
        (None, None)
    } else {
        let (theme_name, theme_seed) = theme::theme_seed(options.theme, options.theme_seed)?;
        write_docx_scaffold_package(&temp_path, &text, &theme_name, &theme_seed)?;
        (Some(theme_name), Some(theme_seed))
    };

    if !options.no_validate {
        crate::validate_owned_mutation_output(&temp_path)?;
    }

    crate::finish_mutation_output(output, &temp_path, Some(output), false, None, false)?;

    Ok(docx_scaffold_result(
        output,
        &text,
        !options.no_validate,
        options.template,
        theme_name.as_deref(),
        theme_seed.as_deref(),
    ))
}

fn validate_scaffold_output(output: &str, force: bool) -> CliResult<()> {
    if output.trim().is_empty() {
        return Err(CliError::invalid_args("output path is required"));
    }
    let output_path = Path::new(output);
    if output_path.is_dir() {
        return Err(CliError::invalid_args("output path is a directory"));
    }
    if output_path.exists() && !force {
        return Err(CliError::invalid_args(
            "output file already exists; pass --force to replace it",
        ));
    }
    Ok(())
}

fn write_docx_scaffold_package(
    path: &str,
    text: &str,
    theme_name: &str,
    theme_seed: &str,
) -> CliResult<()> {
    if let Some(parent) = Path::new(path)
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|err| CliError::unexpected(err.to_string()))?;
    }
    let output = File::create(path).map_err(|err| CliError::unexpected(err.to_string()))?;
    let mut writer = ZipWriter::new(output);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let parts = [
        ("[Content_Types].xml", content_types_xml().to_string()),
        ("_rels/.rels", package_relationships_xml().to_string()),
        (CORE_PROPERTIES_PART, properties::core_properties_xml()?),
        (APP_PROPERTIES_PART, properties::app_properties_xml(text)),
        (DOCUMENT_PART, main_document_xml(text)),
        (DOCUMENT_RELS_PART, document_relationships_xml().to_string()),
        (STYLES_PART, styles::styles_xml().to_string()),
        (NUMBERING_PART, numbering::numbering_xml().to_string()),
        (SETTINGS_PART, settings::settings_xml().to_string()),
        (FONT_TABLE_PART, settings::font_table_xml().to_string()),
        (THEME_PART, theme::theme_xml(theme_name, theme_seed)?),
    ];
    for (name, body) in parts {
        write_zip_string(&mut writer, options, name, &body)?;
    }
    writer
        .finish()
        .map_err(|err| CliError::unexpected(err.to_string()))?;
    Ok(())
}

fn write_docx_template_scaffold_package(template: &str, path: &str, text: &str) -> CliResult<()> {
    let template_path = Path::new(template);
    if !template_path.is_file() {
        return Err(CliError::invalid_args(format!(
            "DOCX template does not exist or is not a file: {template}"
        )));
    }
    let entries = zip_entry_names(template)?;
    if detect_inspect_package_type(template, &entries) != InspectPackageKind::Docx {
        return Err(CliError::invalid_args(format!(
            "--template must name a DOCX package: {template}"
        )));
    }
    let document_part = find_docx_document_part(template, &entries)?;
    let document_xml = zip_text(template, &document_part)?;

    let root_relationships = zip_text(template, "_rels/.rels")?;
    let core_part = related_root_part(&root_relationships, CORE_PROPERTIES_REL)
        .unwrap_or_else(|| CORE_PROPERTIES_PART.to_string());
    let app_part = related_root_part(&root_relationships, APP_PROPERTIES_REL)
        .unwrap_or_else(|| APP_PROPERTIES_PART.to_string());
    let root_relationships = ensure_package_root_relationship_xml(
        ensure_package_root_relationship_xml(root_relationships, CORE_PROPERTIES_REL, &core_part),
        APP_PROPERTIES_REL,
        &app_part,
    );
    let content_types = zip_text(template, "[Content_Types].xml")?;
    let content_types = ensure_content_type_override(
        ensure_content_type_override(content_types, &core_part, CORE_PROPERTIES_CONTENT_TYPE)?,
        &app_part,
        APP_PROPERTIES_CONTENT_TYPE,
    )?;

    let mut overrides = BTreeMap::new();
    overrides.insert(
        document_part,
        replace_template_body_preserving_section(&document_xml, text)?,
    );
    overrides.insert("_rels/.rels".to_string(), root_relationships);
    overrides.insert("[Content_Types].xml".to_string(), content_types);
    overrides.insert(core_part, properties::core_properties_xml()?);
    overrides.insert(app_part, properties::app_properties_xml(text));
    copy_zip_with_part_overrides(template, path, &overrides)
}

fn related_root_part(root_relationships: &str, relationship_type: &str) -> Option<String> {
    relationship_entries_from_xml(root_relationships)
        .into_iter()
        .find(|relationship| relationship.rel_type == relationship_type)
        .map(|relationship| {
            resolve_relationship_target("/", &relationship.target)
                .trim_start_matches('/')
                .to_string()
        })
}

fn replace_template_body_preserving_section(xml: &str, text: &str) -> CliResult<String> {
    let body_tag = docx_body_tag(xml)?;
    let prefix = docx_body_prefix(&body_tag);
    let (content_start, content_end) = docx_body_content_bounds(xml, &body_tag)?;
    let section = xml_direct_child_ranges(xml, content_start, content_end)?
        .into_iter()
        .find(|child| child.kind == "sectPr")
        .map(|child| xml[child.start..child.end].to_string())
        .unwrap_or_else(|| section_properties_xml(&prefix));
    let mut updated = String::with_capacity(xml.len() + text.len());
    updated.push_str(&xml[..content_start]);
    updated.push_str(&render_docx_paragraph(&prefix, text, "Normal"));
    updated.push_str(&section);
    updated.push_str(&xml[content_end..]);
    Ok(updated)
}

fn write_zip_string(
    writer: &mut ZipWriter<File>,
    options: SimpleFileOptions,
    name: &str,
    body: &str,
) -> CliResult<()> {
    writer
        .start_file(name, options)
        .map_err(|err| CliError::unexpected(err.to_string()))?;
    writer
        .write_all(body.as_bytes())
        .map_err(|err| CliError::unexpected(err.to_string()))
}

fn main_document_xml(text: &str) -> String {
    let body = format!(
        "{}{}",
        render_docx_paragraph("w", text, "Normal"),
        section_properties_xml("w")
    );
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>{body}</w:body></w:document>"#
    )
}

fn section_properties_xml(prefix: &str) -> String {
    let qualified = |local: &str| {
        if prefix.is_empty() {
            local.to_string()
        } else {
            format!("{prefix}:{local}")
        }
    };
    let sect_pr = qualified("sectPr");
    let page_size = qualified("pgSz");
    let page_margin = qualified("pgMar");
    format!(
        r#"<{sect_pr}><{page_size} {}="12240" {}="15840"/><{page_margin} {}="1440" {}="1440" {}="1440" {}="1440" {}="720" {}="720" {}="0"/></{sect_pr}>"#,
        qualified("w"),
        qualified("h"),
        qualified("top"),
        qualified("right"),
        qualified("bottom"),
        qualified("left"),
        qualified("header"),
        qualified("footer"),
        qualified("gutter"),
    )
}

fn content_types_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/><Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/><Override PartName="/word/numbering.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml"/><Override PartName="/word/settings.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"/><Override PartName="/word/fontTable.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.fontTable+xml"/><Override PartName="/word/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/></Types>"#
}

fn package_relationships_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/><Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties" Target="docProps/app.xml"/></Relationships>"#
}

fn document_relationships_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering" Target="numbering.xml"/><Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings" Target="settings.xml"/><Relationship Id="rId4" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/fontTable" Target="fontTable.xml"/><Relationship Id="rId5" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="theme/theme1.xml"/></Relationships>"#
}

fn docx_scaffold_result(
    output: &str,
    text: &str,
    validated: bool,
    template: Option<&str>,
    theme_name: Option<&str>,
    theme_seed: Option<&str>,
) -> Value {
    let mut result = Map::new();
    result.insert("output".to_string(), json!(output));
    result.insert("created".to_string(), json!(true));
    result.insert("family".to_string(), json!("docx"));
    result.insert("documentPart".to_string(), json!(DOCUMENT_PART));
    result.insert("stylesPart".to_string(), json!(STYLES_PART));
    result.insert("numberingPart".to_string(), json!(NUMBERING_PART));
    result.insert("settingsPart".to_string(), json!(SETTINGS_PART));
    result.insert("fontTablePart".to_string(), json!(FONT_TABLE_PART));
    result.insert("themePart".to_string(), json!(THEME_PART));
    result.insert(
        "builtInStyleCount".to_string(),
        json!(styles::BUILT_IN_STYLES.len()),
    );
    result.insert("initialBlockCount".to_string(), json!(1));
    result.insert("initialText".to_string(), json!(text));
    result.insert("template".to_string(), json!(template));
    result.insert("theme".to_string(), json!(theme_name));
    result.insert("themeSeed".to_string(), json!(theme_seed));
    result.insert("validated".to_string(), json!(validated));
    result.insert(
        "validateCommand".to_string(),
        json!(format!("ooxml validate --strict {}", command_arg(output))),
    );
    result.insert(
        "conformanceCommand".to_string(),
        json!(format!(
            "ooxml --json conformance check {}",
            command_arg(output)
        )),
    );
    result.insert(
        "readbackCommand".to_string(),
        json!(format!("ooxml --json docx blocks {}", command_arg(output))),
    );
    Value::Object(result)
}
