use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path};

use serde::{Deserialize, Serialize};

use crate::{CliError, CliResult};

pub(crate) const VBA_PROJECT_MANIFEST_FILE: &str = "vba-project.json";
pub(crate) const VBA_PROJECT_MANIFEST_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct VbaProjectManifest {
    pub(crate) schema_version: u32,
    pub(crate) project_name: String,
    pub(crate) code_page: u16,
    pub(crate) family: String,
    pub(crate) modules: Vec<VbaProjectManifestModule>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct VbaProjectManifestModule {
    pub(crate) name: String,
    pub(crate) file: String,
    pub(crate) kind: String,
    pub(crate) host_synthesized: bool,
    pub(crate) source_sha256: String,
}

pub(crate) fn read_vba_project_manifest(
    source_dir: &Path,
) -> CliResult<Option<VbaProjectManifest>> {
    let path = source_dir.join(VBA_PROJECT_MANIFEST_FILE);
    if !path.exists() {
        return Ok(None);
    }
    if !path.is_file() {
        return Err(CliError::invalid_args(format!(
            "VBA project manifest is not a file: {}",
            path.display()
        )));
    }
    let data = fs::read(&path).map_err(|err| {
        CliError::invalid_args(format!(
            "failed to read VBA project manifest {}: {err}",
            path.display()
        ))
    })?;
    let manifest = serde_json::from_slice::<VbaProjectManifest>(&data).map_err(|err| {
        CliError::invalid_args(format!(
            "invalid VBA project manifest {}: {err}",
            path.display()
        ))
    })?;
    validate_vba_project_manifest(&manifest, &path)?;
    Ok(Some(manifest))
}

pub(crate) fn write_vba_project_manifest(
    output_dir: &Path,
    manifest: &VbaProjectManifest,
) -> CliResult<String> {
    let path = output_dir.join(VBA_PROJECT_MANIFEST_FILE);
    validate_vba_project_manifest(manifest, &path)?;
    let mut data = serde_json::to_vec_pretty(manifest).map_err(|err| {
        CliError::unexpected(format!("failed to serialize VBA project manifest: {err}"))
    })?;
    data.push(b'\n');
    fs::write(&path, data).map_err(|err| {
        CliError::unexpected(format!(
            "failed to write VBA project manifest {}: {err}",
            path.display()
        ))
    })?;
    Ok(path.to_string_lossy().to_string())
}

pub(crate) fn manifest_relative_path(value: &str) -> CliResult<&Path> {
    let path = Path::new(value);
    if value.trim().is_empty()
        || value.contains('\\')
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(CliError::invalid_args(format!(
            "VBA project manifest module file must be a relative forward-slash path inside --source-dir: {value:?}"
        )));
    }
    Ok(path)
}

fn validate_vba_project_manifest(manifest: &VbaProjectManifest, path: &Path) -> CliResult<()> {
    if manifest.schema_version != VBA_PROJECT_MANIFEST_SCHEMA_VERSION {
        return Err(CliError::invalid_args(format!(
            "unsupported VBA project manifest schemaVersion {} in {}; expected {}",
            manifest.schema_version,
            path.display(),
            VBA_PROJECT_MANIFEST_SCHEMA_VERSION
        )));
    }
    if manifest.project_name.trim().is_empty() {
        return Err(CliError::invalid_args(format!(
            "VBA project manifest projectName is required: {}",
            path.display()
        )));
    }
    if manifest.code_page == 0 {
        return Err(CliError::invalid_args(format!(
            "VBA project manifest codePage must be positive: {}",
            path.display()
        )));
    }
    if !matches!(manifest.family.as_str(), "xlsx" | "pptx" | "docx") {
        return Err(CliError::invalid_args(format!(
            "VBA project manifest family must be xlsx, pptx, or docx: {}",
            path.display()
        )));
    }
    let mut names = BTreeSet::new();
    let mut files = BTreeSet::new();
    for module in &manifest.modules {
        if module.name.trim().is_empty() {
            return Err(CliError::invalid_args(format!(
                "VBA project manifest module name is required: {}",
                path.display()
            )));
        }
        manifest_relative_path(&module.file)?;
        if !names.insert(module.name.to_ascii_lowercase()) {
            return Err(CliError::invalid_args(format!(
                "VBA project manifest contains duplicate module name {}",
                module.name
            )));
        }
        if !files.insert(module.file.to_ascii_lowercase()) {
            return Err(CliError::invalid_args(format!(
                "VBA project manifest contains duplicate module file {}",
                module.file
            )));
        }
        if !matches!(
            module.kind.as_str(),
            "standard" | "class" | "document" | "userform"
        ) {
            return Err(CliError::invalid_args(format!(
                "VBA project manifest module {} has unsupported kind {:?}",
                module.name, module.kind
            )));
        }
        if module.source_sha256.len() != 64
            || !module
                .source_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(CliError::invalid_args(format!(
                "VBA project manifest module {} has invalid sourceSha256",
                module.name
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> VbaProjectManifest {
        VbaProjectManifest {
            schema_version: VBA_PROJECT_MANIFEST_SCHEMA_VERSION,
            project_name: "VBAProject".to_string(),
            code_page: 1252,
            family: "xlsx".to_string(),
            modules: vec![VbaProjectManifestModule {
                name: "Hello".to_string(),
                file: "Hello.bas".to_string(),
                kind: "standard".to_string(),
                host_synthesized: false,
                source_sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                    .to_string(),
            }],
        }
    }

    #[test]
    fn manifest_round_trip_is_deterministic_and_newline_terminated() {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let temp_dir = std::env::temp_dir().join(format!(
            "ooxml-vba-manifest-{}-{suffix}",
            std::process::id()
        ));
        fs::create_dir_all(&temp_dir).expect("create manifest temp dir");

        let first_path =
            write_vba_project_manifest(&temp_dir, &manifest()).expect("write first manifest");
        let first = fs::read(&first_path).expect("read first manifest");
        let parsed = read_vba_project_manifest(&temp_dir)
            .expect("read manifest")
            .expect("manifest exists");
        let second_path =
            write_vba_project_manifest(&temp_dir, &parsed).expect("write second manifest");
        let second = fs::read(second_path).expect("read second manifest");

        assert_eq!(first, second);
        assert!(first.ends_with(b"\n"));
        assert!(
            !first.contains(&b'\r'),
            "manifest serialization must use LF on every platform"
        );
        assert_eq!(parsed.modules[0].kind, "standard");
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn manifest_rejects_source_paths_outside_source_dir() {
        let mut invalid = manifest();
        invalid.modules[0].file = "../Hello.bas".to_string();
        let error = validate_vba_project_manifest(&invalid, Path::new("vba-project.json"))
            .expect_err("parent traversal must fail");
        assert_eq!(error.code, "invalid_args");
        assert!(error.message.contains("inside --source-dir"));
    }

    #[test]
    fn manifest_paths_are_relative_and_use_forward_slashes() {
        let mut valid = manifest();
        valid.modules[0].file = "modules/Hello.bas".to_string();
        validate_vba_project_manifest(&valid, Path::new("vba-project.json"))
            .expect("forward-slash relative manifest path");

        valid.modules[0].file = "modules\\Hello.bas".to_string();
        let error = validate_vba_project_manifest(&valid, Path::new("vba-project.json"))
            .expect_err("backslash manifest path must fail on every platform");
        assert_eq!(error.code, "invalid_args");
        assert!(error.message.contains("inside --source-dir"));
    }
}
