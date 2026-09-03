use crate::{CliError, CliResult};

pub(super) const DEFAULT_THEME: &str = "corporate-blue";

pub(super) fn theme_seed(
    theme: Option<&str>,
    explicit_seed: Option<&str>,
) -> CliResult<(String, String)> {
    if theme.is_some() && explicit_seed.is_some() {
        return Err(CliError::invalid_args(
            "--theme and --theme-seed cannot be used together",
        ));
    }
    if let Some(seed) = explicit_seed {
        crate::palette::ThemePalette::derive(seed)
            .map_err(|err| CliError::invalid_args(err.to_string()))?;
        return Ok((
            "custom".to_string(),
            seed.trim_start_matches('#').to_ascii_uppercase(),
        ));
    }
    let theme = theme.unwrap_or(DEFAULT_THEME);
    let seed = match theme {
        "neutral" => "5B6573",
        "corporate-blue" => "4472C4",
        "warm" => "C55A11",
        "dark" => "4F46E5",
        _ => {
            return Err(CliError::invalid_args(format!(
                "unknown DOCX theme {theme:?}; expected neutral, corporate-blue, warm, or dark"
            )));
        }
    };
    Ok((theme.to_string(), seed.to_string()))
}

pub(super) fn theme_xml(theme_name: &str, seed: &str) -> CliResult<String> {
    let palette = crate::palette::ThemePalette::derive(seed)
        .map_err(|err| CliError::invalid_args(err.to_string()))?;
    let color = |value: crate::palette::Srgb| value.to_hex();
    Ok(format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="ooxml-cli {theme_name}"><a:themeElements><a:clrScheme name="ooxml-cli {theme_name}"><a:dk1><a:srgbClr val="{}"/></a:dk1><a:lt1><a:srgbClr val="{}"/></a:lt1><a:dk2><a:srgbClr val="{}"/></a:dk2><a:lt2><a:srgbClr val="{}"/></a:lt2><a:accent1><a:srgbClr val="{}"/></a:accent1><a:accent2><a:srgbClr val="{}"/></a:accent2><a:accent3><a:srgbClr val="{}"/></a:accent3><a:accent4><a:srgbClr val="{}"/></a:accent4><a:accent5><a:srgbClr val="{}"/></a:accent5><a:accent6><a:srgbClr val="{}"/></a:accent6><a:hlink><a:srgbClr val="{}"/></a:hlink><a:folHlink><a:srgbClr val="{}"/></a:folHlink></a:clrScheme><a:fontScheme name="ooxml-cli"><a:majorFont><a:latin typeface="Aptos Display"/><a:ea typeface=""/><a:cs typeface=""/></a:majorFont><a:minorFont><a:latin typeface="Aptos"/><a:ea typeface=""/><a:cs typeface=""/></a:minorFont></a:fontScheme><a:fmtScheme name="ooxml-cli"><a:fillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:gradFill rotWithShape="1"><a:gsLst><a:gs pos="0"><a:schemeClr val="phClr"><a:tint val="50000"/><a:satMod val="300000"/></a:schemeClr></a:gs><a:gs pos="100000"><a:schemeClr val="phClr"><a:shade val="50000"/><a:satMod val="300000"/></a:schemeClr></a:gs></a:gsLst><a:lin ang="16200000" scaled="1"/></a:gradFill><a:gradFill rotWithShape="1"><a:gsLst><a:gs pos="0"><a:schemeClr val="phClr"><a:shade val="51000"/><a:satMod val="130000"/></a:schemeClr></a:gs><a:gs pos="80000"><a:schemeClr val="phClr"><a:shade val="93000"/><a:satMod val="130000"/></a:schemeClr></a:gs><a:gs pos="100000"><a:schemeClr val="phClr"><a:shade val="94000"/><a:satMod val="135000"/></a:schemeClr></a:gs></a:gsLst><a:lin ang="16200000" scaled="0"/></a:gradFill></a:fillStyleLst><a:lnStyleLst><a:ln w="6350" cap="flat" cmpd="sng" algn="ctr"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:prstDash val="solid"/><a:miter lim="800000"/></a:ln><a:ln w="12700" cap="flat" cmpd="sng" algn="ctr"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:prstDash val="solid"/><a:miter lim="800000"/></a:ln><a:ln w="19050" cap="flat" cmpd="sng" algn="ctr"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:prstDash val="solid"/><a:miter lim="800000"/></a:ln></a:lnStyleLst><a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle></a:effectStyleLst><a:bgFillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"><a:tint val="95000"/><a:satMod val="170000"/></a:schemeClr></a:solidFill><a:gradFill rotWithShape="1"><a:gsLst><a:gs pos="0"><a:schemeClr val="phClr"><a:tint val="93000"/><a:satMod val="150000"/><a:shade val="98000"/></a:schemeClr></a:gs><a:gs pos="100000"><a:schemeClr val="phClr"><a:tint val="98000"/><a:satMod val="130000"/><a:shade val="90000"/></a:schemeClr></a:gs></a:gsLst><a:lin ang="16200000" scaled="0"/></a:gradFill></a:bgFillStyleLst></a:fmtScheme></a:themeElements></a:theme>"#,
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
    ))
}
