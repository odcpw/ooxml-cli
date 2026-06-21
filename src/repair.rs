use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs;

use crate::{
    CliError, CliResult, command_arg, copy_zip_with_part_overrides, find_xlsx_workbook_part,
    local_name, package_mutation_temp_path, validate, validate_xlsx_mutation_output_flags,
    xlsx_workbook_child_order, zip_entry_names, zip_text,
};

pub(crate) fn repair_normalize(file: &str, args: &[String]) -> CliResult<Value> {
    crate::reject_unknown_flags(
        args,
        &["--out", "--backup"],
        &["--dry-run", "--in-place", "--no-validate"],
    )?;
    let out = crate::parse_string_flag(args, "--out")?;
    let backup = crate::parse_string_flag(args, "--backup")?;
    let dry_run = crate::has_flag(args, "--dry-run");
    let in_place = crate::has_flag(args, "--in-place");
    let no_validate = crate::has_flag(args, "--no-validate");
    validate_xlsx_mutation_output_flags(out.as_deref(), in_place, backup.as_deref(), dry_run)?;

    let entries = zip_entry_names(file)?;
    let workbook_part = find_xlsx_workbook_part(file, &entries).map_err(|_| {
        CliError::unsupported_type(
            "repair normalize currently supports XLSX/XLSM workbook packages only",
        )
    })?;
    let workbook_xml = zip_text(file, &workbook_part)?;
    let normalized = normalize_xlsx_workbook_child_order_xml(&workbook_xml, &workbook_part)?;
    let output_path = resolve_repair_output_path(file, out.as_deref(), in_place, dry_run)?;

    if !dry_run {
        if normalized.changed || !in_place {
            write_repair_output(
                file,
                &output_path,
                &workbook_part,
                &normalized.xml,
                in_place,
                backup.as_deref(),
            )?;
        }
        if !no_validate {
            validate(if in_place { file } else { &output_path }, true)?;
        }
    }

    Ok(json!({
        "file": file,
        "output": if dry_run { Value::Null } else { Value::String(if in_place { file.to_string() } else { output_path.clone() }) },
        "family": "xlsx",
        "operation": "repair normalize",
        "dryRun": dry_run,
        "changed": normalized.changed,
        "repairs": normalized.repairs,
        "workbookPart": workbook_part,
        "validateCommand": if dry_run {
            Value::Null
        } else {
            Value::String(format!(
                "ooxml validate --strict {}",
                command_arg(if in_place { file } else { &output_path })
            ))
        },
        "conformanceCommand": if dry_run {
            Value::Null
        } else {
            Value::String(format!(
                "ooxml --json conformance check {}",
                command_arg(if in_place { file } else { &output_path })
            ))
        },
    }))
}

fn resolve_repair_output_path(
    file: &str,
    out: Option<&str>,
    in_place: bool,
    dry_run: bool,
) -> CliResult<String> {
    if dry_run {
        return Ok(package_mutation_temp_path(file, "repair-normalize"));
    }
    if in_place {
        Ok(package_mutation_temp_path(file, "repair-normalize"))
    } else {
        out.filter(|value| !value.trim().is_empty())
            .map(str::to_string)
            .ok_or_else(|| {
                CliError::invalid_args(
                    "must specify exactly one of --out, --in-place, or --dry-run",
                )
            })
    }
}

fn write_repair_output(
    file: &str,
    output_path: &str,
    workbook_part: &str,
    workbook_xml: &str,
    in_place: bool,
    backup: Option<&str>,
) -> CliResult<()> {
    let mut overrides = BTreeMap::new();
    overrides.insert(workbook_part.to_string(), workbook_xml.to_string());
    copy_zip_with_part_overrides(file, output_path, &overrides)?;
    if !in_place {
        return Ok(());
    }
    if let Some(backup_path) = backup.filter(|value| !value.trim().is_empty()) {
        fs::copy(file, backup_path)
            .map_err(|err| CliError::unexpected(format!("failed to create backup: {err}")))?;
    }
    fs::rename(output_path, file)
        .or_else(|_| {
            fs::copy(output_path, file)?;
            fs::remove_file(output_path)
        })
        .map_err(|err| CliError::unexpected(format!("failed to write output file: {err}")))?;
    Ok(())
}

