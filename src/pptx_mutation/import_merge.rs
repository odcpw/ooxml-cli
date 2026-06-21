use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use regex::Regex;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::sync::OnceLock;

use crate::{
    CliError, CliResult, RelationshipEntry, allocate_relationship_id, attr, attr_exact,
    chrono_like_counter, content_type_for_part, copy_zip_with_binary_part_overrides_and_removals,
    ensure_content_type_override, has_flag, local_name, package_mutation_temp_path, package_type,
    parse_i64_flag, parse_string_flag, relationship_entries_from_xml,
    relationship_target_from_source_to_target, relationships_part_for, replace_xml_span,
    resolve_relationship_target, validate, validate_xlsx_mutation_output_flags, xml_attr_escape,
    zip_bytes, zip_entry_names, zip_text,
};

mod output;

use self::output::{add_layout_readback_commands, add_master_readback_commands, output_basename};

const SLIDE_REL_TYPE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide";
const NOTES_REL_TYPE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesSlide";
const SLIDE_LAYOUT_REL_TYPE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout";
const SLIDE_MASTER_REL_TYPE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster";
const THEME_REL_TYPE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme";

const SLIDE_CONTENT_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.slide+xml";
const NOTES_CONTENT_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.notesSlide+xml";
const LAYOUT_CONTENT_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml";
const MASTER_CONTENT_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml";

#[derive(Clone)]
struct PptxImportMutationOptions {
    out: Option<String>,
    backup: Option<String>,
    dry_run: bool,
    in_place: bool,
    no_validate: bool,
}

#[derive(Clone)]
struct SlideRef {
    part_uri: String,
    layout_uri: String,
    notes_uri: String,
}

#[derive(Clone)]
struct LayoutRef {
    name: String,
    part_uri: String,
    master_uri: String,
    theme_uri: String,
}

#[derive(Clone)]
struct MasterRef {
    number: usize,
    part_uri: String,
    layout_uris: Vec<String>,
    theme_uri: String,
}

struct PptxGraph {
    slides: Vec<SlideRef>,
    layouts: Vec<LayoutRef>,
    masters: Vec<MasterRef>,
}

struct PptxPackageEditor {
    base_file: String,
    entries: BTreeSet<String>,
    text_overrides: BTreeMap<String, String>,
    binary_overrides: BTreeMap<String, Vec<u8>>,
    content_types_xml: String,
}

struct PartImportContext<'a> {
    source_file: &'a str,
    imported: BTreeMap<String, String>,
}

struct ImportMasterResult {
    target_master_uri: String,
    theme_uri: String,
    imported: bool,
    layout_mappings: BTreeMap<String, String>,
}

struct ImportLayoutResult {
    target_layout_uri: String,
    target_master_uri: String,
    theme_uri: String,
    name: String,
    imported: bool,
    master_imported: bool,
}

struct ImportSlideMutationResult {
    staged_path: String,
    new_slide_number: usize,
    new_slide_id: u32,
    new_slide_uri: String,
    notes_uri: String,
}

struct ImportPolicies {
    layout: String,
    theme: String,
    notes: String,
}

#[derive(Clone, Copy)]
struct XmlSpan {
    start: usize,
    end: usize,
}

struct SlideIdSpan {
    rel_id: String,
    id: u32,
    start: usize,
    end: usize,
}

pub(crate) fn pptx_slides_import_slide(file: &str, args: &[String]) -> CliResult<Value> {
    let source = required_string_flag(args, "--source")?;
    let slide = parse_i64_flag(args, "--slide")?
        .ok_or_else(|| CliError::invalid_args("--slide must be specified"))?;
    if slide < 1 {
        return Err(CliError::invalid_args("--slide must be >= 1"));
    }
    let insert_after = parse_i64_flag(args, "--insert-after")?.unwrap_or(0);
    let layout_policy = policy_flag(args, "--layout-policy", "reuse", "layout")?;
    let theme_policy = policy_flag(args, "--theme-policy", "reuse", "theme")?;
    let notes_policy = notes_policy_flag(args)?;
    let options = parse_import_mutation_options(args)?;
    ensure_pptx_package(file)?;
    ensure_pptx_package(&source)?;

    let result = stage_import_slide(
        file,
        &source,
        slide as usize,
        insert_after,
        &ImportPolicies {
            layout: layout_policy,
            theme: theme_policy,
            notes: notes_policy,
        },
        &options,
    )?;
    let output_path = mutation_output_path(file, &options);
    let mut out = Map::new();
    out.insert("newSlideNumber".to_string(), json!(result.new_slide_number));
    out.insert("newSlideId".to_string(), json!(result.new_slide_id));
    out.insert("newSlideUri".to_string(), json!(result.new_slide_uri));
    if !result.notes_uri.is_empty() {
        out.insert("notesUri".to_string(), json!(result.notes_uri));
    }
    finish_import_mutation(file, &result.staged_path, &options, output_path.as_deref())?;
    Ok(Value::Object(out))
}

pub(crate) fn pptx_slides_merge(file: &str, source: &str, args: &[String]) -> CliResult<Value> {
    let layout_policy = policy_flag(args, "--layout-policy", "reuse", "layout")?;
    let theme_policy = policy_flag(args, "--theme-policy", "reuse", "theme")?;
    let options = parse_import_mutation_options(args)?;
    ensure_pptx_package(file)?;
    ensure_pptx_package(source)?;

    let source_graph = parse_pptx_graph(source)?;
    if source_graph.slides.is_empty() {
        return Err(CliError::unexpected("source presentation has no slides"));
    }
    let before_count = parse_pptx_graph(file)?.slides.len();
    let output_path = mutation_output_path(file, &options);
    let staged_path = stage_merge_slides(file, source, &layout_policy, &theme_policy, &options)?;
    let after_count = parse_pptx_graph(&staged_path)?.slides.len();

    let mut out = Map::new();
    out.insert(
        "file".to_string(),
        json!(
            output_path
                .as_deref()
                .map(output_basename)
                .unwrap_or_else(|| ".".to_string())
        ),
    );
    out.insert("sourceFile".to_string(), json!(output_basename(source)));
    out.insert(
        "mergedSlideCount".to_string(),
        json!(after_count.saturating_sub(before_count)),
    );
    out.insert("totalSlideCount".to_string(), json!(after_count));
    out.insert("layoutPolicy".to_string(), json!(layout_policy));
    out.insert("themePolicy".to_string(), json!(theme_policy));
    finish_import_mutation(file, &staged_path, &options, output_path.as_deref())?;
    Ok(Value::Object(out))
}

pub(crate) fn pptx_layouts_import(file: &str, args: &[String]) -> CliResult<Value> {
    let source = required_string_flag(args, "--source")?;
    let selector = required_string_flag(args, "--layout")?;
    let theme_policy = policy_flag(args, "--theme-policy", "reuse", "theme")?;
    let options = parse_import_mutation_options(args)?;
    ensure_pptx_package(file)?;
    ensure_pptx_package(&source)?;

    let mut editor = PptxPackageEditor::new(file)?;
    let mut import_ctx = PartImportContext::new(&source);
    let imported = import_layout_into_editor(
        file,
        &source,
        &mut editor,
        &mut import_ctx,
        &selector,
        &theme_policy,
    )?;
    let output_path = mutation_output_path(file, &options);
    let staged_path = stage_editor(file, editor, &options)?;
    let mut out = Map::new();
    out.insert("file".to_string(), json!(file));
    if let Some(path) = output_path.as_deref() {
        out.insert("output".to_string(), json!(path));
    }
    out.insert("dryRun".to_string(), json!(output_path.is_none()));
    out.insert(
        "targetLayoutUri".to_string(),
        json!(imported.target_layout_uri),
    );
    out.insert(
        "targetMasterUri".to_string(),
        json!(imported.target_master_uri),
    );
    if !imported.theme_uri.is_empty() {
        out.insert("themeUri".to_string(), json!(imported.theme_uri));
    }
    if !imported.name.is_empty() {
        out.insert("name".to_string(), json!(imported.name));
    }
    out.insert("imported".to_string(), json!(imported.imported));
    out.insert(
        "masterImported".to_string(),
        json!(imported.master_imported),
    );
    add_layout_readback_commands(&mut out, output_path.as_deref(), &imported.name);
    finish_import_mutation(file, &staged_path, &options, output_path.as_deref())?;
    Ok(Value::Object(out))
}

