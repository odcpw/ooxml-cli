use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::{
    CliError, CliResult, RelationshipEntry, WorkbookSheet, XmlNamedRange, allocate_relationship_id,
    command_arg, copy_zip_with_part_overrides, local_name, normalize_xl_target,
    relationship_entries_from_xml, relationships, relationships_part_for, render_xml_attrs,
    replace_xml_span, resolve_sheet, selector_candidates, validate,
    validate_xlsx_mutation_output_flags, workbook_sheets, xlsx_ranges_set_temp_path,
    xml_attr_escape, xml_attrs_map, xml_direct_child_ranges, xml_open_tag_from_start,
    xml_tag_prefix, zip_text,
};

const REL_NS: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const OFFICE_R_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const REL_HYPERLINK: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink";

pub(crate) struct XlsxHyperlinkAddOptions<'a> {
    pub(crate) sheet: Option<&'a str>,
    pub(crate) cell: Option<&'a str>,
    pub(crate) url: Option<&'a str>,
    pub(crate) location: Option<&'a str>,
    pub(crate) display: Option<&'a str>,
    pub(crate) tooltip: Option<&'a str>,
    pub(crate) replace: bool,
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
}

pub(crate) struct XlsxHyperlinkUpdateOptions<'a> {
    pub(crate) sheet: Option<&'a str>,
    pub(crate) cell: Option<&'a str>,
    pub(crate) url: Option<&'a str>,
    pub(crate) set_url: bool,
    pub(crate) location: Option<&'a str>,
    pub(crate) set_location: bool,
    pub(crate) display: Option<&'a str>,
    pub(crate) set_display: bool,
    pub(crate) tooltip: Option<&'a str>,
    pub(crate) set_tooltip: bool,
    pub(crate) expect_url: Option<&'a str>,
    pub(crate) has_expect_url: bool,
    pub(crate) expect_location: Option<&'a str>,
    pub(crate) has_expect_location: bool,
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
}

pub(crate) struct XlsxHyperlinkDeleteOptions<'a> {
    pub(crate) sheet: Option<&'a str>,
    pub(crate) cell: Option<&'a str>,
    pub(crate) expect_url: Option<&'a str>,
    pub(crate) has_expect_url: bool,
    pub(crate) expect_location: Option<&'a str>,
    pub(crate) has_expect_location: bool,
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
}

struct XlsxHyperlinkOutputOptions<'a> {
    out: Option<&'a str>,
    backup: Option<&'a str>,
    dry_run: bool,
    no_validate: bool,
    in_place: bool,
}

#[derive(Clone)]
struct XlsxHyperlink {
    ref_: String,
    url: String,
    location: String,
    display: String,
    tooltip: String,
    rel_id: String,
    broken: bool,
}

#[derive(Clone)]
struct XlsxHyperlinkElement {
    info: XlsxHyperlink,
    attrs: BTreeMap<String, String>,
}

#[derive(Clone)]
struct XlsxHyperlinkSheet {
    sheet: WorkbookSheet,
    part: String,
    xml: String,
}

#[derive(Clone)]
struct WorksheetRootBounds {
    start: usize,
    open_end: usize,
    close_start: usize,
    end: usize,
    tag_name: String,
    self_closing: bool,
}

pub(crate) fn xlsx_hyperlinks_list(
    file: &str,
    sheet_selector: Option<&str>,
    include_broken: bool,
) -> CliResult<Value> {
    let context = resolve_hyperlink_sheet(file, sheet_selector)?;
    let mut links = list_hyperlinks_for_context(file, &context).map_err(|err| {
        CliError::unexpected(format!("failed to list hyperlinks: {}", err.message))
    })?;
    if include_broken {
        links.retain(|link| link.broken);
    }
    let hyperlinks = links.iter().map(hyperlink_json).collect::<Vec<_>>();
    Ok(json!({
        "file": file,
        "sheet": context.sheet.name,
        "sheetNumber": context.sheet.position,
        "count": hyperlinks.len(),
        "hyperlinks": empty_array_as_null(hyperlinks),
    }))
}

pub(crate) fn xlsx_hyperlinks_show(
    file: &str,
    sheet_selector: Option<&str>,
    cell: Option<&str>,
) -> CliResult<Value> {
    let raw_ref = cell.unwrap_or_default();
    let norm_ref = normalize_hyperlink_ref(raw_ref)
        .map_err(|err| CliError::invalid_args(format!("invalid --cell: {}", err.message)))?;
    let context = resolve_hyperlink_sheet(file, sheet_selector)?;
    let links = list_hyperlinks_for_context(file, &context).map_err(|err| {
        CliError::unexpected(format!("failed to list hyperlinks: {}", err.message))
    })?;
    for link in &links {
        if normalize_hyperlink_ref(&link.ref_).is_ok_and(|candidate| candidate == norm_ref) {
            return Ok(hyperlink_json(link));
        }
    }
    Err(hyperlink_not_found(&context.sheet, &norm_ref, &links))
}

