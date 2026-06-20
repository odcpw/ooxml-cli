use super::{HostCompatibilityWarning, OfficeCompatibilityReport, SourceModule, SourceProject};
pub(super) fn populate_office_compatibility(project: &mut SourceProject) {
    let host_warnings = host_compatibility_warnings(project);
    for warning in &host_warnings {
        if !project
            .warnings
            .iter()
            .any(|value| value == &warning.message)
        {
            project.warnings.push(warning.message.clone());
        }
    }
    let status = if host_warnings.is_empty() {
        "unverified"
    } else {
        "risk"
    };
    project.host_compatibility_warnings = host_warnings.clone();
    project.office_compatibility = OfficeCompatibilityReport {
        office_load_verified: false,
        status: status.to_string(),
        risks: host_warnings,
        notes: vec![
            "Package validation and source readback do not prove that Microsoft Office will load this VBA project without repair."
                .to_string(),
        ],
    };
}

fn host_compatibility_warnings(project: &SourceProject) -> Vec<HostCompatibilityWarning> {
    if project.family.trim().is_empty() {
        return Vec::new();
    }
    let mut excel_doc_modules = Vec::new();
    let mut powerpoint_doc_modules = Vec::new();
    for module in &project.modules {
        if !module_is_document_like(module) {
            continue;
        }
        let name = module.name.trim();
        if is_excel_document_module_name(name) {
            excel_doc_modules.push(name.to_string());
        } else if is_powerpoint_document_module_name(name) {
            powerpoint_doc_modules.push(name.to_string());
        }
    }
    let mut warnings = Vec::new();
    if project.family == "pptx" && !excel_doc_modules.is_empty() {
        warnings.push(HostCompatibilityWarning {
            code: "VBA_HOST_EXCEL_MODULES_IN_PPTM".to_string(),
            message: format!(
                "PowerPoint macro package contains Excel document module(s): {}. The package can be structurally valid while Office may repair or reject the VBA project; use a PowerPoint-native vbaProject.bin seed for PPTM outputs.",
                excel_doc_modules.join(", ")
            ),
            modules: excel_doc_modules,
        });
    }
    if project.family == "xlsx" && !powerpoint_doc_modules.is_empty() {
        warnings.push(HostCompatibilityWarning {
            code: "VBA_HOST_POWERPOINT_MODULES_IN_XLSM".to_string(),
            message: format!(
                "Excel macro package contains PowerPoint document-like module(s): {}. The package can be structurally valid while Office may repair or reject the VBA project; use an Excel-native vbaProject.bin seed for XLSM outputs.",
                powerpoint_doc_modules.join(", ")
            ),
            modules: powerpoint_doc_modules,
        });
    }
    warnings
}

fn module_is_document_like(module: &SourceModule) -> bool {
    module.kind.eq_ignore_ascii_case("class") || module.extension.eq_ignore_ascii_case(".cls")
}

fn is_excel_document_module_name(name: &str) -> bool {
    let normalized = name.trim().to_ascii_lowercase();
    normalized == "thisworkbook"
        || normalized
            .strip_prefix("sheet")
            .is_some_and(all_ascii_digits)
        || normalized
            .strip_prefix("chart")
            .is_some_and(all_ascii_digits)
}

fn is_powerpoint_document_module_name(name: &str) -> bool {
    let normalized = name.trim().to_ascii_lowercase();
    normalized == "thispresentation"
        || normalized
            .strip_prefix("slide")
            .is_some_and(all_ascii_digits)
}

fn all_ascii_digits(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit())
}
