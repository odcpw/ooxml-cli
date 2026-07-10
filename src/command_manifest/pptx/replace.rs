use super::{PptxCommandId, direct, flag, mutation, spec};

pub(super) const COMMAND_COUNT: usize = 5;

pub(super) fn command_specs() -> Vec<super::CommandSpec> {
    vec![
        spec(
            PptxCommandId::ReplaceText,
            &["pptx", "replace", "text"],
            "text <file>",
            "Replace text in the supported slide target.",
            &["slide", "shape"],
            vec![
                flag("--slide", "slide", "int", "1-based slide number"),
                flag(
                    "--target",
                    "target",
                    "string",
                    "shape selector, title for the frozen slice",
                ),
                flag("--text", "text", "string", "replacement text"),
                flag(
                    "--out",
                    "out",
                    "string",
                    "output file path for direct CLI use",
                ),
            ],
            mutation(None),
            None,
        ),
        spec(
            PptxCommandId::ReplaceTextOccurrences,
            &["pptx", "replace", "text-occurrences"],
            "text-occurrences <file>",
            "Replace matching slide-visible text occurrences across a deck.",
            &["slide", "shape"],
            with_output_flags(vec![
                flag("--match-text", "matchText", "string", "text to find"),
                flag("--new-text", "newText", "string", "replacement text"),
                flag(
                    "--new-text-file",
                    "newTextFile",
                    "string",
                    "file containing replacement text",
                ),
                flag(
                    "--for-slides",
                    "forSlides",
                    "string",
                    "optional slide list/range or slide handle scope",
                ),
                flag(
                    "--for-shape",
                    "forShape",
                    "string",
                    "optional stable shape handle scope",
                ),
                flag(
                    "--ignore-case",
                    "ignoreCase",
                    "bool",
                    "match text case-insensitively",
                ),
                flag(
                    "--expect-count",
                    "expectCount",
                    "int",
                    "stale guard for planned replacement count",
                ),
                flag(
                    "--expect-plan-hash",
                    "expectPlanHash",
                    "string",
                    "stale guard hash from dry-run",
                ),
                flag(
                    "--allow-zero",
                    "allowZero",
                    "bool",
                    "allow a saved no-op when no matches are found",
                ),
            ]),
            mutation(None),
            None,
        ),
        spec(
            PptxCommandId::ReplaceTextFromXlsx,
            &["pptx", "replace", "text-from-xlsx"],
            "text-from-xlsx <file>",
            "Replace one PPTX text target with text joined from an XLSX range.",
            &["slide", "shape", "sheet", "range"],
            with_output_flags(vec![
                flag("--slide", "slide", "int", "1-based destination slide"),
                flag(
                    "--target",
                    "target",
                    "string",
                    "destination text shape selector",
                ),
                flag("--workbook", "workbook", "string", "source XLSX workbook"),
                flag("--sheet", "sheet", "string", "source sheet selector"),
                flag("--range", "range", "string", "source A1 range"),
                flag(
                    "--max-cells",
                    "maxCells",
                    "int",
                    "maximum source cells to read, 0 for unlimited",
                ),
                flag(
                    "--formula-mode",
                    "formulaMode",
                    "string",
                    "source formula handling: value or formula",
                ),
                flag(
                    "--mode",
                    "mode",
                    "string",
                    "replacement mode: plain-text or preserve-format",
                ),
                flag(
                    "--row-sep",
                    "rowSep",
                    "string",
                    "separator between source rows",
                ),
                flag(
                    "--col-sep",
                    "colSep",
                    "string",
                    "separator between source columns",
                ),
            ]),
            direct("direct CLI mutation; serve/MCP op support is not wired yet"),
            None,
        ),
        spec(
            PptxCommandId::ReplaceTextMapFromXlsx,
            &["pptx", "replace", "text-map-from-xlsx"],
            "text-map-from-xlsx <file>",
            "Apply a row-oriented XLSX text replacement map to PPTX text targets.",
            &["slide", "shape", "sheet", "range"],
            with_output_flags(vec![
                flag("--workbook", "workbook", "string", "source XLSX workbook"),
                flag("--sheet", "sheet", "string", "source sheet selector"),
                flag("--range", "range", "string", "source A1 range"),
                flag("--table", "table", "string", "source XLSX table selector"),
                flag(
                    "--max-cells",
                    "maxCells",
                    "int",
                    "maximum source cells to read, 0 for unlimited",
                ),
                flag(
                    "--formula-mode",
                    "formulaMode",
                    "string",
                    "source formula handling: value or formula",
                ),
                flag(
                    "--mode",
                    "mode",
                    "string",
                    "replacement mode: plain-text or preserve-format",
                ),
                flag(
                    "--slide-col",
                    "slideCol",
                    "string",
                    "header name or 1-based column index for slide numbers",
                ),
                flag(
                    "--target-col",
                    "targetCol",
                    "string",
                    "header name or 1-based column index for target selectors",
                ),
                flag(
                    "--text-col",
                    "textCol",
                    "string",
                    "header name or 1-based column index for replacement text",
                ),
                flag(
                    "--expect-source-range",
                    "expectSourceRange",
                    "string",
                    "stale guard for resolved XLSX source range",
                ),
            ]),
            direct("direct CLI mutation; serve/MCP op support is not wired yet"),
            None,
        ),
        spec(
            PptxCommandId::ReplaceImages,
            &["pptx", "replace", "images"],
            "images <file>",
            "Replace a picture shape with a new image file.",
            &["slide", "shape", "image"],
            with_output_flags(vec![
                flag(
                    "--target",
                    "target",
                    "string",
                    "picture selector such as shape:4 or ~Picture 3",
                ),
                flag("--image", "image", "string", "replacement image file"),
                flag(
                    "--fit-mode",
                    "fitMode",
                    "string",
                    "contain/fit or cover/crop",
                ),
                flag("--slide", "slide", "int", "optional 1-based slide number"),
                flag(
                    "--for-slides",
                    "forSlides",
                    "string",
                    "batch slide scope is deferred in the Rust port",
                ),
            ]),
            direct("direct CLI mutation; serve/MCP op support is not wired yet"),
            None,
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
                .filter(|spec| matches!(&spec.execution, ExecutionSupport::ServeMutation { .. }))
                .count(),
            2
        );
        assert_eq!(
            specs
                .iter()
                .filter(|spec| matches!(&spec.execution, ExecutionSupport::DirectOnly { .. }))
                .count(),
            3
        );
    }
}