pub(crate) fn xlsx_hyperlinks_add(
    file: &str,
    options: XlsxHyperlinkAddOptions<'_>,
) -> CliResult<Value> {
    require_existing_file(file)?;
    let raw_ref = required_cell(options.cell)?;
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;
    let context = resolve_hyperlink_sheet(file, options.sheet)?;
    let mutation = add_hyperlink(file, &context, raw_ref, &options).map_err(|err| {
        CliError::invalid_args(format!("failed to add hyperlink: {}", err.message))
    })?;
    hyperlink_mutation_result(
        file,
        &context.sheet,
        "add",
        &mutation,
        options.output_options(),
    )
}

pub(crate) fn xlsx_hyperlinks_update(
    file: &str,
    options: XlsxHyperlinkUpdateOptions<'_>,
) -> CliResult<Value> {
    require_existing_file(file)?;
    let raw_ref = required_cell(options.cell)?;
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;
    let context = resolve_hyperlink_sheet(file, options.sheet)?;
    let mutation = update_hyperlink(file, &context, raw_ref, &options).map_err(|err| {
        CliError::invalid_args(format!("failed to update hyperlink: {}", err.message))
    })?;
    hyperlink_mutation_result(
        file,
        &context.sheet,
        "update",
        &mutation,
        options.output_options(),
    )
}

pub(crate) fn xlsx_hyperlinks_delete(
    file: &str,
    options: XlsxHyperlinkDeleteOptions<'_>,
) -> CliResult<Value> {
    require_existing_file(file)?;
    let raw_ref = required_cell(options.cell)?;
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;
    let context = resolve_hyperlink_sheet(file, options.sheet)?;
    let mutation = delete_hyperlink(file, &context, raw_ref, &options).map_err(|err| {
        CliError::invalid_args(format!("failed to delete hyperlink: {}", err.message))
    })?;
    hyperlink_mutation_result(
        file,
        &context.sheet,
        "delete",
        &mutation,
        options.output_options(),
    )
}

impl<'a> XlsxHyperlinkAddOptions<'a> {
    fn output_options(&self) -> XlsxHyperlinkOutputOptions<'a> {
        XlsxHyperlinkOutputOptions {
            out: self.out,
            backup: self.backup,
            dry_run: self.dry_run,
            no_validate: self.no_validate,
            in_place: self.in_place,
        }
    }
}

impl<'a> XlsxHyperlinkUpdateOptions<'a> {
    fn output_options(&self) -> XlsxHyperlinkOutputOptions<'a> {
        XlsxHyperlinkOutputOptions {
            out: self.out,
            backup: self.backup,
            dry_run: self.dry_run,
            no_validate: self.no_validate,
            in_place: self.in_place,
        }
    }
}

impl<'a> XlsxHyperlinkDeleteOptions<'a> {
    fn output_options(&self) -> XlsxHyperlinkOutputOptions<'a> {
        XlsxHyperlinkOutputOptions {
            out: self.out,
            backup: self.backup,
            dry_run: self.dry_run,
            no_validate: self.no_validate,
            in_place: self.in_place,
        }
    }
}

struct HyperlinkMutation {
    ref_: String,
    hyperlink: Option<XlsxHyperlink>,
    text_overrides: BTreeMap<String, String>,
}

fn add_hyperlink(
    file: &str,
    context: &XlsxHyperlinkSheet,
    raw_ref: &str,
    options: &XlsxHyperlinkAddOptions<'_>,
) -> CliResult<HyperlinkMutation> {
    let url = options.url.unwrap_or_default();
    let location = options.location.unwrap_or_default();
    if (url.is_empty()) == (location.is_empty()) {
        return Err(CliError::invalid_args(
            "specify exactly one of url or location",
        ));
    }
    let norm_ref = normalize_hyperlink_ref(raw_ref)?;
    let (mut rels, mut rels_changed) = worksheet_relationships(file, &context.part);
    let rel_targets = hyperlink_rel_targets(&rels);
    let mut elements = parse_hyperlink_elements(&context.xml, &rel_targets)?;
    if let Some(index) = find_hyperlink_index(&elements, &norm_ref) {
        if !options.replace {
            return Err(CliError::invalid_args(format!(
                "a hyperlink already exists on {norm_ref} (use update)"
            )));
        }
        let existing = elements.remove(index);
        rels_changed |= remove_hyperlink_rel_if_unused(
            &mut rels,
            &existing.info.rel_id,
            &elements,
            &existing.info.ref_,
        );
    }

    let mut attrs = BTreeMap::new();
    attrs.insert("ref".to_string(), norm_ref.clone());
    let mut result = XlsxHyperlink {
        ref_: norm_ref.clone(),
        url: String::new(),
        location: String::new(),
        display: options.display.unwrap_or_default().to_string(),
        tooltip: options.tooltip.unwrap_or_default().to_string(),
        rel_id: String::new(),
        broken: false,
    };
    if !url.is_empty() {
        let rel_id = allocate_relationship_id(&rels);
        rels.push(RelationshipEntry {
            id: rel_id.clone(),
            rel_type: REL_HYPERLINK.to_string(),
            target: url.to_string(),
            target_mode: "External".to_string(),
        });
        rels_changed = true;
        attrs.insert("r:id".to_string(), rel_id.clone());
        result.url = url.to_string();
        result.rel_id = rel_id;
    } else {
        attrs.insert("location".to_string(), location.to_string());
        result.location = location.to_string();
    }
    if let Some(display) = options.display.filter(|value| !value.is_empty()) {
        attrs.insert("display".to_string(), display.to_string());
    }
    if let Some(tooltip) = options.tooltip.filter(|value| !value.is_empty()) {
        attrs.insert("tooltip".to_string(), tooltip.to_string());
    }
    elements.push(XlsxHyperlinkElement {
        info: result.clone(),
        attrs,
    });

    let mut updated_xml = render_hyperlinks_into_sheet(&context.xml, &elements)?;
    if !url.is_empty() {
        updated_xml = ensure_relationships_namespace(&updated_xml)?;
    }
    let mut text_overrides = BTreeMap::new();
    text_overrides.insert(context.part.clone(), updated_xml);
    if rels_changed {
        text_overrides.insert(
            relationships_part_for(&context.part),
            render_relationships(&rels),
        );
    }
    Ok(HyperlinkMutation {
        ref_: norm_ref,
        hyperlink: Some(result),
        text_overrides,
    })
}

