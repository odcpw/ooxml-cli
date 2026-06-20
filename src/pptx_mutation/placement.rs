use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::cli_args::value_flag_present;
use crate::pptx_readback::pptx_shapes_get;
use crate::{
    CliError, CliResult, add_relationship_to_xml, allocate_relationship_id, attr, command_arg,
    copy_zip_with_binary_part_overrides_and_removals, copy_zip_with_part_overrides,
    ensure_content_type_override, local_name, needs_xml_space_preserve, package_mutation_temp_path,
    package_type, parse_i64_flag, parse_string_flag, pptx_slide_show,
    relationship_entries_from_xml, relationship_target_from_source_to_target,
    relationships_part_for, validate, validate_xlsx_mutation_output_flags, xml_attr_escape,
    xml_direct_child_ranges, xml_escape, zip_entry_names, zip_text,
};

const REL_TYPE_IMAGE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";
const R_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

#[derive(Clone)]
struct PlacementMutationOptions {
    out: Option<String>,
    backup: Option<String>,
    dry_run: bool,
    in_place: bool,
    no_validate: bool,
}

#[derive(Clone, Copy)]
struct Bounds {
    x: i64,
    y: i64,
    cx: i64,
    cy: i64,
}

#[derive(Clone, Copy)]
struct XmlSpan {
    start: usize,
    end: usize,
}

struct PlacementStage {
    staged_path: String,
    output_path: Option<String>,
}

struct TextboxRequest {
    slide: u32,
    text: String,
    bounds: Bounds,
    name: String,
    font_size: f64,
    font_family: String,
    bold: bool,
    italic: bool,
    color: String,
    level: i64,
    align: String,
}

struct ImageRequest {
    slide: u32,
    image_path: String,
    bounds: Bounds,
    name: String,
    fit_mode: String,
}

struct TextboxMutation {
    slide: u32,
    slide_part: String,
    shape_id: u32,
    shape_name: String,
    updated_slide_xml: String,
}

struct ImageMutation {
    slide: u32,
    slide_part: String,
    shape_id: u32,
    shape_name: String,
    target_uri: String,
    content_type: String,
    relationship_id: String,
    fit_mode: String,
    updated_slide_xml: String,
    updated_rels_xml: String,
    updated_content_types_xml: String,
    image_data: Vec<u8>,
}

pub(crate) fn pptx_add_textbox(file: &str, args: &[String]) -> CliResult<Value> {
    ensure_pptx(file)?;
    let request = parse_add_textbox_request(args)?;
    let options = parse_placement_mutation_options(args)?;
    let mutation = build_textbox_mutation(file, &request)?;
    let mut text_overrides = BTreeMap::new();
    text_overrides.insert(
        mutation.slide_part.clone(),
        mutation.updated_slide_xml.clone(),
    );
    let stage = stage_placement_mutation(
        file,
        &text_overrides,
        &BTreeMap::new(),
        &options,
        "pptx-add-textbox",
    )?;
    let destination = read_shape_destination(
        &stage.staged_path,
        request.slide,
        mutation.shape_id,
        stage.output_path.as_deref(),
        true,
    )?;
    let result = add_textbox_result_json(
        file,
        &mutation,
        &options,
        stage.output_path.as_deref(),
        destination,
    );
    finish_placement_mutation(
        file,
        &stage.staged_path,
        &options,
        stage.output_path.as_deref(),
    )?;
    Ok(result)
}

pub(crate) fn pptx_place_image(file: &str, args: &[String]) -> CliResult<Value> {
    ensure_pptx(file)?;
    let request = parse_place_image_request(args)?;
    let options = parse_placement_mutation_options(args)?;
    let mutation = build_image_mutation(file, &request)?;
    let mut text_overrides = BTreeMap::new();
    text_overrides.insert(
        mutation.slide_part.clone(),
        mutation.updated_slide_xml.clone(),
    );
    text_overrides.insert(
        relationships_part_for(&mutation.slide_part),
        mutation.updated_rels_xml.clone(),
    );
    text_overrides.insert(
        "[Content_Types].xml".to_string(),
        mutation.updated_content_types_xml.clone(),
    );
    let mut binary_overrides = BTreeMap::new();
    binary_overrides.insert(
        mutation.target_uri.trim_start_matches('/').to_string(),
        mutation.image_data.clone(),
    );
    let stage = stage_placement_mutation(
        file,
        &text_overrides,
        &binary_overrides,
        &options,
        "pptx-place-image",
    )?;
    let destination = read_shape_destination(
        &stage.staged_path,
        request.slide,
        mutation.shape_id,
        stage.output_path.as_deref(),
        false,
    )?;
    let result = place_image_result_json(
        file,
        &mutation,
        &request,
        &options,
        stage.output_path.as_deref(),
        destination,
    );
    finish_placement_mutation(
        file,
        &stage.staged_path,
        &options,
        stage.output_path.as_deref(),
    )?;
    Ok(result)
}

