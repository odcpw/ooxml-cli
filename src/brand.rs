use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::palette::{Srgb, ThemePalette};
use crate::{
    CliError, CliResult, attr, copy_zip_with_binary_part_overrides_and_removals, local_name,
    package_type, parse_string_flag, reject_unknown_flags, validate_xlsx_mutation_output_flags,
    xml_attr_escape, zip_entry_names, zip_text,
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

type BrandPartOverrides = (BTreeMap<String, String>, BTreeMap<String, Vec<u8>>);

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
        Self::from_value_with_logo_check(value, source, true)
    }

    fn from_value_with_logo_check(
        value: &Value,
        source: Option<&str>,
        require_logo_file: bool,
    ) -> CliResult<Self> {
        let object = value
            .as_object()
            .ok_or_else(|| CliError::invalid_args("brand kit must be a JSON object"))?;
        reject_unknown_brand_keys(object)?;
        let name = nonempty_string(object.get("name"), "name")?.to_string();
        let (palette, seed) = parse_brand_colors(object)?;
        let fonts = parse_brand_fonts(object.get("fonts"))?;
        let logo = parse_brand_logo(object.get("logo"), source, require_logo_file)?;
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
            let mut logo_json = Map::new();
            logo_json.insert("path".to_string(), json!(logo.path));
            logo_json.insert("placement".to_string(), json!(logo.placement));
            if let Some(width) = logo.width_emu {
                logo_json.insert("widthEmu".to_string(), json!(width));
            }
            if let Some(height) = logo.height_emu {
                logo_json.insert("heightEmu".to_string(), json!(height));
            }
            out.insert("logo".to_string(), Value::Object(logo_json));
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

pub(crate) fn parse_brand_kit_bytes_for_fuzz(source: &[u8]) -> CliResult<Value> {
    let value: Value = serde_json::from_slice(source)
        .map_err(|err| CliError::invalid_args(format!("invalid --brand JSON: {err}")))?;
    BrandKit::from_value_with_logo_check(&value, None, false).map(|kit| kit.to_json())
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
                    .map(|name| name.strip_prefix("ooxml-cli ").unwrap_or(name))
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
    let (overrides, binary_overrides) = brand_overrides(file, family, &kit)?;
    let mut changed_parts = overrides.keys().cloned().collect::<Vec<_>>();
    changed_parts.extend(binary_overrides.keys().cloned());
    changed_parts.sort();
    let stage = crate::mutation_staging_path(file, options.out, "brand-apply");
    copy_zip_with_binary_part_overrides_and_removals(
        file,
        &stage,
        &overrides,
        &binary_overrides,
        &BTreeSet::new(),
    )?;
    if let Err(err) = apply_post_rewrite_features(&stage, family, &kit) {
        let _ = fs::remove_file(&stage);
        return Err(err);
    }
    if !options.no_validate {
        crate::validate_owned_mutation_output(&stage)?;
    }
    crate::finish_mutation_output(
        file,
        &stage,
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;
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
    let (overrides, binary_overrides) = brand_overrides(file, family, &kit)?;
    let replacement = crate::mutation_staging_path(file, None, "brand-scaffold");
    copy_zip_with_binary_part_overrides_and_removals(
        file,
        &replacement,
        &overrides,
        &binary_overrides,
        &BTreeSet::new(),
    )?;
    fs::rename(&replacement, file).map_err(|err| {
        let _ = fs::remove_file(&replacement);
        CliError::unexpected(format!("failed to install branded scaffold stage: {err}"))
    })?;
    apply_post_rewrite_features(file, family, &kit)?;
    Ok(kit)
}

fn brand_overrides(file: &str, family: &str, kit: &BrandKit) -> CliResult<BrandPartOverrides> {
    let mut overrides = crate::template_workflow::brand_theme_overrides(
        file,
        family,
        &kit.name,
        &kit.palette,
        &kit.fonts.heading,
        &kit.fonts.body,
    )?;
    let mut binary_overrides = BTreeMap::new();
    match family {
        "docx" => add_docx_overrides(file, kit, &mut overrides)?,
        "xlsx" => add_xlsx_overrides(file, kit, &mut overrides, &mut binary_overrides)?,
        "pptx" => add_pptx_overrides(file, kit, &mut overrides)?,
        _ => unreachable!("family checked before override generation"),
    }
    Ok((overrides, binary_overrides))
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
    binary_overrides: &mut BTreeMap<String, Vec<u8>>,
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
    for part in zip_entry_names(file)?
        .into_iter()
        .filter(|part| part.starts_with("xl/tables/") && part.ends_with(".xml"))
    {
        let table = zip_text(file, &part)?;
        let updated = if let Some(style) = kit.table_style.as_deref() {
            rewrite_all_tags(&table, "tableStyleInfo", |tag| {
                set_tag_attrs(tag, &[("name", style)])
            })?
        } else {
            table.clone()
        };
        if updated != table {
            overrides.insert(part, updated);
        }
    }
    if let Some(logo) = kit.logo.as_ref() {
        add_xlsx_logo(file, logo, overrides, binary_overrides)?;
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
    if kit.slide_number_policy == "except-title" {
        for part in zip_entry_names(file)?.into_iter().filter(|part| {
            part.starts_with("ppt/slideLayouts/slideLayout") && part.ends_with(".xml")
        }) {
            let layout = zip_text(file, &part)?;
            let updated = update_pptx_title_layout_slide_numbers(&layout)?;
            if updated != layout {
                overrides.insert(part, updated);
            }
        }
    }
    Ok(())
}

fn add_xlsx_logo(
    file: &str,
    logo: &BrandLogo,
    overrides: &mut BTreeMap<String, String>,
    binary_overrides: &mut BTreeMap<String, Vec<u8>>,
) -> CliResult<()> {
    let entries = zip_entry_names(file)?;
    let worksheet_part = entries
        .iter()
        .filter(|part| part.starts_with("xl/worksheets/") && part.ends_with(".xml"))
        .min()
        .cloned()
        .ok_or_else(|| CliError::unexpected("XLSX brand logo requires a worksheet"))?;
    let worksheet_uri = format!("/{}", worksheet_part.trim_start_matches('/'));
    let worksheet_rels_part = crate::relationships_part_for(&worksheet_part);
    let mut worksheet = overrides
        .get(&worksheet_part)
        .cloned()
        .unwrap_or(zip_text(file, &worksheet_part)?);
    let mut worksheet_rels = if entries.iter().any(|part| part == &worksheet_rels_part) {
        zip_text(file, &worksheet_rels_part)?
    } else {
        relationships_template()
    };

    let (drawing_part, drawing_rel_id, existing_drawing) =
        if let Some((drawing_start, drawing_end)) = find_start_tag(&worksheet, "drawing", 0) {
            let id = tag_attr(&worksheet[drawing_start..drawing_end], "id")
                .ok_or_else(|| CliError::unexpected("XLSX drawing has no relationship id"))?;
            let relationship = crate::relationship_entries_from_xml(&worksheet_rels)
                .into_iter()
                .find(|relationship| relationship.id == id)
                .ok_or_else(|| {
                    CliError::unexpected(format!(
                        "XLSX drawing relationship {id} is missing from /{worksheet_rels_part}"
                    ))
                })?;
            (
                crate::resolve_relationship_target(&worksheet_uri, &relationship.target)
                    .trim_start_matches('/')
                    .to_string(),
                id,
                true,
            )
        } else {
            let part = next_numbered_part(&entries, "xl/drawings/drawing", ".xml");
            let id = crate::allocate_relationship_id(&crate::relationship_entries_from_xml(
                &worksheet_rels,
            ));
            let target = crate::relationship_target_from_source_to_target(
                &worksheet_uri,
                &format!("/{part}"),
            );
            worksheet_rels = crate::opc::append_relationship_xml(
                worksheet_rels,
                &crate::RelationshipEntry::new(
                    &id,
                    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/drawing",
                    &target,
                ),
            );
            let root = find_start_tag(&worksheet, "worksheet", 0)
                .ok_or_else(|| CliError::unexpected("worksheet root not found"))?;
            let root_tag = &worksheet[root.0..root.1];
            if tag_attr(root_tag, "r").is_none() && !root_tag.contains("xmlns:r=") {
                worksheet = replace_range(
                    &worksheet,
                    root.0,
                    root.1,
                    &set_tag_attr(
                        root_tag,
                        "xmlns:r",
                        "http://schemas.openxmlformats.org/officeDocument/2006/relationships",
                    )?,
                );
            }
            let insert_at = worksheet_insert_position(&worksheet, "drawing")?;
            worksheet.insert_str(insert_at, &format!("<drawing r:id=\"{id}\"/>"));
            (part, id, false)
        };

    let drawing_rels_part = crate::relationships_part_for(&drawing_part);
    let mut drawing = if existing_drawing {
        zip_text(file, &drawing_part)?
    } else {
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><xdr:wsDr xmlns:xdr="http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"></xdr:wsDr>"#.to_string()
    };
    let mut drawing_rels =
        if existing_drawing && entries.iter().any(|part| part == &drawing_rels_part) {
            zip_text(file, &drawing_rels_part)?
        } else {
            relationships_template()
        };
    let image_rel_id =
        crate::allocate_relationship_id(&crate::relationship_entries_from_xml(&drawing_rels));
    let (extension, content_type) = image_type(&logo.path)?;
    let media_part = next_numbered_part(&entries, "xl/media/brandLogo", &format!(".{extension}"));
    let drawing_uri = format!("/{}", drawing_part.trim_start_matches('/'));
    let media_target =
        crate::relationship_target_from_source_to_target(&drawing_uri, &format!("/{media_part}"));
    drawing_rels = crate::opc::append_relationship_xml(
        drawing_rels,
        &crate::RelationshipEntry::new(
            &image_rel_id,
            "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image",
            &media_target,
        ),
    );
    let shape_id = next_drawing_shape_id(&drawing);
    let anchor = xlsx_logo_anchor_xml(logo, &image_rel_id, shape_id);
    let close = find_end_tag(&drawing, "wsDr", 0)
        .ok_or_else(|| CliError::unexpected("unterminated XLSX drawing root"))?
        .0;
    drawing.insert_str(close, &anchor);

    let mut content_types = overrides
        .get("[Content_Types].xml")
        .cloned()
        .unwrap_or(zip_text(file, "[Content_Types].xml")?);
    content_types = crate::ensure_content_type_override(
        content_types,
        &drawing_part,
        "application/vnd.openxmlformats-officedocument.drawing+xml",
    )?;
    content_types = ensure_default_content_type(content_types, extension, content_type)?;
    overrides.insert(worksheet_part, worksheet);
    overrides.insert(worksheet_rels_part, worksheet_rels);
    overrides.insert(drawing_part, drawing);
    overrides.insert(drawing_rels_part, drawing_rels);
    overrides.insert("[Content_Types].xml".to_string(), content_types);
    binary_overrides.insert(
        media_part,
        fs::read(&logo.path).map_err(|err| {
            CliError::unexpected(format!("failed to read brand logo {:?}: {err}", logo.path))
        })?,
    );
    let _ = drawing_rel_id;
    Ok(())
}

fn relationships_template() -> String {
    crate::opc::empty_relationships_xml(true)
}

fn next_numbered_part(entries: &[String], prefix: &str, suffix: &str) -> String {
    let mut number = 1_u32;
    loop {
        let candidate = format!("{prefix}{number}{suffix}");
        if !entries
            .iter()
            .any(|entry| entry.eq_ignore_ascii_case(&candidate))
        {
            return candidate;
        }
        number += 1;
    }
}

fn image_type(path: &str) -> CliResult<(&'static str, &'static str)> {
    match Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => Ok(("png", "image/png")),
        "jpg" | "jpeg" => Ok(("jpeg", "image/jpeg")),
        "gif" => Ok(("gif", "image/gif")),
        _ => Err(CliError::invalid_args(
            "brand logo must be a PNG, JPEG, or GIF image",
        )),
    }
}

fn next_drawing_shape_id(xml: &str) -> u32 {
    let mut reader = Reader::from_str(xml);
    let mut maximum = 0_u32;
    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) | Ok(Event::Empty(element))
                if local_name(element.name().as_ref()) == "cNvPr" =>
            {
                maximum = maximum.max(
                    attr(&element, "id")
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(0),
                );
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    maximum.saturating_add(1).max(1)
}

fn xlsx_logo_anchor_xml(logo: &BrandLogo, relationship_id: &str, shape_id: u32) -> String {
    let right = logo.placement.ends_with("right");
    let bottom = logo.placement.starts_with("bottom");
    let col = if right { 7 } else { 0 };
    let row = if bottom { 20 } else { 0 };
    let width = logo.width_emu.unwrap_or(1_200_000);
    let height = logo.height_emu.unwrap_or(400_000);
    format!(
        r#"<xdr:oneCellAnchor><xdr:from><xdr:col>{col}</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>{row}</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:from><xdr:ext cx="{width}" cy="{height}"/><xdr:pic><xdr:nvPicPr><xdr:cNvPr id="{shape_id}" name="Brand Logo" descr="Brand logo"/><xdr:cNvPicPr/></xdr:nvPicPr><xdr:blipFill><a:blip r:embed="{}"/><a:stretch><a:fillRect/></a:stretch></xdr:blipFill><xdr:spPr><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></xdr:spPr></xdr:pic><xdr:clientData/></xdr:oneCellAnchor>"#,
        xml_attr_escape(relationship_id),
    )
}

fn ensure_default_content_type(
    xml: String,
    extension: &str,
    content_type: &str,
) -> CliResult<String> {
    let mut reader = Reader::from_str(&xml);
    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) | Ok(Event::Empty(element))
                if local_name(element.name().as_ref()) == "Default"
                    && attr(&element, "Extension")
                        .is_some_and(|value| value.eq_ignore_ascii_case(extension)) =>
            {
                return Ok(xml);
            }
            Ok(Event::Eof) => break,
            Err(err) => {
                return Err(CliError::unexpected(format!(
                    "invalid [Content_Types].xml: {err}"
                )));
            }
            _ => {}
        }
    }
    let close = xml
        .rfind("</Types>")
        .ok_or_else(|| CliError::unexpected("[Content_Types].xml has no Types close tag"))?;
    let default = format!(
        r#"<Default Extension="{}" ContentType="{}"/>"#,
        xml_attr_escape(extension),
        xml_attr_escape(content_type),
    );
    let mut updated = xml;
    updated.insert_str(close, &default);
    Ok(updated)
}

