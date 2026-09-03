use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::palette::{Srgb, ThemePalette};
use crate::{
    CliError, CliResult, attr, copy_zip_with_part_overrides, local_name, package_type,
    parse_string_flag, reject_unknown_flags, validate_xlsx_mutation_output_flags, xml_attr_escape,
    zip_entry_names, zip_text,
};

#[allow(dead_code)]
const BRAND_SCHEMA: &str = include_str!("../testdata/brand/brand.schema.json");
const COLOR_KEYS: &[(&str, &str)] = &[
    ("dark1", "dk1"),
    ("light1", "lt1"),
    ("dark2", "dk2"),
    ("light2", "lt2"),
    ("accent1", "accent1"),
    ("accent2", "accent2"),
    ("accent3", "accent3"),
    ("accent4", "accent4"),
    ("accent5", "accent5"),
    ("accent6", "accent6"),
    ("hyperlink", "hlink"),
    ("followedHyperlink", "folHlink"),
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BrandFonts {
    pub(crate) heading: String,
    pub(crate) body: String,
    pub(crate) mono: Option<String>,
    pub(crate) fallbacks: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BrandLogo {
    pub(crate) path: String,
    pub(crate) placement: String,
    pub(crate) width_emu: Option<i64>,
    pub(crate) height_emu: Option<i64>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct BrandKit {
    pub(crate) name: String,
    pub(crate) palette: ThemePalette,
    pub(crate) seed: Option<String>,
    pub(crate) fonts: BrandFonts,
    pub(crate) logo: Option<BrandLogo>,
    pub(crate) footer_text: Option<String>,
    pub(crate) slide_number_policy: String,
    pub(crate) page_setup: Option<Value>,
    pub(crate) table_style: Option<String>,
}

pub(crate) struct BrandApplyOptions<'a> {
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) in_place: bool,
    pub(crate) no_validate: bool,
}

impl BrandKit {
    pub(crate) fn load(path: &str) -> CliResult<Self> {
        let body = fs::read_to_string(path).map_err(|err| {
            if err.kind() == std::io::ErrorKind::NotFound {
                CliError::file_not_found(format!("brand file not found: {path}"))
            } else {
                CliError::invalid_args(format!("cannot read --brand {path:?}: {err}"))
            }
        })?;
        let value: Value = serde_json::from_str(&body)
            .map_err(|err| CliError::invalid_args(format!("invalid --brand JSON: {err}")))?;
        Self::from_value(&value, Some(path))
    }

    fn from_value(value: &Value, source: Option<&str>) -> CliResult<Self> {
        let object = value
            .as_object()
            .ok_or_else(|| CliError::invalid_args("brand kit must be a JSON object"))?;
        reject_unknown_brand_keys(object)?;
        let name = nonempty_string(object.get("name"), "name")?.to_string();
        let (palette, seed) = parse_brand_colors(object)?;
        let fonts = parse_brand_fonts(object.get("fonts"))?;
        let logo = parse_brand_logo(object.get("logo"), source)?;
        let footer_text = optional_string(object.get("footerText"), "footerText")?;
        let slide_number_policy =
            optional_string(object.get("slideNumberPolicy"), "slideNumberPolicy")?
                .unwrap_or_else(|| "all".to_string());
        if !matches!(
            slide_number_policy.as_str(),
            "none" | "all" | "except-title"
        ) {
            return Err(CliError::invalid_args(
                "brand slideNumberPolicy must be one of: none, all, except-title",
            ));
        }
        let page_setup = object.get("pageSetup").cloned();
        validate_page_setup(page_setup.as_ref())?;
        let table_style = optional_string(object.get("tableStyle"), "tableStyle")?;
        Ok(Self {
            name,
            palette,
            seed,
            fonts,
            logo,
            footer_text,
            slide_number_policy,
            page_setup,
            table_style,
        })
    }

    #[allow(dead_code)]
    pub(crate) fn theme_seed(&self) -> String {
        self.seed
            .clone()
            .unwrap_or_else(|| self.palette.accent1.to_hex())
    }

    #[allow(dead_code)]
    fn to_json(&self) -> Value {
        let mut colors = Map::new();
        for (json_name, theme_name) in COLOR_KEYS {
            colors.insert(
                (*json_name).to_string(),
                json!(palette_color(&self.palette, theme_name).to_hex()),
            );
        }
        let mut fonts = Map::new();
        fonts.insert("heading".to_string(), json!(self.fonts.heading));
        fonts.insert("body".to_string(), json!(self.fonts.body));
        if let Some(mono) = self.fonts.mono.as_deref() {
            fonts.insert("mono".to_string(), json!(mono));
        }
        if !self.fonts.fallbacks.is_empty() {
            fonts.insert("fallbacks".to_string(), json!(self.fonts.fallbacks));
        }
        let mut out = Map::new();
        out.insert("$schema".to_string(), json!("ooxml-brand.schema.json"));
        out.insert("name".to_string(), json!(self.name));
        out.insert("colors".to_string(), Value::Object(colors));
        out.insert("fonts".to_string(), Value::Object(fonts));
        if let Some(logo) = &self.logo {
            out.insert(
                "logo".to_string(),
                json!({
                    "path": logo.path,
                    "placement": logo.placement,
                    "widthEmu": logo.width_emu,
                    "heightEmu": logo.height_emu,
                }),
            );
        }
        if let Some(footer_text) = self.footer_text.as_deref() {
            out.insert("footerText".to_string(), json!(footer_text));
        }
        out.insert(
            "slideNumberPolicy".to_string(),
            json!(self.slide_number_policy),
        );
        if let Some(page_setup) = &self.page_setup {
            out.insert("pageSetup".to_string(), page_setup.clone());
        }
        if let Some(table_style) = self.table_style.as_deref() {
            out.insert("tableStyle".to_string(), json!(table_style));
        }
        Value::Object(out)
    }
}

#[allow(dead_code)]
pub(crate) fn brand_schema() -> CliResult<Value> {
    serde_json::from_str(BRAND_SCHEMA)
        .map_err(|err| CliError::unexpected(format!("embedded brand schema is invalid: {err}")))
}

#[allow(dead_code)]
pub(crate) fn template_brand_extract(file: &str, args: &[String]) -> CliResult<Value> {
    reject_unknown_flags(args, &["--out", "--name"], &[])?;
    let kind = package_type(file)?;
    if !matches!(kind, "pptx" | "xlsx" | "docx") {
        return Err(CliError::unsupported_type(format!(
            "template brand extract supports PPTX, XLSX, and DOCX files (detected: {kind})"
        )));
    }
    let theme = crate::template_workflow::package_theme(file, kind)?;
    let colors = theme
        .get("colorScheme")
        .and_then(Value::as_object)
        .ok_or_else(|| CliError::unexpected("theme has no color scheme"))?;
    let fonts = theme
        .get("fontScheme")
        .and_then(Value::as_object)
        .ok_or_else(|| CliError::unexpected("theme has no font scheme"))?;
    let mut value = Map::new();
    value.insert(
        "name".to_string(),
        json!(
            parse_string_flag(args, "--name")?
                .filter(|name| !name.trim().is_empty())
                .or_else(|| theme
                    .get("name")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned))
                .unwrap_or_else(|| format!("{} brand", kind.to_ascii_uppercase()))
        ),
    );
    let mut brand_colors = Map::new();
    for (brand_key, theme_key) in COLOR_KEYS {
        let token_key = match *theme_key {
            "dk1" => "dark1",
            "lt1" => "light1",
            "dk2" => "dark2",
            "lt2" => "light2",
            "hlink" => "hypLink",
            "folHlink" => "folLink",
            key => key,
        };
        let color = colors
            .get(token_key)
            .and_then(Value::as_str)
            .ok_or_else(|| CliError::unexpected(format!("theme color {token_key} is missing")))?;
        brand_colors.insert((*brand_key).to_string(), json!(color.to_ascii_uppercase()));
    }
    value.insert("colors".to_string(), Value::Object(brand_colors));
    value.insert(
        "fonts".to_string(),
        json!({
            "heading": fonts.get("majorFont").and_then(Value::as_str).unwrap_or("Aptos Display"),
            "body": fonts.get("minorFont").and_then(Value::as_str).unwrap_or("Aptos"),
        }),
    );
    value.insert("slideNumberPolicy".to_string(), json!("all"));
    let kit = BrandKit::from_value(&Value::Object(value), None)?;
    let output = kit.to_json();
    if let Some(out) = parse_string_flag(args, "--out")?.filter(|path| !path.trim().is_empty()) {
        let mut bytes = serde_json::to_vec_pretty(&output)
            .map_err(|err| CliError::unexpected(format!("failed to serialize brand kit: {err}")))?;
        bytes.push(b'\n');
        fs::write(&out, bytes).map_err(|err| {
            CliError::unexpected(format!("failed to write brand kit {out:?}: {err}"))
        })?;
    }
    Ok(json!({
        "file": file,
        "family": kind,
        "brand": output,
        "schema": "ooxml-brand.schema.json",
    }))
}