pub(crate) fn pptx_masters_import(file: &str, args: &[String]) -> CliResult<Value> {
    let source = required_string_flag(args, "--source")?;
    let master = parse_i64_flag(args, "--master")?
        .ok_or_else(|| CliError::invalid_args("--master must be specified"))?;
    if master < 1 {
        return Err(CliError::invalid_args("--master must be >= 1"));
    }
    let theme_policy = policy_flag(args, "--theme-policy", "reuse", "theme")?;
    let options = parse_import_mutation_options(args)?;
    ensure_pptx_package(file)?;
    ensure_pptx_package(&source)?;

    let source_graph = parse_pptx_graph(&source)?;
    let source_master = source_graph
        .masters
        .get(master as usize - 1)
        .cloned()
        .ok_or_else(|| {
            CliError::invalid_args(format!(
                "master {master} is out of range (1-{})",
                source_graph.masters.len()
            ))
        })?;
    let before_masters = parse_pptx_graph(file)?.masters;
    let mut editor = PptxPackageEditor::new(file)?;
    let mut import_ctx = PartImportContext::new(&source);
    let imported = import_master_into_editor(
        file,
        &source,
        &mut editor,
        &mut import_ctx,
        &source_master.part_uri,
        &theme_policy,
    )?;
    let output_path = mutation_output_path(file, &options);
    let staged_path = stage_editor(file, editor, &options)?;
    let target_master = if imported.imported {
        before_masters.len() + 1
    } else {
        before_masters
            .iter()
            .find(|candidate| candidate.part_uri == imported.target_master_uri)
            .map(|candidate| candidate.number)
            .unwrap_or(1)
    };

    let mut out = Map::new();
    out.insert("file".to_string(), json!(file));
    if let Some(path) = output_path.as_deref() {
        out.insert("output".to_string(), json!(path));
    }
    out.insert("dryRun".to_string(), json!(output_path.is_none()));
    out.insert(
        "targetMasterUri".to_string(),
        json!(imported.target_master_uri),
    );
    out.insert("targetMaster".to_string(), json!(target_master));
    if !imported.theme_uri.is_empty() {
        out.insert("themeUri".to_string(), json!(imported.theme_uri));
    }
    out.insert("imported".to_string(), json!(imported.imported));
    out.insert(
        "layoutCount".to_string(),
        json!(imported.layout_mappings.len()),
    );
    add_master_readback_commands(&mut out, output_path.as_deref(), target_master);
    finish_import_mutation(file, &staged_path, &options, output_path.as_deref())?;
    Ok(Value::Object(out))
}

impl PptxPackageEditor {
    fn new(base_file: &str) -> CliResult<Self> {
        let entries = zip_entry_names(base_file)?
            .into_iter()
            .map(|entry| entry.trim_start_matches('/').to_string())
            .collect::<BTreeSet<_>>();
        let content_types_xml = zip_text(base_file, "[Content_Types].xml")?;
        Ok(Self {
            base_file: base_file.to_string(),
            entries,
            text_overrides: BTreeMap::new(),
            binary_overrides: BTreeMap::new(),
            content_types_xml,
        })
    }

    fn has_part(&self, uri: &str) -> bool {
        self.entries.contains(&part_name(uri))
    }

    fn read_bytes(&self, uri: &str) -> CliResult<Vec<u8>> {
        let part = part_name(uri);
        if let Some(data) = self.binary_overrides.get(&part) {
            return Ok(data.clone());
        }
        if let Some(text) = self.text_overrides.get(&part) {
            return Ok(text.as_bytes().to_vec());
        }
        zip_bytes(&self.base_file, &part)
    }

    fn read_text(&self, uri: &str) -> CliResult<String> {
        let part = part_name(uri);
        if let Some(text) = self.text_overrides.get(&part) {
            return Ok(text.clone());
        }
        if let Some(data) = self.binary_overrides.get(&part) {
            return String::from_utf8(data.clone())
                .map_err(|err| CliError::unexpected(err.to_string()));
        }
        zip_text(&self.base_file, &part)
    }

    fn relationship_entries(&self, source_uri: &str) -> Vec<RelationshipEntry> {
        let rels_uri = package_uri(&relationships_part_for(source_uri));
        if !self.has_part(&rels_uri) {
            return Vec::new();
        }
        self.read_text(&rels_uri)
            .map(|xml| relationship_entries_from_xml(&xml))
            .unwrap_or_default()
    }

    fn content_type(&self, uri: &str) -> String {
        content_type_from_xml(&self.content_types_xml, uri)
    }

    fn add_binary_part(&mut self, uri: &str, data: Vec<u8>, content_type: &str) {
        let part = part_name(uri);
        self.entries.insert(part.clone());
        self.binary_overrides.insert(part.clone(), data);
        if !content_type.is_empty() && !part.ends_with(".rels") {
            self.content_types_xml =
                ensure_content_type_override(self.content_types_xml.clone(), &part, content_type);
            self.text_overrides.insert(
                "[Content_Types].xml".to_string(),
                self.content_types_xml.clone(),
            );
        }
    }

    fn add_text_part(&mut self, uri: &str, text: String, content_type: &str) {
        let part = part_name(uri);
        self.entries.insert(part.clone());
        self.text_overrides.insert(part.clone(), text);
        if !content_type.is_empty() && !part.ends_with(".rels") {
            self.content_types_xml =
                ensure_content_type_override(self.content_types_xml.clone(), &part, content_type);
            self.text_overrides.insert(
                "[Content_Types].xml".to_string(),
                self.content_types_xml.clone(),
            );
        }
    }

    fn add_relationships(&mut self, source_uri: &str, rels: &[RelationshipEntry]) {
        let rels_part = relationships_part_for(source_uri);
        self.entries.insert(rels_part.clone());
        self.text_overrides
            .insert(rels_part, render_relationships_xml(rels));
    }

    fn replace_text_part(&mut self, uri: &str, text: String) {
        let part = part_name(uri);
        self.entries.insert(part.clone());
        self.text_overrides.insert(part, text);
    }
}

impl<'a> PartImportContext<'a> {
    fn new(source_file: &'a str) -> Self {
        Self {
            source_file,
            imported: BTreeMap::new(),
        }
    }

    fn copy_dependency_tree(
        &mut self,
        editor: &mut PptxPackageEditor,
        source_uri: &str,
    ) -> CliResult<String> {
        let source_uri = package_uri(source_uri);
        if let Some(target) = self.imported.get(&source_uri) {
            return Ok(target.clone());
        }
        let source_part = part_name(&source_uri);
        let bytes = zip_bytes(self.source_file, &source_part)?;
        let content_type = content_type_for_part(self.source_file, &source_uri)?;
        if let Some(existing) = find_exact_target_part(editor, &bytes, &content_type)? {
            self.imported.insert(source_uri, existing.clone());
            return Ok(existing);
        }

        let target_uri = allocate_imported_part_uri(editor, &source_uri, &content_type);
        editor.add_binary_part(&target_uri, bytes, &content_type);
        self.imported.insert(source_uri.clone(), target_uri.clone());

        let rels = source_relationship_entries(self.source_file, &source_uri);
        if !rels.is_empty() {
            let mut target_rels = Vec::new();
            for rel in rels {
                if rel.target_mode == "External" {
                    target_rels.push(RelationshipEntry {
                        id: rel.id,
                        rel_type: rel.rel_type,
                        target: rel.target,
                        target_mode: rel.target_mode,
                    });
                    continue;
                }
                let source_target = resolve_relationship_target(&source_uri, &rel.target);
                let copied_target = self.copy_dependency_tree(editor, &source_target)?;
                target_rels.push(RelationshipEntry {
                    id: rel.id,
                    rel_type: rel.rel_type,
                    target: relationship_target_from_source_to_target(&target_uri, &copied_target),
                    target_mode: String::new(),
                });
            }
            editor.add_relationships(&target_uri, &target_rels);
        }
        Ok(target_uri)
    }
}