fn update_pptx_title_layout_slide_numbers(xml: &str) -> CliResult<String> {
    let Some((root_start, root_open_end)) = find_start_tag(xml, "sldLayout", 0) else {
        return Err(CliError::unexpected("PPTX slide layout root not found"));
    };
    let root_tag = &xml[root_start..root_open_end];
    if !matches!(
        tag_attr(root_tag, "type").as_deref(),
        Some("title" | "ctrTitle")
    ) {
        return Ok(xml.to_string());
    }
    if contains_local_tag(xml, "hf") {
        return rewrite_all_tags(xml, "hf", |tag| set_tag_attrs(tag, &[("sldNum", "0")]));
    }
    let root_close = find_end_tag(xml, "sldLayout", root_open_end)
        .ok_or_else(|| CliError::unexpected("unterminated PPTX slide layout root"))?
        .0;
    let insert_at = ["timing", "transition", "extLst"]
        .into_iter()
        .filter_map(|child| find_start_tag(xml, child, root_open_end).map(|(start, _)| start))
        .filter(|start| *start < root_close)
        .min()
        .unwrap_or(root_close);
    let prefix = xml_tag_prefix(root_tag);
    let mut updated = xml.to_string();
    updated.insert_str(insert_at, &format!("<{prefix}hf sldNum=\"0\"/>"));
    Ok(updated)
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
        if let Some(logo) = kit.logo.as_ref() {
            apply_pptx_logo(file, logo)?;
        }
    } else if family == "docx" {
        if let Some(footer) = kit.footer_text.as_deref() {
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
        if let Some(logo) = kit.logo.as_ref() {
            apply_docx_logo(file, logo)?;
        }
    }
    Ok(())
}

