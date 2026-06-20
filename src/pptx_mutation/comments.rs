use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;

use crate::cli_args::value_flag_present;
use crate::{
    CliError, CliResult, RelationshipEntry, add_relationship_to_xml, allocate_relationship_id,
    attr, attr_exact, command_arg, copy_zip_with_part_overrides_and_removals, current_utc_rfc3339,
    decode_xml_text, ensure_content_type_override, local_name, package_mutation_temp_path,
    package_type, relationship_entries_from_xml, relationship_target_from_source_to_target,
    relationships_part_for, remove_xml_span, replace_xml_span, resolve_relationship_target,
    validate, validate_xlsx_mutation_output_flags, xml_attr_escape, xml_direct_child_ranges,
    xml_escape, zip_entry_exists, zip_entry_names, zip_text,
};

const COMMENTS_REL_TYPE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments";
const COMMENT_AUTHORS_REL_TYPE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/commentAuthors";
const COMMENTS_CONTENT_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.comments+xml";
const COMMENT_AUTHORS_CONTENT_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.commentAuthors+xml";
const COMMENT_AUTHORS_PART: &str = "ppt/commentAuthors.xml";
const PRESENTATION_PART: &str = "ppt/presentation.xml";

#[derive(Clone)]
struct PptxCommentMutationOptions {
    out: Option<String>,
    backup: Option<String>,
    dry_run: bool,
    in_place: bool,
    no_validate: bool,
}

#[derive(Clone)]
struct PptxSlideRef {
    number: u32,
    slide_id: u32,
    part: String,
}

#[derive(Clone)]
struct CommentAuthor {
    id: i64,
    name: String,
    initials: String,
    last_idx: i64,
    clr_idx: String,
}

#[derive(Clone)]
struct CommentElement {
    start: usize,
    end: usize,
    id: i64,
    author_id: i64,
    date: String,
    text: String,
    pos_x: String,
    pos_y: String,
}

#[derive(Clone)]
struct CommentSnapshot {
    author_id: i64,
    author: String,
    initials: String,
    date: String,
    text: String,
    content_hash: String,
}

struct AddCommentMutation {
    result: AddCommentResult,
    overrides: BTreeMap<String, String>,
    removals: BTreeSet<String>,
}

struct EditCommentMutation {
    result: EditCommentResult,
    overrides: BTreeMap<String, String>,
    removals: BTreeSet<String>,
}

struct RemoveCommentMutation {
    result: RemoveCommentResult,
    overrides: BTreeMap<String, String>,
    removals: BTreeSet<String>,
}

struct AddCommentResult {
    slide: u32,
    slide_id: u32,
    slide_part_uri: String,
    comments_part: String,
    comment_id: i64,
    author_id: i64,
    author: String,
    initials: String,
    date: String,
    text: String,
    content_hash: String,
    created_part: bool,
    created_relationship: bool,
    created_authors_part: bool,
    created_author: bool,
}

struct EditCommentResult {
    slide: u32,
    slide_id: u32,
    slide_part_uri: String,
    comments_part: String,
    comment_id: i64,
    author_id: i64,
    author: String,
    initials: String,
    date: String,
    text: String,
    content_hash: String,
    previous_text: String,
    previous_hash: String,
}

struct RemoveCommentResult {
    slide: u32,
    slide_id: u32,
    slide_part_uri: String,
    comments_part: String,
    comment_id: i64,
    author_id: i64,
    previous_author: String,
    previous_text: String,
    previous_hash: String,
    removed_part: bool,
}

pub(crate) fn pptx_comments_add(file: &str, args: &[String]) -> CliResult<Value> {
    let slide = crate::parse_i64_flag(args, "--slide")?.unwrap_or(0);
    if slide < 1 {
        return Err(CliError::invalid_args("--slide must be >= 1"));
    }
    let author = crate::parse_string_flag(args, "--author")?.unwrap_or_default();
    if author.is_empty() {
        return Err(CliError::invalid_args("--author is required"));
    }
    let initials = crate::parse_string_flag(args, "--initials")?.unwrap_or_default();
    let text = resolve_comment_text(args)?.unwrap_or_default();
    let date = if value_flag_present(args, "--date") {
        crate::parse_string_flag(args, "--date")?.unwrap_or_default()
    } else {
        current_utc_rfc3339()
    };
    let options = parse_comment_mutation_options(args)?;
    let detected = package_type(file)?;
    if detected != "pptx" {
        return Err(CliError::unsupported_type(format!(
            "file is not a PPTX document (detected: {detected})"
        )));
    }
    let mutation =
        build_add_comment_mutation(file, slide as u32, &author, &initials, &date, &text)?;
    write_comment_mutation(file, &mutation.overrides, &mutation.removals, &options)?;
    Ok(add_comment_result_json(file, &mutation.result, &options))
}

