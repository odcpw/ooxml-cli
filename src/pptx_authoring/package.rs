use serde::Deserialize;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::palette::{Srgb, ThemePalette};
use crate::{CliError, CliResult, xml_escape};

pub(super) const PRESENTATION_PART: &str = "ppt/presentation.xml";
pub(super) const SLIDE_PART: &str = "ppt/slides/slide1.xml";
pub(super) const SLIDE_LAYOUT_PART: &str = "ppt/slideLayouts/slideLayout1.xml";
pub(super) const SLIDE_MASTER_PART: &str = "ppt/slideMasters/slideMaster1.xml";
pub(super) const THEME_PART: &str = "ppt/theme/theme1.xml";
pub(super) const TABLE_STYLES_PART: &str = "ppt/tableStyles.xml";

const LAYOUT_DATA: &str = include_str!("data/layouts.json");
const MASTER_TEXT_STYLE_DATA: &str = include_str!("data/master-text-styles.json");

#[derive(Clone, Debug)]
pub(super) struct SlideSize {
    pub(super) name: String,
    pub(super) width: i64,
    pub(super) height: i64,
    preset: String,
    presentation_format: String,
}

impl SlideSize {
    pub(super) fn parse(value: Option<&str>) -> CliResult<Self> {
        match value.unwrap_or("16:9").trim().to_ascii_lowercase().as_str() {
            "16:9" => Ok(Self {
                name: "16:9".to_string(),
                width: 12_192_000,
                height: 6_858_000,
                preset: "screen16x9".to_string(),
                presentation_format: "On-screen Show (16:9)".to_string(),
            }),
            "4:3" => Ok(Self {
                name: "4:3".to_string(),
                width: 9_144_000,
                height: 6_858_000,
                preset: "screen4x3".to_string(),
                presentation_format: "On-screen Show (4:3)".to_string(),
            }),
            "a4" => Ok(Self {
                name: "A4".to_string(),
                width: 10_692_000,
                height: 7_560_000,
                preset: "A4".to_string(),
                presentation_format: "A4 Paper (210x297 mm)".to_string(),
            }),
            other => Err(CliError::invalid_args(format!(
                "unknown PPTX slide size {other:?}; expected 16:9, 4:3, or A4"
            ))),
        }
    }

    pub(super) fn imported(width: i64, height: i64, preset: String) -> CliResult<Self> {
        if width <= 0 || height <= 0 {
            return Err(CliError::invalid_args(
                "PPTX template has a missing or invalid slide size",
            ));
        }
        let (name, presentation_format) = match preset.as_str() {
            "screen16x9" => ("16:9", "On-screen Show (16:9)"),
            "screen4x3" => ("4:3", "On-screen Show (4:3)"),
            "A4" => ("A4", "A4 Paper (210x297 mm)"),
            _ => ("template", "Template"),
        };
        Ok(Self {
            name: name.to_string(),
            width,
            height,
            preset,
            presentation_format: presentation_format.to_string(),
        })
    }
}

#[derive(Clone, Debug)]
pub(super) struct ThemeChoice {
    pub(super) name: String,
    pub(super) seed: Option<String>,
    palette: ThemePalette,
}

impl ThemeChoice {
    pub(super) fn resolve(theme: Option<&str>, seed: Option<&str>) -> CliResult<Self> {
        if theme.is_some() && seed.is_some() {
            return Err(CliError::invalid_args(
                "--theme and --theme-seed cannot be used together",
            ));
        }
        if let Some(seed) = seed {
            let normalized = seed.trim_start_matches('#').to_ascii_uppercase();
            let palette = ThemePalette::derive(seed)
                .map_err(|err| CliError::invalid_args(err.to_string()))?;
            return Ok(Self {
                name: "custom".to_string(),
                seed: Some(normalized),
                palette,
            });
        }

        let name = theme.unwrap_or("neutral").trim().to_ascii_lowercase();
        let palette = match name.as_str() {
            "neutral" => ThemePalette::derive("5B6573"),
            "corporate" => ThemePalette::derive("4472C4"),
            "warm" => ThemePalette::derive("C55A11"),
            "dark" => {
                return Ok(Self {
                    name,
                    seed: None,
                    palette: dark_palette(),
                });
            }
            _ => {
                return Err(CliError::invalid_args(format!(
                    "unknown PPTX theme {name:?}; expected neutral, corporate, warm, or dark"
                )));
            }
        }
        .map_err(|err| CliError::invalid_args(err.to_string()))?;
        Ok(Self {
            name,
            seed: None,
            palette,
        })
    }
}

