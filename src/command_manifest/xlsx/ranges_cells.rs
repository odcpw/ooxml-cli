use super::{ExecutionSupport, XlsxCommandId, flag, spec};

pub(super) const COMMAND_COUNT: usize = 8;
pub(super) const LEGACY_START: usize = 237;

pub(super) fn command_specs() -> Vec<super::CommandSpec> {
    vec![
        spec(
            XlsxCommandId::RangesExport,
            &["xlsx", "ranges", "export"],
            "export <file>",
            "Export decoded worksheet cells from a range.",
            &["sheet", "range"],
            vec![
                flag("--sheet", "sheet", "string", "sheet selector"),
                flag("--range", "range", "string", "A1 range"),
                flag(
                    "--include-types",
                    "includeTypes",
                    "bool",
                    "include cell types",
                ),
                flag(
                    "--include-formulas",
                    "includeFormulas",
                    "bool",
                    "include formulas",
                ),
                flag(
                    "--include-formats",
                    "includeFormats",
                    "bool",
                    "include style and number-format matrices",
                ),
                flag(
                    "--data-out",
                    "dataOut",
                    "string",
                    "write JSON matrix data to this file",
                ),
                flag(
                    "--max-cells",
                    "maxCells",
                    "number",
                    "maximum cells to export",
                ),
            ],
            ExecutionSupport::ServeInspect {
                reason: Some("read-only command; call via inspect in serve/MCP"),
            },
            None,
        ),
        spec(
            XlsxCommandId::RangesSet,
            &["xlsx", "ranges", "set"],
            "set <file>",
            "Set a worksheet range from a rectangular JSON, CSV, or TSV matrix.",
            &["sheet", "range", "cell"],
            vec![
                flag("--sheet", "sheet", "string", "sheet selector"),
                flag("--range", "range", "string", "A1 range"),
                flag("--anchor", "anchor", "string", "top-left A1 cell"),
                flag("--values", "values", "string", "inline matrix data"),
                flag(
                    "--values-file",
                    "valuesFile",
                    "string",
                    "path to matrix data, or - for stdin",
                ),
                flag(
                    "--data-format",
                    "dataFormat",
                    "string",
                    "matrix data format: json, csv, or tsv",
                ),
                flag(
                    "--null-policy",
                    "nullPolicy",
                    "string",
                    "null handling: skip, clear, or empty-string",
                ),
                flag(
                    "--ragged",
                    "ragged",
                    "string",
                    "ragged handling: reject or fill-empty",
                ),
                flag("--max-cells", "maxCells", "number", "maximum cells to set"),
                flag("--out", "out", "string", "output file path"),
                flag(
                    "--in-place",
                    "inPlace",
                    "bool",
                    "write the input file in place",
                ),
                flag("--backup", "backup", "string", "backup path for --in-place"),
                flag("--dry-run", "dryRun", "bool", "plan without writing"),
                flag(
                    "--overwrite-formulas",
                    "overwriteFormulas",
                    "bool",
                    "allow replacing existing formula cells",
                ),
                flag(
                    "--no-validate",
                    "noValidate",
                    "bool",
                    "accepted for CLI compatibility",
                ),
            ],
            ExecutionSupport::ServeMutation { reason: None },
            None,
        ),
        spec(
            XlsxCommandId::RangesSetFormat,
            &["xlsx", "ranges", "set-format"],
            "set-format <file>",
            "Apply a practical number format to a worksheet range.",
            &["sheet", "range", "style"],
            vec![
                flag("--sheet", "sheet", "string", "sheet selector"),
                flag("--range", "range", "string", "A1 range"),
                flag(
                    "--preset",
                    "preset",
                    "string",
                    "format preset: integer, number, currency, percent, date, datetime, text, or general",
                ),
                flag(
                    "--format-code",
                    "formatCode",
                    "string",
                    "custom SpreadsheetML number format code",
                ),
                flag(
                    "--decimals",
                    "decimals",
                    "number",
                    "decimal places for number, currency, and percent presets",
                ),
                flag(
                    "--currency-symbol",
                    "currencySymbol",
                    "string",
                    "currency literal for the currency preset",
                ),
                flag(
                    "--max-cells",
                    "maxCells",
                    "number",
                    "maximum cells to format",
                ),
                flag("--out", "out", "string", "output file path"),
                flag(
                    "--in-place",
                    "inPlace",
                    "bool",
                    "write the input file in place",
                ),
                flag("--backup", "backup", "string", "backup path for --in-place"),
                flag("--dry-run", "dryRun", "bool", "plan without writing"),
                flag(
                    "--no-validate",
                    "noValidate",
                    "bool",
                    "accepted for CLI compatibility",
                ),
            ],
            ExecutionSupport::ServeMutation { reason: None },
            None,
        ),
        spec(
            XlsxCommandId::RangesSetStyle,
            &["xlsx", "ranges", "set-style"],
            "set-style <file>",
            "Apply font, fill, border, and alignment styling to a worksheet range.",
            &["sheet", "range", "style"],
            vec![
                flag("--sheet", "sheet", "string", "sheet selector"),
                flag("--range", "range", "string", "A1 range"),
                flag("--font-name", "fontName", "string", "font family name"),
                flag("--font-size", "fontSize", "number", "font size in points"),
                flag("--font-bold", "fontBold", "bool", "toggle bold text"),
                flag("--font-italic", "fontItalic", "bool", "toggle italic text"),
                flag(
                    "--font-underline",
                    "fontUnderline",
                    "bool",
                    "toggle underline text",
                ),
                flag(
                    "--font-color",
                    "fontColor",
                    "string",
                    "font color as hex RGB",
                ),
                flag(
                    "--fill-color",
                    "fillColor",
                    "string",
                    "cell fill color as hex RGB",
                ),
                flag(
                    "--border-style",
                    "borderStyle",
                    "string",
                    "border line style",
                ),
                flag(
                    "--border-color",
                    "borderColor",
                    "string",
                    "border color as hex RGB",
                ),
                flag("--border-top", "borderTop", "bool", "apply top border"),
                flag(
                    "--border-bottom",
                    "borderBottom",
                    "bool",
                    "apply bottom border",
                ),
                flag("--border-left", "borderLeft", "bool", "apply left border"),
                flag(
                    "--border-right",
                    "borderRight",
                    "bool",
                    "apply right border",
                ),
                flag(
                    "--alignment-horizontal",
                    "alignmentHorizontal",
                    "string",
                    "horizontal alignment",
                ),
                flag(
                    "--alignment-vertical",
                    "alignmentVertical",
                    "string",
                    "vertical alignment",
                ),
                flag(
                    "--alignment-wrap-text",
                    "alignmentWrapText",
                    "bool",
                    "toggle text wrapping",
                ),
                flag(
                    "--max-cells",
                    "maxCells",
                    "number",
                    "maximum cells to style",
                ),
                flag("--out", "out", "string", "output file path"),
                flag(
                    "--in-place",
                    "inPlace",
                    "bool",
                    "write the input file in place",
                ),
                flag("--backup", "backup", "string", "backup path for --in-place"),
                flag("--dry-run", "dryRun", "bool", "plan without writing"),
                flag(
                    "--no-validate",
                    "noValidate",
                    "bool",
                    "accepted for CLI compatibility",
                ),
            ],
            ExecutionSupport::DirectOnly {
                reason: Some(
                    "direct CLI mutation; serve/MCP operation dispatch is not wired for this command",
                ),
            },
            None,
        ),
        spec(
            XlsxCommandId::CellsExtract,
            &["xlsx", "cells", "extract"],
            "extract <file>",
            "Extract decoded worksheet cells with stable cell selectors.",
            &["sheet", "range", "cell"],
            vec![
                flag("--sheet", "sheet", "string", "sheet selector"),
                flag("--range", "range", "string", "A1 range"),
                flag("--max-rows", "maxRows", "number", "maximum rows to emit"),
                flag("--max-cells", "maxCells", "number", "maximum cells to emit"),
                flag(
                    "--include-empty",
                    "includeEmpty",
                    "bool",
                    "include empty cells inside output bounds",
                ),
            ],
            ExecutionSupport::ServeInspect {
                reason: Some("read-only command; call via inspect in serve/MCP"),
            },
            None,
        ),
        spec(
            XlsxCommandId::CellsSet,
            &["xlsx", "cells", "set"],
            "set <file>",
            "Set a worksheet cell value.",
            &["sheet", "cell"],
            vec![
                flag("--backup", "backup", "string", "backup path"),
                flag("--cell", "cell", "string", "A1 cell reference"),
                flag("--dry-run", "dryRun", "bool", "plan without writing"),
                flag("--formula", "formula", "string", "cell formula"),
                flag("--in-place", "inPlace", "bool", "write in place"),
                flag(
                    "--no-validate",
                    "noValidate",
                    "bool",
                    "skip post-write validation",
                ),
                flag(
                    "--out",
                    "out",
                    "string",
                    "output file path for direct CLI use",
                ),
                flag("--ref", "ref", "string", "A1 cell reference alias"),
                flag("--sheet", "sheet", "string", "sheet selector"),
                flag("--type", "type", "string", "value type"),
                flag("--value", "value", "string", "cell value"),
            ],
            ExecutionSupport::ServeMutation { reason: None },
            None,
        ),
        spec(
            XlsxCommandId::CellsClear,
            &["xlsx", "cells", "clear"],
            "clear <file>",
            "Clear values and formulas from a worksheet cell or range.",
            &["sheet", "range", "cell"],
            vec![
                flag("--sheet", "sheet", "string", "sheet selector"),
                flag("--range", "range", "string", "A1 range"),
                flag("--ref", "ref", "string", "A1 cell reference"),
                flag(
                    "--readback-max-cells",
                    "readbackMaxCells",
                    "number",
                    "maximum cells in emitted destination readback",
                ),
                flag("--out", "out", "string", "output file path"),
                flag(
                    "--in-place",
                    "inPlace",
                    "bool",
                    "write the input file in place",
                ),
                flag("--backup", "backup", "string", "backup path for --in-place"),
                flag("--dry-run", "dryRun", "bool", "plan without writing"),
                flag(
                    "--no-validate",
                    "noValidate",
                    "bool",
                    "accepted for CLI compatibility",
                ),
            ],
            ExecutionSupport::DirectOnly {
                reason: Some(
                    "direct CLI mutation; serve/MCP operation dispatch is not wired for this command",
                ),
            },
            None,
        ),
        spec(
            XlsxCommandId::CellsSetBatch,
            &["xlsx", "cells", "set-batch"],
            "set-batch <file>",
            "Set multiple worksheet cells from inline JSON or a JSON file.",
            &["sheet", "range", "cell"],
            vec![
                flag("--sheet", "sheet", "string", "sheet selector"),
                flag("--cells", "cells", "string", "inline JSON cell assignments"),
                flag(
                    "--cells-file",
                    "cellsFile",
                    "string",
                    "path to JSON cell assignments, or - for stdin",
                ),
                flag(
                    "--details",
                    "details",
                    "bool",
                    "emit per-cell mutation details",
                ),
                flag(
                    "--readback-max-cells",
                    "readbackMaxCells",
                    "number",
                    "maximum cells in emitted destination readback",
                ),
                flag("--out", "out", "string", "output file path"),
                flag(
                    "--in-place",
                    "inPlace",
                    "bool",
                    "write the input file in place",
                ),
                flag("--backup", "backup", "string", "backup path for --in-place"),
                flag("--dry-run", "dryRun", "bool", "plan without writing"),
                flag(
                    "--no-validate",
                    "noValidate",
                    "bool",
                    "accepted for CLI compatibility",
                ),
            ],
            ExecutionSupport::DirectOnly {
                reason: Some(
                    "direct CLI mutation; serve/MCP operation dispatch is not wired for this command",
                ),
            },
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
    fn ranges_cells_segment_matches_frozen_contract_slice() {
        let specs = command_specs();
        let frozen = frozen_contract_commands();
        assert_eq!(specs.len(), COMMAND_COUNT);
        assert_segment_matches_frozen_contract(
            &specs,
            &frozen[LEGACY_START..LEGACY_START + COMMAND_COUNT],
        );
    }

    #[test]
    fn ranges_cells_ids_paths_builds_and_execution_inventory_are_stable() {
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
        assert_eq!(inventory, (0, 3, 2, 3));
    }
}
