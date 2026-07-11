use super::{ExecutionSupport, XlsxCommandId, flag, spec};

pub(super) const COMMAND_COUNT: usize = 14;
pub(super) const LEGACY_START: usize = 166;

pub(super) fn command_specs() -> Vec<super::CommandSpec> {
    vec![
        spec(
            XlsxCommandId::SheetsList,
            &["xlsx", "sheets", "list"],
            "list <file>",
            "List workbook sheets and selectors.",
            &["sheet", "range"],
            vec![],
            ExecutionSupport::ServeInspect {
                reason: Some("read-only command; call via inspect in serve/MCP"),
            },
            None,
        ),
        spec(
            XlsxCommandId::SheetsShow,
            &["xlsx", "sheets", "show"],
            "show <file>",
            "Show worksheet metadata, used ranges, and generated readback commands.",
            &["sheet", "range"],
            vec![flag("--sheet", "sheet", "string", "sheet selector")],
            ExecutionSupport::ServeInspect {
                reason: Some("read-only command; call via inspect in serve/MCP"),
            },
            None,
        ),
        spec(
            XlsxCommandId::SheetsAdd,
            &["xlsx", "sheets", "add"],
            "add <file> --name <name>",
            "Add an empty worksheet and wire workbook.xml, workbook relationships, and content types.",
            &["sheet"],
            vec![
                flag("--name", "name", "string", "new worksheet name"),
                flag(
                    "--after",
                    "after",
                    "string",
                    "insert after sheet selector; omitted appends",
                ),
                flag("--out", "out", "string", "write edited workbook"),
                flag(
                    "--in-place",
                    "inPlace",
                    "bool",
                    "edit the workbook in place",
                ),
                flag("--backup", "backup", "string", "backup path for --in-place"),
                flag(
                    "--dry-run",
                    "dryRun",
                    "bool",
                    "validate mutation without writing",
                ),
                flag(
                    "--no-validate",
                    "noValidate",
                    "bool",
                    "skip strict validation",
                ),
            ],
            ExecutionSupport::ServeMutation { reason: None },
            None,
        ),
        spec(
            XlsxCommandId::SheetsRename,
            &["xlsx", "sheets", "rename"],
            "rename <file> --sheet <sheet> --name <name>",
            "Rename a workbook worksheet by sheet selector.",
            &["sheet"],
            vec![
                flag("--sheet", "sheet", "string", "sheet selector"),
                flag("--name", "name", "string", "new worksheet name"),
                flag("--out", "out", "string", "write edited workbook"),
                flag(
                    "--in-place",
                    "inPlace",
                    "bool",
                    "edit the workbook in place",
                ),
                flag("--backup", "backup", "string", "backup path for --in-place"),
                flag(
                    "--dry-run",
                    "dryRun",
                    "bool",
                    "validate mutation without writing",
                ),
                flag(
                    "--no-validate",
                    "noValidate",
                    "bool",
                    "skip strict validation",
                ),
            ],
            ExecutionSupport::ServeMutation { reason: None },
            None,
        ),
        spec(
            XlsxCommandId::SheetsMove,
            &["xlsx", "sheets", "move"],
            "move <file> --sheet <sheet> (--to <n>|--before <sheet>|--after <sheet>)",
            "Move a workbook sheet in tab order while preserving its part, sheetId, and relationship ID.",
            &["sheet"],
            vec![
                flag("--sheet", "sheet", "string", "sheet selector"),
                flag("--to", "to", "int", "final 1-based workbook sheet position"),
                flag(
                    "--before",
                    "before",
                    "string",
                    "move before this sheet selector",
                ),
                flag(
                    "--after",
                    "after",
                    "string",
                    "move after this sheet selector",
                ),
                flag("--out", "out", "string", "write edited workbook"),
                flag(
                    "--in-place",
                    "inPlace",
                    "bool",
                    "edit the workbook in place",
                ),
                flag("--backup", "backup", "string", "backup path for --in-place"),
                flag(
                    "--dry-run",
                    "dryRun",
                    "bool",
                    "validate mutation without writing",
                ),
                flag(
                    "--no-validate",
                    "noValidate",
                    "bool",
                    "skip strict validation",
                ),
            ],
            ExecutionSupport::ServeMutation { reason: None },
            None,
        ),
        spec(
            XlsxCommandId::SheetsDelete,
            &["xlsx", "sheets", "delete"],
            "delete <file> --sheet <sheet>",
            "Delete one worksheet and remove its workbook relationship, worksheet part, worksheet relationships, and calcChain.",
            &["sheet"],
            vec![
                flag("--sheet", "sheet", "string", "sheet selector"),
                flag("--out", "out", "string", "write edited workbook"),
                flag(
                    "--in-place",
                    "inPlace",
                    "bool",
                    "edit the workbook in place",
                ),
                flag("--backup", "backup", "string", "backup path for --in-place"),
                flag(
                    "--dry-run",
                    "dryRun",
                    "bool",
                    "validate mutation without writing",
                ),
                flag(
                    "--no-validate",
                    "noValidate",
                    "bool",
                    "skip strict validation",
                ),
            ],
            ExecutionSupport::ServeMutation { reason: None },
            None,
        ),
        spec(
            XlsxCommandId::ColwidthsShow,
            &["xlsx", "colwidths", "show"],
            "show <file> --sheet <sheet> --range <columns>",
            "Show resolved worksheet column widths for a column range.",
            &["sheet", "range"],
            vec![
                flag("--sheet", "sheet", "string", "sheet selector"),
                flag("--range", "range", "string", "column span such as B or B:D"),
            ],
            ExecutionSupport::DirectOnly {
                reason: Some("read-only command; direct CLI parity only in the current Rust slice"),
            },
            None,
        ),
        spec(
            XlsxCommandId::ColwidthsSet,
            &["xlsx", "colwidths", "set"],
            "set <file> --sheet <sheet> --range <columns> --width <width>",
            "Set a uniform worksheet column width for a column range.",
            &["sheet", "range"],
            vec![
                flag("--sheet", "sheet", "string", "sheet selector"),
                flag("--range", "range", "string", "column span such as B or B:D"),
                flag(
                    "--width",
                    "width",
                    "number",
                    "column width in character units (0-255)",
                ),
                flag(
                    "--expect-width",
                    "expectWidth",
                    "number",
                    "guard: require the first column to currently have this width",
                ),
                flag("--out", "out", "string", "output file path"),
                flag("--in-place", "inPlace", "bool", "write in place"),
                flag("--backup", "backup", "string", "backup path for --in-place"),
                flag("--dry-run", "dryRun", "bool", "plan without writing"),
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
            XlsxCommandId::RowheightsShow,
            &["xlsx", "rowheights", "show"],
            "show <file> --sheet <sheet> --range <rows>",
            "Show resolved worksheet row heights for a row range.",
            &["sheet", "range"],
            vec![
                flag("--sheet", "sheet", "string", "sheet selector"),
                flag("--range", "range", "string", "row span such as 2 or 2:5"),
            ],
            ExecutionSupport::DirectOnly {
                reason: Some("read-only command; direct CLI parity only in the current Rust slice"),
            },
            None,
        ),
        spec(
            XlsxCommandId::RowheightsSet,
            &["xlsx", "rowheights", "set"],
            "set <file> --sheet <sheet> --range <rows> --height <height>",
            "Set a uniform worksheet row height for a row range.",
            &["sheet", "range"],
            vec![
                flag("--sheet", "sheet", "string", "sheet selector"),
                flag("--range", "range", "string", "row span such as 2 or 2:5"),
                flag(
                    "--height",
                    "height",
                    "number",
                    "row height in points (0-409)",
                ),
                flag(
                    "--expect-height",
                    "expectHeight",
                    "number",
                    "guard: require the first row to currently have this height",
                ),
                flag("--out", "out", "string", "output file path"),
                flag("--in-place", "inPlace", "bool", "write in place"),
                flag("--backup", "backup", "string", "backup path for --in-place"),
                flag("--dry-run", "dryRun", "bool", "plan without writing"),
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
            XlsxCommandId::RowsInsert,
            &["xlsx", "rows", "insert"],
            "insert <file> --sheet <sheet> --at <row>",
            "Insert blank worksheet rows before a 1-based row position.",
            &["sheet", "range"],
            vec![
                flag("--sheet", "sheet", "string", "sheet selector"),
                flag(
                    "--at",
                    "at",
                    "int",
                    "1-based row position for the inserted rows",
                ),
                flag("--count", "count", "int", "number of rows to insert"),
                flag("--out", "out", "string", "output file path"),
                flag("--in-place", "inPlace", "bool", "write in place"),
                flag("--backup", "backup", "string", "backup path for --in-place"),
                flag("--dry-run", "dryRun", "bool", "plan without writing"),
                flag(
                    "--no-validate",
                    "noValidate",
                    "bool",
                    "skip post-write validation",
                ),
            ],
            ExecutionSupport::DirectOnly { reason: None },
            None,
        ),
        spec(
            XlsxCommandId::RowsDelete,
            &["xlsx", "rows", "delete"],
            "delete <file> --sheet <sheet> --row <row>",
            "Delete a band of worksheet rows.",
            &["sheet", "range"],
            vec![
                flag("--sheet", "sheet", "string", "sheet selector"),
                flag("--row", "row", "int", "1-based first row to delete"),
                flag("--count", "count", "int", "number of rows to delete"),
                flag("--out", "out", "string", "output file path"),
                flag("--in-place", "inPlace", "bool", "write in place"),
                flag("--backup", "backup", "string", "backup path for --in-place"),
                flag("--dry-run", "dryRun", "bool", "plan without writing"),
                flag(
                    "--no-validate",
                    "noValidate",
                    "bool",
                    "skip post-write validation",
                ),
            ],
            ExecutionSupport::DirectOnly { reason: None },
            None,
        ),
        spec(
            XlsxCommandId::ColsInsert,
            &["xlsx", "cols", "insert"],
            "insert <file> --sheet <sheet> --at <column>",
            "Insert blank worksheet columns before a column position.",
            &["sheet", "range"],
            vec![
                flag("--sheet", "sheet", "string", "sheet selector"),
                flag(
                    "--at",
                    "at",
                    "string",
                    "column position for the inserted columns",
                ),
                flag("--count", "count", "int", "number of columns to insert"),
                flag("--out", "out", "string", "output file path"),
                flag("--in-place", "inPlace", "bool", "write in place"),
                flag("--backup", "backup", "string", "backup path for --in-place"),
                flag("--dry-run", "dryRun", "bool", "plan without writing"),
                flag(
                    "--no-validate",
                    "noValidate",
                    "bool",
                    "skip post-write validation",
                ),
            ],
            ExecutionSupport::DirectOnly { reason: None },
            None,
        ),
        spec(
            XlsxCommandId::ColsDelete,
            &["xlsx", "cols", "delete"],
            "delete <file> --sheet <sheet> --col <column>",
            "Delete a band of worksheet columns.",
            &["sheet", "range"],
            vec![
                flag("--sheet", "sheet", "string", "sheet selector"),
                flag("--col", "col", "string", "first column to delete"),
                flag("--count", "count", "int", "number of columns to delete"),
                flag("--out", "out", "string", "output file path"),
                flag("--in-place", "inPlace", "bool", "write in place"),
                flag("--backup", "backup", "string", "backup path for --in-place"),
                flag("--dry-run", "dryRun", "bool", "plan without writing"),
                flag(
                    "--no-validate",
                    "noValidate",
                    "bool",
                    "skip post-write validation",
                ),
            ],
            ExecutionSupport::DirectOnly { reason: None },
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
    fn structure_segment_matches_fixed_legacy_slice() {
        let specs = command_specs();
        let legacy = crate::capabilities::legacy_capability_commands();
        assert_eq!(specs.len(), COMMAND_COUNT);
        assert_segment_matches_legacy(&specs, &legacy[LEGACY_START..LEGACY_START + COMMAND_COUNT]);
    }

    #[test]
    fn structure_ids_paths_builds_and_execution_inventory_are_stable() {
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
        assert_eq!(inventory, (0, 6, 2, 6));
    }
}