fn parse_add_textbox_request(args: &[String]) -> CliResult<TextboxRequest> {
    require_value_flags(args, &["--slide", "--text", "--cx", "--cy"])?;
    let slide = parse_i64_flag(args, "--slide")?.unwrap_or(0);
    if slide < 1 {
        return Err(CliError::invalid_args("--slide must be >= 1"));
    }
    let text = parse_string_flag(args, "--text")?.unwrap_or_default();
    if text.is_empty() {
        return Err(CliError::invalid_args("--text is required"));
    }
    let bounds = parse_required_bounds(args, true)?;
    let font_size = parse_f64_flag(args, "--font-size")?.unwrap_or(18.0);
    let font_family = parse_string_flag(args, "--font")?
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "Calibri".to_string());
    Ok(TextboxRequest {
        slide: slide as u32,
        text,
        bounds,
        name: parse_string_flag(args, "--name")?.unwrap_or_default(),
        font_size,
        font_family,
        bold: crate::has_flag(args, "--bold"),
        italic: crate::has_flag(args, "--italic"),
        color: parse_string_flag(args, "--color")?.unwrap_or_default(),
        level: parse_i64_flag(args, "--level")?.unwrap_or(0),
        align: parse_string_flag(args, "--align")?.unwrap_or_default(),
    })
}

fn parse_place_image_request(args: &[String]) -> CliResult<ImageRequest> {
    require_value_flags(args, &["--slide", "--image", "--x", "--y", "--cx", "--cy"])?;
    let slide = parse_i64_flag(args, "--slide")?.unwrap_or(0);
    if slide < 1 {
        return Err(CliError::invalid_args("--slide must be >= 1"));
    }
    let image_path = parse_string_flag(args, "--image")?.unwrap_or_default();
    if image_path.trim().is_empty() {
        return Err(CliError::invalid_args("--image must be specified"));
    }
    if !Path::new(&image_path).exists() {
        return Err(CliError::file_not_found(format!(
            "file not found: {image_path}"
        )));
    }
    let bounds = parse_required_bounds(args, false)?;
    let fit_mode = normalize_fit_mode(
        parse_string_flag(args, "--fit-mode")?
            .as_deref()
            .unwrap_or("contain"),
    )?;
    Ok(ImageRequest {
        slide: slide as u32,
        image_path,
        bounds,
        name: parse_string_flag(args, "--name")?.unwrap_or_default(),
        fit_mode,
    })
}

fn parse_required_bounds(args: &[String], textbox: bool) -> CliResult<Bounds> {
    let x = parse_i64_flag(args, "--x")?.unwrap_or(0);
    let y = parse_i64_flag(args, "--y")?.unwrap_or(0);
    let cx = parse_i64_flag(args, "--cx")?.unwrap_or(0);
    let cy = parse_i64_flag(args, "--cy")?.unwrap_or(0);
    if textbox {
        if cx <= 0 || cy <= 0 {
            return Err(CliError::invalid_args("--cx and --cy must be positive"));
        }
    } else if cx <= 0 || cy <= 0 {
        return Err(CliError::invalid_args(format!(
            "dimensions must be positive: cx={cx}, cy={cy}"
        )));
    }
    Ok(Bounds { x, y, cx, cy })
}

fn require_value_flags(args: &[String], flags: &[&str]) -> CliResult<()> {
    let missing = flags
        .iter()
        .filter(|flag| !value_flag_present(args, flag))
        .map(|flag| format!(r#""{}""#, flag.trim_start_matches("--")))
        .collect::<Vec<_>>();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(CliError::invalid_args(format!(
            "required flag(s) {} not set",
            missing.join(", ")
        )))
    }
}

fn parse_f64_flag(args: &[String], name: &str) -> CliResult<Option<f64>> {
    parse_string_flag(args, name)?
        .map(|value| {
            value
                .parse::<f64>()
                .map_err(|_| CliError::invalid_args(format!("{name} must be a number")))
        })
        .transpose()
}

fn normalize_fit_mode(value: &str) -> CliResult<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "contain" | "fit" => Ok("contain".to_string()),
        "cover" | "crop" => Ok("cover".to_string()),
        other => Err(CliError::invalid_args(format!(
            "invalid fit mode {other:?} (must be 'contain' or 'cover')"
        ))),
    }
}

