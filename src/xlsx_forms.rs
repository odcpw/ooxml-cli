use serde_json::{Map, Value, json};
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::{
    CliError, CliResult, command_arg, default_xlsx_styles_xml, validate, validate_exit_code,
    vba::vba_xlsx_standard_module_project_bin, xml_attr_escape, xml_escape,
};

const WORKBOOK_PART: &str = "xl/workbook.xml";
const FORM_SHEET_PART: &str = "xl/worksheets/sheet1.xml";
const DATA_SHEET_PART: &str = "xl/worksheets/sheet2.xml";
const FORM_SHEET_RELS_PART: &str = "xl/worksheets/_rels/sheet1.xml.rels";
const VML_DRAWING_PART: &str = "xl/drawings/vmlDrawing1.vml";
const VBA_PROJECT_PART: &str = "xl/vbaProject.bin";
const ENTRY_MODULE_NAME: &str = "EntryFormMacros";
const ENTRY_MACRO_NAME: &str = "SubmitEntry";

pub(crate) struct XlsxFormsEntryOptions<'a> {
    pub(crate) out: &'a str,
    pub(crate) fields: Vec<String>,
    pub(crate) form_sheet: Option<&'a str>,
    pub(crate) data_sheet: Option<&'a str>,
    pub(crate) button_caption: Option<&'a str>,
    pub(crate) force: bool,
    pub(crate) no_validate: bool,
}

pub(crate) fn xlsx_forms_entry(options: XlsxFormsEntryOptions<'_>) -> CliResult<Value> {
    let out = options.out.trim();
    if out.is_empty() {
        return Err(CliError::invalid_args("--out is required"));
    }
    let out_path = Path::new(out);
    if out_path.is_dir() {
        return Err(CliError::invalid_args("output path is a directory"));
    }
    if out_path
        .extension()
        .and_then(|value| value.to_str())
        .is_none_or(|ext| !ext.eq_ignore_ascii_case("xlsm"))
    {
        return Err(CliError::invalid_args(
            "xlsx forms entry writes a macro-enabled workbook; --out must end in .xlsm",
        ));
    }
    if out_path.exists() && !options.force {
        return Err(CliError::invalid_args(
            "output file already exists; pass --force to replace it",
        ));
    }

    let form_sheet = validate_sheet_name(options.form_sheet.unwrap_or("Form"), "--sheet")?;
    let data_sheet = validate_sheet_name(options.data_sheet.unwrap_or("Entries"), "--data-sheet")?;
    if form_sheet.eq_ignore_ascii_case(&data_sheet) {
        return Err(CliError::invalid_args(
            "--sheet and --data-sheet must be different worksheet names",
        ));
    }
    let fields = normalize_fields(options.fields)?;
    let button_caption =
        validate_short_text(options.button_caption.unwrap_or("Submit"), "--button", 80)?;

    let macro_source = entry_macro_source(&form_sheet, &data_sheet, fields.len());
    let vba_project = vba_xlsx_standard_module_project_bin(
        ENTRY_MODULE_NAME,
        &macro_source,
        &["Sheet1", "Sheet2"],
    )?;

    if let Some(parent) = out_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|err| {
            CliError::unexpected(format!("failed to create output directory: {err}"))
        })?;
    }
    let temp_path = crate::package_mutation_temp_path(out, "xlsx-forms-entry");
    write_entry_form_package(
        &temp_path,
        &form_sheet,
        &data_sheet,
        &fields,
        &button_caption,
        &vba_project,
    )?;

    if !options.no_validate {
        let report = validate(&temp_path, true)?;
        if validate_exit_code(&report, true) != crate::EXIT_SUCCESS {
            let _ = fs::remove_file(&temp_path);
            return Err(CliError::validation_failed(
                "generated entry form workbook failed strict validation",
            ));
        }
    }

    if out_path.exists() {
        fs::remove_file(out_path)
            .map_err(|err| CliError::unexpected(format!("failed to replace output file: {err}")))?;
    }
    fs::rename(&temp_path, out_path)
        .or_else(|_| {
            fs::copy(&temp_path, out_path)?;
            fs::remove_file(&temp_path)
        })
        .map_err(|err| CliError::unexpected(format!("failed to write output file: {err}")))?;

    Ok(entry_result(
        out,
        &form_sheet,
        &data_sheet,
        &fields,
        &button_caption,
        !options.no_validate,
    ))
}

