use quick_xml::Reader;
use quick_xml::events::Event;
use regex::Regex;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

use crate::cli_args::{flag_present, value_flag_present};
use crate::cli_dispatch::{DispatchBody, DispatchOutput};
use crate::{
    CliError, CliResult, EXIT_SUCCESS, GlobalFlags, InspectPackageKind, apply, attr, attr_exact,
    command_arg, detect_inspect_package_type, docx_rich_block_json, docx_rich_block_reports,
    find_docx_document_part, find_xlsx_workbook_part, has_flag, local_name, package_type,
    parse_i64_flag, parse_string_flag, pptx_extract_notes, pptx_extract_text, reject_unknown_flags,
    relationship_entries, relationship_entries_from_xml, relationships_part_for,
    resolve_relationship_target, shared_strings, sheet_cells, workbook_sheets, xlsx_names_list,
    xlsx_styles, zip_entry_names, zip_text,
};

const FIND_CONTRACT_VERSION: &str = "ooxml-find.v1";

struct FindOptions {
    query: String,
    search_type: String,
    ignore_case: bool,
    regex: bool,
    max: i64,
    to_ops: bool,
    apply: bool,
    replace: String,
}

struct Matcher {
    query: String,
    ignore_case: bool,
    regex: Option<Regex>,
}

#[derive(Clone, Default)]
struct OpSpec {
    command: String,
    args: Vec<OpArg>,
    replace_key: String,
    replace_token: String,
    handle_key: String,
    handle: String,
    position_independent: bool,
}

#[derive(Clone)]
struct OpArg {
    key: String,
    value: String,
}

#[derive(Clone)]
struct FindHit {
    json: Value,
    op: Option<OpSpec>,
}

struct OpsResult {
    ops: Vec<Value>,
    skipped: Vec<usize>,
    position_dependent: Vec<usize>,
    duplicates: Vec<usize>,
}

const NEW_OP_PLACEHOLDER: &str = "<NEW>";

impl OpSpec {
    fn new(command: &str, args: Vec<(&str, String)>, replace_key: &str) -> Self {
        Self {
            command: command.to_string(),
            args: args
                .into_iter()
                .map(|(key, value)| OpArg {
                    key: key.to_string(),
                    value,
                })
                .collect(),
            replace_key: replace_key.to_string(),
            replace_token: NEW_OP_PLACEHOLDER.to_string(),
            handle_key: String::new(),
            handle: String::new(),
            position_independent: false,
        }
    }

    fn with_handle(mut self, key: &str, handle: String) -> Self {
        self.handle_key = key.to_string();
        self.handle = handle;
        self
    }

    fn with_optional_handle(self, key: &str, handle: String) -> Self {
        if handle.is_empty() {
            self
        } else {
            self.with_handle(key, handle)
        }
    }

    fn with_replace_token(mut self, token: String) -> Self {
        self.replace_token = token;
        self
    }

    fn human_command(&self) -> String {
        let mut text = format!("ooxml --json {} <file>", self.command);
        for arg in &self.args {
            text.push_str(" --");
            text.push_str(&arg.key);
            text.push(' ');
            if arg.key == self.replace_key && arg.value == NEW_OP_PLACEHOLDER {
                text.push_str(NEW_OP_PLACEHOLDER);
            } else {
                text.push_str(&command_arg(&arg.value));
            }
        }
        text.push_str(" --out <OUT>");
        text
    }
}

fn hits_to_ops(hits: &[FindHit], new_value: &str) -> OpsResult {
    let replacement = if new_value.is_empty() {
        NEW_OP_PLACEHOLDER
    } else {
        new_value
    };
    let mut result = OpsResult {
        ops: Vec::new(),
        skipped: Vec::new(),
        position_dependent: Vec::new(),
        duplicates: Vec::new(),
    };
    let mut seen = BTreeSet::<String>::new();
    for (index, hit) in hits.iter().enumerate() {
        let Some(spec) = &hit.op else {
            result.skipped.push(index);
            continue;
        };
        let use_handle = !spec.handle_key.is_empty() && !spec.handle.is_empty();
        let mut args = Map::new();
        for arg in &spec.args {
            let mut value = arg.value.clone();
            if arg.key == spec.replace_key {
                if value.contains(&spec.replace_token) {
                    value = value.replace(&spec.replace_token, replacement);
                } else {
                    value = replacement.to_string();
                }
            }
            if use_handle && arg.key == spec.handle_key {
                value.clone_from(&spec.handle);
            }
            args.insert(arg.key.clone(), Value::String(value));
        }
        let op = json!({
            "command": spec.command,
            "args": args,
        });
        let identity = operation_identity(&op);
        if !seen.insert(identity) {
            result.duplicates.push(index);
            continue;
        }
        if (!use_handle && !spec.position_independent)
            || (use_handle && is_address_positional(&spec.handle))
        {
            result.position_dependent.push(index);
        }
        result.ops.push(op);
    }
    result
}

fn operation_identity(op: &Value) -> String {
    let command = op["command"].as_str().unwrap_or_default();
    let mut text = format!("{}:{command}", command.len());
    if let Some(args) = op["args"].as_object() {
        for (key, value) in args {
            let value = value.as_str().unwrap_or_default();
            text.push_str(&format!("|{}:{key}={}:{}", key.len(), value.len(), value));
        }
    }
    text
}