pub(crate) fn template_apply_brand(
    file: &str,
    brand_path: &str,
    options: BrandApplyOptions<'_>,
) -> CliResult<Value> {
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;
    let kit = BrandKit::load(brand_path)?;
    let family = package_type(file)?;
    ensure_brand_family(family)?;
    let overrides = brand_overrides(file, family, &kit)?;
    let changed_parts = overrides.keys().cloned().collect::<Vec<_>>();
    if !options.dry_run {
        let stage = crate::mutation_staging_path(file, options.out, "brand-apply");
        copy_zip_with_part_overrides(file, &stage, &overrides)?;
        apply_post_rewrite_features(&stage, family, &kit)?;
        if !options.no_validate {
            crate::validate_owned_mutation_output(&stage)?;
        }
        crate::finish_mutation_output(
            file,
            &stage,
            options.out,
            options.in_place,
            options.backup,
            false,
        )?;
    }
    Ok(json!({
        "file": file,
        "output": if options.dry_run { Value::Null } else if options.in_place { json!(file) } else { json!(options.out) },
        "family": family,
        "brand": kit.name,
        "brandSource": brand_path,
        "changed": !changed_parts.is_empty() || kit.footer_text.is_some(),
        "changedParts": changed_parts,
        "dryRun": options.dry_run,
        "validated": !options.no_validate && !options.dry_run,
    }))
}

