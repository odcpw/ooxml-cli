use serde_json::{Map, Value, json};
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::{
    CliError, CliResult, command_arg, package_mutation_temp_path, xml_attr_escape, xml_escape,
};

const WORKBOOK_PART: &str = "xl/workbook.xml";
const STYLES_PART: &str = "xl/styles.xml";
const THEME_PART: &str = "xl/theme/theme1.xml";

pub(crate) struct XlsxScaffoldOptions<'a> {
    pub(crate) sheets: Vec<String>,
    pub(crate) theme: Option<&'a str>,
    pub(crate) theme_seed: Option<&'a str>,
    pub(crate) brand: Option<&'a str>,
    pub(crate) force: bool,
    pub(crate) no_validate: bool,
}

struct XlsxScaffoldTheme {
    name: String,
    seed: String,
    major_font: String,
    minor_font: String,
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

    let sheet_names = validate_xlsx_scaffold_sheet_names(options.sheets)?;
    let theme = resolve_xlsx_scaffold_theme(options.theme, options.theme_seed, options.brand)?;
    let temp_path = package_mutation_temp_path(output, "xlsx-scaffold");
    write_xlsx_scaffold_package(&temp_path, &sheet_names, &theme)?;
    if let Some(brand) = options.brand {
        crate::brand::apply_to_staged_package(&temp_path, brand)?;
    }

    if !options.no_validate {
        crate::validate_owned_mutation_output(&temp_path)?;
    }

    crate::finish_mutation_output(output, &temp_path, Some(output), false, None, false)?;

    Ok(xlsx_scaffold_result(
        output,
        &sheet_names,
        &theme,
        !options.no_validate,
    ))
}

fn validate_xlsx_scaffold_sheet_names(values: Vec<String>) -> CliResult<Vec<String>> {
    let values = if values.is_empty() {
        vec!["Sheet1".to_string()]
    } else {
        values
    };
    let mut names = Vec::with_capacity(values.len());
    for value in values {
        let name = validate_xlsx_scaffold_sheet_name(&value)?;
        if names
            .iter()
            .any(|existing: &String| existing.eq_ignore_ascii_case(&name))
        {
            return Err(CliError::invalid_args(format!(
                "duplicate --sheet name {name:?}"
            )));
        }
        names.push(name);
    }
    Ok(names)
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

fn write_xlsx_scaffold_package(
    path: &str,
    sheet_names: &[String],
    theme: &XlsxScaffoldTheme,
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
    write_zip_string(
        &mut writer,
        options,
        "[Content_Types].xml",
        &content_types_xml(sheet_names.len()),
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
        &app_props_xml(sheet_names),
    )?;
    write_zip_string(
        &mut writer,
        options,
        WORKBOOK_PART,
        &workbook_xml(sheet_names),
    )?;
    write_zip_string(
        &mut writer,
        options,
        "xl/_rels/workbook.xml.rels",
        &workbook_relationships_xml(sheet_names.len()),
    )?;
    for (index, _) in sheet_names.iter().enumerate() {
        write_zip_string(
            &mut writer,
            options,
            &format!("xl/worksheets/sheet{}.xml", index + 1),
            worksheet_xml(),
        )?;
    }
    write_zip_string(
        &mut writer,
        options,
        STYLES_PART,
        &themed_xlsx_styles_xml(&theme.minor_font),
    )?;
    write_zip_string(&mut writer, options, THEME_PART, &xlsx_theme_xml(theme)?)?;
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

fn content_types_xml(sheet_count: usize) -> String {
    let worksheets = (1..=sheet_count)
        .map(|index| format!(r#"<Override PartName="/xl/worksheets/sheet{index}.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>"#))
        .collect::<String>();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/><Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>{worksheets}<Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/><Override PartName="/xl/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/></Types>"#
    )
}

fn package_relationships_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/><Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties" Target="docProps/app.xml"/></Relationships>"#
}

fn core_props_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:dcmitype="http://purl.org/dc/dcmitype/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"><dc:creator>ooxml-cli</dc:creator><cp:lastModifiedBy>ooxml-cli</cp:lastModifiedBy></cp:coreProperties>"#
}

fn app_props_xml(sheet_names: &[String]) -> String {
    let titles = sheet_names
        .iter()
        .map(|name| format!("<vt:lpstr>{}</vt:lpstr>", xml_escape(name)))
        .collect::<String>();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties" xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes"><Application>ooxml-cli</Application><DocSecurity>0</DocSecurity><ScaleCrop>false</ScaleCrop><HeadingPairs><vt:vector size="2" baseType="variant"><vt:variant><vt:lpstr>Worksheets</vt:lpstr></vt:variant><vt:variant><vt:i4>{}</vt:i4></vt:variant></vt:vector></HeadingPairs><TitlesOfParts><vt:vector size="{}" baseType="lpstr">{titles}</vt:vector></TitlesOfParts></Properties>"#,
        sheet_names.len(),
        sheet_names.len(),
    )
}