fn stage_import_slide(
    target_file: &str,
    source_file: &str,
    source_slide_number: usize,
    insert_after: i64,
    policies: &ImportPolicies,
    options: &PptxImportMutationOptions,
) -> CliResult<ImportSlideMutationResult> {
    let source_graph = parse_pptx_graph(source_file)?;
    let target_graph = parse_pptx_graph(target_file)?;
    let source_slide = source_graph
        .slides
        .get(source_slide_number - 1)
        .cloned()
        .ok_or_else(|| {
            CliError::unexpected(format!(
                "failed to import slide: source slide {source_slide_number} not found"
            ))
        })?;
    let mut insert_after = insert_after;
    if insert_after == 0 {
        insert_after = target_graph.slides.len() as i64;
    }
    if insert_after < 0 || insert_after as usize > target_graph.slides.len() {
        return Err(CliError::unexpected(format!(
            "insert-after {insert_after} out of range for target with {} slides",
            target_graph.slides.len()
        )));
    }

    let mut editor = PptxPackageEditor::new(target_file)?;
    let mut import_ctx = PartImportContext::new(source_file);
    let new_slide_uri = allocate_numbered_part_uri(&editor, "ppt/slides/slide", ".xml");
    let slide_xml = zip_text(source_file, &part_name(&source_slide.part_uri))?;
    editor.add_text_part(
        &new_slide_uri,
        remint_pptx_creation_ids(&slide_xml, &new_slide_uri),
        SLIDE_CONTENT_TYPE,
    );

    let source_slide_rels = source_relationship_entries(source_file, &source_slide.part_uri);
    let mut new_slide_rels = Vec::new();
    let mut imported_notes_uri = String::new();
    for rel in source_slide_rels {
        if rel.rel_type == NOTES_REL_TYPE {
            if policies.notes == "clone"
                && let Some((notes_uri, notes_rel)) = import_notes(
                    source_file,
                    &source_slide,
                    &new_slide_uri,
                    &rel,
                    &mut editor,
                    &mut import_ctx,
                )?
            {
                imported_notes_uri = notes_uri;
                new_slide_rels.push(notes_rel);
            }
            continue;
        }

        if rel.target_mode == "External" {
            new_slide_rels.push(RelationshipEntry {
                id: rel.id,
                rel_type: rel.rel_type,
                target: rel.target,
                target_mode: rel.target_mode,
            });
            continue;
        }

        let source_target_uri = resolve_relationship_target(&source_slide.part_uri, &rel.target);
        let target_uri = match rel.rel_type.as_str() {
            SLIDE_LAYOUT_REL_TYPE => resolve_slide_layout_target(
                target_file,
                source_file,
                &source_graph,
                &mut editor,
                &mut import_ctx,
                &source_slide.layout_uri,
                policies,
            )?,
            SLIDE_MASTER_REL_TYPE | THEME_REL_TYPE => {
                return Err(CliError::unexpected(format!(
                    "direct slide {} relationships are not supported; import the layout chain instead",
                    rel.rel_type
                )));
            }
            _ => import_ctx.copy_dependency_tree(&mut editor, &source_target_uri)?,
        };
        new_slide_rels.push(RelationshipEntry {
            id: rel.id,
            rel_type: rel.rel_type,
            target: relationship_target_from_source_to_target(&new_slide_uri, &target_uri),
            target_mode: String::new(),
        });
    }
    editor.add_relationships(&new_slide_uri, &new_slide_rels);

    let presentation_xml = editor.read_text("/ppt/presentation.xml")?;
    let pres_rels = editor.relationship_entries("/ppt/presentation.xml");
    let new_slide_id = next_presentation_slide_id(&presentation_xml);
    let rel_id = allocate_relationship_id(&pres_rels);
    let new_fragment = format!(
        r#"<p:sldId id="{new_slide_id}" r:id="{}"/>"#,
        xml_attr_escape(&rel_id)
    );
    let updated_presentation =
        insert_slide_fragment(&presentation_xml, insert_after as usize, &new_fragment)?;
    editor.replace_text_part("/ppt/presentation.xml", updated_presentation);
    let mut updated_pres_rels = pres_rels;
    updated_pres_rels.push(RelationshipEntry {
        id: rel_id,
        rel_type: SLIDE_REL_TYPE.to_string(),
        target: relationship_target_from_source_to_target("/ppt/presentation.xml", &new_slide_uri),
        target_mode: String::new(),
    });
    editor.add_relationships("/ppt/presentation.xml", &updated_pres_rels);

    let staged_path = stage_editor(target_file, editor, options)?;
    Ok(ImportSlideMutationResult {
        staged_path,
        new_slide_number: insert_after as usize + 1,
        new_slide_id,
        new_slide_uri,
        notes_uri: imported_notes_uri,
    })
}

fn stage_merge_slides(
    target_file: &str,
    source_file: &str,
    layout_policy: &str,
    theme_policy: &str,
    options: &PptxImportMutationOptions,
) -> CliResult<String> {
    let source_count = parse_pptx_graph(source_file)?.slides.len();
    let mut current = target_file.to_string();
    let mut temps = Vec::new();
    for slide_number in 1..=source_count {
        let loop_options = PptxImportMutationOptions {
            out: Some(package_mutation_temp_path(target_file, "pptx-merge-step")),
            backup: None,
            dry_run: false,
            in_place: false,
            no_validate: true,
        };
        let staged = stage_import_slide(
            &current,
            source_file,
            slide_number,
            parse_pptx_graph(&current)?.slides.len() as i64,
            &ImportPolicies {
                layout: layout_policy.to_string(),
                theme: theme_policy.to_string(),
                notes: "clone".to_string(),
            },
            &loop_options,
        )?
        .staged_path;
        if current != target_file {
            temps.push(current);
        }
        current = staged;
    }

    let final_path = stage_path_for_options(target_file, options)?;
    if current != final_path {
        fs::copy(&current, &final_path)
            .map_err(|err| CliError::unexpected(format!("failed to stage merged PPTX: {err}")))?;
        temps.push(current);
    }
    for temp in temps {
        let _ = fs::remove_file(temp);
    }
    if !options.no_validate {
        validate(&final_path, true)?;
    }
    Ok(final_path)
}

fn import_notes(
    source_file: &str,
    source_slide: &SlideRef,
    new_slide_uri: &str,
    slide_notes_rel: &RelationshipEntry,
    editor: &mut PptxPackageEditor,
    import_ctx: &mut PartImportContext<'_>,
) -> CliResult<Option<(String, RelationshipEntry)>> {
    if source_slide.notes_uri.is_empty() {
        return Ok(None);
    }
    let new_notes_uri = allocate_numbered_part_uri(editor, "ppt/notesSlides/notesSlide", ".xml");
    let notes_xml = zip_text(source_file, &part_name(&source_slide.notes_uri))?;
    editor.add_text_part(
        &new_notes_uri,
        remint_pptx_creation_ids(&notes_xml, &new_notes_uri),
        NOTES_CONTENT_TYPE,
    );

    let source_rels = source_relationship_entries(source_file, &source_slide.notes_uri);
    let mut new_notes_rels = Vec::new();
    for rel in source_rels {
        if rel.target_mode == "External" {
            new_notes_rels.push(RelationshipEntry {
                id: rel.id,
                rel_type: rel.rel_type,
                target: rel.target,
                target_mode: rel.target_mode,
            });
            continue;
        }
        let target_uri = if rel.rel_type == SLIDE_REL_TYPE {
            new_slide_uri.to_string()
        } else {
            let source_target = resolve_relationship_target(&source_slide.notes_uri, &rel.target);
            import_ctx.copy_dependency_tree(editor, &source_target)?
        };
        new_notes_rels.push(RelationshipEntry {
            id: rel.id,
            rel_type: rel.rel_type,
            target: relationship_target_from_source_to_target(&new_notes_uri, &target_uri),
            target_mode: String::new(),
        });
    }
    editor.add_relationships(&new_notes_uri, &new_notes_rels);
    Ok(Some((
        new_notes_uri.clone(),
        RelationshipEntry {
            id: slide_notes_rel.id.clone(),
            rel_type: NOTES_REL_TYPE.to_string(),
            target: relationship_target_from_source_to_target(new_slide_uri, &new_notes_uri),
            target_mode: String::new(),
        },
    )))
}