#[allow(dead_code)]
pub(crate) fn apply_to_staged_package(file: &str, brand_path: &str) -> CliResult<BrandKit> {
    let kit = BrandKit::load(brand_path)?;
    let family = package_type(file)?;
    ensure_brand_family(family)?;
    let overrides = brand_overrides(file, family, &kit)?;
    let replacement = crate::mutation_staging_path(file, None, "brand-scaffold");
    copy_zip_with_part_overrides(file, &replacement, &overrides)?;
    fs::rename(&replacement, file).map_err(|err| {
        let _ = fs::remove_file(&replacement);
        CliError::unexpected(format!("failed to install branded scaffold stage: {err}"))
    })?;
    apply_post_rewrite_features(file, family, &kit)?;
    Ok(kit)
}

fn brand_overrides(
    file: &str,
    family: &str,
    kit: &BrandKit,
) -> CliResult<BTreeMap<String, String>> {
    let mut overrides = crate::template_workflow::brand_theme_overrides(
        file,
        family,
        &kit.palette,
        &kit.fonts.heading,
        &kit.fonts.body,
    )?;
    match family {
        "docx" => add_docx_overrides(file, kit, &mut overrides)?,
        "xlsx" => add_xlsx_overrides(file, kit, &mut overrides)?,
        "pptx" => add_pptx_overrides(file, kit, &mut overrides)?,
        _ => unreachable!("family checked before override generation"),
    }
    Ok(overrides)
}

fn add_docx_overrides(
    file: &str,
    kit: &BrandKit,
    overrides: &mut BTreeMap<String, String>,
) -> CliResult<()> {
    if zip_entry_names(file)?
        .iter()
        .any(|part| part == "word/styles.xml")
    {
        let styles = zip_text(file, "word/styles.xml")?;
        let updated = update_docx_styles(&styles, kit)?;
        if updated != styles {
            overrides.insert("word/styles.xml".to_string(), updated);
        }
    }
    let document_part = crate::find_docx_document_part(file, &zip_entry_names(file)?)?;
    let document = zip_text(file, &document_part)?;
    let updated = update_docx_document(&document, kit)?;
    if updated != document {
        overrides.insert(document_part, updated);
    }
    Ok(())
}

fn add_xlsx_overrides(
    file: &str,
    kit: &BrandKit,
    overrides: &mut BTreeMap<String, String>,
) -> CliResult<()> {
    if zip_entry_names(file)?
        .iter()
        .any(|part| part == "xl/styles.xml")
    {
        let styles = zip_text(file, "xl/styles.xml")?;
        let updated = update_xlsx_styles(&styles, kit)?;
        if updated != styles {
            overrides.insert("xl/styles.xml".to_string(), updated);
        }
    }
    for part in zip_entry_names(file)?
        .into_iter()
        .filter(|part| part.starts_with("xl/worksheets/") && part.ends_with(".xml"))
    {
        let worksheet = zip_text(file, &part)?;
        let updated = update_xlsx_worksheet(&worksheet, kit)?;
        if updated != worksheet {
            overrides.insert(part, updated);
        }
    }
    Ok(())
}

fn add_pptx_overrides(
    file: &str,
    kit: &BrandKit,
    overrides: &mut BTreeMap<String, String>,
) -> CliResult<()> {
    if let Some(page_setup) = kit.page_setup.as_ref() {
        let presentation = zip_text(file, "ppt/presentation.xml")?;
        let updated = update_pptx_page_setup(&presentation, page_setup)?;
        if updated != presentation {
            overrides.insert("ppt/presentation.xml".to_string(), updated);
        }
    }
    Ok(())
}

