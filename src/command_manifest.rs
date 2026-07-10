use std::borrow::Cow;

use serde::Serialize;
use serde_json::Value;

mod core;
mod docx;
mod pptx;
mod vba;
mod xlsx;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum CommandId {
    Core(core::CoreCommandId),
    Pptx(pptx::PptxCommandId),
    Xlsx(xlsx::XlsxCommandId),
    Docx(docx::DocxCommandId),
    Vba(vba::VbaCommandId),
}

struct CommandSpec {
    id: CommandId,
    path: &'static [&'static str],
    use_text: &'static str,
    short: &'static str,
    target_object_kinds: &'static [&'static str],
    local_flags: Vec<FlagSpec>,
    execution: ExecutionSupport,
    flag_constraints: Option<Value>,
}

struct FlagSpec {
    name: &'static str,
    arg_name: &'static str,
    flag_type: &'static str,
    description: &'static str,
}

enum ExecutionSupport {
    DirectOnly { reason: Option<&'static str> },
    ServeInspect { reason: Option<&'static str> },
    ServeMutation { reason: Option<&'static str> },
    GroupOnly { reason: Option<&'static str> },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CapabilityCommandDto<'a> {
    path: Cow<'a, str>,
    #[serde(rename = "use")]
    use_text: &'a str,
    short: &'a str,
    target_object_kinds: &'a [&'a str],
    local_flags: Vec<CapabilityFlagDto<'a>>,
    op_compatible: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    op_ineligible_reason: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    flag_constraints: Option<&'a Value>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CapabilityFlagDto<'a> {
    name: &'a str,
    arg_name: &'a str,
    #[serde(rename = "type")]
    flag_type: &'a str,
    description: &'a str,
}

fn command_specs() -> Vec<CommandSpec> {
    let mut specs = Vec::new();
    specs.extend(core::command_specs());
    specs.extend(pptx::command_specs());
    specs.extend(xlsx::command_specs());
    specs.extend(docx::command_specs());
    specs.extend(vba::command_specs());
    specs
}

fn capability_value(spec: &CommandSpec) -> Value {
    capability_value_from_parts(
        spec.path,
        spec.use_text,
        spec.short,
        spec.target_object_kinds,
        &spec.local_flags,
        &spec.execution,
        spec.flag_constraints.as_ref(),
    )
}

#[allow(clippy::too_many_arguments)]
fn capability_value_from_parts<'a>(
    path: &'a [&'a str],
    use_text: &'a str,
    short: &'a str,
    target_object_kinds: &'a [&'a str],
    local_flags: &'a [FlagSpec],
    execution: &'a ExecutionSupport,
    flag_constraints: Option<&'a Value>,
) -> Value {
    let (op_compatible, op_ineligible_reason) = match execution {
        ExecutionSupport::ServeMutation { reason } => (true, *reason),
        ExecutionSupport::DirectOnly { reason }
        | ExecutionSupport::ServeInspect { reason }
        | ExecutionSupport::GroupOnly { reason } => (false, *reason),
    };
    let dto = CapabilityCommandDto {
        path: Cow::Owned(format!("ooxml {}", path.join(" "))),
        use_text,
        short,
        target_object_kinds,
        local_flags: local_flags
            .iter()
            .map(|flag| CapabilityFlagDto {
                name: flag.name,
                arg_name: flag.arg_name,
                flag_type: flag.flag_type,
                description: flag.description,
            })
            .collect(),
        op_compatible,
        op_ineligible_reason,
        flag_constraints,
    };
    serde_json::to_value(&dto).expect("serialize capability command DTO")
}

#[cfg(test)]
fn assert_segment_matches_legacy(specs: &[CommandSpec], legacy: &[Value]) {
    assert_eq!(specs.len(), legacy.len(), "manifest segment length");
    for (index, (spec, expected)) in specs.iter().zip(legacy).enumerate() {
        let actual = capability_value(spec);
        assert_eq!(actual, *expected, "manifest segment value at index {index}");
        assert_eq!(
            serde_json::to_string(&actual).expect("serialize shadow capability value"),
            serde_json::to_string(expected).expect("serialize legacy capability value"),
            "manifest segment JSON at index {index}"
        );
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use serde_json::json;

    use super::*;

    #[test]
    fn core_segment_matches_legacy_and_leads_root_aggregation() {
        let core = core::command_specs();
        let root = command_specs();
        let legacy = crate::capabilities::capability_commands();

        assert_eq!(core.len(), 36);
        assert_eq!(
            root[..core.len()]
                .iter()
                .map(|spec| spec.id)
                .collect::<Vec<_>>(),
            core.iter().map(|spec| spec.id).collect::<Vec<_>>()
        );
        assert_segment_matches_legacy(&core, &legacy[..core.len()]);
    }

    #[test]
    fn pptx_group_segment_matches_legacy_after_core() {
        let core_len = core::command_specs().len();
        let pptx = pptx::command_specs();
        let groups = &pptx[..pptx::GROUP_COMMAND_COUNT];
        let root = command_specs();
        let legacy = crate::capabilities::capability_commands();
        let root_pptx_end = core_len + pptx.len();
        let group_end = core_len + pptx::GROUP_COMMAND_COUNT;

        assert_eq!(groups.len(), 20);
        assert_eq!(
            root[core_len..root_pptx_end]
                .iter()
                .map(|spec| spec.id)
                .collect::<Vec<_>>(),
            pptx.iter().map(|spec| spec.id).collect::<Vec<_>>()
        );
        assert_segment_matches_legacy(groups, &legacy[core_len..group_end]);
    }

    #[test]
    fn pptx_group_ids_paths_and_repeated_builds_are_unique_stable_groups() {
        let first = pptx::command_specs();
        let second = pptx::command_specs();
        let first_groups = &first[..pptx::GROUP_COMMAND_COUNT];
        let second_groups = &second[..pptx::GROUP_COMMAND_COUNT];
        assert_eq!(first_groups.len(), 20);
        assert_eq!(
            first_groups
                .iter()
                .map(|spec| spec.id)
                .collect::<BTreeSet<_>>()
                .len(),
            first_groups.len()
        );
        assert_eq!(
            first_groups
                .iter()
                .map(|spec| spec.path)
                .collect::<BTreeSet<_>>()
                .len(),
            first_groups.len()
        );
        assert!(first_groups.iter().all(|spec| matches!(
            &spec.execution,
            ExecutionSupport::GroupOnly {
                reason: Some("it is a command group, not a leaf mutation command")
            }
        )));
        assert_eq!(
            first_groups
                .iter()
                .map(capability_value)
                .collect::<Vec<_>>(),
            second_groups
                .iter()
                .map(capability_value)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn current_pptx_shadow_matches_legacy_and_is_unique_stable() {
        let core_len = core::command_specs().len();
        let first = pptx::command_specs();
        let second = pptx::command_specs();
        let legacy = crate::capabilities::capability_commands();
        let end = core_len + first.len();

        assert_segment_matches_legacy(&first, &legacy[core_len..end]);
        assert_eq!(
            first
                .iter()
                .map(|spec| spec.id)
                .collect::<BTreeSet<_>>()
                .len(),
            first.len()
        );
        assert_eq!(
            first
                .iter()
                .map(|spec| spec.path)
                .collect::<BTreeSet<_>>()
                .len(),
            first.len()
        );
        assert_eq!(
            first.iter().map(capability_value).collect::<Vec<_>>(),
            second.iter().map(capability_value).collect::<Vec<_>>()
        );
    }

    #[test]
    fn complete_pptx_shadow_has_expected_execution_inventory() {
        let specs = pptx::command_specs();
        assert_eq!(specs.len(), 108);
        let inventory = specs.iter().fold(
            (0, 0, 0, 0),
            |(groups, direct, inspect, mutation), spec| match &spec.execution {
                ExecutionSupport::GroupOnly { .. } => (groups + 1, direct, inspect, mutation),
                ExecutionSupport::DirectOnly { .. } => (groups, direct + 1, inspect, mutation),
                ExecutionSupport::ServeInspect { .. } => (groups, direct, inspect + 1, mutation),
                ExecutionSupport::ServeMutation { .. } => (groups, direct, inspect, mutation + 1),
            },
        );
        assert_eq!(inventory, (20, 64, 13, 11));
        assert_eq!(
            specs
                .iter()
                .filter(|spec| match &spec.execution {
                    ExecutionSupport::ServeInspect {
                        reason: Some(reason),
                    } => reason.contains("call via inspect in serve/MCP"),
                    _ => false,
                })
                .count(),
            8
        );
    }

    #[test]
    fn xlsx_root_owned_segments_match_their_noncontiguous_legacy_offsets() {
        let legacy = crate::capabilities::capability_commands();
        let xlsx_start = core::command_specs().len() + pptx::command_specs().len();
        let front = xlsx::front_command_specs();
        let forms = xlsx::forms_command_specs();

        assert_eq!(xlsx_start, 144);
        assert_eq!(front.len(), xlsx::FRONT_COMMAND_COUNT);
        assert_eq!(xlsx::FRONT_COMMAND_COUNT, 22);
        assert_segment_matches_legacy(&front, &legacy[xlsx_start..166]);
        assert_eq!(forms.len(), 1);
        assert_segment_matches_legacy(&forms, &legacy[218..219]);
    }

    #[test]
    fn xlsx_root_owned_ids_paths_and_repeated_builds_are_unique_stable() {
        let first = xlsx::front_command_specs()
            .into_iter()
            .chain(xlsx::forms_command_specs())
            .collect::<Vec<_>>();
        let second = xlsx::front_command_specs()
            .into_iter()
            .chain(xlsx::forms_command_specs())
            .collect::<Vec<_>>();

        assert_eq!(first.len(), xlsx::ROOT_OWNED_COMMAND_COUNT);
        assert_eq!(xlsx::ROOT_OWNED_COMMAND_COUNT, 23);
        assert_eq!(
            first
                .iter()
                .map(|spec| spec.id)
                .collect::<BTreeSet<_>>()
                .len(),
            first.len()
        );
        assert_eq!(
            first
                .iter()
                .map(|spec| spec.path)
                .collect::<BTreeSet<_>>()
                .len(),
            first.len()
        );
        assert_eq!(
            first.iter().map(capability_value).collect::<Vec<_>>(),
            second.iter().map(capability_value).collect::<Vec<_>>()
        );
        assert_eq!(
            first
                .iter()
                .filter(|spec| matches!(&spec.execution, ExecutionSupport::GroupOnly { .. }))
                .count(),
            21
        );
        assert!(
            first[..xlsx::GROUP_COMMAND_COUNT]
                .iter()
                .all(|spec| matches!(
                    &spec.execution,
                    ExecutionSupport::GroupOnly {
                        reason: Some("it is a command group, not a leaf mutation command")
                    }
                ))
        );
        assert!(
            first[xlsx::GROUP_COMMAND_COUNT..]
                .iter()
                .all(|spec| matches!(
                    &spec.execution,
                    ExecutionSupport::DirectOnly {
                        reason: Some("it creates a package and is not an apply/serve mutation op")
                    }
                ))
        );
    }

    #[test]
    fn root_aggregation_appends_current_xlsx_builder_after_core_and_pptx() {
        let core_len = core::command_specs().len();
        let pptx_len = pptx::command_specs().len();
        let xlsx = xlsx::command_specs();
        let root = command_specs();
        let start = core_len + pptx_len;
        let end = start + xlsx.len();

        assert_eq!(
            root[start..end]
                .iter()
                .map(|spec| spec.id)
                .collect::<Vec<_>>(),
            xlsx.iter().map(|spec| spec.id).collect::<Vec<_>>()
        );
    }

    #[test]
    fn current_xlsx_shadow_is_a_contiguous_legacy_prefix_and_unique_stable() {
        let start = core::command_specs().len() + pptx::command_specs().len();
        let first = xlsx::command_specs();
        let second = xlsx::command_specs();
        let legacy = crate::capabilities::capability_commands();
        let end = start + first.len();

        assert_segment_matches_legacy(&first, &legacy[start..end]);
        assert_eq!(
            first
                .iter()
                .map(|spec| spec.id)
                .collect::<BTreeSet<_>>()
                .len(),
            first.len()
        );
        assert_eq!(
            first
                .iter()
                .map(|spec| spec.path)
                .collect::<BTreeSet<_>>()
                .len(),
            first.len()
        );
        assert_eq!(
            first.iter().map(capability_value).collect::<Vec<_>>(),
            second.iter().map(capability_value).collect::<Vec<_>>()
        );
    }

    #[test]
    fn complete_xlsx_shadow_has_expected_execution_inventory() {
        let specs = xlsx::command_specs();
        assert_eq!(specs.len(), 104);
        let inventory = specs.iter().fold(
            (0, 0, 0, 0),
            |(groups, direct, inspect, mutation), spec| match &spec.execution {
                ExecutionSupport::GroupOnly { .. } => (groups + 1, direct, inspect, mutation),
                ExecutionSupport::DirectOnly { .. } => (groups, direct + 1, inspect, mutation),
                ExecutionSupport::ServeInspect { .. } => (groups, direct, inspect + 1, mutation),
                ExecutionSupport::ServeMutation { .. } => (groups, direct, inspect, mutation + 1),
            },
        );
        assert_eq!(inventory, (21, 34, 17, 32));
        assert_eq!(
            specs
                .iter()
                .filter(|spec| match &spec.execution {
                    ExecutionSupport::ServeInspect {
                        reason: Some(reason),
                    } => reason.contains("call via inspect in serve/MCP"),
                    _ => false,
                })
                .count(),
            14
        );
    }

    #[test]
    fn xlsx_serve_inspect_classification_matches_independent_dispatch_oracle() {
        const SERVE_INSPECT_PATHS: &[&str] = &[
            "ooxml xlsx sheets list",
            "ooxml xlsx sheets show",
            "ooxml xlsx comments list",
            "ooxml xlsx conditional-formats list",
            "ooxml xlsx conditional-formats show",
            "ooxml xlsx hyperlinks list",
            "ooxml xlsx hyperlinks show",
            "ooxml xlsx filters-sorts show",
            "ooxml xlsx names list",
            "ooxml xlsx names show",
            "ooxml xlsx tables list",
            "ooxml xlsx tables show",
            "ooxml xlsx tables export",
            "ooxml xlsx workbook metadata inspect",
            "ooxml xlsx ranges export",
            "ooxml xlsx cells extract",
            "ooxml xlsx freeze show",
        ];
        let actual = xlsx::command_specs()
            .iter()
            .filter(|spec| matches!(&spec.execution, ExecutionSupport::ServeInspect { .. }))
            .filter_map(|spec| capability_value(spec)["path"].as_str().map(str::to_owned))
            .collect::<BTreeSet<_>>();
        let expected = SERVE_INSPECT_PATHS
            .iter()
            .map(|path| (*path).to_string())
            .collect::<BTreeSet<_>>();

        assert_eq!(actual, expected);
        assert_eq!(actual.len(), 17);
    }

    #[test]
    fn xlsx_serve_mutations_match_legacy_op_compatible_set() {
        let specs = xlsx::command_specs();
        let start = core::command_specs().len() + pptx::command_specs().len();
        let legacy = crate::capabilities::capability_commands();
        let expected = legacy[start..start + specs.len()]
            .iter()
            .filter(|command| command["opCompatible"] == true)
            .filter_map(|command| command["path"].as_str().map(str::to_owned))
            .collect::<BTreeSet<_>>();
        let actual = specs
            .iter()
            .filter(|spec| matches!(&spec.execution, ExecutionSupport::ServeMutation { .. }))
            .filter_map(|spec| capability_value(spec)["path"].as_str().map(str::to_owned))
            .collect::<BTreeSet<_>>();

        assert_eq!(actual, expected);
        assert_eq!(actual.len(), 32);
    }

    #[test]
    fn core_ids_paths_and_repeated_builds_are_unique_and_stable() {
        let first = core::command_specs();
        let second = core::command_specs();
        assert_eq!(first.len(), 36);
        assert_eq!(
            first
                .iter()
                .map(|spec| spec.id)
                .collect::<BTreeSet<_>>()
                .len(),
            first.len()
        );
        assert_eq!(
            first
                .iter()
                .map(|spec| spec.path)
                .collect::<BTreeSet<_>>()
                .len(),
            first.len()
        );
        assert_eq!(
            first.iter().map(capability_value).collect::<Vec<_>>(),
            second.iter().map(capability_value).collect::<Vec<_>>()
        );
    }

    #[test]
    fn core_execution_support_has_two_mutations_and_only_semantic_groups() {
        let specs = core::command_specs();
        let mutations = specs
            .iter()
            .filter_map(|spec| match &spec.execution {
                ExecutionSupport::ServeMutation { reason } => Some((spec.id, *reason)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            mutations,
            vec![
                (
                    CommandId::Core(core::CoreCommandId::RepairNormalize),
                    Some(
                        "narrow repair command; run validate/conformance after normalization before handing the file to a user"
                    )
                ),
                (CommandId::Core(core::CoreCommandId::TemplateApply), None),
            ]
        );
        assert_eq!(
            specs
                .iter()
                .filter_map(|spec| match &spec.execution {
                    ExecutionSupport::GroupOnly { .. } => Some(spec.id),
                    _ => None,
                })
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([
                CommandId::Core(core::CoreCommandId::Completion),
                CommandId::Core(core::CoreCommandId::Conformance),
                CommandId::Core(core::CoreCommandId::Template),
                CommandId::Core(core::CoreCommandId::TemplateProfile),
            ])
        );
        assert!(
            specs
                .iter()
                .all(|spec| !matches!(&spec.execution, ExecutionSupport::ServeInspect { .. }))
        );
    }

    #[test]
    fn capability_dto_uses_exact_wire_keys_and_omits_absent_optionals() {
        let actual = capability_value_from_parts(
            &["version"],
            "version",
            "Print the version.",
            &[],
            &[],
            &ExecutionSupport::DirectOnly { reason: None },
            None,
        );
        assert_eq!(
            actual,
            json!({
                "path": "ooxml version",
                "use": "version",
                "short": "Print the version.",
                "targetObjectKinds": [],
                "localFlags": [],
                "opCompatible": false
            })
        );
        assert!(actual.get("opIneligibleReason").is_none());
        assert!(actual.get("flagConstraints").is_none());
    }

    #[test]
    fn capability_dto_preserves_flags_constraints_and_exact_reason() {
        let constraint = json!({"requiresAny": ["--out", "--in-place"]});
        let actual = capability_value_from_parts(
            &["xlsx", "cells", "set"],
            "xlsx cells set <file>",
            "Set a cell.",
            &["cell"],
            &[FlagSpec {
                name: "--value",
                arg_name: "value",
                flag_type: "string",
                description: "replacement value",
            }],
            &ExecutionSupport::DirectOnly {
                reason: Some("exact legacy reason"),
            },
            Some(&constraint),
        );
        assert_eq!(actual["path"], "ooxml xlsx cells set");
        assert_eq!(actual["opCompatible"], false);
        assert_eq!(actual["opIneligibleReason"], "exact legacy reason");
        assert_eq!(actual["flagConstraints"], constraint);
        assert_eq!(
            actual["localFlags"],
            json!([{
                "name": "--value",
                "argName": "value",
                "type": "string",
                "description": "replacement value"
            }])
        );
    }

    #[test]
    fn only_serve_mutation_is_operation_compatible() {
        for execution in [
            ExecutionSupport::DirectOnly { reason: None },
            ExecutionSupport::ServeInspect { reason: None },
            ExecutionSupport::GroupOnly { reason: None },
        ] {
            let value = capability_value_from_parts(
                &["test-only"],
                "test-only",
                "Test-only DTO input.",
                &[],
                &[],
                &execution,
                None,
            );
            assert_eq!(value["opCompatible"], false);
        }
        let mutation_without_reason = capability_value_from_parts(
            &["test-only"],
            "test-only",
            "Test-only DTO input.",
            &[],
            &[],
            &ExecutionSupport::ServeMutation { reason: None },
            None,
        );
        assert_eq!(mutation_without_reason["opCompatible"], true);
        assert!(mutation_without_reason.get("opIneligibleReason").is_none());

        let mutation_with_reason = capability_value_from_parts(
            &["test-only"],
            "test-only",
            "Test-only DTO input.",
            &[],
            &[],
            &ExecutionSupport::ServeMutation {
                reason: Some("exact mutation advisory"),
            },
            None,
        );
        assert_eq!(mutation_with_reason["opCompatible"], true);
        assert_eq!(
            mutation_with_reason["opIneligibleReason"],
            "exact mutation advisory"
        );
    }
}