fn dark_palette() -> ThemePalette {
    ThemePalette {
        dk1: Srgb::new(249, 250, 251),
        lt1: Srgb::new(17, 24, 39),
        dk2: Srgb::new(229, 231, 235),
        lt2: Srgb::new(31, 41, 55),
        accent1: Srgb::new(96, 165, 250),
        accent2: Srgb::new(251, 146, 60),
        accent3: Srgb::new(52, 211, 153),
        accent4: Srgb::new(250, 204, 21),
        accent5: Srgb::new(192, 132, 252),
        accent6: Srgb::new(244, 114, 182),
        hlink: Srgb::new(125, 211, 252),
        fol_hlink: Srgb::new(216, 180, 254),
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LayoutSpec {
    name: String,
    layout_type: String,
    placeholders: Vec<PlaceholderSpec>,
}

#[derive(Clone, Debug, Deserialize)]
struct PlaceholderSpec {
    name: String,
    #[serde(rename = "type")]
    placeholder_type: String,
    #[serde(default)]
    idx: Option<u32>,
    rect: [i64; 4],
    #[serde(default)]
    align: Option<String>,
    #[serde(default)]
    vertical: bool,
}

#[derive(Clone, Debug, Deserialize)]
struct MasterTextStyles {
    title: Vec<u32>,
    body: Vec<u32>,
    other: Vec<u32>,
}

pub(super) fn layout_names() -> CliResult<Vec<String>> {
    Ok(layout_specs()?
        .into_iter()
        .map(|layout| layout.name)
        .collect())
}

pub(super) fn write_package(
    path: &str,
    title: &str,
    subtitle: &str,
    size: &SlideSize,
    theme: &ThemeChoice,
) -> CliResult<()> {
    let layouts = layout_specs()?;
    let styles = master_text_styles()?;
    if let Some(parent) = Path::new(path)
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|err| CliError::unexpected(err.to_string()))?;
    }
    let output = File::create(path).map_err(|err| CliError::unexpected(err.to_string()))?;
    let mut writer = ZipWriter::new(output);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    let fixed_parts = [
        ("[Content_Types].xml", content_types_xml(layouts.len())),
        ("_rels/.rels", package_relationships_xml().to_string()),
        ("docProps/core.xml", core_props_xml()?),
        ("docProps/app.xml", app_props_xml(title, size)),
        (PRESENTATION_PART, presentation_xml(size)),
        (
            "ppt/_rels/presentation.xml.rels",
            presentation_relationships_xml().to_string(),
        ),
        (SLIDE_PART, slide_xml(title, subtitle)),
        (
            "ppt/slides/_rels/slide1.xml.rels",
            slide_relationships_xml().to_string(),
        ),
    ];
    for (name, body) in fixed_parts {
        write_zip_string(&mut writer, options, name, &body)?;
    }
    for (index, layout) in layouts.iter().enumerate() {
        let number = index + 1;
        write_zip_string(
            &mut writer,
            options,
            &format!("ppt/slideLayouts/slideLayout{number}.xml"),
            &slide_layout_xml(layout, size),
        )?;
        write_zip_string(
            &mut writer,
            options,
            &format!("ppt/slideLayouts/_rels/slideLayout{number}.xml.rels"),
            slide_layout_relationships_xml(),
        )?;
    }
    write_zip_string(
        &mut writer,
        options,
        SLIDE_MASTER_PART,
        &slide_master_xml(layouts.len(), size, &styles),
    )?;
    write_zip_string(
        &mut writer,
        options,
        "ppt/slideMasters/_rels/slideMaster1.xml.rels",
        &slide_master_relationships_xml(layouts.len()),
    )?;
    write_zip_string(&mut writer, options, THEME_PART, &theme_xml(theme))?;
    write_zip_string(&mut writer, options, TABLE_STYLES_PART, table_styles_xml())?;
    writer
        .finish()
        .map_err(|err| CliError::unexpected(err.to_string()))?;
    Ok(())
}

