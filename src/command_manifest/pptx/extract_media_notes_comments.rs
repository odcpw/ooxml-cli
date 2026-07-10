use super::{PptxCommandId, direct, flag, inspect, mutation, spec};

pub(super) const COMMAND_COUNT: usize = 14;

pub(super) fn command_specs() -> Vec<super::CommandSpec> {
    vec![
        extract_spec(
            PptxCommandId::ExtractText,
            &["pptx", "extract", "text"],
            "text <file>",
            "Extract slide text grouped by shape.",
            &["slide", "shape"],
        ),
        extract_spec(
            PptxCommandId::ExtractNotes,
            &["pptx", "extract", "notes"],
            "notes <file>",
            "Extract speaker notes from slides.",
            &["slide"],
        ),
        spec(
            PptxCommandId::ExtractImages,
            &["pptx", "extract", "images"],
            "images <file>",
            "Extract slide image files and emit an extraction manifest.",
            &["image", "slide"],
            vec![
                flag(
                    "--out",
                    "out",
                    "string",
                    "output directory for extracted images; defaults to current directory",
                ),
                flag(
                    "--slide",
                    "slide",
                    "int",
                    "1-based slide number to extract; default all slides",
                ),
                flag(
                    "--include-layout-images",
                    "includeLayoutImages",
                    "bool",
                    "include image references from slide layouts",
                ),
            ],
            direct("direct CLI export; writes image files to an output directory"),
            None,
        ),
        spec(
            PptxCommandId::ExtractXml,
            &["pptx", "extract", "xml"],
            "xml <file>",
            "Extract raw slide, layout, and master XML files for debugging.",
            &["slide", "layout", "master"],
            vec![
                flag(
                    "--slide",
                    "slide",
                    "int",
                    "1-based slide number to extract; repeatable",
                ),
                flag(
                    "--layout",
                    "layout",
                    "int",
                    "1-based layout number to extract; repeatable",
                ),
                flag(
                    "--master",
                    "master",
                    "int",
                    "1-based master number to extract; repeatable",
                ),
                flag("--out", "out", "string", "required output directory"),
            ],
            direct("direct CLI export; writes raw XML files to an output directory"),
            None,
        ),
        spec(
            PptxCommandId::MediaList,
            &["pptx", "media", "list"],
            "list <file>",
            "List embedded slide audio/video media clips.",
            &["slide", "media"],
            vec![flag(
                "--slide",
                "slide",
                "int",
                "optional 1-based slide filter; default all slides",
            )],
            direct("read-only command; use before media replacement to discover shape ids/names"),
            None,
        ),
        spec(
            PptxCommandId::MediaAdd,
            &["pptx", "media", "add"],
            "add <file> --slide <n> --file <media>",
            "Embed a local audio/video media clip on a slide.",
            &["slide", "media"],
            with_output_flags(vec![
                flag("--slide", "slide", "int", "1-based target slide"),
                flag("--file", "file", "string", "local media file path"),
                flag(
                    "--kind",
                    "kind",
                    "string",
                    "media kind video|audio; default inferred from extension",
                ),
                flag(
                    "--poster",
                    "poster",
                    "string",
                    "optional poster image; synthesized when omitted",
                ),
                flag("--name", "name", "string", "shape name for the media pic"),
                flag("--x", "x", "int", "left position in EMUs"),
                flag("--y", "y", "int", "top position in EMUs"),
                flag("--cx", "cx", "int", "width in EMUs"),
                flag("--cy", "cy", "int", "height in EMUs"),
                flag(
                    "--play-trigger",
                    "playTrigger",
                    "string",
                    "click or none; default click",
                ),
                flag(
                    "--play-cmd",
                    "playCmd",
                    "bool",
                    "also emit the experimental playFrom timing command",
                ),
                flag("--volume", "volume", "int", "playback volume 0..100"),
                flag("--mute", "mute", "bool", "mute the clip"),
                flag(
                    "--insert-after-shape",
                    "insertAfterShape",
                    "int",
                    "insert after this shape id; default append",
                ),
            ]),
            direct("direct CLI mutation; serve/MCP op support is not wired yet"),
            None,
        ),
        spec(
            PptxCommandId::MediaReplace,
            &["pptx", "media", "replace"],
            "replace <file> --slide <n> (--shape <id>|--shape-name <name>) --file <media>",
            "Replace an existing embedded media clip by shape id or shape name.",
            &["slide", "shape", "media"],
            with_output_flags(vec![
                flag("--slide", "slide", "int", "1-based target slide"),
                flag("--shape", "shape", "int", "target media shape id"),
                flag(
                    "--shape-name",
                    "shapeName",
                    "string",
                    "target media shape name",
                ),
                flag("--file", "file", "string", "new local media file path"),
                flag(
                    "--kind",
                    "kind",
                    "string",
                    "new media kind video|audio; default inferred from extension",
                ),
                flag(
                    "--poster",
                    "poster",
                    "string",
                    "optional replacement poster image",
                ),
                flag(
                    "--volume",
                    "volume",
                    "int",
                    "optional playback volume update 0..100",
                ),
                flag("--mute", "mute", "bool", "optional mute update"),
                flag(
                    "--expect-shape-name",
                    "expectShapeName",
                    "string",
                    "guard: require resolved shape name",
                ),
                flag(
                    "--expect-media-kind",
                    "expectMediaKind",
                    "string",
                    "guard: require existing media kind",
                ),
            ]),
            direct("direct CLI mutation; serve/MCP op support is not wired yet"),
            None,
        ),
        spec(
            PptxCommandId::NotesShow,
            &["pptx", "notes", "show"],
            "show <file> --slide <n>",
            "Show speaker notes for one slide.",
            &["slide"],
            vec![flag("--slide", "slide", "int", "1-based slide number")],
            inspect("read-only command; call via inspect in serve/MCP"),
            None,
        ),
        notes_mutation(
            PptxCommandId::NotesSet,
            &["pptx", "notes", "set"],
            "set <file> --slide <n> --text <text>",
            "Set speaker notes text for a slide.",
            vec![
                flag("--slide", "slide", "int", "1-based slide number"),
                flag(
                    "--text",
                    "text",
                    "string",
                    "notes text; embedded newlines become separate paragraphs",
                ),
            ],
        ),
        notes_mutation(
            PptxCommandId::NotesClear,
            &["pptx", "notes", "clear"],
            "clear <file> --slide <n>",
            "Clear speaker notes text for a slide.",
            vec![flag("--slide", "slide", "int", "1-based slide number")],
        ),
        spec(
            PptxCommandId::CommentsList,
            &["pptx", "comments", "list"],
            "list <file> [--slide <n>] [--comment-id <id>]",
            "List PPTX slide comments with stable selectors, authors, dates, text, and hashes.",
            &["slide", "comment"],
            vec![
                flag("--slide", "slide", "int", "optional 1-based slide number"),
                flag(
                    "--comment-id",
                    "commentId",
                    "int",
                    "show only this comment id; requires --slide",
                ),
            ],
            inspect(
                "read-only command; generated handles can be used by comment mutation commands",
            ),
            None,
        ),
        spec(
            PptxCommandId::CommentsAdd,
            &["pptx", "comments", "add"],
            "add <file> --slide <n> --author <name> (--text <text>|--text-file <path>)",
            "Add a legacy slide comment to a PPTX slide.",
            &["slide", "comment"],
            with_output_flags(vec![
                flag("--slide", "slide", "int", "1-based slide number"),
                flag("--author", "author", "string", "comment author name"),
                flag(
                    "--initials",
                    "initials",
                    "string",
                    "optional author initials",
                ),
                flag(
                    "--date",
                    "date",
                    "string",
                    "RFC3339 timestamp; defaults to now",
                ),
                flag("--text", "text", "string", "comment text"),
                flag("--text-file", "textFile", "string", "path to comment text"),
            ]),
            direct("direct CLI mutation; serve/MCP op support is not wired yet"),
            None,
        ),
        spec(
            PptxCommandId::CommentsEdit,
            &["pptx", "comments", "edit"],
            "edit <file> --slide <n> --comment-id <id>",
            "Edit an existing PPTX slide comment by id, author id, or stable handle.",
            &["slide", "comment"],
            with_output_flags({
                let mut flags = comment_target_flags();
                flags.extend([
                    flag("--text", "text", "string", "new comment text"),
                    flag(
                        "--text-file",
                        "textFile",
                        "string",
                        "path to new comment text",
                    ),
                    flag("--author", "author", "string", "new author name"),
                    flag(
                        "--date",
                        "date",
                        "string",
                        "new RFC3339 timestamp; empty clears it",
                    ),
                    expect_hash_flag(),
                ]);
                flags
            }),
            direct("direct CLI mutation; serve/MCP op support is not wired yet"),
            None,
        ),
        spec(
            PptxCommandId::CommentsRemove,
            &["pptx", "comments", "remove"],
            "remove <file> --slide <n> --comment-id <id>",
            "Remove an existing PPTX slide comment by id, author id, or stable handle.",
            &["slide", "comment"],
            with_output_flags({
                let mut flags = comment_target_flags();
                flags.push(expect_hash_flag());
                flags
            }),
            direct("direct CLI mutation; serve/MCP op support is not wired yet"),
            None,
        ),
    ]
}

