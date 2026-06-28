use serde_json::{Map, Value, json};
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::{
    CliError, CliResult, command_arg, validate, validate_exit_code,
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
const CLEAR_MACRO_NAME: &str = "ClearEntryForm";
const SAMPLE_MACRO_NAME: &str = "FillSampleEntry";
const FIRST_INPUT_ROW: usize = 5;

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

    let macro_source = entry_macro_source(&form_sheet, &data_sheet, &fields);
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
        &form_controls_vml(button_caption, fields.len()),
    )?;
    write_zip_string(
        &mut writer,
        options,
        "xl/styles.xml",
        xlsx_forms_styles_xml(),
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

fn entry_macro_source(form_sheet: &str, data_sheet: &str, fields: &[String]) -> String {
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
    for (index, _field) in fields.iter().enumerate() {
        let row = FIRST_INPUT_ROW + index;
        let column = index + 2;
        source.push_str(&format!(
            "    dataSheet.Cells(nextRow, {column}).Value = formSheet.Range(\"B{row}\").Value\r\n"
        ));
    }
    source.push_str(
        "\r\n\
    dataSheet.Columns.AutoFit\r\n\
    ClearEntryFormCells formSheet\r\n",
    );
    source.push_str(&format!(
        "    formSheet.Range(\"B{FIRST_INPUT_ROW}\").Select\r\n\
    MsgBox \"Entry submitted.\", vbInformation, \"Entry Form\"\r\n\
End Sub\r\n\r\n\
Public Sub {CLEAR_MACRO_NAME}()\r\n\
    Dim formSheet As Worksheet\r\n\
    Set formSheet = ThisWorkbook.Worksheets(\"{}\")\r\n\
    ClearEntryFormCells formSheet\r\n\
    formSheet.Range(\"B{FIRST_INPUT_ROW}\").Select\r\n\
End Sub\r\n\r\n\
Public Sub {SAMPLE_MACRO_NAME}()\r\n\
    Dim formSheet As Worksheet\r\n\
    Set formSheet = ThisWorkbook.Worksheets(\"{}\")\r\n",
        vba_string(form_sheet),
        vba_string(form_sheet),
    ));
    for (index, field) in fields.iter().enumerate() {
        let row = FIRST_INPUT_ROW + index;
        source.push_str(&format!(
            "    formSheet.Range(\"B{row}\").Value = \"{}\"\r\n",
            vba_string(&sample_value(field, index))
        ));
    }
    source.push_str(&format!(
        "    formSheet.Range(\"B{FIRST_INPUT_ROW}\").Select\r\n\
End Sub\r\n\r\n\
Private Sub ClearEntryFormCells(ByVal formSheet As Worksheet)\r\n"
    ));
    for index in 0..fields.len() {
        let row = FIRST_INPUT_ROW + index;
        source.push_str(&format!(
            "    formSheet.Range(\"B{row}\").ClearContents\r\n"
        ));
    }
    source.push_str("End Sub\r\n");
    source
}

fn vba_string(value: &str) -> String {
    value.replace('"', "\"\"")
}

fn sample_value(field: &str, index: usize) -> String {
    match field.trim().to_ascii_lowercase().as_str() {
        "name" | "full name" => "Jane Designer".to_string(),
        "email" | "e-mail" => "jane@example.com".to_string(),
        "phone" | "telephone" => "+1 555 0100".to_string(),
        "company" | "organization" | "organisation" => "Example Studio".to_string(),
        "notes" | "note" | "message" => "Generated by ooxml-cli".to_string(),
        "date" => "2026-06-28".to_string(),
        _ => format!("Sample {}", index + 1),
    }
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

fn xlsx_forms_styles_xml() -> &'static str {
    r##"<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><fonts count="3"><font><sz val="11"/><name val="Aptos"/></font><font><b/><sz val="16"/><color rgb="FFFFFFFF"/><name val="Aptos Display"/></font><font><b/><sz val="11"/><color rgb="FF203040"/><name val="Aptos"/></font></fonts><fills count="5"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill><fill><patternFill patternType="solid"><fgColor rgb="FF1F4E79"/><bgColor indexed="64"/></patternFill></fill><fill><patternFill patternType="solid"><fgColor rgb="FFEAF3F8"/><bgColor indexed="64"/></patternFill></fill><fill><patternFill patternType="solid"><fgColor rgb="FFF4F6F8"/><bgColor indexed="64"/></patternFill></fill></fills><borders count="2"><border/><border><left style="thin"><color rgb="FFB7C9D6"/></left><right style="thin"><color rgb="FFB7C9D6"/></right><top style="thin"><color rgb="FFB7C9D6"/></top><bottom style="thin"><color rgb="FFB7C9D6"/></bottom><diagonal/></border></borders><cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs><cellXfs count="5"><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/><xf numFmtId="0" fontId="1" fillId="2" borderId="0" xfId="0" applyFont="1" applyFill="1"/><xf numFmtId="0" fontId="2" fillId="4" borderId="0" xfId="0" applyFont="1" applyFill="1"/><xf numFmtId="0" fontId="0" fillId="3" borderId="1" xfId="0" applyFill="1" applyBorder="1"/><xf numFmtId="0" fontId="2" fillId="4" borderId="1" xfId="0" applyFont="1" applyFill="1" applyBorder="1"/></cellXfs><cellStyles count="1"><cellStyle name="Normal" xfId="0" builtinId="0"/></cellStyles><dxfs count="0"/><tableStyles count="0" defaultTableStyle="TableStyleMedium2" defaultPivotStyle="PivotStyleLight16"/></styleSheet>"##
}

fn form_sheet_xml(fields: &[String]) -> String {
    let last_field_row = FIRST_INPUT_ROW + fields.len().saturating_sub(1);
    let button_row = last_field_row + 3;
    let bottom_row = button_row + 3;
    let dimension = format!("A1:F{bottom_row}");
    let mut rows = String::new();
    rows.push_str(&inline_string_row_styled(1, 24, &[("A", "Entry Form", 1)]));
    rows.push_str(&inline_string_row_styled(
        2,
        18,
        &[(
            "A",
            "Fill in the worksheet text inputs, then choose an action.",
            0,
        )],
    ));
    rows.push_str(&inline_string_row_styled(
        4,
        18,
        &[("A", "Field", 2), ("B", "Text input", 2)],
    ));
    for (index, field) in fields.iter().enumerate() {
        let row = FIRST_INPUT_ROW + index;
        rows.push_str(&field_input_row(row, field));
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheetPr codeName="Sheet1"/><dimension ref="{dimension}"/><sheetViews><sheetView showGridLines="0" workbookViewId="0"/></sheetViews><sheetFormatPr defaultRowHeight="15"/><cols><col min="1" max="1" width="20" customWidth="1"/><col min="2" max="2" width="42" customWidth="1"/><col min="3" max="3" width="3" customWidth="1"/><col min="4" max="6" width="16" customWidth="1"/></cols><sheetData>{rows}</sheetData><pageMargins left="0.7" right="0.7" top="0.75" bottom="0.75" header="0.3" footer="0.3"/><legacyDrawing r:id="rId1"/></worksheet>"#
    )
}

fn data_sheet_xml(fields: &[String]) -> String {
    let last_column = column_name(fields.len() + 1);
    let mut headers = vec![("A".to_string(), "Timestamp".to_string(), 4)];
    for (index, field) in fields.iter().enumerate() {
        headers.push((column_name(index + 2), field.clone(), 4));
    }
    let header_refs = headers
        .iter()
        .map(|(col, value, style)| (col.as_str(), value.as_str(), *style))
        .collect::<Vec<_>>();
    let row = inline_string_row_styled(1, 18, &header_refs);
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetPr codeName="Sheet2"/><dimension ref="A1:{last_column}1"/><sheetViews><sheetView workbookViewId="0"/></sheetViews><sheetFormatPr defaultRowHeight="15"/><cols><col min="1" max="1" width="21" customWidth="1"/><col min="2" max="{max_col}" width="24" customWidth="1"/></cols><sheetData>{row}</sheetData><pageMargins left="0.7" right="0.7" top="0.75" bottom="0.75" header="0.3" footer="0.3"/></worksheet>"#,
        max_col = fields.len() + 1,
    )
}

