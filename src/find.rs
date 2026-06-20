use regex::Regex;
use serde_json::{Map, Value, json};
use std::fs;

use crate::cli_dispatch::{DispatchBody, DispatchOutput};
use crate::{
    CliError, CliResult, EXIT_SUCCESS, GlobalFlags, InspectPackageKind, command_arg,
    detect_inspect_package_type, docx_rich_block_json, docx_rich_block_reports,
    find_docx_document_part, find_xlsx_workbook_part, has_flag, package_type, parse_i64_flag,
    parse_string_flag, pptx_extract_notes, pptx_extract_text, reject_unknown_flags,
    relationship_entries, relationships_part_for, resolve_relationship_target, shared_strings,
    sheet_cells, workbook_sheets, xlsx_names_list, xlsx_styles, zip_entry_names, zip_text,
};

const FIND_CONTRACT_VERSION: &str = "ooxml-find.v1";

struct FindOptions {
    query: String,
    search_type: String,
    ignore_case: bool,
    regex: bool,
    max: i64,
}

struct Matcher {
    query: String,
    ignore_case: bool,
    regex: Option<Regex>,
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
    if has_flag(args, "--to-ops") || has_flag(args, "--apply") || has_value_flag(args, "--replace")
    {
        return Err(CliError::invalid_args(
            "find mutation composition flags (--to-ops, --apply, --replace) are not implemented in the Rust port",
        ));
    }
    reject_unknown_flags(
        args,
        &["--type", "--max", "--format"],
        &["--ignore-case", "--regex", "--json"],
    )?;
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
        hit["index"] = json!(index);
    }
    let value = json!({
        "contractVersion": FIND_CONTRACT_VERSION,
        "packageType": package,
        "query": options.query,
        "type": options.search_type,
        "ignoreCase": options.ignore_case,
        "regex": options.regex,
        "max": options.max,
        "truncated": truncated,
        "totalHits": hits.len(),
        "hits": hits,
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
                {"name": "--json", "type": "bool", "description": "emit machine-readable JSON"}
            ],
            "exitCodes": [
                {"code": 0, "description": "search completed, including zero-hit searches"},
                {"code": 2, "description": "invalid arguments or invalid regex"},
                {"code": 3, "description": "file not found"},
                {"code": 4, "description": "unsupported package type"}
            ],
            "notes": [
                "Search is read-only in the Rust port.",
                "Go find --to-ops/--apply mutation composition is intentionally unadvertised until the Rust apply derivation loop is ported.",
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

fn search_pptx(file: &str, matcher: &Matcher, options: &FindOptions) -> CliResult<Vec<Value>> {
    if !wants_text(&options.search_type) {
        return Ok(Vec::new());
    }
    let mut hits = Vec::new();
    let text = pptx_extract_text(file, &[])?;
    if let Some(slides) = text["slides"].as_array() {
        for slide in slides {
            let slide_number = slide["slide"].as_i64().unwrap_or_default();
            if let Some(shapes) = slide["shapes"].as_array() {
                for shape in shapes {
                    let full = shape["text"]["plainText"].as_str().unwrap_or_default();
                    if let Some(matched) = matcher.find(full) {
                        let key = shape["key"].as_str().unwrap_or_default();
                        let location = format!("slide:{slide_number}");
                        let selectors = if key.is_empty() {
                            vec![format!("slide:{slide_number}")]
                        } else {
                            vec![format!("slide:{slide_number}"), key.to_string()]
                        };
                        let mutation_command = human_command(
                            "pptx replace text-occurrences",
                            &[("match-text", &matched), ("new-text", "<NEW>")],
                            "new-text",
                        );
                        let metadata = vec![
                            ("slide", slide_number.to_string()),
                            (
                                "shapeId",
                                shape["id"].as_i64().unwrap_or_default().to_string(),
                            ),
                        ];
                        hits.push(hit_json(HitInput {
                            package_type: "pptx",
                            kind: "pptx-text",
                            location,
                            part_uri: None,
                            primary_selector: format!("slide:{slide_number}"),
                            handle: None,
                            selectors,
                            matched,
                            context: context_snippet(full),
                            mutation_command,
                            mutation_note: Some("shape-scoped handles are not available from the current Rust readback helper; review selector scope before mutating".to_string()),
                            metadata,
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
            let full = note["notes"]["plainText"].as_str().unwrap_or_default();
            if let Some(matched) = matcher.find(full) {
                hits.push(hit_json(HitInput {
                    package_type: "pptx",
                    kind: "pptx-notes",
                    location: format!("slide:{slide_number} notes"),
                    part_uri: note["partUri"].as_str().map(ToOwned::to_owned),
                    primary_selector: format!("slide:{slide_number}"),
                    handle: None,
                    selectors: vec![format!("slide:{slide_number}")],
                    matched,
                    context: context_snippet(full),
                    mutation_command: String::new(),
                    mutation_note: Some(
                        "speaker-notes text has no semantic mutation command; edit notes with pptx notes set"
                            .to_string(),
                    ),
                    metadata: vec![("slide", slide_number.to_string())],
                }));
            }
        }
    }
    Ok(hits)
}

fn search_xlsx(file: &str, matcher: &Matcher, options: &FindOptions) -> CliResult<Vec<Value>> {
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
                        mutation_command: human_command(
                            "xlsx cells set",
                            &[
                                ("sheet", &sheet.name),
                                ("cell", &cell_ref),
                                ("value", "<NEW>"),
                            ],
                            "value",
                        ),
                        mutation_note: None,
                        metadata: vec![("sheet", sheet.name.clone()), ("ref", cell_ref.clone())],
                    }));
                }
                if (options.search_type == "all" || options.search_type == "formula")
                    && !cell.formula.is_empty()
                    && let Some(matched) = matcher.find(&cell.formula)
                {
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
                        mutation_command: human_command(
                            "xlsx cells set",
                            &[
                                ("sheet", &sheet.name),
                                ("cell", &cell_ref),
                                ("formula", "<NEW>"),
                            ],
                            "formula",
                        ),
                        mutation_note: None,
                        metadata: vec![
                            ("sheet", sheet.name.clone()),
                            ("ref", cell_ref.clone()),
                            ("formula", cell.formula.clone()),
                        ],
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
                    mutation_command: human_command(
                        "xlsx names update",
                        &[("name", name_text), ("ref", "<NEW>")],
                        "ref",
                    ),
                    mutation_note: None,
                    metadata: vec![
                        ("name", name_text.to_string()),
                        ("ref", ref_text.to_string()),
                        ("matchedField", field.to_string()),
                    ],
                }));
            }
        }
    }
    Ok(hits)
}