fn resolve_slide_layout_target(
    target_file: &str,
    source_file: &str,
    source_graph: &PptxGraph,
    editor: &mut PptxPackageEditor,
    import_ctx: &mut PartImportContext<'_>,
    source_layout_uri: &str,
    policies: &ImportPolicies,
) -> CliResult<String> {
    match policies.layout.as_str() {
        "reuse" => find_compatible_target_layout_structural(source_file, target_file, source_layout_uri)?
            .map(|layout| layout.part_uri)
            .ok_or_else(|| {
                CliError::unexpected(format!(
                    "layout-policy reuse requires an explicit compatible target layout; no exact match found for {source_layout_uri}"
                ))
            }),
        "import" => {
            if let Some(existing) =
                find_compatible_target_layout_structural(source_file, target_file, source_layout_uri)?
            {
                return Ok(existing.part_uri);
            }
            let source_layout = source_graph
                .layouts
                .iter()
                .find(|layout| layout.part_uri == source_layout_uri)
                .ok_or_else(|| CliError::unexpected(format!("layout not found: {source_layout_uri}")))?;
            let master = import_master_into_editor(
                target_file,
                source_file,
                editor,
                import_ctx,
                &source_layout.master_uri,
                &policies.theme,
            )?;
            master
                .layout_mappings
                .get(source_layout_uri)
                .cloned()
                .ok_or_else(|| CliError::unexpected(format!("imported layout missing for {source_layout_uri}")))
        }
        other => Err(CliError::unexpected(format!("unknown layout policy: {other}"))),
    }
}

fn import_layout_into_editor(
    target_file: &str,
    source_file: &str,
    editor: &mut PptxPackageEditor,
    import_ctx: &mut PartImportContext<'_>,
    selector: &str,
    theme_policy: &str,
) -> CliResult<ImportLayoutResult> {
    let source_graph = parse_pptx_graph(source_file)?;
    let source_layout = resolve_source_layout_selector(&source_graph, selector)?;
    if let Some(existing) =
        find_compatible_target_layout_exact(source_file, target_file, &source_layout.part_uri)?
    {
        return Ok(ImportLayoutResult {
            target_layout_uri: existing.part_uri,
            target_master_uri: existing.master_uri,
            theme_uri: existing.theme_uri,
            name: source_layout.name,
            imported: false,
            master_imported: false,
        });
    }

    let source_master = source_graph
        .masters
        .iter()
        .find(|master| master.part_uri == source_layout.master_uri)
        .cloned()
        .ok_or_else(|| CliError::unexpected("source layout has no slide master"))?;
    let imported_theme = import_theme_for_master(
        target_file,
        source_file,
        editor,
        import_ctx,
        &source_master,
        theme_policy,
    )?;
    let new_master_uri = allocate_numbered_part_uri(editor, "ppt/slideMasters/slideMaster", ".xml");
    let new_layout_uri = allocate_numbered_part_uri(editor, "ppt/slideLayouts/slideLayout", ".xml");

    let layout_xml = zip_text(source_file, &part_name(&source_layout.part_uri))?;
    editor.add_text_part(
        &new_layout_uri,
        remint_pptx_creation_ids(&layout_xml, &new_layout_uri),
        LAYOUT_CONTENT_TYPE,
    );
    let layout_rels = source_relationship_entries(source_file, &source_layout.part_uri);
    let mut new_layout_rels = Vec::new();
    for rel in layout_rels {
        if rel.target_mode == "External" {
            new_layout_rels.push(rel);
            continue;
        }
        let target_uri = if rel.rel_type == SLIDE_MASTER_REL_TYPE {
            new_master_uri.clone()
        } else {
            let source_target = resolve_relationship_target(&source_layout.part_uri, &rel.target);
            import_ctx.copy_dependency_tree(editor, &source_target)?
        };
        new_layout_rels.push(RelationshipEntry {
            id: rel.id,
            rel_type: rel.rel_type,
            target: relationship_target_from_source_to_target(&new_layout_uri, &target_uri),
            target_mode: String::new(),
        });
    }
    editor.add_relationships(&new_layout_uri, &new_layout_rels);

    let source_master_rels = source_relationship_entries(source_file, &source_master.part_uri);
    let selected_layout_rel = source_master_rels
        .iter()
        .find(|rel| {
            rel.rel_type == SLIDE_LAYOUT_REL_TYPE
                && resolve_relationship_target(&source_master.part_uri, &rel.target)
                    == source_layout.part_uri
        })
        .cloned()
        .ok_or_else(|| {
            CliError::unexpected("selected layout relationship not found in source master")
        })?;
    let source_master_xml = zip_text(source_file, &part_name(&source_master.part_uri))?;
    let master_xml = remint_master_layout_ids(
        &remint_pptx_creation_ids(
            &prune_master_layout_list(&source_master_xml, &selected_layout_rel.id)?,
            &new_master_uri,
        ),
        &new_master_uri,
    );
    editor.add_text_part(&new_master_uri, master_xml, MASTER_CONTENT_TYPE);
    let mut new_master_rels = Vec::new();
    for rel in source_master_rels {
        if rel.target_mode == "External" {
            new_master_rels.push(rel);
            continue;
        }
        if rel.rel_type == SLIDE_LAYOUT_REL_TYPE {
            if rel.id == selected_layout_rel.id {
                new_master_rels.push(RelationshipEntry {
                    id: rel.id,
                    rel_type: rel.rel_type,
                    target: relationship_target_from_source_to_target(
                        &new_master_uri,
                        &new_layout_uri,
                    ),
                    target_mode: String::new(),
                });
            }
            continue;
        }
        let target_uri = if rel.rel_type == THEME_REL_TYPE {
            imported_theme.clone()
        } else {
            let source_target = resolve_relationship_target(&source_master.part_uri, &rel.target);
            import_ctx.copy_dependency_tree(editor, &source_target)?
        };
        new_master_rels.push(RelationshipEntry {
            id: rel.id,
            rel_type: rel.rel_type,
            target: relationship_target_from_source_to_target(&new_master_uri, &target_uri),
            target_mode: String::new(),
        });
    }
    editor.add_relationships(&new_master_uri, &new_master_rels);
    register_imported_master(editor, &new_master_uri)?;

    Ok(ImportLayoutResult {
        target_layout_uri: new_layout_uri,
        target_master_uri: new_master_uri,
        theme_uri: imported_theme,
        name: source_layout.name,
        imported: true,
        master_imported: true,
    })
}

fn import_master_into_editor(
    target_file: &str,
    source_file: &str,
    editor: &mut PptxPackageEditor,
    import_ctx: &mut PartImportContext<'_>,
    source_master_uri: &str,
    theme_policy: &str,
) -> CliResult<ImportMasterResult> {
    let source_graph = parse_pptx_graph(source_file)?;
    let source_master = source_graph
        .masters
        .iter()
        .find(|master| master.part_uri == source_master_uri)
        .cloned()
        .ok_or_else(|| CliError::unexpected(format!("master not found: {source_master_uri}")))?;
    if let Some(existing) =
        find_compatible_target_master_exact(source_file, target_file, &source_master.part_uri)?
    {
        return Ok(existing);
    }

    let imported_theme = import_theme_for_master(
        target_file,
        source_file,
        editor,
        import_ctx,
        &source_master,
        theme_policy,
    )?;
    let new_master_uri = allocate_numbered_part_uri(editor, "ppt/slideMasters/slideMaster", ".xml");
    let source_master_rels = source_relationship_entries(source_file, &source_master.part_uri);
    let mut layout_mappings = BTreeMap::new();

    for rel in source_master_rels
        .iter()
        .filter(|rel| rel.rel_type == SLIDE_LAYOUT_REL_TYPE && rel.target_mode != "External")
    {
        let source_layout_uri = resolve_relationship_target(&source_master.part_uri, &rel.target);
        let new_layout_uri =
            allocate_numbered_part_uri(editor, "ppt/slideLayouts/slideLayout", ".xml");
        let layout_xml = zip_text(source_file, &part_name(&source_layout_uri))?;
        editor.add_text_part(
            &new_layout_uri,
            remint_pptx_creation_ids(&layout_xml, &new_layout_uri),
            LAYOUT_CONTENT_TYPE,
        );
        let layout_rels = source_relationship_entries(source_file, &source_layout_uri);
        let mut new_layout_rels = Vec::new();
        for layout_rel in layout_rels {
            if layout_rel.target_mode == "External" {
                new_layout_rels.push(layout_rel);
                continue;
            }
            let target_uri = if layout_rel.rel_type == SLIDE_MASTER_REL_TYPE {
                new_master_uri.clone()
            } else {
                let source_target =
                    resolve_relationship_target(&source_layout_uri, &layout_rel.target);
                import_ctx.copy_dependency_tree(editor, &source_target)?
            };
            new_layout_rels.push(RelationshipEntry {
                id: layout_rel.id,
                rel_type: layout_rel.rel_type,
                target: relationship_target_from_source_to_target(&new_layout_uri, &target_uri),
                target_mode: String::new(),
            });
        }
        editor.add_relationships(&new_layout_uri, &new_layout_rels);
        layout_mappings.insert(source_layout_uri, new_layout_uri);
    }

    let master_xml = zip_text(source_file, &part_name(&source_master.part_uri))?;
    editor.add_text_part(
        &new_master_uri,
        remint_master_layout_ids(
            &remint_pptx_creation_ids(&master_xml, &new_master_uri),
            &new_master_uri,
        ),
        MASTER_CONTENT_TYPE,
    );
    let mut new_master_rels = Vec::new();
    for rel in source_master_rels {
        if rel.target_mode == "External" {
            new_master_rels.push(rel);
            continue;
        }
        let source_target = resolve_relationship_target(&source_master.part_uri, &rel.target);
        let target_uri = match rel.rel_type.as_str() {
            SLIDE_LAYOUT_REL_TYPE => layout_mappings
                .get(&source_target)
                .cloned()
                .ok_or_else(|| CliError::unexpected("imported master layout mapping missing"))?,
            THEME_REL_TYPE => imported_theme.clone(),
            _ => import_ctx.copy_dependency_tree(editor, &source_target)?,
        };
        new_master_rels.push(RelationshipEntry {
            id: rel.id,
            rel_type: rel.rel_type,
            target: relationship_target_from_source_to_target(&new_master_uri, &target_uri),
            target_mode: String::new(),
        });
    }
    editor.add_relationships(&new_master_uri, &new_master_rels);
    register_imported_master(editor, &new_master_uri)?;

    Ok(ImportMasterResult {
        target_master_uri: new_master_uri,
        theme_uri: imported_theme,
        imported: true,
        layout_mappings,
    })
}