fn update_hyperlink(
    file: &str,
    context: &XlsxHyperlinkSheet,
    raw_ref: &str,
    options: &XlsxHyperlinkUpdateOptions<'_>,
) -> CliResult<HyperlinkMutation> {
    if options.set_url && options.set_location {
        return Err(CliError::invalid_args(
            "specify only one of url or location",
        ));
    }
    let norm_ref = normalize_hyperlink_ref(raw_ref)?;
    let (mut rels, mut rels_changed) = worksheet_relationships(file, &context.part);
    let rel_targets = hyperlink_rel_targets(&rels);
    let mut elements = parse_hyperlink_elements(&context.xml, &rel_targets)?;
    let index = find_hyperlink_index(&elements, &norm_ref)
        .ok_or_else(|| CliError::invalid_args(format!("no hyperlink found on {norm_ref}")))?;
    check_hyperlink_guards(
        &elements[index].info,
        options.has_expect_url,
        options.expect_url.unwrap_or_default(),
        options.has_expect_location,
        options.expect_location.unwrap_or_default(),
    )?;

    if options.set_url {
        remove_attr(&mut elements[index].attrs, "location");
        let url = options.url.unwrap_or_default();
        let rel_id = rel_id_attr(&elements[index].attrs);
        if rel_id.is_empty() {
            let new_rel_id = allocate_relationship_id(&rels);
            rels.push(RelationshipEntry {
                id: new_rel_id.clone(),
                rel_type: REL_HYPERLINK.to_string(),
                target: url.to_string(),
                target_mode: "External".to_string(),
            });
            elements[index]
                .attrs
                .insert("r:id".to_string(), new_rel_id.clone());
            rels_changed = true;
        } else {
            for rel in &mut rels {
                if rel.id == rel_id && rel.rel_type == REL_HYPERLINK {
                    rel.target = url.to_string();
                    rel.target_mode = "External".to_string();
                    rels_changed = true;
                }
            }
        }
    }
    if options.set_location {
        let rel_id = rel_id_attr(&elements[index].attrs);
        if !rel_id.is_empty() {
            let ref_to_exclude = elements[index].info.ref_.clone();
            rels_changed |=
                remove_hyperlink_rel_if_unused(&mut rels, &rel_id, &elements, &ref_to_exclude);
            remove_rel_id_attr(&mut elements[index].attrs);
        }
        elements[index].attrs.insert(
            "location".to_string(),
            options.location.unwrap_or_default().to_string(),
        );
    }
    if options.set_display {
        if let Some(display) = options.display.filter(|value| !value.is_empty()) {
            elements[index]
                .attrs
                .insert("display".to_string(), display.to_string());
        } else {
            remove_attr(&mut elements[index].attrs, "display");
        }
    }
    if options.set_tooltip {
        if let Some(tooltip) = options.tooltip.filter(|value| !value.is_empty()) {
            elements[index]
                .attrs
                .insert("tooltip".to_string(), tooltip.to_string());
        } else {
            remove_attr(&mut elements[index].attrs, "tooltip");
        }
    }

    let mut updated_xml = render_hyperlinks_into_sheet(&context.xml, &elements)?;
    if options.set_url {
        updated_xml = ensure_relationships_namespace(&updated_xml)?;
    }
    let rel_targets = hyperlink_rel_targets(&rels);
    let updated_elements = parse_hyperlink_elements(&updated_xml, &rel_targets)?;
    let hyperlink = updated_elements
        .into_iter()
        .find(|element| {
            normalize_hyperlink_ref(&element.info.ref_).is_ok_and(|found| found == norm_ref)
        })
        .map(|element| element.info)
        .ok_or_else(|| CliError::unexpected("updated hyperlink was not found"))?;
    let mut text_overrides = BTreeMap::new();
    text_overrides.insert(context.part.clone(), updated_xml);
    if rels_changed {
        text_overrides.insert(
            relationships_part_for(&context.part),
            render_relationships(&rels),
        );
    }
    Ok(HyperlinkMutation {
        ref_: norm_ref,
        hyperlink: Some(hyperlink),
        text_overrides,
    })
}