pub(crate) fn pptx_comments_edit(file: &str, args: &[String]) -> CliResult<Value> {
    let handle_given = value_flag_present(args, "--handle");
    let slide_given = value_flag_present(args, "--slide");
    let comment_given = value_flag_present(args, "--comment-id");
    let author_id_given = value_flag_present(args, "--author-id");
    if handle_given && (slide_given || comment_given || author_id_given) {
        return Err(CliError::invalid_args(
            "cannot specify --handle with --slide, --comment-id, or --author-id",
        ));
    }
    let mut slide = crate::parse_i64_flag(args, "--slide")?.unwrap_or(0);
    let mut comment_id = crate::parse_i64_flag(args, "--comment-id")?.unwrap_or(0);
    let mut author_id = crate::parse_i64_flag(args, "--author-id")?.unwrap_or(0);
    let mut author_id_set = author_id_given;
    if !handle_given && slide < 1 {
        return Err(CliError::invalid_args("--slide must be >= 1"));
    }
    if !handle_given && !comment_given {
        return Err(CliError::invalid_args(
            "either --handle or --comment-id is required",
        ));
    }
    if !handle_given && comment_id < 0 {
        return Err(CliError::invalid_args("--comment-id must be >= 0"));
    }
    if author_id_set && author_id < 0 {
        return Err(CliError::invalid_args("--author-id must be >= 0"));
    }
    let text_set = value_flag_present(args, "--text") || value_flag_present(args, "--text-file");
    let author_set = value_flag_present(args, "--author");
    let date_set = value_flag_present(args, "--date");
    if !text_set && !author_set && !date_set {
        return Err(CliError::invalid_args(
            "specify at least one of --text, --text-file, --author, or --date",
        ));
    }
    let text = resolve_comment_text(args)?.unwrap_or_default();
    let author = crate::parse_string_flag(args, "--author")?.unwrap_or_default();
    let date = crate::parse_string_flag(args, "--date")?.unwrap_or_default();
    let expect_hash = crate::parse_string_flag(args, "--expect-hash")?.unwrap_or_default();
    let handle = crate::parse_string_flag(args, "--handle")?.unwrap_or_default();
    let options = parse_comment_mutation_options(args)?;
    let detected = package_type(file)?;
    if detected != "pptx" {
        return Err(CliError::unsupported_type(format!(
            "file is not a PPTX document (detected: {detected})"
        )));
    }
    if handle_given {
        let resolved = resolve_comment_handle_target(file, &handle)?;
        slide = resolved.0 as i64;
        comment_id = resolved.1;
        author_id = resolved.2;
        author_id_set = true;
    }
    let mutation = build_edit_comment_mutation(
        file,
        EditCommentSpec {
            slide: slide as u32,
            comment_id,
            author_id,
            author_id_set,
            expect_hash: &expect_hash,
            text: &text,
            text_set,
            author: &author,
            author_set,
            date: &date,
            date_set,
        },
    )?;
    write_comment_mutation(file, &mutation.overrides, &mutation.removals, &options)?;
    Ok(edit_comment_result_json(file, &mutation.result, &options))
}

pub(crate) fn pptx_comments_remove(file: &str, args: &[String]) -> CliResult<Value> {
    let handle_given = value_flag_present(args, "--handle");
    let slide_given = value_flag_present(args, "--slide");
    let comment_given = value_flag_present(args, "--comment-id");
    let author_id_given = value_flag_present(args, "--author-id");
    if handle_given && (slide_given || comment_given || author_id_given) {
        return Err(CliError::invalid_args(
            "cannot specify --handle with --slide, --comment-id, or --author-id",
        ));
    }
    let mut slide = crate::parse_i64_flag(args, "--slide")?.unwrap_or(0);
    let mut comment_id = crate::parse_i64_flag(args, "--comment-id")?.unwrap_or(0);
    let mut author_id = crate::parse_i64_flag(args, "--author-id")?.unwrap_or(0);
    let mut author_id_set = author_id_given;
    if !handle_given && slide < 1 {
        return Err(CliError::invalid_args("--slide must be >= 1"));
    }
    if !handle_given && !comment_given {
        return Err(CliError::invalid_args(
            "either --handle or --comment-id is required",
        ));
    }
    if !handle_given && comment_id < 0 {
        return Err(CliError::invalid_args("--comment-id must be >= 0"));
    }
    if author_id_set && author_id < 0 {
        return Err(CliError::invalid_args("--author-id must be >= 0"));
    }
    let expect_hash = crate::parse_string_flag(args, "--expect-hash")?.unwrap_or_default();
    let handle = crate::parse_string_flag(args, "--handle")?.unwrap_or_default();
    let options = parse_comment_mutation_options(args)?;
    let detected = package_type(file)?;
    if detected != "pptx" {
        return Err(CliError::unsupported_type(format!(
            "file is not a PPTX document (detected: {detected})"
        )));
    }
    if handle_given {
        let resolved = resolve_comment_handle_target(file, &handle)?;
        slide = resolved.0 as i64;
        comment_id = resolved.1;
        author_id = resolved.2;
        author_id_set = true;
    }
    let mutation = build_remove_comment_mutation(
        file,
        slide as u32,
        comment_id,
        author_id,
        author_id_set,
        &expect_hash,
    )?;
    write_comment_mutation(file, &mutation.overrides, &mutation.removals, &options)?;
    Ok(remove_comment_result_json(file, &mutation.result, &options))
}

fn parse_comment_mutation_options(args: &[String]) -> CliResult<PptxCommentMutationOptions> {
    let out = crate::parse_string_flag(args, "--out")?;
    let backup = crate::parse_string_flag(args, "--backup")?;
    let dry_run = crate::has_flag(args, "--dry-run");
    let in_place = crate::has_flag(args, "--in-place");
    let no_validate = crate::has_flag(args, "--no-validate");
    validate_xlsx_mutation_output_flags(out.as_deref(), in_place, backup.as_deref(), dry_run)?;
    Ok(PptxCommentMutationOptions {
        out,
        backup,
        dry_run,
        in_place,
        no_validate,
    })
}

fn resolve_comment_text(args: &[String]) -> CliResult<Option<String>> {
    let text_set = value_flag_present(args, "--text");
    let file_set = value_flag_present(args, "--text-file");
    if text_set && file_set {
        return Err(CliError::invalid_args(
            "cannot specify both --text and --text-file",
        ));
    }
    if file_set {
        let path = crate::parse_string_flag(args, "--text-file")?.unwrap_or_default();
        return fs::read_to_string(&path)
            .map(Some)
            .map_err(|_| CliError::file_not_found(format!("file not found: {path}")));
    }
    crate::parse_string_flag(args, "--text")
}