fn import_theme_for_master(
    target_file: &str,
    source_file: &str,
    editor: &mut PptxPackageEditor,
    import_ctx: &mut PartImportContext<'_>,
    source_master: &MasterRef,
    theme_policy: &str,
) -> CliResult<String> {
    if source_master.theme_uri.is_empty() {
        return Ok(String::new());
    }
    if theme_policy == "reuse"
        && let Some(theme_uri) =
            find_matching_theme_by_bytes(source_file, target_file, &source_master.theme_uri)?
    {
        return Ok(theme_uri);
    }
    import_ctx.copy_dependency_tree(editor, &source_master.theme_uri)
}

fn register_imported_master(editor: &mut PptxPackageEditor, master_uri: &str) -> CliResult<()> {
    let presentation_xml = editor.read_text("/ppt/presentation.xml")?;
    let pres_rels = editor.relationship_entries("/ppt/presentation.xml");
    let rel_id = allocate_relationship_id(&pres_rels);
    let master_id = next_presentation_master_id(&presentation_xml);
    let fragment = format!(
        r#"<p:sldMasterId id="{master_id}" r:id="{}"/>"#,
        xml_attr_escape(&rel_id)
    );
    let updated_presentation = insert_master_fragment(&presentation_xml, &fragment)?;
    editor.replace_text_part("/ppt/presentation.xml", updated_presentation);

    let mut updated_rels = pres_rels;
    updated_rels.push(RelationshipEntry {
        id: rel_id,
        rel_type: SLIDE_MASTER_REL_TYPE.to_string(),
        target: relationship_target_from_source_to_target("/ppt/presentation.xml", master_uri),
        target_mode: String::new(),
    });
    editor.add_relationships("/ppt/presentation.xml", &updated_rels);
    Ok(())
}

fn parse_pptx_graph(file: &str) -> CliResult<PptxGraph> {
    let presentation_xml = zip_text(file, "ppt/presentation.xml")?;
    let pres_rels = source_relationship_entries(file, "/ppt/presentation.xml");
    let slide_spans = slide_id_spans(&presentation_xml)?;
    let slides = slide_spans
        .into_iter()
        .map(|span| {
            let rel = pres_rels
                .iter()
                .find(|rel| rel.id == span.rel_id)
                .ok_or_else(|| {
                    CliError::unexpected(format!("missing relationship {}", span.rel_id))
                })?;
            let part_uri = resolve_relationship_target("/ppt/presentation.xml", &rel.target);
            let (layout_uri, notes_uri) = slide_related_parts(file, &part_uri);
            Ok(SlideRef {
                part_uri,
                layout_uri,
                notes_uri,
            })
        })
        .collect::<CliResult<Vec<_>>>()?;

    let mut masters = Vec::new();
    for (index, span) in master_id_spans(&presentation_xml)?.into_iter().enumerate() {
        let rel = pres_rels
            .iter()
            .find(|rel| rel.id == span.rel_id)
            .ok_or_else(|| CliError::unexpected(format!("missing relationship {}", span.rel_id)))?;
        let part_uri = resolve_relationship_target("/ppt/presentation.xml", &rel.target);
        let (layout_uris, theme_uri) = master_related_parts(file, &part_uri);
        masters.push(MasterRef {
            number: index + 1,
            part_uri,
            layout_uris,
            theme_uri,
        });
    }

    let mut layouts = Vec::new();
    for master in &masters {
        for layout_uri in &master.layout_uris {
            let layout_xml = zip_text(file, &part_name(layout_uri))?;
            let name = layout_name(&layout_xml);
            let master_uri =
                layout_master_part(file, layout_uri).unwrap_or_else(|| master.part_uri.clone());
            layouts.push(LayoutRef {
                name,
                part_uri: layout_uri.clone(),
                master_uri,
                theme_uri: master.theme_uri.clone(),
            });
        }
    }

    Ok(PptxGraph {
        slides,
        layouts,
        masters,
    })
}

fn slide_related_parts(file: &str, slide_uri: &str) -> (String, String) {
    let mut layout_uri = String::new();
    let mut notes_uri = String::new();
    for rel in source_relationship_entries(file, slide_uri) {
        match rel.rel_type.as_str() {
            SLIDE_LAYOUT_REL_TYPE => {
                layout_uri = resolve_relationship_target(slide_uri, &rel.target)
            }
            NOTES_REL_TYPE => notes_uri = resolve_relationship_target(slide_uri, &rel.target),
            _ => {}
        }
    }
    (layout_uri, notes_uri)
}

fn master_related_parts(file: &str, master_uri: &str) -> (Vec<String>, String) {
    let mut layout_uris = Vec::new();
    let mut theme_uri = String::new();
    for rel in source_relationship_entries(file, master_uri) {
        match rel.rel_type.as_str() {
            SLIDE_LAYOUT_REL_TYPE => {
                layout_uris.push(resolve_relationship_target(master_uri, &rel.target));
            }
            THEME_REL_TYPE => theme_uri = resolve_relationship_target(master_uri, &rel.target),
            _ => {}
        }
    }
    (layout_uris, theme_uri)
}

fn layout_master_part(file: &str, layout_uri: &str) -> Option<String> {
    source_relationship_entries(file, layout_uri)
        .into_iter()
        .find(|rel| rel.rel_type == SLIDE_MASTER_REL_TYPE && rel.target_mode != "External")
        .map(|rel| resolve_relationship_target(layout_uri, &rel.target))
}

fn source_relationship_entries(file: &str, source_uri: &str) -> Vec<RelationshipEntry> {
    zip_text(file, &relationships_part_for(source_uri))
        .map(|xml| relationship_entries_from_xml(&xml))
        .unwrap_or_default()
}

fn resolve_source_layout_selector(graph: &PptxGraph, selector: &str) -> CliResult<LayoutRef> {
    if let Ok(number) = selector.parse::<usize>()
        && number >= 1
    {
        return graph
            .layouts
            .get(number - 1)
            .cloned()
            .ok_or_else(|| CliError::invalid_args(format!("layout not found: {selector}")));
    }
    graph
        .layouts
        .iter()
        .find(|layout| layout.name == selector)
        .cloned()
        .ok_or_else(|| CliError::invalid_args(format!("layout not found: {selector}")))
}

fn find_compatible_target_layout_exact(
    source_file: &str,
    target_file: &str,
    source_layout_uri: &str,
) -> CliResult<Option<LayoutRef>> {
    find_compatible_target_layout(source_file, target_file, source_layout_uri, false)
}

fn find_compatible_target_layout_structural(
    source_file: &str,
    target_file: &str,
    source_layout_uri: &str,
) -> CliResult<Option<LayoutRef>> {
    find_compatible_target_layout(source_file, target_file, source_layout_uri, true)
}

