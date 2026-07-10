use super::{PptxCommandId, flag, inspect, mutation, spec};

pub(super) const COMMAND_COUNT: usize = 7;

pub(super) fn command_specs() -> Vec<super::CommandSpec> {
    vec![
        spec(
            PptxCommandId::TablesShow,
            &["pptx", "tables", "show"],
            "show <file> --slide <n>",
            "Show table graphic frames and cell text for one slide.",
            &["slide", "table"],
            vec![
                flag("--slide", "slide", "int", "1-based slide number"),
                flag(
                    "--table-id",
                    "tableId",
                    "int",
                    "optional table shape ID to show",
                ),
                flag(
                    "--target",
                    "target",
                    "string",
                    "optional table selector such as table:1, shape:2, or ~Table 1",
                ),
                flag(
                    "--details",
                    "details",
                    "bool",
                    "include enriched row, column, cell, and span details",
                ),
            ],
            inspect("read-only command; call via inspect in serve/MCP"),
            None,
        ),
        spec(
            PptxCommandId::TablesSetCell,
            &["pptx", "tables", "set-cell"],
            "set-cell <file> --slide <n> (--table-id <id>|--target <selector>) --row <n> --col <n> --text <text>",
            "Set plain text in one PowerPoint table cell.",
            &["slide", "table"],
            with_output_flags({
                let mut flags = table_target_flags();
                flags.extend([
                    flag("--row", "row", "int", "1-based table row"),
                    flag("--col", "col", "int", "1-based table column"),
                    flag(
                        "--text",
                        "text",
                        "string",
                        "replacement cell text; empty string clears the cell",
                    ),
                    flag(
                        "--text-file",
                        "textFile",
                        "string",
                        "path to replacement cell text",
                    ),
                ]);
                flags
            }),
            mutation(None),
            None,
        ),
        spec(
            PptxCommandId::TablesDeleteRow,
            &["pptx", "tables", "delete-row"],
            "delete-row <file> --slide <n> (--table-id <id>|--target <selector>) --row <n>",
            "Delete one row from a PowerPoint table.",
            &["slide", "table"],
            with_output_flags({
                let mut flags = table_target_flags();
                flags.push(flag("--row", "row", "int", "1-based table row to delete"));
                flags
            }),
            mutation(None),
            None,
        ),
        spec(
            PptxCommandId::TablesInsertRow,
            &["pptx", "tables", "insert-row"],
            "insert-row <file> --slide <n> (--table-id <id>|--target <selector>) --at <n>",
            "Insert an empty row into a PowerPoint table.",
            &["slide", "table"],
            with_output_flags({
                let mut flags = table_target_flags();
                flags.push(flag(
                    "--at",
                    "at",
                    "int",
                    "1-based row position for insertion; rows+1 appends",
                ));
                flags
            }),
            mutation(None),
            None,
        ),
        spec(
            PptxCommandId::TablesDeleteCol,
            &["pptx", "tables", "delete-col"],
            "delete-col <file> --slide <n> (--table-id <id>|--target <selector>) --col <n>",
            "Delete one column from a PowerPoint table.",
            &["slide", "table"],
            with_output_flags({
                let mut flags = table_target_flags();
                flags.push(flag(
                    "--col",
                    "col",
                    "int",
                    "1-based table column to delete",
                ));
                flags
            }),
            mutation(None),
            None,
        ),
        spec(
            PptxCommandId::TablesInsertCol,
            &["pptx", "tables", "insert-col"],
            "insert-col <file> --slide <n> (--table-id <id>|--target <selector>) --at <n>",
            "Insert an empty column into a PowerPoint table.",
            &["slide", "table"],
            with_output_flags({
                let mut flags = table_target_flags();
                flags.extend([
                    flag(
                        "--at",
                        "at",
                        "int",
                        "1-based column position for insertion; cols+1 appends",
                    ),
                    flag(
                        "--width-emu",
                        "widthEmu",
                        "int",
                        "inserted column width in EMUs; 0 uses existing average",
                    ),
                ]);
                flags
            }),
            mutation(None),
            None,
        ),
        spec(
            PptxCommandId::TablesUpdateFromXlsx,
            &["pptx", "tables", "update-from-xlsx"],
            "update-from-xlsx <file> --workbook <xlsx> (--sheet <sheet> --range <A1>|--table <selector>) --slide <n> (--table-id <id>|--target <selector>)",
            "Refresh plain text cell contents in an existing PowerPoint table from an XLSX range or table.",
            &["slide", "table", "sheet", "range"],
            with_output_flags({
                let mut flags = table_target_flags();
                flags.extend([
                    flag(
                        "--workbook",
                        "workbook",
                        "string",
                        "source XLSX workbook path",
                    ),
                    flag("--sheet", "sheet", "string", "source sheet selector"),
                    flag("--range", "range", "string", "source A1 range"),
                    flag(
                        "--table",
                        "table",
                        "string",
                        "source workbook table selector",
                    ),
                    flag(
                        "--max-cells",
                        "maxCells",
                        "int",
                        "maximum source cells to read; 0 for unlimited",
                    ),
                    flag(
                        "--formula-mode",
                        "formulaMode",
                        "string",
                        "formula handling: value or formula",
                    ),
                    flag(
                        "--expect-source-range",
                        "expectSourceRange",
                        "string",
                        "fail if resolved XLSX source range differs",
                    ),
                ]);
                flags
            }),
            mutation(None),
            None,
        ),
    ]
}

fn table_target_flags() -> Vec<super::FlagSpec> {
    vec![
        flag("--slide", "slide", "int", "1-based slide number"),
        flag(
            "--table-id",
            "tableId",
            "int",
            "table graphic-frame shape ID",
        ),
        flag(
            "--target",
            "target",
            "string",
            "table selector such as table:1, shape:2, or ~Table 1",
        ),
    ]
}

fn with_output_flags(mut flags: Vec<super::FlagSpec>) -> Vec<super::FlagSpec> {
    flags.extend([
        flag("--out", "out", "string", "output file path"),
        flag("--backup", "backup", "string", "backup path for --in-place"),
        flag(
            "--dry-run",
            "dryRun",
            "bool",
            "plan and validate without writing",
        ),
        flag(
            "--in-place",
            "inPlace",
            "bool",
            "write back to the input file",
        ),
        flag(
            "--no-validate",
            "noValidate",
            "bool",
            "skip strict validation of the mutated package",
        ),
    ]);
    flags
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command_manifest::ExecutionSupport;

    #[test]
    fn owner_contract() {
        let specs = command_specs();
        assert_eq!(specs.len(), COMMAND_COUNT);
        assert_eq!(
            specs
                .iter()
                .filter(|spec| matches!(&spec.execution, ExecutionSupport::ServeInspect { .. }))
                .count(),
            1
        );
        assert_eq!(
            specs
                .iter()
                .filter(|spec| matches!(&spec.execution, ExecutionSupport::ServeMutation { .. }))
                .count(),
            6
        );
    }
}