fn apply_post_rewrite_features(file: &str, family: &str, kit: &BrandKit) -> CliResult<()> {
    if family == "pptx" {
        let mut args = Vec::<String>::new();
        if let Some(footer) = kit.footer_text.as_deref() {
            args.extend(["--footer".to_string(), footer.to_string()]);
            args.push("--show-footer=true".to_string());
        }
        args.push(format!(
            "--show-slide-number={}",
            kit.slide_number_policy != "none"
        ));
        args.extend(["--in-place".to_string(), "--no-validate".to_string()]);
        crate::pptx_mutation::pptx_fields_set(file, &args)?;
    } else if family == "docx"
        && let Some(footer) = kit.footer_text.as_deref()
    {
        crate::docx_headers::docx_headers_footers_set_text(
            file,
            "footer",
            crate::docx_headers::DocxHeaderFooterSetTextOptions {
                id: "",
                ref_type: "default",
                section: 0,
                index: 1,
                selector: None,
                selector_given: false,
                index_given: false,
                text: footer,
                page_numbers: false,
                out: None,
                backup: None,
                dry_run: false,
                in_place: true,
                no_validate: true,
            },
        )?;
    }
    Ok(())
}

fn update_docx_styles(xml: &str, kit: &BrandKit) -> CliResult<String> {
    let mut updated = rewrite_all_tags(xml, "rFonts", |tag| {
        set_tag_attrs(
            tag,
            &[
                ("ascii", kit.fonts.body.as_str()),
                ("hAnsi", kit.fonts.body.as_str()),
                ("eastAsia", kit.fonts.body.as_str()),
                ("cs", kit.fonts.body.as_str()),
            ],
        )
    })?;
    for style_id in ["Title", "Heading1", "Heading2", "Heading3", "Heading4"] {
        updated = update_docx_named_style_font(&updated, style_id, &kit.fonts.heading)?;
    }
    updated = rewrite_all_tags(&updated, "color", |tag| {
        rewrite_theme_bound_color(tag, "themeColor", "val", &kit.palette)
    })?;
    updated = rewrite_all_tags(&updated, "shd", |tag| {
        rewrite_theme_bound_color(tag, "themeFill", "fill", &kit.palette)
    })?;
    Ok(updated)
}

