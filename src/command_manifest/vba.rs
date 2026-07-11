use serde_json::Value;

use super::{CommandId, CommandSpec, ExecutionSupport, FlagSpec};

pub(super) const COMMAND_COUNT: usize = 16;
pub(super) const LEGACY_START: usize = 293;

command_id_enum! {
pub(crate) enum VbaCommandId {
    Vba,
    BuildBin,
    Create,
    Rebuild,
    Inspect,
    ExtractBin,
    InspectBin,
    List,
    Extract,
    AddModule,
    ReplaceModule,
    RemoveModule,
    OfficeCheck,
    RunSmoke,
    Attach,
    Remove,
}}

pub(super) fn command_specs() -> Vec<CommandSpec> {
    vec![
        spec(
            VbaCommandId::Vba,
            &["vba"],
            "vba",
            "Inspect and edit Rust-supported VBA macro project surfaces.",
            &[],
            vec![],
            ExecutionSupport::GroupOnly {
                reason: Some("command group help surface; run a VBA leaf command for work"),
            },
            None,
        ),
        spec(
            VbaCommandId::BuildBin,
            &["vba", "build-bin"],
            "build-bin --family xlsx|pptx|docx --source Module1.bas --out vbaProject.bin",
            "Build a source-only vbaProject.bin from .bas/.cls source using pure Rust; XLSM also accepts minimal .frm for package/list/extract only, not runtime-loaded forms or PPTM/DOCM UserForms.",
            &["module"],
            vec![
                flag(
                    "--family",
                    "family",
                    "string",
                    "target host family; xlsx, pptx, and docx support .bas/.cls, while only xlsx accepts minimal .frm for XLSM package/list/extract; generated forms are not runtime-loadable and PPTM/DOCM UserForms are unsupported",
                ),
                flag(
                    "--force",
                    "force",
                    "bool",
                    "overwrite an existing vbaProject.bin output",
                ),
                flag("--out", "out", "string", "output vbaProject.bin path"),
                flag(
                    "--source",
                    "source",
                    "stringArray",
                    "repeatable .bas/.cls source file; xlsx alone also accepts minimal .frm for XLSM package/list/extract, not runtime-loadable forms or PPTM/DOCM UserForms",
                ),
            ],
            ExecutionSupport::DirectOnly {
                reason: Some(
                    "creates a standalone vbaProject.bin rather than mutating the open package session; use vba attach as the serve/apply op",
                ),
            },
            None,
        ),
        spec(
            VbaCommandId::Create,
            &["vba", "create"],
            "create <workbook.xlsx|deck.pptx|document.docx> --pure --source Module1.bas --out <workbook.xlsm|deck.pptm|document.docm>",
            "Create an XLSM/PPTM/DOCM from an existing package and VBA source files using pure Rust.",
            &["package", "module"],
            vec![
                flag("--backup", "backup", "string", "backup path for --in-place"),
                flag(
                    "--dry-run",
                    "dryRun",
                    "bool",
                    "pure mode only: validate and report without writing",
                ),
                flag(
                    "--enable-vba-object-model-access",
                    "enableVbaObjectModelAccess",
                    "bool",
                    "legacy Office-COM mode only: temporarily enable Trust access to the VBA project object model",
                ),
                flag(
                    "--extract-bin",
                    "extractBin",
                    "string",
                    "legacy Office-COM mode only: optional path to write the created vbaProject.bin seed",
                ),
                flag(
                    "--family",
                    "family",
                    "string",
                    "target Office family; pure mode supports xlsx, pptx, or docx and can infer from input extension",
                ),
                flag(
                    "--force",
                    "force",
                    "bool",
                    "legacy Office-COM mode only: overwrite existing helper outputs",
                ),
                flag(
                    "--in-place",
                    "inPlace",
                    "bool",
                    "write back to the input package",
                ),
                flag(
                    "--no-validate",
                    "noValidate",
                    "bool",
                    "skip strict package validation after mutation",
                ),
                flag(
                    "--office-create-script",
                    "officeCreateScript",
                    "string",
                    "legacy Office-COM mode only: path to windows-office-vba-create.ps1",
                ),
                flag(
                    "--out",
                    "out",
                    "string",
                    "pure mode output macro-enabled package path",
                ),
                flag(
                    "--pure",
                    "pure",
                    "bool",
                    "build vbaProject.bin in Rust and attach it to the input package",
                ),
                flag(
                    "--source",
                    "source",
                    "stringArray",
                    "repeatable .bas/.cls source file, plus package/list/extract-only .frm for pure XLSM mode; legacy Office-COM mode supports .bas/.cls",
                ),
                flag(
                    "--visible",
                    "visible",
                    "bool",
                    "show the Office application window during creation",
                ),
            ],
            ExecutionSupport::ServeMutation {
                reason: Some(
                    "preferred cross-platform macro authoring path; legacy Office-COM create remains available without --pure for XLSM/PPTM seeds",
                ),
            },
            Some(serde_json::json!({
                "modes": [
                    {
                        "allowedFlags": [
                            "--pure",
                            "--source",
                            "--family",
                            "--out",
                            "--in-place",
                            "--backup",
                            "--dry-run",
                            "--no-validate"
                        ],
                        "conflictsWith": [
                            "--extract-bin",
                            "--office-create-script",
                            "--enable-vba-object-model-access",
                            "--visible",
                            "--force"
                        ],
                        "name": "pure",
                        "recommendedCommand": "ooxml --json vba create <input.xlsx|input.pptx|input.docx> --pure --source Module1.bas --out <output.xlsm|output.pptm|output.docm>",
                        "when": [
                            "--pure"
                        ]
                    },
                    {
                        "allowedFlags": [
                            "--family",
                            "--source",
                            "--extract-bin",
                            "--office-create-script",
                            "--enable-vba-object-model-access",
                            "--visible",
                            "--force"
                        ],
                        "conflictsWith": [
                            "--out",
                            "--backup",
                            "--dry-run",
                            "--no-validate",
                            "--in-place"
                        ],
                        "name": "legacy-office-com",
                        "recommendedCommand": "ooxml --json vba create <input.xlsx|input.pptx> <output.xlsm|output.pptm> --office-create-script <windows-office-vba-create.ps1>",
                        "when": [
                            "not --pure"
                        ]
                    }
                ],
                "rules": [
                    "--pure cannot be combined with legacy Office-COM create flags.",
                    "Pure mode writes with --out, --in-place, or --dry-run; legacy Office-COM mode uses the positional output path."
                ]
            })),
        ),
        spec(
            VbaCommandId::Rebuild,
            &["vba", "rebuild"],
            "rebuild <workbook.xlsm|deck.pptm|document.docm> --source-dir macros --out <edited.xlsm|edited.pptm|edited.docm>",
            "Rebuild a macro-enabled package from .bas/.cls sources using pure Rust; XLSM also accepts minimal .frm for package/list/extract only, not runtime-loaded forms or PPTM/DOCM UserForms.",
            &["package", "module"],
            vec![
                flag("--backup", "backup", "string", "backup path for --in-place"),
                flag(
                    "--family",
                    "family",
                    "string",
                    "target Office family; supports xlsx, pptx, or docx and can infer from input extension",
                ),
                flag(
                    "--in-place",
                    "inPlace",
                    "bool",
                    "write back to the input package",
                ),
                flag(
                    "--no-validate",
                    "noValidate",
                    "bool",
                    "skip strict package validation after mutation",
                ),
                flag(
                    "--out",
                    "out",
                    "string",
                    "output macro-enabled package path",
                ),
                flag(
                    "--source-dir",
                    "sourceDir",
                    "string",
                    "directory recursively scanned for .bas/.cls source; XLSM rebuild also scans minimal .frm for package/list/extract, but generated forms are not runtime-loadable and PPTM/DOCM UserForms are unsupported",
                ),
            ],
            ExecutionSupport::ServeMutation {
                reason: Some(
                    "safe module-set replacement path; rebuilds a fresh source-only vbaProject.bin rather than patching Office-authored binary metadata",
                ),
            },
            None,
        ),
        spec(
            VbaCommandId::Inspect,
            &["vba", "inspect"],
            "inspect <file>",
            "Inspect opaque VBA package state for XLSM/PPTM/DOCM package wiring.",
            &["package", "module"],
            vec![],
            ExecutionSupport::DirectOnly {
                reason: Some("read-only command; use vba attach/remove for package mutation"),
            },
            None,
        ),
        spec(
            VbaCommandId::ExtractBin,
            &["vba", "extract-bin"],
            "extract-bin <file>",
            "Extract opaque vbaProject.bin bytes.",
            &["package", "module"],
            vec![flag("--out", "out", "string", "output vbaProject.bin path")],
            ExecutionSupport::DirectOnly {
                reason: Some("read-only binary extraction command"),
            },
            None,
        ),
        spec(
            VbaCommandId::InspectBin,
            &["vba", "inspect-bin"],
            "inspect-bin <vbaProject.bin>",
            "Inspect a standalone parseable vbaProject.bin before package attach.",
            &["module"],
            vec![flag(
                "--family",
                "family",
                "string",
                "target host family for compatibility checks: pptx, docx, or xlsx",
            )],
            ExecutionSupport::DirectOnly {
                reason: Some("read-only source-module inspection command"),
            },
            None,
        ),
        spec(
            VbaCommandId::List,
            &["vba", "list"],
            "list <file>",
            "List parseable VBA source modules with selectors and SHA-256 digests.",
            &["package", "module"],
            vec![],
            ExecutionSupport::DirectOnly {
                reason: Some("read-only source-module listing command"),
            },
            None,
        ),
        spec(
            VbaCommandId::Extract,
            &["vba", "extract"],
            "extract <file>",
            "Extract parseable VBA source to .bas/.cls files and minimal .frm from supported XLSM packages; generated forms are not runtime-loadable and PPTM/DOCM UserForms are unsupported.",
            &["package", "module"],
            vec![
                flag(
                    "--module",
                    "module",
                    "string",
                    "optional module selector from vba list",
                ),
                flag(
                    "--out-dir",
                    "outDir",
                    "string",
                    "directory for extracted .bas/.cls modules and minimal .frm from supported XLSM packages; forms are not runtime-loadable and PPTM/DOCM UserForms are unsupported",
                ),
            ],
            ExecutionSupport::DirectOnly {
                reason: Some(
                    "source extraction writes .bas/.cls and supported XLSM .frm files but does not mutate the Office package; .frm support is package/list/extract only, not runtime-loadable or available for PPTM/DOCM",
                ),
            },
            None,
        ),
        spec(
            VbaCommandId::AddModule,
            &["vba", "add-module"],
            "add-module <file>",
            "Experimental source-stream rewrite for non-Office-shaped VBA projects; Office-authored projects are guarded.",
            &["package", "module"],
            vec![
                flag(
                    "--allow-experimental-vba-source-rewrite",
                    "allowExperimentalVbaSourceRewrite",
                    "bool",
                    "allow source rewriting that is package-valid but not Office-load verified",
                ),
                flag(
                    "--backup",
                    "backup",
                    "string",
                    "backup file path for --in-place",
                ),
                flag(
                    "--dry-run",
                    "dryRun",
                    "bool",
                    "validate mutation without writing an output file",
                ),
                flag(
                    "--expect-module-count",
                    "expectModuleCount",
                    "integer",
                    "guard on the current module count before adding",
                ),
                flag(
                    "--in-place",
                    "inPlace",
                    "bool",
                    "modify the input file in place",
                ),
                flag(
                    "--kind",
                    "kind",
                    "string",
                    "module kind: standard/bas or class/cls",
                ),
                flag("--name", "name", "string", "module name to add"),
                flag(
                    "--no-validate",
                    "noValidate",
                    "bool",
                    "skip validation after mutation",
                ),
                flag("--out", "out", "string", "output file path"),
                flag("--source", "source", "string", ".bas/.cls source file"),
            ],
            ExecutionSupport::DirectOnly {
                reason: Some(
                    "not a first-class macro authoring path; use vba create --pure or vba attach for opaque seeds",
                ),
            },
            None,
        ),
        spec(
            VbaCommandId::ReplaceModule,
            &["vba", "replace-module"],
            "replace-module <file>",
            "Experimental source-stream rewrite for non-Office-shaped VBA projects; Office-authored projects are guarded.",
            &["package", "module"],
            vec![
                flag(
                    "--allow-experimental-vba-source-rewrite",
                    "allowExperimentalVbaSourceRewrite",
                    "bool",
                    "allow source rewriting that is package-valid but not Office-load verified",
                ),
                flag(
                    "--backup",
                    "backup",
                    "string",
                    "backup file path for --in-place",
                ),
                flag(
                    "--dry-run",
                    "dryRun",
                    "bool",
                    "validate mutation without writing an output file",
                ),
                flag(
                    "--expect-sha256",
                    "expectSha256",
                    "string",
                    "guard on the current decoded module-source SHA-256",
                ),
                flag(
                    "--in-place",
                    "inPlace",
                    "bool",
                    "modify the input file in place",
                ),
                flag(
                    "--module",
                    "module",
                    "string",
                    "module selector from vba list",
                ),
                flag(
                    "--no-validate",
                    "noValidate",
                    "bool",
                    "skip validation after mutation",
                ),
                flag("--out", "out", "string", "output file path"),
                flag("--source", "source", "string", ".bas/.cls source file"),
            ],
            ExecutionSupport::DirectOnly {
                reason: Some(
                    "not a first-class macro authoring path; use vba create --pure or vba attach for opaque seeds",
                ),
            },
            None,
        ),
        spec(
            VbaCommandId::RemoveModule,
            &["vba", "remove-module"],
            "remove-module <file>",
            "Experimental source-stream rewrite for non-Office-shaped VBA projects; Office-authored projects are guarded.",
            &["package", "module"],
            vec![
                flag(
                    "--allow-experimental-vba-source-rewrite",
                    "allowExperimentalVbaSourceRewrite",
                    "bool",
                    "allow source rewriting that is package-valid but not Office-load verified",
                ),
                flag(
                    "--backup",
                    "backup",
                    "string",
                    "backup file path for --in-place",
                ),
                flag(
                    "--dry-run",
                    "dryRun",
                    "bool",
                    "validate mutation without writing an output file",
                ),
                flag(
                    "--expect-sha256",
                    "expectSha256",
                    "string",
                    "guard on the current decoded module-source SHA-256",
                ),
                flag(
                    "--in-place",
                    "inPlace",
                    "bool",
                    "modify the input file in place",
                ),
                flag(
                    "--module",
                    "module",
                    "string",
                    "module selector from vba list",
                ),
                flag(
                    "--no-validate",
                    "noValidate",
                    "bool",
                    "skip validation after mutation",
                ),
                flag("--out", "out", "string", "output file path"),
            ],
            ExecutionSupport::DirectOnly {
                reason: Some(
                    "not a first-class macro authoring path; use vba create --pure, vba attach for opaque seeds, or vba remove for package-level removal",
                ),
            },
            None,
        ),
        spec(
            VbaCommandId::OfficeCheck,
            &["vba", "office-check"],
            "office-check <file>",
            "Validate a macro package and run a local Office-open check when available.",
            &["module"],
            vec![flag(
                "--out-dir",
                "outDir",
                "string",
                "optional directory to keep Office-open check output for inspection",
            )],
            ExecutionSupport::DirectOnly {
                reason: Some(
                    "read-only compatibility evidence; on Windows this prefers Microsoft Office COM, but macros are not executed",
                ),
            },
            None,
        ),
        spec(
            VbaCommandId::RunSmoke,
            &["vba", "run-smoke"],
            "run-smoke [file.xlsm]",
            "Explicit opt-in Excel VBA macro execution smoke for a harmless generated or provided XLSM.",
            &["module"],
            vec![
                flag(
                    "--expected-cell",
                    "expectedCell",
                    "string",
                    "Excel cell to verify after the macro run; default A1",
                ),
                flag(
                    "--expected-value",
                    "expectedValue",
                    "string",
                    "expected cell or marker value after the macro run",
                ),
                flag(
                    "--macro",
                    "macro",
                    "string",
                    "macro name to run in a provided .xlsm; generated smoke workbooks use AgentSmokeRun",
                ),
                flag(
                    "--out-dir",
                    "outDir",
                    "string",
                    "directory for smoke artifacts and summary.json",
                ),
                flag(
                    "--smoke-mode",
                    "smokeMode",
                    "string",
                    "generated smoke mode when no file is supplied: Standard or Class; omit for provided .xlsm files",
                ),
                flag(
                    "--timeout-seconds",
                    "timeoutSeconds",
                    "integer",
                    "bounded Office COM macro-run timeout; default 120",
                ),
                flag(
                    "--visible",
                    "visible",
                    "bool",
                    "show Excel during the macro-run smoke",
                ),
            ],
            ExecutionSupport::DirectOnly {
                reason: Some(
                    "Windows desktop Excel proof gate; executes VBA through Office COM, so use only when the user explicitly asks for macro execution proof",
                ),
            },
            None,
        ),
        spec(
            VbaCommandId::Attach,
            &["vba", "attach"],
            "attach <file>",
            "Attach or replace opaque vbaProject.bin and macro package wiring.",
            &["package", "module"],
            vec![
                flag(
                    "--allow-host-family-risk",
                    "allowHostFamilyRisk",
                    "bool",
                    "accepted for legacy CLI compatibility; opaque Rust attach does not parse source-project host risk yet",
                ),
                flag(
                    "--backup",
                    "backup",
                    "string",
                    "backup file path for --in-place",
                ),
                flag("--bin", "bin", "string", "vbaProject.bin to attach"),
                flag(
                    "--dry-run",
                    "dryRun",
                    "bool",
                    "validate mutation without writing an output file",
                ),
                flag(
                    "--in-place",
                    "inPlace",
                    "bool",
                    "modify the input file in place",
                ),
                flag(
                    "--no-validate",
                    "noValidate",
                    "bool",
                    "skip validation after mutation",
                ),
                flag("--out", "out", "string", "output file path"),
            ],
            ExecutionSupport::ServeMutation { reason: None },
            None,
        ),
        spec(
            VbaCommandId::Remove,
            &["vba", "remove"],
            "remove <file>",
            "Remove opaque VBA package wiring and restore non-macro main content type.",
            &["package", "module"],
            vec![
                flag(
                    "--backup",
                    "backup",
                    "string",
                    "backup file path for --in-place",
                ),
                flag(
                    "--dry-run",
                    "dryRun",
                    "bool",
                    "validate mutation without writing an output file",
                ),
                flag(
                    "--in-place",
                    "inPlace",
                    "bool",
                    "modify the input file in place",
                ),
                flag(
                    "--no-validate",
                    "noValidate",
                    "bool",
                    "skip validation after mutation",
                ),
                flag("--out", "out", "string", "output file path"),
            ],
            ExecutionSupport::ServeMutation { reason: None },
            None,
        ),
    ]
}