fn delete_hyperlink(
    file: &str,
    context: &XlsxHyperlinkSheet,
    raw_ref: &str,
    options: &XlsxHyperlinkDeleteOptions<'_>,
) -> CliResult<HyperlinkMutation> {
    let norm_ref = normalize_hyperlink_ref(raw_ref)?;
    let (mut rels, mut rels_changed) = worksheet_relationships(file, &context.part);
    let rel_targets = hyperlink_rel_targets(&rels);
    let mut elements = parse_hyperlink_elements(&context.xml, &rel_targets)?;
    let index = find_hyperlink_index(&elements, &norm_ref)
        .ok_or_else(|| CliError::invalid_args(format!("no hyperlink found on {norm_ref}")))?;
    check_hyperlink_guards(
        &elements[index].info,
        options.has_expect_url,
        options.expect_url.unwrap_or_default(),
        options.has_expect_location,
        options.expect_location.unwrap_or_default(),
    )?;
    let removed = elements.remove(index);
    rels_changed |= remove_hyperlink_rel_if_unused(
        &mut rels,
        &removed.info.rel_id,
        &elements,
        &removed.info.ref_,
    );

    let updated_xml = render_hyperlinks_into_sheet(&context.xml, &elements)?;
    let mut text_overrides = BTreeMap::new();
    text_overrides.insert(context.part.clone(), updated_xml);
    if rels_changed {
        text_overrides.insert(
            relationships_part_for(&context.part),
            render_relationships(&rels),
        );
    }
    Ok(HyperlinkMutation {
        ref_: norm_ref,
        hyperlink: None,
        text_overrides,
    })
}

fn hyperlink_mutation_result(
    file: &str,
    sheet: &WorkbookSheet,
    action: &str,
    mutation: &HyperlinkMutation,
    options: XlsxHyperlinkOutputOptions<'_>,
) -> CliResult<Value> {
    let output_path = write_xlsx_hyperlink_mutation(file, &options, &mutation.text_overrides)?;
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("sheet".to_string(), json!(sheet.name));
    result.insert("sheetNumber".to_string(), json!(sheet.position));
    result.insert("action".to_string(), json!(action));
    result.insert("ref".to_string(), json!(mutation.ref_));
    if let Some(hyperlink) = mutation.hyperlink.as_ref() {
        result.insert("hyperlink".to_string(), hyperlink_json(hyperlink));
    }
    if let Some(output_path) = output_path.as_deref() {
        result.insert("output".to_string(), json!(output_path));
    }
    result.insert("dryRun".to_string(), json!(options.dry_run));
    if let Some(output_path) = output_path.as_deref() {
        let selector = xlsx_sheet_selector(sheet);
        result.insert(
            "validateCommand".to_string(),
            json!(format!(
                "ooxml validate --strict {}",
                command_arg(output_path)
            )),
        );
        result.insert(
            "hyperlinksListCommand".to_string(),
            json!(format!(
                "ooxml --json xlsx hyperlinks list {} --sheet {}",
                command_arg(output_path),
                command_arg(&selector)
            )),
        );
    }
    Ok(Value::Object(result))
}

fn write_xlsx_hyperlink_mutation(
    file: &str,
    options: &XlsxHyperlinkOutputOptions<'_>,
    text_overrides: &BTreeMap<String, String>,
) -> CliResult<Option<String>> {
    let output_path = options.out.filter(|value| !value.trim().is_empty());
    let commit_path = if options.in_place {
        Some(file)
    } else {
        output_path
    };
    let readback_path = if options.dry_run || options.in_place || output_path == Some(file) {
        xlsx_ranges_set_temp_path(file)
    } else {
        output_path
            .ok_or_else(|| {
                CliError::invalid_args(
                    "must specify exactly one of --out, --in-place, or --dry-run",
                )
            })?
            .to_string()
    };
    copy_zip_with_part_overrides(file, &readback_path, text_overrides)?;
    if !options.no_validate {
        validate(&readback_path, true)?;
    }
    if options.dry_run {
        let _ = fs::remove_file(&readback_path);
    } else if options.in_place || output_path == Some(file) {
        if let Some(backup_path) = options.backup.filter(|value| !value.trim().is_empty()) {
            fs::copy(file, backup_path)
                .map_err(|err| CliError::unexpected(format!("failed to create backup: {err}")))?;
        }
        fs::rename(&readback_path, file)
            .or_else(|_| {
                fs::copy(&readback_path, file)?;
                fs::remove_file(&readback_path)
            })
            .map_err(|err| CliError::unexpected(format!("failed to write output file: {err}")))?;
    }
    Ok(commit_path.map(ToString::to_string))
}