fn find_compatible_target_layout(
    source_file: &str,
    target_file: &str,
    source_layout_uri: &str,
    ignore_layout_name: bool,
) -> CliResult<Option<LayoutRef>> {
    let source_graph = parse_pptx_graph(source_file)?;
    let target_graph = parse_pptx_graph(target_file)?;
    let source_layout = source_graph
        .layouts
        .iter()
        .find(|layout| layout.part_uri == source_layout_uri)
        .ok_or_else(|| CliError::unexpected(format!("layout not found: {source_layout_uri}")))?;
    let source_theme = source_layout.theme_uri.clone();
    let source_fp = part_fingerprint(
        source_file,
        &source_layout.part_uri,
        &[SLIDE_MASTER_REL_TYPE],
        ignore_layout_name,
        &mut BTreeMap::new(),
    )?;
    for target_layout in target_graph.layouts {
        if !same_part_bytes(
            source_file,
            &source_theme,
            target_file,
            &target_layout.theme_uri,
        )? {
            continue;
        }
        let target_fp = part_fingerprint(
            target_file,
            &target_layout.part_uri,
            &[SLIDE_MASTER_REL_TYPE],
            ignore_layout_name,
            &mut BTreeMap::new(),
        )?;
        if target_fp == source_fp {
            return Ok(Some(target_layout));
        }
    }
    Ok(None)
}

fn find_compatible_target_master_exact(
    source_file: &str,
    target_file: &str,
    source_master_uri: &str,
) -> CliResult<Option<ImportMasterResult>> {
    let source_graph = parse_pptx_graph(source_file)?;
    let target_graph = parse_pptx_graph(target_file)?;
    let source_master = source_graph
        .masters
        .iter()
        .find(|master| master.part_uri == source_master_uri)
        .ok_or_else(|| CliError::unexpected(format!("master not found: {source_master_uri}")))?;
    let source_fp = part_fingerprint(
        source_file,
        &source_master.part_uri,
        &[SLIDE_LAYOUT_REL_TYPE, THEME_REL_TYPE],
        false,
        &mut BTreeMap::new(),
    )?;

    for target_master in target_graph.masters {
        if source_master.layout_uris.len() != target_master.layout_uris.len() {
            continue;
        }
        if !same_part_bytes(
            source_file,
            &source_master.theme_uri,
            target_file,
            &target_master.theme_uri,
        )? {
            continue;
        }
        let target_fp = part_fingerprint(
            target_file,
            &target_master.part_uri,
            &[SLIDE_LAYOUT_REL_TYPE, THEME_REL_TYPE],
            false,
            &mut BTreeMap::new(),
        )?;
        if target_fp != source_fp {
            continue;
        }
        let mut mapping = BTreeMap::new();
        let mut used_targets = BTreeSet::new();
        for source_layout_uri in &source_master.layout_uris {
            let Some(target_layout) =
                find_compatible_target_layout_exact(source_file, target_file, source_layout_uri)?
            else {
                mapping.clear();
                break;
            };
            if !target_master.layout_uris.contains(&target_layout.part_uri)
                || !used_targets.insert(target_layout.part_uri.clone())
            {
                mapping.clear();
                break;
            }
            mapping.insert(source_layout_uri.clone(), target_layout.part_uri);
        }
        if mapping.len() == source_master.layout_uris.len() {
            return Ok(Some(ImportMasterResult {
                target_master_uri: target_master.part_uri,
                theme_uri: target_master.theme_uri,
                imported: false,
                layout_mappings: mapping,
            }));
        }
    }
    Ok(None)
}

fn find_matching_theme_by_bytes(
    source_file: &str,
    target_file: &str,
    source_theme_uri: &str,
) -> CliResult<Option<String>> {
    if source_theme_uri.is_empty() {
        return Ok(None);
    }
    let source_bytes = zip_bytes(source_file, &part_name(source_theme_uri))?;
    for master in parse_pptx_graph(target_file)?.masters {
        if master.theme_uri.is_empty() {
            continue;
        }
        if zip_bytes(target_file, &part_name(&master.theme_uri)).unwrap_or_default() == source_bytes
        {
            return Ok(Some(master.theme_uri));
        }
    }
    Ok(None)
}

fn find_exact_target_part(
    editor: &PptxPackageEditor,
    source_bytes: &[u8],
    source_content_type: &str,
) -> CliResult<Option<String>> {
    for entry in &editor.entries {
        if entry.ends_with(".rels") || entry == "[Content_Types].xml" {
            continue;
        }
        let uri = package_uri(entry);
        if editor.content_type(&uri) != source_content_type {
            continue;
        }
        if editor.read_bytes(&uri)? == source_bytes {
            return Ok(Some(uri));
        }
    }
    Ok(None)
}

fn part_fingerprint(
    file: &str,
    part_uri: &str,
    ignore_rel_types: &[&str],
    ignore_layout_name: bool,
    memo: &mut BTreeMap<String, String>,
) -> CliResult<String> {
    let part_uri = package_uri(part_uri);
    let memo_key = format!(
        "{}|{}|{}",
        part_uri,
        ignore_rel_types.join(","),
        ignore_layout_name
    );
    if let Some(value) = memo.get(&memo_key) {
        return Ok(value.clone());
    }
    let bytes = zip_bytes(file, &part_name(&part_uri))?;
    let content_type = content_type_for_part(file, &part_uri)?;
    let mut rel_fps = Vec::new();
    for rel in source_relationship_entries(file, &part_uri) {
        if ignore_rel_types.contains(&rel.rel_type.as_str()) {
            continue;
        }
        if rel.target_mode == "External" {
            rel_fps.push(format!(
                "{}|{}|external|{}",
                rel.rel_type, rel.target_mode, rel.target
            ));
        } else {
            let target_uri = resolve_relationship_target(&part_uri, &rel.target);
            let target_fp = part_fingerprint(file, &target_uri, &[], ignore_layout_name, memo)?;
            rel_fps.push(format!(
                "{}|{}|{}",
                rel.rel_type, rel.target_mode, target_fp
            ));
        }
    }
    rel_fps.sort();
    let mut hasher = Sha256::new();
    hasher.update(content_type.as_bytes());
    hasher.update([0]);
    if ignore_layout_name && part_uri.starts_with("/ppt/slideLayouts/") {
        hasher.update(normalize_layout_name_for_fingerprint(&bytes).as_bytes());
    } else {
        hasher.update(&bytes);
    }
    for rel_fp in rel_fps {
        hasher.update([0]);
        hasher.update(rel_fp.as_bytes());
    }
    let value = format!("{:x}", hasher.finalize());
    memo.insert(memo_key, value.clone());
    Ok(value)
}

fn same_part_bytes(
    left_file: &str,
    left_uri: &str,
    right_file: &str,
    right_uri: &str,
) -> CliResult<bool> {
    if left_uri.is_empty() || right_uri.is_empty() {
        return Ok(left_uri.is_empty() && right_uri.is_empty());
    }
    Ok(
        zip_bytes(left_file, &part_name(left_uri))?
            == zip_bytes(right_file, &part_name(right_uri))?,
    )
}

fn stage_editor(
    file: &str,
    editor: PptxPackageEditor,
    options: &PptxImportMutationOptions,
) -> CliResult<String> {
    let write_path = stage_path_for_options(file, options)?;
    copy_zip_with_binary_part_overrides_and_removals(
        file,
        &write_path,
        &editor.text_overrides,
        &editor.binary_overrides,
        &BTreeSet::new(),
    )?;
    if !options.no_validate {
        validate(&write_path, true)?;
    }
    Ok(write_path)
}

fn stage_path_for_options(file: &str, options: &PptxImportMutationOptions) -> CliResult<String> {
    let output_path = options
        .out
        .as_deref()
        .filter(|value| !value.trim().is_empty());
    if options.dry_run || options.in_place || output_path == Some(file) {
        Ok(package_mutation_temp_path(file, "pptx-import"))
    } else {
        output_path
            .ok_or_else(|| {
                CliError::invalid_args(
                    "must specify exactly one of --out, --in-place, or --dry-run",
                )
            })
            .map(ToString::to_string)
    }
}