fn update_docx_named_style_font(xml: &str, style_id: &str, font: &str) -> CliResult<String> {
    let needle = format!(r#"w:styleId="{style_id}""#);
    let Some(id_at) = xml.find(&needle) else {
        return Ok(xml.to_string());
    };
    let Some(start) = xml[..id_at].rfind("<w:style ") else {
        return Ok(xml.to_string());
    };
    let Some(relative_end) = xml[id_at..].find("</w:style>") else {
        return Err(CliError::unexpected(format!(
            "unterminated DOCX style {style_id}"
        )));
    };
    let end = id_at + relative_end + "</w:style>".len();
    let fragment = &xml[start..end];
    let mut replacement = rewrite_all_tags(fragment, "rFonts", |tag| {
        set_tag_attrs(
            tag,
            &[
                ("ascii", font),
                ("hAnsi", font),
                ("eastAsia", font),
                ("cs", font),
            ],
        )
    })?;
    if !contains_local_tag(&replacement, "rFonts") {
        let rpr_start = replacement.find("<w:rPr>").ok_or_else(|| {
            CliError::unexpected(format!("DOCX style {style_id} has no run properties"))
        })? + "<w:rPr>".len();
        let rfonts = format!(
            r#"<w:rFonts w:ascii="{}" w:hAnsi="{}" w:eastAsia="{}" w:cs="{}"/>"#,
            xml_attr_escape(font),
            xml_attr_escape(font),
            xml_attr_escape(font),
            xml_attr_escape(font),
        );
        replacement.insert_str(rpr_start, &rfonts);
    }
    Ok(replace_range(xml, start, end, &replacement))
}

fn update_docx_document(xml: &str, kit: &BrandKit) -> CliResult<String> {
    let mut updated = xml.to_string();
    if let Some(style) = kit.table_style.as_deref() {
        updated = rewrite_all_tags(&updated, "tblStyle", |tag| {
            set_tag_attrs(tag, &[("val", style)])
        })?;
    }
    let Some(setup) = kit.page_setup.as_ref().and_then(Value::as_object) else {
        return Ok(updated);
    };
    if let Some(page) = setup.get("paperSize").and_then(Value::as_str) {
        let (width, height) = docx_paper_twips(page)?;
        updated = rewrite_all_tags(&updated, "pgSz", |tag| {
            let orientation = setup.get("orientation").and_then(Value::as_str);
            let (width, height) = if orientation == Some("landscape") {
                (height, width)
            } else {
                (width, height)
            };
            let width = width.to_string();
            let height = height.to_string();
            let mut changes = vec![("w", width.as_str()), ("h", height.as_str())];
            if let Some(orientation) = orientation {
                changes.push(("orient", orientation));
            }
            set_tag_attrs(tag, &changes)
        })?;
    }
    Ok(updated)
}

fn update_xlsx_styles(xml: &str, kit: &BrandKit) -> CliResult<String> {
    let fonts_start = find_start_tag(xml, "fonts", 0).map(|(_, end)| end);
    let fonts_end =
        fonts_start.and_then(|start| find_end_tag(xml, "fonts", start).map(|(at, _)| at));
    let mut updated = xml.to_string();
    if let (Some(start), Some(end)) = (fonts_start, fonts_end) {
        let fragment = &xml[start..end];
        let rewritten = rewrite_all_tags(fragment, "name", |tag| {
            set_tag_attrs(tag, &[("val", kit.fonts.body.as_str())])
        })?;
        updated = replace_range(xml, start, end, &rewritten);
    }
    if let Some(style) = kit.table_style.as_deref() {
        updated = rewrite_all_tags(&updated, "tableStyles", |tag| {
            set_tag_attrs(tag, &[("defaultTableStyle", style)])
        })?;
    }
    Ok(updated)
}

fn update_xlsx_worksheet(xml: &str, kit: &BrandKit) -> CliResult<String> {
    let Some(setup) = kit.page_setup.as_ref().and_then(Value::as_object) else {
        return Ok(xml.to_string());
    };
    let Some(orientation) = setup.get("orientation").and_then(Value::as_str) else {
        return Ok(xml.to_string());
    };
    rewrite_all_tags(xml, "pageSetup", |tag| {
        set_tag_attrs(tag, &[("orientation", orientation)])
    })
}

fn update_pptx_page_setup(xml: &str, setup: &Value) -> CliResult<String> {
    let setup = setup
        .as_object()
        .ok_or_else(|| CliError::invalid_args("brand pageSetup must be an object"))?;
    let Some(size) = setup.get("slideSize").and_then(Value::as_str) else {
        return Ok(xml.to_string());
    };
    let (cx, cy, kind) = match size {
        "screen16x9" => (12_192_000_i64, 6_858_000_i64, "screen16x9"),
        "screen4x3" => (9_144_000, 6_858_000, "screen4x3"),
        "A4" => (10_080_000, 7_560_000, "A4"),
        _ => {
            return Err(CliError::invalid_args(
                "brand pageSetup.slideSize must be screen16x9, screen4x3, or A4",
            ));
        }
    };
    let cx = cx.to_string();
    let cy = cy.to_string();
    rewrite_all_tags(xml, "sldSz", |tag| {
        set_tag_attrs(tag, &[("cx", &cx), ("cy", &cy), ("type", kind)])
    })
}

fn parse_brand_colors(object: &Map<String, Value>) -> CliResult<(ThemePalette, Option<String>)> {
    let legacy_seed = object
        .get("themeSeed")
        .or_else(|| object.get("seed"))
        .and_then(Value::as_str);
    let colors = object.get("colors").and_then(Value::as_object);
    let seed = colors
        .and_then(|colors| colors.get("seed"))
        .and_then(Value::as_str)
        .or(legacy_seed);
    if let Some(seed) = seed {
        if colors.is_some_and(|colors| colors.keys().any(|key| key != "seed")) {
            return Err(CliError::invalid_args(
                "brand colors must use either seed or the complete color scheme, not both",
            ));
        }
        let normalized = normalize_hex(seed, "colors.seed")?;
        let palette = ThemePalette::derive(&normalized)
            .map_err(|err| CliError::invalid_args(err.to_string()))?;
        return Ok((palette, Some(normalized)));
    }
    let colors = colors.ok_or_else(|| {
        CliError::invalid_args("brand colors requires seed or a complete 12-color scheme")
    })?;
    let color = |key: &str| -> CliResult<Srgb> {
        let value = nonempty_string(colors.get(key), &format!("colors.{key}"))?;
        Srgb::from_hex(value).map_err(|err| CliError::invalid_args(err.to_string()))
    };
    for key in colors.keys() {
        if key != "seed" && !COLOR_KEYS.iter().any(|(name, _)| key == name) {
            return Err(CliError::invalid_args(format!(
                "unknown brand colors property {key:?}"
            )));
        }
    }
    Ok((
        ThemePalette {
            dk1: color("dark1")?,
            lt1: color("light1")?,
            dk2: color("dark2")?,
            lt2: color("light2")?,
            accent1: color("accent1")?,
            accent2: color("accent2")?,
            accent3: color("accent3")?,
            accent4: color("accent4")?,
            accent5: color("accent5")?,
            accent6: color("accent6")?,
            hlink: color("hyperlink")?,
            fol_hlink: color("followedHyperlink")?,
        },
        None,
    ))
}

fn parse_brand_fonts(value: Option<&Value>) -> CliResult<BrandFonts> {
    let fonts = value
        .and_then(Value::as_object)
        .ok_or_else(|| CliError::invalid_args("brand fonts must be an object"))?;
    for key in fonts.keys() {
        if !matches!(
            key.as_str(),
            "heading" | "body" | "mono" | "fallbacks" | "major" | "minor"
        ) {
            return Err(CliError::invalid_args(format!(
                "unknown brand fonts property {key:?}"
            )));
        }
    }
    let heading = nonempty_string(
        fonts.get("heading").or_else(|| fonts.get("major")),
        "fonts.heading",
    )?
    .to_string();
    let body = nonempty_string(
        fonts.get("body").or_else(|| fonts.get("minor")),
        "fonts.body",
    )?
    .to_string();
    let mono = optional_string(fonts.get("mono"), "fonts.mono")?;
    let fallbacks = match fonts.get("fallbacks") {
        None => Vec::new(),
        Some(Value::Array(values)) => values
            .iter()
            .enumerate()
            .map(|(index, value)| {
                nonempty_string(Some(value), &format!("fonts.fallbacks[{index}]"))
                    .map(ToOwned::to_owned)
            })
            .collect::<CliResult<Vec<_>>>()?,
        Some(_) => {
            return Err(CliError::invalid_args(
                "brand fonts.fallbacks must be an array of font names",
            ));
        }
    };
    Ok(BrandFonts {
        heading,
        body,
        mono,
        fallbacks,
    })
}

fn parse_brand_logo(value: Option<&Value>, source: Option<&str>) -> CliResult<Option<BrandLogo>> {
    let Some(value) = value else {
        return Ok(None);
    };
    let logo = value
        .as_object()
        .ok_or_else(|| CliError::invalid_args("brand logo must be an object"))?;
    for key in logo.keys() {
        if !matches!(
            key.as_str(),
            "path" | "placement" | "widthEmu" | "heightEmu"
        ) {
            return Err(CliError::invalid_args(format!(
                "unknown brand logo property {key:?}"
            )));
        }
    }
    let raw_path = nonempty_string(logo.get("path"), "logo.path")?;
    let path = if Path::new(raw_path).is_absolute() {
        Path::new(raw_path).to_path_buf()
    } else if let Some(parent) = source.and_then(|path| Path::new(path).parent()) {
        parent.join(raw_path)
    } else {
        Path::new(raw_path).to_path_buf()
    };
    if !path.is_file() {
        return Err(CliError::file_not_found(format!(
            "brand logo file not found: {}",
            path.display()
        )));
    }
    let placement = optional_string(logo.get("placement"), "logo.placement")?
        .unwrap_or_else(|| "top-right".to_string());
    if !matches!(
        placement.as_str(),
        "top-left" | "top-right" | "bottom-left" | "bottom-right"
    ) {
        return Err(CliError::invalid_args(
            "brand logo.placement must be top-left, top-right, bottom-left, or bottom-right",
        ));
    }
    let width_emu = optional_positive_i64(logo.get("widthEmu"), "logo.widthEmu")?;
    let height_emu = optional_positive_i64(logo.get("heightEmu"), "logo.heightEmu")?;
    Ok(Some(BrandLogo {
        path: path.to_string_lossy().to_string(),
        placement,
        width_emu,
        height_emu,
    }))
}

fn validate_page_setup(value: Option<&Value>) -> CliResult<()> {
    let Some(value) = value else {
        return Ok(());
    };
    let setup = value
        .as_object()
        .ok_or_else(|| CliError::invalid_args("brand pageSetup must be an object"))?;
    for key in setup.keys() {
        if !matches!(
            key.as_str(),
            "orientation" | "paperSize" | "slideSize" | "margins"
        ) {
            return Err(CliError::invalid_args(format!(
                "unknown brand pageSetup property {key:?}"
            )));
        }
    }
    if let Some(orientation) = setup.get("orientation") {
        let orientation = nonempty_string(Some(orientation), "pageSetup.orientation")?;
        if !matches!(orientation, "portrait" | "landscape") {
            return Err(CliError::invalid_args(
                "brand pageSetup.orientation must be portrait or landscape",
            ));
        }
    }
    Ok(())
}

fn reject_unknown_brand_keys(object: &Map<String, Value>) -> CliResult<()> {
    for key in object.keys() {
        if !matches!(
            key.as_str(),
            "$schema"
                | "name"
                | "colors"
                | "fonts"
                | "logo"
                | "footerText"
                | "slideNumberPolicy"
                | "pageSetup"
                | "tableStyle"
                | "themeSeed"
                | "seed"
        ) {
            return Err(CliError::invalid_args(format!(
                "unknown brand property {key:?}"
            )));
        }
    }
    Ok(())
}

fn ensure_brand_family(family: &str) -> CliResult<()> {
    if matches!(family, "pptx" | "xlsx" | "docx") {
        Ok(())
    } else {
        Err(CliError::unsupported_type(format!(
            "brand kits support PPTX, XLSX, and DOCX files (detected: {family})"
        )))
    }
}

fn optional_string(value: Option<&Value>, path: &str) -> CliResult<Option<String>> {
    let Some(value) = value else {
        return Ok(None);
    };
    Ok(Some(nonempty_string(Some(value), path)?.to_string()))
}

fn nonempty_string<'a>(value: Option<&'a Value>, path: &str) -> CliResult<&'a str> {
    value
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| CliError::invalid_args(format!("brand {path} must be a non-empty string")))
}

