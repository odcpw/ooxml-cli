use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::{
    CliError, CliResult, RelationshipEntry, WorkbookSheet, add_relationship_to_xml,
    allocate_relationship_id, copy_zip_with_binary_part_overrides_and_removals,
    ensure_content_type_override, local_name, needs_xml_space_preserve, normalize_xlsx_cell_ref,
    relationship_entries_from_xml, relationship_target_from_source_to_target,
    relationships_part_for, remove_xml_span, replace_xml_span, resolve_relationship_target,
    resolve_sheet, resolve_sheet_by_sheet_id_unique, validate, validate_xlsx_mutation_output_flags,
    workbook_sheets, xlsx_ranges_set_temp_path, xml_attr_escape, xml_attrs_map,
    xml_direct_child_ranges, xml_escape, xml_open_tag_from_start, xml_tag_prefix, zip_entry_exists,
    zip_entry_names, zip_text,
};

mod output;

use self::output::{
    add_comment_readback_commands, comment_json, mutation_base_result, xlsx_comments_list_command,
};

const XLSX_NS: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const REL_NS: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const OFFICE_REL_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const REL_COMMENTS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments";
const REL_VML_DRAWING: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/vmlDrawing";
const CONTENT_TYPE_COMMENTS: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.comments+xml";
const CONTENT_TYPE_VML: &str = "application/vnd.openxmlformats-officedocument.vmlDrawing";

#[derive(Clone)]
struct XlsxCommentInfo {
    id: i64,
    author_id_attr: String,
    author: String,
    text: String,
    content_hash: String,
    anchored_to_cell: String,
    anchored_to_cell_row: Option<u32>,
    anchored_to_cell_column: Option<u32>,
}

#[derive(Clone)]
struct XlsxCommentsPart {
    uri: String,
    exists: bool,
    rel_exists: bool,
}

#[derive(Clone)]
struct XlsxCommentsSheet {
    sheet: WorkbookSheet,
    part: String,
    sheets: Vec<WorkbookSheet>,
}

#[derive(Clone)]
struct XlsxCommentsDoc {
    authors: Vec<String>,
    comments: Vec<XlsxCommentInfo>,
}

struct XlsxCommentOutputOptions<'a> {
    out: Option<&'a str>,
    in_place: bool,
    backup: Option<&'a str>,
    dry_run: bool,
    no_validate: bool,
}

struct XlsxCommentPackageEdits<'a> {
    text_overrides: &'a BTreeMap<String, String>,
    binary_overrides: &'a BTreeMap<String, Vec<u8>>,
    removals: &'a BTreeSet<String>,
}

pub(crate) struct XlsxCommentsAddOptions<'a> {
    pub(crate) sheet: Option<&'a str>,
    pub(crate) cell: Option<&'a str>,
    pub(crate) author: Option<&'a str>,
    pub(crate) text: Option<&'a str>,
    pub(crate) text_file: Option<&'a str>,
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
}

pub(crate) struct XlsxCommentsUpdateOptions<'a> {
    pub(crate) sheet: Option<&'a str>,
    pub(crate) comment_id: Option<i64>,
    pub(crate) handle: Option<&'a str>,
    pub(crate) text: Option<&'a str>,
    pub(crate) text_present: bool,
    pub(crate) text_file: Option<&'a str>,
    pub(crate) author: Option<&'a str>,
    pub(crate) author_present: bool,
    pub(crate) expect_hash: Option<&'a str>,
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
}

pub(crate) struct XlsxCommentsRemoveOptions<'a> {
    pub(crate) sheet: Option<&'a str>,
    pub(crate) comment_id: Option<i64>,
    pub(crate) handle: Option<&'a str>,
    pub(crate) expect_hash: Option<&'a str>,
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
}

pub(crate) fn xlsx_comments_list(
    file: &str,
    sheet_selector: Option<&str>,
    comment_id: Option<i64>,
) -> CliResult<Value> {
    let context = resolve_comments_sheet(file, sheet_selector)?;
    let part = find_comments_part(file, &context.part)?;
    let mut comments = if part.exists {
        read_comments_doc(file, &part.uri)?.comments
    } else {
        Vec::new()
    };
    if let Some(comment_id) = comment_id {
        comments.retain(|comment| comment.id == comment_id);
        if comments.is_empty() {
            return Err(CliError::target_not_found(format!(
                "target not found: comment {comment_id}"
            )));
        }
    }
    let comments = comments
        .iter()
        .map(|comment| comment_json(comment, &context.sheet, &context.sheets))
        .collect::<Vec<_>>();

    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("sheet".to_string(), json!(context.sheet.name));
    result.insert("sheetNumber".to_string(), json!(context.sheet.position));
    if part.exists {
        result.insert("commentsPart".to_string(), json!(part.uri));
    }
    result.insert("comments".to_string(), Value::Array(comments));
    result.insert(
        "listCommand".to_string(),
        json!(xlsx_comments_list_command(file, &context.sheet)),
    );
    Ok(Value::Object(result))
}

