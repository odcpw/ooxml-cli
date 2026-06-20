use crate::InspectPackageKind;

pub(super) const VBA_PROJECT_CONTENT_TYPE: &str = "application/vnd.ms-office.vbaProject";
pub(super) const VBA_PROJECT_REL_TYPE: &str =
    "http://schemas.microsoft.com/office/2006/relationships/vbaProject";

pub(super) struct VbaFamilySpec {
    pub(super) family: &'static str,
    pub(super) package_kind: InspectPackageKind,
    pub(super) default_main_part: &'static str,
    pub(super) default_vba_part: &'static str,
    pub(super) non_macro_content_type: &'static str,
    pub(super) macro_content_type: &'static str,
    pub(super) non_macro_extension: &'static str,
    pub(super) macro_extension: &'static str,
}

pub(super) const VBA_FAMILIES: &[VbaFamilySpec] = &[
    VbaFamilySpec {
        family: "pptx",
        package_kind: InspectPackageKind::Pptx,
        default_main_part: "/ppt/presentation.xml",
        default_vba_part: "/ppt/vbaProject.bin",
        non_macro_content_type: "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml",
        macro_content_type: "application/vnd.ms-powerpoint.presentation.macroEnabled.main+xml",
        non_macro_extension: ".pptx",
        macro_extension: ".pptm",
    },
    VbaFamilySpec {
        family: "xlsx",
        package_kind: InspectPackageKind::Xlsx,
        default_main_part: "/xl/workbook.xml",
        default_vba_part: "/xl/vbaProject.bin",
        non_macro_content_type: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml",
        macro_content_type: "application/vnd.ms-excel.sheet.macroEnabled.main+xml",
        non_macro_extension: ".xlsx",
        macro_extension: ".xlsm",
    },
];

pub(crate) struct VbaMutationOptions<'a> {
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
}

pub(super) struct VbaInfo {
    pub(super) family: &'static VbaFamilySpec,
    pub(super) package_type: &'static str,
    pub(super) macro_enabled: bool,
    pub(super) main_part_uri: String,
    pub(super) main_content_type: String,
    pub(super) project: Option<VbaProjectInfo>,
    pub(super) signature_artifacts: Vec<SignatureArtifact>,
    pub(super) warnings: Vec<String>,
}

pub(super) struct VbaProjectInfo {
    pub(super) part_uri: String,
    pub(super) content_type: String,
    pub(super) exists: bool,
    pub(super) size_bytes: Option<usize>,
    pub(super) sha256: Option<String>,
    pub(super) relationship_id: String,
    pub(super) relationship_type: String,
    pub(super) relationship_target: String,
}

#[derive(Clone)]
pub(super) struct SignatureArtifact {
    pub(super) kind: String,
    pub(super) part_uri: String,
    pub(super) source_uri: String,
    pub(super) relationship_id: String,
    pub(super) rel_type: String,
    pub(super) target: String,
}
