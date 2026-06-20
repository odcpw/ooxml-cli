use serde_json::{Map, Value, json};
use std::path::Path;

use crate::command_arg;

use super::super::model::VbaInfo;
use super::super::output::{
    vba_info_json, vba_inspect_command, vba_list_command, vba_office_check_command,
    vba_package_readback_command, vba_validate_command,
};
use super::{
    HostCompatibilityWarning, OfficeCompatibilityReport, ProjectMetadata, ProjectModuleDeclaration,
    ProjectReference, ProjectWorkspaceEntry, SourceModule, SourceMutationResult, SourceProject,
};
pub(super) fn source_mutation_result_json(result: &SourceMutationResult) -> Value {
    let mut object = Map::new();
    object.insert("action".to_string(), json!(result.action));
    if !result.family.is_empty() {
        object.insert("family".to_string(), json!(result.family));
    }
    if !result.part_uri.is_empty() {
        object.insert("partUri".to_string(), json!(result.part_uri));
    }
    object.insert(
        "module".to_string(),
        source_module_json(&result.module, false),
    );
    if let Some(previous_count) = result.previous_count {
        object.insert("previousCount".to_string(), json!(previous_count));
    }
    if let Some(module_count) = result.module_count {
        object.insert("moduleCount".to_string(), json!(module_count));
    }
    if !result.previous_sha256.is_empty() {
        object.insert("previousSha256".to_string(), json!(result.previous_sha256));
    }
    if !result.sha256.is_empty() {
        object.insert("sha256".to_string(), json!(result.sha256));
    }
    if let Some(source_bytes) = result.source_bytes {
        object.insert("sourceBytes".to_string(), json!(source_bytes));
    }
    if let Some(line_count) = result.line_count {
        object.insert("lineCount".to_string(), json!(line_count));
    }
    if !result.warnings.is_empty() {
        object.insert("warnings".to_string(), json!(result.warnings));
    }
    object.insert("purgedCaches".to_string(), json!(result.purged_caches));
    object.insert(
        "recompilesOnOpen".to_string(),
        json!(result.recompiles_on_open),
    );
    object.insert(
        "officeLoadVerified".to_string(),
        json!(result.office_load_verified),
    );
    if !result.compatibility_status.is_empty() {
        object.insert(
            "compatibilityStatus".to_string(),
            json!(result.compatibility_status),
        );
    }
    Value::Object(object)
}

pub(super) fn vba_source_mutation_output_json(
    file: &str,
    output: Option<&str>,
    dry_run: bool,
    result: &SourceMutationResult,
    info: &VbaInfo,
    project: &SourceProject,
    target: &str,
) -> Value {
    let mut object = Map::new();
    object.insert("file".to_string(), json!(file));
    if let Some(output) = output.filter(|value| !value.trim().is_empty()) {
        object.insert("output".to_string(), json!(output));
    }
    object.insert("dryRun".to_string(), json!(dry_run));
    object.insert("result".to_string(), source_mutation_result_json(result));
    object.insert("vba".to_string(), vba_info_json(info));
    object.insert("project".to_string(), source_project_json(project, false));
    let module_selector = result.module.primary_selector.as_str();
    if dry_run {
        object.insert(
            "inspectCommandTemplate".to_string(),
            json!(vba_inspect_command(target)),
        );
        object.insert(
            "validateCommandTemplate".to_string(),
            json!(vba_validate_command(target)),
        );
        if let Some(command) = vba_office_check_command(target, info) {
            object.insert("officeCheckCommandTemplate".to_string(), json!(command));
        }
        object.insert(
            "packageReadbackCommandTemplate".to_string(),
            json!(vba_package_readback_command(target, info.family.family)),
        );
        object.insert(
            "listCommandTemplate".to_string(),
            json!(vba_list_command(target)),
        );
        if result.action != "remove-module" {
            object.insert(
                "extractCommandTemplate".to_string(),
                json!(vba_extract_module_command(target, module_selector)),
            );
        }
    } else {
        object.insert(
            "inspectCommand".to_string(),
            json!(vba_inspect_command(target)),
        );
        object.insert(
            "validateCommand".to_string(),
            json!(vba_validate_command(target)),
        );
        if let Some(command) = vba_office_check_command(target, info) {
            object.insert("officeCheckCommand".to_string(), json!(command));
        }
        object.insert(
            "packageReadbackCommand".to_string(),
            json!(vba_package_readback_command(target, info.family.family)),
        );
        object.insert("listCommand".to_string(), json!(vba_list_command(target)));
        if result.action != "remove-module" {
            object.insert(
                "extractCommand".to_string(),
                json!(vba_extract_module_command(target, module_selector)),
            );
        }
    }
    Value::Object(object)
}

