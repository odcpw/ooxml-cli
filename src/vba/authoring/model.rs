use std::collections::BTreeSet;

use super::{VbaAuthoringError, VbaAuthoringResult};

pub(super) const DEFAULT_PROJECT_NAME: &str = "VBAProject";
pub(super) const DEFAULT_WORD_PROJECT_NAME: &str = "Project";
pub(super) const DEFAULT_CODE_PAGE: u16 = 1252;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum VbaHostFamily {
    Xlsx,
    Pptx,
    Docx,
}

impl VbaHostFamily {
    #[cfg(test)]
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Xlsx => "xlsx",
            Self::Pptx => "pptx",
            Self::Docx => "docx",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum VbaModuleKind {
    Document,
    Standard,
    Class,
    UserForm,
}

impl VbaModuleKind {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Document => "document",
            Self::Standard => "standard",
            Self::Class => "class",
            Self::UserForm => "userform",
        }
    }

    pub(super) fn project_key(self) -> &'static str {
        match self {
            Self::Document => "Document",
            Self::Standard => "Module",
            Self::Class => "Class",
            Self::UserForm => "BaseClass",
        }
    }

    pub(super) fn dir_record_id(self) -> u16 {
        match self {
            Self::Document => 0x0022,
            Self::Standard => 0x0021,
            Self::Class => 0x0022,
            Self::UserForm => 0x0022,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct VbaUserFormModel {
    pub(super) caption: String,
}

impl VbaUserFormModel {
    pub(super) fn new(caption: impl Into<String>) -> Self {
        Self {
            caption: caption.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct VbaModuleModel {
    pub(super) name: String,
    pub(super) stream_name: String,
    pub(super) kind: VbaModuleKind,
    pub(super) source: Vec<u8>,
    pub(super) user_form: Option<VbaUserFormModel>,
}

impl VbaModuleModel {
    pub(super) fn excel_workbook_document() -> Self {
        Self::new(
            "ThisWorkbook",
            None::<String>,
            VbaModuleKind::Document,
            b"Attribute VB_Name = \"ThisWorkbook\"\r\nAttribute VB_Base = \"0{00020819-0000-0000-C000-000000000046}\"\r\nAttribute VB_GlobalNameSpace = False\r\nAttribute VB_Creatable = False\r\nAttribute VB_PredeclaredId = True\r\nAttribute VB_Exposed = True\r\nAttribute VB_TemplateDerived = False\r\nAttribute VB_Customizable = True\r\n".to_vec(),
        )
    }

    pub(super) fn excel_sheet_document(name: impl Into<String>) -> Self {
        let name = name.into();
        let source = format!(
            "Attribute VB_Name = \"{name}\"\r\nAttribute VB_Base = \"0{{00020820-0000-0000-C000-000000000046}}\"\r\nAttribute VB_GlobalNameSpace = False\r\nAttribute VB_Creatable = False\r\nAttribute VB_PredeclaredId = True\r\nAttribute VB_Exposed = True\r\nAttribute VB_TemplateDerived = False\r\nAttribute VB_Customizable = True\r\n"
        );
        Self::new(
            name,
            None::<String>,
            VbaModuleKind::Document,
            source.into_bytes(),
        )
    }

    pub(super) fn word_document_document() -> Self {
        Self::new(
            "ThisDocument",
            None::<String>,
            VbaModuleKind::Document,
            b"Attribute VB_Name = \"ThisDocument\"\r\nAttribute VB_Base = \"1Normal.ThisDocument\"\r\nAttribute VB_GlobalNameSpace = False\r\nAttribute VB_Creatable = False\r\nAttribute VB_PredeclaredId = True\r\nAttribute VB_Exposed = True\r\nAttribute VB_TemplateDerived = True\r\nAttribute VB_Customizable = True\r\n".to_vec(),
        )
    }

    #[cfg(test)]
    pub(super) fn standard(name: impl Into<String>, source: Vec<u8>) -> Self {
        Self::new(name, None::<String>, VbaModuleKind::Standard, source)
    }

    #[cfg(test)]
    pub(super) fn class(name: impl Into<String>, source: Vec<u8>) -> Self {
        Self::new(name, None::<String>, VbaModuleKind::Class, source)
    }

    pub(super) fn new(
        name: impl Into<String>,
        stream_name: Option<impl Into<String>>,
        kind: VbaModuleKind,
        source: Vec<u8>,
    ) -> Self {
        let name = name.into();
        let stream_name = stream_name.map(Into::into).unwrap_or_else(|| name.clone());
        Self {
            name,
            stream_name,
            kind,
            source,
            user_form: None,
        }
    }

    pub(super) fn user_form(
        name: impl Into<String>,
        stream_name: Option<impl Into<String>>,
        source: Vec<u8>,
        form: VbaUserFormModel,
    ) -> Self {
        let mut module = Self::new(name, stream_name, VbaModuleKind::UserForm, source);
        module.user_form = Some(form);
        module
    }

    pub(super) fn validate_for_build(&self) -> VbaAuthoringResult<()> {
        validate_identifier("module name", &self.name)?;
        validate_identifier("module stream name", &self.stream_name)?;
        if self.source.is_empty() {
            return Err(VbaAuthoringError::invalid_model(format!(
                "VBA module {} has empty source",
                self.name
            )));
        }
        if self.kind == VbaModuleKind::UserForm {
            let Some(form) = &self.user_form else {
                return Err(VbaAuthoringError::invalid_model(format!(
                    "VBA UserForm module {} is missing designer metadata",
                    self.name
                )));
            };
            if form.caption.len() > 255 {
                return Err(VbaAuthoringError::invalid_model(format!(
                    "VBA UserForm module {} caption is longer than 255 bytes",
                    self.name
                )));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct VbaProjectModel {
    pub(super) host_family: VbaHostFamily,
    pub(super) project_name: String,
    pub(super) code_page: u16,
    pub(super) modules: Vec<VbaModuleModel>,
}

impl VbaProjectModel {
    pub(super) fn xlsx(modules: Vec<VbaModuleModel>) -> Self {
        Self {
            host_family: VbaHostFamily::Xlsx,
            project_name: DEFAULT_PROJECT_NAME.to_string(),
            code_page: DEFAULT_CODE_PAGE,
            modules,
        }
    }

    pub(super) fn pptx(modules: Vec<VbaModuleModel>) -> Self {
        Self {
            host_family: VbaHostFamily::Pptx,
            project_name: DEFAULT_PROJECT_NAME.to_string(),
            code_page: DEFAULT_CODE_PAGE,
            modules,
        }
    }

    pub(super) fn docx(modules: Vec<VbaModuleModel>) -> Self {
        Self {
            host_family: VbaHostFamily::Docx,
            project_name: DEFAULT_WORD_PROJECT_NAME.to_string(),
            code_page: DEFAULT_CODE_PAGE,
            modules,
        }
    }

    pub(super) fn validate(&self) -> VbaAuthoringResult<()> {
        validate_identifier("project name", &self.project_name)?;
        if self.code_page != DEFAULT_CODE_PAGE {
            return Err(VbaAuthoringError::invalid_model(
                "pure VBA authoring currently supports only Windows-1252 code page 1252",
            ));
        }
        if self.modules.is_empty() {
            return Err(VbaAuthoringError::invalid_model(
                "VBA project must contain at least one module",
            ));
        }

        let mut names = BTreeSet::new();
        let mut stream_names = BTreeSet::new();
        for module in &self.modules {
            validate_identifier("module name", &module.name)?;
            validate_identifier("module stream name", &module.stream_name)?;
            if module.source.is_empty() {
                return Err(VbaAuthoringError::invalid_model(format!(
                    "VBA module {} has empty source",
                    module.name
                )));
            }
            module.validate_for_build()?;
            let name_key = module.name.to_ascii_lowercase();
            if !names.insert(name_key) {
                return Err(VbaAuthoringError::invalid_model(format!(
                    "duplicate VBA module name {}",
                    module.name
                )));
            }
            let stream_key = module.stream_name.to_ascii_lowercase();
            if !stream_names.insert(stream_key) {
                return Err(VbaAuthoringError::invalid_model(format!(
                    "duplicate VBA module stream name {}",
                    module.stream_name
                )));
            }
        }
        Ok(())
    }
}

fn validate_identifier(label: &str, value: &str) -> VbaAuthoringResult<()> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(VbaAuthoringError::invalid_model(format!(
            "{label} is required"
        )));
    }
    if trimmed != value {
        return Err(VbaAuthoringError::invalid_model(format!(
            "{label} {value:?} must not have leading or trailing whitespace"
        )));
    }
    if value.len() > 255 {
        return Err(VbaAuthoringError::invalid_model(format!(
            "{label} {value:?} is longer than 255 bytes"
        )));
    }
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return Err(VbaAuthoringError::invalid_model(format!(
            "{label} is required"
        )));
    };
    if !(first == '_' || first.is_ascii_alphabetic()) {
        return Err(VbaAuthoringError::invalid_model(format!(
            "{label} {value:?} must start with a letter or underscore"
        )));
    }
    if chars.any(|ch| !(ch == '_' || ch.is_ascii_alphanumeric())) {
        return Err(VbaAuthoringError::invalid_model(format!(
            "{label} {value:?} must contain only ASCII letters, digits, or underscores"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_model_is_xlsx_vbaproject_codepage_1252() {
        let project = VbaProjectModel::xlsx(vec![VbaModuleModel::standard(
            "Module1",
            b"Public Sub Hello()\r\nEnd Sub\r\n".to_vec(),
        )]);
        assert_eq!(project.host_family.as_str(), "xlsx");
        assert_eq!(project.project_name, "VBAProject");
        assert_eq!(project.code_page, 1252);
        assert!(project.validate().is_ok());
    }

    #[test]
    fn pptx_model_uses_user_modules_without_host_documents() {
        let project = VbaProjectModel::pptx(vec![VbaModuleModel::standard(
            "Module1",
            b"Public Sub Hello()\r\nEnd Sub\r\n".to_vec(),
        )]);
        assert_eq!(project.host_family.as_str(), "pptx");
        assert_eq!(project.project_name, "VBAProject");
        assert_eq!(project.code_page, 1252);
        assert_eq!(project.modules.len(), 1);
        assert_eq!(project.modules[0].name, "Module1");
        assert!(project.validate().is_ok());
    }

    #[test]
    fn docx_model_uses_standard_modules_without_host_documents() {
        let project = VbaProjectModel::docx(vec![VbaModuleModel::standard(
            "Module1",
            b"Public Sub Hello()\r\nEnd Sub\r\n".to_vec(),
        )]);
        assert_eq!(project.host_family.as_str(), "docx");
        assert_eq!(project.project_name, "Project");
        assert_eq!(project.code_page, 1252);
        assert_eq!(project.modules.len(), 1);
        assert_eq!(project.modules[0].name, "Module1");
        assert!(project.validate().is_ok());
    }

    #[test]
    fn detects_duplicate_module_stream_names_case_insensitively() {
        let project = VbaProjectModel::xlsx(vec![
            VbaModuleModel::standard("Module1", b"Sub A()\r\nEnd Sub\r\n".to_vec()),
            VbaModuleModel::new(
                "Module2",
                Some("module1"),
                VbaModuleKind::Standard,
                b"Sub B()\r\nEnd Sub\r\n".to_vec(),
            ),
        ]);
        let err = project.validate().expect_err("duplicate stream");
        assert!(err.message.contains("duplicate VBA module stream name"));
    }
}
