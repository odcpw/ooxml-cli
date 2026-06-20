use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Map, Value, json};
use std::fs;
use std::path::Path;

use crate::{
    CliError, CliResult, InspectPackageKind, attr, detect_inspect_package_type, local_name,
    package_type, pptx_charts_list, pptx_masters_list, pptx_masters_show, reject_unknown_flags,
    xlsx_charts_list, zip_entry_names, zip_text,
};

pub(crate) fn template_tokens(file: &str, args: &[String]) -> CliResult<Value> {
    reject_unknown_flags(args, &["--for"], &[])?;
    let kind = resolve_template_kind(
        file,
        parse_string_flag_local(args, "--for")?.as_deref(),
        "template tokens supports PPTX/POTX and XLSX/XLTX files",
    )?;
    template_tokens_for_kind(file, kind)
}

pub(crate) fn template_profile_save(file: &str, args: &[String]) -> CliResult<Value> {
    reject_unknown_flags(args, &["--description", "--for", "--name", "--out"], &[])?;
    let kind = resolve_template_kind(
        file,
        parse_string_flag_local(args, "--for")?.as_deref(),
        "profile save reads PPTX/POTX and XLSX/XLTX files",
    )?;
    let name = parse_string_flag_local(args, "--name")?;
    let description = parse_string_flag_local(args, "--description")?;
    let out = parse_string_flag_local(args, "--out")?;
    let tokens = template_tokens_for_kind(file, kind)?;
    let profile = profile_from_tokens(&tokens, name.as_deref(), description.as_deref());
    validate_profile(&profile)?;
    if let Some(out) = out.as_deref().filter(|value| !value.trim().is_empty()) {
        let mut data = serde_json::to_vec_pretty(&profile).map_err(|err| {
            CliError::unexpected(format!("failed to marshal profile JSON: {err}"))
        })?;
        data.push(b'\n');
        fs::write(out, data)
            .map_err(|err| CliError::unexpected(format!("failed to write profile: {err}")))?;
    }
    Ok(profile)
}

pub(crate) fn template_profile_inspect(profile_path: &str) -> CliResult<Value> {
    let text = fs::read_to_string(profile_path).map_err(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            CliError::file_not_found(format!("file not found: {profile_path}"))
        } else {
            CliError::unexpected(format!("failed to read profile: {err}"))
        }
    })?;
    let profile: Value = serde_json::from_str(&text)
        .map_err(|err| CliError::unexpected(format!("failed to parse profile: {err}")))?;
    validate_profile(&profile)?;
    Ok(profile)
}

fn template_tokens_for_kind(file: &str, kind: &str) -> CliResult<Value> {
    match kind {
        "pptx" => pptx_template_tokens(file),
        "xlsx" => xlsx_template_tokens(file),
        other => Err(CliError::unsupported_type(format!(
            "template tokens supports PPTX/POTX and XLSX/XLTX files (detected: {other}); pass --for to override"
        ))),
    }
}

fn resolve_template_kind(
    file: &str,
    requested: Option<&str>,
    unsupported_prefix: &str,
) -> CliResult<&'static str> {
    if let Some(requested) = requested.map(str::trim).filter(|value| !value.is_empty()) {
        match requested.to_ascii_lowercase().as_str() {
            "pptx" | "potx" => return Ok("pptx"),
            "xlsx" | "xltx" => return Ok("xlsx"),
            "auto" => {}
            other => {
                return Err(CliError::invalid_args(format!(
                    "invalid --for {other:?}; expected pptx, xlsx, or auto"
                )));
            }
        }
    }
    match package_type(file)? {
        "pptx" => Ok("pptx"),
        "xlsx" => Ok("xlsx"),
        "docx" => Err(CliError::unsupported_type(format!(
            "{unsupported_prefix} (detected: docx); pass --for to override"
        ))),
        _ => {
            let entries = zip_entry_names(file)?;
            match detect_inspect_package_type(file, &entries) {
                InspectPackageKind::Pptx => Ok("pptx"),
                InspectPackageKind::Xlsx => Ok("xlsx"),
                InspectPackageKind::Docx => Err(CliError::unsupported_type(format!(
                    "{unsupported_prefix} (detected: docx); pass --for to override"
                ))),
                InspectPackageKind::Unknown => Err(CliError::unsupported_type(format!(
                    "{unsupported_prefix} (detected: unknown); pass --for to override"
                ))),
            }
        }
    }
}

