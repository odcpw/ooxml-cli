use serde_json::{Map, Value, json};
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::{
    CliError, CliResult, EXIT_SUCCESS, command_arg, default_xlsx_styles_xml,
    package_mutation_temp_path, validate, validate_exit_code, xml_attr_escape, xml_escape,
};

const WORKBOOK_PART: &str = "xl/workbook.xml";
const WORKSHEET_PART: &str = "xl/worksheets/sheet1.xml";
const STYLES_PART: &str = "xl/styles.xml";

pub(crate) struct XlsxScaffoldOptions<'a> {
    pub(crate) sheet: Option<&'a str>,
    pub(crate) force: bool,
    pub(crate) no_validate: bool,
}

pub(crate) fn xlsx_scaffold(output: &str, options: XlsxScaffoldOptions<'_>) -> CliResult<Value> {
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

    let sheet_name = validate_xlsx_scaffold_sheet_name(options.sheet.unwrap_or("Sheet1"))?;
    let temp_path = package_mutation_temp_path(output, "xlsx-scaffold");
    write_xlsx_scaffold_package(&temp_path, &sheet_name)?;

    if !options.no_validate {
        let report = validate(&temp_path, true)?;
        if validate_exit_code(&report, true) != EXIT_SUCCESS {
            let _ = fs::remove_file(&temp_path);
            return Err(CliError::validation_failed(
                "generated XLSX scaffold failed strict validation",
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

    Ok(xlsx_scaffold_result(
        output,
        &sheet_name,
        !options.no_validate,
    ))
}

fn validate_xlsx_scaffold_sheet_name(value: &str) -> CliResult<String> {
    let name = value.trim();
    if name.is_empty() {
        return Err(CliError::invalid_args("--sheet cannot be empty"));
    }
    if name.chars().count() > 31 {
        return Err(CliError::invalid_args(
            "--sheet exceeds Excel's 31-character worksheet name limit",
        ));
    }
    if name
        .chars()
        .any(|ch| matches!(ch, '[' | ']' | ':' | '*' | '?' | '/' | '\\'))
    {
        return Err(CliError::invalid_args(
            "--sheet contains invalid Excel worksheet name characters: []:*?/\\",
        ));
    }
    if name.starts_with('\'') || name.ends_with('\'') {
        return Err(CliError::invalid_args(
            "--sheet cannot start or end with an apostrophe",
        ));
    }
    Ok(name.to_string())
}

fn write_xlsx_scaffold_package(path: &str, sheet_name: &str) -> CliResult<()> {
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
    write_zip_string(&mut writer, options, "docProps/core.xml", core_props_xml())?;
    write_zip_string(
        &mut writer,
        options,
        "docProps/app.xml",
        &app_props_xml(sheet_name),
    )?;
    write_zip_string(
        &mut writer,
        options,
        WORKBOOK_PART,
        &workbook_xml(sheet_name),
    )?;
    write_zip_string(
        &mut writer,
        options,
        "xl/_rels/workbook.xml.rels",
        workbook_relationships_xml(),
    )?;
    write_zip_string(&mut writer, options, WORKSHEET_PART, worksheet_xml())?;
    write_zip_string(
        &mut writer,
        options,
        STYLES_PART,
        &default_xlsx_styles_xml(),
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

fn content_types_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/><Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/><Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/></Types>"#
}

fn package_relationships_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/><Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties" Target="docProps/app.xml"/></Relationships>"#
}

fn core_props_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:dcmitype="http://purl.org/dc/dcmitype/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"><dc:creator>ooxml-cli</dc:creator><cp:lastModifiedBy>ooxml-cli</cp:lastModifiedBy></cp:coreProperties>"#
}

fn app_props_xml(sheet_name: &str) -> String {
    let escaped_sheet = xml_escape(sheet_name);
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties" xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes"><Application>ooxml-cli</Application><DocSecurity>0</DocSecurity><ScaleCrop>false</ScaleCrop><HeadingPairs><vt:vector size="2" baseType="variant"><vt:variant><vt:lpstr>Worksheets</vt:lpstr></vt:variant><vt:variant><vt:i4>1</vt:i4></vt:variant></vt:vector></HeadingPairs><TitlesOfParts><vt:vector size="1" baseType="lpstr"><vt:lpstr>{escaped_sheet}</vt:lpstr></vt:vector></TitlesOfParts></Properties>"#
    )
}

fn workbook_xml(sheet_name: &str) -> String {
    let escaped_sheet = xml_attr_escape(sheet_name);
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><workbookPr defaultThemeVersion="164011"/><bookViews><workbookView activeTab="0"/></bookViews><sheets><sheet name="{escaped_sheet}" sheetId="1" r:id="rId1"/></sheets><calcPr calcId="191029" calcMode="auto"/></workbook>"#
    )
}

fn workbook_relationships_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"#
}

fn worksheet_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><dimension ref="A1"/><sheetViews><sheetView workbookViewId="0"/></sheetViews><sheetFormatPr defaultRowHeight="15"/><sheetData/><pageMargins left="0.7" right="0.7" top="0.75" bottom="0.75" header="0.3" footer="0.3"/></worksheet>"#
}

fn xlsx_scaffold_result(output: &str, sheet_name: &str, validated: bool) -> Value {
    let mut result = Map::new();
    result.insert("output".to_string(), json!(output));
    result.insert("created".to_string(), json!(true));
    result.insert("family".to_string(), json!("xlsx"));
    result.insert("workbookPart".to_string(), json!(WORKBOOK_PART));
    result.insert("worksheetPart".to_string(), json!(WORKSHEET_PART));
    result.insert("stylesPart".to_string(), json!(STYLES_PART));
    result.insert("sheet".to_string(), json!(sheet_name));
    result.insert("sheetId".to_string(), json!("1"));
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
        json!(format!(
            "ooxml --json xlsx sheets list {}",
            command_arg(output)
        )),
    );
    result.insert(
        "rangeSetCommandTemplate".to_string(),
        json!(format!(
            "ooxml --json xlsx ranges set {} --sheet {} --anchor A1 --values <json|csv> --data-format json --out <output.xlsx>",
            command_arg(output),
            command_arg(sheet_name)
        )),
    );
    Value::Object(result)
}