fn inline_string_row_styled(row: usize, height: usize, cells: &[(&str, &str, usize)]) -> String {
    let mut out = format!(r#"<row r="{row}" ht="{height}" customHeight="1">"#);
    for (column, value, style) in cells {
        let style_attr = if *style == 0 {
            String::new()
        } else {
            format!(r#" s="{style}""#)
        };
        out.push_str(&format!(
            r#"<c r="{column}{row}"{style_attr} t="inlineStr"><is><t>{}</t></is></c>"#,
            xml_escape(value)
        ));
    }
    out.push_str("</row>");
    out
}

fn field_input_row(row: usize, field: &str) -> String {
    format!(
        r#"<row r="{row}" ht="20" customHeight="1"><c r="A{row}" s="2" t="inlineStr"><is><t>{}</t></is></c><c r="B{row}" s="3"/><c r="C{row}" s="3"/><c r="D{row}" s="3"/></row>"#,
        xml_escape(field)
    )
}

fn form_controls_vml(button_caption: &str, field_count: usize) -> String {
    let frame_height = 120 + (field_count * 20);
    let button_top = 120 + (field_count * 20);
    let clear_top = button_top;
    let sample_top = button_top;
    format!(
        r##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><xml xmlns:v="urn:schemas-microsoft-com:vml" xmlns:o="urn:schemas-microsoft-com:office:office" xmlns:x="urn:schemas-microsoft-com:office:excel"><o:shapelayout v:ext="edit"><o:idmap v:ext="edit" data="1"/></o:shapelayout><v:shapetype id="_x0000_t201" coordsize="21600,21600" o:spt="201" path="m,l,21600r21600,l21600,xe"><v:stroke joinstyle="miter"/><v:path shadowok="f" o:extrusionok="f" strokeok="f" fillok="f" o:connecttype="rect"/><o:lock v:ext="edit" shapetype="t"/></v:shapetype>{frame}{label}{submit}{clear}{sample}</xml>"##,
        frame = group_box_vml(frame_height),
        label = label_vml(),
        submit = button_vml(
            "_x0000_s1027",
            "SubmitButton",
            205,
            button_top,
            92,
            28,
            button_caption,
            ENTRY_MACRO_NAME,
            "3, 5, 10, 5, 4, 55, 12, 10",
        ),
        clear = button_vml(
            "_x0000_s1028",
            "ClearButton",
            305,
            clear_top,
            86,
            28,
            "Clear",
            CLEAR_MACRO_NAME,
            "4, 10, 10, 5, 5, 35, 12, 10",
        ),
        sample = button_vml(
            "_x0000_s1029",
            "SampleButton",
            400,
            sample_top,
            110,
            28,
            "Fill Sample",
            SAMPLE_MACRO_NAME,
            "5, 10, 10, 5, 6, 55, 12, 10",
        ),
    )
}

fn group_box_vml(height: usize) -> String {
    format!(
        r##"<v:shape id="EntryFrame" o:spid="_x0000_s1025" type="#_x0000_t201" style="position:absolute;margin-left:15pt;margin-top:50pt;width:520pt;height:{height}pt;z-index:1;mso-wrap-style:tight" fillcolor="window [65]" strokecolor="windowText [64]" o:insetmode="auto"><v:fill color2="window [65]"/><o:lock v:ext="edit" rotation="t"/><v:textbox style="mso-direction-alt:auto" o:singleclick="f"><div style="text-align:left"><font face="Segoe UI" size="160" color="auto">Entry details</font></div></v:textbox><x:ClientData ObjectType="GBox"><x:SizeWithCells/><x:Anchor>0, 15, 2, 8, 7, 30, 13, 12</x:Anchor><x:NoThreeD/></x:ClientData></v:shape>"##
    )
}

fn label_vml() -> &'static str {
    r##"<v:shape id="EntryLabel" o:spid="_x0000_s1026" type="#_x0000_t201" style="position:absolute;margin-left:300pt;margin-top:64pt;width:120pt;height:18pt;z-index:2;mso-wrap-style:tight" filled="f" fillcolor="windowText [64]" strokecolor="windowText [64]" o:insetmode="auto"><v:fill color2="window [65]"/><o:lock v:ext="edit" rotation="t"/><v:textbox style="mso-direction-alt:auto" o:singleclick="f"><div style="text-align:left"><font face="Segoe UI" size="160" color="auto">Worksheet inputs</font></div></v:textbox><x:ClientData ObjectType="Label"><x:Anchor>4, 5, 3, 2, 5, 35, 4, 12</x:Anchor><x:AutoFill>False</x:AutoFill></x:ClientData></v:shape>"##
}

#[allow(clippy::too_many_arguments)]
fn button_vml(
    spid: &str,
    id: &str,
    left: usize,
    top: usize,
    width: usize,
    height: usize,
    caption: &str,
    macro_name: &str,
    anchor: &str,
) -> String {
    format!(
        r##"<v:shape id="{id}" o:spid="{spid}" type="#_x0000_t201" style="position:absolute;margin-left:{left}pt;margin-top:{top}pt;width:{width}pt;height:{height}pt;z-index:3;mso-wrap-style:tight" o:button="t" fillcolor="buttonFace [67]" strokecolor="windowText [64]" o:insetmode="auto"><v:fill color2="buttonFace [67]" o:detectmouseclick="t"/><v:shadow on="t" color="buttonShadow [65]" obscured="t"/><o:lock v:ext="edit" rotation="t"/><v:path o:connecttype="none"/><v:textbox style="mso-direction-alt:auto" o:singleclick="f"><div style="text-align:center"><font face="Aptos Narrow" size="220" color="#000000">{caption}</font></div></v:textbox><x:ClientData ObjectType="Button"><x:SizeWithCells/><x:Anchor>{anchor}</x:Anchor><x:PrintObject>False</x:PrintObject><x:AutoFill>False</x:AutoFill><x:FmlaMacro>{macro_name}</x:FmlaMacro><x:TextHAlign>Center</x:TextHAlign><x:TextVAlign>Center</x:TextVAlign></x:ClientData></v:shape>"##,
        caption = xml_escape(caption),
        macro_name = xml_escape(macro_name),
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
        json!(format!(
            "B{FIRST_INPUT_ROW}:B{}",
            FIRST_INPUT_ROW + fields.len() - 1
        )),
    );
    result.insert("textInputKind".to_string(), json!("worksheet-cells"));
    result.insert(
        "macros".to_string(),
        json!([ENTRY_MACRO_NAME, CLEAR_MACRO_NAME, SAMPLE_MACRO_NAME]),
    );
    result.insert(
        "controls".to_string(),
        json!([
            {
                "caption": "Entry details",
                "controlType": "GroupBox",
                "part": VML_DRAWING_PART,
            },
            {
                "caption": "Worksheet inputs",
                "controlType": "Label",
                "part": VML_DRAWING_PART,
            },
        ]),
    );
    result.insert(
        "buttons".to_string(),
        json!([
            {
                "caption": button_caption,
                "controlType": "FormControlButton",
                "macro": ENTRY_MACRO_NAME,
                "part": VML_DRAWING_PART,
            },
            {
                "caption": "Clear",
                "controlType": "FormControlButton",
                "macro": CLEAR_MACRO_NAME,
                "part": VML_DRAWING_PART,
            },
            {
                "caption": "Fill Sample",
                "controlType": "FormControlButton",
                "macro": SAMPLE_MACRO_NAME,
                "part": VML_DRAWING_PART,
            },
        ]),
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
        let source = entry_macro_source(
            "Form",
            "Entries",
            &["Name".to_string(), "Email".to_string(), "Notes".to_string()],
        );
        assert!(source.contains("Attribute VB_Name = \"EntryFormMacros\""));
        assert!(source.contains("Public Sub SubmitEntry()"));
        assert!(source.contains("Public Sub ClearEntryForm()"));
        assert!(source.contains("Public Sub FillSampleEntry()"));
        assert!(source.contains("ThisWorkbook.Worksheets(\"Form\")"));
        assert!(
            source.contains("dataSheet.Cells(nextRow, 4).Value = formSheet.Range(\"B7\").Value")
        );
        assert!(source.contains("formSheet.Range(\"B5\").ClearContents"));
        assert!(source.contains("formSheet.Range(\"B7\").ClearContents"));
        assert!(source.contains("formSheet.Range(\"B6\").Value = \"jane@example.com\""));
    }

    #[test]
    fn vml_controls_are_form_controls_not_activex() {
        let vml = form_controls_vml("Submit", 3);
        assert!(vml.contains(r#"ObjectType="Button""#));
        assert!(vml.contains(r#"ObjectType="GBox""#));
        assert!(vml.contains(r#"ObjectType="Label""#));
        assert!(vml.contains("<x:FmlaMacro>SubmitEntry</x:FmlaMacro>"));
        assert!(vml.contains("<x:FmlaMacro>ClearEntryForm</x:FmlaMacro>"));
        assert!(vml.contains("<x:FmlaMacro>FillSampleEntry</x:FmlaMacro>"));
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