fn build_add_comment_mutation(
    file: &str,
    slide: u32,
    author: &str,
    initials: &str,
    date: &str,
    text: &str,
) -> CliResult<AddCommentMutation> {
    let slides = pptx_slide_refs(file)?;
    let slide_ref = slide_ref_by_number(&slides, slide)?;
    let entries = zip_entry_names(file)?;
    let mut overrides = BTreeMap::new();
    let removals = BTreeSet::new();
    let mut content_types = zip_text(file, "[Content_Types].xml")?;

    let slide_rels_part = relationships_part_for(&slide_ref.part);
    let slide_rels_xml = zip_text(file, &slide_rels_part).unwrap_or_else(|_| relationships_xml());
    let slide_rels = relationship_entries_from_xml(&slide_rels_xml);
    let (comments_part, created_part, created_relationship) = if let Some(part) =
        slide_comments_part(&entries, &slide_ref.part, &slide_rels)
    {
        (part, false, false)
    } else {
        let part = allocate_numbered_part_name(&entries, "ppt/comments/comment", ".xml");
        content_types = ensure_content_type_override(content_types, &part, COMMENTS_CONTENT_TYPE);
        let target = relationship_target_from_source_to_target(&slide_ref.part, &part);
        let rels_xml = add_relationship_to_xml(
            slide_rels_xml,
            &allocate_relationship_id(&slide_rels),
            COMMENTS_REL_TYPE,
            &target,
        );
        overrides.insert(slide_rels_part, rels_xml);
        overrides.insert(part.clone(), comments_template());
        (part, true, true)
    };

    let comments_xml = overrides
        .get(&comments_part)
        .cloned()
        .unwrap_or_else(|| zip_text(file, &comments_part).unwrap_or_else(|_| comments_template()));
    ensure_comments_root(&comments_xml, &comments_part)?;
    let comments = parse_comment_elements(&comments_xml);
    let next_idx = comments.iter().map(|comment| comment.id).max().unwrap_or(0) + 1;

    let author_update = ensure_comment_author(
        file,
        &entries,
        &mut overrides,
        &mut content_types,
        author,
        initials,
        next_idx,
    )?;

    let updated_comments = append_comment_element(
        &comments_xml,
        &render_comment_element(author_update.author_id, next_idx, date, text, "0", "0"),
    )?;
    overrides.insert(comments_part.clone(), updated_comments);
    overrides.insert("[Content_Types].xml".to_string(), content_types);

    Ok(AddCommentMutation {
        result: AddCommentResult {
            slide,
            slide_id: slide_ref.slide_id,
            slide_part_uri: package_uri(&slide_ref.part),
            comments_part: package_uri(&comments_part),
            comment_id: next_idx,
            author_id: author_update.author_id,
            author: author.to_string(),
            initials: initials.to_string(),
            date: date.to_string(),
            text: text.to_string(),
            content_hash: comment_content_hash(author, date, text),
            created_part,
            created_relationship,
            created_authors_part: author_update.created_part,
            created_author: author_update.created_author,
        },
        overrides,
        removals,
    })
}

struct EditCommentSpec<'a> {
    slide: u32,
    comment_id: i64,
    author_id: i64,
    author_id_set: bool,
    expect_hash: &'a str,
    text: &'a str,
    text_set: bool,
    author: &'a str,
    author_set: bool,
    date: &'a str,
    date_set: bool,
}

fn build_edit_comment_mutation(
    file: &str,
    spec: EditCommentSpec<'_>,
) -> CliResult<EditCommentMutation> {
    let slides = pptx_slide_refs(file)?;
    let slide_ref = slide_ref_by_number(&slides, spec.slide)?;
    let entries = zip_entry_names(file)?;
    let mut overrides = BTreeMap::new();
    let removals = BTreeSet::new();
    let mut content_types = zip_text(file, "[Content_Types].xml")?;
    let comments_part =
        existing_slide_comments_part(file, &entries, &slide_ref.part, spec.comment_id)?;
    let comments_xml = zip_text(file, &comments_part)?;
    let comments = parse_comment_elements(&comments_xml);
    let comment = find_comment_by_id(
        &comments,
        spec.author_id,
        spec.author_id_set,
        spec.comment_id,
    )?;
    let authors = read_comment_authors(file, &entries, &overrides)?;
    let before = snapshot_comment(comment, &authors);
    if !spec.expect_hash.is_empty() && spec.expect_hash != before.content_hash {
        return Err(CliError::invalid_args(format!(
            "comment hash mismatch: comment {} expected {} but found {}",
            spec.comment_id, spec.expect_hash, before.content_hash
        )));
    }

    let mut author_id = before.author_id;
    let mut author_name = before.author.clone();
    let mut initials = before.initials.clone();
    if spec.author_set {
        let author_update = ensure_comment_author(
            file,
            &entries,
            &mut overrides,
            &mut content_types,
            spec.author,
            "",
            0,
        )?;
        author_id = author_update.author_id;
        author_name = spec.author.to_string();
        let refreshed_authors = read_comment_authors(file, &entries, &overrides)?;
        initials = refreshed_authors
            .iter()
            .find(|author| author.id == author_id)
            .map(|author| author.initials.clone())
            .unwrap_or_default();
    }
    let date = if spec.date_set {
        spec.date.to_string()
    } else {
        before.date.clone()
    };
    let text = if spec.text_set {
        spec.text.to_string()
    } else {
        before.text.clone()
    };
    let replacement = render_comment_element(
        author_id,
        spec.comment_id,
        &date,
        &text,
        &comment.pos_x,
        &comment.pos_y,
    );
    let updated_comments =
        replace_xml_span(&comments_xml, comment.start, comment.end, &replacement);
    overrides.insert(comments_part.clone(), updated_comments);
    overrides.insert("[Content_Types].xml".to_string(), content_types);

    Ok(EditCommentMutation {
        result: EditCommentResult {
            slide: spec.slide,
            slide_id: slide_ref.slide_id,
            slide_part_uri: package_uri(&slide_ref.part),
            comments_part: package_uri(&comments_part),
            comment_id: spec.comment_id,
            author_id,
            author: author_name.clone(),
            initials,
            date: date.clone(),
            text: text.clone(),
            content_hash: comment_content_hash(&author_name, &date, &text),
            previous_text: before.text,
            previous_hash: before.content_hash,
        },
        overrides,
        removals,
    })
}