fn layout_specs() -> CliResult<Vec<LayoutSpec>> {
    let layouts: Vec<LayoutSpec> = serde_json::from_str(LAYOUT_DATA)
        .map_err(|err| CliError::unexpected(format!("invalid built-in PPTX layout data: {err}")))?;
    if layouts.len() != 11 {
        return Err(CliError::unexpected(format!(
            "built-in PPTX layout data has {} entries; expected 11",
            layouts.len()
        )));
    }
    Ok(layouts)
}

fn master_text_styles() -> CliResult<MasterTextStyles> {
    serde_json::from_str(MASTER_TEXT_STYLE_DATA).map_err(|err| {
        CliError::unexpected(format!(
            "invalid built-in PPTX master text style data: {err}"
        ))
    })
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

fn content_types_xml(layout_count: usize) -> String {
    let layouts = (1..=layout_count)
        .map(|number| format!(r#"<Override PartName="/ppt/slideLayouts/slideLayout{number}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/>"#))
        .collect::<String>();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/><Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/><Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/><Override PartName="/ppt/slides/slide1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>{layouts}<Override PartName="/ppt/slideMasters/slideMaster1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"/><Override PartName="/ppt/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/><Override PartName="/ppt/tableStyles.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.tableStyles+xml"/></Types>"#
    )
}

fn package_relationships_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/><Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties" Target="docProps/app.xml"/></Relationships>"#
}

fn core_props_xml() -> CliResult<String> {
    let dates = source_date_epoch_timestamp()?.map_or_else(String::new, |timestamp| {
        format!(
            r#"<dcterms:created xsi:type="dcterms:W3CDTF">{timestamp}</dcterms:created><dcterms:modified xsi:type="dcterms:W3CDTF">{timestamp}</dcterms:modified>"#
        )
    });
    Ok(format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:dcmitype="http://purl.org/dc/dcmitype/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"><dc:creator>ooxml-cli</dc:creator><cp:lastModifiedBy>ooxml-cli</cp:lastModifiedBy>{dates}</cp:coreProperties>"#
    ))
}

fn source_date_epoch_timestamp() -> CliResult<Option<String>> {
    let value = match std::env::var("SOURCE_DATE_EPOCH") {
        Ok(value) => value,
        Err(std::env::VarError::NotPresent) => return Ok(None),
        Err(err) => {
            return Err(CliError::invalid_args(format!(
                "SOURCE_DATE_EPOCH is not valid UTF-8: {err}"
            )));
        }
    };
    let seconds = value
        .parse::<u64>()
        .map_err(|_| CliError::invalid_args("SOURCE_DATE_EPOCH must be a non-negative integer"))?;
    if seconds > 253_402_300_799 {
        return Err(CliError::invalid_args(
            "SOURCE_DATE_EPOCH must represent a UTC instant no later than 9999-12-31T23:59:59Z",
        ));
    }
    Ok(Some(format_unix_timestamp(seconds)))
}