#[allow(clippy::too_many_arguments)]
fn spec(
    id: VbaCommandId,
    path: &'static [&'static str],
    use_text: &'static str,
    short: &'static str,
    target_object_kinds: &'static [&'static str],
    local_flags: Vec<FlagSpec>,
    execution: ExecutionSupport,
    flag_constraints: Option<Value>,
) -> CommandSpec {
    CommandSpec {
        id: CommandId::Vba(id),
        path,
        use_text,
        short,
        target_object_kinds,
        local_flags,
        execution,
        flag_constraints,
    }
}

fn flag(
    name: &'static str,
    arg_name: &'static str,
    flag_type: &'static str,
    description: &'static str,
) -> FlagSpec {
    FlagSpec {
        name,
        arg_name,
        flag_type,
        description,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::command_manifest::{
        assert_segment_matches_frozen_contract, capability_value, frozen_contract_commands,
    };

    const SERVE_MUTATION_PATHS: &[&str] = &[
        "ooxml vba create",
        "ooxml vba rebuild",
        "ooxml vba attach",
        "ooxml vba remove",
    ];

    #[test]
    fn complete_vba_segment_matches_frozen_tail_and_root_placement() {
        let specs = command_specs();
        let frozen = frozen_contract_commands();
        let root = crate::command_manifest::command_specs();
        let start = crate::command_manifest::core::command_specs().len()
            + crate::command_manifest::pptx::command_specs().len()
            + crate::command_manifest::xlsx::command_specs().len()
            + crate::command_manifest::docx::command_specs().len();
        assert_eq!(start, LEGACY_START);
        assert_eq!(specs.len(), COMMAND_COUNT);
        assert_segment_matches_frozen_contract(
            &specs,
            &frozen[LEGACY_START..LEGACY_START + COMMAND_COUNT],
        );
        assert_eq!(
            root[start..start + COMMAND_COUNT]
                .iter()
                .map(|spec| spec.id)
                .collect::<Vec<_>>(),
            specs.iter().map(|spec| spec.id).collect::<Vec<_>>()
        );
        assert_eq!(start + COMMAND_COUNT, 309);
    }

    #[test]
    fn vba_ids_paths_and_repeated_builds_are_unique_stable() {
        let first = command_specs();
        let second = command_specs();
        assert_eq!(
            first
                .iter()
                .map(|spec| spec.id)
                .collect::<BTreeSet<_>>()
                .len(),
            COMMAND_COUNT
        );
        assert_eq!(
            first
                .iter()
                .map(|spec| spec.path)
                .collect::<BTreeSet<_>>()
                .len(),
            COMMAND_COUNT
        );
        assert_eq!(
            first.iter().map(capability_value).collect::<Vec<_>>(),
            second.iter().map(capability_value).collect::<Vec<_>>()
        );
    }

    #[test]
    fn complete_vba_shadow_has_expected_execution_inventory_and_no_inspect() {
        let specs = command_specs();
        let inventory = specs.iter().fold(
            (0, 0, 0, 0),
            |(groups, direct, inspect, mutation), spec| match &spec.execution {
                ExecutionSupport::GroupOnly { .. } => (groups + 1, direct, inspect, mutation),
                ExecutionSupport::DirectOnly { .. } => (groups, direct + 1, inspect, mutation),
                ExecutionSupport::ServeInspect { .. } => (groups, direct, inspect + 1, mutation),
                ExecutionSupport::ServeMutation { .. } => (groups, direct, inspect, mutation + 1),
            },
        );
        assert_eq!(inventory, (1, 11, 0, 4));
    }

    #[test]
    fn vba_serve_mutations_match_frozen_and_live_dispatch_with_advisories() {
        let specs = command_specs();
        let frozen = frozen_contract_commands();
        let expected = frozen[LEGACY_START..LEGACY_START + COMMAND_COUNT]
            .iter()
            .filter(|command| command["opCompatible"] == true)
            .filter_map(|command| command["path"].as_str().map(str::to_owned))
            .collect::<BTreeSet<_>>();
        let actual = specs
            .iter()
            .filter(|spec| matches!(&spec.execution, ExecutionSupport::ServeMutation { .. }))
            .filter_map(|spec| capability_value(spec)["path"].as_str().map(str::to_owned))
            .collect::<BTreeSet<_>>();
        let dispatch_oracle = SERVE_MUTATION_PATHS
            .iter()
            .map(|path| (*path).to_owned())
            .collect::<BTreeSet<_>>();
        assert_eq!(actual, expected);
        assert_eq!(actual, dispatch_oracle);
        let advisories = specs
            .iter()
            .filter_map(|spec| match &spec.execution {
                ExecutionSupport::ServeMutation {
                    reason: Some(reason),
                } => Some((spec.path, *reason)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            advisories,
            vec![
                (
                    (&["vba", "create"] as &[&str]),
                    "preferred cross-platform macro authoring path; legacy Office-COM create remains available without --pure for XLSM/PPTM seeds"
                ),
                (
                    (&["vba", "rebuild"] as &[&str]),
                    "safe module-set replacement path; rebuilds a fresh source-only vbaProject.bin rather than patching Office-authored binary metadata"
                ),
            ]
        );
    }
}
