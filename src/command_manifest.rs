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
    use serde_json::json;

    use super::*;

    #[test]
    fn empty_family_aggregation_is_deterministic_and_matches_empty_legacy_segment() {
        let first = command_specs();
        let second = command_specs();
        assert!(first.is_empty());
        assert!(second.is_empty());
        assert_segment_matches_legacy(&first, &[]);
        assert_segment_matches_legacy(&second, &[]);
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