fn finish_import_mutation(
    file: &str,
    staged_path: &str,
    options: &PptxImportMutationOptions,
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

fn mutation_output_path(file: &str, options: &PptxImportMutationOptions) -> Option<String> {
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

fn parse_import_mutation_options(args: &[String]) -> CliResult<PptxImportMutationOptions> {
    let out = parse_string_flag(args, "--out")?;
    let backup = parse_string_flag(args, "--backup")?;
    let dry_run = has_flag(args, "--dry-run");
    let in_place = has_flag(args, "--in-place");
    let no_validate = has_flag(args, "--no-validate");
    validate_xlsx_mutation_output_flags(out.as_deref(), in_place, backup.as_deref(), dry_run)?;
    Ok(PptxImportMutationOptions {
        out,
        backup,
        dry_run,
        in_place,
        no_validate,
    })
}

fn policy_flag(args: &[String], flag: &str, default: &str, kind: &str) -> CliResult<String> {
    let value = parse_string_flag(args, flag)?
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| default.to_string());
    if value != "reuse" && value != "import" {
        return Err(CliError::unexpected(format!(
            "unknown {kind} policy: {value}"
        )));
    }
    Ok(value)
}

fn notes_policy_flag(args: &[String]) -> CliResult<String> {
    let value = parse_string_flag(args, "--notes-policy")?
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "drop".to_string());
    if value != "drop" && value != "clone" {
        return Err(CliError::unexpected(format!(
            "unknown notes policy: {value}"
        )));
    }
    Ok(value)
}

fn required_string_flag(args: &[String], name: &str) -> CliResult<String> {
    let value = parse_string_flag(args, name)?
        .ok_or_else(|| CliError::invalid_args(format!("{name} must be specified")))?;
    if value.trim().is_empty() {
        return Err(CliError::invalid_args(format!("{name} must be specified")));
    }
    Ok(value)
}

fn ensure_pptx_package(file: &str) -> CliResult<()> {
    let detected = package_type(file)?;
    if detected != "pptx" {
        return Err(CliError::unsupported_type(format!(
            "file is not a PPTX document (detected: {detected})"
        )));
    }
    Ok(())
}

fn slide_id_spans(xml: &str) -> CliResult<Vec<SlideIdSpan>> {
    id_spans(xml, "sldId")
}

fn master_id_spans(xml: &str) -> CliResult<Vec<SlideIdSpan>> {
    id_spans(xml, "sldMasterId")
}

