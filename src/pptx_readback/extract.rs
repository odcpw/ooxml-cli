use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use super::slide_layout_and_notes_parts;
use super::slide_parts::{PptxSlidePartRef, pptx_slide_part_refs};
use crate::{
    CliError, CliResult, attr, content_type_for_part, has_flag, local_name, package_type,
    parse_string_flag, relationship_entries, relationships, relationships_part_for,
    resolve_relationship_target, zip_bytes, zip_text,
};

pub(crate) fn pptx_extract_images(file: &str, args: &[String]) -> CliResult<Value> {
    ensure_pptx(file)?;

    let slide_filter = parse_last_i64_flag(args, "--slide")?.unwrap_or(0);
    let out_dir = parse_string_flag(args, "--out")?
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| ".".to_string());
    let include_layout = has_flag(args, "--include-layout-images");

    fs::create_dir_all(&out_dir)
        .map_err(|err| CliError::unexpected(format!("failed to create output directory: {err}")))?;

    let slides = pptx_slide_part_refs(file)?;
    let slides_to_process = selected_image_slides(&slides, slide_filter)?;
    let mut images = Vec::new();
    for slide in slides_to_process {
        images.extend(extract_images_from_part(
            file,
            &format!("/{}", slide.part.trim_start_matches('/')),
            false,
        ));
        if include_layout
            && let Ok((Some(layout_part), _)) = slide_layout_and_notes_parts(file, &slide.part)
        {
            images.extend(extract_images_from_part(
                file,
                &format!("/{}", layout_part.trim_start_matches('/')),
                true,
            ));
        }
    }

    write_extracted_images(file, &out_dir, &mut images)?;

    let mut output = Map::new();
    output.insert("file".to_string(), json!(file));
    if slide_filter > 0 {
        output.insert("slideNumber".to_string(), json!(slide_filter));
    }
    output.insert("outputDirectory".to_string(), json!(out_dir));
    output.insert("includeLayout".to_string(), json!(include_layout));
    output.insert("imagesCount".to_string(), json!(images.len()));
    output.insert(
        "images".to_string(),
        if images.is_empty() {
            Value::Null
        } else {
            Value::Array(images.into_iter().map(ExtractedImage::into_json).collect())
        },
    );
    Ok(Value::Object(output))
}