pub(crate) fn xlsx_comments_add(
    file: &str,
    options: XlsxCommentsAddOptions<'_>,
) -> CliResult<Value> {
    if !Path::new(file).exists() {
        return Err(CliError::file_not_found(format!("file not found: {file}")));
    }
    let cell = options
        .cell
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| CliError::invalid_args("--cell is required"))?;
    let cell = normalize_xlsx_cell_ref(cell, "--cell")?;
    let author = options
        .author
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| CliError::invalid_args("--author is required"))?;
    let text = resolve_comment_text(options.text, options.text_file, options.text.is_some())?;
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;

    let context = resolve_comments_sheet(file, options.sheet)?;
    let part = find_comments_part(file, &context.part)?;
    let mut doc = if part.exists {
        read_comments_doc(file, &part.uri)?
    } else {
        XlsxCommentsDoc {
            authors: Vec::new(),
            comments: Vec::new(),
        }
    };
    if doc.comments.iter().any(|comment| {
        normalize_xlsx_cell_ref(&comment.anchored_to_cell, "comment ref")
            .is_ok_and(|existing| existing == cell)
    }) {
        return Err(CliError::invalid_args(format!(
            "cell already has a comment: {cell}"
        )));
    }
    let author_id = doc.authors.len();
    doc.authors.push(author.to_string());
    doc.comments.push(make_comment_info(
        i64::MAX,
        author_id.to_string(),
        author.to_string(),
        text.clone(),
        cell.clone(),
    ));
    sort_comments_by_cell(&mut doc.comments);
    renumber_comments(&mut doc.comments);
    let added = doc
        .comments
        .iter()
        .find(|comment| comment.anchored_to_cell == cell && comment.text == text)
        .cloned()
        .ok_or_else(|| CliError::unexpected("added comment was not found after insertion"))?;

    let mut text_overrides = BTreeMap::new();
    let mut binary_overrides = BTreeMap::new();
    let mut removals = BTreeSet::new();
    text_overrides.insert(
        part.uri.trim_start_matches('/').to_string(),
        render_comments_doc(&doc),
    );
    let vml_uri = sync_comments_vml(
        file,
        &context.part,
        &doc.comments,
        &mut text_overrides,
        &mut binary_overrides,
        &mut removals,
    )?;
    let mut content_types = zip_text(file, "[Content_Types].xml")?;
    content_types = ensure_content_type_override(content_types, &part.uri, CONTENT_TYPE_COMMENTS);
    if let Some(vml_uri) = vml_uri.as_deref().filter(|value| !value.is_empty()) {
        content_types = ensure_content_type_override(content_types, vml_uri, CONTENT_TYPE_VML);
    }
    text_overrides.insert("[Content_Types].xml".to_string(), content_types);
    let created_ref =
        ensure_comments_relationship(file, &context.part, &part.uri, &mut text_overrides)?;

    let commit_path = write_xlsx_comment_mutation(
        file,
        XlsxCommentOutputOptions {
            out: options.out,
            in_place: options.in_place,
            backup: options.backup,
            dry_run: options.dry_run,
            no_validate: options.no_validate,
        },
        XlsxCommentPackageEdits {
            text_overrides: &text_overrides,
            binary_overrides: &binary_overrides,
            removals: &removals,
        },
    )?;
    let mut result = mutation_base_result(
        file,
        &context,
        &added,
        commit_path.as_deref(),
        options.dry_run,
    );
    result.insert("author".to_string(), json!(added.author));
    result.insert("text".to_string(), json!(added.text));
    result.insert("contentHash".to_string(), json!(added.content_hash));
    result.insert("anchoredToCell".to_string(), json!(added.anchored_to_cell));
    result.insert("createdPart".to_string(), json!(!part.exists));
    result.insert("createdRef".to_string(), json!(created_ref));
    result.insert("operation".to_string(), json!("added"));
    add_comment_readback_commands(&mut result, commit_path.as_deref(), &context.sheet);
    Ok(Value::Object(result))
}

pub(crate) fn xlsx_comments_update(
    file: &str,
    options: XlsxCommentsUpdateOptions<'_>,
) -> CliResult<Value> {
    if !Path::new(file).exists() {
        return Err(CliError::file_not_found(format!("file not found: {file}")));
    }
    if options.handle.is_none() && options.comment_id.is_none() {
        return Err(CliError::invalid_args(
            "either --handle or --comment-id is required",
        ));
    }
    if options.handle.is_some() && options.comment_id.is_some() {
        return Err(CliError::invalid_args(
            "cannot specify both --handle and --comment-id",
        ));
    }
    if options.comment_id.is_some_and(|value| value < 0) {
        return Err(CliError::invalid_args("--comment-id must be >= 0"));
    }
    if options.text_present && options.text_file.is_some() {
        return Err(CliError::invalid_args(
            "cannot specify both --text and --text-file",
        ));
    }
    if !options.text_present && options.text_file.is_none() && !options.author_present {
        return Err(CliError::invalid_args(
            "specify at least one of --text, --text-file, or --author",
        ));
    }
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;
    let text = resolve_comment_text(options.text, options.text_file, options.text_present)?;

    let (context, comment_id) =
        resolve_comment_mutation_target(file, options.sheet, options.comment_id, options.handle)?;
    let part = find_comments_part(file, &context.part)?;
    if !part.exists {
        return Err(CliError::target_not_found("target not found: comment"));
    }
    let mut doc = read_comments_doc(file, &part.uri)?;
    let target_index = comment_index(&doc.comments, comment_id)?;
    let before = doc.comments[target_index].clone();
    guard_comment_hash(comment_id, options.expect_hash, &before.content_hash)?;
    if options.author_present {
        let author = options.author.unwrap_or("").to_string();
        let author_id = doc.authors.len();
        doc.authors.push(author.clone());
        doc.comments[target_index].author = author;
        doc.comments[target_index].author_id_attr = author_id.to_string();
    }
    if options.text_present || options.text_file.is_some() {
        doc.comments[target_index].text = text;
    }
    refresh_comment_hash_and_anchor(&mut doc.comments[target_index]);
    let updated = doc.comments[target_index].clone();

    let mut text_overrides = BTreeMap::new();
    let binary_overrides = BTreeMap::new();
    let removals = BTreeSet::new();
    text_overrides.insert(
        part.uri.trim_start_matches('/').to_string(),
        render_comments_doc(&doc),
    );
    let commit_path = write_xlsx_comment_mutation(
        file,
        XlsxCommentOutputOptions {
            out: options.out,
            in_place: options.in_place,
            backup: options.backup,
            dry_run: options.dry_run,
            no_validate: options.no_validate,
        },
        XlsxCommentPackageEdits {
            text_overrides: &text_overrides,
            binary_overrides: &binary_overrides,
            removals: &removals,
        },
    )?;

    let mut result = mutation_base_result(
        file,
        &context,
        &updated,
        commit_path.as_deref(),
        options.dry_run,
    );
    result.insert("author".to_string(), json!(updated.author));
    result.insert("text".to_string(), json!(updated.text));
    result.insert("contentHash".to_string(), json!(updated.content_hash));
    result.insert(
        "anchoredToCell".to_string(),
        json!(updated.anchored_to_cell),
    );
    result.insert("previousText".to_string(), json!(before.text));
    result.insert("previousHash".to_string(), json!(before.content_hash));
    result.insert("operation".to_string(), json!("updated"));
    add_comment_readback_commands(&mut result, commit_path.as_deref(), &context.sheet);
    Ok(Value::Object(result))
}