fn optional_positive_i64(value: Option<&Value>, path: &str) -> CliResult<Option<i64>> {
    let Some(value) = value else {
        return Ok(None);
    };
    let value = value.as_i64().filter(|value| *value > 0).ok_or_else(|| {
        CliError::invalid_args(format!("brand {path} must be a positive integer"))
    })?;
    Ok(Some(value))
}

fn normalize_hex(value: &str, path: &str) -> CliResult<String> {
    Srgb::from_hex(value)
        .map(|color| color.to_hex())
        .map_err(|_| CliError::invalid_args(format!("brand {path} must be RRGGBB or #RRGGBB")))
}

#[allow(dead_code)]
fn palette_color(palette: &ThemePalette, name: &str) -> Srgb {
    match name {
        "dk1" => palette.dk1,
        "lt1" => palette.lt1,
        "dk2" => palette.dk2,
        "lt2" => palette.lt2,
        "accent1" => palette.accent1,
        "accent2" => palette.accent2,
        "accent3" => palette.accent3,
        "accent4" => palette.accent4,
        "accent5" => palette.accent5,
        "accent6" => palette.accent6,
        "hlink" => palette.hlink,
        "folHlink" => palette.fol_hlink,
        _ => unreachable!("reviewed theme color table"),
    }
}

