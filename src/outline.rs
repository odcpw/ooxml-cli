use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::fs;

use crate::inspect::inspect;
use crate::xlsx_freeze::xlsx_freeze_show;
use crate::{
    CliError, CliResult, append_xml_text_event, attr, command_arg,
    docx_block_has_section_properties, docx_blocks_show, docx_body_block_ranges, docx_body_tag,
    docx_fields_list, docx_headers_footers_list, docx_images_list, docx_tables_show,
    find_docx_document_part, is_xml_text_event, local_name, pptx_charts_list, pptx_layouts_list,
    pptx_masters_list, pptx_masters_show, pptx_shapes_show, pptx_slides_list, xlsx_charts_list,
    xlsx_comments_list, xlsx_conditional_formats_list, xlsx_data_validations_list, xlsx_names_list,
    xlsx_pivots_list, xlsx_sheets_show, xlsx_tables_list, zip_entry_names, zip_text,
};

const EMU_PER_INCH: f64 = 914_400.0;
const TWIPS_PER_INCH: f64 = 1_440.0;

#[derive(Clone, Debug)]
pub(crate) struct OutlineOptions<'a> {
    pub(crate) depth: u32,
    pub(crate) text_preview: usize,
    pub(crate) slide: Option<u32>,
    pub(crate) sheet: Option<&'a str>,
    pub(crate) section: Option<u32>,
}

pub(crate) fn outline(file: &str, options: OutlineOptions<'_>) -> CliResult<Value> {
    if options.depth > 3 {
        return Err(CliError::invalid_args("--depth must be between 0 and 3"));
    }
    if options.slide == Some(0) {
        return Err(CliError::invalid_args("--slide must be >= 1"));
    }
    if options.section == Some(0) {
        return Err(CliError::invalid_args("--section must be >= 1"));
    }
    if options.sheet.is_some_and(|sheet| sheet.trim().is_empty()) {
        return Err(CliError::invalid_args("--sheet must not be empty"));
    }
    let scope_count = usize::from(options.slide.is_some())
        + usize::from(options.sheet.is_some())
        + usize::from(options.section.is_some());
    if scope_count > 1 {
        return Err(CliError::invalid_args(
            "specify only one of --slide, --sheet, or --section",
        ));
    }

    let inspected = inspect(file)?;
    let family = inspected["type"]
        .as_str()
        .ok_or_else(|| CliError::unexpected("inspect returned no package type"))?;
    validate_family_scope(family, &options)?;
    let file_size = fs::metadata(file)
        .map_err(|err| CliError::unexpected(format!("failed to read file metadata: {err}")))?
        .len();
    let mut result = Map::new();
    result.insert("schemaVersion".to_string(), json!(1));
    result.insert("file".to_string(), json!(file));
    result.insert("type".to_string(), json!(family));
    result.insert("fileSizeBytes".to_string(), json!(file_size));
    result.insert("depth".to_string(), json!(options.depth));
    result.insert("textPreviewChars".to_string(), json!(options.text_preview));
    result.insert("summary".to_string(), inspected["summary"].clone());
    result.insert(
        "checkCommand".to_string(),
        json!(format!("ooxml --json check {}", command_arg(file))),
    );
    if let Some(scope) = scope_json(&options) {
        result.insert("scope".to_string(), scope);
    }

    match family {
        "pptx" => outline_pptx(file, &options, &inspected, &mut result)?,
        "xlsx" => outline_xlsx(file, &options, &mut result)?,
        "docx" => outline_docx(file, &options, &mut result)?,
        other => {
            return Err(CliError::unsupported_type(format!(
                "outline does not support package type: {other}"
            )));
        }
    }
    Ok(Value::Object(result))
}

fn validate_family_scope(family: &str, options: &OutlineOptions<'_>) -> CliResult<()> {
    let mismatch = match family {
        "pptx" => options.sheet.is_some() || options.section.is_some(),
        "xlsx" => options.slide.is_some() || options.section.is_some(),
        "docx" => options.slide.is_some() || options.sheet.is_some(),
        _ => false,
    };
    if mismatch {
        let valid = match family {
            "pptx" => "--slide",
            "xlsx" => "--sheet",
            "docx" => "--section",
            _ => "no scope flag",
        };
        return Err(CliError::invalid_args(format!(
            "scope flag does not match {family}; use {valid}"
        )));
    }
    Ok(())
}