pub(crate) fn xlsx_comments_remove(
    file: &str,
    options: XlsxCommentsRemoveOptions<'_>,
) -> CliResult<Value> {
    if !Path::new(file).exists() {
        return Err(CliError::file_not_found(format!("file not found: {file}")));
    }
    if options.handle.is_none() && options.comment_id.is_none() {
        return Err(CliError::invalid_args(
            "either --handle or --comment-id is required",
        ));
    }
    if options.handle.is_some() && options.comment_id.is_some() {
        return Err(CliError::invalid_args(
            "cannot specify both --handle and --comment-id",
        ));
    }
    if options.comment_id.is_some_and(|value| value < 0) {
        return Err(CliError::invalid_args("--comment-id must be >= 0"));
    }
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;

    let (context, comment_id) =
        resolve_comment_mutation_target(file, options.sheet, options.comment_id, options.handle)?;
    let part = find_comments_part(file, &context.part)?;
    if !part.exists {
        return Err(CliError::target_not_found("target not found: comment"));
    }
    let mut doc = read_comments_doc(file, &part.uri)?;
    let target_index = comment_index(&doc.comments, comment_id)?;
    let before = doc.comments[target_index].clone();
    guard_comment_hash(comment_id, options.expect_hash, &before.content_hash)?;
    doc.comments.remove(target_index);
    renumber_comments(&mut doc.comments);
    let removed_part = doc.comments.is_empty();

    let mut text_overrides = BTreeMap::new();
    let mut binary_overrides = BTreeMap::new();
    let mut removals = BTreeSet::new();
    if removed_part {
        removals.insert(part.uri.trim_start_matches('/').to_string());
        remove_comments_relationship(file, &context.part, &mut text_overrides)?;
    } else {
        text_overrides.insert(
            part.uri.trim_start_matches('/').to_string(),
            render_comments_doc(&doc),
        );
    }
    let vml_uri = sync_comments_vml(
        file,
        &context.part,
        &doc.comments,
        &mut text_overrides,
        &mut binary_overrides,
        &mut removals,
    )?;
    let mut content_types = zip_text(file, "[Content_Types].xml")?;
    if removed_part {
        content_types = remove_content_type_override(&content_types, &part.uri);
        if let Some(vml_uri) = vml_uri.as_deref().filter(|value| !value.is_empty()) {
            content_types = remove_content_type_override(&content_types, vml_uri);
        }
    } else if let Some(vml_uri) = vml_uri.as_deref().filter(|value| !value.is_empty()) {
        content_types = ensure_content_type_override(content_types, vml_uri, CONTENT_TYPE_VML);
    }
    text_overrides.insert("[Content_Types].xml".to_string(), content_types);

    let commit_path = write_xlsx_comment_mutation(
        file,
        XlsxCommentOutputOptions {
            out: options.out,
            in_place: options.in_place,
            backup: options.backup,
            dry_run: options.dry_run,
            no_validate: options.no_validate,
        },
        XlsxCommentPackageEdits {
            text_overrides: &text_overrides,
            binary_overrides: &binary_overrides,
            removals: &removals,
        },
    )?;
    let mut result = mutation_base_result(
        file,
        &context,
        &before,
        commit_path.as_deref(),
        options.dry_run,
    );
    result.insert("previousAuthor".to_string(), json!(before.author));
    result.insert("previousText".to_string(), json!(before.text));
    result.insert("previousHash".to_string(), json!(before.content_hash));
    result.insert("anchoredToCell".to_string(), json!(before.anchored_to_cell));
    result.insert("removedPart".to_string(), json!(removed_part));
    result.insert("operation".to_string(), json!("removed"));
    add_comment_readback_commands(&mut result, commit_path.as_deref(), &context.sheet);
    Ok(Value::Object(result))
}

fn resolve_comment_text(
    text: Option<&str>,
    text_file: Option<&str>,
    text_present: bool,
) -> CliResult<String> {
    if text_present && text_file.is_some() {
        return Err(CliError::invalid_args(
            "cannot specify both --text and --text-file",
        ));
    }
    if let Some(path) = text_file {
        return fs::read_to_string(path)
            .map_err(|_| CliError::file_not_found(format!("file not found: {path}")));
    }
    Ok(text.unwrap_or("").to_string())
}