pub(crate) fn pptx_extract_xml(file: &str, args: &[String]) -> CliResult<Value> {
    ensure_pptx(file)?;

    let out_dir = parse_string_flag(args, "--out")?
        .filter(|value| !value.is_empty())
        .ok_or_else(|| CliError::invalid_args("output directory required (--out)"))?;
    fs::create_dir_all(&out_dir)
        .map_err(|err| CliError::unexpected(format!("failed to create output directory: {err}")))?;

    let mut items = Vec::new();
    let slides = pptx_slide_part_refs(file)?;
    for number in parse_i64_flags(args, "--slide")? {
        if number < 1 || number as usize > slides.len() {
            return Err(CliError::invalid_args(format!(
                "slide number {number} is out of range (1-{})",
                slides.len()
            )));
        }
        let slide = &slides[number as usize - 1];
        items.push(ExtractXmlItem {
            item_type: "slide".to_string(),
            number,
            part_uri: format!("/{}", slide.part.trim_start_matches('/')),
        });
    }

    let graph = pptx_extract_xml_graph(file)?;
    for number in parse_i64_flags(args, "--layout")? {
        if number < 1 || number as usize > graph.layouts.len() {
            return Err(CliError::invalid_args(format!(
                "layout number {number} is out of range (1-{})",
                graph.layouts.len()
            )));
        }
        items.push(ExtractXmlItem {
            item_type: "layout".to_string(),
            number,
            part_uri: graph.layouts[number as usize - 1].clone(),
        });
    }
    for number in parse_i64_flags(args, "--master")? {
        if number < 1 || number as usize > graph.masters.len() {
            return Err(CliError::invalid_args(format!(
                "master number {number} is out of range (1-{})",
                graph.masters.len()
            )));
        }
        items.push(ExtractXmlItem {
            item_type: "master".to_string(),
            number,
            part_uri: graph.masters[number as usize - 1].part_uri.clone(),
        });
    }

    if items.is_empty() {
        for (index, slide) in slides.iter().enumerate() {
            items.push(ExtractXmlItem {
                item_type: "slide".to_string(),
                number: index as i64 + 1,
                part_uri: format!("/{}", slide.part.trim_start_matches('/')),
            });
        }
    }

    let mut extracted = Vec::new();
    for item in &items {
        extract_xml_item(file, item, &out_dir).map_err(|err| {
            CliError::unexpected(format!(
                "failed to extract {} {}: {}",
                item.item_type, item.number, err.message
            ))
        })?;
        extracted.push(format!("{}-{}", item.item_type, item.number));
    }

    Ok(json!({
        "extracted": if extracted.is_empty() { Value::Null } else { json!(extracted) },
        "file": file,
        "output_dir": out_dir,
    }))
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

fn selected_image_slides(
    slides: &[PptxSlidePartRef],
    slide_filter: i64,
) -> CliResult<Vec<&PptxSlidePartRef>> {
    if slide_filter > 0 {
        if slide_filter as usize > slides.len() {
            return Err(CliError::invalid_args(format!(
                "slide number {slide_filter} is out of range (1-{})",
                slides.len()
            )));
        }
        Ok(vec![&slides[slide_filter as usize - 1]])
    } else {
        Ok(slides.iter().collect())
    }
}

#[derive(Clone)]
struct ExtractedImage {
    source_part_uri: String,
    shape_id: i64,
    shape_name: String,
    relationship_id: String,
    target_uri: String,
    content_type: String,
    file_path: String,
    file_size: i64,
    geometry: Option<Value>,
    is_layout_image: bool,
}

impl ExtractedImage {
    fn into_json(self) -> Value {
        let mut image = Map::new();
        image.insert("sourcePartUri".to_string(), json!(self.source_part_uri));
        image.insert("shapeId".to_string(), json!(self.shape_id));
        image.insert("shapeName".to_string(), json!(self.shape_name));
        image.insert("relationshipId".to_string(), json!(self.relationship_id));
        image.insert("targetUri".to_string(), json!(self.target_uri));
        image.insert("contentType".to_string(), json!(self.content_type));
        image.insert("filePath".to_string(), json!(self.file_path));
        image.insert("fileSize".to_string(), json!(self.file_size));
        if let Some(geometry) = self.geometry {
            image.insert("geometry".to_string(), geometry);
        }
        if self.is_layout_image {
            image.insert("isLayoutImage".to_string(), json!(true));
        }
        Value::Object(image)
    }
}

fn extract_images_from_part(
    file: &str,
    source_part_uri: &str,
    is_layout: bool,
) -> Vec<ExtractedImage> {
    let Ok(xml) = zip_text(file, source_part_uri.trim_start_matches('/')) else {
        return Vec::new();
    };
    let rels = relationship_entries(file, &relationships_part_for(source_part_uri))
        .unwrap_or_default()
        .into_iter()
        .map(|rel| (rel.id.clone(), rel))
        .collect::<BTreeMap<_, _>>();

    pptx_picture_models(&xml)
        .into_iter()
        .filter_map(|picture| {
            if picture.relationship_id.is_empty() {
                return None;
            }
            let rel = rels.get(&picture.relationship_id)?;
            let target_uri = resolve_relationship_target(source_part_uri, &rel.target);
            let data = zip_bytes(file, target_uri.trim_start_matches('/')).ok()?;
            let content_type = content_type_for_part(file, &target_uri).unwrap_or_default();
            let filename = package_base_name(&target_uri).to_string();
            let geometry = picture.geometry_json();
            Some(ExtractedImage {
                source_part_uri: source_part_uri.to_string(),
                shape_id: picture.shape_id,
                shape_name: picture.shape_name,
                relationship_id: picture.relationship_id,
                target_uri,
                content_type,
                file_path: filename,
                file_size: data.len() as i64,
                geometry,
                is_layout_image: is_layout,
            })
        })
        .collect()
}

fn write_extracted_images(
    file: &str,
    out_dir: &str,
    images: &mut [ExtractedImage],
) -> CliResult<()> {
    let mut file_counter = BTreeMap::<String, usize>::new();
    for image in images {
        let Ok(data) = zip_bytes(file, image.target_uri.trim_start_matches('/')) else {
            continue;
        };

        let mut filename = package_base_name(&image.target_uri).to_string();
        if let Some(count) = file_counter.get(&filename).copied() {
            let extension = Path::new(&filename)
                .extension()
                .and_then(|value| value.to_str())
                .map(|value| format!(".{value}"))
                .unwrap_or_default();
            let base = filename
                .strip_suffix(&extension)
                .unwrap_or(&filename)
                .to_string();
            filename = format!("{base}_{count}{extension}");
            file_counter.insert(filename.clone(), count + 1);
        } else {
            file_counter.insert(filename.clone(), 1);
        }

        let out_path = Path::new(out_dir).join(&filename);
        fs::write(&out_path, data)
            .map_err(|err| CliError::unexpected(format!("failed to write image file: {err}")))?;
        image.file_path = filename;
    }
    Ok(())
}

#[derive(Default)]
struct PictureModel {
    shape_id: i64,
    shape_name: String,
    relationship_id: String,
    saw_xfrm: bool,
    rotation: i64,
    flip_h: bool,
    flip_v: bool,
    crop: Option<CropInfo>,
}

impl PictureModel {
    fn geometry_json(&self) -> Option<Value> {
        if !self.saw_xfrm
            || (self.rotation == 0 && !self.flip_h && !self.flip_v && self.crop.is_none())
        {
            return None;
        }
        let mut geometry = Map::new();
        if self.rotation != 0 {
            geometry.insert("rotation".to_string(), json!(self.rotation));
        }
        if self.flip_h {
            geometry.insert("flipH".to_string(), json!(true));
        }
        if self.flip_v {
            geometry.insert("flipV".to_string(), json!(true));
        }
        if let Some(crop) = self.crop.as_ref() {
            geometry.insert("crop".to_string(), crop.to_json());
        }
        Some(Value::Object(geometry))
    }
}

#[derive(Default)]
struct CropInfo {
    left: i64,
    top: i64,
    right: i64,
    bottom: i64,
}

impl CropInfo {
    fn to_json(&self) -> Value {
        let mut crop = Map::new();
        if self.left != 0 {
            crop.insert("left".to_string(), json!(self.left));
        }
        if self.top != 0 {
            crop.insert("top".to_string(), json!(self.top));
        }
        if self.right != 0 {
            crop.insert("right".to_string(), json!(self.right));
        }
        if self.bottom != 0 {
            crop.insert("bottom".to_string(), json!(self.bottom));
        }
        Value::Object(crop)
    }
}

struct PictureBuilder {
    depth: usize,
    model: PictureModel,
    in_blip_fill: bool,
    in_sp_pr: bool,
}

fn pptx_picture_models(xml: &str) -> Vec<PictureModel> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut path = Vec::<String>::new();
    let mut current: Option<PictureBuilder> = None;
    let mut pictures = Vec::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if current.is_none()
                    && path.last().map(String::as_str) == Some("spTree")
                    && name == "pic"
                {
                    current = Some(PictureBuilder {
                        depth: path.len() + 1,
                        model: PictureModel::default(),
                        in_blip_fill: false,
                        in_sp_pr: false,
                    });
                } else if let Some(picture) = current.as_mut() {
                    visit_picture_element(picture, &name, &e);
                }
                path.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if let Some(picture) = current.as_mut() {
                    visit_picture_element(picture, &name, &e);
                }
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if let Some(mut picture) = current.take() {
                    if path.len() == picture.depth && name == "pic" {
                        if !picture.model.relationship_id.is_empty() {
                            pictures.push(picture.model);
                        }
                    } else {
                        if name == "blipFill" {
                            picture.in_blip_fill = false;
                        }
                        if name == "spPr" {
                            picture.in_sp_pr = false;
                        }
                        current = Some(picture);
                    }
                }
                path.pop();
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }

    pictures
}