fn apply_docx_logo(file: &str, logo: &BrandLogo) -> CliResult<()> {
    let after = if logo.placement.starts_with("bottom") {
        let document = zip_text(file, "word/document.xml")?;
        crate::docx_rich_block_reports(&document, false)
            .map_err(|err| CliError::unexpected(err.message))?
            .len()
    } else {
        0
    };
    crate::docx_images::docx_images_insert(
        file,
        crate::docx_images::DocxImageInsertOptions {
            after,
            image_file: &logo.path,
            expected_hash: "",
            width: logo.width_emu.unwrap_or(1_200_000),
            height: logo.height_emu.unwrap_or(400_000),
            caption: None,
            align: if logo.placement.ends_with("right") {
                "right"
            } else {
                "left"
            },
            image: crate::docx_images::DocxImagePipelineArgs {
                fit: Some("contain"),
                max_dpi: None,
                keep_original: false,
                alt: "Brand Logo",
            },
            mutation: crate::DocxParagraphMutationOptions {
                text: None,
                text_file: None,
                style: "",
                out: None,
                backup: None,
                dry_run: false,
                in_place: true,
                no_validate: true,
            },
        },
    )?;
    Ok(())
}

fn apply_pptx_logo(file: &str, logo: &BrandLogo) -> CliResult<()> {
    let slides = crate::pptx_slides_list(file)?
        .get("slides")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let (page_width, page_height) = pptx_page_size(file)?;
    let width = logo.width_emu.unwrap_or(1_200_000);
    let height = logo.height_emu.unwrap_or(400_000);
    let margin = 228_600_i64;
    let x = if logo.placement.ends_with("right") {
        page_width.saturating_sub(width).saturating_sub(margin)
    } else {
        margin
    };
    let y = if logo.placement.starts_with("bottom") {
        page_height.saturating_sub(height).saturating_sub(margin)
    } else {
        margin
    };
    for slide in 1..=slides {
        crate::pptx_mutation::pptx_place_image(
            file,
            &[
                "--slide".to_string(),
                slide.to_string(),
                "--image".to_string(),
                logo.path.clone(),
                "--x".to_string(),
                x.to_string(),
                "--y".to_string(),
                y.to_string(),
                "--cx".to_string(),
                width.to_string(),
                "--cy".to_string(),
                height.to_string(),
                "--name".to_string(),
                "Brand Logo".to_string(),
                "--fit".to_string(),
                "contain".to_string(),
                "--in-place".to_string(),
                "--no-validate".to_string(),
            ],
        )?;
    }
    Ok(())
}