fn workbook_xml(sheet_names: &[String]) -> String {
    let sheets = sheet_names
        .iter()
        .enumerate()
        .map(|(index, name)| {
            format!(
                r#"<sheet name="{}" sheetId="{}" r:id="rId{}"/>"#,
                xml_attr_escape(name),
                index + 1,
                index + 1
            )
        })
        .collect::<String>();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><workbookPr defaultThemeVersion="164011"/><bookViews><workbookView activeTab="0"/></bookViews><sheets>{sheets}</sheets><calcPr calcId="191029" calcMode="auto"/></workbook>"#
    )
}

fn workbook_relationships_xml(sheet_count: usize) -> String {
    let worksheets = (1..=sheet_count)
        .map(|index| format!(r#"<Relationship Id="rId{index}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet{index}.xml"/>"#))
        .collect::<String>();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{worksheets}<Relationship Id="rId{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/><Relationship Id="rId{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="theme/theme1.xml"/></Relationships>"#,
        sheet_count + 1,
        sheet_count + 2
    )
}

fn worksheet_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><dimension ref="A1"/><sheetViews><sheetView workbookViewId="0"/></sheetViews><sheetFormatPr defaultRowHeight="15"/><sheetData/><pageMargins left="0.7" right="0.7" top="0.75" bottom="0.75" header="0.3" footer="0.3"/></worksheet>"#
}

fn resolve_xlsx_scaffold_theme(
    theme: Option<&str>,
    theme_seed: Option<&str>,
    brand: Option<&str>,
) -> CliResult<XlsxScaffoldTheme> {
    let selected = usize::from(theme.is_some())
        + usize::from(theme_seed.is_some())
        + usize::from(brand.is_some());
    if selected > 1 {
        return Err(CliError::invalid_args(
            "use only one of --theme, --theme-seed, or --brand",
        ));
    }
    if let Some(path) = brand {
        let kit = crate::brand::BrandKit::load(path)?;
        return build_xlsx_scaffold_theme(
            &kit.name,
            &kit.theme_seed(),
            &kit.fonts.heading,
            &kit.fonts.body,
        );
    }
    if let Some(seed) = theme_seed {
        return build_xlsx_scaffold_theme("custom", seed, "Aptos Display", "Aptos");
    }
    let name = theme.unwrap_or("corporate").trim().to_ascii_lowercase();
    let seed = match name.as_str() {
        "neutral" => "5B6573",
        "corporate" | "corporate-blue" => "4472C4",
        "warm" => "C55A11",
        "dark" => "4F46E5",
        _ => {
            return Err(CliError::invalid_args(format!(
                "unknown XLSX theme {name:?}; expected neutral, corporate, corporate-blue, warm, or dark"
            )));
        }
    };
    build_xlsx_scaffold_theme(&name, seed, "Aptos Display", "Aptos")
}

fn build_xlsx_scaffold_theme(
    name: &str,
    seed: &str,
    major_font: &str,
    minor_font: &str,
) -> CliResult<XlsxScaffoldTheme> {
    crate::palette::ThemePalette::derive(seed)
        .map_err(|err| CliError::invalid_args(err.to_string()))?;
    if name.is_empty() || major_font.trim().is_empty() || minor_font.trim().is_empty() {
        return Err(CliError::invalid_args(
            "theme name and brand fonts cannot be empty",
        ));
    }
    Ok(XlsxScaffoldTheme {
        name: name.to_string(),
        seed: seed.trim_start_matches('#').to_ascii_uppercase(),
        major_font: major_font.to_string(),
        minor_font: minor_font.to_string(),
    })
}

fn themed_xlsx_styles_xml(minor_font: &str) -> String {
    format!(
        r#"<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><fonts count="1"><font><sz val="11"/><color theme="1"/><name val="{}"/><family val="2"/><scheme val="minor"/></font></fonts><fills count="2"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill></fills><borders count="1"><border/></borders><cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs><cellXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/></cellXfs><cellStyles count="1"><cellStyle name="Normal" xfId="0" builtinId="0"/></cellStyles><dxfs count="0"/><tableStyles count="0" defaultTableStyle="TableStyleMedium2" defaultPivotStyle="PivotStyleLight16"/></styleSheet>"#,
        xml_attr_escape(minor_font)
    )
}

fn xlsx_theme_xml(theme: &XlsxScaffoldTheme) -> CliResult<String> {
    let palette = crate::palette::ThemePalette::derive(&theme.seed)
        .map_err(|err| CliError::invalid_args(err.to_string()))?;
    let color = |value: crate::palette::Srgb| value.to_hex();
    let name = xml_attr_escape(&theme.name);
    Ok(format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="ooxml-cli {name}"><a:themeElements><a:clrScheme name="ooxml-cli {name}"><a:dk1><a:srgbClr val="{}"/></a:dk1><a:lt1><a:srgbClr val="{}"/></a:lt1><a:dk2><a:srgbClr val="{}"/></a:dk2><a:lt2><a:srgbClr val="{}"/></a:lt2><a:accent1><a:srgbClr val="{}"/></a:accent1><a:accent2><a:srgbClr val="{}"/></a:accent2><a:accent3><a:srgbClr val="{}"/></a:accent3><a:accent4><a:srgbClr val="{}"/></a:accent4><a:accent5><a:srgbClr val="{}"/></a:accent5><a:accent6><a:srgbClr val="{}"/></a:accent6><a:hlink><a:srgbClr val="{}"/></a:hlink><a:folHlink><a:srgbClr val="{}"/></a:folHlink></a:clrScheme><a:fontScheme name="ooxml-cli"><a:majorFont><a:latin typeface="{}"/><a:ea typeface=""/><a:cs typeface=""/></a:majorFont><a:minorFont><a:latin typeface="{}"/><a:ea typeface=""/><a:cs typeface=""/></a:minorFont></a:fontScheme><a:fmtScheme name="ooxml-cli"><a:fillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"><a:tint val="95000"/></a:schemeClr></a:solidFill><a:solidFill><a:schemeClr val="phClr"><a:shade val="65000"/></a:schemeClr></a:solidFill></a:fillStyleLst><a:lnStyleLst><a:ln w="6350" cap="flat" cmpd="sng" algn="ctr"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:prstDash val="solid"/><a:miter lim="800000"/></a:ln><a:ln w="12700" cap="flat" cmpd="sng" algn="ctr"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:prstDash val="solid"/><a:miter lim="800000"/></a:ln><a:ln w="19050" cap="flat" cmpd="sng" algn="ctr"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:prstDash val="solid"/><a:miter lim="800000"/></a:ln></a:lnStyleLst><a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle></a:effectStyleLst><a:bgFillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"><a:tint val="95000"/></a:schemeClr></a:solidFill><a:solidFill><a:schemeClr val="phClr"><a:shade val="65000"/></a:schemeClr></a:solidFill></a:bgFillStyleLst></a:fmtScheme></a:themeElements></a:theme>"#,
        color(palette.dk1),
        color(palette.lt1),
        color(palette.dk2),
        color(palette.lt2),
        color(palette.accent1),
        color(palette.accent2),
        color(palette.accent3),
        color(palette.accent4),
        color(palette.accent5),
        color(palette.accent6),
        color(palette.hlink),
        color(palette.fol_hlink),
        xml_attr_escape(&theme.major_font),
        xml_attr_escape(&theme.minor_font),
    ))
}

fn xlsx_scaffold_result(
    output: &str,
    sheet_names: &[String],
    theme: &XlsxScaffoldTheme,
    validated: bool,
) -> Value {
    let mut result = Map::new();
    result.insert("output".to_string(), json!(output));
    result.insert("created".to_string(), json!(true));
    result.insert("family".to_string(), json!("xlsx"));
    result.insert("workbookPart".to_string(), json!(WORKBOOK_PART));
    result.insert(
        "worksheetPart".to_string(),
        json!("xl/worksheets/sheet1.xml"),
    );
    result.insert("stylesPart".to_string(), json!(STYLES_PART));
    result.insert("themePart".to_string(), json!(THEME_PART));
    result.insert("sheet".to_string(), json!(sheet_names[0]));
    result.insert("sheets".to_string(), json!(sheet_names));
    result.insert("sheetCount".to_string(), json!(sheet_names.len()));
    result.insert("sheetId".to_string(), json!("1"));
    result.insert("theme".to_string(), json!(theme.name));
    result.insert("themeSeed".to_string(), json!(theme.seed));
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
            command_arg(&sheet_names[0])
        )),
    );
    Value::Object(result)
}