fn visit_picture_element(picture: &mut PictureBuilder, name: &str, e: &BytesStart<'_>) {
    match name {
        "blipFill" => picture.in_blip_fill = true,
        "spPr" => picture.in_sp_pr = true,
        "cNvPr" => {
            picture.model.shape_id = attr(e, "id")
                .and_then(|value| value.parse().ok())
                .unwrap_or_default();
            picture.model.shape_name = attr(e, "name").unwrap_or_default();
        }
        "blip" if picture.in_blip_fill => {
            picture.model.relationship_id = attr(e, "embed").unwrap_or_default();
        }
        "xfrm" if picture.in_sp_pr => {
            picture.model.saw_xfrm = true;
            picture.model.rotation = attr(e, "rot")
                .and_then(|value| value.parse().ok())
                .unwrap_or_default();
            picture.model.flip_h = attr(e, "flipH").as_deref() == Some("1");
            picture.model.flip_v = attr(e, "flipV").as_deref() == Some("1");
        }
        "srcRect" if picture.in_blip_fill => {
            picture.model.crop = Some(CropInfo {
                left: attr(e, "l")
                    .and_then(|value| value.parse().ok())
                    .unwrap_or_default(),
                top: attr(e, "t")
                    .and_then(|value| value.parse().ok())
                    .unwrap_or_default(),
                right: attr(e, "r")
                    .and_then(|value| value.parse().ok())
                    .unwrap_or_default(),
                bottom: attr(e, "b")
                    .and_then(|value| value.parse().ok())
                    .unwrap_or_default(),
            });
        }
        _ => {}
    }
}