fn resolve_hyperlink_sheet(
    file: &str,
    sheet_selector: Option<&str>,
) -> CliResult<XlsxHyperlinkSheet> {
    let workbook = zip_text(file, "xl/workbook.xml")?;
    let sheets = workbook_sheets(&workbook)?;
    let sheet = resolve_sheet(&sheets, sheet_selector.unwrap_or("1"))?;
    let rels = relationships(file, "xl/_rels/workbook.xml.rels")?;
    let target = rels
        .get(&sheet.rel_id)
        .ok_or_else(|| CliError::unexpected(format!("missing relationship {}", sheet.rel_id)))?;
    let part = normalize_xl_target(target);
    let xml = zip_text(file, &part)?;
    Ok(XlsxHyperlinkSheet { sheet, part, xml })
}

fn list_hyperlinks_for_context(
    file: &str,
    context: &XlsxHyperlinkSheet,
) -> CliResult<Vec<XlsxHyperlink>> {
    let (rels, _) = worksheet_relationships(file, &context.part);
    let rel_targets = hyperlink_rel_targets(&rels);
    Ok(parse_hyperlink_elements(&context.xml, &rel_targets)?
        .into_iter()
        .map(|element| element.info)
        .collect())
}

fn worksheet_relationships(file: &str, sheet_part: &str) -> (Vec<RelationshipEntry>, bool) {
    let rels_part = relationships_part_for(sheet_part);
    let xml = zip_text(file, &rels_part).unwrap_or_else(|_| relationships_template());
    (relationship_entries_from_xml(&xml), false)
}

fn hyperlink_rel_targets(rels: &[RelationshipEntry]) -> BTreeMap<String, String> {
    rels.iter()
        .filter(|rel| rel.rel_type == REL_HYPERLINK)
        .map(|rel| (rel.id.clone(), rel.target.clone()))
        .collect()
}

fn parse_hyperlink_elements(
    sheet_xml: &str,
    rel_targets: &BTreeMap<String, String>,
) -> CliResult<Vec<XlsxHyperlinkElement>> {
    let Some(range) = hyperlinks_range(sheet_xml)? else {
        return Ok(Vec::new());
    };
    let fragment = &sheet_xml[range.start..range.end];
    let (open_end, _, close_start, self_closing) = crate::xml_fragment_bounds(fragment)?;
    if self_closing {
        return Ok(Vec::new());
    }
    let child_ranges = xml_direct_child_ranges(fragment, open_end + 1, close_start)?;
    let mut elements = Vec::new();
    for child in child_ranges {
        if child.kind != "hyperlink" {
            continue;
        }
        let child_xml = &fragment[child.start..child.end];
        let attrs = hyperlink_attrs(child_xml)?;
        let info = hyperlink_from_attrs(&attrs, rel_targets);
        elements.push(XlsxHyperlinkElement { info, attrs });
    }
    Ok(elements)
}

fn hyperlink_attrs(xml: &str) -> CliResult<BTreeMap<String, String>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "hyperlink" =>
            {
                return Ok(xml_attrs_map(&e));
            }
            Ok(Event::Eof) => return Err(CliError::unexpected("hyperlink element not found")),
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
}

fn hyperlink_from_attrs(
    attrs: &BTreeMap<String, String>,
    rel_targets: &BTreeMap<String, String>,
) -> XlsxHyperlink {
    let rel_id = rel_id_attr(attrs);
    let mut link = XlsxHyperlink {
        ref_: attr_value(attrs, "ref"),
        url: String::new(),
        location: attr_value(attrs, "location"),
        display: attr_value(attrs, "display"),
        tooltip: attr_value(attrs, "tooltip"),
        rel_id: rel_id.clone(),
        broken: false,
    };
    if !rel_id.is_empty() {
        if let Some(target) = rel_targets.get(&rel_id) {
            link.url = target.clone();
        } else {
            link.broken = true;
        }
    }
    link
}

fn find_hyperlink_index(elements: &[XlsxHyperlinkElement], norm_ref: &str) -> Option<usize> {
    elements.iter().position(|element| {
        normalize_hyperlink_ref(&element.info.ref_).is_ok_and(|candidate| candidate == norm_ref)
    })
}

fn render_hyperlinks_into_sheet(
    sheet_xml: &str,
    elements: &[XlsxHyperlinkElement],
) -> CliResult<String> {
    let root = worksheet_root_bounds(sheet_xml)?;
    let prefix = xml_tag_prefix(&root.tag_name);
    let range = direct_worksheet_child_range(sheet_xml, &root, "hyperlinks")?;
    if elements.is_empty() {
        if let Some(range) = range {
            return Ok(replace_xml_span(sheet_xml, range.start, range.end, ""));
        }
        return Ok(sheet_xml.to_string());
    }
    let child_xml = elements
        .iter()
        .map(|element| render_hyperlink_element(&prefix, &element.attrs))
        .collect::<String>();
    let container = format!(
        "<{}>{}</{}>",
        element_name(&prefix, "hyperlinks"),
        child_xml,
        element_name(&prefix, "hyperlinks")
    );
    if let Some(range) = range {
        return Ok(replace_xml_span(
            sheet_xml,
            range.start,
            range.end,
            &container,
        ));
    }
    insert_worksheet_child(sheet_xml, &root, "hyperlinks", &container)
}

fn render_hyperlink_element(prefix: &str, attrs: &BTreeMap<String, String>) -> String {
    format!(
        "<{}{}/>",
        element_name(prefix, "hyperlink"),
        render_xml_attrs(attrs)
    )
}