fn is_address_positional(handle: &str) -> bool {
    handle.starts_with("H:xlsx/ws:") && (handle.contains("/cell:") || handle.contains("/comment:"))
}

fn report_find_ops_diagnostics(ops_count: usize, ops: &OpsResult, hits_found: usize) {
    if !ops.skipped.is_empty() {
        eprintln!(
            "find->ops: {hits_found} hit(s), {ops_count} op(s), {} skipped (no mutation command): hits {:?}",
            ops.skipped.len(),
            ops.skipped
        );
    }
    if !ops.position_dependent.is_empty() {
        eprintln!(
            "find->ops: {} op(s) are position-dependent (no stable handle; may break if an earlier batch op shifts their position): hits {:?}",
            ops.position_dependent.len(),
            ops.position_dependent
        );
    }
    if !ops.duplicates.is_empty() {
        eprintln!(
            "find->ops: {} hit(s) collapsed into an earlier identical op (the op already covers them): hits {:?}",
            ops.duplicates.len(),
            ops.duplicates
        );
    }
}

fn find_apply(
    file: &str,
    find_args: &[String],
    hits: &[FindHit],
    options: &FindOptions,
) -> CliResult<Value> {
    let ops = hits_to_ops(hits, &options.replace);
    report_find_ops_diagnostics(ops.ops.len(), &ops, hits.len());
    let ops_path = write_temp_ops_file(&ops.ops)?;
    let mut apply_args = vec!["--ops".to_string(), ops_path.to_string_lossy().to_string()];
    if let Some(out) = parse_string_flag(find_args, "--out")? {
        apply_args.extend(["--out".to_string(), out]);
    }
    if let Some(backup) = parse_string_flag(find_args, "--backup")? {
        apply_args.extend(["--backup".to_string(), backup]);
    }
    for flag in ["--dry-run", "--in-place", "--no-validate"] {
        if flag_present(find_args, flag) {
            apply_args.push(flag.to_string());
        }
    }
    let result = apply(file, &apply_args);
    let _ = fs::remove_file(&ops_path);
    result
}

fn write_temp_ops_file(ops: &[Value]) -> CliResult<PathBuf> {
    let path = std::env::temp_dir().join(format!(
        "ooxml-find-ops-{}-{}.json",
        std::process::id(),
        crate::chrono_like_counter()
    ));
    let data = serde_json::to_vec(ops)
        .map_err(|err| CliError::unexpected(format!("failed to encode operations: {err}")))?;
    fs::write(&path, data).map_err(|err| {
        CliError::unexpected(format!("failed to write temporary ops file: {err}"))
    })?;
    Ok(path)
}

pub(crate) fn find(flags: &GlobalFlags, args: &[String]) -> CliResult<DispatchOutput> {
    match args {
        [sub, rest @ ..] if sub == "capabilities" => find_capabilities(rest),
        [sub, rest @ ..] if sub == "robot-docs" => find_robot_docs(rest),
        [query, file, rest @ ..] => find_search(flags, query, file, rest),
        _ => Err(CliError::invalid_args(
            "find requires <query> <file>, or use find capabilities / find robot-docs",
        )),
    }
}

fn find_search(
    flags: &GlobalFlags,
    query: &str,
    file: &str,
    args: &[String],
) -> CliResult<DispatchOutput> {
    reject_unknown_flags(
        args,
        &[
            "--type",
            "--max",
            "--format",
            "--replace",
            "--out",
            "--backup",
        ],
        &[
            "--ignore-case",
            "--regex",
            "--json",
            "--to-ops",
            "--apply",
            "--dry-run",
            "--in-place",
            "--no-validate",
        ],
    )?;
    validate_find_compose_flags(args)?;
    if query.is_empty() {
        return Err(CliError::invalid_args("query must not be empty"));
    }
    if fs::metadata(file).is_err() {
        return Err(CliError::file_not_found(format!("file not found: {file}")));
    }

    let search_type = parse_string_flag(args, "--type")?.unwrap_or_else(|| "all".to_string());
    if !matches!(search_type.as_str(), "all" | "text" | "formula" | "name") {
        return Err(CliError::invalid_args(format!(
            "invalid --type: {search_type} (expected all, text, formula, or name)"
        )));
    }
    let max = parse_i64_flag(args, "--max")?.unwrap_or(0);
    if max < 0 {
        return Err(CliError::invalid_args("--max must be >= 0"));
    }
    let options = FindOptions {
        query: query.to_string(),
        search_type,
        ignore_case: has_flag(args, "--ignore-case"),
        regex: has_flag(args, "--regex"),
        max,
        to_ops: has_flag(args, "--to-ops"),
        apply: has_flag(args, "--apply"),
        replace: parse_string_flag(args, "--replace")?.unwrap_or_default(),
    };
    let matcher = Matcher::new(&options)?;
    let package = detect_package(file)?;
    let mut hits = match package.as_str() {
        "pptx" => search_pptx(file, &matcher, &options)?,
        "xlsx" => search_xlsx(file, &matcher, &options)?,
        "docx" => search_docx(file, &matcher, &options)?,
        _ => {
            return Err(CliError::unsupported_type(format!(
                "unsupported package type: {package}"
            )));
        }
    };
    let truncated = options.max > 0 && hits.len() > options.max as usize;
    if truncated {
        hits.truncate(options.max as usize);
    }
    for (index, hit) in hits.iter_mut().enumerate() {
        hit.json["index"] = json!(index);
    }
    if options.to_ops {
        let ops = hits_to_ops(&hits, &options.replace);
        report_find_ops_diagnostics(ops.ops.len(), &ops, hits.len());
        return Ok(DispatchOutput {
            body: DispatchBody::Json(Value::Array(ops.ops)),
            exit_code: EXIT_SUCCESS,
        });
    }
    if options.apply {
        let value = find_apply(file, args, &hits, &options)?;
        return Ok(DispatchOutput {
            body: DispatchBody::Json(value),
            exit_code: EXIT_SUCCESS,
        });
    }
    let json_hits = hits.into_iter().map(|hit| hit.json).collect::<Vec<_>>();
    let value = json!({
        "contractVersion": FIND_CONTRACT_VERSION,
        "packageType": package,
        "query": options.query,
        "type": options.search_type,
        "ignoreCase": options.ignore_case,
        "regex": options.regex,
        "max": options.max,
        "truncated": truncated,
        "totalHits": json_hits.len(),
        "hits": json_hits,
    });
    if wants_json(flags, args) {
        Ok(DispatchOutput {
            body: DispatchBody::Json(value),
            exit_code: EXIT_SUCCESS,
        })
    } else {
        Ok(DispatchOutput {
            body: DispatchBody::Text(render_find_text(&value)),
            exit_code: EXIT_SUCCESS,
        })
    }
}