fn scope_json(options: &OutlineOptions<'_>) -> Option<Value> {
    if let Some(slide) = options.slide {
        Some(json!({"slide": slide}))
    } else if let Some(sheet) = options.sheet {
        Some(json!({"sheet": sheet}))
    } else {
        options.section.map(|section| json!({"section": section}))
    }
}

fn outline_pptx(
    file: &str,
    options: &OutlineOptions<'_>,
    inspected: &Value,
    result: &mut Map<String, Value>,
) -> CliResult<()> {
    let size = &inspected["summary"]["slideSize"];
    let cx = size["cx"].as_i64().unwrap_or_default();
    let cy = size["cy"].as_i64().unwrap_or_default();
    result.insert(
        "slideSize".to_string(),
        json!({
            "emu": {"width": cx, "height": cy},
            "inches": {"width": cx as f64 / EMU_PER_INCH, "height": cy as f64 / EMU_PER_INCH},
        }),
    );
    if options.depth == 0 {
        return Ok(());
    }

    let masters = pptx_masters_list(file)?;
    let layouts = pptx_layouts_list(file, None)?;
    let master_items = array_at(&masters, "masters");
    if !master_items.is_empty() {
        let first = pptx_masters_show(file, 1)?;
        if let Some(theme) = first.get("theme") {
            result.insert("theme".to_string(), theme.clone());
        }
    }
    result.insert(
        "masters".to_string(),
        Value::Array(
            master_items
                .iter()
                .map(|item| {
                    project_fields(
                        item,
                        &["index", "uri", "primarySelector", "layouts", "theme"],
                    )
                })
                .collect(),
        ),
    );
    result.insert(
        "layouts".to_string(),
        Value::Array(
            array_at(&layouts, "layouts")
                .iter()
                .map(|item| {
                    project_fields(
                        item,
                        &[
                            "id",
                            "number",
                            "name",
                            "partUri",
                            "masterId",
                            "primarySelector",
                            "placeholderCount",
                            "placeholders",
                        ],
                    )
                })
                .collect(),
        ),
    );

    let slides_report = pptx_slides_list(file)?;
    let mut slide_items = array_at(&slides_report, "slides").to_vec();
    if let Some(wanted) = options.slide {
        slide_items.retain(|slide| slide["number"].as_u64() == Some(u64::from(wanted)));
        if slide_items.is_empty() {
            return Err(CliError::target_not_found(format!(
                "target not found: slide {wanted}"
            )));
        }
    }
    let chart_report = if options.depth >= 3 {
        Some(pptx_charts_list(
            file,
            options.slide.map(i64::from).unwrap_or_default(),
        )?)
    } else {
        None
    };
    let all_charts = chart_report
        .as_ref()
        .map(|value| array_at(value, "charts"))
        .unwrap_or_default();

    let mut slides = Vec::new();
    for slide in slide_items {
        let number = slide["number"].as_u64().unwrap_or_default() as u32;
        let shapes_report = pptx_shapes_show(file, number, true, true)?;
        let source_shapes = array_at(&shapes_report, "shapes");
        let shapes = source_shapes
            .iter()
            .map(|shape| compact_pptx_shape(shape, options.text_preview))
            .collect::<Vec<_>>();
        let title = source_shapes
            .iter()
            .find(|shape| shape["targetKind"] == "title")
            .and_then(shape_text)
            .map(|text| text_preview(&text, options.text_preview))
            .filter(|text| !text.is_empty());
        let charts = all_charts
            .iter()
            .filter(|chart| chart["slide"].as_u64() == Some(u64::from(number)))
            .map(|chart| compact_pptx_chart(chart, source_shapes))
            .collect::<Vec<_>>();
        let tables = source_shapes
            .iter()
            .filter(|shape| shape.get("tableInfo").is_some())
            .map(compact_pptx_table)
            .collect::<Vec<_>>();
        let images = source_shapes
            .iter()
            .filter(|shape| shape.get("imageRef").is_some())
            .map(compact_pptx_image)
            .collect::<Vec<_>>();

        let mut item = Map::new();
        copy_fields(
            &slide,
            &mut item,
            &[
                "number",
                "slideId",
                "handle",
                "partUri",
                "primarySelector",
                "layout",
                "layoutNumber",
                "layoutPartUri",
                "notes",
            ],
        );
        if let Some(title) = title {
            item.insert("title".to_string(), json!(title));
        }
        item.insert("shapeCount".to_string(), json!(source_shapes.len()));
        item.insert("chartCount".to_string(), json!(charts.len()));
        item.insert("tableCount".to_string(), json!(tables.len()));
        item.insert("imageCount".to_string(), json!(images.len()));
        if options.depth >= 2 {
            item.insert("shapes".to_string(), Value::Array(shapes));
        }
        if options.depth >= 3 {
            item.insert("charts".to_string(), Value::Array(charts));
            item.insert("tables".to_string(), Value::Array(tables));
            item.insert("images".to_string(), Value::Array(images));
        }
        slides.push(Value::Object(item));
    }
    result.insert("slides".to_string(), Value::Array(slides));
    Ok(())
}

