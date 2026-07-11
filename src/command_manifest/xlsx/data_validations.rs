use super::{ExecutionSupport, XlsxCommandId, flag, spec};

pub(super) const COMMAND_COUNT: usize = 5;
pub(super) const LEGACY_START: usize = 201;

pub(super) fn command_specs() -> Vec<super::CommandSpec> {
    vec![
        spec(
            XlsxCommandId::DataValidationsList,
            &["xlsx", "data-validations", "list"],
            "list <file> [--sheet <sheet>]",
            "List worksheet data-validation rules and their target ranges.",
            &["data-validation", "range", "sheet"],
            vec![flag("--sheet", "sheet", "string", "sheet selector")],
            ExecutionSupport::DirectOnly {
                reason: Some("read-only command; call via direct CLI in the current Rust slice"),
            },
            None,
        ),
        spec(
            XlsxCommandId::DataValidationsShow,
            &["xlsx", "data-validations", "show"],
            "show <file> --range <sqref> [--sheet <sheet>]",
            "Show the validation rule that targets a specific worksheet range.",
            &["data-validation", "range", "sheet"],
            vec![
                flag("--sheet", "sheet", "string", "sheet selector"),
                flag("--range", "range", "string", "target sqref such as A2:A20"),
            ],
            ExecutionSupport::DirectOnly {
                reason: Some("read-only command; call via direct CLI in the current Rust slice"),
            },
            None,
        ),
        spec(
            XlsxCommandId::DataValidationsCreate,
            &["xlsx", "data-validations", "create"],
            "create <file> --range <sqref> --type <type>",
            "Create a worksheet data-validation rule such as a dropdown list or numeric constraint.",
            &["data-validation", "range", "sheet"],
            vec![
                flag("--sheet", "sheet", "string", "sheet selector"),
                flag(
                    "--range",
                    "range",
                    "string",
                    "target sqref; space-separated ranges are accepted",
                ),
                flag(
                    "--type",
                    "type",
                    "string",
                    "validation type: list, whole, decimal, date, time, textLength, or custom",
                ),
                flag(
                    "--list-values",
                    "listValues",
                    "string",
                    "comma-separated inline values for list validations",
                ),
                flag(
                    "--list-range",
                    "listRange",
                    "string",
                    "worksheet range source for list validations",
                ),
                flag(
                    "--operator",
                    "operator",
                    "string",
                    "operator such as between, equal, greaterThan, or lessThanOrEqual",
                ),
                flag("--formula1", "formula1", "string", "first formula or bound"),
                flag(
                    "--formula2",
                    "formula2",
                    "string",
                    "second formula or bound for between/notBetween",
                ),
                flag("--allow-blank", "allowBlank", "bool", "allow blank cells"),
                flag(
                    "--show-input-message",
                    "showInputMessage",
                    "bool",
                    "show the input prompt",
                ),
                flag(
                    "--input-title",
                    "inputTitle",
                    "string",
                    "input prompt title",
                ),
                flag(
                    "--input-message",
                    "inputMessage",
                    "string",
                    "input prompt message",
                ),
                flag(
                    "--show-error-message",
                    "showErrorMessage",
                    "bool",
                    "show the error alert",
                ),
                flag("--error-title", "errorTitle", "string", "error alert title"),
                flag(
                    "--error-message",
                    "errorMessage",
                    "string",
                    "error alert message",
                ),
                flag(
                    "--error-style",
                    "errorStyle",
                    "string",
                    "error alert style: stop, warning, or information",
                ),
                flag("--out", "out", "string", "write edited workbook"),
                flag(
                    "--in-place",
                    "inPlace",
                    "bool",
                    "edit the workbook in place",
                ),
                flag(
                    "--backup",
                    "backup",
                    "string",
                    "backup file path for --in-place",
                ),
                flag("--dry-run", "dryRun", "bool", "validate without writing"),
                flag(
                    "--no-validate",
                    "noValidate",
                    "bool",
                    "skip post-write validation",
                ),
            ],
            ExecutionSupport::ServeMutation { reason: None },
            Some(serde_json::json!({
                "errorStyleValues": [
                    "stop",
                    "warning",
                    "information"
                ],
                "modeFlag": "--type",
                "modes": [
                    {
                        "forbidden": [
                            "--operator",
                            "--formula1",
                            "--formula2"
                        ],
                        "notes": [
                            "Inline list values are comma-separated; --list-range uses a worksheet range formula source."
                        ],
                        "oneOf": [
                            "--list-values",
                            "--list-range"
                        ],
                        "required": [
                            "--range",
                            "--type"
                        ],
                        "value": "list"
                    },
                    {
                        "notes": [
                            "--formula2 is required when --operator is between or notBetween."
                        ],
                        "optional": [
                            "--operator",
                            "--formula2"
                        ],
                        "required": [
                            "--range",
                            "--type",
                            "--formula1"
                        ],
                        "value": "whole"
                    },
                    {
                        "notes": [
                            "--formula2 is required when --operator is between or notBetween."
                        ],
                        "optional": [
                            "--operator",
                            "--formula2"
                        ],
                        "required": [
                            "--range",
                            "--type",
                            "--formula1"
                        ],
                        "value": "decimal"
                    },
                    {
                        "notes": [
                            "--formula2 is required when --operator is between or notBetween."
                        ],
                        "optional": [
                            "--operator",
                            "--formula2"
                        ],
                        "required": [
                            "--range",
                            "--type",
                            "--formula1"
                        ],
                        "value": "date"
                    },
                    {
                        "notes": [
                            "--formula2 is required when --operator is between or notBetween."
                        ],
                        "optional": [
                            "--operator",
                            "--formula2"
                        ],
                        "required": [
                            "--range",
                            "--type",
                            "--formula1"
                        ],
                        "value": "time"
                    },
                    {
                        "aliases": [
                            "text-length",
                            "textlength"
                        ],
                        "notes": [
                            "--formula2 is required when --operator is between or notBetween."
                        ],
                        "optional": [
                            "--operator",
                            "--formula2"
                        ],
                        "required": [
                            "--range",
                            "--type",
                            "--formula1"
                        ],
                        "value": "textLength"
                    },
                    {
                        "forbidden": [
                            "--operator",
                            "--formula2",
                            "--list-values",
                            "--list-range"
                        ],
                        "required": [
                            "--range",
                            "--type",
                            "--formula1"
                        ],
                        "value": "custom"
                    }
                ],
                "operatorValues": [
                    "between",
                    "notBetween",
                    "equal",
                    "notEqual",
                    "greaterThan",
                    "lessThan",
                    "greaterThanOrEqual",
                    "lessThanOrEqual"
                ],
                "outputRequiredOneOf": [
                    "--out",
                    "--in-place",
                    "--dry-run"
                ],
                "rules": [
                    "--type is required for create.",
                    "list requires exactly one list source: --list-values or --list-range.",
                    "non-list validation types require --formula1.",
                    "between and notBetween require --formula2.",
                    "operator is not valid for list or custom validations."
                ]
            })),
        ),
        spec(
            XlsxCommandId::DataValidationsUpdate,
            &["xlsx", "data-validations", "update"],
            "update <file> --range <sqref>",
            "Update an existing worksheet data-validation rule with optional type/formula guards.",
            &["data-validation", "range", "sheet"],
            vec![
                flag("--sheet", "sheet", "string", "sheet selector"),
                flag(
                    "--range",
                    "range",
                    "string",
                    "target sqref; space-separated ranges are accepted",
                ),
                flag(
                    "--type",
                    "type",
                    "string",
                    "validation type: list, whole, decimal, date, time, textLength, or custom",
                ),
                flag(
                    "--list-values",
                    "listValues",
                    "string",
                    "comma-separated inline values for list validations",
                ),
                flag(
                    "--list-range",
                    "listRange",
                    "string",
                    "worksheet range source for list validations",
                ),
                flag(
                    "--operator",
                    "operator",
                    "string",
                    "operator such as between, equal, greaterThan, or lessThanOrEqual",
                ),
                flag("--formula1", "formula1", "string", "first formula or bound"),
                flag(
                    "--formula2",
                    "formula2",
                    "string",
                    "second formula or bound for between/notBetween",
                ),
                flag("--allow-blank", "allowBlank", "bool", "allow blank cells"),
                flag(
                    "--show-input-message",
                    "showInputMessage",
                    "bool",
                    "show the input prompt",
                ),
                flag(
                    "--input-title",
                    "inputTitle",
                    "string",
                    "input prompt title",
                ),
                flag(
                    "--input-message",
                    "inputMessage",
                    "string",
                    "input prompt message",
                ),
                flag(
                    "--show-error-message",
                    "showErrorMessage",
                    "bool",
                    "show the error alert",
                ),
                flag("--error-title", "errorTitle", "string", "error alert title"),
                flag(
                    "--error-message",
                    "errorMessage",
                    "string",
                    "error alert message",
                ),
                flag(
                    "--error-style",
                    "errorStyle",
                    "string",
                    "error alert style: stop, warning, or information",
                ),
                flag(
                    "--expect-type",
                    "expectType",
                    "string",
                    "guard: require the current validation type to match",
                ),
                flag(
                    "--expect-formula1",
                    "expectFormula1",
                    "string",
                    "guard: require the current formula1 to match",
                ),
                flag("--out", "out", "string", "write edited workbook"),
                flag(
                    "--in-place",
                    "inPlace",
                    "bool",
                    "edit the workbook in place",
                ),
                flag(
                    "--backup",
                    "backup",
                    "string",
                    "backup file path for --in-place",
                ),
                flag("--dry-run", "dryRun", "bool", "validate without writing"),
                flag(
                    "--no-validate",
                    "noValidate",
                    "bool",
                    "skip post-write validation",
                ),
            ],
            ExecutionSupport::ServeMutation { reason: None },
            None,
        ),
        spec(
            XlsxCommandId::DataValidationsDelete,
            &["xlsx", "data-validations", "delete"],
            "delete <file> --range <sqref>",
            "Delete a worksheet data-validation rule by target range with optional type/formula guards.",
            &["data-validation", "range", "sheet"],
            vec![
                flag("--sheet", "sheet", "string", "sheet selector"),
                flag("--range", "range", "string", "target sqref such as A2:A20"),
                flag(
                    "--expect-type",
                    "expectType",
                    "string",
                    "guard: require the current validation type to match",
                ),
                flag(
                    "--expect-formula1",
                    "expectFormula1",
                    "string",
                    "guard: require the current formula1 to match",
                ),
                flag("--out", "out", "string", "write edited workbook"),
                flag(
                    "--in-place",
                    "inPlace",
                    "bool",
                    "edit the workbook in place",
                ),
                flag(
                    "--backup",
                    "backup",
                    "string",
                    "backup file path for --in-place",
                ),
                flag("--dry-run", "dryRun", "bool", "validate without writing"),
                flag(
                    "--no-validate",
                    "noValidate",
                    "bool",
                    "skip post-write validation",
                ),
            ],
            ExecutionSupport::ServeMutation { reason: None },
            None,
        ),
    ]
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::command_manifest::{assert_segment_matches_legacy, capability_value};

    #[test]
    fn data_validations_segment_matches_fixed_legacy_slice() {
        let specs = command_specs();
        let legacy = crate::capabilities::legacy_capability_commands();
        assert_eq!(specs.len(), COMMAND_COUNT);
        assert_segment_matches_legacy(&specs, &legacy[LEGACY_START..LEGACY_START + COMMAND_COUNT]);
    }

    #[test]
    fn data_validations_ids_paths_builds_and_execution_inventory_are_stable() {
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
        let inventory = first.iter().fold(
            (0, 0, 0, 0),
            |(groups, direct, inspect, mutation), spec| match &spec.execution {
                ExecutionSupport::GroupOnly { .. } => (groups + 1, direct, inspect, mutation),
                ExecutionSupport::DirectOnly { .. } => (groups, direct + 1, inspect, mutation),
                ExecutionSupport::ServeInspect { .. } => (groups, direct, inspect + 1, mutation),
                ExecutionSupport::ServeMutation { .. } => (groups, direct, inspect, mutation + 1),
            },
        );
        assert_eq!(inventory, (0, 2, 0, 3));
    }
}