fn format_unix_timestamp(seconds: u64) -> String {
    let days = (seconds / 86_400) as i64;
    let day_seconds = seconds % 86_400;
    let hour = day_seconds / 3_600;
    let minute = day_seconds % 3_600 / 60;
    let second = day_seconds % 60;
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

fn civil_from_days(days_since_epoch: i64) -> (i64, u32, u32) {
    let z = days_since_epoch + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (year, month as u32, day as u32)
}

fn app_props_xml(title: &str, size: &SlideSize) -> String {
    let title = xml_escape(title);
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties" xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes"><Application>ooxml-cli</Application><PresentationFormat>{}</PresentationFormat><Slides>1</Slides><Notes>0</Notes><HiddenSlides>0</HiddenSlides><MMClips>0</MMClips><ScaleCrop>false</ScaleCrop><HeadingPairs><vt:vector size="2" baseType="variant"><vt:variant><vt:lpstr>Slides</vt:lpstr></vt:variant><vt:variant><vt:i4>1</vt:i4></vt:variant></vt:vector></HeadingPairs><TitlesOfParts><vt:vector size="1" baseType="lpstr"><vt:lpstr>{title}</vt:lpstr></vt:vector></TitlesOfParts></Properties>"#,
        size.presentation_format
    )
}

fn presentation_xml(size: &SlideSize) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:presentation xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" saveSubsetFonts="1" autoCompressPictures="0"><p:sldMasterIdLst><p:sldMasterId id="2147483648" r:id="rId1"/></p:sldMasterIdLst><p:sldIdLst><p:sldId id="256" r:id="rId4"/></p:sldIdLst><p:sldSz cx="{}" cy="{}" type="{}"/><p:notesSz cx="6858000" cy="9144000"/><p:defaultTextStyle><a:defPPr><a:defRPr lang="en-US"/></a:defPPr><a:lvl1pPr marL="0" algn="l" defTabSz="457200" rtl="0" eaLnBrk="1" latinLnBrk="0" hangingPunct="1"><a:defRPr sz="1800" kern="1200"><a:solidFill><a:schemeClr val="tx1"/></a:solidFill><a:latin typeface="+mn-lt"/><a:ea typeface="+mn-ea"/><a:cs typeface="+mn-cs"/></a:defRPr></a:lvl1pPr></p:defaultTextStyle></p:presentation>"#,
        size.width, size.height, size.preset
    )
}

fn presentation_relationships_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="slideMasters/slideMaster1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="theme/theme1.xml"/><Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/tableStyles" Target="tableStyles.xml"/><Relationship Id="rId4" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide1.xml"/></Relationships>"#
}

fn slide_xml(title: &str, subtitle: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><p:cSld><p:spTree>{}{}{}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>"#,
        group_shape_xml(),
        slide_placeholder_xml(2, "Title 1", "ctrTitle", None, title),
        slide_placeholder_xml(3, "Subtitle 2", "subTitle", Some(1), subtitle),
    )
}

fn slide_placeholder_xml(
    id: u32,
    name: &str,
    placeholder_type: &str,
    idx: Option<u32>,
    text: &str,
) -> String {
    let idx = idx
        .map(|idx| format!(r#" idx="{idx}""#))
        .unwrap_or_default();
    let paragraph = if text.is_empty() {
        "<a:p/>".to_string()
    } else {
        format!(
            r#"<a:p><a:r><a:rPr lang="en-US"/><a:t>{}</a:t></a:r><a:endParaRPr lang="en-US"/></a:p>"#,
            xml_escape(text)
        )
    };
    format!(
        r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="{}"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph type="{placeholder_type}"{idx}/></p:nvPr></p:nvSpPr><p:spPr/><p:txBody><a:bodyPr/><a:lstStyle/>{paragraph}</p:txBody></p:sp>"#,
        xml_escape(name)
    )
}

fn group_shape_xml() -> &'static str {
    r#"<p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>"#
}

fn slide_relationships_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/></Relationships>"#
}