fn extract_spec(
    id: PptxCommandId,
    path: &'static [&'static str],
    use_text: &'static str,
    short: &'static str,
    targets: &'static [&'static str],
) -> super::CommandSpec {
    spec(
        id,
        path,
        use_text,
        short,
        targets,
        vec![flag(
            "--slide",
            "slide",
            "int",
            "1-based slide number; repeatable",
        )],
        inspect("read-only command; call via inspect in serve/MCP"),
        None,
    )
}

fn notes_mutation(
    id: PptxCommandId,
    path: &'static [&'static str],
    use_text: &'static str,
    short: &'static str,
    flags: Vec<super::FlagSpec>,
) -> super::CommandSpec {
    spec(
        id,
        path,
        use_text,
        short,
        &["slide"],
        with_output_flags(flags),
        mutation(None),
        None,
    )
}

fn comment_target_flags() -> Vec<super::FlagSpec> {
    vec![
        flag("--slide", "slide", "int", "1-based slide number"),
        flag(
            "--comment-id",
            "commentId",
            "int",
            "comment id from comments list",
        ),
        flag(
            "--author-id",
            "authorId",
            "int",
            "disambiguating authorId from comments list",
        ),
        flag("--handle", "handle", "string", "stable PPTX comment handle"),
    ]
}

fn expect_hash_flag() -> super::FlagSpec {
    flag(
        "--expect-hash",
        "expectHash",
        "string",
        "expected sha256 content hash from comments list",
    )
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
            4
        );
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
            8
        );
    }
}