fn parse_placement_mutation_options(args: &[String]) -> CliResult<PlacementMutationOptions> {
    let out = parse_string_flag(args, "--out")?;
    let backup = parse_string_flag(args, "--backup")?;
    let dry_run = crate::has_flag(args, "--dry-run");
    let in_place = crate::has_flag(args, "--in-place");
    let no_validate = crate::has_flag(args, "--no-validate");
    validate_xlsx_mutation_output_flags(out.as_deref(), in_place, backup.as_deref(), dry_run)?;
    Ok(PlacementMutationOptions {
        out,
        backup,
        dry_run,
        in_place,
        no_validate,
    })
}

fn build_textbox_mutation(file: &str, request: &TextboxRequest) -> CliResult<TextboxMutation> {
    let slide_part = slide_part_for_number(file, request.slide)?;
    let slide_xml = zip_text(file, &slide_part)?;
    let shape_id = next_shape_id(&slide_xml)?;
    let shape_name = if request.name.is_empty() {
        format!("TextBox {shape_id}")
    } else {
        request.name.clone()
    };
    let shape_xml = textbox_shape_xml(shape_id, &shape_name, request);
    let updated_slide_xml = insert_shape_into_sp_tree(&slide_xml, &shape_xml)?;
    Ok(TextboxMutation {
        slide: request.slide,
        slide_part,
        shape_id,
        shape_name,
        updated_slide_xml,
    })
}

fn build_image_mutation(file: &str, request: &ImageRequest) -> CliResult<ImageMutation> {
    let slide_part = slide_part_for_number(file, request.slide)?;
    let slide_xml = zip_text(file, &slide_part)?;
    let shape_id = next_shape_id(&slide_xml)?;
    let image_data = fs::read(&request.image_path)
        .map_err(|err| CliError::unexpected(format!("failed to read image file: {err}")))?;
    let content_type = image_content_type(&request.image_path)?;
    validate_image_payload(&content_type, &image_data)?;
    let extension = image_extension_for_content_type(&content_type)?;
    let target_uri = allocate_image_part(file, shape_id, extension)?;
    let rels_part = relationships_part_for(&slide_part);
    let rels_xml = zip_text(file, &rels_part).unwrap_or_else(|_| {
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"></Relationships>"#
            .to_string()
    });
    let rels = relationship_entries_from_xml(&rels_xml);
    let relationship_id = allocate_relationship_id(&rels);
    let rel_target =
        relationship_target_from_source_to_target(&format!("/{slide_part}"), &target_uri);
    let updated_rels_xml =
        add_relationship_to_xml(rels_xml, &relationship_id, REL_TYPE_IMAGE, &rel_target);
    let content_types = zip_text(file, "[Content_Types].xml")?;
    let updated_content_types_xml =
        ensure_content_type_override(content_types, &target_uri, &content_type);
    let shape_name = if request.name.is_empty() {
        Path::new(&target_uri)
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("image")
            .to_string()
    } else {
        request.name.clone()
    };
    let pic_xml = picture_shape_xml(shape_id, &shape_name, &relationship_id, request);
    let slide_xml = ensure_root_namespace(slide_xml, "r", R_NS)?;
    let updated_slide_xml = insert_shape_into_sp_tree(&slide_xml, &pic_xml)?;
    Ok(ImageMutation {
        slide: request.slide,
        slide_part,
        shape_id,
        shape_name,
        target_uri,
        content_type,
        relationship_id,
        fit_mode: request.fit_mode.clone(),
        updated_slide_xml,
        updated_rels_xml,
        updated_content_types_xml,
        image_data,
    })
}

fn slide_part_for_number(file: &str, slide: u32) -> CliResult<String> {
    let show = pptx_slide_show(file, slide)?;
    show.get("slides")
        .and_then(Value::as_array)
        .and_then(|slides| slides.first())
        .and_then(|entry| entry.get("partUri"))
        .and_then(Value::as_str)
        .map(|part| part.trim_start_matches('/').to_string())
        .ok_or_else(|| CliError::unexpected("slide readback missing partUri"))
}

