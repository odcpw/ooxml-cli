use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::{
    CliError, CliResult, InspectPackageKind, attr, copy_zip_with_part_overrides,
    detect_inspect_package_type, has_flag, local_name, package_mutation_temp_path, package_type,
    parse_string_flag, pptx_charts_list, pptx_masters_list, pptx_masters_show,
    reject_unknown_flags, validate, validate_xlsx_mutation_output_flags, xlsx_charts_list,
    xml_attr_escape, xml_direct_child_ranges, xml_fragment_bounds, xml_token_name, zip_entry_names,
    zip_text,
};

const TEMPLATE_COLOR_ORDER: &[(&str, &str)] = &[
    ("dk1", "dark1"),
    ("lt1", "light1"),
    ("dk2", "dark2"),
    ("lt2", "light2"),
    ("accent1", "accent1"),
    ("accent2", "accent2"),
    ("accent3", "accent3"),
    ("accent4", "accent4"),
    ("accent5", "accent5"),
    ("accent6", "accent6"),
    ("hlink", "hypLink"),
    ("folHlink", "folLink"),
];

const DECORATIVE_TEMPLATE_KEYS: &[&str] = &[
    "gradients",
    "animations",
    "3dFormats",
    "conditionalFormats",
    "transitions",
];

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

pub(crate) fn template_apply(file: &str, args: &[String]) -> CliResult<Value> {
    reject_unknown_flags(
        args,
        &[
            "--for",
            "--from",
            "--tokens",
            "--profile",
            "--out",
            "--backup",
        ],
        &[
            "--target-colors",
            "--target-fonts",
            "--target-charts",
            "--target-text-styles",
            "--target-ranges",
            "--dry-run",
            "--in-place",
            "--no-validate",
        ],
    )?;
    let target_kind = resolve_template_kind(
        file,
        parse_string_flag(args, "--for")?.as_deref(),
        "template apply supports PPTX/POTX and XLSX/XLTX files",
    )?;
    let from = parse_string_flag(args, "--from")?;
    let tokens_path = parse_string_flag(args, "--tokens")?;
    let profile_path = parse_string_flag(args, "--profile")?;
    let out = parse_string_flag(args, "--out")?;
    let backup = parse_string_flag(args, "--backup")?;
    let dry_run = has_flag(args, "--dry-run");
    let in_place = has_flag(args, "--in-place");
    let no_validate = has_flag(args, "--no-validate");
    validate_xlsx_mutation_output_flags(out.as_deref(), in_place, backup.as_deref(), dry_run)?;

    let source_count = [&from, &tokens_path, &profile_path]
        .iter()
        .filter(|value| value.as_deref().is_some_and(|path| !path.trim().is_empty()))
        .count();
    if source_count != 1 {
        return Err(CliError::invalid_args(
            "must specify exactly one of --from, --tokens, or --profile",
        ));
    }

    let target_colors = has_flag(args, "--target-colors");
    let target_fonts = has_flag(args, "--target-fonts");
    let target_charts = has_flag(args, "--target-charts");
    let target_text_styles = has_flag(args, "--target-text-styles");
    let target_ranges = has_flag(args, "--target-ranges");
    let explicit_targets =
        target_colors || target_fonts || target_charts || target_text_styles || target_ranges;
    let want_colors = if explicit_targets {
        target_colors
    } else {
        true
    };
    let want_fonts = if explicit_targets { target_fonts } else { true };
    if target_charts {
        return Err(CliError::invalid_args(
            "template apply --target-charts is not yet implemented in the Rust port",
        ));
    }
    if target_text_styles {
        return Err(CliError::invalid_args(
            "template apply --target-text-styles is not yet implemented in the Rust port",
        ));
    }

    let (tokens, profile_source, profile_name) = load_apply_tokens(
        target_kind,
        from.as_deref(),
        tokens_path.as_deref(),
        profile_path.as_deref(),
    )?;
    let colors = if want_colors {
        theme_color_updates_from_tokens(&tokens, target_kind)?
    } else {
        Vec::new()
    };
    let fonts = if want_fonts {
        theme_font_updates_from_tokens(&tokens, target_kind)
    } else {
        ThemeFontUpdates::default()
    };

    let mut applied_colors = Vec::<Value>::new();
    let mut applied_font_parts = Vec::<Value>::new();
    let mut skipped = Vec::<String>::new();
    let mut overrides = BTreeMap::<String, String>::new();
    if want_colors || want_fonts {
        for part in template_theme_parts(file, target_kind)? {
            let part_uri = format!("/{}", part.trim_start_matches('/'));
            let xml = zip_text(file, &part)?;
            let current_theme = parse_theme_xml(&xml).unwrap_or_else(|| json!({}));
            let part_result =
                apply_theme_updates_to_part(&xml, &part_uri, &current_theme, &colors, &fonts)?;
            applied_colors.extend(part_result.applied_colors);
            if let Some(font_part) = part_result.applied_font_part {
                applied_font_parts.push(font_part);
            }
            skipped.extend(part_result.skipped);
            if part_result.updated_xml != xml {
                overrides.insert(part, part_result.updated_xml);
            }
        }
    }

    if target_ranges {
        skipped.push(
            "ranges: range/cell style transfer is not supported (no per-range style in the token model)"
                .to_string(),
        );
    }

    let total_updates = applied_colors.len() + applied_font_parts.len();
    let ranges_only = explicit_targets
        && target_ranges
        && !target_colors
        && !target_fonts
        && !target_charts
        && !target_text_styles;
    let output_path = if !dry_run && !ranges_only {
        let output = write_template_apply_output(
            file,
            out.as_deref(),
            backup.as_deref(),
            in_place,
            no_validate,
            &overrides,
        )?;
        Some(output)
    } else {
        None
    };

    let mut applied = Map::new();
    applied.insert("colors".to_string(), Value::Array(applied_colors));
    if !applied_font_parts.is_empty() {
        let mut fonts_json = Map::new();
        if let Some(major_font) = fonts.major_font.as_deref() {
            fonts_json.insert("majorFont".to_string(), json!(major_font));
        }
        if let Some(minor_font) = fonts.minor_font.as_deref() {
            fonts_json.insert("minorFont".to_string(), json!(minor_font));
        }
        applied.insert("fonts".to_string(), Value::Object(fonts_json));
        applied.insert("fontParts".to_string(), Value::Array(applied_font_parts));
    }
    applied.insert("charts".to_string(), Value::Array(Vec::new()));
    applied.insert("textStyles".to_string(), Value::Array(Vec::new()));

    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    if let Some(output_path) = output_path {
        result.insert("output".to_string(), json!(output_path));
    }
    result.insert("dryRun".to_string(), json!(dry_run));
    result.insert("changed".to_string(), json!(total_updates > 0));
    result.insert("targetType".to_string(), json!(target_kind));
    result.insert("profileSource".to_string(), json!(profile_source));
    if let Some(profile_name) = profile_name.filter(|value| !value.trim().is_empty()) {
        result.insert("profileName".to_string(), json!(profile_name));
    }
    result.insert(
        "schemaVersion".to_string(),
        tokens
            .get("schemaVersion")
            .cloned()
            .unwrap_or_else(|| json!("1.0")),
    );
    result.insert("applied".to_string(), Value::Object(applied));
    result.insert("skipped".to_string(), json!(skipped));
    result.insert("totalUpdates".to_string(), json!(total_updates));
    Ok(Value::Object(result))
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

#[derive(Default)]
struct ThemeFontUpdates {
    major_font: Option<String>,
    minor_font: Option<String>,
}

struct ThemeColorUpdate {
    ooxml_name: &'static str,
    json_name: &'static str,
    hex: String,
}

struct ThemePartApplyResult {
    updated_xml: String,
    applied_colors: Vec<Value>,
    applied_font_part: Option<Value>,
    skipped: Vec<String>,
}

fn load_apply_tokens(
    target_kind: &str,
    from: Option<&str>,
    tokens_path: Option<&str>,
    profile_path: Option<&str>,
) -> CliResult<(Value, String, Option<String>)> {
    if let Some(from) = from.filter(|value| !value.trim().is_empty()) {
        let source_kind = resolve_template_kind(
            from,
            None,
            "template apply --from supports PPTX/POTX and XLSX/XLTX files",
        )?;
        let tokens = template_tokens_for_kind(from, source_kind)?;
        return Ok((tokens, from.to_string(), None));
    }
    if let Some(tokens_path) = tokens_path.filter(|value| !value.trim().is_empty()) {
        let tokens = read_json_file(tokens_path, "tokens")?;
        reject_decorative_apply_tokens(&tokens)?;
        let profile_name = tokens
            .get("source")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        return Ok((tokens, tokens_path.to_string(), profile_name));
    }
    if let Some(profile_path) = profile_path.filter(|value| !value.trim().is_empty()) {
        let profile = template_profile_inspect(profile_path)?;
        let profile_name = profile
            .get("metadata")
            .and_then(|metadata| metadata.get("name"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        let tokens = tokens_from_profile(&profile, target_kind);
        return Ok((tokens, profile_path.to_string(), profile_name));
    }
    Err(CliError::invalid_args(
        "must specify exactly one of --from, --tokens, or --profile",
    ))
}

fn read_json_file(path: &str, label: &str) -> CliResult<Value> {
    let text = fs::read_to_string(path).map_err(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            CliError::file_not_found(format!("file not found: {path}"))
        } else {
            CliError::unexpected(format!("failed to read {label}: {err}"))
        }
    })?;
    serde_json::from_str(&text)
        .map_err(|err| CliError::unexpected(format!("failed to parse {label}: {err}")))
}

fn reject_decorative_apply_tokens(tokens: &Value) -> CliResult<()> {
    for key in DECORATIVE_TEMPLATE_KEYS {
        if tokens.get(*key).is_some() {
            return Err(CliError::invalid_args(format!(
                "template apply cannot apply decorative token key {key:?}"
            )));
        }
    }
    Ok(())
}

fn tokens_from_profile(profile: &Value, target_kind: &str) -> Value {
    let mut target = Map::new();
    if let Some(theme) = profile.get("design").and_then(|design| design.get("theme")) {
        target.insert("theme".to_string(), theme.clone());
    }
    if target_kind == "pptx" {
        target.insert(
            "defaultTextStyles".to_string(),
            profile
                .get("design")
                .and_then(|design| design.get("placeholders"))
                .cloned()
                .unwrap_or_else(|| Value::Array(Vec::new())),
        );
        target.insert("tableStyles".to_string(), Value::Array(Vec::new()));
        target.insert("chartStyles".to_string(), Value::Array(Vec::new()));
    } else {
        target.insert("namedCellStyles".to_string(), Value::Array(Vec::new()));
        target.insert("chartStyles".to_string(), Value::Array(Vec::new()));
    }
    let source = profile
        .get("metadata")
        .and_then(|metadata| metadata.get("sourceFile"))
        .and_then(Value::as_str)
        .unwrap_or("profile");
    json!({
        "schemaVersion": profile
            .get("schemaVersion")
            .cloned()
            .unwrap_or_else(|| json!("1.0")),
        "type": target_kind,
        "source": source,
        target_kind: Value::Object(target),
    })
}

fn theme_color_updates_from_tokens(
    tokens: &Value,
    target_kind: &str,
) -> CliResult<Vec<ThemeColorUpdate>> {
    let Some(color_scheme) = tokens
        .get(target_kind)
        .and_then(|target| target.get("theme"))
        .and_then(|theme| theme.get("colorScheme"))
        .and_then(Value::as_object)
    else {
        return Ok(Vec::new());
    };
    let mut updates = Vec::new();
    for (ooxml_name, json_name) in TEMPLATE_COLOR_ORDER {
        let Some(hex) = color_scheme.get(*json_name).and_then(Value::as_str) else {
            continue;
        };
        if !is_hex_color(hex) {
            return Err(CliError::invalid_args(format!(
                "invalid theme color {json_name}={hex:?}"
            )));
        }
        updates.push(ThemeColorUpdate {
            ooxml_name,
            json_name,
            hex: hex.to_ascii_uppercase(),
        });
    }
    Ok(updates)
}

fn theme_font_updates_from_tokens(tokens: &Value, target_kind: &str) -> ThemeFontUpdates {
    let font_scheme = tokens
        .get(target_kind)
        .and_then(|target| target.get("theme"))
        .and_then(|theme| theme.get("fontScheme"));
    ThemeFontUpdates {
        major_font: font_scheme
            .and_then(|fonts| fonts.get("majorFont"))
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(ToOwned::to_owned),
        minor_font: font_scheme
            .and_then(|fonts| fonts.get("minorFont"))
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(ToOwned::to_owned),
    }
}

fn template_theme_parts(file: &str, target_kind: &str) -> CliResult<Vec<String>> {
    let prefix = match target_kind {
        "pptx" => "ppt/theme/",
        "xlsx" => "xl/theme/",
        other => {
            return Err(CliError::unsupported_type(format!(
                "template apply supports PPTX/POTX and XLSX/XLTX files (detected: {other})"
            )));
        }
    };
    let mut parts = zip_entry_names(file)?
        .into_iter()
        .filter(|entry| entry.starts_with(prefix) && entry.ends_with(".xml"))
        .collect::<Vec<_>>();
    parts.sort();
    if parts.is_empty() {
        return Err(CliError::unexpected(format!(
            "no theme part found for {target_kind} package"
        )));
    }
    Ok(parts)
}

fn apply_theme_updates_to_part(
    xml: &str,
    part_uri: &str,
    current_theme: &Value,
    colors: &[ThemeColorUpdate],
    fonts: &ThemeFontUpdates,
) -> CliResult<ThemePartApplyResult> {
    let mut updated = xml.to_string();
    let mut applied_colors = Vec::new();
    let mut skipped = Vec::new();
    let current_colors = current_theme.get("colorScheme").and_then(Value::as_object);
    for color in colors {
        let current = current_colors
            .and_then(|scheme| scheme.get(color.json_name))
            .and_then(Value::as_str);
        if current.is_some_and(|value| value.eq_ignore_ascii_case(&color.hex)) {
            skipped.push(format!(
                "color {} in {part_uri}: already set to #{}",
                color.ooxml_name, color.hex
            ));
            continue;
        }
        updated = update_theme_color(&updated, color.ooxml_name, &color.hex).map_err(|err| {
            CliError::invalid_args(format!(
                "failed to update theme color {} in {part_uri}: {err}",
                color.ooxml_name
            ))
        })?;
        applied_colors.push(json!({
            "partUri": part_uri,
            "colorName": color.ooxml_name,
            "hexValue": color.hex,
        }));
    }

    let current_fonts = current_theme.get("fontScheme").and_then(Value::as_object);
    let major_current = current_fonts
        .and_then(|scheme| scheme.get("majorFont"))
        .and_then(Value::as_str);
    let minor_current = current_fonts
        .and_then(|scheme| scheme.get("minorFont"))
        .and_then(Value::as_str);
    let major_changed = fonts
        .major_font
        .as_deref()
        .is_some_and(|font| major_current != Some(font));
    let minor_changed = fonts
        .minor_font
        .as_deref()
        .is_some_and(|font| minor_current != Some(font));
    let font_part = if major_changed || minor_changed {
        updated = update_theme_font(
            &updated,
            fonts.major_font.as_deref(),
            fonts.minor_font.as_deref(),
        )
        .map_err(|err| {
            CliError::invalid_args(format!("failed to update theme fonts in {part_uri}: {err}"))
        })?;
        let mut font_json = Map::new();
        font_json.insert("partUri".to_string(), json!(part_uri));
        if let Some(major_font) = fonts.major_font.as_deref() {
            font_json.insert("majorFont".to_string(), json!(major_font));
        }
        if let Some(minor_font) = fonts.minor_font.as_deref() {
            font_json.insert("minorFont".to_string(), json!(minor_font));
        }
        Some(Value::Object(font_json))
    } else if fonts.major_font.is_some() || fonts.minor_font.is_some() {
        skipped.push(format!("{part_uri}: fonts already up to date"));
        None
    } else {
        None
    };

    Ok(ThemePartApplyResult {
        updated_xml: updated,
        applied_colors,
        applied_font_part: font_part,
        skipped,
    })
}

fn write_template_apply_output(
    file: &str,
    out: Option<&str>,
    backup: Option<&str>,
    in_place: bool,
    no_validate: bool,
    overrides: &BTreeMap<String, String>,
) -> CliResult<String> {
    let output_path = out.filter(|value| !value.trim().is_empty());
    let write_path = if in_place || output_path == Some(file) {
        package_mutation_temp_path(file, "template-apply")
    } else {
        output_path
            .ok_or_else(|| {
                CliError::invalid_args(
                    "must specify exactly one of --out, --in-place, or --dry-run",
                )
            })?
            .to_string()
    };
    copy_zip_with_part_overrides(file, &write_path, overrides)?;
    if !no_validate {
        validate(&write_path, true)?;
    }
    if in_place || output_path == Some(file) {
        if let Some(backup_path) = backup.filter(|value| !value.trim().is_empty()) {
            fs::copy(file, backup_path)
                .map_err(|err| CliError::unexpected(format!("failed to create backup: {err}")))?;
        }
        fs::rename(&write_path, file)
            .or_else(|_| {
                fs::copy(&write_path, file)?;
                fs::remove_file(&write_path)
            })
            .map_err(|err| CliError::unexpected(format!("failed to write output file: {err}")))?;
        Ok(file.to_string())
    } else {
        Ok(write_path)
    }
}

fn update_theme_color(xml: &str, color_name: &str, hex: &str) -> Result<String, String> {
    let theme_elements =
        first_element_span(xml, "themeElements", 0, xml.len()).ok_or("themeElements not found")?;
    let color_scheme = first_element_span(xml, "clrScheme", theme_elements.0, theme_elements.1)
        .ok_or("clrScheme not found")?;
    let color_span = first_element_span(xml, color_name, color_scheme.0, color_scheme.1)
        .ok_or_else(|| format!("theme color {color_name} not found"))?;
    let color_xml = &xml[color_span.0..color_span.1];
    let updated_color = rewrite_theme_color_element(color_xml, hex)?;
    Ok(replace_span(
        xml,
        color_span.0,
        color_span.1,
        &updated_color,
    ))
}

fn rewrite_theme_color_element(color_xml: &str, hex: &str) -> Result<String, String> {
    let (open_end, tag_name, close_start, self_closing) =
        xml_fragment_bounds(color_xml).map_err(|err| err.message)?;
    let prefix = tag_prefix(&tag_name);
    let srgb_tag = qualified_name(&prefix, "srgbClr");
    let srgb = format!("<{srgb_tag} val=\"{}\"/>", xml_attr_escape(hex));
    if self_closing {
        let open_tag = xml_open_tag(color_xml, open_end);
        return Ok(format!("{open_tag}{srgb}</{tag_name}>"));
    }
    let children =
        xml_direct_child_ranges(color_xml, open_end + 1, close_start).map_err(|err| err.message)?;
    let mut out = String::new();
    out.push_str(&color_xml[..=open_end]);
    for child in children {
        if child.kind != "srgbClr" && child.kind != "sysClr" {
            out.push_str(&color_xml[child.start..child.end]);
        }
    }
    out.push_str(&srgb);
    out.push_str(&color_xml[close_start..]);
    Ok(out)
}

fn update_theme_font(
    xml: &str,
    major_font: Option<&str>,
    minor_font: Option<&str>,
) -> Result<String, String> {
    let theme_elements =
        first_element_span(xml, "themeElements", 0, xml.len()).ok_or("themeElements not found")?;
    let font_scheme = first_element_span(xml, "fontScheme", theme_elements.0, theme_elements.1)
        .ok_or("fontScheme not found")?;
    let mut updated = xml.to_string();
    if let Some(major_font) = major_font {
        updated = set_theme_latin_font(&updated, font_scheme, "majorFont", major_font)?;
    }
    if let Some(minor_font) = minor_font {
        updated = set_theme_latin_font(&updated, font_scheme, "minorFont", minor_font)?;
    }
    Ok(updated)
}

fn set_theme_latin_font(
    xml: &str,
    font_scheme_span: (usize, usize),
    font_kind: &str,
    typeface: &str,
) -> Result<String, String> {
    let font_span = first_element_span(xml, font_kind, font_scheme_span.0, font_scheme_span.1)
        .ok_or_else(|| format!("{font_kind} element not found"))?;
    let font_xml = &xml[font_span.0..font_span.1];
    let (open_end, tag_name, close_start, self_closing) =
        xml_fragment_bounds(font_xml).map_err(|err| err.message)?;
    if self_closing {
        return Err(format!("{font_kind} element is self-closing"));
    }
    let prefix = tag_prefix(&tag_name);
    let children =
        xml_direct_child_ranges(font_xml, open_end + 1, close_start).map_err(|err| err.message)?;
    let updated_font = if let Some(latin) = children.iter().find(|child| child.kind == "latin") {
        set_attr_on_element(font_xml, latin.start, "typeface", typeface)
            .map_err(|err| err.message)?
    } else {
        let latin = format!(
            "<{} typeface=\"{}\"/>",
            qualified_name(&prefix, "latin"),
            xml_attr_escape(typeface)
        );
        insert_at_index(font_xml, open_end + 1, &latin)
    };
    Ok(replace_span(xml, font_span.0, font_span.1, &updated_font))
}

fn first_element_span(
    xml: &str,
    wanted: &str,
    range_start: usize,
    range_end: usize,
) -> Option<(usize, usize)> {
    let mut cursor = range_start;
    while cursor < range_end {
        let relative_start = xml[cursor..range_end].find('<')?;
        let tag_start = cursor + relative_start;
        let relative_end = xml[tag_start..range_end].find('>')?;
        let tag_end = tag_start + relative_end;
        let token = xml[tag_start + 1..tag_end].trim_start();
        if token.starts_with('/') || token.starts_with('?') || token.starts_with('!') {
            cursor = tag_end + 1;
            continue;
        }
        let name = xml_token_name(token)?;
        let self_closing = token.trim_end().ends_with('/');
        if local_name(name.as_bytes()) == wanted {
            if self_closing {
                return Some((tag_start, tag_end + 1));
            }
            return find_matching_element_end(xml, wanted, tag_end + 1, range_end)
                .map(|end| (tag_start, end));
        }
        cursor = tag_end + 1;
    }
    None
}

fn find_matching_element_end(
    xml: &str,
    wanted: &str,
    range_start: usize,
    range_end: usize,
) -> Option<usize> {
    let mut depth = 1usize;
    let mut cursor = range_start;
    while cursor < range_end {
        let relative_start = xml[cursor..range_end].find('<')?;
        let tag_start = cursor + relative_start;
        let relative_end = xml[tag_start..range_end].find('>')?;
        let tag_end = tag_start + relative_end;
        let token = xml[tag_start + 1..tag_end].trim_start();
        if token.starts_with('?') || token.starts_with('!') {
            cursor = tag_end + 1;
            continue;
        }
        if let Some(name) = xml_token_name(token)
            && local_name(name.as_bytes()) == wanted
        {
            if token.starts_with('/') {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(tag_end + 1);
                }
            } else if !token.trim_end().ends_with('/') {
                depth += 1;
            }
        }
        cursor = tag_end + 1;
    }
    None
}