fn pptx_template_tokens(file: &str) -> CliResult<Value> {
    let mut theme = Value::Null;
    let mut default_text_styles = Vec::<Value>::new();
    if let Some(masters) = pptx_masters_list(file)?
        .get("masters")
        .and_then(Value::as_array)
    {
        for master in masters {
            let index = master
                .get("index")
                .and_then(Value::as_i64)
                .unwrap_or(default_text_styles.len() as i64 + 1);
            let shown = pptx_masters_show(file, index)?;
            if theme.is_null()
                && let Some(master_theme) = shown.get("theme")
            {
                theme = master_theme.clone();
            }
            if let Some(master_ref) = shown.get("uri").and_then(Value::as_str) {
                default_text_styles.extend(pptx_master_default_text_styles(file, master_ref)?);
            }
        }
    }
    let mut pptx = Map::new();
    if !theme.is_null() {
        pptx.insert("theme".to_string(), theme);
    }
    pptx.insert(
        "defaultTextStyles".to_string(),
        Value::Array(default_text_styles),
    );
    pptx.insert("tableStyles".to_string(), Value::Array(Vec::new()));
    pptx.insert(
        "chartStyles".to_string(),
        Value::Array(pptx_chart_style_summaries(file)?),
    );
    Ok(json!({
        "schemaVersion": "1.0",
        "type": "pptx",
        "source": file_name(file),
        "pptx": Value::Object(pptx),
    }))
}

fn xlsx_template_tokens(file: &str) -> CliResult<Value> {
    let mut xlsx = Map::new();
    if let Some(theme) = xlsx_theme(file)? {
        xlsx.insert("theme".to_string(), theme);
    }
    xlsx.insert("namedCellStyles".to_string(), Value::Array(Vec::new()));
    xlsx.insert(
        "chartStyles".to_string(),
        Value::Array(xlsx_chart_style_summaries(file)?),
    );
    Ok(json!({
        "schemaVersion": "1.0",
        "type": "xlsx",
        "source": file_name(file),
        "xlsx": Value::Object(xlsx),
    }))
}

fn profile_from_tokens(tokens: &Value, name: Option<&str>, description: Option<&str>) -> Value {
    let source_type = tokens
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let mut metadata = Map::new();
    if let Some(name) = name.filter(|value| !value.trim().is_empty()) {
        metadata.insert("name".to_string(), json!(name));
    }
    if let Some(description) = description.filter(|value| !value.trim().is_empty()) {
        metadata.insert("description".to_string(), json!(description));
    }
    if let Some(source) = tokens.get("source").and_then(Value::as_str) {
        metadata.insert("sourceFile".to_string(), json!(source));
    }
    if !source_type.is_empty() {
        metadata.insert("sourceType".to_string(), json!(source_type));
    }

    let mut design = Map::new();
    match source_type {
        "pptx" => {
            if let Some(pptx) = tokens.get("pptx") {
                if let Some(theme) = pptx.get("theme") {
                    design.insert("theme".to_string(), theme.clone());
                }
                design.insert(
                    "placeholders".to_string(),
                    pptx.get("defaultTextStyles")
                        .cloned()
                        .unwrap_or_else(|| Value::Array(Vec::new())),
                );
            }
        }
        "xlsx" => {
            if let Some(theme) = tokens.get("xlsx").and_then(|xlsx| xlsx.get("theme")) {
                design.insert("theme".to_string(), theme.clone());
            }
        }
        _ => {}
    }
    json!({
        "schemaVersion": "1.0",
        "format": "ooxml-design-profile",
        "metadata": Value::Object(metadata),
        "design": Value::Object(design),
    })
}

fn validate_profile(profile: &Value) -> CliResult<()> {
    if !profile.is_object() {
        return Err(CliError::unexpected(
            "profile validation failed: profile is nil",
        ));
    }
    if profile
        .get("format")
        .and_then(Value::as_str)
        .unwrap_or_default()
        != "ooxml-design-profile"
    {
        return Err(CliError::unexpected(
            "profile validation failed: unsupported profile format",
        ));
    }
    if profile
        .get("schemaVersion")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .is_empty()
    {
        return Err(CliError::unexpected(
            "profile validation failed: schemaVersion is required",
        ));
    }
    if let Some(color_scheme) = profile
        .get("design")
        .and_then(|design| design.get("theme"))
        .and_then(|theme| theme.get("colorScheme"))
        .and_then(Value::as_object)
    {
        for (key, value) in color_scheme {
            if key == "name" {
                continue;
            }
            let Some(color) = value.as_str() else {
                continue;
            };
            if !is_hex_color(color) {
                return Err(CliError::unexpected(format!(
                    "profile validation failed: invalid color {key}={color:?}"
                )));
            }
        }
    }
    Ok(())
}