fn resolve_comments_sheet(
    file: &str,
    sheet_selector: Option<&str>,
) -> CliResult<XlsxCommentsSheet> {
    let workbook = zip_text(file, "xl/workbook.xml")?;
    let sheets = workbook_sheets(&workbook)?;
    if sheets.is_empty() {
        return Err(CliError::invalid_args("workbook has no sheets"));
    }
    let sheet = resolve_sheet(&sheets, sheet_selector.unwrap_or("1"))?;
    let rels = relationship_entries_optional(file, "xl/_rels/workbook.xml.rels");
    let target = rels
        .iter()
        .find(|rel| rel.id == sheet.rel_id)
        .map(|rel| rel.target.clone())
        .ok_or_else(|| CliError::unexpected(format!("missing relationship {}", sheet.rel_id)))?;
    let part = normalize_xl_relationship_target(&target);
    if !part.starts_with("xl/worksheets/") {
        return Err(CliError::invalid_args(format!(
            "sheet {:?} is not a worksheet",
            sheet.name
        )));
    }
    Ok(XlsxCommentsSheet {
        sheet,
        part,
        sheets,
    })
}

fn normalize_xl_relationship_target(target: &str) -> String {
    let target = target.trim_start_matches('/');
    if target.starts_with("xl/") {
        target.to_string()
    } else {
        format!("xl/{}", target.trim_start_matches("../"))
    }
}

fn find_comments_part(file: &str, worksheet_part: &str) -> CliResult<XlsxCommentsPart> {
    let entries = zip_entry_names(file)?;
    let worksheet_uri = format!("/{}", worksheet_part.trim_start_matches('/'));
    let rels_part = relationships_part_for(worksheet_part);
    let mut uri = String::new();
    let mut rel_exists = false;
    for rel in relationship_entries_optional(file, &rels_part) {
        if rel.target_mode == "External" {
            continue;
        }
        if rel.rel_type == REL_COMMENTS {
            uri = resolve_relationship_target(&worksheet_uri, &rel.target);
            rel_exists = true;
            break;
        }
    }
    if uri.is_empty() {
        uri = conventional_comments_part_uri(&worksheet_uri);
    }
    let exists = zip_entry_exists(&entries, &uri);
    Ok(XlsxCommentsPart {
        uri,
        exists,
        rel_exists,
    })
}

fn conventional_comments_part_uri(worksheet_uri: &str) -> String {
    let name = worksheet_uri
        .trim_start_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or("sheet1.xml")
        .trim_end_matches(".xml");
    let digits = name
        .chars()
        .rev()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>();
    let digits = if digits.is_empty() {
        "1".to_string()
    } else {
        digits
    };
    format!("/xl/comments{digits}.xml")
}

fn read_comments_doc(file: &str, comments_uri: &str) -> CliResult<XlsxCommentsDoc> {
    let xml = zip_text(file, comments_uri.trim_start_matches('/'))?;
    parse_comments_xml(&xml)
}

fn parse_comments_xml(xml: &str) -> CliResult<XlsxCommentsDoc> {
    let authors = parse_comment_authors(xml)?;
    let comments = parse_comment_entries(xml, &authors)?;
    Ok(XlsxCommentsDoc { authors, comments })
}

fn parse_comment_authors(xml: &str) -> CliResult<Vec<String>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut stack = Vec::<String>::new();
    let mut authors = Vec::new();
    let mut current = String::new();
    let mut in_author = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if name == "author" && stack.last().map(String::as_str) == Some("authors") {
                    in_author = true;
                    current.clear();
                }
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if name == "author" && stack.last().map(String::as_str) == Some("authors") {
                    authors.push(String::new());
                }
            }
            Ok(Event::Text(e)) if in_author => {
                current.push_str(&crate::decode_xml_text(e.as_ref()));
            }
            Ok(Event::GeneralRef(e)) if in_author => {
                current.push_str(&crate::xml_general_ref(e.as_ref()));
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if name == "author" && in_author {
                    authors.push(current.clone());
                    in_author = false;
                }
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(authors)
}

fn parse_comment_entries(xml: &str, authors: &[String]) -> CliResult<Vec<XlsxCommentInfo>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut stack = Vec::<String>::new();
    let mut comments = Vec::<XlsxCommentInfo>::new();
    let mut current: Option<XlsxCommentInfo> = None;
    let mut in_comment_text = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if name == "comment" && stack.last().map(String::as_str) == Some("commentList") {
                    let author_id_attr = crate::attr(&e, "authorId").unwrap_or_default();
                    let author = comment_author(authors, &author_id_attr);
                    let cell = crate::attr(&e, "ref").unwrap_or_default();
                    current = Some(make_comment_info(
                        comments.len() as i64,
                        author_id_attr,
                        author,
                        String::new(),
                        cell,
                    ));
                } else if name == "t"
                    && current.is_some()
                    && stack.iter().any(|item| item == "text")
                {
                    in_comment_text = true;
                }
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if name == "comment" && stack.last().map(String::as_str) == Some("commentList") {
                    let author_id_attr = crate::attr(&e, "authorId").unwrap_or_default();
                    let author = comment_author(authors, &author_id_attr);
                    let cell = crate::attr(&e, "ref").unwrap_or_default();
                    comments.push(make_comment_info(
                        comments.len() as i64,
                        author_id_attr,
                        author,
                        String::new(),
                        cell,
                    ));
                }
            }
            Ok(Event::Text(e)) if in_comment_text => {
                if let Some(comment) = current.as_mut() {
                    comment.text.push_str(&crate::decode_xml_text(e.as_ref()));
                }
            }
            Ok(Event::GeneralRef(e)) if in_comment_text => {
                if let Some(comment) = current.as_mut() {
                    comment.text.push_str(&crate::xml_general_ref(e.as_ref()));
                }
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if name == "t" {
                    in_comment_text = false;
                }
                if name == "comment"
                    && let Some(mut comment) = current.take()
                {
                    refresh_comment_hash_and_anchor(&mut comment);
                    comments.push(comment);
                }
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    renumber_comments(&mut comments);
    Ok(comments)
}