fn next_shape_id(slide_xml: &str) -> CliResult<u32> {
    let sp_tree = find_first_element_span(slide_xml, "spTree")?
        .ok_or_else(|| CliError::unexpected("shape tree not found in slide"))?;
    let fragment = &slide_xml[sp_tree.start..sp_tree.end];
    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(true);
    let mut max_id = 0_u32;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "cNvPr" =>
            {
                if let Some(id) = attr(&e, "id").and_then(|value| value.trim().parse::<u32>().ok())
                {
                    max_id = max_id.max(id);
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(max_id + 1)
}

fn insert_shape_into_sp_tree(slide_xml: &str, shape_xml: &str) -> CliResult<String> {
    let sp_tree = find_first_element_span(slide_xml, "spTree")?
        .ok_or_else(|| CliError::unexpected("shape tree not found in slide"))?;
    let (content_start, content_end) =
        element_content_bounds(&slide_xml[sp_tree.start..sp_tree.end])?;
    let children = xml_direct_child_ranges(
        slide_xml,
        sp_tree.start + content_start,
        sp_tree.start + content_end,
    )?;
    let insert_at = children
        .iter()
        .find(|child| child.kind == "extLst")
        .map(|child| child.start)
        .unwrap_or(sp_tree.start + content_end);
    Ok(insert_xml_at(slide_xml, insert_at, shape_xml))
}

fn textbox_shape_xml(shape_id: u32, shape_name: &str, request: &TextboxRequest) -> String {
    let mut paragraph = String::new();
    paragraph.push_str("<a:p>");
    if request.level > 0 || !request.align.is_empty() {
        paragraph.push_str("<a:pPr");
        if !request.align.is_empty() {
            paragraph.push_str(&format!(r#" algn="{}""#, xml_attr_escape(&request.align)));
        }
        if request.level > 0 {
            paragraph.push_str(&format!(r#" lvl="{}""#, request.level));
        }
        paragraph.push_str("/>");
    }
    paragraph.push_str("<a:r>");
    paragraph.push_str(&run_properties_xml(
        request.font_size,
        &request.font_family,
        request.bold,
        request.italic,
        &request.color,
    ));
    paragraph.push_str(&text_element_xml(&request.text));
    paragraph.push_str("</a:r><a:endParaRPr lang=\"en-US\" sz=\"1800\"/></a:p>");
    format!(
        r#"<p:sp><p:nvSpPr><p:cNvPr id="{shape_id}" name="{}"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="{}" y="{}"/><a:ext cx="{}" cy="{}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr><p:txBody><a:bodyPr anchor="t" anchorCtr="false" wrap="square" rtlCol="false"/><a:lstStyle/>{paragraph}</p:txBody></p:sp>"#,
        xml_attr_escape(shape_name),
        request.bounds.x,
        request.bounds.y,
        request.bounds.cx,
        request.bounds.cy
    )
}

fn run_properties_xml(
    font_size: f64,
    font_family: &str,
    bold: bool,
    italic: bool,
    color: &str,
) -> String {
    let size = (font_size * 100.0) as i64;
    let mut xml = format!(r#"<a:rPr lang="en-US" sz="{size}""#);
    if bold {
        xml.push_str(r#" b="1""#);
    }
    if italic {
        xml.push_str(r#" i="1""#);
    }
    xml.push('>');
    if !color.is_empty() {
        xml.push_str(&format!(
            r#"<a:solidFill><a:srgbClr val="{}"/></a:solidFill>"#,
            xml_attr_escape(color)
        ));
    }
    if !font_family.is_empty() {
        xml.push_str(&format!(
            r#"<a:latin typeface="{}"/>"#,
            xml_attr_escape(font_family)
        ));
    }
    xml.push_str("</a:rPr>");
    xml
}

fn text_element_xml(text: &str) -> String {
    if needs_xml_space_preserve(text) {
        format!(r#"<a:t xml:space="preserve">{}</a:t>"#, xml_escape(text))
    } else {
        format!("<a:t>{}</a:t>", xml_escape(text))
    }
}

fn picture_shape_xml(
    shape_id: u32,
    shape_name: &str,
    rel_id: &str,
    request: &ImageRequest,
) -> String {
    let fit = if request.fit_mode == "cover" {
        r#"<a:tile tx="0" ty="0" sx="100000" sy="100000" flip="none" algn="ctr"/>"#
    } else {
        "<a:stretch><a:fillRect/></a:stretch>"
    };
    format!(
        r#"<p:pic><p:nvPicPr><p:cNvPr id="{shape_id}" name="{}"/><p:cNvPicPr/><p:nvPr/></p:nvPicPr><p:blipFill><a:blip r:embed="{}"/>{fit}</p:blipFill><p:spPr><a:xfrm><a:off x="{}" y="{}"/><a:ext cx="{}" cy="{}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr></p:pic>"#,
        xml_attr_escape(shape_name),
        xml_attr_escape(rel_id),
        request.bounds.x,
        request.bounds.y,
        request.bounds.cx,
        request.bounds.cy
    )
}

fn image_content_type(path: &str) -> CliResult<String> {
    let ext = Path::new(path)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match ext.as_str() {
        "png" => Ok("image/png".to_string()),
        "jpg" | "jpeg" => Ok("image/jpeg".to_string()),
        "gif" => Ok("image/gif".to_string()),
        "bmp" => Ok("image/bmp".to_string()),
        "tif" | "tiff" => Ok("image/tiff".to_string()),
        _ => Err(CliError::invalid_args(format!(
            "unsupported image content type for {path:?}"
        ))),
    }
}

fn image_extension_for_content_type(content_type: &str) -> CliResult<&'static str> {
    match content_type {
        "image/png" => Ok(".png"),
        "image/jpeg" => Ok(".jpeg"),
        "image/gif" => Ok(".gif"),
        "image/bmp" => Ok(".bmp"),
        "image/tiff" => Ok(".tiff"),
        _ => Err(CliError::invalid_args(format!(
            "unsupported image content type {content_type:?}"
        ))),
    }
}

fn validate_image_payload(content_type: &str, data: &[u8]) -> CliResult<()> {
    let ok = match content_type {
        "image/png" => data.starts_with(b"\x89PNG\r\n\x1a\n"),
        "image/jpeg" => data.starts_with(&[0xff, 0xd8, 0xff]),
        "image/gif" => data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a"),
        _ => true,
    };
    if ok {
        Ok(())
    } else {
        Err(CliError::invalid_args(format!(
            "image payload does not match content type {content_type}"
        )))
    }
}

fn allocate_image_part(file: &str, shape_id: u32, extension: &str) -> CliResult<String> {
    let entries = zip_entry_names(file)?;
    let base = format!("/ppt/media/image{shape_id}");
    let mut candidate = format!("{base}{extension}");
    let mut counter = 1_u32;
    while entries
        .iter()
        .any(|entry| format!("/{}", entry.trim_start_matches('/')) == candidate)
    {
        candidate = format!("{base}_{counter}{extension}");
        counter += 1;
    }
    Ok(candidate)
}

fn ensure_root_namespace(xml: String, prefix: &str, uri: &str) -> CliResult<String> {
    if xml.contains(&format!("xmlns:{prefix}=")) {
        return Ok(xml);
    }
    let root = find_first_element_span(&xml, "sld")?
        .ok_or_else(|| CliError::unexpected("slide root not found"))?;
    let open_end = xml[root.start..root.end]
        .find('>')
        .ok_or_else(|| CliError::unexpected("invalid slide root XML"))?
        + root.start;
    let insert = format!(r#" xmlns:{prefix}="{}""#, xml_attr_escape(uri));
    Ok(insert_xml_at(&xml, open_end, &insert))
}

fn stage_placement_mutation(
    file: &str,
    text_overrides: &BTreeMap<String, String>,
    binary_overrides: &BTreeMap<String, Vec<u8>>,
    options: &PlacementMutationOptions,
    label: &str,
) -> CliResult<PlacementStage> {
    let output_path = placement_output_path(file, options);
    let requested_out = options
        .out
        .as_deref()
        .filter(|value| !value.trim().is_empty());
    let staged_path = if options.dry_run || options.in_place || requested_out == Some(file) {
        package_mutation_temp_path(file, label)
    } else {
        requested_out
            .ok_or_else(|| {
                CliError::invalid_args(
                    "must specify exactly one of --out, --in-place, or --dry-run",
                )
            })?
            .to_string()
    };
    if binary_overrides.is_empty() {
        copy_zip_with_part_overrides(file, &staged_path, text_overrides)?;
    } else {
        copy_zip_with_binary_part_overrides_and_removals(
            file,
            &staged_path,
            text_overrides,
            binary_overrides,
            &BTreeSet::new(),
        )?;
    }
    if !options.no_validate {
        validate(&staged_path, true)?;
    }
    Ok(PlacementStage {
        staged_path,
        output_path,
    })
}

fn finish_placement_mutation(
    file: &str,
    staged_path: &str,
    options: &PlacementMutationOptions,
    output_path: Option<&str>,
) -> CliResult<()> {
    if options.dry_run {
        let _ = fs::remove_file(staged_path);
    } else if options.in_place || output_path == Some(file) {
        if let Some(backup_path) = options
            .backup
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            fs::copy(file, backup_path)
                .map_err(|err| CliError::unexpected(format!("failed to create backup: {err}")))?;
        }
        fs::rename(staged_path, file)
            .or_else(|_| {
                fs::copy(staged_path, file)?;
                fs::remove_file(staged_path)
            })
            .map_err(|err| CliError::unexpected(format!("failed to write output file: {err}")))?;
    }
    Ok(())
}

fn placement_output_path(file: &str, options: &PlacementMutationOptions) -> Option<String> {
    if options.dry_run {
        None
    } else if options.in_place {
        Some(file.to_string())
    } else {
        options
            .out
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(ToString::to_string)
    }
}

fn read_shape_destination(
    readback_path: &str,
    slide: u32,
    shape_id: u32,
    output_path: Option<&str>,
    include_text: bool,
) -> CliResult<Value> {
    let target = format!("shape:{shape_id}");
    let get = pptx_shapes_get(readback_path, slide, &target, include_text, true)?;
    let entry = get
        .get("shapes")
        .and_then(Value::as_array)
        .and_then(|shapes| shapes.first())
        .ok_or_else(|| CliError::unexpected("shape readback missing destination"))?;
    Ok(shape_destination_from_entry(
        entry,
        slide,
        output_path,
        &target,
    ))
}

fn shape_destination_from_entry(
    entry: &Value,
    slide: u32,
    file: Option<&str>,
    target: &str,
) -> Value {
    let mut out = Map::new();
    if let Some(file) = file {
        out.insert("file".to_string(), json!(file));
    }
    out.insert("slide".to_string(), json!(slide));
    out.insert("target".to_string(), json!(target));
    copy_json_field(entry, &mut out, "shapeId");
    copy_json_field(entry, &mut out, "shapeName");
    copy_json_field(entry, &mut out, "targetKind");
    copy_json_field(entry, &mut out, "primarySelector");
    copy_json_field(entry, &mut out, "handle");
    copy_json_field(entry, &mut out, "selectors");
    copy_json_field(entry, &mut out, "textPreview");
    copy_json_field(entry, &mut out, "bounds");
    copy_json_field(entry, &mut out, "imageRef");
    Value::Object(out)
}

fn copy_json_field(source: &Value, dest: &mut Map<String, Value>, key: &str) {
    if let Some(value) = source.get(key) {
        dest.insert(key.to_string(), value.clone());
    }
}

fn add_textbox_result_json(
    file: &str,
    mutation: &TextboxMutation,
    options: &PlacementMutationOptions,
    output_path: Option<&str>,
    destination: Value,
) -> Value {
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    if let Some(output_path) = output_path {
        result.insert("output".to_string(), json!(output_path));
    }
    result.insert("dryRun".to_string(), json!(options.dry_run));
    result.insert("shapeId".to_string(), json!(mutation.shape_id));
    result.insert("shapeName".to_string(), json!(mutation.shape_name));
    result.insert("createdAt".to_string(), json!(crate::current_utc_rfc3339()));
    result.insert("destination".to_string(), destination);
    add_shape_readback_commands(
        &mut result,
        output_path,
        options.dry_run,
        mutation.slide,
        mutation.shape_id,
        true,
    );
    Value::Object(result)
}

fn place_image_result_json(
    file: &str,
    mutation: &ImageMutation,
    request: &ImageRequest,
    options: &PlacementMutationOptions,
    output_path: Option<&str>,
    destination: Value,
) -> Value {
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    if let Some(output_path) = output_path {
        result.insert("output".to_string(), json!(output_path));
    }
    result.insert("dryRun".to_string(), json!(options.dry_run));
    result.insert("slideNumber".to_string(), json!(mutation.slide));
    result.insert("shapeId".to_string(), json!(mutation.shape_id));
    result.insert("shapeName".to_string(), json!(mutation.shape_name));
    result.insert("targetUri".to_string(), json!(mutation.target_uri));
    result.insert("contentType".to_string(), json!(mutation.content_type));
    result.insert(
        "relationshipId".to_string(),
        json!(mutation.relationship_id),
    );
    result.insert("x".to_string(), json!(request.bounds.x));
    result.insert("y".to_string(), json!(request.bounds.y));
    result.insert("cx".to_string(), json!(request.bounds.cx));
    result.insert("cy".to_string(), json!(request.bounds.cy));
    result.insert("fitMode".to_string(), json!(mutation.fit_mode));
    result.insert("destination".to_string(), destination);
    add_shape_readback_commands(
        &mut result,
        output_path,
        options.dry_run,
        mutation.slide,
        mutation.shape_id,
        false,
    );
    Value::Object(result)
}

fn add_shape_readback_commands(
    result: &mut Map<String, Value>,
    output_path: Option<&str>,
    dry_run: bool,
    slide: u32,
    shape_id: u32,
    include_text: bool,
) {
    let target_file = output_path.unwrap_or("<out.pptx>");
    let suffix = if dry_run { "Template" } else { "" };
    let mut readback = format!(
        "ooxml --json pptx shapes get {} --slide {slide} --target shape:{shape_id}",
        command_arg(target_file)
    );
    if include_text {
        readback.push_str(" --include-text");
    }
    readback.push_str(" --include-bounds");
    result.insert(format!("readbackCommand{suffix}"), json!(readback));
    result.insert(
        format!("slideReadbackCommand{suffix}"),
        json!(format!(
            "ooxml --json pptx slides show {} --slide {slide} --include-text --include-bounds",
            command_arg(target_file)
        )),
    );
    result.insert(
        format!("validateCommand{suffix}"),
        json!(format!(
            "ooxml validate --strict {}",
            command_arg(target_file)
        )),
    );
    result.insert(
        format!("renderCommand{suffix}"),
        json!(format!(
            "ooxml pptx render {} --out render-check",
            command_arg(target_file)
        )),
    );
}

fn ensure_pptx(file: &str) -> CliResult<()> {
    let detected = package_type(file)?;
    if detected != "pptx" {
        return Err(CliError::unsupported_type(format!(
            "file is not a PPTX document (detected: {detected})"
        )));
    }
    Ok(())
}

fn find_first_element_span(xml: &str, wanted_local: &str) -> CliResult<Option<XmlSpan>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut active: Option<(usize, usize)> = None;
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                if let Some((_, depth)) = active.as_mut() {
                    *depth += 1;
                } else if local_name(e.name().as_ref()) == wanted_local {
                    active = Some((before, 1));
                }
            }
            Ok(Event::Empty(e)) => {
                if active.is_none() && local_name(e.name().as_ref()) == wanted_local {
                    return Ok(Some(XmlSpan {
                        start: before,
                        end: reader.buffer_position() as usize,
                    }));
                }
            }
            Ok(Event::End(e)) => {
                if let Some((start, depth)) = active.as_mut() {
                    if *depth == 1 && local_name(e.name().as_ref()) == wanted_local {
                        return Ok(Some(XmlSpan {
                            start: *start,
                            end: reader.buffer_position() as usize,
                        }));
                    }
                    *depth = depth.saturating_sub(1);
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(None)
}

fn element_content_bounds(fragment: &str) -> CliResult<(usize, usize)> {
    let open_end = fragment
        .find('>')
        .ok_or_else(|| CliError::unexpected("invalid PPTX XML"))?;
    if fragment[..=open_end].trim_end().ends_with("/>") {
        return Ok((open_end + 1, open_end + 1));
    }
    let close_start = fragment
        .rfind("</")
        .ok_or_else(|| CliError::unexpected("invalid PPTX XML"))?;
    Ok((open_end + 1, close_start))
}

fn insert_xml_at(xml: &str, index: usize, insert: &str) -> String {
    let mut out = String::with_capacity(xml.len() + insert.len());
    out.push_str(&xml[..index]);
    out.push_str(insert);
    out.push_str(&xml[index..]);
    out
}