fn compact_pptx_shape(shape: &Value, preview_chars: usize) -> Value {
    let mut item = Map::new();
    if let Some(selector) = shape.get("primarySelector") {
        item.insert("selector".to_string(), selector.clone());
    }
    copy_fields(
        shape,
        &mut item,
        &[
            "handle",
            "shapeId",
            "shapeName",
            "shapeType",
            "targetKind",
            "placeholder",
            "bounds",
            "boundsSource",
        ],
    );
    if let Some(kind) = shape.get("targetKind") {
        item.insert("kind".to_string(), kind.clone());
    }
    if let Some(text) = shape_text(shape) {
        let preview = text_preview(&text, preview_chars);
        if !preview.is_empty() {
            item.insert("textPreview".to_string(), json!(preview));
        }
    }
    Value::Object(item)
}

fn compact_pptx_chart(chart: &Value, shapes: &[Value]) -> Value {
    let mut item = Map::new();
    if let Some(selector) = chart.get("primarySelector") {
        item.insert("selector".to_string(), selector.clone());
    }
    copy_fields(
        chart,
        &mut item,
        &["number", "partUri", "title", "types", "shapeId"],
    );
    if let Some(shape_id) = chart["shapeId"].as_str()
        && let Some(handle) = shapes.iter().find_map(|shape| {
            (shape["shapeId"]
                .as_u64()
                .map(|id| id.to_string())
                .as_deref()
                == Some(shape_id))
            .then(|| shape.get("handle").cloned())
            .flatten()
        })
    {
        item.insert("handle".to_string(), handle);
    }
    Value::Object(item)
}

fn compact_pptx_table(shape: &Value) -> Value {
    let table = &shape["tableInfo"];
    let mut item = Map::new();
    item.insert("selector".to_string(), shape["primarySelector"].clone());
    copy_fields(shape, &mut item, &["handle", "shapeId", "shapeName"]);
    item.insert(
        "rows".to_string(),
        json!(array_len_or_number(table, "rows")),
    );
    item.insert(
        "cols".to_string(),
        json!(array_len_or_number(table, "cols")),
    );
    Value::Object(item)
}

fn compact_pptx_image(shape: &Value) -> Value {
    let image = &shape["imageRef"];
    let mut item = Map::new();
    item.insert("selector".to_string(), shape["primarySelector"].clone());
    copy_fields(shape, &mut item, &["handle", "shapeId", "shapeName"]);
    copy_fields(
        image,
        &mut item,
        &["relationshipId", "partUri", "targetUri", "contentType"],
    );
    Value::Object(item)
}

