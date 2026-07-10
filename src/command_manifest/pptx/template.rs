use super::{PptxCommandId, direct, flag, spec};

pub(super) const COMMAND_COUNT: usize = 5;

pub(super) fn command_specs() -> Vec<super::CommandSpec> {
    vec![
        spec(
            PptxCommandId::TemplateInspect,
            &["pptx", "template", "inspect"],
            "inspect <manifest-file>",
            "Inspect a captured template manifest.",
            &["template"],
            vec![flag(
                "--format",
                "format",
                "string",
                "Output format: 'text' or 'json'",
            )],
            direct("read-only command; call the compile leaf for generation"),
            None,
        ),
        spec(
            PptxCommandId::TemplateCapture,
            &["pptx", "template", "capture"],
            "capture <file> [--name <name>] [--slides <n,n>]",
            "Capture a PPTX/POTX deck into a template manifest.",
            &["template", "slide", "shape"],
            vec![
                flag("--author", "author", "string", "template author"),
                flag(
                    "--description",
                    "description",
                    "string",
                    "template description",
                ),
                flag("--name", "name", "string", "template name"),
                flag(
                    "--organization",
                    "organization",
                    "string",
                    "template organization",
                ),
                flag(
                    "--slides",
                    "slides",
                    "string",
                    "comma-separated 1-based slide numbers to capture",
                ),
                flag(
                    "--strict-shapes",
                    "strictShapes",
                    "bool",
                    "accepted for legacy CLI compatibility",
                ),
                flag(
                    "--version",
                    "version",
                    "string",
                    "semantic template version, e.g. 1.0.0",
                ),
            ],
            direct("read-only manifest extraction"),
            None,
        ),
        spec(
            PptxCommandId::TemplateCompile,
            &["pptx", "template", "compile"],
            "compile <manifest> <spec> --archetype <pptx> --out <pptx>",
            "Compile a presentation from a template manifest and specification.",
            &["template", "slide", "shape"],
            vec![
                flag(
                    "--archetype",
                    "archetype",
                    "string",
                    "path to archetype PPTX file",
                ),
                flag(
                    "--continue-on-error",
                    "continueOnError",
                    "bool",
                    "continue compilation even if individual slots fail",
                ),
                flag(
                    "--image-base-dir",
                    "imageBaseDir",
                    "string",
                    "base directory for relative image paths in spec",
                ),
                flag("--out", "out", "string", "output PPTX file path"),
            ],
            direct("it does not accept the mutation output flags injected by the op engine"),
            None,
        ),
        spec(
            PptxCommandId::XlsxBindingsPlan,
            &["pptx", "xlsx-bindings", "plan"],
            "plan <file> --workbook <bindings.xlsx> (--range <A1:Z9>|--table <name>)",
            "Resolve XLSX-driven PPTX binding rows without writing the deck.",
            &[
                "template", "slide", "shape", "sheet", "range", "table", "image",
            ],
            vec![
                flag(
                    "--max-cells",
                    "maxCells",
                    "int",
                    "maximum binding/source cells to read",
                ),
                flag("--range", "range", "string", "binding A1 range"),
                flag("--sheet", "sheet", "string", "binding sheet selector"),
                flag(
                    "--table",
                    "table",
                    "string",
                    "binding workbook table selector",
                ),
                flag(
                    "--workbook",
                    "workbook",
                    "string",
                    "XLSX workbook containing binding rows",
                ),
            ],
            direct("read-only mixed binding plan; apply is not ported yet"),
            None,
        ),
        spec(
            PptxCommandId::XlsxBindingsApply,
            &["pptx", "xlsx-bindings", "apply"],
            "apply <file> --workbook <bindings.xlsx> (--range <A1:Z9>|--table <name>) --out <pptx>",
            "Apply XLSX-driven PPTX binding rows to a PowerPoint deck.",
            &[
                "template", "slide", "shape", "sheet", "range", "table", "image",
            ],
            vec![
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
                    "--max-cells",
                    "maxCells",
                    "int",
                    "maximum binding/source cells to read",
                ),
                flag(
                    "--no-validate",
                    "noValidate",
                    "bool",
                    "skip strict validation of the mutated package",
                ),
                flag("--out", "out", "string", "output file path"),
                flag("--range", "range", "string", "binding A1 range"),
                flag("--sheet", "sheet", "string", "binding sheet selector"),
                flag(
                    "--table",
                    "table",
                    "string",
                    "binding workbook table selector",
                ),
                flag(
                    "--workbook",
                    "workbook",
                    "string",
                    "XLSX workbook containing binding rows",
                ),
            ],
            direct(
                "direct CLI mutation; serve/MCP op support is not wired for binding batches yet",
            ),
            None,
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command_manifest::ExecutionSupport;

    #[test]
    fn owner_contract() {
        let specs = command_specs();
        assert_eq!(specs.len(), COMMAND_COUNT);
        assert!(
            specs
                .iter()
                .all(|spec| matches!(&spec.execution, ExecutionSupport::DirectOnly { .. }))
        );
    }
}
