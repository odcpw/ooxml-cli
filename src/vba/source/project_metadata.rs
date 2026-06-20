use crate::{CliError, CliResult};

use super::super::cfb::CfbFile;
use super::cfb_paths::find_cfb_stream_path;
use super::codec::{decode_mbcs, decode_utf16_le, utf16le_bytes};
use super::{
    ProjectMetadata, ProjectModuleDeclaration, ProjectReference, ProjectWorkspaceEntry,
    SourceModule,
};
pub(super) fn remove_project_stream_module_lines(
    data: &[u8],
    module: &SourceModule,
) -> (Vec<u8>, usize) {
    if data.is_empty() {
        return (Vec::new(), 0);
    }
    let text = String::from_utf8_lossy(data);
    let line_ending = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let trailing = text.ends_with('\n');
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut lines = normalized.split('\n').collect::<Vec<_>>();
    if lines.last() == Some(&"") {
        lines.pop();
    }
    let mut kept = Vec::new();
    let mut removed = 0;
    for line in lines {
        if is_project_stream_module_line(line, module) {
            removed += 1;
        } else {
            kept.push(line.to_string());
        }
    }
    if removed == 0 {
        return (data.to_vec(), 0);
    }
    let mut out = kept.join(line_ending);
    if trailing || !kept.is_empty() {
        out.push_str(line_ending);
    }
    (out.into_bytes(), removed)
}

pub(super) fn add_project_stream_module_lines(
    data: &[u8],
    module: &SourceModule,
) -> CliResult<(Vec<u8>, usize)> {
    let text = String::from_utf8_lossy(data);
    let line_ending = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let trailing = text.ends_with('\n');
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut lines = normalized
        .split('\n')
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    if lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    for line in &lines {
        if is_project_stream_module_line(line, module) {
            return Err(CliError::unexpected(format!(
                "PROJECT stream already contains module entry for {}",
                module.name
            )));
        }
    }
    let module_line = if module.kind == "class" {
        format!("Class={}", module.name)
    } else {
        format!("Module={}", module.name)
    };
    let workspace_line = format!("{}=0, 0, 0, 0, C", module.name);
    let mut insert_at = lines.len();
    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        let lower = trimmed.to_ascii_lowercase();
        if lower == "[workspace]" || trimmed.starts_with('[') || lower.starts_with("name=") {
            insert_at = idx;
            break;
        }
        if project_stream_module_declaration_key(line) {
            insert_at = idx + 1;
        }
    }
    let mut out_lines = Vec::with_capacity(lines.len() + 3);
    out_lines.extend_from_slice(&lines[..insert_at]);
    out_lines.push(module_line);
    out_lines.extend_from_slice(&lines[insert_at..]);

    let mut workspace_at = None;
    let mut workspace_end = out_lines.len();
    for (idx, line) in out_lines.iter().enumerate() {
        if !line.trim().eq_ignore_ascii_case("[Workspace]") {
            continue;
        }
        workspace_at = Some(idx);
        workspace_end = idx + 1;
        for (scan, candidate) in out_lines.iter().enumerate().skip(idx + 1) {
            if candidate.trim().starts_with('[') {
                break;
            }
            workspace_end = scan + 1;
        }
        break;
    }
    if workspace_at.is_some() {
        out_lines.insert(workspace_end, workspace_line);
    } else {
        out_lines.push("[Workspace]".to_string());
        out_lines.push(workspace_line);
    }
    let added = out_lines.len() - lines.len();
    let mut out = out_lines.join(line_ending);
    if trailing || !out_lines.is_empty() {
        out.push_str(line_ending);
    }
    Ok((out.into_bytes(), added))
}

fn project_stream_module_declaration_key(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('[') {
        return false;
    }
    let Some((key, _)) = trimmed.split_once('=') else {
        return false;
    };
    matches!(
        key.trim().to_ascii_lowercase().as_str(),
        "document" | "module" | "class" | "baseclass"
    )
}

fn is_project_stream_module_line(line: &str, module: &SourceModule) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('[') {
        return false;
    }
    let Some((key, mut value)) = trimmed.split_once('=') else {
        return false;
    };
    let key = key.trim().to_ascii_lowercase();
    value = value.trim();
    let names = [module.name.as_str(), module.stream_name.as_str()];
    match key.as_str() {
        "module" | "class" | "baseclass" => matches_any_module_name(value, &names),
        "document" => {
            if let Some((before, _)) = value.split_once('/') {
                value = before;
            }
            matches_any_module_name(value, &names)
        }
        _ => matches_any_module_name(key.trim(), &names),
    }
}

#[derive(Clone)]
struct ProjectWmModuleEntry {
    name: String,
    display_name: String,
}

pub(super) fn add_project_wm_module_entry(
    data: &[u8],
    module: &SourceModule,
) -> CliResult<(Vec<u8>, usize)> {
    let mut entries = parse_project_wm_module_entries(data)?;
    for entry in &entries {
        if project_wm_entry_matches_module(entry, module) {
            return Err(CliError::unexpected(format!(
                "PROJECTwm stream already contains module entry for {}",
                module.name
            )));
        }
    }
    entries.push(ProjectWmModuleEntry {
        name: module.name.clone(),
        display_name: module.name.clone(),
    });
    Ok((build_project_wm_module_entries(&entries), 1))
}

pub(super) fn remove_project_wm_module_entry(
    data: &[u8],
    module: &SourceModule,
) -> CliResult<(Vec<u8>, usize)> {
    let entries = parse_project_wm_module_entries(data)?;
    let mut kept = Vec::new();
    let mut removed = 0;
    for entry in entries {
        if project_wm_entry_matches_module(&entry, module) {
            removed += 1;
        } else {
            kept.push(entry);
        }
    }
    if removed == 0 {
        return Ok((data.to_vec(), 0));
    }
    Ok((build_project_wm_module_entries(&kept), removed))
}