fn slide_layout_xml(layout: &LayoutSpec, size: &SlideSize) -> String {
    let shapes = layout
        .placeholders
        .iter()
        .enumerate()
        .map(|(index, placeholder)| layout_placeholder_xml(index as u32 + 2, placeholder, size))
        .collect::<String>();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:sldLayout xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" type="{}" preserve="1"><p:cSld name="{}"><p:spTree>{}{shapes}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sldLayout>"#,
        layout.layout_type,
        xml_escape(&layout.name),
        group_shape_xml(),
    )
}

fn layout_placeholder_xml(id: u32, placeholder: &PlaceholderSpec, size: &SlideSize) -> String {
    let idx = placeholder
        .idx
        .map(|idx| format!(r#" idx="{idx}""#))
        .unwrap_or_default();
    let [x, y, cx, cy] = scaled_rect(placeholder.rect, size);
    let align = match placeholder.align.as_deref() {
        Some("center") => r#" algn="ctr""#,
        _ => "",
    };
    let vertical = if placeholder.vertical {
        r#" vert="vert""#
    } else {
        ""
    };
    format!(
        r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="{}"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph type="{}"{idx}/></p:nvPr></p:nvSpPr><p:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr><p:txBody><a:bodyPr{vertical} lIns="91440" tIns="45720" rIns="91440" bIns="45720"><a:normAutofit/></a:bodyPr><a:lstStyle/><a:p><a:pPr{align}/><a:endParaRPr lang="en-US"/></a:p></p:txBody></p:sp>"#,
        xml_escape(&placeholder.name),
        placeholder.placeholder_type,
    )
}

fn scaled_rect(rect: [i64; 4], size: &SlideSize) -> [i64; 4] {
    [
        rect[0] * size.width / 10_000,
        rect[1] * size.height / 10_000,
        rect[2] * size.width / 10_000,
        rect[3] * size.height / 10_000,
    ]
}

fn slide_layout_relationships_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="../slideMasters/slideMaster1.xml"/></Relationships>"#
}

fn slide_master_xml(layout_count: usize, size: &SlideSize, styles: &MasterTextStyles) -> String {
    let layout_ids = (1..=layout_count)
        .map(|number| {
            format!(
                r#"<p:sldLayoutId id="{}" r:id="rId{number}"/>"#,
                2_147_483_648_u64 + number as u64
            )
        })
        .collect::<String>();
    let master_title = PlaceholderSpec {
        name: "Master Title Placeholder".to_string(),
        placeholder_type: "title".to_string(),
        idx: None,
        rect: [400, 400, 9200, 1000],
        align: None,
        vertical: false,
    };
    let master_body = PlaceholderSpec {
        name: "Master Body Placeholder".to_string(),
        placeholder_type: "body".to_string(),
        idx: Some(1),
        rect: [400, 1750, 9200, 7850],
        align: None,
        vertical: false,
    };
    let title_shape = layout_placeholder_xml(2, &master_title, size);
    let body_shape = layout_placeholder_xml(3, &master_body, size);
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:sldMaster xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cSld><p:bg><p:bgPr><a:solidFill><a:schemeClr val="bg1"/></a:solidFill><a:effectLst/></p:bgPr></p:bg><p:spTree>{}{title_shape}{body_shape}</p:spTree></p:cSld><p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/><p:sldLayoutIdLst>{layout_ids}</p:sldLayoutIdLst>{}</p:sldMaster>"#,
        group_shape_xml(),
        master_text_styles_xml(styles),
    )
}

fn master_text_styles_xml(styles: &MasterTextStyles) -> String {
    let title = styles
        .title
        .first()
        .copied()
        .unwrap_or(40)
        .saturating_mul(100);
    format!(
        r#"<p:txStyles><p:titleStyle><a:lvl1pPr algn="ctr"><a:buNone/><a:defRPr sz="{title}" kern="1200">{}</a:defRPr></a:lvl1pPr></p:titleStyle><p:bodyStyle>{}</p:bodyStyle><p:otherStyle>{}</p:otherStyle></p:txStyles>"#,
        text_run_style_xml("+mj"),
        paragraph_levels_xml(&styles.body, true),
        paragraph_levels_xml(&styles.other, false),
    )
}