struct NormalizedWorkbook {
    xml: String,
    changed: bool,
    repairs: Vec<Value>,
}

fn normalize_xlsx_workbook_child_order_xml(
    xml: &str,
    workbook_part: &str,
) -> CliResult<NormalizedWorkbook> {
    let spans = workbook_direct_child_spans(xml)?;
    let mut sorted = spans.children.clone();
    sorted.sort_by_key(|child| (xlsx_workbook_child_order(&child.local_name), child.index));
    let changed = spans
        .children
        .iter()
        .zip(sorted.iter())
        .any(|(before, after)| before.index != after.index);
    if !changed {
        return Ok(NormalizedWorkbook {
            xml: xml.to_string(),
            changed: false,
            repairs: Vec::new(),
        });
    }

    let mut normalized = String::with_capacity(xml.len());
    normalized.push_str(&xml[..spans.inner_start]);
    for child in &sorted {
        normalized.push_str(&xml[child.start_with_leading..child.end]);
    }
    normalized.push_str(&xml[spans.trailing_start..]);

    Ok(NormalizedWorkbook {
        xml: normalized,
        changed: true,
        repairs: vec![json!({
            "code": "XLSX_WORKBOOK_CHILD_ORDER_NORMALIZED",
            "part": workbook_part,
            "before": spans.children.iter().map(|child| child.local_name.clone()).collect::<Vec<_>>(),
            "after": sorted.iter().map(|child| child.local_name.clone()).collect::<Vec<_>>(),
        })],
    })
}

struct WorkbookChildSpans {
    inner_start: usize,
    trailing_start: usize,
    children: Vec<WorkbookChildSpan>,
}

#[derive(Clone)]
struct WorkbookChildSpan {
    index: usize,
    local_name: String,
    start_with_leading: usize,
    end: usize,
}

fn workbook_direct_child_spans(xml: &str) -> CliResult<WorkbookChildSpans> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut depth = 0usize;
    let mut inner_start = None;
    let mut cursor = 0usize;
    let mut current_child: Option<(usize, String, usize)> = None;
    let mut children = Vec::new();

    loop {
        let start = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                if depth == 0 {
                    if local_name(e.name().as_ref()) != "workbook" {
                        return Err(CliError::unexpected("xlsx workbook root element not found"));
                    }
                    let after_root_start = reader.buffer_position() as usize;
                    inner_start = Some(after_root_start);
                    cursor = after_root_start;
                } else if depth == 1 {
                    current_child = Some((
                        cursor,
                        local_name(e.name().as_ref()).to_string(),
                        children.len(),
                    ));
                }
                depth += 1;
            }
            Ok(Event::Empty(e)) => {
                if depth == 1 {
                    let end = reader.buffer_position() as usize;
                    children.push(WorkbookChildSpan {
                        index: children.len(),
                        local_name: local_name(e.name().as_ref()).to_string(),
                        start_with_leading: cursor,
                        end,
                    });
                    cursor = end;
                }
            }
            Ok(Event::End(e)) => {
                if depth == 2
                    && let Some((start_with_leading, local_name, index)) = current_child.take()
                {
                    let end = reader.buffer_position() as usize;
                    children.push(WorkbookChildSpan {
                        index,
                        local_name,
                        start_with_leading,
                        end,
                    });
                    cursor = end;
                } else if depth == 1 {
                    if local_name(e.name().as_ref()) != "workbook" {
                        return Err(CliError::unexpected(
                            "xlsx workbook root closing tag not found",
                        ));
                    }
                    return Ok(WorkbookChildSpans {
                        inner_start: inner_start.ok_or_else(|| {
                            CliError::unexpected("xlsx workbook root start tag not found")
                        })?,
                        trailing_start: cursor.min(start),
                        children,
                    });
                }
                depth = depth.saturating_sub(1);
            }
            Ok(Event::Eof) => {
                return Err(CliError::unexpected(
                    "xlsx workbook XML ended before </workbook>",
                ));
            }
            Err(err) => {
                return Err(CliError::unexpected(format!(
                    "failed to parse xlsx workbook XML: {err}"
                )));
            }
            _ => {}
        }
    }
}