fn rewrite_theme_bound_color(
    tag: &str,
    theme_attr: &str,
    color_attr: &str,
    palette: &ThemePalette,
) -> CliResult<String> {
    let Some(theme) = tag_attr(tag, theme_attr) else {
        return Ok(tag.to_string());
    };
    let color = match theme.as_str() {
        "accent1" => palette.accent1,
        "accent2" => palette.accent2,
        "accent3" => palette.accent3,
        "accent4" => palette.accent4,
        "accent5" => palette.accent5,
        "accent6" => palette.accent6,
        "hyperlink" => palette.hlink,
        "followedHyperlink" => palette.fol_hlink,
        "text1" | "dk1" => palette.dk1,
        "text2" | "dk2" => palette.dk2,
        "background1" | "lt1" => palette.lt1,
        "background2" | "lt2" => palette.lt2,
        _ => return Ok(tag.to_string()),
    };
    let value = color.to_hex();
    set_tag_attrs(tag, &[(color_attr, &value)])
}

fn rewrite_all_tags<F>(xml: &str, wanted: &str, mut rewrite: F) -> CliResult<String>
where
    F: FnMut(&str) -> CliResult<String>,
{
    let mut out = String::with_capacity(xml.len());
    let mut cursor = 0;
    while let Some((start, end)) = find_start_tag(xml, wanted, cursor) {
        out.push_str(&xml[cursor..start]);
        out.push_str(&rewrite(&xml[start..end])?);
        cursor = end;
    }
    out.push_str(&xml[cursor..]);
    Ok(out)
}

fn contains_local_tag(xml: &str, wanted: &str) -> bool {
    find_start_tag(xml, wanted, 0).is_some()
}

fn find_start_tag(xml: &str, wanted: &str, from: usize) -> Option<(usize, usize)> {
    let mut cursor = from;
    while let Some(relative) = xml[cursor..].find('<') {
        let start = cursor + relative;
        let end = start + xml[start..].find('>')? + 1;
        let token = xml[start + 1..end - 1].trim_start();
        if !token.starts_with(['/', '?', '!']) {
            let name = token
                .split(|ch: char| ch.is_whitespace() || ch == '/')
                .next()?;
            if local_name(name.as_bytes()) == wanted {
                return Some((start, end));
            }
        }
        cursor = end;
    }
    None
}

fn find_end_tag(xml: &str, wanted: &str, from: usize) -> Option<(usize, usize)> {
    let mut cursor = from;
    while let Some(relative) = xml[cursor..].find("</") {
        let start = cursor + relative;
        let end = start + xml[start..].find('>')? + 1;
        let name = xml[start + 2..end - 1].trim();
        if local_name(name.as_bytes()) == wanted {
            return Some((start, end));
        }
        cursor = end;
    }
    None
}

fn set_tag_attrs(tag: &str, changes: &[(&str, &str)]) -> CliResult<String> {
    let mut updated = tag.to_string();
    for (name, value) in changes {
        updated = set_tag_attr(&updated, name, value)?;
    }
    Ok(updated)
}