fn validate_find_compose_flags(args: &[String]) -> CliResult<()> {
    let to_ops = has_flag(args, "--to-ops");
    let apply_flag = has_flag(args, "--apply");
    let replace_present = value_flag_present(args, "--replace");
    let replace = parse_string_flag(args, "--replace")?;
    if to_ops && apply_flag {
        return Err(CliError::invalid_args(
            "--to-ops and --apply are mutually exclusive; use --to-ops to emit ops, or --apply to run them",
        ));
    }
    if apply_flag && !replace_present {
        return Err(CliError::invalid_args("--apply requires --replace <new>"));
    }
    if apply_flag && replace_present && replace.as_deref().unwrap_or_default().is_empty() {
        return Err(CliError::invalid_args(
            "--replace must be non-empty with --apply (use --to-ops to emit a placeholder)",
        ));
    }
    if replace_present && !to_ops && !apply_flag {
        return Err(CliError::invalid_args(
            "--replace requires --to-ops (emit) or --apply (run); find is read-only otherwise",
        ));
    }
    Ok(())
}

fn find_capabilities(args: &[String]) -> CliResult<DispatchOutput> {
    reject_unknown_flags(args, &["--format"], &["--json"])?;
    Ok(DispatchOutput {
        body: DispatchBody::Json(json!({
            "tool": "ooxml",
            "contractVersion": FIND_CONTRACT_VERSION,
            "readOnly": true,
            "packageTypes": ["pptx", "xlsx", "docx"],
            "searchTypes": [
                {"name": "all", "description": "search all supported text, formula, and name fields"},
                {"name": "text", "description": "search PPTX/DOCX text and XLSX cell values"},
                {"name": "formula", "description": "search XLSX formulas only"},
                {"name": "name", "description": "search XLSX defined names only"}
            ],
            "hitKinds": [
                "pptx-text",
                "pptx-notes",
                "xlsx-value",
                "xlsx-formula",
                "xlsx-name",
                "docx-text"
            ],
            "flags": [
                {"name": "--type", "type": "string", "description": "one of all, text, formula, or name"},
                {"name": "--ignore-case", "type": "bool", "description": "case-insensitive matching"},
                {"name": "--regex", "type": "bool", "description": "treat query as a Rust regular expression"},
                {"name": "--max", "type": "int", "description": "maximum hits to return; 0 means unlimited"},
                {"name": "--to-ops", "type": "bool", "description": "emit apply-compatible operations as a bare JSON array"},
                {"name": "--replace", "type": "string", "description": "replacement value substituted into generated ops"},
                {"name": "--apply", "type": "bool", "description": "run generated ops through the apply engine"},
                {"name": "--out", "type": "string", "description": "output file for --apply"},
                {"name": "--in-place", "type": "bool", "description": "write --apply changes in place"},
                {"name": "--dry-run", "type": "bool", "description": "plan --apply without writing"},
                {"name": "--backup", "type": "string", "description": "backup path for --in-place"},
                {"name": "--no-validate", "type": "bool", "description": "skip final validation for --apply"},
                {"name": "--json", "type": "bool", "description": "emit machine-readable JSON"}
            ],
            "exitCodes": [
                {"code": 0, "description": "search completed, including zero-hit searches"},
                {"code": 2, "description": "invalid arguments or invalid regex"},
                {"code": 3, "description": "file not found"},
                {"code": 4, "description": "unsupported package type"}
            ],
            "notes": [
                "find is read-only by default; --to-ops is also read-only and prints operations for ooxml apply.",
                "--replace <new> --apply mutates only through the apply engine and requires --out, --in-place, or --dry-run.",
                "Hits with no semantic mutation command are skipped from generated ops and reported on stderr.",
                "Each hit includes a mutationCommand only when a direct semantic edit command exists for that hit kind."
            ]
        })),
        exit_code: EXIT_SUCCESS,
    })
}