fn write_entry_form_package(
    path: &str,
    form_sheet: &str,
    data_sheet: &str,
    fields: &[String],
    button_caption: &str,
    vba_project: &[u8],
) -> CliResult<()> {
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
        &app_props_xml(form_sheet, data_sheet),
    )?;
    write_zip_string(
        &mut writer,
        options,
        WORKBOOK_PART,
        &workbook_xml(form_sheet, data_sheet),
    )?;
    write_zip_string(
        &mut writer,
        options,
        "xl/_rels/workbook.xml.rels",
        workbook_relationships_xml(),
    )?;
    write_zip_string(
        &mut writer,
        options,
        FORM_SHEET_PART,
        &form_sheet_xml(fields),
    )?;
    write_zip_string(
        &mut writer,
        options,
        DATA_SHEET_PART,
        &data_sheet_xml(fields),
    )?;
    write_zip_string(
        &mut writer,
        options,
        FORM_SHEET_RELS_PART,
        form_sheet_relationships_xml(),
    )?;
    write_zip_string(
        &mut writer,
        options,
        VML_DRAWING_PART,
        &form_button_vml(button_caption),
    )?;
    write_zip_string(
        &mut writer,
        options,
        "xl/styles.xml",
        &default_xlsx_styles_xml(),
    )?;
    write_zip_bytes(&mut writer, options, VBA_PROJECT_PART, vba_project)?;

    writer
        .finish()
        .map_err(|err| CliError::unexpected(err.to_string()))?;
    Ok(())
}

fn normalize_fields(values: Vec<String>) -> CliResult<Vec<String>> {
    let raw = if values.is_empty() {
        vec!["Name".to_string(), "Email".to_string(), "Notes".to_string()]
    } else {
        values
    };
    let mut fields = Vec::new();
    for value in raw {
        for part in value.split(',') {
            let field = validate_short_text(part, "--field", 80)?;
            if !field.is_empty() {
                fields.push(field);
            }
        }
    }
    if fields.is_empty() {
        return Err(CliError::invalid_args(
            "at least one non-empty --field is required",
        ));
    }
    if fields.len() > 40 {
        return Err(CliError::invalid_args(
            "xlsx forms entry supports at most 40 fields",
        ));
    }
    Ok(fields)
}

fn validate_sheet_name(value: &str, flag: &str) -> CliResult<String> {
    let name = validate_short_text(value, flag, 31)?;
    if name.is_empty() {
        return Err(CliError::invalid_args(format!("{flag} cannot be empty")));
    }
    if name
        .chars()
        .any(|ch| matches!(ch, '[' | ']' | ':' | '*' | '?' | '/' | '\\'))
    {
        return Err(CliError::invalid_args(format!(
            "{flag} contains invalid Excel worksheet name characters: []:*?/\\"
        )));
    }
    if name.starts_with('\'') || name.ends_with('\'') {
        return Err(CliError::invalid_args(format!(
            "{flag} cannot start or end with an apostrophe"
        )));
    }
    Ok(name)
}

fn validate_short_text(value: &str, flag: &str, max_chars: usize) -> CliResult<String> {
    let text = value.trim();
    if text.chars().any(|ch| ch.is_control()) {
        return Err(CliError::invalid_args(format!(
            "{flag} cannot contain control characters"
        )));
    }
    if text.chars().count() > max_chars {
        return Err(CliError::invalid_args(format!(
            "{flag} must be at most {max_chars} characters"
        )));
    }
    Ok(text.to_string())
}