fn set_attr_on_element(
    xml: &str,
    element_start: usize,
    attr_name: &str,
    value: &str,
) -> CliResult<String> {
    let tag_end = xml[element_start..]
        .find('>')
        .map(|offset| element_start + offset + 1)
        .ok_or_else(|| CliError::unexpected("invalid XML start tag"))?;
    let start_tag = &xml[element_start..tag_end];
    let replacement = set_attr_on_start_tag(start_tag, attr_name, value)?;
    Ok(replace_span(xml, element_start, tag_end, &replacement))
}

fn set_attr_on_start_tag(start_tag: &str, attr_name: &str, value: &str) -> CliResult<String> {
    let Some(open_end) = start_tag.find('>') else {
        return Err(CliError::unexpected("invalid XML start tag"));
    };
    let Some(token_name) = xml_token_name(&start_tag[1..open_end]) else {
        return Err(CliError::unexpected("invalid XML start tag"));
    };
    let mut cursor = 1 + token_name.len();
    while cursor < open_end {
        while cursor < open_end && start_tag.as_bytes()[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor >= open_end || start_tag.as_bytes()[cursor] == b'/' {
            break;
        }
        let name_start = cursor;
        while cursor < open_end {
            let byte = start_tag.as_bytes()[cursor];
            if byte == b'=' || byte.is_ascii_whitespace() || byte == b'/' {
                break;
            }
            cursor += 1;
        }
        let name_end = cursor;
        while cursor < open_end && start_tag.as_bytes()[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor >= open_end || start_tag.as_bytes()[cursor] != b'=' {
            continue;
        }
        cursor += 1;
        while cursor < open_end && start_tag.as_bytes()[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor >= open_end {
            break;
        }
        let quote = start_tag.as_bytes()[cursor];
        if quote != b'"' && quote != b'\'' {
            continue;
        }
        cursor += 1;
        let value_start = cursor;
        while cursor < open_end && start_tag.as_bytes()[cursor] != quote {
            cursor += 1;
        }
        if cursor >= open_end {
            break;
        }
        let value_end = cursor;
        cursor += 1;
        let existing_name = &start_tag[name_start..name_end];
        if local_name(existing_name.as_bytes()) == attr_name {
            if &start_tag[value_start..value_end] == value {
                return Ok(start_tag.to_string());
            }
            let mut out = String::with_capacity(start_tag.len() + value.len());
            out.push_str(&start_tag[..value_start]);
            out.push_str(&xml_attr_escape(value));
            out.push_str(&start_tag[value_end..]);
            return Ok(out);
        }
    }

    let insert_at = if start_tag[..open_end].trim_end().ends_with('/') {
        start_tag[..open_end]
            .rfind('/')
            .ok_or_else(|| CliError::unexpected("invalid XML start tag"))?
    } else {
        open_end
    };
    let mut out = String::with_capacity(start_tag.len() + attr_name.len() + value.len() + 4);
    out.push_str(&start_tag[..insert_at]);
    out.push(' ');
    out.push_str(attr_name);
    out.push_str("=\"");
    out.push_str(&xml_attr_escape(value));
    out.push('"');
    out.push_str(&start_tag[insert_at..]);
    Ok(out)
}

fn xml_open_tag(fragment: &str, open_end: usize) -> String {
    let start_tag = &fragment[..=open_end];
    if !start_tag.trim_end().ends_with("/>") {
        return start_tag.to_string();
    }
    let slash = start_tag
        .rfind('/')
        .unwrap_or_else(|| start_tag.len().saturating_sub(1));
    let mut out = String::new();
    out.push_str(&start_tag[..slash]);
    out.push('>');
    out
}

fn replace_span(xml: &str, start: usize, end: usize, replacement: &str) -> String {
    let mut out = String::with_capacity(xml.len() - (end - start) + replacement.len());
    out.push_str(&xml[..start]);
    out.push_str(replacement);
    out.push_str(&xml[end..]);
    out
}

fn insert_at_index(xml: &str, index: usize, insertion: &str) -> String {
    let mut out = String::with_capacity(xml.len() + insertion.len());
    out.push_str(&xml[..index]);
    out.push_str(insertion);
    out.push_str(&xml[index..]);
    out
}

fn tag_prefix(tag_name: &str) -> String {
    tag_name
        .split_once(':')
        .map(|(prefix, _)| prefix.to_string())
        .unwrap_or_default()
}

fn qualified_name(prefix: &str, local: &str) -> String {
    if prefix.is_empty() {
        local.to_string()
    } else {
        format!("{prefix}:{local}")
    }
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