fn set_tag_attr(tag: &str, wanted: &str, value: &str) -> CliResult<String> {
    let close = tag
        .rfind('>')
        .ok_or_else(|| CliError::unexpected("invalid XML tag"))?;
    let bytes = tag.as_bytes();
    let mut cursor = tag.find(char::is_whitespace).unwrap_or(close);
    while cursor < close {
        while cursor < close && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor >= close || bytes[cursor] == b'/' {
            break;
        }
        let name_start = cursor;
        while cursor < close && !bytes[cursor].is_ascii_whitespace() && bytes[cursor] != b'=' {
            cursor += 1;
        }
        let name_end = cursor;
        while cursor < close && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor >= close || bytes[cursor] != b'=' {
            continue;
        }
        cursor += 1;
        while cursor < close && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor >= close || !matches!(bytes[cursor], b'\'' | b'"') {
            continue;
        }
        let quote = bytes[cursor];
        cursor += 1;
        let value_start = cursor;
        while cursor < close && bytes[cursor] != quote {
            cursor += 1;
        }
        let value_end = cursor;
        cursor = cursor.saturating_add(1);
        if local_name(&tag.as_bytes()[name_start..name_end]) == wanted {
            return Ok(format!(
                "{}{}{}",
                &tag[..value_start],
                xml_attr_escape(value),
                &tag[value_end..]
            ));
        }
    }
    let insert = if tag[..close].trim_end().ends_with('/') {
        tag[..close].rfind('/').unwrap_or(close)
    } else {
        close
    };
    let prefix = tag[1..]
        .split(|ch: char| ch.is_whitespace() || ch == '/' || ch == '>')
        .next()
        .and_then(|name| name.split_once(':').map(|(prefix, _)| prefix));
    let qualified = prefix
        .map(|prefix| format!("{prefix}:{wanted}"))
        .unwrap_or_else(|| wanted.to_string());
    Ok(format!(
        "{} {}=\"{}\"{}",
        &tag[..insert],
        qualified,
        xml_attr_escape(value),
        &tag[insert..]
    ))
}

fn tag_attr(tag: &str, wanted: &str) -> Option<String> {
    let mut reader = Reader::from_str(tag);
    match reader.read_event().ok()? {
        Event::Start(start) | Event::Empty(start) => attr(&start, wanted),
        _ => None,
    }
}

fn replace_range(xml: &str, start: usize, end: usize, replacement: &str) -> String {
    let mut out = String::with_capacity(xml.len() - (end - start) + replacement.len());
    out.push_str(&xml[..start]);
    out.push_str(replacement);
    out.push_str(&xml[end..]);
    out
}

fn docx_paper_twips(name: &str) -> CliResult<(i64, i64)> {
    match name {
        "letter" => Ok((12_240, 15_840)),
        "A4" => Ok((11_906, 16_838)),
        _ => Err(CliError::invalid_args(
            "brand pageSetup.paperSize must be letter or A4",
        )),
    }
}

#[allow(dead_code)]
pub(crate) fn brand_schema_command(args: &[String]) -> CliResult<Value> {
    reject_unknown_flags(args, &[], &[])?;
    Ok(json!({
        "schema": "brand",
        "document": brand_schema()?,
    }))
}

#[cfg(test)]
mod tests {
    use super::{BrandKit, brand_schema};
    use serde_json::json;

    #[test]
    fn schema_and_seed_brand_are_valid_and_deterministic() {
        let schema = brand_schema().expect("embedded schema");
        assert_eq!(
            schema["$id"],
            "https://ooxml-cli.dev/schemas/brand.schema.json"
        );
        let input = json!({
            "name": "Northwind",
            "colors": {"seed": "#4472C4"},
            "fonts": {
                "heading": "Aptos Display",
                "body": "Aptos",
                "mono": "Cascadia Mono",
                "fallbacks": ["Arial", "Liberation Sans"]
            },
            "slideNumberPolicy": "except-title",
            "tableStyle": "TableStyleMedium2"
        });
        let first = BrandKit::from_value(&input, None).expect("brand");
        let second = BrandKit::from_value(&input, None).expect("same brand");
        assert_eq!(first, second);
        assert_eq!(first.palette.accent1.to_hex(), "4472C4");
        assert_eq!(first.fonts.heading, "Aptos Display");
    }

    #[test]
    fn incomplete_full_scheme_is_rejected() {
        let error = BrandKit::from_value(
            &json!({
                "name": "Broken",
                "colors": {"accent1": "4472C4"},
                "fonts": {"heading": "Arial", "body": "Arial"}
            }),
            None,
        )
        .expect_err("incomplete scheme must fail");
        assert!(error.message.contains("colors.dark1"));
    }
}