fn entry_macro_source(form_sheet: &str, data_sheet: &str, field_count: usize) -> String {
    let mut source = format!(
        "Attribute VB_Name = \"{ENTRY_MODULE_NAME}\"\r\n\
Option Explicit\r\n\r\n\
Public Sub {ENTRY_MACRO_NAME}()\r\n\
    Dim formSheet As Worksheet\r\n\
    Dim dataSheet As Worksheet\r\n\
    Set formSheet = ThisWorkbook.Worksheets(\"{}\")\r\n\
    Set dataSheet = ThisWorkbook.Worksheets(\"{}\")\r\n\r\n\
    Dim nextRow As Long\r\n\
    nextRow = dataSheet.Cells(dataSheet.Rows.Count, 1).End(xlUp).Row + 1\r\n\
    If nextRow < 2 Then nextRow = 2\r\n\r\n\
    dataSheet.Cells(nextRow, 1).Value = Now\r\n",
        vba_string(form_sheet),
        vba_string(data_sheet),
    );
    for index in 0..field_count {
        let row = index + 3;
        let column = index + 2;
        source.push_str(&format!(
            "    dataSheet.Cells(nextRow, {column}).Value = formSheet.Range(\"B{row}\").Value\r\n"
        ));
    }
    if field_count == 1 {
        source.push_str("    formSheet.Range(\"B3\").ClearContents\r\n");
    } else {
        source.push_str(&format!(
            "    formSheet.Range(\"B3:B{}\").ClearContents\r\n",
            field_count + 2
        ));
    }
    source.push_str("End Sub\r\n");
    source
}

fn vba_string(value: &str) -> String {
    value.replace('"', "\"\"")
}

fn content_types_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="vml" ContentType="application/vnd.openxmlformats-officedocument.vmlDrawing"/><Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/><Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.ms-excel.sheet.macroEnabled.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/><Override PartName="/xl/worksheets/sheet2.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/><Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/><Override PartName="/xl/vbaProject.bin" ContentType="application/vnd.ms-office.vbaProject"/></Types>"#
}

fn package_relationships_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/><Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties" Target="docProps/app.xml"/></Relationships>"#
}

fn core_props_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:dcmitype="http://purl.org/dc/dcmitype/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"><dc:creator>ooxml-cli</dc:creator><cp:lastModifiedBy>ooxml-cli</cp:lastModifiedBy></cp:coreProperties>"#
}

fn app_props_xml(form_sheet: &str, data_sheet: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties" xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes"><Application>ooxml-cli</Application><DocSecurity>0</DocSecurity><ScaleCrop>false</ScaleCrop><HeadingPairs><vt:vector size="2" baseType="variant"><vt:variant><vt:lpstr>Worksheets</vt:lpstr></vt:variant><vt:variant><vt:i4>2</vt:i4></vt:variant></vt:vector></HeadingPairs><TitlesOfParts><vt:vector size="2" baseType="lpstr"><vt:lpstr>{}</vt:lpstr><vt:lpstr>{}</vt:lpstr></vt:vector></TitlesOfParts></Properties>"#,
        xml_escape(form_sheet),
        xml_escape(data_sheet)
    )
}

fn workbook_xml(form_sheet: &str, data_sheet: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><workbookPr codeName="ThisWorkbook" defaultThemeVersion="164011"/><bookViews><workbookView activeTab="0"/></bookViews><sheets><sheet name="{}" sheetId="1" r:id="rId1"/><sheet name="{}" sheetId="2" r:id="rId2"/></sheets><calcPr calcId="191029" calcMode="auto"/></workbook>"#,
        xml_attr_escape(form_sheet),
        xml_attr_escape(data_sheet)
    )
}

fn workbook_relationships_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet2.xml"/><Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/><Relationship Id="rId4" Type="http://schemas.microsoft.com/office/2006/relationships/vbaProject" Target="vbaProject.bin"/></Relationships>"#
}

fn form_sheet_relationships_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/vmlDrawing" Target="../drawings/vmlDrawing1.vml"/></Relationships>"#
}

