use crate::{
    CliError, CliResult, InspectPackageKind, detect_inspect_package_type, package_type,
    zip_entry_names,
};

pub(super) fn resolve_template_kind(
    file: &str,
    requested: Option<&str>,
    unsupported_prefix: &str,
) -> CliResult<&'static str> {
    if let Some(requested) = requested.map(str::trim).filter(|value| !value.is_empty()) {
        match requested.to_ascii_lowercase().as_str() {
            "pptx" | "potx" => return Ok("pptx"),
            "xlsx" | "xltx" => return Ok("xlsx"),
            "auto" => {}
            other => {
                return Err(CliError::invalid_args(format!(
                    "invalid --for {other:?}; expected pptx, xlsx, or auto"
                )));
            }
        }
    }
    match package_type(file)? {
        "pptx" => Ok("pptx"),
        "xlsx" => Ok("xlsx"),
        "docx" => Err(CliError::unsupported_type(format!(
            "{unsupported_prefix} (detected: docx); pass --for to override"
        ))),
        _ => {
            let entries = zip_entry_names(file)?;
            match detect_inspect_package_type(file, &entries) {
                InspectPackageKind::Pptx => Ok("pptx"),
                InspectPackageKind::Xlsx => Ok("xlsx"),
                InspectPackageKind::Docx => Err(CliError::unsupported_type(format!(
                    "{unsupported_prefix} (detected: docx); pass --for to override"
                ))),
                InspectPackageKind::Unknown => Err(CliError::unsupported_type(format!(
                    "{unsupported_prefix} (detected: unknown); pass --for to override"
                ))),
            }
        }
    }
}

pub(super) fn parse_string_flag_local(args: &[String], flag: &str) -> CliResult<Option<String>> {
    let mut out = None;
    let mut i = 0;
    while i < args.len() {
        if args[i] == flag {
            let Some(value) = args.get(i + 1) else {
                return Err(CliError::invalid_args(format!("{flag} requires a value")));
            };
            out = Some(value.clone());
            i += 2;
        } else {
            i += 1;
        }
    }
    Ok(out)
}