fn find_robot_docs(args: &[String]) -> CliResult<DispatchOutput> {
    reject_unknown_flags(args, &[], &[])?;
    Ok(DispatchOutput {
        body: DispatchBody::Text(FIND_ROBOT_DOCS.to_string()),
        exit_code: EXIT_SUCCESS,
    })
}

impl Matcher {
    fn new(options: &FindOptions) -> CliResult<Self> {
        let regex = if options.regex {
            let pattern = if options.ignore_case {
                format!("(?i){}", options.query)
            } else {
                options.query.clone()
            };
            Some(Regex::new(&pattern).map_err(|err| {
                CliError::invalid_args(format!("invalid regular expression: {err}"))
            })?)
        } else {
            None
        };
        Ok(Self {
            query: options.query.clone(),
            ignore_case: options.ignore_case,
            regex,
        })
    }

    fn find(&self, value: &str) -> Option<String> {
        if value.is_empty() {
            return None;
        }
        if let Some(regex) = &self.regex {
            return regex.find(value).map(|mat| mat.as_str().to_string());
        }
        if self.ignore_case {
            let folded_value = value.to_lowercase();
            let folded_query = self.query.to_lowercase();
            let index = folded_value.find(&folded_query)?;
            let mut byte_start = 0usize;
            let mut folded_seen = 0usize;
            for (original_index, ch) in value.char_indices() {
                if folded_seen == index {
                    byte_start = original_index;
                    break;
                }
                folded_seen += ch.to_lowercase().to_string().len();
            }
            let mut byte_end = value.len();
            let target_end = index + folded_query.len();
            folded_seen = 0;
            for (original_index, ch) in value.char_indices() {
                folded_seen += ch.to_lowercase().to_string().len();
                if folded_seen >= target_end {
                    byte_end = original_index + ch.len_utf8();
                    break;
                }
            }
            return value.get(byte_start..byte_end).map(ToOwned::to_owned);
        }
        let index = value.find(&self.query)?;
        value
            .get(index..index + self.query.len())
            .map(ToOwned::to_owned)
    }
}

fn detect_package(file: &str) -> CliResult<String> {
    let entries = zip_entry_names(file)?;
    let detected = detect_inspect_package_type(file, &entries);
    Ok(match detected {
        InspectPackageKind::Pptx => "pptx".to_string(),
        InspectPackageKind::Xlsx => "xlsx".to_string(),
        InspectPackageKind::Docx => "docx".to_string(),
        InspectPackageKind::Unknown => package_type(file)?.to_string(),
    })
}

struct PptxFindSlideInfo {
    number: u32,
    slide_id: u32,
    part: String,
    slide_id_unique: bool,
    shape_id_counts: BTreeMap<u32, usize>,
}

impl PptxFindSlideInfo {
    fn slide_handle(&self) -> Option<String> {
        if self.slide_id != 0 && self.slide_id_unique {
            Some(format!("H:pptx/s:{}", self.slide_id))
        } else {
            None
        }
    }

    fn shape_handle(&self, shape_id: u32) -> Option<String> {
        if shape_id == 0
            || self.slide_id == 0
            || !self.slide_id_unique
            || self
                .shape_id_counts
                .get(&shape_id)
                .copied()
                .unwrap_or_default()
                != 1
        {
            None
        } else {
            Some(format!("H:pptx/s:{}/shape:n:{shape_id}", self.slide_id))
        }
    }
}

