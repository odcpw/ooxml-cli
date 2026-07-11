use super::{ExecutionSupport, XlsxCommandId, flag, spec};

pub(super) const COMMAND_COUNT: usize = 5;
pub(super) const LEGACY_START: usize = 196;

pub(super) fn command_specs() -> Vec<super::CommandSpec> {
    vec![
        spec(
            XlsxCommandId::ConditionalFormatsList,
            &["xlsx", "conditional-formats", "list"],
            "list <file> [--sheet <sheet>] [--range <sqref>]",
            "List worksheet conditional-formatting rules.",
            &["conditional-format", "range", "sheet"],
            vec![
                flag("--sheet", "sheet", "string", "sheet selector"),
                flag("--range", "range", "string", "optional sqref filter"),
            ],
            ExecutionSupport::ServeInspect {
                reason: Some("read-only command; call via inspect in serve/MCP"),
            },
            None,
        ),
        spec(
            XlsxCommandId::ConditionalFormatsShow,
            &["xlsx", "conditional-formats", "show"],
            "show <file> --rule <selector> [--sheet <sheet>]",
            "Show one worksheet conditional-formatting rule.",
            &["conditional-format", "range", "sheet"],
            vec![
                flag("--sheet", "sheet", "string", "sheet selector"),
                flag(
                    "--rule",
                    "rule",
                    "string",
                    "rule selector such as cfRule:1, rule:1, block:1/rule:1, priority:1, or sqref:A1:A5",
                ),
            ],
            ExecutionSupport::ServeInspect {
                reason: Some("read-only command; call via inspect in serve/MCP"),
            },
            None,
        ),
        spec(
            XlsxCommandId::ConditionalFormatsAdd,
            &["xlsx", "conditional-formats", "add"],
            "add <file> --range <sqref> [--type expression|cell-is|color-scale|data-bar|icon-set]",
            "Add an expression, cellIs, color-scale, data-bar, or icon-set conditional-formatting rule.",
            &["conditional-format", "range", "sheet", "style"],
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
                    "conditional-formatting rule type: expression, cell-is, color-scale, data-bar, or icon-set",
                ),
                flag(
                    "--operator",
                    "operator",
                    "string",
                    "cellIs operator: between, notBetween, equal, notEqual, greaterThan, lessThan, greaterThanOrEqual, or lessThanOrEqual",
                ),
                flag(
                    "--formula",
                    "formula",
                    "string",
                    "expression formula or first cellIs formula/bound",
                ),
                flag(
                    "--formula2",
                    "formula2",
                    "string",
                    "second cellIs formula/bound for between/notBetween",
                ),
                flag(
                    "--cfvo",
                    "cfvo",
                    "string",
                    "threshold value: repeat 2 or 3 times for color-scale, exactly 2 times for data-bar, or 3/4/5 times for icon-set based on --icon-set; examples: min, max, num:0, percent:10, or percentile:50",
                ),
                flag(
                    "--color",
                    "color",
                    "string",
                    "color hex: repeat 2 or 3 times for color-scale, exactly once for data-bar, and never for icon-set; examples: #F8696B, FFEB84, or FF63BE7B",
                ),
                flag(
                    "--icon-set",
                    "iconSet",
                    "string",
                    "icon-set name for --type icon-set; examples: 3TrafficLights1, 4Arrows, or 5Rating",
                ),
                flag("--priority", "priority", "int", "optional cfRule priority"),
                flag(
                    "--stop-if-true",
                    "stopIfTrue",
                    "bool",
                    "set stopIfTrue on the rule",
                ),
                flag(
                    "--dxf-id",
                    "dxfId",
                    "int",
                    "optional differential style id to reference",
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
                "defaultMode": "expression",
                "modeFlag": "--type",
                "modes": [
                    {
                        "forbidden": [
                            "--operator",
                            "--formula2",
                            "--cfvo",
                            "--color",
                            "--icon-set"
                        ],
                        "required": [
                            "--range",
                            "--formula"
                        ],
                        "value": "expression"
                    },
                    {
                        "notes": [
                            "--formula2 is only used for between/notBetween operators."
                        ],
                        "optional": [
                            "--operator",
                            "--formula2",
                            "--dxf-id",
                            "--stop-if-true"
                        ],
                        "required": [
                            "--range",
                            "--formula"
                        ],
                        "value": "cell-is"
                    },
                    {
                        "forbidden": [
                            "--formula",
                            "--formula2",
                            "--operator",
                            "--icon-set",
                            "--dxf-id",
                            "--stop-if-true"
                        ],
                        "repeat": {
                            "--cfvo": "2 or 3",
                            "--color": "2 or 3"
                        },
                        "required": [
                            "--range",
                            "--cfvo",
                            "--color"
                        ],
                        "value": "color-scale"
                    },
                    {
                        "forbidden": [
                            "--formula",
                            "--formula2",
                            "--operator",
                            "--icon-set",
                            "--dxf-id",
                            "--stop-if-true"
                        ],
                        "repeat": {
                            "--cfvo": "exactly 2",
                            "--color": "exactly 1"
                        },
                        "required": [
                            "--range",
                            "--cfvo",
                            "--color"
                        ],
                        "value": "data-bar"
                    },
                    {
                        "forbidden": [
                            "--formula",
                            "--formula2",
                            "--operator",
                            "--color",
                            "--dxf-id",
                            "--stop-if-true"
                        ],
                        "repeat": {
                            "--cfvo": "3, 4, or 5 based on --icon-set"
                        },
                        "required": [
                            "--range",
                            "--icon-set",
                            "--cfvo"
                        ],
                        "value": "icon-set"
                    }
                ],
                "rules": [
                    "Use --dry-run first when composing a conditional-format rule.",
                    "For existing rules, discover selectors with ooxml --json xlsx conditional-formats list <file> --sheet <sheet>."
                ]
            })),
        ),
        spec(
            XlsxCommandId::ConditionalFormatsDelete,
            &["xlsx", "conditional-formats", "delete"],
            "delete <file> --rule <selector>",
            "Delete a worksheet conditional-formatting rule.",
            &["conditional-format", "range", "sheet"],
            vec![
                flag("--sheet", "sheet", "string", "sheet selector"),
                flag(
                    "--rule",
                    "rule",
                    "string",
                    "rule selector such as cfRule:1, rule:1, block:1/rule:1, priority:1, or sqref:A1:A5",
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
            XlsxCommandId::ConditionalFormatsReorder,
            &["xlsx", "conditional-formats", "reorder"],
            "reorder <file> --sheet <selector> --rule <selector> --priority <n>",
            "Change a conditional-formatting rule priority; if selection fails, list rules first and retry with cfRule:<n> or priority:<n>.",
            &["conditional-format", "range", "sheet"],
            vec![
                flag("--sheet", "sheet", "string", "sheet selector"),
                flag(
                    "--rule",
                    "rule",
                    "string",
                    "rule selector such as cfRule:1, rule:1, block:1/rule:1, priority:1, or sqref:A1:A5",
                ),
                flag("--priority", "priority", "int", "new cfRule priority"),
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
    use crate::command_manifest::{
        assert_segment_matches_frozen_contract, capability_value, frozen_contract_commands,
    };

    #[test]
    fn conditional_formatting_segment_matches_frozen_contract_slice() {
        let specs = command_specs();
        let frozen = frozen_contract_commands();
        assert_eq!(specs.len(), COMMAND_COUNT);
        assert_segment_matches_frozen_contract(
            &specs,
            &frozen[LEGACY_START..LEGACY_START + COMMAND_COUNT],
        );
    }

    #[test]
    fn conditional_formatting_ids_paths_builds_and_execution_inventory_are_stable() {
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
        assert_eq!(inventory, (0, 0, 2, 3));
    }
}