struct ExtractXmlGraph {
    masters: Vec<ExtractXmlMaster>,
    layouts: Vec<String>,
}

struct ExtractXmlMaster {
    part_uri: String,
}

struct ExtractXmlItem {
    item_type: String,
    number: i64,
    part_uri: String,
}

fn pptx_extract_xml_graph(file: &str) -> CliResult<ExtractXmlGraph> {
    let presentation = zip_text(file, "ppt/presentation.xml")?;
    let rels = relationships(file, "ppt/_rels/presentation.xml.rels")?;
    let mut reader = Reader::from_str(&presentation);
    reader.config_mut().trim_text(true);
    let mut masters = Vec::new();
    let mut layouts = Vec::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "sldMasterId" =>
            {
                let Some(rel_id) = crate::attr_exact(&e, "r:id") else {
                    continue;
                };
                let Some(target) = rels.get(&rel_id) else {
                    return Err(CliError::unexpected(format!(
                        "relationship {rel_id} not found in presentation.xml.rels"
                    )));
                };
                let master_uri = resolve_relationship_target("/ppt/presentation.xml", target);
                layouts.extend(master_layout_part_uris(file, &master_uri));
                masters.push(ExtractXmlMaster {
                    part_uri: master_uri,
                });
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }

    Ok(ExtractXmlGraph { masters, layouts })
}

fn master_layout_part_uris(file: &str, master_uri: &str) -> Vec<String> {
    relationship_entries(file, &relationships_part_for(master_uri))
        .unwrap_or_default()
        .into_iter()
        .filter(|rel| {
            rel.rel_type
                == "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout"
        })
        .map(|rel| resolve_relationship_target(master_uri, &rel.target))
        .collect()
}

fn extract_xml_item(file: &str, item: &ExtractXmlItem, out_dir: &str) -> CliResult<()> {
    let item_dir = Path::new(out_dir).join(format!("{}-{}", item.item_type, item.number));
    fs::create_dir_all(&item_dir)
        .map_err(|err| CliError::unexpected(format!("failed to create item directory: {err}")))?;

    let xml_data = zip_bytes(file, item.part_uri.trim_start_matches('/'))
        .map_err(|err| CliError::unexpected(format!("failed to read XML part: {}", err.message)))?;
    let xml_file_name = package_base_name(&item.part_uri);
    fs::write(item_dir.join(xml_file_name), xml_data)
        .map_err(|err| CliError::unexpected(format!("failed to write XML file: {err}")))?;

    let rels_uri = format!("{}.rels", item.part_uri);
    if let Ok(rels_data) = zip_bytes(file, rels_uri.trim_start_matches('/')) {
        fs::write(item_dir.join(format!("{xml_file_name}.rels")), rels_data)
            .map_err(|err| CliError::unexpected(format!("failed to write rels file: {err}")))?;
    }

    fs::write(
        item_dir.join("EXTRACTION_SUMMARY.txt"),
        extraction_summary(item, xml_file_name),
    )
    .map_err(|err| CliError::unexpected(format!("failed to write summary file: {err}")))?;
    Ok(())
}

fn extraction_summary(item: &ExtractXmlItem, xml_file_name: &str) -> String {
    format!(
        concat!(
            "=== XML Extraction Summary ===\n\n",
            "Type: {}\n",
            "Number: {}\n",
            "Part URI: {}\n\n",
            "Files:\n",
            "  - {} (main XML content)\n",
            "  - {}.rels (relationships, if present)\n\n",
            "Note: These are raw extracts from the OPC package.\n",
            "Original package bytes are preserved without reserialization.\n",
        ),
        item.item_type, item.number, item.part_uri, xml_file_name, xml_file_name
    )
}

fn parse_last_i64_flag(args: &[String], name: &str) -> CliResult<Option<i64>> {
    Ok(parse_i64_flags(args, name)?.into_iter().last())
}

fn parse_i64_flags(args: &[String], name: &str) -> CliResult<Vec<i64>> {
    let mut values = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if args[i] == name {
            let Some(value) = args.get(i + 1) else {
                return Err(CliError::invalid_args(format!("{name} requires a value")));
            };
            values.push(
                value
                    .parse::<i64>()
                    .map_err(|_| CliError::invalid_args(format!("{name} must be an integer")))?,
            );
            i += 2;
        } else if let Some(value) = args[i].strip_prefix(&format!("{name}=")) {
            values.push(
                value
                    .parse::<i64>()
                    .map_err(|_| CliError::invalid_args(format!("{name} must be an integer")))?,
            );
            i += 1;
        } else {
            i += 1;
        }
    }
    Ok(values)
}

fn package_base_name(uri: &str) -> &str {
    uri.trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or_default()
}
