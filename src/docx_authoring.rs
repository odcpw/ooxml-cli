use serde_json::{Map, Value, json};
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::{
    CliError, CliResult, EXIT_SUCCESS, command_arg, package_mutation_temp_path,
    render_docx_paragraph, resolve_optional_docx_paragraph_text, validate, validate_exit_code,
};

const DOCUMENT_PART: &str = "word/document.xml";

pub(crate) struct DocxScaffoldOptions<'a> {
    pub(crate) text: Option<&'a str>,
    pub(crate) text_file: Option<&'a str>,
    pub(crate) force: bool,
    pub(crate) no_validate: bool,
}

pub(crate) fn docx_scaffold(output: &str, options: DocxScaffoldOptions<'_>) -> CliResult<Value> {
    if output.trim().is_empty() {
        return Err(CliError::invalid_args("output path is required"));
    }
    let output_path = Path::new(output);
    if output_path.is_dir() {
        return Err(CliError::invalid_args("output path is a directory"));
    }
    if output_path.exists() && !options.force {
        return Err(CliError::invalid_args(
            "output file already exists; pass --force to replace it",
        ));
    }

    let text = resolve_optional_docx_paragraph_text(options.text, options.text_file)?;
    let temp_path = package_mutation_temp_path(output, "docx-scaffold");
    write_docx_scaffold_package(&temp_path, &text)?;

    if !options.no_validate {
        let report = validate(&temp_path, true)?;
        if validate_exit_code(&report, true) != EXIT_SUCCESS {
            let _ = fs::remove_file(&temp_path);
            return Err(CliError::validation_failed(
                "generated DOCX scaffold failed strict validation",
            ));
        }
    }

    if output_path.exists() {
        fs::remove_file(output_path)
            .map_err(|err| CliError::unexpected(format!("failed to replace output file: {err}")))?;
    }
    fs::rename(&temp_path, output_path)
        .or_else(|_| {
            fs::copy(&temp_path, output_path)?;
            fs::remove_file(&temp_path)
        })
        .map_err(|err| CliError::unexpected(format!("failed to write output file: {err}")))?;

    Ok(docx_scaffold_result(output, &text, !options.no_validate))
}

fn write_docx_scaffold_package(path: &str, text: &str) -> CliResult<()> {
    if let Some(parent) = Path::new(path)
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|err| CliError::unexpected(err.to_string()))?;
    }
    let output = File::create(path).map_err(|err| CliError::unexpected(err.to_string()))?;
    let mut writer = ZipWriter::new(output);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    write_zip_string(
        &mut writer,
        options,
        "[Content_Types].xml",
        content_types_xml(),
    )?;
    write_zip_string(
        &mut writer,
        options,
        "_rels/.rels",
        package_relationships_xml(),
    )?;
    write_zip_string(
        &mut writer,
        options,
        DOCUMENT_PART,
        &main_document_xml(text),
    )?;
    writer
        .finish()
        .map_err(|err| CliError::unexpected(err.to_string()))?;
    Ok(())
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
    let mut body = String::new();
    body.push_str(&render_docx_paragraph("w", text, ""));
    body.push_str(
        r#"<w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/></w:sectPr>"#,
    );
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>{body}</w:body></w:document>"#
    )
}

fn content_types_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#
}

fn package_relationships_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#
}

fn docx_scaffold_result(output: &str, text: &str, validated: bool) -> Value {
    let mut result = Map::new();
    result.insert("output".to_string(), json!(output));
    result.insert("created".to_string(), json!(true));
    result.insert("family".to_string(), json!("docx"));
    result.insert("documentPart".to_string(), json!(DOCUMENT_PART));
    result.insert("initialBlockCount".to_string(), json!(1));
    result.insert("initialText".to_string(), json!(text));
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
