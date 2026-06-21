use serde_json::{Map, Value, json};

use crate::command_arg;

use super::model::{SignatureArtifact, VbaInfo, VbaProjectInfo};

pub(super) fn vba_info_json(info: &VbaInfo) -> Value {
    let mut object = Map::new();
    object.insert("family".to_string(), json!(info.family.family));
    object.insert("packageType".to_string(), json!(info.package_type));
    object.insert("macroEnabled".to_string(), json!(info.macro_enabled));
    object.insert(
        "hasVbaProject".to_string(),
        json!(info.project.as_ref().is_some_and(|project| project.exists)),
    );
    object.insert("mainPartUri".to_string(), json!(info.main_part_uri));
    object.insert("mainContentType".to_string(), json!(info.main_content_type));
    if let Some(project) = &info.project {
        object.insert("vbaProject".to_string(), vba_project_json(project));
    }
    object.insert(
        "nonMacroExtension".to_string(),
        json!(info.family.non_macro_extension),
    );
    object.insert(
        "macroExtension".to_string(),
        json!(info.family.macro_extension),
    );
    if !info.signature_artifacts.is_empty() {
        object.insert(
            "signatureArtifacts".to_string(),
            Value::Array(
                info.signature_artifacts
                    .iter()
                    .map(signature_artifact_json)
                    .collect(),
            ),
        );
    }
    if !info.warnings.is_empty() {
        object.insert("warnings".to_string(), json!(info.warnings));
    }
    Value::Object(object)
}

fn vba_project_json(project: &VbaProjectInfo) -> Value {
    let mut object = Map::new();
    object.insert("partUri".to_string(), json!(project.part_uri));
    object.insert("contentType".to_string(), json!(project.content_type));
    object.insert("exists".to_string(), json!(project.exists));
    if let Some(size) = project.size_bytes {
        object.insert("sizeBytes".to_string(), json!(size));
    }
    if let Some(sha256) = &project.sha256 {
        object.insert("sha256".to_string(), json!(sha256));
    }
    if !project.relationship_id.is_empty() {
        object.insert("relationshipId".to_string(), json!(project.relationship_id));
    }
    if !project.relationship_type.is_empty() {
        object.insert(
            "relationshipType".to_string(),
            json!(project.relationship_type),
        );
    }
    if !project.relationship_target.is_empty() {
        object.insert(
            "relationshipTarget".to_string(),
            json!(project.relationship_target),
        );
    }
    Value::Object(object)
}

fn signature_artifact_json(artifact: &SignatureArtifact) -> Value {
    let mut object = Map::new();
    object.insert("kind".to_string(), json!(artifact.kind));
    if !artifact.part_uri.is_empty() {
        object.insert("partUri".to_string(), json!(artifact.part_uri));
    }
    if !artifact.source_uri.is_empty() {
        object.insert("sourceUri".to_string(), json!(artifact.source_uri));
    }
    if !artifact.relationship_id.is_empty() {
        object.insert(
            "relationshipId".to_string(),
            json!(artifact.relationship_id),
        );
    }
    if !artifact.rel_type.is_empty() {
        object.insert("type".to_string(), json!(artifact.rel_type));
    }
    if !artifact.target.is_empty() {
        object.insert("target".to_string(), json!(artifact.target));
    }
    Value::Object(object)
}

pub(super) fn vba_mutation_result_json(
    action: &str,
    info: &VbaInfo,
    vba_part_uri: Option<&str>,
    macro_enabled: bool,
) -> Value {
    let mut object = Map::new();
    object.insert("action".to_string(), json!(action));
    object.insert("family".to_string(), json!(info.family.family));
    object.insert("mainPartUri".to_string(), json!(info.main_part_uri));
    if let Some(vba_part_uri) = vba_part_uri.filter(|part| !part.trim().is_empty()) {
        object.insert("vbaPartUri".to_string(), json!(vba_part_uri));
    }
    object.insert("macroEnabled".to_string(), json!(macro_enabled));
    Value::Object(object)
}

pub(super) fn vba_inspect_command(file: &str) -> String {
    format!("ooxml --json vba inspect {}", command_arg(file))
}

pub(super) fn vba_list_command(file: &str) -> String {
    format!("ooxml --json vba list {}", command_arg(file))
}

pub(super) fn vba_validate_command(file: &str) -> String {
    format!("ooxml --json validate --strict {}", command_arg(file))
}

pub(super) fn vba_conformance_command(file: &str) -> String {
    format!("ooxml --json conformance check {}", command_arg(file))
}

pub(super) fn vba_office_check_command(file: &str, info: &VbaInfo) -> Option<String> {
    if !info.macro_enabled && !info.project.as_ref().is_some_and(|project| project.exists) {
        return None;
    }
    Some(format!(
        "ooxml --json vba office-check {}",
        command_arg(file)
    ))
}

pub(super) fn vba_extract_bin_command(file: &str, info: &VbaInfo) -> Option<String> {
    if !info.project.as_ref().is_some_and(|project| project.exists) {
        return None;
    }
    Some(format!(
        "ooxml --json vba extract-bin {} --out vbaProject.bin",
        command_arg(file)
    ))
}

pub(super) fn vba_package_readback_command(file: &str, family: &str) -> String {
    match family {
        "pptx" => format!("ooxml --json pptx slides list {}", command_arg(file)),
        "docx" => format!("ooxml --json docx blocks {}", command_arg(file)),
        "xlsx" => format!("ooxml --json xlsx sheets list {}", command_arg(file)),
        _ => String::new(),
    }
}

pub(super) fn vba_extract_modules_template(file: &str) -> String {
    format!(
        "ooxml --json vba extract {} --out-dir macros",
        command_arg(file)
    )
}

pub(super) fn vba_next_mutation_template(file: &str, info: &VbaInfo) -> String {
    if info.macro_enabled || info.project.as_ref().is_some_and(|project| project.exists) {
        return format!(
            "ooxml --json vba remove {} --out {}",
            command_arg(file),
            command_arg(&vba_output_placeholder(info.family.non_macro_extension))
        );
    }
    format!(
        "ooxml --json vba attach {} --bin vbaProject.bin --out {}",
        command_arg(file),
        command_arg(&vba_output_placeholder(info.family.macro_extension))
    )
}

pub(super) fn vba_standalone_attach_template(bin_path: &str, family: &str) -> String {
    match family {
        "pptx" => format!(
            "ooxml --json vba attach deck.pptx --bin {} --out deck.pptm",
            command_arg(bin_path)
        ),
        "docx" => format!(
            "ooxml --json vba attach document.docx --bin {} --out document.docm",
            command_arg(bin_path)
        ),
        "xlsx" => format!(
            "ooxml --json vba attach workbook.xlsx --bin {} --out workbook.xlsm",
            command_arg(bin_path)
        ),
        _ => format!(
            "ooxml --json vba attach <target.pptx|target.docx|target.xlsx> --bin {} --out <macro-output.pptm|macro-output.docm|macro-output.xlsm>",
            command_arg(bin_path)
        ),
    }
}

pub(super) fn vba_attach_template_for_bin(bin_path: &str, info: &VbaInfo) -> String {
    format!(
        "ooxml --json vba attach {} --bin {} --out {}",
        command_arg(&vba_output_placeholder(info.family.non_macro_extension)),
        command_arg(bin_path),
        command_arg(&vba_output_placeholder(info.family.macro_extension))
    )
}

pub(super) fn vba_output_placeholder(extension: &str) -> String {
    if extension.is_empty() {
        "<out>".to_string()
    } else {
        format!("<out{extension}>")
    }
}