fn comment_author(authors: &[String], author_id_attr: &str) -> String {
    author_id_attr
        .parse::<usize>()
        .ok()
        .and_then(|index| authors.get(index))
        .cloned()
        .unwrap_or_default()
}

fn make_comment_info(
    id: i64,
    author_id_attr: String,
    author: String,
    text: String,
    anchored_to_cell: String,
) -> XlsxCommentInfo {
    let mut comment = XlsxCommentInfo {
        id,
        author_id_attr,
        author,
        text,
        content_hash: String::new(),
        anchored_to_cell,
        anchored_to_cell_row: None,
        anchored_to_cell_column: None,
    };
    refresh_comment_hash_and_anchor(&mut comment);
    comment
}

fn refresh_comment_hash_and_anchor(comment: &mut XlsxCommentInfo) {
    comment.content_hash = comment_content_hash(&comment.author, &comment.text);
    match parse_comment_cell(&comment.anchored_to_cell) {
        Ok((col, row)) => {
            comment.anchored_to_cell_column = Some(col);
            comment.anchored_to_cell_row = Some(row);
        }
        Err(_) => {
            comment.anchored_to_cell_column = None;
            comment.anchored_to_cell_row = None;
        }
    }
}

fn comment_content_hash(author: &str, text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(author.as_bytes());
    hasher.update([0]);
    hasher.update(text.as_bytes());
    format!("sha256:{}", lower_hex(&hasher.finalize()))
}

fn lower_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn sort_comments_by_cell(comments: &mut [XlsxCommentInfo]) {
    comments.sort_by_key(|comment| {
        parse_comment_cell(&comment.anchored_to_cell)
            .map(|(col, row)| (row, col))
            .unwrap_or((u32::MAX, u32::MAX))
    });
}

fn renumber_comments(comments: &mut [XlsxCommentInfo]) {
    for (idx, comment) in comments.iter_mut().enumerate() {
        comment.id = idx as i64;
        refresh_comment_hash_and_anchor(comment);
    }
}

fn comment_index(comments: &[XlsxCommentInfo], comment_id: i64) -> CliResult<usize> {
    comments
        .iter()
        .position(|comment| comment.id == comment_id)
        .ok_or_else(|| CliError::target_not_found("target not found: comment"))
}

fn guard_comment_hash(comment_id: i64, expected: Option<&str>, actual: &str) -> CliResult<()> {
    let expected = expected.unwrap_or("").trim();
    if !expected.is_empty() && expected != actual {
        return Err(CliError::invalid_args(format!(
            "comment hash mismatch: comment {comment_id} expected {expected} but found {actual}"
        )));
    }
    Ok(())
}