fn build_remove_comment_mutation(
    file: &str,
    slide: u32,
    comment_id: i64,
    author_id: i64,
    author_id_set: bool,
    expect_hash: &str,
) -> CliResult<RemoveCommentMutation> {
    let slides = pptx_slide_refs(file)?;
    let slide_ref = slide_ref_by_number(&slides, slide)?;
    let entries = zip_entry_names(file)?;
    let mut overrides = BTreeMap::new();
    let mut removals = BTreeSet::new();
    let mut content_types = zip_text(file, "[Content_Types].xml")?;
    let comments_part = existing_slide_comments_part(file, &entries, &slide_ref.part, comment_id)?;
    let comments_xml = zip_text(file, &comments_part)?;
    let comments = parse_comment_elements(&comments_xml);
    let comment = find_comment_by_id(&comments, author_id, author_id_set, comment_id)?;
    let authors = read_comment_authors(file, &entries, &overrides)?;
    let before = snapshot_comment(comment, &authors);
    if !expect_hash.is_empty() && expect_hash != before.content_hash {
        return Err(CliError::invalid_args(format!(
            "comment hash mismatch: comment {comment_id} expected {expect_hash} but found {}",
            before.content_hash
        )));
    }

    let updated_comments = remove_xml_span(&comments_xml, comment.start, comment.end);
    let removed_part = parse_comment_elements(&updated_comments).is_empty();
    if removed_part {
        removals.insert(comments_part.clone());
        content_types = remove_content_type_override(&content_types, &comments_part)?;
        let slide_rels_part = relationships_part_for(&slide_ref.part);
        let slide_rels_xml =
            zip_text(file, &slide_rels_part).unwrap_or_else(|_| relationships_xml());
        let slide_rels = relationship_entries_from_xml(&slide_rels_xml);
        let kept = slide_rels
            .into_iter()
            .filter(|rel| {
                !(rel.rel_type == COMMENTS_REL_TYPE
                    && rel.target_mode != "External"
                    && package_part_name(&resolve_relationship_target(
                        &package_uri(&slide_ref.part),
                        &rel.target,
                    )) == comments_part)
            })
            .collect::<Vec<_>>();
        overrides.insert(slide_rels_part, render_relationships(&kept));
    } else {
        overrides.insert(comments_part.clone(), updated_comments);
    }
    overrides.insert("[Content_Types].xml".to_string(), content_types);

    Ok(RemoveCommentMutation {
        result: RemoveCommentResult {
            slide,
            slide_id: slide_ref.slide_id,
            slide_part_uri: package_uri(&slide_ref.part),
            comments_part: package_uri(&comments_part),
            comment_id,
            author_id: before.author_id,
            previous_author: before.author,
            previous_text: before.text,
            previous_hash: before.content_hash,
            removed_part,
        },
        overrides,
        removals,
    })
}

struct EnsureAuthorResult {
    author_id: i64,
    created_part: bool,
    created_author: bool,
}

fn ensure_comment_author(
    file: &str,
    entries: &[String],
    overrides: &mut BTreeMap<String, String>,
    content_types: &mut String,
    name: &str,
    initials: &str,
    for_idx: i64,
) -> CliResult<EnsureAuthorResult> {
    let (authors_part, exists) = find_comment_authors_part(file, entries);
    let mut created_part = false;
    if !exists && !overrides.contains_key(&authors_part) {
        *content_types = ensure_content_type_override(
            content_types.clone(),
            &authors_part,
            COMMENT_AUTHORS_CONTENT_TYPE,
        );
        ensure_presentation_comment_authors_rel(file, overrides, &authors_part)?;
        overrides.insert(authors_part.clone(), comment_authors_template());
        created_part = true;
    }

    let authors_xml = overrides.get(&authors_part).cloned().unwrap_or_else(|| {
        zip_text(file, &authors_part).unwrap_or_else(|_| comment_authors_template())
    });
    let mut authors = parse_comment_authors(&authors_xml);
    let mut created_author = false;
    let author_id = if let Some(author) = authors.iter_mut().find(|author| author.name == name) {
        if for_idx > 0 && for_idx > author.last_idx {
            author.last_idx = for_idx;
        }
        author.id
    } else {
        let next_id = authors.iter().map(|author| author.id).max().unwrap_or(-1) + 1;
        authors.push(CommentAuthor {
            id: next_id,
            name: name.to_string(),
            initials: initials.to_string(),
            last_idx: for_idx.max(0),
            clr_idx: next_id.to_string(),
        });
        created_author = true;
        next_id
    };
    overrides.insert(authors_part, render_comment_authors(&authors));
    Ok(EnsureAuthorResult {
        author_id,
        created_part,
        created_author,
    })
}

fn ensure_presentation_comment_authors_rel(
    file: &str,
    overrides: &mut BTreeMap<String, String>,
    authors_part: &str,
) -> CliResult<()> {
    let pres_rels_part = relationships_part_for(PRESENTATION_PART);
    let pres_rels_xml = overrides
        .get(&pres_rels_part)
        .cloned()
        .unwrap_or_else(|| zip_text(file, &pres_rels_part).unwrap_or_else(|_| relationships_xml()));
    let rels = relationship_entries_from_xml(&pres_rels_xml);
    if rels
        .iter()
        .any(|rel| rel.rel_type == COMMENT_AUTHORS_REL_TYPE && rel.target_mode != "External")
    {
        return Ok(());
    }
    let target = relationship_target_from_source_to_target(PRESENTATION_PART, authors_part);
    let updated = add_relationship_to_xml(
        pres_rels_xml,
        &allocate_relationship_id(&rels),
        COMMENT_AUTHORS_REL_TYPE,
        &target,
    );
    overrides.insert(pres_rels_part, updated);
    Ok(())
}

fn existing_slide_comments_part(
    file: &str,
    entries: &[String],
    slide_part: &str,
    comment_id: i64,
) -> CliResult<String> {
    let rels_xml =
        zip_text(file, &relationships_part_for(slide_part)).unwrap_or_else(|_| relationships_xml());
    let rels = relationship_entries_from_xml(&rels_xml);
    slide_comments_part(entries, slide_part, &rels)
        .ok_or_else(|| CliError::target_not_found("target not found: comment"))
        .map_err(|err| {
            if err.code == "target_not_found" {
                CliError::target_not_found("target not found: comment")
            } else {
                err
            }
        })
        .map_err(|_| CliError::target_not_found("target not found: comment"))
        .and_then(|part| {
            if part.is_empty() {
                Err(CliError::target_not_found(format!(
                    "target not found: comment {comment_id}"
                )))
            } else {
                Ok(part)
            }
        })
}

fn slide_comments_part(
    entries: &[String],
    slide_part: &str,
    rels: &[RelationshipEntry],
) -> Option<String> {
    for rel in rels {
        if rel.target_mode == "External" || rel.rel_type != COMMENTS_REL_TYPE {
            continue;
        }
        let uri = resolve_relationship_target(&package_uri(slide_part), &rel.target);
        if zip_entry_exists(entries, &uri) {
            return Some(package_part_name(&uri));
        }
        return None;
    }
    None
}

