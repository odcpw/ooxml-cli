use super::{ExecutionSupport, XlsxCommandId, flag, spec};

pub(super) const COMMAND_COUNT: usize = 4;
pub(super) const LEGACY_START: usize = 192;

pub(super) fn command_specs() -> Vec<super::CommandSpec> {
    vec![
        spec(
            XlsxCommandId::CommentsList,
            &["xlsx", "comments", "list"],
            "list <file> [--sheet <sheet>] [--comment-id <id>]",
            "List worksheet comments, authors, selectors, hashes, and anchored cells.",
            &["comment", "sheet", "cell"],
            vec![
                flag("--sheet", "sheet", "string", "sheet selector"),
                flag(
                    "--comment-id",
                    "commentId",
                    "int",
                    "only return the comment with this zero-based id",
                ),
            ],
            ExecutionSupport::ServeInspect {
                reason: Some("read-only command; call via inspect in serve/MCP"),
            },
            None,
        ),
        spec(
            XlsxCommandId::CommentsAdd,
            &["xlsx", "comments", "add"],
            "add <file> --cell <A1> --author <name> [--text <text>]",
            "Add a worksheet cell comment, creating comments and legacy VML drawing parts when needed.",
            &["comment", "sheet", "cell"],
            vec![
                flag("--sheet", "sheet", "string", "sheet selector"),
                flag("--cell", "cell", "string", "target cell such as C3"),
                flag("--author", "author", "string", "comment author"),
                flag("--text", "text", "string", "comment text"),
                flag(
                    "--text-file",
                    "textFile",
                    "string",
                    "read comment text from file",
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
            XlsxCommandId::CommentsUpdate,
            &["xlsx", "comments", "update"],
            "update <file> (--handle <handle>|--comment-id <id>)",
            "Update a worksheet comment's text and/or author with optional hash guard.",
            &["comment", "sheet", "cell"],
            vec![
                flag(
                    "--sheet",
                    "sheet",
                    "string",
                    "sheet selector used with --comment-id",
                ),
                flag(
                    "--comment-id",
                    "commentId",
                    "int",
                    "zero-based comment id on the selected sheet",
                ),
                flag("--handle", "handle", "string", "published comment handle"),
                flag("--text", "text", "string", "replacement comment text"),
                flag(
                    "--text-file",
                    "textFile",
                    "string",
                    "read replacement text from file",
                ),
                flag(
                    "--author",
                    "author",
                    "string",
                    "replacement/additional author",
                ),
                flag(
                    "--expect-hash",
                    "expectHash",
                    "string",
                    "guard: expected current comment content hash",
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
            XlsxCommandId::CommentsRemove,
            &["xlsx", "comments", "remove"],
            "remove <file> (--handle <handle>|--comment-id <id>)",
            "Remove a worksheet comment, cleaning orphaned comments/VML parts when the sheet has no comments left.",
            &["comment", "sheet", "cell"],
            vec![
                flag(
                    "--sheet",
                    "sheet",
                    "string",
                    "sheet selector used with --comment-id",
                ),
                flag(
                    "--comment-id",
                    "commentId",
                    "int",
                    "zero-based comment id on the selected sheet",
                ),
                flag("--handle", "handle", "string", "published comment handle"),
                flag(
                    "--expect-hash",
                    "expectHash",
                    "string",
                    "guard: expected current comment content hash",
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
    ]
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::command_manifest::{assert_segment_matches_legacy, capability_value};

    #[test]
    fn comments_segment_matches_fixed_legacy_slice() {
        let specs = command_specs();
        let legacy = crate::capabilities::capability_commands();
        assert_eq!(specs.len(), COMMAND_COUNT);
        assert_segment_matches_legacy(&specs, &legacy[LEGACY_START..LEGACY_START + COMMAND_COUNT]);
    }

    #[test]
    fn comments_ids_paths_builds_and_execution_inventory_are_stable() {
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
        assert_eq!(inventory, (0, 0, 1, 3));
    }
}