fn vba_extract_module_command(file: &str, module_selector: &str) -> String {
    let mut command = format!(
        "ooxml --json vba extract {} --out-dir macros",
        command_arg(file)
    );
    if !module_selector.trim().is_empty() {
        command.push_str(&format!(" --module {}", command_arg(module_selector)));
    }
    command
}

pub(super) fn source_project_json(project: &SourceProject, include_source: bool) -> Value {
    let mut object = Map::new();
    if !project.family.is_empty() {
        object.insert("family".to_string(), json!(project.family));
    }
    if !project.part_uri.is_empty() {
        object.insert("partUri".to_string(), json!(project.part_uri));
    }
    if project.code_page != 0 {
        object.insert("codePage".to_string(), json!(project.code_page));
    }
    object.insert("moduleCount".to_string(), json!(project.module_count));
    object.insert(
        "modules".to_string(),
        Value::Array(
            project
                .modules
                .iter()
                .map(|module| source_module_json(module, include_source))
                .collect(),
        ),
    );
    if let Some(metadata) = &project.project_metadata {
        object.insert(
            "projectMetadata".to_string(),
            project_metadata_json(metadata),
        );
    }
    object.insert(
        "officeCompatibility".to_string(),
        office_compatibility_json(&project.office_compatibility),
    );
    if !project.host_compatibility_warnings.is_empty() {
        object.insert(
            "hostCompatibilityWarnings".to_string(),
            Value::Array(
                project
                    .host_compatibility_warnings
                    .iter()
                    .map(host_compatibility_warning_json)
                    .collect(),
            ),
        );
    }
    if !project.warnings.is_empty() {
        object.insert("warnings".to_string(), json!(project.warnings));
    }
    Value::Object(object)
}

pub(super) fn source_module_json(module: &SourceModule, include_source: bool) -> Value {
    let mut object = Map::new();
    object.insert("number".to_string(), json!(module.number));
    object.insert("name".to_string(), json!(module.name));
    object.insert("streamName".to_string(), json!(module.stream_name));
    object.insert("kind".to_string(), json!(module.kind));
    object.insert("extension".to_string(), json!(module.extension));
    if module.code_page != 0 {
        object.insert("codePage".to_string(), json!(module.code_page));
    }
    object.insert("sourceOffset".to_string(), json!(module.source_offset));
    if let Some(source_bytes) = module.source_bytes {
        object.insert("sourceBytes".to_string(), json!(source_bytes));
    }
    if let Some(line_count) = module.line_count {
        object.insert("lineCount".to_string(), json!(line_count));
    }
    if !module.sha256.is_empty() {
        object.insert("sha256".to_string(), json!(module.sha256));
    }
    if !module.sha256_basis.is_empty() {
        object.insert("sha256Basis".to_string(), json!(module.sha256_basis));
    }
    if !module.line_ending.is_empty() {
        object.insert("lineEnding".to_string(), json!(module.line_ending));
    }
    object.insert(
        "trailingNewline".to_string(),
        json!(module.trailing_newline),
    );
    if !module.primary_selector.is_empty() {
        object.insert(
            "primarySelector".to_string(),
            json!(module.primary_selector),
        );
    }
    if !module.selectors.is_empty() {
        object.insert("selectors".to_string(), json!(module.selectors));
    }
    if include_source && !module.source.is_empty() {
        object.insert("source".to_string(), json!(module.source));
    }
    if !module.warnings.is_empty() {
        object.insert("warnings".to_string(), json!(module.warnings));
    }
    Value::Object(object)
}