fn pptx_master_default_text_styles(file: &str, master_ref: &str) -> CliResult<Vec<Value>> {
    let xml = zip_text(file, master_ref.trim_start_matches('/'))?;
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(true);
    let mut in_tx_styles = false;
    let mut role = String::new();
    let mut in_first_level = false;
    let mut current = Map::<String, Value>::new();
    let mut out = Vec::<Value>::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                match name.as_str() {
                    "txStyles" => in_tx_styles = true,
                    "titleStyle" if in_tx_styles => role = "title".to_string(),
                    "bodyStyle" if in_tx_styles => role = "body".to_string(),
                    "otherStyle" if in_tx_styles => role = "other".to_string(),
                    "lvl1pPr" if in_tx_styles && !role.is_empty() => {
                        in_first_level = true;
                        current.clear();
                        current.insert("masterRef".to_string(), json!(master_ref));
                        current.insert("role".to_string(), json!(role));
                    }
                    "defRPr" if in_first_level => {
                        if let Some(size) =
                            attr(&e, "sz").and_then(|value| value.parse::<i64>().ok())
                        {
                            current.insert("sizePt".to_string(), json!(size / 100));
                        }
                    }
                    "schemeClr" if in_first_level => {
                        if let Some(color_ref) = attr(&e, "val") {
                            current.insert("colorRef".to_string(), json!(color_ref));
                        }
                    }
                    "srgbClr" if in_first_level => {
                        if let Some(color) = attr(&e, "val") {
                            current.insert("color".to_string(), json!(color));
                        }
                    }
                    "latin" if in_first_level => {
                        if let Some(typeface) = attr(&e, "typeface") {
                            insert_font_reference(&mut current, &typeface);
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                match name.as_str() {
                    "defRPr" if in_first_level => {
                        if let Some(size) =
                            attr(&e, "sz").and_then(|value| value.parse::<i64>().ok())
                        {
                            current.insert("sizePt".to_string(), json!(size / 100));
                        }
                    }
                    "schemeClr" if in_first_level => {
                        if let Some(color_ref) = attr(&e, "val") {
                            current.insert("colorRef".to_string(), json!(color_ref));
                        }
                    }
                    "srgbClr" if in_first_level => {
                        if let Some(color) = attr(&e, "val") {
                            current.insert("color".to_string(), json!(color));
                        }
                    }
                    "latin" if in_first_level => {
                        if let Some(typeface) = attr(&e, "typeface") {
                            insert_font_reference(&mut current, &typeface);
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(e)) => match local_name(e.name().as_ref()) {
                "txStyles" => in_tx_styles = false,
                "titleStyle" | "bodyStyle" | "otherStyle" => role.clear(),
                "lvl1pPr" if in_first_level => {
                    in_first_level = false;
                    out.push(Value::Object(current.clone()));
                    current.clear();
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(out)
}

fn insert_font_reference(style: &mut Map<String, Value>, typeface: &str) {
    match typeface {
        "+mj-lt" | "+mj-ea" | "+mj-cs" => {
            style.insert("fontRef".to_string(), json!("major"));
        }
        "+mn-lt" | "+mn-ea" | "+mn-cs" => {
            style.insert("fontRef".to_string(), json!("minor"));
        }
        other if !other.is_empty() => {
            style.insert("fontName".to_string(), json!(other));
        }
        _ => {}
    }
}

fn pptx_chart_style_summaries(file: &str) -> CliResult<Vec<Value>> {
    let charts = pptx_charts_list(file, 0)?;
    Ok(chart_style_summaries(&charts))
}

fn xlsx_chart_style_summaries(file: &str) -> CliResult<Vec<Value>> {
    let charts = xlsx_charts_list(file, None)?;
    Ok(chart_style_summaries(&charts))
}

fn chart_style_summaries(charts: &Value) -> Vec<Value> {
    charts
        .get("charts")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|chart| {
            let part_uri = chart.get("partUri").and_then(Value::as_str)?;
            let mut out = Map::new();
            out.insert("partUri".to_string(), json!(part_uri));
            if let Some(chart_type) = chart
                .get("types")
                .and_then(Value::as_array)
                .and_then(|types| types.first())
                .and_then(Value::as_str)
            {
                out.insert("chartType".to_string(), json!(chart_type));
            }
            Some(Value::Object(out))
        })
        .collect()
}

fn xlsx_theme(file: &str) -> CliResult<Option<Value>> {
    let entries = zip_entry_names(file)?;
    let theme_part = entries
        .iter()
        .find(|entry| entry.starts_with("xl/theme/") && entry.ends_with(".xml"))
        .cloned();
    let Some(theme_part) = theme_part else {
        return Ok(None);
    };
    Ok(parse_theme_xml(&zip_text(file, &theme_part)?))
}

fn parse_theme_xml(xml: &str) -> Option<Value> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut theme_name = String::new();
    let mut color_scheme = Map::new();
    let mut font_scheme = Map::new();
    let mut in_theme_elements = false;
    let mut in_color_scheme = false;
    let mut in_font_scheme = false;
    let mut current_color = String::new();
    let mut current_font = String::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                match name.as_str() {
                    "theme" => theme_name = attr(&e, "name").unwrap_or_default(),
                    "themeElements" => in_theme_elements = true,
                    "clrScheme" if in_theme_elements => {
                        in_color_scheme = true;
                        if let Some(value) = attr(&e, "name") {
                            color_scheme.insert("name".to_string(), json!(value));
                        }
                    }
                    "fontScheme" if in_theme_elements => {
                        in_font_scheme = true;
                        if let Some(value) = attr(&e, "name") {
                            font_scheme.insert("name".to_string(), json!(value));
                        }
                    }
                    "dk1" | "lt1" | "dk2" | "lt2" | "accent1" | "accent2" | "accent3"
                    | "accent4" | "accent5" | "accent6" | "hlink" | "folHlink"
                        if in_color_scheme =>
                    {
                        current_color = name;
                    }
                    "majorFont" | "minorFont" if in_font_scheme => current_font = name,
                    _ => {}
                }
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if in_color_scheme && !current_color.is_empty() {
                    if name == "srgbClr" {
                        if let Some(value) = attr(&e, "val") {
                            insert_theme_color(&mut color_scheme, &current_color, value);
                        }
                    } else if name == "sysClr"
                        && let Some(value) = attr(&e, "lastClr")
                    {
                        insert_theme_color(&mut color_scheme, &current_color, value);
                    }
                }
                if in_font_scheme && !current_font.is_empty() {
                    match (current_font.as_str(), name.as_str()) {
                        ("majorFont", "latin") => {
                            if let Some(value) = attr(&e, "typeface") {
                                font_scheme.insert("majorFont".to_string(), json!(value));
                            }
                        }
                        ("minorFont", "latin") => {
                            if let Some(value) = attr(&e, "typeface") {
                                font_scheme.insert("minorFont".to_string(), json!(value));
                            }
                        }
                        _ => {}
                    }
                }
            }
            Ok(Event::End(e)) => match local_name(e.name().as_ref()) {
                "themeElements" => in_theme_elements = false,
                "clrScheme" => in_color_scheme = false,
                "fontScheme" => in_font_scheme = false,
                "dk1" | "lt1" | "dk2" | "lt2" | "accent1" | "accent2" | "accent3" | "accent4"
                | "accent5" | "accent6" | "hlink" | "folHlink" => current_color.clear(),
                "majorFont" | "minorFont" => current_font.clear(),
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(_) => return None,
            _ => {}
        }
    }
    let mut theme = Map::new();
    if !theme_name.is_empty() {
        theme.insert("name".to_string(), json!(theme_name));
    }
    if !color_scheme.is_empty() {
        theme.insert("colorScheme".to_string(), Value::Object(color_scheme));
    }
    if !font_scheme.is_empty() {
        theme.insert("fontScheme".to_string(), Value::Object(font_scheme));
    }
    Some(Value::Object(theme))
}

fn insert_theme_color(color_scheme: &mut Map<String, Value>, key: &str, value: String) {
    let json_key = match key {
        "dk1" => "dark1",
        "lt1" => "light1",
        "dk2" => "dark2",
        "lt2" => "light2",
        "hlink" => "hypLink",
        "folHlink" => "folLink",
        other => other,
    };
    color_scheme.insert(json_key.to_string(), json!(value));
}

fn parse_string_flag_local(args: &[String], flag: &str) -> CliResult<Option<String>> {
    let mut out = None;
    let mut i = 0;
    while i < args.len() {
        if args[i] == flag {
            let Some(value) = args.get(i + 1) else {
                return Err(CliError::invalid_args(format!("{flag} requires a value")));
            };
            out = Some(value.clone());
            i += 2;
        } else {
            i += 1;
        }
    }
    Ok(out)
}

fn file_name(file: &str) -> String {
    Path::new(file)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(file)
        .to_string()
}

fn is_hex_color(value: &str) -> bool {
    value.len() == 6 && value.chars().all(|ch| ch.is_ascii_hexdigit())
}
