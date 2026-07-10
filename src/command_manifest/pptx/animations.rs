use super::{PptxCommandId, direct, flag, spec};

pub(super) const COMMAND_COUNT: usize = 5;

pub(super) fn command_specs() -> Vec<super::CommandSpec> {
    vec![
        spec(
            PptxCommandId::AnimationsList,
            &["pptx", "animations", "list"],
            "list <file>",
            "List PPTX slide animation timing, builds, embedded media, and stale targets.",
            &["slide", "shape", "animation"],
            vec![],
            direct("read-only command; generated selectors feed animation mutation commands"),
            None,
        ),
        spec(
            PptxCommandId::AnimationsAdd,
            &["pptx", "animations", "add"],
            "add <file> --slide <n> --shape <selector> --effect <kind>",
            "Add an entrance animation to a PowerPoint shape.",
            &["slide", "shape", "animation"],
            with_output_flags(vec![
                flag("--slide", "slide", "int", "1-based slide number"),
                flag(
                    "--shape",
                    "shape",
                    "string",
                    "target shape selector such as shape:2, ~Title 1, or stable shape handle",
                ),
                flag(
                    "--effect",
                    "effect",
                    "string",
                    "entrance effect: appear, fade, wipe, or fly-in",
                ),
                flag(
                    "--direction",
                    "direction",
                    "string",
                    "direction for wipe/fly-in: up, down, left, or right",
                ),
                flag(
                    "--duration-ms",
                    "durationMs",
                    "int",
                    "effect duration in milliseconds",
                ),
                flag(
                    "--start",
                    "start",
                    "string",
                    "start trigger: onClick, withPrevious, or afterPrevious",
                ),
                flag(
                    "--by-paragraph",
                    "byParagraph",
                    "bool",
                    "fan out one effect per paragraph and add a by-paragraph build",
                ),
                flag(
                    "--paragraph-range",
                    "paragraphRange",
                    "string",
                    "single 0-based inclusive paragraph range A:B",
                ),
                flag(
                    "--expect-shape-name",
                    "expectShapeName",
                    "string",
                    "stale guard for the resolved shape name",
                ),
                flag(
                    "--expect-paragraph-count",
                    "expectParagraphCount",
                    "int",
                    "stale guard for by-paragraph paragraph count",
                ),
            ]),
            direct("direct CLI mutation; serve/MCP op support is not wired yet"),
            None,
        ),
        spec(
            PptxCommandId::AnimationsRemove,
            &["pptx", "animations", "remove"],
            "remove <file> --slide <n> --effect-id <id>",
            "Remove a supported entrance animation by effect id.",
            &["slide", "shape", "animation"],
            with_output_flags(vec![
                flag("--slide", "slide", "int", "1-based slide number"),
                flag(
                    "--effect-id",
                    "effectId",
                    "int",
                    "effect cTn id from animations list",
                ),
                flag(
                    "--expect-shape-name",
                    "expectShapeName",
                    "string",
                    "stale guard for the effect target shape name",
                ),
            ]),
            direct("direct CLI mutation; serve/MCP op support is not wired yet"),
            None,
        ),
        spec(
            PptxCommandId::AnimationsReorder,
            &["pptx", "animations", "reorder"],
            "reorder <file> --slide <n> --order <ids>",
            "Reorder the top-level click animation steps on a slide.",
            &["slide", "animation"],
            with_output_flags(vec![
                flag("--slide", "slide", "int", "1-based slide number"),
                flag(
                    "--order",
                    "order",
                    "string",
                    "comma-separated permutation of clickStep ids from animations list",
                ),
            ]),
            direct("direct CLI mutation; serve/MCP op support is not wired yet"),
            None,
        ),
        spec(
            PptxCommandId::AnimationsPruneStale,
            &["pptx", "animations", "prune-stale"],
            "prune-stale <file> [--slide <n>]",
            "Remove supported animation effects/builds whose targets are stale.",
            &["slide", "shape", "animation"],
            with_output_flags(vec![flag(
                "--slide",
                "slide",
                "int",
                "optional 1-based slide number; default all slides",
            )]),
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
        assert!(
            specs
                .iter()
                .all(|spec| matches!(&spec.execution, ExecutionSupport::DirectOnly { .. }))
        );
    }
}