fn project_metadata_json(metadata: &ProjectMetadata) -> Value {
    let mut object = Map::new();
    if !metadata.stream_name.is_empty() {
        object.insert("streamName".to_string(), json!(metadata.stream_name));
    }
    object.insert("present".to_string(), json!(metadata.present));
    if metadata.line_count > 0 {
        object.insert("lineCount".to_string(), json!(metadata.line_count));
    }
    if !metadata.id.is_empty() {
        object.insert("id".to_string(), json!(metadata.id));
    }
    if !metadata.name.is_empty() {
        object.insert("name".to_string(), json!(metadata.name));
    }
    if !metadata.modules.is_empty() {
        object.insert(
            "modules".to_string(),
            Value::Array(
                metadata
                    .modules
                    .iter()
                    .map(project_module_declaration_json)
                    .collect(),
            ),
        );
    }
    if !metadata.references.is_empty() {
        object.insert(
            "references".to_string(),
            Value::Array(
                metadata
                    .references
                    .iter()
                    .map(project_reference_json)
                    .collect(),
            ),
        );
    }
    if !metadata.workspace_entries.is_empty() {
        object.insert(
            "workspaceEntries".to_string(),
            Value::Array(
                metadata
                    .workspace_entries
                    .iter()
                    .map(project_workspace_entry_json)
                    .collect(),
            ),
        );
    }
    if metadata.has_project_wm {
        object.insert("hasProjectWm".to_string(), json!(true));
    }
    if !metadata.project_wm_stream.is_empty() {
        object.insert(
            "projectWmStream".to_string(),
            json!(metadata.project_wm_stream),
        );
    }
    if !metadata.warnings.is_empty() {
        object.insert("warnings".to_string(), json!(metadata.warnings));
    }
    Value::Object(object)
}

fn project_module_declaration_json(module: &ProjectModuleDeclaration) -> Value {
    let mut object = Map::new();
    object.insert("kind".to_string(), json!(module.kind));
    object.insert("name".to_string(), json!(module.name));
    if !module.value.is_empty() {
        object.insert("value".to_string(), json!(module.value));
    }
    object.insert("line".to_string(), json!(module.line));
    Value::Object(object)
}

fn project_reference_json(reference: &ProjectReference) -> Value {
    json!({
        "kind": reference.kind,
        "value": reference.value,
        "line": reference.line,
    })
}

fn project_workspace_entry_json(entry: &ProjectWorkspaceEntry) -> Value {
    json!({
        "name": entry.name,
        "value": entry.value,
        "line": entry.line,
    })
}

fn office_compatibility_json(report: &OfficeCompatibilityReport) -> Value {
    let mut object = Map::new();
    object.insert(
        "officeLoadVerified".to_string(),
        json!(report.office_load_verified),
    );
    object.insert("status".to_string(), json!(report.status));
    if !report.risks.is_empty() {
        object.insert(
            "risks".to_string(),
            Value::Array(
                report
                    .risks
                    .iter()
                    .map(host_compatibility_warning_json)
                    .collect(),
            ),
        );
    }
    if !report.notes.is_empty() {
        object.insert("notes".to_string(), json!(report.notes));
    }
    Value::Object(object)
}

fn host_compatibility_warning_json(warning: &HostCompatibilityWarning) -> Value {
    let mut object = Map::new();
    object.insert("code".to_string(), json!(warning.code));
    object.insert("message".to_string(), json!(warning.message));
    if !warning.modules.is_empty() {
        object.insert("modules".to_string(), json!(warning.modules));
    }
    Value::Object(object)
}

pub(super) fn module_extract_item_json(
    module: &SourceModule,
    output_path: &Path,
    bytes_written: usize,
) -> Value {
    let mut object = Map::new();
    object.insert("number".to_string(), json!(module.number));
    object.insert("name".to_string(), json!(module.name));
    object.insert("streamName".to_string(), json!(module.stream_name));
    object.insert("kind".to_string(), json!(module.kind));
    object.insert("extension".to_string(), json!(module.extension));
    object.insert(
        "outputPath".to_string(),
        json!(output_path.to_string_lossy().to_string()),
    );
    object.insert("bytesWritten".to_string(), json!(bytes_written));
    if let Some(line_count) = module.line_count {
        object.insert("lineCount".to_string(), json!(line_count));
    }
    if !module.sha256.is_empty() {
        object.insert("sha256".to_string(), json!(module.sha256));
    }
    if !module.sha256_basis.is_empty() {
        object.insert("sha256Basis".to_string(), json!(module.sha256_basis));
    }
    if !module.line_ending.is_empty() {
        object.insert("lineEnding".to_string(), json!(module.line_ending));
    }
    object.insert(
        "trailingNewline".to_string(),
        json!(module.trailing_newline),
    );
    if !module.primary_selector.is_empty() {
        object.insert(
            "primarySelector".to_string(),
            json!(module.primary_selector),
        );
    }
    if !module.selectors.is_empty() {
        object.insert("selectors".to_string(), json!(module.selectors));
    }
    if !module.warnings.is_empty() {
        object.insert("warnings".to_string(), json!(module.warnings));
    }
    Value::Object(object)
}