fn form_sheet_xml(fields: &[String]) -> String {
    let last_field_row = fields.len() + 2;
    let dimension = format!("A1:B{last_field_row}");
    let mut rows = String::new();
    rows.push_str(&inline_string_row(1, &[("A", "Entry Form")]));
    rows.push_str(&inline_string_row(2, &[("A", "Field"), ("B", "Value")]));
    for (index, field) in fields.iter().enumerate() {
        let row = index + 3;
        rows.push_str(&inline_string_row(row, &[("A", field)]));
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheetPr codeName="Sheet1"/><dimension ref="{dimension}"/><sheetViews><sheetView workbookViewId="0"/></sheetViews><sheetFormatPr defaultRowHeight="15"/><cols><col min="1" max="1" width="18" customWidth="1"/><col min="2" max="2" width="36" customWidth="1"/></cols><sheetData>{rows}</sheetData><pageMargins left="0.7" right="0.7" top="0.75" bottom="0.75" header="0.3" footer="0.3"/><legacyDrawing r:id="rId1"/></worksheet>"#
    )
}

fn data_sheet_xml(fields: &[String]) -> String {
    let last_column = column_name(fields.len() + 1);
    let mut headers = vec![("A".to_string(), "Timestamp".to_string())];
    for (index, field) in fields.iter().enumerate() {
        headers.push((column_name(index + 2), field.clone()));
    }
    let header_refs = headers
        .iter()
        .map(|(col, value)| (col.as_str(), value.as_str()))
        .collect::<Vec<_>>();
    let row = inline_string_row(1, &header_refs);
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetPr codeName="Sheet2"/><dimension ref="A1:{last_column}1"/><sheetViews><sheetView workbookViewId="0"/></sheetViews><sheetFormatPr defaultRowHeight="15"/><sheetData>{row}</sheetData><pageMargins left="0.7" right="0.7" top="0.75" bottom="0.75" header="0.3" footer="0.3"/></worksheet>"#
    )
}