fn search_docx(file: &str, matcher: &Matcher, options: &FindOptions) -> CliResult<Vec<Value>> {
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
                let mutation_command = if handle.is_some() {
                    human_command(
                        "docx paragraphs set",
                        &[
                            ("handle", handle.as_deref().unwrap_or_default()),
                            ("text", "<NEW>"),
                        ],
                        "text",
                    )
                } else {
                    String::new()
                };
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
}

fn hit_json(input: HitInput) -> Value {
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
    Value::Object(map)
}

fn wants_text(search_type: &str) -> bool {
    search_type == "all" || search_type == "text"
}

fn has_value_flag(args: &[String], name: &str) -> bool {
    args.iter()
        .any(|arg| arg == name || arg.starts_with(&format!("{name}=")))
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

fn human_command(command: &str, args: &[(&str, &str)], replace_key: &str) -> String {
    let mut text = format!("ooxml --json {command} <file>");
    for (key, value) in args {
        text.push_str(" --");
        text.push_str(key);
        text.push(' ');
        if *key == replace_key && *value == "<NEW>" {
            text.push_str("<NEW>");
        } else {
            text.push_str(&command_arg(value));
        }
    }
    text.push_str(" --out <OUT>");
    text
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
Use find to locate text-like content across PPTX, XLSX, and DOCX packages before editing. The Rust command is read-only.

Commands:
- ooxml --json find <query> <file>
- ooxml --json find <query> <file> --type text
- ooxml --json find <query> <file> --type formula
- ooxml --json find <query> <file> --type name
- ooxml --json find <query> <file> --ignore-case
- ooxml --json find <query> <file> --regex
- ooxml --json find capabilities
- ooxml find robot-docs

Result contract:
- contractVersion is ooxml-find.v1.
- hits are deterministic and ordered in package traversal order.
- zero hits exit 0 with an empty hits array.
- mutationCommand is advisory; review selector scope before running it.

Unported behavior:
Go find --to-ops, --replace, and --apply are intentionally not advertised or accepted in Rust yet.
"#;
