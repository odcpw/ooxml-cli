use serde_json::{Value, json};

use super::super::{capability_command, capability_command_with_flag_constraints, flag};
use super::xlsx_data_validation_mutation_flags;

pub(super) fn commands() -> Vec<Value> {
    vec![
        capability_command(
            "ooxml xlsx data-validations list",
            "list <file> [--sheet <sheet>]",
            "List worksheet data-validation rules and their target ranges.",
            &["data-validation", "range", "sheet"],
            false,
            Some("read-only command; call via direct CLI in the current Rust slice"),
            vec![flag("--sheet", "sheet", "string", "sheet selector")],
        ),
        capability_command(
            "ooxml xlsx data-validations show",
            "show <file> --range <sqref> [--sheet <sheet>]",
            "Show the validation rule that targets a specific worksheet range.",
            &["data-validation", "range", "sheet"],
            false,
            Some("read-only command; call via direct CLI in the current Rust slice"),
            vec![
                flag("--sheet", "sheet", "string", "sheet selector"),
                flag("--range", "range", "string", "target sqref such as A2:A20"),
            ],
        ),
        {
            capability_command_with_flag_constraints(
                "ooxml xlsx data-validations create",
                "create <file> --range <sqref> --type <type>",
                "Create a worksheet data-validation rule such as a dropdown list or numeric constraint.",
                &["data-validation", "range", "sheet"],
                true,
                None,
                xlsx_data_validation_mutation_flags(false),
                Some(json!({
                    "modeFlag": "--type",
                    "modes": [
                        {
                            "value": "list",
                            "required": ["--range", "--type"],
                            "oneOf": ["--list-values", "--list-range"],
                            "forbidden": ["--operator", "--formula1", "--formula2"],
                            "notes": ["Inline list values are comma-separated; --list-range uses a worksheet range formula source."]
                        },
                        {
                            "value": "whole",
                            "required": ["--range", "--type", "--formula1"],
                            "optional": ["--operator", "--formula2"],
                            "notes": ["--formula2 is required when --operator is between or notBetween."]
                        },
                        {
                            "value": "decimal",
                            "required": ["--range", "--type", "--formula1"],
                            "optional": ["--operator", "--formula2"],
                            "notes": ["--formula2 is required when --operator is between or notBetween."]
                        },
                        {
                            "value": "date",
                            "required": ["--range", "--type", "--formula1"],
                            "optional": ["--operator", "--formula2"],
                            "notes": ["--formula2 is required when --operator is between or notBetween."]
                        },
                        {
                            "value": "time",
                            "required": ["--range", "--type", "--formula1"],
                            "optional": ["--operator", "--formula2"],
                            "notes": ["--formula2 is required when --operator is between or notBetween."]
                        },
                        {
                            "value": "textLength",
                            "aliases": ["text-length", "textlength"],
                            "required": ["--range", "--type", "--formula1"],
                            "optional": ["--operator", "--formula2"],
                            "notes": ["--formula2 is required when --operator is between or notBetween."]
                        },
                        {
                            "value": "custom",
                            "required": ["--range", "--type", "--formula1"],
                            "forbidden": ["--operator", "--formula2", "--list-values", "--list-range"]
                        }
                    ],
                    "operatorValues": ["between", "notBetween", "equal", "notEqual", "greaterThan", "lessThan", "greaterThanOrEqual", "lessThanOrEqual"],
                    "errorStyleValues": ["stop", "warning", "information"],
                    "outputRequiredOneOf": ["--out", "--in-place", "--dry-run"],
                    "rules": [
                        "--type is required for create.",
                        "list requires exactly one list source: --list-values or --list-range.",
                        "non-list validation types require --formula1.",
                        "between and notBetween require --formula2.",
                        "operator is not valid for list or custom validations."
                    ]
                })),
            )
        },
        capability_command(
            "ooxml xlsx data-validations update",
            "update <file> --range <sqref>",
            "Update an existing worksheet data-validation rule with optional type/formula guards.",
            &["data-validation", "range", "sheet"],
            true,
            None,
            xlsx_data_validation_mutation_flags(true),
        ),
        capability_command(
            "ooxml xlsx data-validations delete",
            "delete <file> --range <sqref>",
            "Delete a worksheet data-validation rule by target range with optional type/formula guards.",
            &["data-validation", "range", "sheet"],
            true,
            None,
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
        ),
    ]
}