fn render_comments_doc(doc: &XlsxCommentsDoc) -> String {
    let mut out = String::new();
    out.push_str(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#);
    out.push_str(&format!(r#"<comments xmlns="{XLSX_NS}">"#));
    out.push_str("<authors>");
    for author in &doc.authors {
        out.push_str("<author>");
        out.push_str(&xml_escape(author));
        out.push_str("</author>");
    }
    out.push_str("</authors><commentList>");
    for comment in &doc.comments {
        out.push_str(&format!(
            r#"<comment ref="{}" authorId="{}">"#,
            xml_attr_escape(&comment.anchored_to_cell),
            xml_attr_escape(&comment.author_id_attr)
        ));
        out.push_str("<text><t");
        if needs_xml_space_preserve(&comment.text) {
            out.push_str(r#" xml:space="preserve""#);
        }
        out.push('>');
        out.push_str(&xml_escape(&comment.text));
        out.push_str("</t></text></comment>");
    }
    out.push_str("</commentList></comments>");
    out
}

fn resolve_comment_mutation_target(
    file: &str,
    sheet_selector: Option<&str>,
    comment_id: Option<i64>,
    handle: Option<&str>,
) -> CliResult<(XlsxCommentsSheet, i64)> {
    if let Some(handle) = handle {
        return resolve_comment_handle_target(file, handle);
    }
    let context = resolve_comments_sheet(file, sheet_selector)?;
    Ok((context, comment_id.unwrap_or(0)))
}

fn resolve_comment_handle_target(file: &str, handle: &str) -> CliResult<(XlsxCommentsSheet, i64)> {
    let (sheet_id, cell) = parse_xlsx_comment_handle(handle)?;
    let workbook = zip_text(file, "xl/workbook.xml")?;
    let sheets = workbook_sheets(&workbook)?;
    let sheet = resolve_sheet_by_sheet_id_unique(&sheets, sheet_id, handle)?;
    let context = resolve_comments_sheet(file, Some(&format!("sheetId:{sheet_id}")))?;
    let listing = if find_comments_part(file, &context.part)?.exists {
        read_comments_doc(file, &find_comments_part(file, &context.part)?.uri)?.comments
    } else {
        Vec::new()
    };
    let matches = listing
        .iter()
        .filter(|comment| comment.anchored_to_cell == cell)
        .map(|comment| comment.id)
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [id] => Ok((
            XlsxCommentsSheet {
                sheet,
                part: context.part,
                sheets,
            },
            *id,
        )),
        [] => Err(CliError::target_not_found(format!(
            "HANDLE_STALE: no comment anchored at {cell} on sheetId \"{sheet_id}\""
        ))),
        _ => Err(CliError::target_not_found(format!(
            "HANDLE_AMBIGUOUS: {} comments anchored at {cell} on sheetId \"{sheet_id}\"; cannot resolve to one",
            matches.len()
        ))),
    }
}

fn parse_xlsx_comment_handle(handle: &str) -> CliResult<(u32, String)> {
    let value = handle.trim();
    let body = value.trim_start_matches("H:");
    let parts = body.split('/').collect::<Vec<_>>();
    if parts.first().copied() != Some("xlsx") {
        return Err(CliError::invalid_args(format!(
            "HANDLE_FORMAT_MISMATCH: handle format tag does not match package format \"xlsx\" (handle {value:?})"
        )));
    }
    if parts.len() != 3 {
        return Err(CliError::invalid_args(
            "expected a comment handle (H:xlsx/ws:<sheetId>/comment:a:<A1>)",
        ));
    }
    let Some(sheet_id) = parts[1].strip_prefix("ws:") else {
        return Err(CliError::invalid_args(format!(
            "HANDLE_MALFORMED: worksheet scope is malformed (handle {value:?})"
        )));
    };
    let sheet_id = sheet_id.parse::<u32>().map_err(|_| {
        CliError::invalid_args(format!(
            "HANDLE_MALFORMED: worksheet sheetId must be numeric (handle {value:?})"
        ))
    })?;
    let Some(cell) = parts[2].strip_prefix("comment:a:") else {
        return Err(CliError::invalid_args(
            "expected a comment handle (H:xlsx/ws:<sheetId>/comment:a:<A1>)",
        ));
    };
    Ok((sheet_id, normalize_xlsx_cell_ref(cell, "comment handle")?))
}

fn ensure_comments_relationship(
    file: &str,
    worksheet_part: &str,
    comments_uri: &str,
    text_overrides: &mut BTreeMap<String, String>,
) -> CliResult<bool> {
    let part = find_comments_part(file, worksheet_part)?;
    let worksheet_uri = format!("/{}", worksheet_part.trim_start_matches('/'));
    let rels_part = relationships_part_for(worksheet_part);
    let rels_xml = relationships_xml_for_edit(file, &rels_part, text_overrides);
    let rels = relationship_entries_from_xml(&rels_xml);
    if part.rel_exists
        || rels
            .iter()
            .any(|rel| rel.rel_type == REL_COMMENTS && rel.target_mode != "External")
    {
        return Ok(false);
    }
    let id = allocate_relationship_id(&rels);
    let target = relationship_target_from_source_to_target(&worksheet_uri, comments_uri);
    text_overrides.insert(
        rels_part,
        add_relationship_to_xml(rels_xml, &id, REL_COMMENTS, &target),
    );
    Ok(true)
}

fn remove_comments_relationship(
    file: &str,
    worksheet_part: &str,
    text_overrides: &mut BTreeMap<String, String>,
) -> CliResult<()> {
    let rels_part = relationships_part_for(worksheet_part);
    let rels_xml = relationships_xml_for_edit(file, &rels_part, text_overrides);
    let updated = rewrite_relationships_xml(&rels_xml, |rel| {
        if rel.rel_type == REL_COMMENTS {
            None
        } else {
            Some(render_relationship(rel))
        }
    });
    if updated != rels_xml {
        text_overrides.insert(rels_part, updated);
    }
    Ok(())
}

fn sync_comments_vml(
    file: &str,
    worksheet_part: &str,
    comments: &[XlsxCommentInfo],
    text_overrides: &mut BTreeMap<String, String>,
    binary_overrides: &mut BTreeMap<String, Vec<u8>>,
    removals: &mut BTreeSet<String>,
) -> CliResult<Option<String>> {
    let (vml_uri, vml_exists) = find_vml_drawing_part(file, worksheet_part)?;
    if comments.is_empty() {
        if vml_exists && !vml_uri.is_empty() {
            removals.insert(vml_uri.trim_start_matches('/').to_string());
        }
        remove_vml_relationship(file, worksheet_part, text_overrides)?;
        remove_worksheet_legacy_drawing(file, worksheet_part, text_overrides)?;
        return Ok((!vml_uri.is_empty()).then_some(vml_uri));
    }

    let vml_uri = if vml_uri.is_empty() {
        allocate_numbered_part(file, "/xl/drawings/vmlDrawing", ".vml")?
    } else {
        vml_uri
    };
    binary_overrides.insert(
        vml_uri.trim_start_matches('/').to_string(),
        build_comments_vml(comments)?,
    );
    let rid = ensure_vml_relationship(file, worksheet_part, &vml_uri, text_overrides)?;
    add_worksheet_legacy_drawing_ref(file, worksheet_part, &rid, text_overrides)?;
    Ok(Some(vml_uri))
}

fn find_vml_drawing_part(file: &str, worksheet_part: &str) -> CliResult<(String, bool)> {
    let entries = zip_entry_names(file)?;
    let worksheet_uri = format!("/{}", worksheet_part.trim_start_matches('/'));
    let rels_part = relationships_part_for(worksheet_part);
    let mut uri = String::new();
    for rel in relationship_entries_optional(file, &rels_part) {
        if rel.target_mode == "External" {
            continue;
        }
        if rel.rel_type == REL_VML_DRAWING {
            uri = resolve_relationship_target(&worksheet_uri, &rel.target);
            break;
        }
    }
    let exists = !uri.is_empty() && zip_entry_exists(&entries, &uri);
    Ok((uri, exists))
}

fn ensure_vml_relationship(
    file: &str,
    worksheet_part: &str,
    vml_uri: &str,
    text_overrides: &mut BTreeMap<String, String>,
) -> CliResult<String> {
    let worksheet_uri = format!("/{}", worksheet_part.trim_start_matches('/'));
    let rels_part = relationships_part_for(worksheet_part);
    let rels_xml = relationships_xml_for_edit(file, &rels_part, text_overrides);
    let rels = relationship_entries_from_xml(&rels_xml);
    if let Some(rel) = rels
        .iter()
        .find(|rel| rel.rel_type == REL_VML_DRAWING && rel.target_mode != "External")
    {
        return Ok(rel.id.clone());
    }
    let id = allocate_relationship_id(&rels);
    let target = relationship_target_from_source_to_target(&worksheet_uri, vml_uri);
    text_overrides.insert(
        rels_part,
        add_relationship_to_xml(rels_xml, &id, REL_VML_DRAWING, &target),
    );
    Ok(id)
}

fn remove_vml_relationship(
    file: &str,
    worksheet_part: &str,
    text_overrides: &mut BTreeMap<String, String>,
) -> CliResult<()> {
    let rels_part = relationships_part_for(worksheet_part);
    let rels_xml = relationships_xml_for_edit(file, &rels_part, text_overrides);
    let updated = rewrite_relationships_xml(&rels_xml, |rel| {
        if rel.rel_type == REL_VML_DRAWING {
            None
        } else {
            Some(render_relationship(rel))
        }
    });
    if updated != rels_xml {
        text_overrides.insert(rels_part, updated);
    }
    Ok(())
}

fn relationship_entries_optional(file: &str, part: &str) -> Vec<RelationshipEntry> {
    zip_text(file, part)
        .map(|xml| relationship_entries_from_xml(&xml))
        .unwrap_or_default()
}

fn relationships_xml_for_edit(
    file: &str,
    rels_part: &str,
    text_overrides: &BTreeMap<String, String>,
) -> String {
    text_overrides
        .get(rels_part)
        .cloned()
        .unwrap_or_else(|| zip_text(file, rels_part).unwrap_or_else(|_| relationships_template()))
}

fn relationships_template() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="{REL_NS}"></Relationships>"#
    )
}

fn rewrite_relationships_xml<F>(xml: &str, mut mapper: F) -> String
where
    F: FnMut(&RelationshipEntry) -> Option<String>,
{
    let body = relationship_entries_from_xml(xml)
        .iter()
        .filter_map(&mut mapper)
        .collect::<String>();
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

fn build_comments_vml(comments: &[XlsxCommentInfo]) -> CliResult<Vec<u8>> {
    let mut out = String::new();
    out.push_str(r#"<xml xmlns:v="urn:schemas-microsoft-com:vml" xmlns:o="urn:schemas-microsoft-com:office:office" xmlns:x="urn:schemas-microsoft-com:office:excel">"#);
    out.push_str(r#"<o:shapelayout v:ext="edit"><o:idmap v:ext="edit" data="1"/></o:shapelayout>"#);
    out.push_str(r#"<v:shapetype id="_x0000_t202" coordsize="21600,21600" o:spt="202" path="m,l,21600r21600,l21600,xe">"#);
    out.push_str(r#"<v:stroke joinstyle="miter"/><v:path gradientshapeok="t" o:connecttype="rect"/></v:shapetype>"#);
    for (idx, comment) in comments.iter().enumerate() {
        let (col, row) = parse_comment_cell(&comment.anchored_to_cell)?;
        let row0 = row - 1;
        let col0 = col - 1;
        let shape_id = format!("_x0000_s{}", 1025 + idx);
        out.push_str(&format!(
            r##"<v:shape id="{shape_id}" type="#_x0000_t202" style="position:absolute;visibility:hidden" fillcolor="#ffffe1" o:insetmode="auto">"##
        ));
        out.push_str(r##"<v:fill color2="#ffffe1"/><v:shadow color="black" obscured="t"/>"##);
        out.push_str(r#"<v:path o:connecttype="none"/><v:textbox style="mso-direction-alt:auto"><div style="text-align:left"/></v:textbox>"#);
        out.push_str(r#"<x:ClientData ObjectType="Note">"#);
        out.push_str(r#"<x:MoveWithCells/><x:SizeWithCells/>"#);
        out.push_str(&format!(
            r#"<x:Anchor>{}, 15, {}, 2, {}, 31, {}, 4</x:Anchor>"#,
            col0 + 1,
            row0,
            col0 + 3,
            row0 + 4
        ));
        out.push_str(r#"<x:AutoFill>False</x:AutoFill>"#);
        out.push_str(&format!(r#"<x:Row>{row0}</x:Row>"#));
        out.push_str(&format!(r#"<x:Column>{col0}</x:Column>"#));
        out.push_str("</x:ClientData></v:shape>");
    }
    out.push_str("</xml>");
    Ok(out.into_bytes())
}

fn parse_comment_cell(cell: &str) -> CliResult<(u32, u32)> {
    let normalized = normalize_xlsx_cell_ref(cell, "comment ref")?;
    crate::parse_cell_ref(&normalized)
}

fn allocate_numbered_part(file: &str, prefix: &str, suffix: &str) -> CliResult<String> {
    let entries = zip_entry_names(file)?;
    for idx in 1.. {
        let uri = format!("{prefix}{idx}{suffix}");
        if !zip_entry_exists(&entries, &uri) {
            return Ok(uri);
        }
    }
    unreachable!("infinite range must return a part")
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

fn add_worksheet_legacy_drawing_ref(
    file: &str,
    worksheet_part: &str,
    rid: &str,
    text_overrides: &mut BTreeMap<String, String>,
) -> CliResult<()> {
    let worksheet_xml = current_text_part(file, worksheet_part, text_overrides)?;
    let root = worksheet_root_bounds(&worksheet_xml)?;
    let prefix = xml_tag_prefix(&root.tag_name);
    let legacy_xml = format!(
        r#"<{} r:id="{}"/>"#,
        element_name(&prefix, "legacyDrawing"),
        xml_attr_escape(rid)
    );
    let updated = if let Some(existing) =
        direct_worksheet_child_range(&worksheet_xml, &root, "legacyDrawing")?
    {
        replace_xml_span(&worksheet_xml, existing.start, existing.end, &legacy_xml)
    } else {
        insert_worksheet_child(&worksheet_xml, &root, "legacyDrawing", &legacy_xml)?
    };
    let updated = ensure_relationships_namespace(updated, &root)?;
    text_overrides.insert(worksheet_part.to_string(), updated);
    Ok(())
}

fn remove_worksheet_legacy_drawing(
    file: &str,
    worksheet_part: &str,
    text_overrides: &mut BTreeMap<String, String>,
) -> CliResult<()> {
    let worksheet_xml = current_text_part(file, worksheet_part, text_overrides)?;
    let root = worksheet_root_bounds(&worksheet_xml)?;
    if let Some(existing) = direct_worksheet_child_range(&worksheet_xml, &root, "legacyDrawing")? {
        text_overrides.insert(
            worksheet_part.to_string(),
            remove_xml_span(&worksheet_xml, existing.start, existing.end),
        );
    }
    Ok(())
}

fn current_text_part(
    file: &str,
    part: &str,
    text_overrides: &BTreeMap<String, String>,
) -> CliResult<String> {
    text_overrides
        .get(part)
        .cloned()
        .map(Ok)
        .unwrap_or_else(|| zip_text(file, part))
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
    local_name: &str,
) -> CliResult<Option<crate::XmlNamedRange>> {
    Ok(
        xml_direct_child_ranges(xml, root.open_end, root.close_start)?
            .into_iter()
            .find(|child| child.kind == local_name),
    )
}

fn insert_worksheet_child(
    xml: &str,
    root: &WorksheetRootBounds,
    local_name: &str,
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
    let target_order = worksheet_child_order(local_name);
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

fn ensure_relationships_namespace(xml: String, root: &WorksheetRootBounds) -> CliResult<String> {
    let open = &xml[root.start..root.open_end];
    if open.contains("xmlns:r=") {
        return Ok(xml);
    }
    let insert_at = root.open_end.saturating_sub(1);
    Ok(replace_xml_span(
        &xml,
        insert_at,
        insert_at,
        &format!(r#" xmlns:r="{OFFICE_REL_NS}""#),
    ))
}

fn element_name(prefix: &str, local: &str) -> String {
    if prefix.is_empty() {
        local.to_string()
    } else {
        format!("{prefix}:{local}")
    }
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

fn remove_content_type_override(xml: &str, part_uri: &str) -> String {
    let normalized = format!("/{}", part_uri.trim_start_matches('/'));
    remove_xml_elements_matching(xml, "Override", |attrs| {
        attrs
            .get("PartName")
            .is_some_and(|value| value == &normalized)
    })
}

fn remove_xml_elements_matching<F>(xml: &str, element_local: &str, predicate: F) -> String
where
    F: Fn(&BTreeMap<String, String>) -> bool,
{
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut spans = Vec::<(usize, usize)>::new();
    loop {
        let start = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == element_local => {
                if predicate(&xml_attrs_map(&e)) {
                    spans.push((start, reader.buffer_position() as usize));
                }
            }
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == element_local => {
                if predicate(&xml_attrs_map(&e)) {
                    let mut depth = 1usize;
                    loop {
                        match reader.read_event() {
                            Ok(Event::Start(inner))
                                if local_name(inner.name().as_ref()) == element_local =>
                            {
                                depth += 1;
                            }
                            Ok(Event::End(inner))
                                if local_name(inner.name().as_ref()) == element_local =>
                            {
                                depth -= 1;
                                if depth == 0 {
                                    spans.push((start, reader.buffer_position() as usize));
                                    break;
                                }
                            }
                            Ok(Event::Eof) | Err(_) => {
                                spans.push((start, reader.buffer_position() as usize));
                                break;
                            }
                            _ => {}
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    if spans.is_empty() {
        return xml.to_string();
    }
    let mut out = String::with_capacity(xml.len());
    let mut cursor = 0usize;
    for (start, end) in spans {
        if start > cursor {
            out.push_str(&xml[cursor..start]);
        }
        cursor = end;
    }
    out.push_str(&xml[cursor..]);
    out
}

fn write_xlsx_comment_mutation(
    file: &str,
    options: XlsxCommentOutputOptions<'_>,
    edits: XlsxCommentPackageEdits<'_>,
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
    copy_zip_with_binary_part_overrides_and_removals(
        file,
        &readback_path,
        edits.text_overrides,
        edits.binary_overrides,
        edits.removals,
    )?;
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