fn paragraph_levels_xml(sizes: &[u32], bullets: bool) -> String {
    sizes
        .iter()
        .take(5)
        .enumerate()
        .map(|(index, size)| {
            let level = index + 1;
            let margin = 342_900_i64 * (level as i64);
            let bullet = if bullets {
                r#"<a:buFont typeface="Arial"/><a:buChar char="•"/>"#
            } else {
                "<a:buNone/>"
            };
            format!(
                r#"<a:lvl{level}pPr marL="{margin}" indent="-285750" algn="l" defTabSz="457200">{bullet}<a:defRPr sz="{}" kern="1200">{}</a:defRPr></a:lvl{level}pPr>"#,
                size.saturating_mul(100),
                text_run_style_xml("+mn"),
            )
        })
        .collect()
}

fn text_run_style_xml(prefix: &str) -> String {
    format!(
        r#"<a:solidFill><a:schemeClr val="tx1"/></a:solidFill><a:latin typeface="{prefix}-lt"/><a:ea typeface="{prefix}-ea"/><a:cs typeface="{prefix}-cs"/>"#
    )
}

fn slide_master_relationships_xml(layout_count: usize) -> String {
    let layout_relationships = (1..=layout_count)
        .map(|number| {
            format!(
                r#"<Relationship Id="rId{number}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout{number}.xml"/>"#
            )
        })
        .collect::<String>();
    let theme_id = layout_count + 1;
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{layout_relationships}<Relationship Id="rId{theme_id}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="../theme/theme1.xml"/></Relationships>"#
    )
}

fn theme_xml(theme: &ThemeChoice) -> String {
    let color = |value: Srgb| value.to_hex();
    let palette = &theme.palette;
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="ooxml-cli {}"><a:themeElements><a:clrScheme name="ooxml-cli {}"><a:dk1><a:srgbClr val="{}"/></a:dk1><a:lt1><a:srgbClr val="{}"/></a:lt1><a:dk2><a:srgbClr val="{}"/></a:dk2><a:lt2><a:srgbClr val="{}"/></a:lt2><a:accent1><a:srgbClr val="{}"/></a:accent1><a:accent2><a:srgbClr val="{}"/></a:accent2><a:accent3><a:srgbClr val="{}"/></a:accent3><a:accent4><a:srgbClr val="{}"/></a:accent4><a:accent5><a:srgbClr val="{}"/></a:accent5><a:accent6><a:srgbClr val="{}"/></a:accent6><a:hlink><a:srgbClr val="{}"/></a:hlink><a:folHlink><a:srgbClr val="{}"/></a:folHlink></a:clrScheme><a:fontScheme name="ooxml-cli"><a:majorFont><a:latin typeface="Aptos Display"/><a:ea typeface=""/><a:cs typeface=""/></a:majorFont><a:minorFont><a:latin typeface="Aptos"/><a:ea typeface=""/><a:cs typeface=""/></a:minorFont></a:fontScheme>{}</a:themeElements><a:objectDefaults/><a:extraClrSchemeLst/></a:theme>"#,
        xml_escape(&theme.name),
        xml_escape(&theme.name),
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
        format_scheme_xml(),
    )
}