fn id_spans(xml: &str, local: &str) -> CliResult<Vec<SlideIdSpan>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut spans = Vec::new();
    let mut current: Option<(usize, String, u32, usize)> = None;
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == local => {
                if let Some(span) =
                    id_span_from_attrs(before, reader.buffer_position() as usize, &e)
                {
                    spans.push(span);
                }
            }
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == local => {
                if let (Some(id), Some(rel_id)) = (attr_exact(&e, "id"), attr_exact(&e, "r:id"))
                    && let Ok(id) = id.parse::<u32>()
                {
                    current = Some((before, rel_id, id, 1));
                }
            }
            Ok(Event::Start(_)) => {
                if let Some((_, _, _, depth)) = current.as_mut() {
                    *depth += 1;
                }
            }
            Ok(Event::End(e)) => {
                if let Some((start, rel_id, id, depth)) = current.as_mut() {
                    if *depth == 1 && local_name(e.name().as_ref()) == local {
                        spans.push(SlideIdSpan {
                            rel_id: rel_id.clone(),
                            id: *id,
                            start: *start,
                            end: reader.buffer_position() as usize,
                        });
                        current = None;
                    } else {
                        *depth = depth.saturating_sub(1);
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(spans)
}

fn id_span_from_attrs(start: usize, end: usize, e: &BytesStart<'_>) -> Option<SlideIdSpan> {
    let id = attr_exact(e, "id")?.parse::<u32>().ok()?;
    let rel_id = attr_exact(e, "r:id")?;
    Some(SlideIdSpan {
        rel_id,
        id,
        start,
        end,
    })
}

fn insert_slide_fragment(
    presentation_xml: &str,
    insert_after: usize,
    new_fragment: &str,
) -> CliResult<String> {
    let refs = slide_id_spans(presentation_xml)?;
    if refs.is_empty() {
        return insert_new_slide_list(presentation_xml, new_fragment);
    }
    if insert_after > refs.len() {
        return Err(CliError::unexpected(format!(
            "insert-after {insert_after} out of range for target with {} slides",
            refs.len()
        )));
    }
    let insert_at = if insert_after == refs.len() {
        refs.last()
            .map(|span| span.end)
            .unwrap_or(presentation_xml.len())
    } else {
        refs.get(insert_after)
            .map(|span| span.start)
            .unwrap_or(presentation_xml.len())
    };
    Ok(insert_xml_at(presentation_xml, insert_at, new_fragment))
}

fn insert_new_slide_list(presentation_xml: &str, new_fragment: &str) -> CliResult<String> {
    let list = format!("<p:sldIdLst>{new_fragment}</p:sldIdLst>");
    if let Some(master_list) = find_first_element_span(presentation_xml, "sldMasterIdLst")? {
        return Ok(insert_xml_at(presentation_xml, master_list.end, &list));
    }
    if let Some(close) = presentation_xml.rfind("</") {
        return Ok(insert_xml_at(presentation_xml, close, &list));
    }
    Err(CliError::unexpected("invalid presentation XML"))
}

fn insert_master_fragment(presentation_xml: &str, fragment: &str) -> CliResult<String> {
    if let Some(list) = find_first_element_span(presentation_xml, "sldMasterIdLst")? {
        let (_, content_end) = element_content_bounds(&presentation_xml[list.start..list.end])?;
        return Ok(insert_xml_at(
            presentation_xml,
            list.start + content_end,
            fragment,
        ));
    }
    let list = format!("<p:sldMasterIdLst>{fragment}</p:sldMasterIdLst>");
    if let Some(slide_list) = find_first_element_span(presentation_xml, "sldIdLst")? {
        return Ok(insert_xml_at(presentation_xml, slide_list.start, &list));
    }
    if let Some(close) = presentation_xml.rfind("</") {
        return Ok(insert_xml_at(presentation_xml, close, &list));
    }
    Err(CliError::unexpected("invalid presentation XML"))
}

fn prune_master_layout_list(master_xml: &str, keep_rel_id: &str) -> CliResult<String> {
    let mut out = master_xml.to_string();
    for span in layout_id_spans(master_xml)?
        .into_iter()
        .rev()
        .filter(|span| span.rel_id != keep_rel_id)
    {
        out = replace_xml_span(&out, span.start, span.end, "");
    }
    Ok(out)
}

fn layout_id_spans(xml: &str) -> CliResult<Vec<SlideIdSpan>> {
    id_spans(xml, "sldLayoutId")
}

fn next_presentation_slide_id(xml: &str) -> u32 {
    slide_id_spans(xml)
        .unwrap_or_default()
        .into_iter()
        .map(|span| span.id)
        .max()
        .unwrap_or(255)
        .saturating_add(1)
}

fn next_presentation_master_id(xml: &str) -> u32 {
    const BASE: u32 = 2_147_483_648;
    let max_id = master_id_spans(xml)
        .unwrap_or_default()
        .into_iter()
        .map(|span| span.id)
        .max()
        .unwrap_or(0);
    if max_id < BASE {
        BASE
    } else {
        max_id.saturating_add(1)
    }
}

fn layout_name(xml: &str) -> String {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "cSld" =>
            {
                return attr(&e, "name").unwrap_or_default();
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    String::new()
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

fn allocate_imported_part_uri(
    editor: &PptxPackageEditor,
    source_uri: &str,
    content_type: &str,
) -> String {
    let source_part = part_name(source_uri);
    if source_part.starts_with("ppt/slideLayouts/slideLayout") && source_part.ends_with(".xml") {
        return allocate_numbered_part_uri(editor, "ppt/slideLayouts/slideLayout", ".xml");
    }
    if source_part.starts_with("ppt/slideMasters/slideMaster") && source_part.ends_with(".xml") {
        return allocate_numbered_part_uri(editor, "ppt/slideMasters/slideMaster", ".xml");
    }
    if source_part.starts_with("ppt/theme/theme") && source_part.ends_with(".xml") {
        return allocate_numbered_part_uri(editor, "ppt/theme/theme", ".xml");
    }
    if source_part.starts_with("ppt/media/") {
        return allocate_numbered_part_uri(
            editor,
            &format!("ppt/media/{}", media_stem(content_type)),
            Path::new(&source_part)
                .extension()
                .and_then(|value| value.to_str())
                .filter(|value| !value.is_empty())
                .map(|value| format!(".{value}"))
                .unwrap_or_default()
                .as_str(),
        );
    }
    if !editor.has_part(source_uri) {
        return package_uri(source_uri);
    }
    allocate_sibling_import_uri(editor, source_uri)
}

fn allocate_numbered_part_uri(editor: &PptxPackageEditor, prefix: &str, suffix: &str) -> String {
    let mut next = 1_u32;
    for entry in &editor.entries {
        let normalized = entry.trim_start_matches('/');
        if let Some(raw) = normalized
            .strip_prefix(prefix)
            .and_then(|tail| tail.strip_suffix(suffix))
            && let Ok(value) = raw.parse::<u32>()
            && value >= next
        {
            next = value + 1;
        }
    }
    package_uri(&format!("{prefix}{next}{suffix}"))
}

fn allocate_sibling_import_uri(editor: &PptxPackageEditor, source_uri: &str) -> String {
    let source_part = part_name(source_uri);
    let path = Path::new(&source_part);
    let parent = path
        .parent()
        .map(|value| value.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default();
    let stem = path
        .file_stem()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "part".to_string());
    let suffix = path
        .extension()
        .map(|value| format!(".{}", value.to_string_lossy()))
        .unwrap_or_default();
    for index in 1.. {
        let name = if parent.is_empty() {
            format!("{stem}-import{index}{suffix}")
        } else {
            format!("{parent}/{stem}-import{index}{suffix}")
        };
        if !editor.entries.contains(&name) {
            return package_uri(&name);
        }
    }
    unreachable!()
}

fn media_stem(content_type: &str) -> &'static str {
    if content_type.starts_with("image/") {
        "image"
    } else if content_type.starts_with("video/") {
        "video"
    } else if content_type.starts_with("audio/") {
        "audio"
    } else {
        "media"
    }
}

fn content_type_from_xml(xml: &str, part_uri: &str) -> String {
    let normalized = part_name(part_uri);
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut defaults = BTreeMap::new();
    let mut overrides = BTreeMap::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "Default" =>
            {
                if let (Some(extension), Some(content_type)) =
                    (attr(&e, "Extension"), attr(&e, "ContentType"))
                {
                    defaults.insert(extension, content_type);
                }
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "Override" =>
            {
                if let (Some(part_name), Some(content_type)) =
                    (attr(&e, "PartName"), attr(&e, "ContentType"))
                {
                    overrides.insert(part_name.trim_start_matches('/').to_string(), content_type);
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    if let Some(content_type) = overrides.get(&normalized) {
        return content_type.clone();
    }
    let extension = Path::new(&normalized)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    defaults.get(extension).cloned().unwrap_or_default()
}

fn render_relationships_xml(rels: &[RelationshipEntry]) -> String {
    let mut out = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
    );
    for rel in rels {
        let target_mode = if rel.target_mode.is_empty() {
            String::new()
        } else {
            format!(r#" TargetMode="{}""#, xml_attr_escape(&rel.target_mode))
        };
        out.push_str(&format!(
            r#"<Relationship Id="{}" Type="{}" Target="{}"{} />"#,
            xml_attr_escape(&rel.id),
            xml_attr_escape(&rel.rel_type),
            xml_attr_escape(&rel.target),
            target_mode
        ));
    }
    out.push_str("</Relationships>");
    out
}

fn remint_pptx_creation_ids(xml: &str, scope: &str) -> String {
    let mut reminted = xml.to_string();
    static CREATION_ID_RE: OnceLock<Regex> = OnceLock::new();
    let re = CREATION_ID_RE.get_or_init(|| {
        Regex::new(r#"(<[^>]*\bcreationId\b[^>]*\bval=")([^"]*)(")"#)
            .expect("valid creationId regex")
    });
    if reminted.contains("creationId") {
        let mut index = 0_u64;
        reminted = re
            .replace_all(&reminted, |captures: &regex::Captures<'_>| {
                index += 1;
                format!(
                    "{}{}{}",
                    &captures[1],
                    minted_creation_id(scope, index),
                    &captures[3]
                )
            })
            .into_owned();
    }
    if reminted.contains(":fld") || reminted.contains("<fld") {
        reminted = remint_pptx_field_ids(&reminted, scope);
    }
    reminted
}

fn normalize_layout_name_for_fingerprint(bytes: &[u8]) -> String {
    let xml = String::from_utf8_lossy(bytes);
    static LAYOUT_NAME_RE: OnceLock<Regex> = OnceLock::new();
    let re = LAYOUT_NAME_RE.get_or_init(|| {
        Regex::new(r#"(<p:cSld\b[^>]*\bname=")([^"]*)(")"#).expect("valid slide layout name regex")
    });
    re.replace(&xml, "${1}${3}").into_owned()
}

fn remint_pptx_field_ids(xml: &str, scope: &str) -> String {
    static FIELD_ID_RE: OnceLock<Regex> = OnceLock::new();
    let re = FIELD_ID_RE.get_or_init(|| {
        Regex::new(r#"(<[^>]*\bfld\b[^>]*\bid=")([^"]+)(")"#).expect("valid field id regex")
    });
    let mut index = 0_u64;
    re.replace_all(xml, |captures: &regex::Captures<'_>| {
        index += 1;
        format!(
            "{}{}{}",
            &captures[1],
            minted_guid(scope, index),
            &captures[3]
        )
    })
    .into_owned()
}

fn remint_master_layout_ids(xml: &str, scope: &str) -> String {
    if !xml.contains("sldLayoutId") {
        return xml.to_string();
    }
    static LAYOUT_ID_RE: OnceLock<Regex> = OnceLock::new();
    let re = LAYOUT_ID_RE.get_or_init(|| {
        Regex::new(r#"(<[^>]*\bsldLayoutId\b[^>]*\s+id=")([^"]*)(")"#)
            .expect("valid slide layout id regex")
    });
    let mut index = 10_000_u64;
    re.replace_all(xml, |captures: &regex::Captures<'_>| {
        index += 1;
        format!(
            "{}{}{}",
            &captures[1],
            minted_master_layout_id(scope, index),
            &captures[3]
        )
    })
    .into_owned()
}

fn minted_master_layout_id(scope: &str, index: u64) -> String {
    const BASE: u64 = 2_147_483_648;
    const WIDTH: u64 = 2_147_483_647;
    let mut hasher = Sha256::new();
    hasher.update(scope.as_bytes());
    hasher.update([0]);
    hasher.update(index.to_le_bytes());
    hasher.update([0]);
    hasher.update(chrono_like_counter().to_le_bytes());
    let digest = hasher.finalize();
    let raw = u32::from_le_bytes([digest[0], digest[1], digest[2], digest[3]]) as u64;
    (BASE + (raw % WIDTH)).to_string()
}

fn minted_creation_id(scope: &str, index: u64) -> String {
    let mut hasher = Sha256::new();
    hasher.update(scope.as_bytes());
    hasher.update([0]);
    hasher.update(index.to_le_bytes());
    hasher.update([0]);
    hasher.update(chrono_like_counter().to_le_bytes());
    let digest = hasher.finalize();
    let mut raw = u32::from_le_bytes([digest[0], digest[1], digest[2], digest[3]]);
    if raw == 0 {
        raw = 1;
    }
    raw.to_string()
}

fn minted_guid(scope: &str, index: u64) -> String {
    let mut hasher = Sha256::new();
    hasher.update(scope.as_bytes());
    hasher.update([0x47]);
    hasher.update(index.to_le_bytes());
    hasher.update([0]);
    hasher.update(chrono_like_counter().to_le_bytes());
    let digest = hasher.finalize();
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    format!(
        "{{{:02X}{:02X}{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}}}",
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        bytes[4],
        bytes[5],
        bytes[6],
        bytes[7],
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15]
    )
}

fn insert_xml_at(xml: &str, index: usize, insert: &str) -> String {
    let mut out = String::with_capacity(xml.len() + insert.len());
    out.push_str(&xml[..index]);
    out.push_str(insert);
    out.push_str(&xml[index..]);
    out
}

fn part_name(uri: &str) -> String {
    uri.trim_start_matches('/').to_string()
}

fn package_uri(part: &str) -> String {
    format!("/{}", part.trim_start_matches('/'))
}