fn inline_string_row(row: usize, cells: &[(&str, &str)]) -> String {
    let mut out = format!(r#"<row r="{row}">"#);
    for (column, value) in cells {
        out.push_str(&format!(
            r#"<c r="{column}{row}" t="inlineStr"><is><t>{}</t></is></c>"#,
            xml_escape(value)
        ));
    }
    out.push_str("</row>");
    out
}

fn form_button_vml(button_caption: &str) -> String {
    format!(
        r##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><xml xmlns:v="urn:schemas-microsoft-com:vml" xmlns:o="urn:schemas-microsoft-com:office:office" xmlns:x="urn:schemas-microsoft-com:office:excel"><o:shapelayout v:ext="edit"><o:idmap v:ext="edit" data="1"/></o:shapelayout><v:shapetype id="_x0000_t201" coordsize="21600,21600" o:spt="201" path="m,l,21600r21600,l21600,xe"><v:stroke joinstyle="miter"/><v:path shadowok="f" o:extrusionok="f" strokeok="f" fillok="f" o:connecttype="rect"/></v:shapetype><v:shape id="_x0000_s1025" type="#_x0000_t201" style="position:absolute;margin-left:150pt;margin-top:165pt;width:105pt;height:28pt;z-index:1;mso-wrap-style:tight" fillcolor="buttonFace [67]" strokecolor="windowText [64]" o:button="t"><v:fill color2="buttonFace [67]"/><v:shadow on="t" color="buttonShadow [65]" obscured="t"/><v:path o:connecttype="none"/><v:textbox style="mso-direction-alt:auto" o:singleclick="f"><div style="text-align:center">{}</div></v:textbox><x:ClientData ObjectType="Button"><x:SizeWithCells/><x:Anchor>2, 15, 8, 5, 4, 15, 10, 5</x:Anchor><x:PrintObject>False</x:PrintObject><x:AutoFill>False</x:AutoFill><x:FmlaMacro>{ENTRY_MACRO_NAME}</x:FmlaMacro><x:TextHAlign>Center</x:TextHAlign><x:TextVAlign>Center</x:TextVAlign></x:ClientData></v:shape></xml>"##,
        xml_escape(button_caption)
    )
}

fn column_name(mut number: usize) -> String {
    let mut out = Vec::new();
    while number > 0 {
        number -= 1;
        out.push((b'A' + (number % 26) as u8) as char);
        number /= 26;
    }
    out.iter().rev().collect()
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

fn write_zip_bytes(
    writer: &mut ZipWriter<File>,
    options: SimpleFileOptions,
    name: &str,
    body: &[u8],
) -> CliResult<()> {
    writer
        .start_file(name, options)
        .map_err(|err| CliError::unexpected(err.to_string()))?;
    writer
        .write_all(body)
        .map_err(|err| CliError::unexpected(err.to_string()))
}

fn entry_result(
    out: &str,
    form_sheet: &str,
    data_sheet: &str,
    fields: &[String],
    button_caption: &str,
    validated: bool,
) -> Value {
    let mut result = Map::new();
    result.insert("output".to_string(), json!(out));
    result.insert("created".to_string(), json!(true));
    result.insert("family".to_string(), json!("xlsx"));
    result.insert("macroEnabled".to_string(), json!(true));
    result.insert("formKind".to_string(), json!("worksheet-form-control"));
    result.insert("activeX".to_string(), json!(false));
    result.insert("formSheet".to_string(), json!(form_sheet));
    result.insert("dataSheet".to_string(), json!(data_sheet));
    result.insert("fields".to_string(), json!(fields));
    result.insert(
        "inputRange".to_string(),
        json!(format!("B3:B{}", fields.len() + 2)),
    );
    result.insert(
        "button".to_string(),
        json!({
            "caption": button_caption,
            "controlType": "FormControlButton",
            "macro": ENTRY_MACRO_NAME,
            "part": VML_DRAWING_PART,
        }),
    );
    result.insert("vbaProjectPart".to_string(), json!(VBA_PROJECT_PART));
    result.insert("validated".to_string(), json!(validated));
    result.insert(
        "validateCommand".to_string(),
        json!(format!(
            "ooxml --json --strict validate {}",
            command_arg(out)
        )),
    );
    result.insert(
        "conformanceCommand".to_string(),
        json!(format!(
            "ooxml --json conformance check {}",
            command_arg(out)
        )),
    );
    result.insert(
        "vbaListCommand".to_string(),
        json!(format!("ooxml --json vba list {}", command_arg(out))),
    );
    Value::Object(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macro_source_reads_input_cells_appends_row_and_clears_inputs() {
        let source = entry_macro_source("Form", "Entries", 3);
        assert!(source.contains("Attribute VB_Name = \"EntryFormMacros\""));
        assert!(source.contains("Public Sub SubmitEntry()"));
        assert!(source.contains("ThisWorkbook.Worksheets(\"Form\")"));
        assert!(
            source.contains("dataSheet.Cells(nextRow, 4).Value = formSheet.Range(\"B5\").Value")
        );
        assert!(source.contains("formSheet.Range(\"B3:B5\").ClearContents"));
    }

    #[test]
    fn vml_button_is_form_control_not_activex() {
        let vml = form_button_vml("Submit");
        assert!(vml.contains(r#"ObjectType="Button""#));
        assert!(vml.contains("<x:FmlaMacro>SubmitEntry</x:FmlaMacro>"));
        assert!(!vml.to_ascii_lowercase().contains("activex"));
        assert!(!vml.contains("OLEObject"));
    }

    #[test]
    fn fields_default_and_split_comma_values() {
        assert_eq!(
            normalize_fields(Vec::new()).unwrap(),
            vec!["Name".to_string(), "Email".to_string(), "Notes".to_string()]
        );
        assert_eq!(
            normalize_fields(vec!["One, Two".to_string(), "Three".to_string()]).unwrap(),
            vec!["One".to_string(), "Two".to_string(), "Three".to_string()]
        );
    }
}