fn remove_hyperlink_rel_if_unused(
    rels: &mut Vec<RelationshipEntry>,
    rel_id: &str,
    elements: &[XlsxHyperlinkElement],
    exclude_ref: &str,
) -> bool {
    if rel_id.is_empty() {
        return false;
    }
    if elements
        .iter()
        .any(|element| element.info.rel_id == rel_id && element.info.ref_ != exclude_ref)
    {
        return false;
    }
    let before = rels.len();
    rels.retain(|rel| !(rel.id == rel_id && rel.rel_type == REL_HYPERLINK));
    rels.len() != before
}

fn check_hyperlink_guards(
    current: &XlsxHyperlink,
    has_url: bool,
    expect_url: &str,
    has_location: bool,
    expect_location: &str,
) -> CliResult<()> {
    if has_url && current.url != expect_url {
        return Err(CliError::invalid_args(format!(
            "expected url {:?} but found {:?}",
            expect_url, current.url
        )));
    }
    if has_location && current.location != expect_location {
        return Err(CliError::invalid_args(format!(
            "expected location {:?} but found {:?}",
            expect_location, current.location
        )));
    }
    Ok(())
}

fn hyperlinks_range(sheet_xml: &str) -> CliResult<Option<XmlNamedRange>> {
    let root = worksheet_root_bounds(sheet_xml)?;
    direct_worksheet_child_range(sheet_xml, &root, "hyperlinks")
}

fn worksheet_root_bounds(xml: &str) -> CliResult<WorksheetRootBounds> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "worksheet" => {
                let open_end = reader.buffer_position() as usize;
                let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let close_tag = format!("</{tag_name}>");
                let close_start = xml
                    .rfind(&close_tag)
                    .ok_or_else(|| CliError::unexpected("worksheet root has no closing tag"))?;
                return Ok(WorksheetRootBounds {
                    start: before,
                    open_end,
                    close_start,
                    end: close_start + close_tag.len(),
                    tag_name,
                    self_closing: false,
                });
            }
            Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == "worksheet" => {
                return Ok(WorksheetRootBounds {
                    start: before,
                    open_end: reader.buffer_position() as usize,
                    close_start: reader.buffer_position() as usize,
                    end: reader.buffer_position() as usize,
                    tag_name: String::from_utf8_lossy(e.name().as_ref()).to_string(),
                    self_closing: true,
                });
            }
            Ok(Event::Eof) => return Err(CliError::unexpected("worksheet root not found")),
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
}

fn direct_worksheet_child_range(
    xml: &str,
    root: &WorksheetRootBounds,
    kind: &str,
) -> CliResult<Option<XmlNamedRange>> {
    if root.self_closing || root.open_end >= root.close_start {
        return Ok(None);
    }
    Ok(
        xml_direct_child_ranges(xml, root.open_end, root.close_start)?
            .into_iter()
            .find(|child| child.kind == kind),
    )
}

fn insert_worksheet_child(
    xml: &str,
    root: &WorksheetRootBounds,
    local: &str,
    child_xml: &str,
) -> CliResult<String> {
    if root.self_closing {
        let start_tag = xml_open_tag_from_start(&xml[root.start..root.open_end]);
        let mut updated = String::new();
        updated.push_str(&xml[..root.start]);
        updated.push_str(&start_tag);
        updated.push_str(child_xml);
        updated.push_str(&format!("</{}>", root.tag_name));
        updated.push_str(&xml[root.end..]);
        return Ok(updated);
    }
    let target_order = worksheet_child_order(local);
    let insert_at = xml_direct_child_ranges(xml, root.open_end, root.close_start)?
        .into_iter()
        .find(|child| worksheet_child_order(&child.kind) > target_order)
        .map(|child| child.start)
        .unwrap_or(root.close_start);
    let mut updated = String::new();
    updated.push_str(&xml[..insert_at]);
    updated.push_str(child_xml);
    updated.push_str(&xml[insert_at..]);
    Ok(updated)
}