fn pptx_page_size(file: &str) -> CliResult<(i64, i64)> {
    let presentation = zip_text(file, "ppt/presentation.xml")?;
    let (start, end) = find_start_tag(&presentation, "sldSz", 0)
        .ok_or_else(|| CliError::unexpected("PPTX presentation has no slide size"))?;
    let tag = &presentation[start..end];
    let width = tag_attr(tag, "cx")
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| CliError::unexpected("PPTX slide width is invalid"))?;
    let height = tag_attr(tag, "cy")
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| CliError::unexpected("PPTX slide height is invalid"))?;
    Ok((width, height))
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
    let page_size = setup
        .get("paperSize")
        .and_then(Value::as_str)
        .map(docx_paper_twips)
        .transpose()?;
    let orientation = setup.get("orientation").and_then(Value::as_str);
    if page_size.is_some() || orientation.is_some() {
        updated = rewrite_all_tags(&updated, "pgSz", |tag| {
            let mut dimensions = page_size.unwrap_or_else(|| {
                (
                    tag_attr(tag, "w")
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(12_240),
                    tag_attr(tag, "h")
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(15_840),
                )
            });
            if (orientation == Some("landscape") && dimensions.0 < dimensions.1)
                || (orientation == Some("portrait") && dimensions.0 > dimensions.1)
            {
                dimensions = (dimensions.1, dimensions.0);
            }
            let (width, height) = dimensions;
            let width = width.to_string();
            let height = height.to_string();
            let mut changes = vec![("w", width.as_str()), ("h", height.as_str())];
            if let Some(orientation) = orientation {
                changes.push(("orient", orientation));
            }
            set_tag_attrs(tag, &changes)
        })?;
    }
    if let Some(margins) = setup.get("margins").and_then(Value::as_object) {
        let changes = margin_changes(margins, |value| (value * 1440.0).round().to_string())?;
        updated = rewrite_all_tags(&updated, "pgMar", |tag| {
            let borrowed = changes
                .iter()
                .map(|(key, value)| (key.as_str(), value.as_str()))
                .collect::<Vec<_>>();
            set_tag_attrs(tag, &borrowed)
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
    let mut updated = xml.to_string();
    if let Some(setup) = kit.page_setup.as_ref().and_then(Value::as_object) {
        let orientation = setup.get("orientation").and_then(Value::as_str);
        let paper_size = setup
            .get("paperSize")
            .and_then(Value::as_str)
            .map(xlsx_paper_size)
            .transpose()?;
        if orientation.is_some() || paper_size.is_some() {
            let paper_size = paper_size.map(|value| value.to_string());
            if contains_local_tag(&updated, "pageSetup") {
                updated = rewrite_all_tags(&updated, "pageSetup", |tag| {
                    let mut changes = Vec::new();
                    if let Some(orientation) = orientation {
                        changes.push(("orientation", orientation));
                    }
                    if let Some(paper_size) = paper_size.as_deref() {
                        changes.push(("paperSize", paper_size));
                    }
                    set_tag_attrs(tag, &changes)
                })?;
            } else {
                let mut tag = "<pageSetup/>".to_string();
                let mut changes = Vec::new();
                if let Some(orientation) = orientation {
                    changes.push(("orientation", orientation));
                }
                if let Some(paper_size) = paper_size.as_deref() {
                    changes.push(("paperSize", paper_size));
                }
                tag = set_tag_attrs(&tag, &changes)?;
                let insert_at = worksheet_insert_position(&updated, "pageSetup")?;
                updated.insert_str(insert_at, &tag);
            }
        }
        if let Some(margins) = setup.get("margins").and_then(Value::as_object) {
            let changes = margin_changes(margins, format_decimal)?;
            if contains_local_tag(&updated, "pageMargins") {
                updated = rewrite_all_tags(&updated, "pageMargins", |tag| {
                    let borrowed = changes
                        .iter()
                        .map(|(key, value)| (key.as_str(), value.as_str()))
                        .collect::<Vec<_>>();
                    set_tag_attrs(tag, &borrowed)
                })?;
            } else {
                let mut tag = "<pageMargins left=\"0.7\" right=\"0.7\" top=\"0.75\" bottom=\"0.75\" header=\"0.3\" footer=\"0.3\"/>".to_string();
                let borrowed = changes
                    .iter()
                    .map(|(key, value)| (key.as_str(), value.as_str()))
                    .collect::<Vec<_>>();
                tag = set_tag_attrs(&tag, &borrowed)?;
                let insert_at = worksheet_insert_position(&updated, "pageMargins")?;
                updated.insert_str(insert_at, &tag);
            }
        }
    }
    if let Some(footer) = kit.footer_text.as_deref() {
        updated = update_xlsx_footer(&updated, footer)?;
    }
    Ok(updated)
}

fn update_xlsx_footer(xml: &str, footer: &str) -> CliResult<String> {
    let text = crate::xml_escape(footer);
    if let Some((start, open_end)) = find_start_tag(xml, "headerFooter", 0) {
        let tag = &xml[start..open_end];
        if tag.trim_end().ends_with("/>") {
            let prefix = xml_tag_prefix(tag);
            let replacement = format!(
                "<{}headerFooter><{}oddFooter>{}</{}oddFooter></{}headerFooter>",
                prefix, prefix, text, prefix, prefix
            );
            return Ok(replace_range(xml, start, open_end, &replacement));
        }
        let (close_start, _) = find_end_tag(xml, "headerFooter", open_end)
            .ok_or_else(|| CliError::unexpected("unterminated XLSX headerFooter"))?;
        if let Some((footer_start, footer_open_end)) = find_start_tag(xml, "oddFooter", open_end)
            && footer_start < close_start
        {
            let (footer_close, footer_end) = find_end_tag(xml, "oddFooter", footer_open_end)
                .ok_or_else(|| CliError::unexpected("unterminated XLSX oddFooter"))?;
            let replacement = format!(
                "{}{}{}",
                &xml[footer_start..footer_open_end],
                text,
                &xml[footer_close..footer_end]
            );
            return Ok(replace_range(xml, footer_start, footer_end, &replacement));
        }
        let prefix = xml_tag_prefix(tag);
        let odd_footer = format!("<{prefix}oddFooter>{text}</{prefix}oddFooter>");
        let mut updated = xml.to_string();
        updated.insert_str(close_start, &odd_footer);
        return Ok(updated);
    }
    let prefix = xml_tag_prefix(xml);
    let header_footer = format!(
        "<{prefix}headerFooter><{prefix}oddFooter>{text}</{prefix}oddFooter></{prefix}headerFooter>"
    );
    let insert_at = worksheet_insert_position(xml, "headerFooter")?;
    let mut updated = xml.to_string();
    updated.insert_str(insert_at, &header_footer);
    Ok(updated)
}

fn worksheet_insert_position(xml: &str, child: &str) -> CliResult<usize> {
    const ORDER: &[&str] = &[
        "sheetPr",
        "dimension",
        "sheetViews",
        "sheetFormatPr",
        "cols",
        "sheetData",
        "sheetCalcPr",
        "sheetProtection",
        "protectedRanges",
        "scenarios",
        "autoFilter",
        "sortState",
        "dataConsolidate",
        "customSheetViews",
        "mergeCells",
        "phoneticPr",
        "conditionalFormatting",
        "dataValidations",
        "hyperlinks",
        "printOptions",
        "pageMargins",
        "pageSetup",
        "headerFooter",
        "rowBreaks",
        "colBreaks",
        "customProperties",
        "cellWatches",
        "ignoredErrors",
        "smartTags",
        "drawing",
        "legacyDrawing",
        "legacyDrawingHF",
        "picture",
        "oleObjects",
        "controls",
        "webPublishItems",
        "tableParts",
        "extLst",
    ];
    let rank = ORDER
        .iter()
        .position(|name| *name == child)
        .ok_or_else(|| {
            CliError::unexpected(format!("unknown worksheet child order entry {child}"))
        })?;
    let root_open = find_start_tag(xml, "worksheet", 0)
        .ok_or_else(|| CliError::unexpected("worksheet root not found"))?
        .1;
    let root_close = find_end_tag(xml, "worksheet", root_open)
        .ok_or_else(|| CliError::unexpected("unterminated worksheet root"))?
        .0;
    for candidate in ORDER.iter().skip(rank + 1) {
        if let Some((start, _)) = find_start_tag(xml, candidate, root_open)
            && start < root_close
        {
            return Ok(start);
        }
    }
    Ok(root_close)
}

fn xml_tag_prefix(tag: &str) -> String {
    let Some(start) = tag.find('<') else {
        return String::new();
    };
    let token = tag[start + 1..]
        .trim_start()
        .split(|ch: char| ch.is_whitespace() || ch == '/' || ch == '>')
        .next()
        .unwrap_or_default();
    token
        .split_once(':')
        .map(|(prefix, _)| format!("{prefix}:"))
        .unwrap_or_default()
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

fn parse_brand_logo(
    value: Option<&Value>,
    source: Option<&str>,
    require_logo_file: bool,
) -> CliResult<Option<BrandLogo>> {
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
    if require_logo_file && !path.is_file() {
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
    if let Some(margins) = setup.get("margins") {
        let margins = margins
            .as_object()
            .ok_or_else(|| CliError::invalid_args("brand pageSetup.margins must be an object"))?;
        for (key, value) in margins {
            if !matches!(
                key.as_str(),
                "top" | "bottom" | "left" | "right" | "header" | "footer"
            ) {
                return Err(CliError::invalid_args(format!(
                    "unknown brand pageSetup.margins property {key:?}"
                )));
            }
            if value
                .as_f64()
                .is_none_or(|value| !value.is_finite() || value < 0.0)
            {
                return Err(CliError::invalid_args(format!(
                    "brand pageSetup.margins.{key} must be a non-negative number of inches"
                )));
            }
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

fn xlsx_paper_size(name: &str) -> CliResult<i64> {
    match name {
        "letter" => Ok(1),
        "A4" => Ok(9),
        _ => Err(CliError::invalid_args(
            "brand pageSetup.paperSize must be letter or A4",
        )),
    }
}

fn margin_changes<F>(margins: &Map<String, Value>, format: F) -> CliResult<Vec<(String, String)>>
where
    F: Fn(f64) -> String,
{
    let mut changes = Vec::new();
    for key in ["top", "bottom", "left", "right", "header", "footer"] {
        if let Some(value) = margins.get(key) {
            let value = value.as_f64().ok_or_else(|| {
                CliError::invalid_args(format!(
                    "brand pageSetup.margins.{key} must be a number of inches"
                ))
            })?;
            changes.push((key.to_string(), format(value)));
        }
    }
    Ok(changes)
}

fn format_decimal(value: f64) -> String {
    let formatted = format!("{value:.6}");
    formatted
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
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