fn find_comment_authors_part(file: &str, entries: &[String]) -> (String, bool) {
    let rels_xml = zip_text(file, &relationships_part_for(PRESENTATION_PART))
        .unwrap_or_else(|_| relationships_xml());
    for rel in relationship_entries_from_xml(&rels_xml) {
        if rel.target_mode == "External" || rel.rel_type != COMMENT_AUTHORS_REL_TYPE {
            continue;
        }
        let uri = resolve_relationship_target("/ppt/presentation.xml", &rel.target);
        let part = package_part_name(&uri);
        return (part.clone(), zip_entry_exists(entries, &part));
    }
    (
        COMMENT_AUTHORS_PART.to_string(),
        zip_entry_exists(entries, COMMENT_AUTHORS_PART),
    )
}

fn read_comment_authors(
    file: &str,
    entries: &[String],
    overrides: &BTreeMap<String, String>,
) -> CliResult<Vec<CommentAuthor>> {
    let (authors_part, exists) = find_comment_authors_part(file, entries);
    if let Some(xml) = overrides.get(&authors_part) {
        return Ok(parse_comment_authors(xml));
    }
    if !exists {
        return Ok(Vec::new());
    }
    Ok(parse_comment_authors(&zip_text(file, &authors_part)?))
}

fn parse_comment_authors(xml: &str) -> Vec<CommentAuthor> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut authors = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "cmAuthor" =>
            {
                let id = attr(&e, "id")
                    .and_then(|value| value.parse::<i64>().ok())
                    .unwrap_or_default();
                authors.push(CommentAuthor {
                    id,
                    name: attr(&e, "name").unwrap_or_default(),
                    initials: attr(&e, "initials").unwrap_or_default(),
                    last_idx: attr(&e, "lastIdx")
                        .and_then(|value| value.parse::<i64>().ok())
                        .unwrap_or_default(),
                    clr_idx: attr(&e, "clrIdx").unwrap_or_else(|| id.to_string()),
                });
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    authors
}

fn render_comment_authors(authors: &[CommentAuthor]) -> String {
    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:cmAuthorLst xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">"#,
    );
    for author in authors {
        xml.push_str(&format!(
            r#"<p:cmAuthor id="{}" name="{}" initials="{}" lastIdx="{}" clrIdx="{}"/>"#,
            author.id,
            xml_attr_escape(&author.name),
            xml_attr_escape(&author.initials),
            author.last_idx,
            xml_attr_escape(&author.clr_idx)
        ));
    }
    xml.push_str("</p:cmAuthorLst>");
    xml
}