fn ensure_relationships_namespace(xml: &str) -> CliResult<String> {
    let root = worksheet_root_bounds(xml)?;
    let start_tag = &xml[root.start..root.open_end];
    if start_tag.contains("xmlns:r=") {
        return Ok(xml.to_string());
    }
    let relative_insert = start_tag
        .rfind("/>")
        .unwrap_or_else(|| start_tag.len().saturating_sub(1));
    let insert_at = root.start + relative_insert;
    let attr = format!(r#" xmlns:r="{OFFICE_R_NS}""#);
    let mut updated = String::with_capacity(xml.len() + attr.len());
    updated.push_str(&xml[..insert_at]);
    updated.push_str(&attr);
    updated.push_str(&xml[insert_at..]);
    Ok(updated)
}

fn worksheet_child_order(local_name: &str) -> i32 {
    match local_name {
        "sheetPr" => 10,
        "dimension" => 20,
        "sheetViews" => 30,
        "sheetFormatPr" => 40,
        "cols" => 50,
        "sheetData" => 60,
        "sheetCalcPr" => 70,
        "sheetProtection" => 80,
        "protectedRanges" => 90,
        "scenarios" => 100,
        "autoFilter" => 110,
        "sortState" => 120,
        "dataConsolidate" => 130,
        "customSheetViews" => 140,
        "mergeCells" => 150,
        "phoneticPr" => 160,
        "conditionalFormatting" => 170,
        "dataValidations" => 180,
        "hyperlinks" => 190,
        "printOptions" => 200,
        "pageMargins" => 210,
        "pageSetup" => 220,
        "headerFooter" => 230,
        "rowBreaks" => 240,
        "colBreaks" => 250,
        "customProperties" => 260,
        "cellWatches" => 270,
        "ignoredErrors" => 280,
        "smartTags" => 290,
        "drawing" => 300,
        "legacyDrawing" => 310,
        "legacyDrawingHF" => 320,
        "picture" => 330,
        "oleObjects" => 340,
        "controls" => 350,
        "webPublishItems" => 360,
        "tableParts" => 370,
        "extLst" => 380,
        _ => 1000,
    }
}

fn normalize_hyperlink_ref(ref_: &str) -> CliResult<String> {
    let ref_ = ref_.trim();
    if ref_.is_empty() {
        return Err(CliError::invalid_args("hyperlink ref cannot be empty"));
    }
    if ref_.contains(':') {
        return normalize_hyperlink_range(ref_);
    }
    parse_hyperlink_cell(ref_).map(|cell| cell.to_a1())
}

fn normalize_hyperlink_range(ref_: &str) -> CliResult<String> {
    let parts = ref_.split(':').collect::<Vec<_>>();
    if parts.len() > 2 {
        return Err(CliError::invalid_args(format!(
            "invalid range reference {ref_:?}"
        )));
    }
    let start = parse_hyperlink_cell(parts[0])
        .map_err(|err| CliError::invalid_args(format!("invalid range start: {}", err.message)))?;
    let end = if let Some(end) = parts.get(1) {
        if end.trim().is_empty() {
            return Err(CliError::invalid_args("range end cannot be empty"));
        }
        parse_hyperlink_cell(end)
            .map_err(|err| CliError::invalid_args(format!("invalid range end: {}", err.message)))?
    } else {
        start
    };
    if start == end {
        Ok(start.to_a1())
    } else {
        Ok(format!("{}:{}", start.to_a1(), end.to_a1()))
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct HyperlinkCellRef {
    col: u32,
    row: u32,
    abs_col: bool,
    abs_row: bool,
}

impl HyperlinkCellRef {
    fn to_a1(self) -> String {
        let mut out = String::new();
        if self.abs_col {
            out.push('$');
        }
        out.push_str(&column_name(self.col));
        if self.abs_row {
            out.push('$');
        }
        out.push_str(&self.row.to_string());
        out
    }
}

fn parse_hyperlink_cell(value: &str) -> CliResult<HyperlinkCellRef> {
    let mut rest = value.trim();
    if rest.is_empty() {
        return Err(CliError::invalid_args("cell reference cannot be empty"));
    }
    let abs_col = if let Some(after) = rest.strip_prefix('$') {
        rest = after;
        if rest.is_empty() {
            return Err(CliError::invalid_args("missing column in cell reference"));
        }
        true
    } else {
        false
    };
    let col_len = rest
        .bytes()
        .take_while(|byte| byte.is_ascii_alphabetic())
        .count();
    if col_len == 0 {
        return Err(CliError::invalid_args("missing column in cell reference"));
    }
    let col_letters = &rest[..col_len];
    let col = column_letters_to_index(col_letters)?;
    rest = &rest[col_len..];
    if rest.is_empty() {
        return Err(CliError::invalid_args("missing row in cell reference"));
    }
    let abs_row = if let Some(after) = rest.strip_prefix('$') {
        rest = after;
        if rest.is_empty() {
            return Err(CliError::invalid_args("missing row in cell reference"));
        }
        true
    } else {
        false
    };
    if rest.contains('$') {
        return Err(CliError::invalid_args(
            "invalid absolute marker in row reference",
        ));
    }
    if !rest.chars().all(|ch| ch.is_ascii_digit()) {
        return Err(CliError::invalid_args(format!(
            "invalid row {rest:?} in cell reference"
        )));
    }
    let row = rest
        .parse::<u32>()
        .map_err(|err| CliError::invalid_args(format!("invalid row {rest:?}: {err}")))?;
    if row == 0 || row > 1_048_576 {
        return Err(CliError::invalid_args(format!(
            "row {row} out of XLSX bounds 1-1048576"
        )));
    }
    Ok(HyperlinkCellRef {
        col,
        row,
        abs_col,
        abs_row,
    })
}

fn column_letters_to_index(letters: &str) -> CliResult<u32> {
    if letters.trim().is_empty() {
        return Err(CliError::invalid_args("column letters cannot be empty"));
    }
    let mut col = 0u32;
    for ch in letters.chars() {
        if !ch.is_ascii_alphabetic() {
            return Err(CliError::invalid_args(format!(
                "invalid column letter {ch:?}"
            )));
        }
        col = col * 26 + (ch.to_ascii_uppercase() as u32 - 'A' as u32 + 1);
        if col > 16_384 {
            return Err(CliError::invalid_args(format!(
                "column {letters:?} out of XLSX bounds A-XFD"
            )));
        }
    }
    Ok(col)
}

fn column_name(mut col: u32) -> String {
    let mut chars = Vec::new();
    while col > 0 {
        col -= 1;
        chars.push((b'A' + (col % 26) as u8) as char);
        col /= 26;
    }
    chars.iter().rev().collect()
}

fn hyperlink_json(link: &XlsxHyperlink) -> Value {
    let mut map = Map::new();
    map.insert("ref".to_string(), json!(link.ref_));
    if !link.ref_.is_empty() {
        map.insert("primarySelector".to_string(), json!(link.ref_));
        map.insert("selectors".to_string(), json!([link.ref_]));
    }
    if !link.url.is_empty() {
        map.insert("url".to_string(), json!(link.url));
    }
    if !link.location.is_empty() {
        map.insert("location".to_string(), json!(link.location));
    }
    if !link.display.is_empty() {
        map.insert("display".to_string(), json!(link.display));
    }
    if !link.tooltip.is_empty() {
        map.insert("tooltip".to_string(), json!(link.tooltip));
    }
    if !link.rel_id.is_empty() {
        map.insert("relId".to_string(), json!(link.rel_id));
    }
    if link.broken {
        map.insert("broken".to_string(), json!(true));
    }
    Value::Object(map)
}

fn empty_array_as_null(items: Vec<Value>) -> Value {
    if items.is_empty() {
        Value::Null
    } else {
        Value::Array(items)
    }
}

fn hyperlink_not_found(sheet: &WorkbookSheet, selector: &str, links: &[XlsxHyperlink]) -> CliError {
    let owned = links
        .iter()
        .map(|link| (link.ref_.clone(), vec![link.ref_.clone()]))
        .collect::<Vec<_>>();
    let borrowed = owned
        .iter()
        .map(|(primary, selectors)| (primary.as_str(), selectors.as_slice()))
        .collect::<Vec<_>>();
    let candidates = selector_candidates(&borrowed, selector, 3);
    let mut message = format!("hyperlink not found: {selector}");
    if !candidates.is_empty() {
        message.push_str(&format!("; did you mean: {}", candidates.join(", ")));
    }
    message.push_str(&format!(
        "; discover with `ooxml --json xlsx hyperlinks list <file> --sheet {}`",
        xlsx_sheet_selector(sheet)
    ));
    CliError::target_not_found(message)
}

fn required_cell(cell: Option<&str>) -> CliResult<&str> {
    cell.map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| CliError::invalid_args("--cell is required"))
}

fn require_existing_file(file: &str) -> CliResult<()> {
    if !Path::new(file).exists() {
        return Err(CliError::file_not_found(format!("file not found: {file}")));
    }
    Ok(())
}

fn xlsx_sheet_selector(sheet: &WorkbookSheet) -> String {
    format!("sheetId:{}", sheet.sheet_id)
}

fn attr_value(attrs: &BTreeMap<String, String>, local: &str) -> String {
    attrs
        .iter()
        .find_map(|(key, value)| (attr_local_name(key) == local).then(|| value.clone()))
        .unwrap_or_default()
}

fn rel_id_attr(attrs: &BTreeMap<String, String>) -> String {
    attrs
        .iter()
        .find_map(|(key, value)| {
            (key == "r:id" || attr_local_name(key) == "id").then(|| value.clone())
        })
        .unwrap_or_default()
}

fn remove_attr(attrs: &mut BTreeMap<String, String>, local: &str) {
    if let Some(key) = attrs
        .keys()
        .find(|key| attr_local_name(key) == local)
        .cloned()
    {
        attrs.remove(&key);
    }
}

fn remove_rel_id_attr(attrs: &mut BTreeMap<String, String>) {
    if let Some(key) = attrs
        .keys()
        .find(|key| *key == "r:id" || attr_local_name(key) == "id")
        .cloned()
    {
        attrs.remove(&key);
    }
}

fn attr_local_name(key: &str) -> &str {
    key.rsplit_once(':').map(|(_, local)| local).unwrap_or(key)
}

fn relationships_template() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="{REL_NS}"></Relationships>"#
    )
}

fn render_relationships(rels: &[RelationshipEntry]) -> String {
    let body = rels.iter().map(render_relationship).collect::<String>();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="{REL_NS}">{body}</Relationships>"#
    )
}

fn render_relationship(rel: &RelationshipEntry) -> String {
    let mut out = format!(
        r#"<Relationship Id="{}" Type="{}" Target="{}""#,
        xml_attr_escape(&rel.id),
        xml_attr_escape(&rel.rel_type),
        xml_attr_escape(&rel.target)
    );
    if !rel.target_mode.is_empty() {
        out.push_str(&format!(
            r#" TargetMode="{}""#,
            xml_attr_escape(&rel.target_mode)
        ));
    }
    out.push_str("/>");
    out
}

fn element_name(prefix: &str, local: &str) -> String {
    if prefix.is_empty() {
        local.to_string()
    } else {
        format!("{prefix}:{local}")
    }
}