fn format_scheme_xml() -> &'static str {
    r#"<a:fmtScheme name="ooxml-cli"><a:fillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:gradFill rotWithShape="1"><a:gsLst><a:gs pos="0"><a:schemeClr val="phClr"><a:tint val="50000"/><a:satMod val="300000"/></a:schemeClr></a:gs><a:gs pos="100000"><a:schemeClr val="phClr"><a:shade val="50000"/><a:satMod val="300000"/></a:schemeClr></a:gs></a:gsLst><a:lin ang="16200000" scaled="1"/></a:gradFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:fillStyleLst><a:lnStyleLst><a:ln w="6350" cap="flat" cmpd="sng" algn="ctr"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:prstDash val="solid"/><a:miter lim="800000"/></a:ln><a:ln w="12700" cap="flat" cmpd="sng" algn="ctr"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:prstDash val="solid"/><a:miter lim="800000"/></a:ln><a:ln w="19050" cap="flat" cmpd="sng" algn="ctr"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:prstDash val="solid"/><a:miter lim="800000"/></a:ln></a:lnStyleLst><a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle></a:effectStyleLst><a:bgFillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"><a:tint val="95000"/></a:schemeClr></a:solidFill><a:solidFill><a:schemeClr val="phClr"><a:shade val="80000"/></a:schemeClr></a:solidFill></a:bgFillStyleLst></a:fmtScheme>"#
}

fn table_styles_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><a:tblStyleLst xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" def="{5C22544A-7EE6-4342-B048-85BDC9FD1C3A}"/>"#
}

#[cfg(test)]
mod tests {
    use super::{
        SlideSize, ThemeChoice, format_unix_timestamp, layout_specs, master_text_styles,
        scaled_rect, write_package,
    };

    #[test]
    fn reviewed_data_has_standard_layout_order_and_typographic_scale() {
        let layouts = layout_specs().expect("layout data");
        assert_eq!(layouts.len(), 11);
        assert_eq!(layouts[0].name, "Title Slide");
        assert_eq!(layouts[1].name, "Title and Content");
        assert_eq!(layouts[10].name, "Vertical Title and Text");
        let styles = master_text_styles().expect("master text style data");
        assert_eq!(styles.title, [40]);
        assert_eq!(styles.body, [28, 20, 18, 16, 14]);
        assert_eq!(styles.other, [14, 14, 14, 14, 14]);
    }

    #[test]
    fn size_and_theme_vocabulary_is_strict_and_deterministic() {
        let wide = SlideSize::parse(None).expect("default size");
        assert_eq!((wide.width, wide.height), (12_192_000, 6_858_000));
        assert_eq!(scaled_rect([400, 400, 9200, 1000], &wide)[0], 487_680);
        assert!(SlideSize::parse(Some("letter")).is_err());
        assert_eq!(
            ThemeChoice::resolve(Some("corporate"), None)
                .expect("corporate theme")
                .name,
            "corporate"
        );
        assert!(ThemeChoice::resolve(Some("warm"), Some("C55A11")).is_err());
    }

    #[test]
    fn package_bytes_and_source_date_epoch_format_are_deterministic() {
        let temp = std::env::temp_dir().join(format!(
            "ooxml-pptx-authoring-package-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&temp);
        std::fs::create_dir_all(&temp).expect("create package test directory");
        let first = temp.join("first.pptx");
        let second = temp.join("second.pptx");
        let size = SlideSize::parse(None).expect("default size");
        let theme = ThemeChoice::resolve(Some("dark"), None).expect("dark theme");
        write_package(
            first.to_str().expect("first path"),
            "Deterministic",
            "Bytes",
            &size,
            &theme,
        )
        .expect("write first package");
        write_package(
            second.to_str().expect("second path"),
            "Deterministic",
            "Bytes",
            &size,
            &theme,
        )
        .expect("write second package");
        assert_eq!(
            std::fs::read(&first).expect("read first package"),
            std::fs::read(&second).expect("read second package")
        );
        assert_eq!(format_unix_timestamp(0), "1970-01-01T00:00:00Z");
        assert_eq!(format_unix_timestamp(946_684_800), "2000-01-01T00:00:00Z");
        std::fs::remove_dir_all(temp).expect("remove package test directory");
    }
}