fn parse_comment_elements(xml: &str) -> Vec<CommentElement> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut comments = Vec::new();
    let mut current: Option<CommentElement> = None;
    let mut depth = 0usize;
    let mut in_text = false;
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "cm" => {
                depth = 1;
                current = Some(CommentElement {
                    start: before,
                    end: before,
                    id: attr(&e, "idx")
                        .and_then(|value| value.parse::<i64>().ok())
                        .unwrap_or_default(),
                    author_id: attr(&e, "authorId")
                        .and_then(|value| value.parse::<i64>().ok())
                        .unwrap_or_default(),
                    date: attr(&e, "dt").unwrap_or_default(),
                    text: String::new(),
                    pos_x: "0".to_string(),
                    pos_y: "0".to_string(),
                });
            }
            Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == "cm" => {
                comments.push(CommentElement {
                    start: before,
                    end: reader.buffer_position() as usize,
                    id: attr(&e, "idx")
                        .and_then(|value| value.parse::<i64>().ok())
                        .unwrap_or_default(),
                    author_id: attr(&e, "authorId")
                        .and_then(|value| value.parse::<i64>().ok())
                        .unwrap_or_default(),
                    date: attr(&e, "dt").unwrap_or_default(),
                    text: String::new(),
                    pos_x: "0".to_string(),
                    pos_y: "0".to_string(),
                });
            }
            Ok(Event::Start(e)) if current.is_some() => {
                depth += 1;
                match local_name(e.name().as_ref()) {
                    "text" => in_text = true,
                    "pos" => {
                        if let Some(comment) = current.as_mut() {
                            comment.pos_x = attr(&e, "x").unwrap_or_else(|| "0".to_string());
                            comment.pos_y = attr(&e, "y").unwrap_or_else(|| "0".to_string());
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(e)) if current.is_some() && local_name(e.name().as_ref()) == "pos" => {
                if let Some(comment) = current.as_mut() {
                    comment.pos_x = attr(&e, "x").unwrap_or_else(|| "0".to_string());
                    comment.pos_y = attr(&e, "y").unwrap_or_else(|| "0".to_string());
                }
            }
            Ok(Event::Text(e)) if in_text => {
                if let Some(comment) = current.as_mut() {
                    comment.text.push_str(&decode_xml_text(e.as_ref()));
                }
            }
            Ok(Event::CData(e)) if in_text => {
                if let Some(comment) = current.as_mut() {
                    comment.text.push_str(&decode_xml_text(e.as_ref()));
                }
            }
            Ok(Event::End(e)) if current.is_some() => {
                let name = local_name(e.name().as_ref()).to_string();
                if name == "text" {
                    in_text = false;
                }
                if depth == 1 && name == "cm" {
                    if let Some(mut comment) = current.take() {
                        comment.end = reader.buffer_position() as usize;
                        comments.push(comment);
                    }
                    depth = 0;
                    in_text = false;
                } else {
                    depth = depth.saturating_sub(1);
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    comments
}

fn find_comment_by_id(
    comments: &[CommentElement],
    author_id: i64,
    author_id_set: bool,
    comment_id: i64,
) -> CliResult<&CommentElement> {
    let matches = comments
        .iter()
        .filter(|comment| {
            comment.id == comment_id && (!author_id_set || comment.author_id == author_id)
        })
        .collect::<Vec<_>>();
    if author_id_set {
        return matches
            .first()
            .copied()
            .ok_or_else(|| CliError::target_not_found("target not found: comment"));
    }
    match matches.as_slice() {
        [] => Err(CliError::target_not_found("target not found: comment")),
        [comment] => Ok(*comment),
        _ => {
            let author_ids = matches
                .iter()
                .map(|comment| comment.author_id.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            Err(CliError::invalid_args(format!(
                "comment id is ambiguous; specify --author-id: comment idx {comment_id} matches authorIds {author_ids} on this slide"
            )))
        }
    }
}

fn snapshot_comment(comment: &CommentElement, authors: &[CommentAuthor]) -> CommentSnapshot {
    let author = authors.iter().find(|author| author.id == comment.author_id);
    let author_name = author.map(|author| author.name.clone()).unwrap_or_default();
    let initials = author
        .map(|author| author.initials.clone())
        .unwrap_or_default();
    CommentSnapshot {
        author_id: comment.author_id,
        author: author_name.clone(),
        initials,
        date: comment.date.clone(),
        text: comment.text.clone(),
        content_hash: comment_content_hash(&author_name, &comment.date, &comment.text),
    }
}

fn append_comment_element(xml: &str, comment: &str) -> CliResult<String> {
    let close = xml
        .rfind("</")
        .ok_or_else(|| CliError::unexpected("comments part has no closing root"))?;
    Ok(format!("{}{}{}", &xml[..close], comment, &xml[close..]))
}

fn ensure_comments_root(xml: &str, part: &str) -> CliResult<()> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                if local_name(e.name().as_ref()) == "cmLst" {
                    return Ok(());
                }
                return Err(CliError::unexpected(format!(
                    "comments part /{part} has no p:cmLst root"
                )));
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Err(CliError::unexpected(format!(
        "comments part /{part} has no p:cmLst root"
    )))
}

fn render_comment_element(
    author_id: i64,
    comment_id: i64,
    date: &str,
    text: &str,
    pos_x: &str,
    pos_y: &str,
) -> String {
    let date_attr = if date.is_empty() {
        String::new()
    } else {
        format!(r#" dt="{}""#, xml_attr_escape(date))
    };
    format!(
        r#"<p:cm authorId="{author_id}"{date_attr} idx="{comment_id}"><p:pos x="{}" y="{}"/><p:text>{}</p:text></p:cm>"#,
        xml_attr_escape(pos_x),
        xml_attr_escape(pos_y),
        xml_escape(text)
    )
}

fn comment_content_hash(author: &str, date: &str, text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(author.as_bytes());
    hasher.update([0]);
    hasher.update(date.as_bytes());
    hasher.update([0]);
    hasher.update(text.as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

fn pptx_slide_refs(file: &str) -> CliResult<Vec<PptxSlideRef>> {
    let presentation = zip_text(file, PRESENTATION_PART)?;
    let slide_refs = presentation_slide_refs(&presentation);
    let rels =
        relationship_entries_from_xml(&zip_text(file, &relationships_part_for(PRESENTATION_PART))?);
    slide_refs
        .into_iter()
        .enumerate()
        .map(|(index, (slide_id, rel_id))| {
            let rel = rels
                .iter()
                .find(|candidate| candidate.id == rel_id)
                .ok_or_else(|| CliError::unexpected(format!("missing relationship {rel_id}")))?;
            Ok(PptxSlideRef {
                number: index as u32 + 1,
                slide_id,
                part: package_part_name(&resolve_relationship_target(
                    "/ppt/presentation.xml",
                    &rel.target,
                )),
            })
        })
        .collect()
}

fn presentation_slide_refs(xml: &str) -> Vec<(u32, String)> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut slides = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "sldId" =>
            {
                if let Some(rel_id) = attr_exact(&e, "r:id") {
                    slides.push((
                        attr(&e, "id")
                            .and_then(|value| value.parse::<u32>().ok())
                            .unwrap_or_default(),
                        rel_id,
                    ));
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    slides
}

fn slide_ref_by_number(slides: &[PptxSlideRef], slide: u32) -> CliResult<&PptxSlideRef> {
    slides.get(slide as usize - 1).ok_or_else(|| {
        CliError::invalid_args(format!(
            "slide out of range: slide {slide} (presentation has {} slides)",
            slides.len()
        ))
    })
}

fn resolve_comment_handle_target(file: &str, handle: &str) -> CliResult<(u32, i64, i64)> {
    let parsed = parse_comment_handle(handle)?;
    let slides = pptx_slide_refs(file)?;
    let matches = slides
        .iter()
        .filter(|slide| slide.slide_id == parsed.0)
        .collect::<Vec<_>>();
    let slide = match matches.as_slice() {
        [] => {
            return Err(CliError::invalid_args(format!(
                "HANDLE_SCOPE_STALE: slide sldId {} was not found (handle {:?})",
                parsed.0, handle
            )));
        }
        [slide] => *slide,
        _ => {
            return Err(CliError::invalid_args(format!(
                "HANDLE_AMBIGUOUS: slide sldId {} is not unique (handle {:?})",
                parsed.0, handle
            )));
        }
    };
    let entries = zip_entry_names(file)?;
    let rels_xml = zip_text(file, &relationships_part_for(&slide.part))
        .unwrap_or_else(|_| relationships_xml());
    let rels = relationship_entries_from_xml(&rels_xml);
    let comments_part = slide_comments_part(&entries, &slide.part, &rels).ok_or_else(|| {
        CliError::invalid_args(format!(
            "HANDLE_STALE: comment idx {} authorId {} was not found on slide sldId {} (handle {:?})",
            parsed.1, parsed.2, parsed.0, handle
        ))
    })?;
    let comments = parse_comment_elements(&zip_text(file, &comments_part)?);
    let matches = comments
        .iter()
        .filter(|comment| comment.id == parsed.1 && comment.author_id == parsed.2)
        .count();
    match matches {
        0 => Err(CliError::invalid_args(format!(
            "HANDLE_STALE: comment idx {} authorId {} was not found on slide sldId {} (handle {:?})",
            parsed.1, parsed.2, parsed.0, handle
        ))),
        1 => Ok((slide.number, parsed.1, parsed.2)),
        count => Err(CliError::invalid_args(format!(
            "HANDLE_AMBIGUOUS: {count} comments share idx {} authorId {} on slide sldId {} (handle {:?})",
            parsed.1, parsed.2, parsed.0, handle
        ))),
    }
}

fn parse_comment_handle(handle: &str) -> CliResult<(u32, i64, i64)> {
    let trimmed = handle.trim();
    let Some(body) = trimmed.strip_prefix("H:") else {
        return Err(CliError::invalid_args(format!(
            "HANDLE_MALFORMED: missing handle version prefix \"H:\" (handle {handle:?})"
        )));
    };
    let parts = body.split('/').collect::<Vec<_>>();
    if parts.len() != 3 || parts[0] != "pptx" {
        return Err(CliError::invalid_args(format!(
            "HANDLE_MALFORMED: expected a PPTX comment handle (H:pptx/s:<sldId>/comment:idx:<id>:authorId:<id>) (handle {handle:?})"
        )));
    }
    let slide_id = parts[1]
        .strip_prefix("s:")
        .and_then(|value| value.parse::<u32>().ok())
        .ok_or_else(|| {
            CliError::invalid_args(format!(
                "HANDLE_MALFORMED: slide scope must be s:<sldId> (handle {handle:?})"
            ))
        })?;
    let obj = parts[2].strip_prefix("comment:").ok_or_else(|| {
        CliError::invalid_args(format!(
            "HANDLE_MALFORMED: expected a PPTX comment handle (H:pptx/s:<sldId>/comment:idx:<id>:authorId:<id>) (handle {handle:?})"
        ))
    })?;
    let ref_parts = obj.split(':').collect::<Vec<_>>();
    if ref_parts.len() != 4 || ref_parts[0] != "idx" || ref_parts[2] != "authorId" {
        return Err(CliError::invalid_args(format!(
            "HANDLE_MALFORMED: comment objref must be idx:<comment-id>:authorId:<author-id> (handle {handle:?})"
        )));
    }
    let comment_id = ref_parts[1].parse::<i64>().map_err(|_| {
        CliError::invalid_args(format!(
            "HANDLE_MALFORMED: comment id must be numeric (handle {handle:?})"
        ))
    })?;
    let author_id = ref_parts[3].parse::<i64>().map_err(|_| {
        CliError::invalid_args(format!(
            "HANDLE_MALFORMED: author id must be numeric (handle {handle:?})"
        ))
    })?;
    if comment_id < 0 || author_id < 0 {
        return Err(CliError::invalid_args(format!(
            "HANDLE_MALFORMED: comment and author ids must be non-negative (handle {handle:?})"
        )));
    }
    Ok((slide_id, comment_id, author_id))
}

fn add_common_comment_result_fields(
    out: &mut Map<String, Value>,
    command_target: &str,
    dry_run: bool,
    slide: u32,
    handle: &str,
    comment_id: i64,
    author_id: i64,
) {
    out.insert("dryRun".to_string(), json!(dry_run));
    out.insert("handle".to_string(), json!(handle));
    out.insert("primarySelector".to_string(), json!(handle));
    out.insert(
        "selectors".to_string(),
        json!(comment_selectors(handle, comment_id, author_id)),
    );
    let suffix = if dry_run { "Template" } else { "" };
    out.insert(
        format!("readbackCommand{suffix}"),
        json!(format!(
            "ooxml --json pptx comments list {} --slide {}",
            command_arg(command_target),
            slide
        )),
    );
    out.insert(
        format!("slideReadbackCommand{suffix}"),
        json!(format!(
            "ooxml --json pptx slides show {} --slide {} --include-text --include-bounds",
            command_arg(command_target),
            slide
        )),
    );
    out.insert(
        format!("validateCommand{suffix}"),
        json!(format!(
            "ooxml validate --strict {}",
            command_arg(command_target)
        )),
    );
    out.insert(
        format!("renderCommand{suffix}"),
        json!(format!(
            "ooxml pptx render {} --out render-check",
            command_arg(command_target)
        )),
    );
}

fn add_comment_result_json(
    file: &str,
    result: &AddCommentResult,
    options: &PptxCommentMutationOptions,
) -> Value {
    let mut out = base_mutation_json(file, options, "added");
    let command_target = command_target(file, options);
    let handle = comment_handle(result.slide_id, result.comment_id, result.author_id);
    add_common_comment_result_fields(
        &mut out,
        &command_target,
        options.dry_run,
        result.slide,
        &handle,
        result.comment_id,
        result.author_id,
    );
    out.insert("slide".to_string(), json!(result.slide));
    out.insert("slidePartUri".to_string(), json!(result.slide_part_uri));
    out.insert("commentsPart".to_string(), json!(result.comments_part));
    out.insert("commentId".to_string(), json!(result.comment_id));
    out.insert("authorId".to_string(), json!(result.author_id));
    out.insert("author".to_string(), json!(result.author));
    if !result.initials.is_empty() {
        out.insert("initials".to_string(), json!(result.initials));
    }
    if !result.date.is_empty() {
        out.insert("date".to_string(), json!(result.date));
    }
    out.insert("text".to_string(), json!(result.text));
    out.insert("contentHash".to_string(), json!(result.content_hash));
    out.insert("createdPart".to_string(), json!(result.created_part));
    out.insert(
        "createdRelationship".to_string(),
        json!(result.created_relationship),
    );
    out.insert(
        "createdAuthorsPart".to_string(),
        json!(result.created_authors_part),
    );
    out.insert("createdAuthor".to_string(), json!(result.created_author));
    Value::Object(out)
}

fn edit_comment_result_json(
    file: &str,
    result: &EditCommentResult,
    options: &PptxCommentMutationOptions,
) -> Value {
    let mut out = base_mutation_json(file, options, "edited");
    let command_target = command_target(file, options);
    let handle = comment_handle(result.slide_id, result.comment_id, result.author_id);
    add_common_comment_result_fields(
        &mut out,
        &command_target,
        options.dry_run,
        result.slide,
        &handle,
        result.comment_id,
        result.author_id,
    );
    out.insert("slide".to_string(), json!(result.slide));
    out.insert("slidePartUri".to_string(), json!(result.slide_part_uri));
    out.insert("commentsPart".to_string(), json!(result.comments_part));
    out.insert("commentId".to_string(), json!(result.comment_id));
    out.insert("authorId".to_string(), json!(result.author_id));
    out.insert("author".to_string(), json!(result.author));
    if !result.initials.is_empty() {
        out.insert("initials".to_string(), json!(result.initials));
    }
    if !result.date.is_empty() {
        out.insert("date".to_string(), json!(result.date));
    }
    out.insert("text".to_string(), json!(result.text));
    out.insert("contentHash".to_string(), json!(result.content_hash));
    out.insert("previousText".to_string(), json!(result.previous_text));
    out.insert("previousHash".to_string(), json!(result.previous_hash));
    Value::Object(out)
}

fn remove_comment_result_json(
    file: &str,
    result: &RemoveCommentResult,
    options: &PptxCommentMutationOptions,
) -> Value {
    let mut out = base_mutation_json(file, options, "removed");
    let command_target = command_target(file, options);
    let handle = comment_handle(result.slide_id, result.comment_id, result.author_id);
    add_common_comment_result_fields(
        &mut out,
        &command_target,
        options.dry_run,
        result.slide,
        &handle,
        result.comment_id,
        result.author_id,
    );
    out.insert("slide".to_string(), json!(result.slide));
    out.insert("slidePartUri".to_string(), json!(result.slide_part_uri));
    out.insert("commentsPart".to_string(), json!(result.comments_part));
    out.insert("commentId".to_string(), json!(result.comment_id));
    out.insert("authorId".to_string(), json!(result.author_id));
    out.insert("previousAuthor".to_string(), json!(result.previous_author));
    out.insert("previousText".to_string(), json!(result.previous_text));
    out.insert("previousHash".to_string(), json!(result.previous_hash));
    out.insert("removedPart".to_string(), json!(result.removed_part));
    Value::Object(out)
}

fn base_mutation_json(
    file: &str,
    options: &PptxCommentMutationOptions,
    operation: &str,
) -> Map<String, Value> {
    let mut out = Map::new();
    out.insert("file".to_string(), json!(file));
    if !options.dry_run
        && let Some(output) = mutation_output_path(file, options)
    {
        out.insert("output".to_string(), json!(output));
    }
    out.insert("operation".to_string(), json!(operation));
    out
}

fn command_target(file: &str, options: &PptxCommentMutationOptions) -> String {
    if options.dry_run {
        "<out.pptx>".to_string()
    } else {
        mutation_output_path(file, options).unwrap_or_else(|| file.to_string())
    }
}

fn mutation_output_path(file: &str, options: &PptxCommentMutationOptions) -> Option<String> {
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

fn write_comment_mutation(
    file: &str,
    overrides: &BTreeMap<String, String>,
    removals: &BTreeSet<String>,
    options: &PptxCommentMutationOptions,
) -> CliResult<()> {
    let output_path = options
        .out
        .as_deref()
        .filter(|value| !value.trim().is_empty());
    let write_path = if options.dry_run || options.in_place || output_path == Some(file) {
        package_mutation_temp_path(file, "pptx-comments")
    } else {
        output_path
            .ok_or_else(|| {
                CliError::invalid_args(
                    "must specify exactly one of --out, --in-place, or --dry-run",
                )
            })?
            .to_string()
    };
    copy_zip_with_part_overrides_and_removals(file, &write_path, overrides, removals)?;
    if !options.no_validate {
        validate(&write_path, true)?;
    }
    if options.dry_run {
        let _ = fs::remove_file(&write_path);
    } else if options.in_place || output_path == Some(file) {
        if let Some(backup_path) = options
            .backup
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            fs::copy(file, backup_path)
                .map_err(|err| CliError::unexpected(format!("failed to create backup: {err}")))?;
        }
        fs::rename(&write_path, file)
            .or_else(|_| {
                fs::copy(&write_path, file)?;
                fs::remove_file(&write_path)
            })
            .map_err(|err| CliError::unexpected(format!("failed to write output file: {err}")))?;
    }
    Ok(())
}

fn comment_handle(slide_id: u32, comment_id: i64, author_id: i64) -> String {
    if slide_id > 0 {
        format!("H:pptx/s:{slide_id}/comment:idx:{comment_id}:authorId:{author_id}")
    } else {
        format!("comment:{comment_id}:authorId:{author_id}")
    }
}

fn comment_selectors(handle: &str, comment_id: i64, author_id: i64) -> Vec<String> {
    vec![
        handle.to_string(),
        format!("comment:{comment_id}:authorId:{author_id}"),
        format!("comment:{comment_id}"),
        comment_id.to_string(),
        format!("authorId:{author_id}"),
    ]
}

fn remove_content_type_override(xml: &str, part: &str) -> CliResult<String> {
    let normalized = package_uri(part);
    let open_end = xml
        .find('>')
        .ok_or_else(|| CliError::unexpected("invalid [Content_Types].xml"))?;
    let close_start = xml
        .rfind("</")
        .ok_or_else(|| CliError::unexpected("invalid [Content_Types].xml"))?;
    let mut out = xml.to_string();
    for child in xml_direct_child_ranges(xml, open_end + 1, close_start)?
        .into_iter()
        .rev()
    {
        if child.kind != "Override" {
            continue;
        }
        let fragment = &xml[child.start..child.end];
        let mut reader = Reader::from_str(fragment);
        reader.config_mut().trim_text(true);
        let remove = loop {
            match reader.read_event() {
                Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                    break attr(&e, "PartName").as_deref() == Some(normalized.as_str());
                }
                Ok(Event::Eof) => break false,
                Err(_) => break false,
                _ => {}
            }
        };
        if remove {
            out = remove_xml_span(&out, child.start, child.end);
        }
    }
    Ok(out)
}

fn render_relationships(rels: &[RelationshipEntry]) -> String {
    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
    );
    for rel in rels {
        let target_mode = if rel.target_mode.is_empty() {
            String::new()
        } else {
            format!(r#" TargetMode="{}""#, xml_attr_escape(&rel.target_mode))
        };
        xml.push_str(&format!(
            r#"<Relationship Id="{}" Type="{}" Target="{}"{} />"#,
            xml_attr_escape(&rel.id),
            xml_attr_escape(&rel.rel_type),
            xml_attr_escape(&rel.target),
            target_mode
        ));
    }
    xml.push_str("</Relationships>");
    xml
}

fn allocate_numbered_part_name(entries: &[String], prefix: &str, suffix: &str) -> String {
    let mut next = 1u32;
    for entry in entries {
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
    format!("{prefix}{next}{suffix}")
}

fn comments_template() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:cmLst xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"></p:cmLst>"#.to_string()
}

fn comment_authors_template() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:cmAuthorLst xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"></p:cmAuthorLst>"#.to_string()
}

fn relationships_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"></Relationships>"#.to_string()
}

fn package_uri(part: &str) -> String {
    format!("/{}", part.trim_start_matches('/'))
}

fn package_part_name(uri: &str) -> String {
    uri.trim_start_matches('/').to_string()
}