fn outline_xlsx(
    file: &str,
    options: &OutlineOptions<'_>,
    result: &mut Map<String, Value>,
) -> CliResult<()> {
    if options.depth == 0 {
        return Ok(());
    }
    let report = xlsx_sheets_show(file, options.sheet)?;
    // Workbook-scoped names remain relevant when the tree is narrowed to one
    // sheet, so outline must not apply the names-list local-scope filter.
    let names = xlsx_names_list(file, None)?;
    result.insert(
        "names".to_string(),
        Value::Array(
            array_at(&names, "names")
                .iter()
                .map(|name| {
                    project_fields(
                        name,
                        &[
                            "name",
                            "scope",
                            "scopeSheet",
                            "ref",
                            "formula",
                            "primarySelector",
                            "handle",
                        ],
                    )
                })
                .collect(),
        ),
    );

    let mut sheets = Vec::new();
    for sheet in array_at(&report, "sheets") {
        let selector = sheet["primarySelector"]
            .as_str()
            .ok_or_else(|| CliError::unexpected("sheet readback returned no primarySelector"))?;
        let freeze = xlsx_freeze_show(file, Some(selector))?;
        let validations = xlsx_data_validations_list(file, Some(selector))?;
        let conditional_formats = xlsx_conditional_formats_list(file, Some(selector), None)?;
        let comments = xlsx_comments_list(file, Some(selector), None)?;
        let table_report = if options.depth >= 3 {
            Some(xlsx_tables_list(file, Some(selector))?)
        } else {
            None
        };
        let chart_report = if options.depth >= 3 {
            Some(xlsx_charts_list(file, Some(selector))?)
        } else {
            None
        };
        let pivot_report = if options.depth >= 3 {
            Some(xlsx_pivots_list(file, Some(selector))?)
        } else {
            None
        };

        let tables = table_report
            .as_ref()
            .map(|value| {
                array_at(value, "tables")
                    .iter()
                    .map(compact_xlsx_table)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let charts = chart_report
            .as_ref()
            .map(|value| {
                array_at(value, "charts")
                    .iter()
                    .map(|chart| {
                        project_fields(
                            chart,
                            &[
                                "number",
                                "name",
                                "primarySelector",
                                "partUri",
                                "title",
                                "types",
                                "anchor",
                            ],
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let pivots = pivot_report
            .as_ref()
            .map(|value| {
                array_at(value, "pivots")
                    .iter()
                    .map(|pivot| {
                        project_fields(
                            pivot,
                            &[
                                "number",
                                "name",
                                "primarySelector",
                                "partUri",
                                "location",
                                "source",
                            ],
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let mut item = Map::new();
        copy_fields(
            sheet,
            &mut item,
            &[
                "number",
                "name",
                "sheetId",
                "state",
                "partUri",
                "primarySelector",
                "handle",
                "dimensionDeclared",
                "usedRange",
                "rowCount",
                "cellCount",
            ],
        );
        item.insert("freeze".to_string(), freeze["state"].clone());
        item.insert(
            "validationCount".to_string(),
            json!(validations["count"].as_u64().unwrap_or_default()),
        );
        item.insert(
            "conditionalFormatCount".to_string(),
            json!(conditional_formats["count"].as_u64().unwrap_or_default()),
        );
        item.insert(
            "commentCount".to_string(),
            json!(array_at(&comments, "comments").len()),
        );
        if options.depth >= 3 {
            item.insert("tables".to_string(), Value::Array(tables));
            item.insert("charts".to_string(), Value::Array(charts));
            item.insert("pivots".to_string(), Value::Array(pivots));
        }
        sheets.push(Value::Object(item));
    }
    result.insert("sheets".to_string(), Value::Array(sheets));
    Ok(())
}

fn compact_xlsx_table(table: &Value) -> Value {
    project_fields(
        table,
        &[
            "name",
            "displayName",
            "range",
            "style",
            "primarySelector",
            "handle",
            "partUri",
        ],
    )
}

fn outline_docx(
    file: &str,
    options: &OutlineOptions<'_>,
    result: &mut Map<String, Value>,
) -> CliResult<()> {
    let entries = zip_entry_names(file)?;
    let document_part = find_docx_document_part(file, &entries)?;
    let document_xml = zip_text(file, &document_part)?;
    let blocks_report = docx_blocks_show(file, 0, false)?;
    if let Some(hash) = blocks_report.get("documentHash") {
        result.insert("documentHash".to_string(), hash.clone());
    }
    if options.depth == 0 {
        return Ok(());
    }

    let block_sections = docx_block_sections(&document_xml)?;
    let header_report = docx_headers_footers_list(file)?;
    let header_sections = array_at(&header_report, "sections");
    let page_setups = docx_section_page_setups(&document_xml)?;
    let section_count = page_setups
        .len()
        .max(header_sections.len())
        .max(block_sections.iter().copied().max().unwrap_or(1))
        .max(1);
    if let Some(wanted) = options.section
        && wanted as usize > section_count
    {
        return Err(CliError::target_not_found(format!(
            "target not found: section {wanted}"
        )));
    }

    let mut sections = Vec::new();
    for index in 1..=section_count {
        if options
            .section
            .is_some_and(|wanted| wanted as usize != index)
        {
            continue;
        }
        let header_section = header_sections
            .iter()
            .find(|section| section["sectionIndex"].as_u64() == Some(index as u64));
        let headers = header_section
            .map(|section| header_footer_refs(section, "headers"))
            .unwrap_or_default();
        let footers = header_section
            .map(|section| header_footer_refs(section, "footers"))
            .unwrap_or_default();
        let mut section = Map::new();
        section.insert("number".to_string(), json!(index));
        section.insert(
            "pageSetup".to_string(),
            page_setups
                .get(index - 1)
                .cloned()
                .unwrap_or_else(|| json!({})),
        );
        section.insert("headers".to_string(), Value::Array(headers));
        section.insert("footers".to_string(), Value::Array(footers));
        sections.push(Value::Object(section));
    }
    result.insert("sections".to_string(), Value::Array(sections.clone()));
    result.insert("coreProperties".to_string(), docx_core_properties(file)?);

    let mut blocks = Vec::new();
    for block in array_at(&blocks_report, "blocks") {
        let index = block["index"].as_u64().unwrap_or_default() as usize;
        let section = block_sections
            .get(index.saturating_sub(1))
            .copied()
            .unwrap_or(1);
        if options
            .section
            .is_some_and(|wanted| wanted as usize != section)
        {
            continue;
        }
        let mut item = Map::new();
        copy_fields(
            block,
            &mut item,
            &[
                "id",
                "index",
                "kind",
                "primarySelector",
                "handle",
                "styleId",
                "listLevel",
                "numId",
                "contentHash",
            ],
        );
        item.insert("section".to_string(), json!(section));
        if let Some(text) = block.get("text").and_then(Value::as_str) {
            let preview = text_preview(text, options.text_preview);
            if !preview.is_empty() {
                item.insert("textPreview".to_string(), json!(preview));
            }
        }
        blocks.push(Value::Object(item));
    }
    if options.depth >= 2 {
        result.insert("blocks".to_string(), Value::Array(blocks));
    } else {
        result.insert("blockCount".to_string(), json!(blocks.len()));
    }

    if options.depth >= 3 {
        let tables_report = docx_tables_show(file, 0, false)?;
        let images_report = docx_images_list(file)?;
        let fields_report = docx_fields_list(file, None)?;
        let wanted = options.section.map(|section| section as usize);
        result.insert(
            "tables".to_string(),
            Value::Array(
                array_at(&tables_report, "tables")
                    .iter()
                    .filter(|table| item_in_docx_section(table, "block", wanted, &block_sections))
                    .map(|table| {
                        project_fields(
                            table,
                            &[
                                "table",
                                "block",
                                "rows",
                                "cols",
                                "styleId",
                                "contentHash",
                                "primarySelector",
                                "handle",
                            ],
                        )
                    })
                    .collect(),
            ),
        );
        result.insert(
            "images".to_string(),
            Value::Array(
                array_at(&images_report, "images")
                    .iter()
                    .filter(|image| {
                        item_in_docx_section(image, "blockIndex", wanted, &block_sections)
                    })
                    .map(|image| {
                        project_fields(
                            image,
                            &[
                                "index",
                                "primarySelector",
                                "blockIndex",
                                "mediaUri",
                                "contentType",
                                "width",
                                "height",
                                "widthInches",
                                "heightInches",
                            ],
                        )
                    })
                    .collect(),
            ),
        );
        let (headers, footers) = flatten_docx_header_footers(&sections);
        result.insert("headers".to_string(), Value::Array(headers));
        result.insert("footers".to_string(), Value::Array(footers));
        result.insert(
            "fields".to_string(),
            Value::Array(
                array_at(&fields_report, "fields")
                    .iter()
                    .filter(|field| {
                        field_in_docx_section(field, wanted, &block_sections, &sections)
                    })
                    .map(|field| {
                        project_fields(
                            field,
                            &[
                                "index",
                                "fieldType",
                                "instruction",
                                "cachedResult",
                                "location",
                                "partUri",
                                "blockIndex",
                            ],
                        )
                    })
                    .collect(),
            ),
        );
    }
    Ok(())
}

fn docx_block_sections(xml: &str) -> CliResult<Vec<usize>> {
    let body_tag = docx_body_tag(xml)?;
    let ranges = docx_body_block_ranges(xml, &body_tag)?;
    let mut current = 1usize;
    let mut sections = Vec::with_capacity(ranges.len());
    for range in ranges {
        sections.push(current);
        if docx_block_has_section_properties(&xml[range.start..range.end]) {
            current += 1;
        }
    }
    Ok(sections)
}

fn docx_section_page_setups(xml: &str) -> CliResult<Vec<Value>> {
    let mut reader = Reader::from_str(xml);
    let mut current: Option<Map<String, Value>> = None;
    let mut sections = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) => match local_name(element.name().as_ref()) {
                "sectPr" if current.is_none() => current = Some(Map::new()),
                "pgSz" if current.is_some() => note_page_size(&mut current, &element),
                "pgMar" if current.is_some() => note_page_margins(&mut current, &element),
                "cols" if current.is_some() => note_columns(&mut current, &element),
                _ => {}
            },
            Ok(Event::Empty(element)) => match local_name(element.name().as_ref()) {
                "sectPr" if current.is_none() => sections.push(json!({})),
                "pgSz" if current.is_some() => note_page_size(&mut current, &element),
                "pgMar" if current.is_some() => note_page_margins(&mut current, &element),
                "cols" if current.is_some() => note_columns(&mut current, &element),
                "titlePg" if current.is_some() => {
                    current
                        .as_mut()
                        .expect("section page setup")
                        .insert("titlePage".to_string(), json!(true));
                }
                _ => {}
            },
            Ok(Event::End(element)) if local_name(element.name().as_ref()) == "sectPr" => {
                sections.push(Value::Object(current.take().unwrap_or_default()));
            }
            Ok(Event::Eof) => break,
            Err(err) => {
                return Err(CliError::unexpected(format!(
                    "failed to parse DOCX section setup: {err}"
                )));
            }
            _ => {}
        }
    }
    if sections.is_empty() {
        sections.push(json!({}));
    }
    Ok(sections)
}

fn note_page_size(
    current: &mut Option<Map<String, Value>>,
    element: &quick_xml::events::BytesStart<'_>,
) {
    let width = attr(element, "w").and_then(|value| value.parse::<i64>().ok());
    let height = attr(element, "h").and_then(|value| value.parse::<i64>().ok());
    let mut size = Map::new();
    if let Some(width) = width {
        size.insert("widthTwips".to_string(), json!(width));
        size.insert(
            "widthInches".to_string(),
            json!(width as f64 / TWIPS_PER_INCH),
        );
    }
    if let Some(height) = height {
        size.insert("heightTwips".to_string(), json!(height));
        size.insert(
            "heightInches".to_string(),
            json!(height as f64 / TWIPS_PER_INCH),
        );
    }
    if let Some(orientation) = attr(element, "orient") {
        size.insert("orientation".to_string(), json!(orientation));
    }
    current
        .as_mut()
        .expect("section page setup")
        .insert("pageSize".to_string(), Value::Object(size));
}

fn note_page_margins(
    current: &mut Option<Map<String, Value>>,
    element: &quick_xml::events::BytesStart<'_>,
) {
    let mut margins = Map::new();
    for key in [
        "top", "right", "bottom", "left", "header", "footer", "gutter",
    ] {
        if let Some(value) = attr(element, key).and_then(|value| value.parse::<i64>().ok()) {
            margins.insert(format!("{key}Twips"), json!(value));
            margins.insert(format!("{key}Inches"), json!(value as f64 / TWIPS_PER_INCH));
        }
    }
    current
        .as_mut()
        .expect("section page setup")
        .insert("margins".to_string(), Value::Object(margins));
}

fn note_columns(
    current: &mut Option<Map<String, Value>>,
    element: &quick_xml::events::BytesStart<'_>,
) {
    let mut columns = Map::new();
    if let Some(count) = attr(element, "num").and_then(|value| value.parse::<u32>().ok()) {
        columns.insert("count".to_string(), json!(count));
    }
    if let Some(space) = attr(element, "space").and_then(|value| value.parse::<i64>().ok()) {
        columns.insert("spaceTwips".to_string(), json!(space));
        columns.insert(
            "spaceInches".to_string(),
            json!(space as f64 / TWIPS_PER_INCH),
        );
    }
    current
        .as_mut()
        .expect("section page setup")
        .insert("columns".to_string(), Value::Object(columns));
}

fn docx_core_properties(file: &str) -> CliResult<Value> {
    let xml = match zip_text(file, "docProps/core.xml") {
        Ok(xml) => xml,
        Err(_) => return Ok(json!({})),
    };
    let wanted = [
        "title",
        "subject",
        "creator",
        "keywords",
        "description",
        "lastModifiedBy",
        "revision",
        "created",
        "modified",
        "category",
        "contentStatus",
    ];
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(false);
    let mut active = None::<String>;
    let mut values = BTreeMap::<String, String>::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) => {
                let name = local_name(element.name().as_ref()).to_string();
                if wanted.contains(&name.as_str()) {
                    active = Some(name);
                }
            }
            Ok(event) if active.is_some() && is_xml_text_event(&event) => {
                let key = active.as_ref().expect("active core property").clone();
                append_xml_text_event(values.entry(key).or_default(), &event);
            }
            Ok(Event::End(element)) => {
                if active.as_deref() == Some(local_name(element.name().as_ref())) {
                    active = None;
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => {
                return Err(CliError::unexpected(format!(
                    "failed to parse DOCX core properties: {err}"
                )));
            }
            _ => {}
        }
    }
    Ok(serde_json::to_value(values).expect("serialize core properties"))
}

fn header_footer_refs(section: &Value, kind: &str) -> Vec<Value> {
    ["default", "first", "even"]
        .into_iter()
        .filter_map(|slot| {
            section[kind][slot]
                .as_object()
                .map(|_| section[kind][slot].clone())
        })
        .collect()
}

fn flatten_docx_header_footers(sections: &[Value]) -> (Vec<Value>, Vec<Value>) {
    let mut headers = Vec::new();
    let mut footers = Vec::new();
    for section in sections {
        headers.extend(array_at(section, "headers").iter().cloned());
        footers.extend(array_at(section, "footers").iter().cloned());
    }
    (headers, footers)
}

fn item_in_docx_section(
    item: &Value,
    block_key: &str,
    wanted: Option<usize>,
    block_sections: &[usize],
) -> bool {
    let Some(wanted) = wanted else {
        return true;
    };
    let block = item[block_key].as_u64().unwrap_or_default() as usize;
    block_sections.get(block.saturating_sub(1)).copied() == Some(wanted)
}

fn field_in_docx_section(
    field: &Value,
    wanted: Option<usize>,
    block_sections: &[usize],
    sections: &[Value],
) -> bool {
    let Some(wanted) = wanted else {
        return true;
    };
    if field["partUri"] == "/word/document.xml" {
        return item_in_docx_section(field, "blockIndex", Some(wanted), block_sections);
    }
    sections
        .iter()
        .flat_map(|section| {
            array_at(section, "headers")
                .iter()
                .chain(array_at(section, "footers").iter())
        })
        .any(|reference| reference["partUri"] == field["partUri"])
}

fn array_at<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

fn project_fields(value: &Value, fields: &[&str]) -> Value {
    let mut result = Map::new();
    copy_fields(value, &mut result, fields);
    Value::Object(result)
}

fn copy_fields(value: &Value, result: &mut Map<String, Value>, fields: &[&str]) {
    for field in fields {
        if let Some(item) = value.get(*field)
            && !item.is_null()
        {
            result.insert((*field).to_string(), item.clone());
        }
    }
}

fn array_len_or_number(value: &Value, key: &str) -> usize {
    value[key]
        .as_array()
        .map(Vec::len)
        .or_else(|| value[key].as_u64().map(|number| number as usize))
        .unwrap_or_default()
}

fn shape_text(shape: &Value) -> Option<String> {
    let paragraphs = array_at(shape, "paragraphs");
    if !paragraphs.is_empty() {
        return Some(
            paragraphs
                .iter()
                .filter_map(|paragraph| paragraph["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
    shape["textPreview"].as_str().map(ToString::to_string)
}

fn text_preview(text: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let count = normalized.chars().count();
    if count <= max_chars {
        return normalized;
    }
    if max_chars <= 3 {
        return normalized.chars().take(max_chars).collect();
    }
    let mut preview = normalized.chars().take(max_chars - 3).collect::<String>();
    preview.push_str("...");
    preview
}

#[cfg(test)]
mod tests {
    use super::text_preview;

    #[test]
    fn preview_is_whitespace_normalized_unicode_safe_and_bounded() {
        assert_eq!(text_preview("  hello\n world  ", 20), "hello world");
        assert_eq!(text_preview("abcdef", 5), "ab...");
        assert_eq!(text_preview("åßçdé", 4), "å...");
        assert_eq!(text_preview("hidden", 0), "");
    }
}