fn parse_project_wm_module_entries(data: &[u8]) -> CliResult<Vec<ProjectWmModuleEntry>> {
    let mut entries = Vec::new();
    let mut pos = 0;
    while pos < data.len() {
        if data[pos] == 0 {
            if pos + 1 < data.len() && data[pos + 1] == 0 {
                return Ok(entries);
            }
            return Err(CliError::unexpected(format!(
                "PROJECTwm stream has an empty module name at byte {pos}"
            )));
        }
        let name_start = pos;
        while pos < data.len() && data[pos] != 0 {
            pos += 1;
        }
        if pos >= data.len() {
            return Err(CliError::unexpected(
                "PROJECTwm stream module name is unterminated",
            ));
        }
        let name = String::from_utf8_lossy(&data[name_start..pos]).to_string();
        pos += 1;
        let display_start = pos;
        loop {
            if pos + 1 >= data.len() {
                return Err(CliError::unexpected(format!(
                    "PROJECTwm stream display name for {name} is unterminated"
                )));
            }
            if data[pos] == 0 && data[pos + 1] == 0 {
                let display_name = decode_utf16_le(&data[display_start..pos]);
                pos += 2;
                entries.push(ProjectWmModuleEntry { name, display_name });
                break;
            }
            pos += 2;
        }
    }
    Ok(entries)
}

fn build_project_wm_module_entries(entries: &[ProjectWmModuleEntry]) -> Vec<u8> {
    let mut out = Vec::new();
    for entry in entries {
        let display_name = if entry.display_name.is_empty() {
            &entry.name
        } else {
            &entry.display_name
        };
        out.extend_from_slice(entry.name.as_bytes());
        out.push(0);
        out.extend(utf16le_bytes(display_name));
        out.extend([0, 0]);
    }
    out.extend([0, 0]);
    out
}

fn project_wm_entry_matches_module(entry: &ProjectWmModuleEntry, module: &SourceModule) -> bool {
    let names = [module.name.as_str(), module.stream_name.as_str()];
    matches_any_module_name(&entry.name, &names)
        || matches_any_module_name(&entry.display_name, &names)
}

fn matches_any_module_name(value: &str, names: &[&str]) -> bool {
    let value = value.trim_matches('"');
    names
        .iter()
        .any(|name| !name.trim().is_empty() && value.eq_ignore_ascii_case(name.trim()))
}

pub(super) fn parse_project_metadata(
    cfb_file: &CfbFile<'_>,
    code_page: i32,
) -> Option<ProjectMetadata> {
    let streams = cfb_file.streams();
    let project_wm_path = find_cfb_stream_path(&streams, "PROJECTwm");
    let project_path = find_cfb_stream_path(&streams, "PROJECT");
    let has_project_wm = !project_wm_path.is_empty();
    if project_path.is_empty() {
        if has_project_wm {
            return Some(ProjectMetadata {
                stream_name: String::new(),
                present: false,
                line_count: 0,
                id: String::new(),
                name: String::new(),
                modules: Vec::new(),
                references: Vec::new(),
                workspace_entries: Vec::new(),
                has_project_wm,
                project_wm_stream: project_wm_path,
                warnings: vec![
                    "PROJECTwm stream exists but PROJECT stream was not found".to_string(),
                ],
            });
        }
        return None;
    }
    let mut metadata = ProjectMetadata {
        stream_name: project_path.clone(),
        present: true,
        line_count: 0,
        id: String::new(),
        name: String::new(),
        modules: Vec::new(),
        references: Vec::new(),
        workspace_entries: Vec::new(),
        has_project_wm,
        project_wm_stream: project_wm_path,
        warnings: Vec::new(),
    };
    let data = match cfb_file.stream(&project_path) {
        Ok(data) => data,
        Err(err) => {
            metadata.warnings.push(err);
            return Some(metadata);
        }
    };
    let text = decode_mbcs(&data, code_page)
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    let mut lines = text.split('\n').collect::<Vec<_>>();
    if lines.last() == Some(&"") {
        lines.pop();
    }
    metadata.line_count = lines.len();
    let mut in_workspace = false;
    for (idx, line) in lines.iter().enumerate() {
        let line_no = idx + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_workspace = trimmed.eq_ignore_ascii_case("[Workspace]");
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        let lower_key = key.to_ascii_lowercase();
        if in_workspace {
            metadata.workspace_entries.push(ProjectWorkspaceEntry {
                name: key.to_string(),
                value: value.to_string(),
                line: line_no,
            });
            continue;
        }
        match lower_key.as_str() {
            "id" => metadata.id = value.trim_matches('"').to_string(),
            "name" => metadata.name = value.trim_matches('"').to_string(),
            "module" | "class" | "baseclass" | "document" => {
                let mut name = value.to_string();
                if lower_key == "document"
                    && let Some((before, _)) = value.split_once('/')
                {
                    name = before.to_string();
                }
                metadata.modules.push(ProjectModuleDeclaration {
                    kind: lower_key,
                    name: name.trim_matches('"').to_string(),
                    value: value.to_string(),
                    line: line_no,
                });
            }
            "reference" | "object" | "package" | "control" => {
                metadata.references.push(ProjectReference {
                    kind: lower_key,
                    value: value.to_string(),
                    line: line_no,
                });
            }
            _ => {}
        }
    }
    Some(metadata)
}