fn pptx_find_slide_infos(file: &str) -> CliResult<Vec<PptxFindSlideInfo>> {
    let presentation = zip_text(file, "ppt/presentation.xml")?;
    let slide_ids = presentation_slide_refs(&presentation);
    let rels = relationship_entries_from_xml(&zip_text(file, "ppt/_rels/presentation.xml.rels")?);
    let mut id_counts = BTreeMap::<u32, usize>::new();
    for (slide_id, _) in &slide_ids {
        *id_counts.entry(*slide_id).or_default() += 1;
    }
    slide_ids
        .into_iter()
        .enumerate()
        .map(|(index, (slide_id, rel_id))| {
            let rel = rels
                .iter()
                .find(|candidate| candidate.id == rel_id)
                .ok_or_else(|| CliError::unexpected(format!("missing relationship {rel_id}")))?;
            let part = resolve_relationship_target("/ppt/presentation.xml", &rel.target)
                .trim_start_matches('/')
                .to_string();
            let slide_xml = zip_text(file, &part)?;
            Ok(PptxFindSlideInfo {
                number: index as u32 + 1,
                slide_id,
                part,
                slide_id_unique: id_counts.get(&slide_id).copied().unwrap_or_default() == 1,
                shape_id_counts: pptx_top_level_shape_id_counts(&slide_xml),
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
                if let (Some(id), Some(rel)) = (attr_exact(&e, "id"), attr_exact(&e, "r:id"))
                    && let Ok(id) = id.parse::<u32>()
                {
                    slides.push((id, rel));
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    slides
}

fn pptx_top_level_shape_id_counts(xml: &str) -> BTreeMap<u32, usize> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut path = Vec::<String>::new();
    let mut current: Option<(String, usize, Option<u32>)> = None;
    let mut counts = BTreeMap::<u32, usize>::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if current.is_none()
                    && path.last().map(String::as_str) == Some("spTree")
                    && matches!(name.as_str(), "sp" | "graphicFrame")
                {
                    current = Some((name.clone(), path.len() + 1, None));
                } else if let Some((_, _, id)) = current.as_mut()
                    && id.is_none()
                    && name == "cNvPr"
                    && let Some(raw) = attr(&e, "id")
                    && let Ok(parsed) = raw.parse::<u32>()
                {
                    *id = Some(parsed);
                }
                path.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if current.is_none()
                    && path.last().map(String::as_str) == Some("spTree")
                    && matches!(name.as_str(), "sp" | "graphicFrame")
                    && let Some(raw) = attr(&e, "id")
                    && let Ok(parsed) = raw.parse::<u32>()
                {
                    *counts.entry(parsed).or_default() += 1;
                } else if let Some((_, _, id)) = current.as_mut()
                    && id.is_none()
                    && name == "cNvPr"
                    && let Some(raw) = attr(&e, "id")
                    && let Ok(parsed) = raw.parse::<u32>()
                {
                    *id = Some(parsed);
                }
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if let Some((kind, depth, id)) = current.take() {
                    if path.len() == depth && name == kind {
                        if let Some(id) = id {
                            *counts.entry(id).or_default() += 1;
                        }
                    } else {
                        current = Some((kind, depth, id));
                    }
                }
                path.pop();
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    counts
}

fn search_pptx(file: &str, matcher: &Matcher, options: &FindOptions) -> CliResult<Vec<FindHit>> {
    if !wants_text(&options.search_type) {
        return Ok(Vec::new());
    }
    let mut hits = Vec::new();
    let slide_infos = pptx_find_slide_infos(file)?;
    let text = pptx_extract_text(file, &[])?;
    if let Some(slides) = text["slides"].as_array() {
        for slide in slides {
            let slide_number = slide["slide"].as_i64().unwrap_or_default();
            let slide_info = slide_infos
                .iter()
                .find(|info| i64::from(info.number) == slide_number);
            if let Some(shapes) = slide["shapes"].as_array() {
                for shape in shapes {
                    let full = shape["text"]["plainText"].as_str().unwrap_or_default();
                    if let Some(matched) = matcher.find(full) {
                        let key = shape["key"].as_str().unwrap_or_default();
                        let shape_id = shape["id"].as_i64().unwrap_or_default();
                        let shape_handle = slide_info
                            .and_then(|info| info.shape_handle(u32::try_from(shape_id).ok()?));
                        let slide_handle = slide_info.and_then(PptxFindSlideInfo::slide_handle);
                        let target_handle = shape_handle.clone().or(slide_handle.clone());
                        let location = if shape_id > 0 {
                            format!("slide:{slide_number} shape:{shape_id}")
                        } else {
                            format!("slide:{slide_number}")
                        };
                        let selectors = if key.is_empty() {
                            vec![format!("slide:{slide_number}")]
                        } else {
                            vec![format!("slide:{slide_number}"), key.to_string()]
                        };
                        let op = if let Some(handle) = shape_handle.clone() {
                            OpSpec::new(
                                "pptx replace text-occurrences",
                                vec![
                                    ("match-text", matched.clone()),
                                    ("new-text", NEW_OP_PLACEHOLDER.to_string()),
                                    ("for-shape", handle.clone()),
                                ],
                                "new-text",
                            )
                            .with_handle("for-shape", handle)
                        } else {
                            let selector = slide_handle
                                .clone()
                                .unwrap_or_else(|| slide_number.to_string());
                            OpSpec::new(
                                "pptx replace text-occurrences",
                                vec![
                                    ("match-text", matched.clone()),
                                    ("new-text", NEW_OP_PLACEHOLDER.to_string()),
                                    ("for-slides", selector),
                                ],
                                "new-text",
                            )
                            .with_optional_handle("for-slides", slide_handle.unwrap_or_default())
                        };
                        let mutation_command = op.human_command();
                        let mutation_note = if shape_handle.is_none() {
                            Some("shape scope unavailable (shape cNvPr id missing or not unique on slide); this op is SLIDE-WIDE and may rewrite the same text in sibling shapes".to_string())
                        } else {
                            None
                        };
                        let metadata = vec![
                            ("slide", slide_number.to_string()),
                            ("shapeId", shape_id.to_string()),
                        ];
                        hits.push(hit_json(HitInput {
                            package_type: "pptx",
                            kind: "pptx-text",
                            location,
                            part_uri: slide_info.map(|info| format!("/{}", info.part)),
                            primary_selector: format!("slide:{slide_number}"),
                            handle: target_handle,
                            selectors,
                            matched,
                            context: context_snippet(full),
                            mutation_command,
                            mutation_note,
                            metadata,
                            op: Some(op),
                        }));
                    }
                }
            }
        }
    }
    let notes = pptx_extract_notes(file, &[])?;
    if let Some(items) = notes["notes"].as_array() {
        for note in items {
            let slide_number = note["slide"].as_i64().unwrap_or_default();
            let slide_info = slide_infos
                .iter()
                .find(|info| i64::from(info.number) == slide_number);
            let full = note["notes"]["plainText"].as_str().unwrap_or_default();
            if let Some(matched) = matcher.find(full) {
                hits.push(hit_json(HitInput {
                    package_type: "pptx",
                    kind: "pptx-notes",
                    location: format!("slide:{slide_number} notes"),
                    part_uri: note["partUri"].as_str().map(ToOwned::to_owned),
                    primary_selector: format!("slide:{slide_number}"),
                    handle: slide_info.and_then(PptxFindSlideInfo::slide_handle),
                    selectors: vec![format!("slide:{slide_number}")],
                    matched,
                    context: context_snippet(full),
                    mutation_command: String::new(),
                    mutation_note: Some(
                        "speaker-notes text has no semantic mutation command; edit notes with pptx notes set"
                            .to_string(),
                    ),
                    metadata: vec![("slide", slide_number.to_string())],
                    op: None,
                }));
            }
        }
    }
    Ok(hits)
}

fn search_xlsx(file: &str, matcher: &Matcher, options: &FindOptions) -> CliResult<Vec<FindHit>> {
    let mut hits = Vec::new();
    if options.search_type != "name" {
        let entries = zip_entry_names(file)?;
        let workbook_part = find_xlsx_workbook_part(file, &entries)?;
        let workbook_xml = zip_text(file, &workbook_part)?;
        let sheets = workbook_sheets(&workbook_xml)?;
        let rels = relationship_entries(file, &relationships_part_for(&workbook_part))?;
        let shared = shared_strings(file)?;
        let styles = xlsx_styles(file)?;
        for sheet in sheets {
            let Some(rel) = rels.iter().find(|rel| rel.id == sheet.rel_id) else {
                continue;
            };
            let part_uri = resolve_relationship_target(&format!("/{workbook_part}"), &rel.target);
            let part = part_uri.trim_start_matches('/');
            let Ok(sheet_xml) = zip_text(file, part) else {
                continue;
            };
            let cells = sheet_cells(&sheet_xml, &shared, &styles);
            for (cell_ref, cell) in cells {
                let cell_handle = format!("H:xlsx/ws:{}/cell:a:{cell_ref}", sheet.sheet_id);
                if wants_text(&options.search_type)
                    && let Some(matched) = matcher.find(&cell.display_value)
                {
                    let op = OpSpec::new(
                        "xlsx cells set",
                        vec![
                            ("sheet", sheet.name.clone()),
                            ("cell", cell_ref.clone()),
                            ("value", NEW_OP_PLACEHOLDER.to_string()),
                        ],
                        "value",
                    )
                    .with_handle("cell", cell_handle.clone());
                    hits.push(hit_json(HitInput {
                        package_type: "xlsx",
                        kind: "xlsx-value",
                        location: format!("sheet:{} ref:{}", sheet.name, cell_ref),
                        part_uri: Some(part_uri.clone()),
                        primary_selector: format!("{}!{}", sheet.name, cell_ref),
                        handle: Some(cell_handle.clone()),
                        selectors: vec![format!("{}!{}", sheet.name, cell_ref), cell_ref.clone()],
                        matched,
                        context: context_snippet(&cell.display_value),
                        mutation_command: op.human_command(),
                        mutation_note: None,
                        metadata: vec![("sheet", sheet.name.clone()), ("ref", cell_ref.clone())],
                        op: Some(op),
                    }));
                }
                if (options.search_type == "all" || options.search_type == "formula")
                    && !cell.formula.is_empty()
                    && let Some(matched) = matcher.find(&cell.formula)
                {
                    let op = OpSpec::new(
                        "xlsx cells set",
                        vec![
                            ("sheet", sheet.name.clone()),
                            ("cell", cell_ref.clone()),
                            ("formula", NEW_OP_PLACEHOLDER.to_string()),
                        ],
                        "formula",
                    )
                    .with_handle("cell", cell_handle.clone());
                    hits.push(hit_json(HitInput {
                        package_type: "xlsx",
                        kind: "xlsx-formula",
                        location: format!("sheet:{} ref:{}", sheet.name, cell_ref),
                        part_uri: Some(part_uri.clone()),
                        primary_selector: format!("{}!{}", sheet.name, cell_ref),
                        handle: Some(cell_handle.clone()),
                        selectors: vec![format!("{}!{}", sheet.name, cell_ref), cell_ref.clone()],
                        matched,
                        context: context_snippet(&cell.formula),
                        mutation_command: op.human_command(),
                        mutation_note: None,
                        metadata: vec![
                            ("sheet", sheet.name.clone()),
                            ("ref", cell_ref.clone()),
                            ("formula", cell.formula.clone()),
                        ],
                        op: Some(op),
                    }));
                }
            }
        }
    }
    if options.search_type == "all" || options.search_type == "name" {
        let names = xlsx_names_list(file, None)?;
        if let Some(items) = names["names"].as_array() {
            for name in items {
                let name_text = name["name"].as_str().unwrap_or_default();
                let ref_text = name["ref"].as_str().unwrap_or_default();
                let (matched, field) = if let Some(matched) = matcher.find(name_text) {
                    (matched, "name")
                } else if let Some(matched) = matcher.find(ref_text) {
                    (matched, "ref")
                } else {
                    continue;
                };
                let scope = name["scope"].as_str().unwrap_or("workbook");
                let handle = if scope == "workbook" && !name_text.is_empty() {
                    Some(format!("H:xlsx/wb/name:n:{name_text}"))
                } else {
                    None
                };
                let op = OpSpec::new(
                    "xlsx names update",
                    vec![
                        ("name", name_text.to_string()),
                        ("ref", NEW_OP_PLACEHOLDER.to_string()),
                    ],
                    "ref",
                )
                .with_optional_handle("name", handle.clone().unwrap_or_default());
                let selectors = name["selectors"]
                    .as_array()
                    .map(|items| {
                        items
                            .iter()
                            .filter_map(Value::as_str)
                            .map(ToOwned::to_owned)
                            .collect::<Vec<_>>()
                    })
                    .filter(|items| !items.is_empty())
                    .unwrap_or_else(|| vec![name_text.to_string()]);
                hits.push(hit_json(HitInput {
                    package_type: "xlsx",
                    kind: "xlsx-name",
                    location: if scope.is_empty() {
                        format!("name:{name_text}")
                    } else {
                        format!("name:{name_text} scope:{scope}")
                    },
                    part_uri: None,
                    primary_selector: name["primarySelector"]
                        .as_str()
                        .unwrap_or(name_text)
                        .to_string(),
                    handle,
                    selectors,
                    matched,
                    context: context_snippet(&format!("{name_text} = {ref_text}")),
                    mutation_command: op.human_command(),
                    mutation_note: None,
                    metadata: vec![
                        ("name", name_text.to_string()),
                        ("ref", ref_text.to_string()),
                        ("matchedField", field.to_string()),
                    ],
                    op: Some(op),
                }));
            }
        }
    }
    Ok(hits)
}

fn search_docx(file: &str, matcher: &Matcher, options: &FindOptions) -> CliResult<Vec<FindHit>> {
    if !wants_text(&options.search_type) {
        return Ok(Vec::new());
    }
    let entries = zip_entry_names(file)?;
    let document_part = find_docx_document_part(file, &entries)?;
    let document_xml = zip_text(file, &document_part)?;
    let blocks = docx_rich_block_reports(&document_xml, false)?;
    let mut hits = Vec::new();
    for report in blocks {
        let block = docx_rich_block_json(report);
        let text = block["text"].as_str().unwrap_or_default();
        for line in split_docx_lines(block["kind"].as_str().unwrap_or_default(), text) {
            if let Some(matched) = matcher.find(&line) {
                let handle = block["handle"].as_str().map(ToOwned::to_owned);
                let op = handle.as_ref().map(|handle| {
                    let token = collision_free_replace_token(&line);
                    let template = line.replacen(&matched, &token, 1);
                    OpSpec::new(
                        "docx paragraphs set",
                        vec![("handle", handle.clone()), ("text", template)],
                        "text",
                    )
                    .with_replace_token(token)
                    .with_handle("handle", handle.clone())
                });
                let mutation_command = op
                    .as_ref()
                    .map(OpSpec::human_command)
                    .unwrap_or_else(String::new);
                hits.push(hit_json(HitInput {
                    package_type: "docx",
                    kind: "docx-text",
                    location: format!("block:{}", block["index"].as_i64().unwrap_or_default()),
                    part_uri: Some(format!("/{document_part}")),
                    primary_selector: block["id"]
                        .as_str()
                        .unwrap_or_else(|| block["primarySelector"].as_str().unwrap_or_default())
                        .to_string(),
                    handle,
                    selectors: vec![
                        block["id"]
                            .as_str()
                            .unwrap_or_else(|| block["primarySelector"].as_str().unwrap_or_default())
                            .to_string(),
                        format!("block:{}", block["index"].as_i64().unwrap_or_default()),
                    ],
                    matched,
                    context: context_snippet(&line),
                    mutation_command,
                    mutation_note: if block["handle"].as_str().is_some() {
                        None
                    } else {
                        Some("no semantic Rust mutation command is available for this DOCX hit because the paragraph has no stable handle".to_string())
                    },
                    metadata: vec![
                        ("block", block["index"].as_i64().unwrap_or_default().to_string()),
                        ("kind", block["kind"].as_str().unwrap_or_default().to_string()),
                    ],
                    op,
                }));
            }
        }
    }
    Ok(hits)
}

struct HitInput {
    package_type: &'static str,
    kind: &'static str,
    location: String,
    part_uri: Option<String>,
    primary_selector: String,
    handle: Option<String>,
    selectors: Vec<String>,
    matched: String,
    context: String,
    mutation_command: String,
    mutation_note: Option<String>,
    metadata: Vec<(&'static str, String)>,
    op: Option<OpSpec>,
}

fn hit_json(input: HitInput) -> FindHit {
    let mut map = Map::new();
    map.insert("index".to_string(), json!(0));
    map.insert("packageType".to_string(), json!(input.package_type));
    map.insert("kind".to_string(), json!(input.kind));
    map.insert("location".to_string(), json!(input.location));
    if let Some(part_uri) = input.part_uri.filter(|value| !value.is_empty()) {
        map.insert("partUri".to_string(), json!(part_uri));
    }
    map.insert("primarySelector".to_string(), json!(input.primary_selector));
    if let Some(handle) = input.handle.filter(|value| !value.is_empty()) {
        map.insert("handle".to_string(), json!(handle));
    }
    map.insert("selectors".to_string(), json!(input.selectors));
    map.insert("matchedValue".to_string(), json!(input.matched));
    map.insert("context".to_string(), json!(input.context));
    map.insert("mutationCommand".to_string(), json!(input.mutation_command));
    if let Some(note) = input.mutation_note.filter(|value| !value.is_empty()) {
        map.insert("mutationNote".to_string(), json!(note));
    }
    if !input.metadata.is_empty() {
        let metadata = input
            .metadata
            .into_iter()
            .map(|(key, value)| (key.to_string(), Value::String(value)))
            .collect();
        map.insert("metadata".to_string(), Value::Object(metadata));
    }
    FindHit {
        json: Value::Object(map),
        op: input.op,
    }
}

fn wants_text(search_type: &str) -> bool {
    search_type == "all" || search_type == "text"
}

fn wants_json(flags: &GlobalFlags, args: &[String]) -> bool {
    flags.json
        || has_flag(args, "--json")
        || args
            .windows(2)
            .any(|pair| (pair[0] == "--format" || pair[0] == "-f") && pair[1] == "json")
        || args
            .iter()
            .any(|arg| arg == "--format=json" || arg == "-f=json")
}

fn context_snippet(value: &str) -> String {
    const MAX_LEN: usize = 160;
    let text = value.replace(['\n', '\r'], " ").trim().to_string();
    if text.len() <= MAX_LEN {
        text
    } else {
        format!("{}\u{2026}", &text[..MAX_LEN])
    }
}

fn collision_free_replace_token(text: &str) -> String {
    if !text.contains(NEW_OP_PLACEHOLDER) {
        return NEW_OP_PLACEHOLDER.to_string();
    }
    for index in 1.. {
        let token = format!("<OOXML_NEW_{index}>");
        if !text.contains(&token) {
            return token;
        }
    }
    unreachable!("unbounded token search should always find a free token")
}

fn split_docx_lines(kind: &str, text: &str) -> Vec<String> {
    if kind == "table" {
        text.split('\n')
            .flat_map(|row| row.split('\t'))
            .map(ToOwned::to_owned)
            .collect()
    } else {
        vec![text.to_string()]
    }
}

fn render_find_text(value: &Value) -> String {
    let mut out = format!(
        "ooxml find: packageType={} query={} type={} totalHits={} truncated={}\n",
        value["packageType"].as_str().unwrap_or_default(),
        value["query"].as_str().unwrap_or_default(),
        value["type"].as_str().unwrap_or_default(),
        value["totalHits"].as_i64().unwrap_or_default(),
        value["truncated"].as_bool().unwrap_or(false)
    );
    if let Some(hits) = value["hits"].as_array() {
        for hit in hits {
            out.push_str(&format!(
                "- #{} {} {}: {}\n",
                hit["index"].as_i64().unwrap_or_default(),
                hit["kind"].as_str().unwrap_or_default(),
                hit["location"].as_str().unwrap_or_default(),
                hit["context"].as_str().unwrap_or_default()
            ));
        }
    }
    out
}

const FIND_ROBOT_DOCS: &str = r#"OOXML find robot guide

Purpose:
Use find to locate text-like content across PPTX, XLSX, and DOCX packages before editing. The command is read-only unless --replace --apply is used with an output mode.

Commands:
- ooxml --json find <query> <file>
- ooxml --json find <query> <file> --type text
- ooxml --json find <query> <file> --type formula
- ooxml --json find <query> <file> --type name
- ooxml --json find <query> <file> --ignore-case
- ooxml --json find <query> <file> --regex
- ooxml --json find <query> <file> --to-ops [--replace <new>]
- ooxml --json find <query> <file> --replace <new> --apply (--out <file>|--in-place|--dry-run)
- ooxml --json find capabilities
- ooxml find robot-docs

Result contract:
- contractVersion is ooxml-find.v1.
- hits are deterministic and ordered in package traversal order.
- zero hits exit 0 with an empty hits array.
- mutationCommand is advisory; review selector scope before running it.
- --to-ops prints a bare JSON array of {command,args} accepted by ooxml apply.
- --replace fills the generated replacement argument; without it, --to-ops leaves <NEW>.
- --apply requires --replace and runs the generated ops through ooxml apply.
- Hits with no semantic mutation command are skipped from generated ops and reported on stderr.
"#;
