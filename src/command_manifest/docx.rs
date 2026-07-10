use serde_json::Value;

use super::{CommandId, CommandSpec, ExecutionSupport, FlagSpec};

pub(super) const COMMAND_COUNT: usize = 45;
pub(super) const LEGACY_START: usize = 248;

command_id_enum! {
pub(super) enum DocxCommandId {
    Docx,
    Comments,
    Fields,
    Footers,
    Headers,
    Images,
    Paragraphs,
    Styles,
    Tables,
    Scaffold,
    Text,
    Blocks,
    BlocksReplace,
    BlocksDelete,
    BlocksInsertAfter,
    ParagraphsAppend,
    ParagraphsInsert,
    ParagraphsSet,
    ParagraphsClear,
    StylesList,
    StylesShow,
    StylesApply,
    CommentsList,
    CommentsAdd,
    CommentsEdit,
    CommentsRemove,
    FieldsList,
    FieldsInsert,
    FieldsSetResult,
    HeadersList,
    HeadersShow,
    HeadersSetText,
    FootersList,
    FootersShow,
    FootersSetText,
    ImagesList,
    ImagesReplace,
    ImagesInsert,
    Replace,
    TablesShow,
    TablesCreate,
    TablesSetCell,
    TablesClearCell,
    TablesInsertRow,
    TablesDeleteRow,
}}

pub(super) fn command_specs() -> Vec<CommandSpec> {
    vec![
        spec(
            DocxCommandId::Docx,
            &["docx"],
            "docx",
            "Work with DOCX documents",
            &[],
            vec![],
            ExecutionSupport::GroupOnly {
                reason: Some("it is a command group, not a leaf mutation command"),
            },
            None,
        ),
        spec(
            DocxCommandId::Comments,
            &["docx", "comments"],
            "comments",
            "Inspect and mutate DOCX comments",
            &[],
            vec![],
            ExecutionSupport::GroupOnly {
                reason: Some("it is a command group, not a leaf mutation command"),
            },
            None,
        ),
        spec(
            DocxCommandId::Fields,
            &["docx", "fields"],
            "fields",
            "Inspect and edit DOCX fields (PAGE, NUMPAGES, DATE, etc.)",
            &[],
            vec![],
            ExecutionSupport::GroupOnly {
                reason: Some("it is a command group, not a leaf mutation command"),
            },
            None,
        ),
        spec(
            DocxCommandId::Footers,
            &["docx", "footers"],
            "footers",
            "Inspect and edit DOCX footers",
            &[],
            vec![],
            ExecutionSupport::GroupOnly {
                reason: Some("it is a command group, not a leaf mutation command"),
            },
            None,
        ),
        spec(
            DocxCommandId::Headers,
            &["docx", "headers"],
            "headers",
            "Inspect and edit DOCX headers",
            &[],
            vec![],
            ExecutionSupport::GroupOnly {
                reason: Some("it is a command group, not a leaf mutation command"),
            },
            None,
        ),
        spec(
            DocxCommandId::Images,
            &["docx", "images"],
            "images",
            "Inspect and mutate inline images in a DOCX document",
            &[],
            vec![],
            ExecutionSupport::GroupOnly {
                reason: Some("it is a command group, not a leaf mutation command"),
            },
            None,
        ),
        spec(
            DocxCommandId::Paragraphs,
            &["docx", "paragraphs"],
            "paragraphs",
            "Mutate DOCX body paragraphs",
            &[],
            vec![],
            ExecutionSupport::GroupOnly {
                reason: Some("it is a command group, not a leaf mutation command"),
            },
            None,
        ),
        spec(
            DocxCommandId::Styles,
            &["docx", "styles"],
            "styles",
            "Inspect DOCX style definitions from word/styles.xml",
            &[],
            vec![],
            ExecutionSupport::GroupOnly {
                reason: Some("it is a command group, not a leaf mutation command"),
            },
            None,
        ),
        spec(
            DocxCommandId::Tables,
            &["docx", "tables"],
            "tables",
            "Inspect and mutate DOCX tables",
            &[],
            vec![],
            ExecutionSupport::GroupOnly {
                reason: Some("it is a command group, not a leaf mutation command"),
            },
            None,
        ),
        spec(
            DocxCommandId::Scaffold,
            &["docx", "scaffold"],
            "scaffold <output.docx> (or --out <output.docx>)",
            "Create a minimal DOCX package from scratch and validate it by default.",
            &["package"],
            vec![
                flag(
                    "--out",
                    "out",
                    "string",
                    "output document path; accepted as an alternative to positional <output.docx>",
                ),
                flag(
                    "--text",
                    "text",
                    "string",
                    "optional initial paragraph text",
                ),
                flag(
                    "--text-file",
                    "textFile",
                    "string",
                    "path to optional initial paragraph text",
                ),
                flag(
                    "--force",
                    "force",
                    "bool",
                    "replace an existing output file",
                ),
                flag(
                    "--no-validate",
                    "noValidate",
                    "bool",
                    "skip post-write strict validation",
                ),
            ],
            ExecutionSupport::DirectOnly {
                reason: Some("it creates a package and is not an apply/serve mutation op"),
            },
            None,
        ),
        spec(
            DocxCommandId::Text,
            &["docx", "text"],
            "text <file>",
            "Extract DOCX paragraph text.",
            &["package"],
            vec![],
            ExecutionSupport::ServeInspect {
                reason: Some("read-only command"),
            },
            None,
        ),
        spec(
            DocxCommandId::Blocks,
            &["docx", "blocks"],
            "blocks <file>",
            "Show stable DOCX body blocks with hashes, selectors, paragraph metadata, table cells, and optional runs.",
            &[],
            vec![
                flag(
                    "--block",
                    "block",
                    "int",
                    "1-based body block index to show",
                ),
                flag(
                    "--include-runs",
                    "includeRuns",
                    "bool",
                    "include paragraph run text and basic run properties",
                ),
            ],
            ExecutionSupport::ServeInspect {
                reason: Some(
                    "read-only command; block hashes and selectors feed hash-guarded DOCX mutations",
                ),
            },
            None,
        ),
        spec(
            DocxCommandId::BlocksReplace,
            &["docx", "blocks", "replace"],
            "replace <file>",
            "Replace a hash-guarded DOCX body block with a paragraph.",
            &["paragraph"],
            vec![
                flag("--backup", "backup", "string", "backup path for --in-place"),
                flag(
                    "--block",
                    "block",
                    "int",
                    "1-based body block index from docx blocks",
                ),
                flag("--dry-run", "dryRun", "bool", "plan without writing"),
                flag(
                    "--expect-hash",
                    "expectHash",
                    "string",
                    "expected sha256: content hash from docx blocks",
                ),
                flag("--in-place", "inPlace", "bool", "write in place"),
                flag(
                    "--no-validate",
                    "noValidate",
                    "bool",
                    "skip post-write validation",
                ),
                flag("--out", "out", "string", "output file path"),
                flag(
                    "--style",
                    "style",
                    "string",
                    "optional paragraph style ID; default preserves paragraph style when replacing a paragraph",
                ),
                flag("--text", "text", "string", "replacement paragraph text"),
                flag(
                    "--text-file",
                    "textFile",
                    "string",
                    "path to replacement paragraph text",
                ),
            ],
            ExecutionSupport::ServeMutation { reason: None },
            None,
        ),
        spec(
            DocxCommandId::BlocksDelete,
            &["docx", "blocks", "delete"],
            "delete <file>",
            "Delete a hash-guarded DOCX body block.",
            &["paragraph", "table"],
            vec![
                flag("--backup", "backup", "string", "backup path for --in-place"),
                flag(
                    "--block",
                    "block",
                    "int",
                    "1-based body block index from docx blocks",
                ),
                flag("--dry-run", "dryRun", "bool", "plan without writing"),
                flag(
                    "--expect-hash",
                    "expectHash",
                    "string",
                    "expected sha256: content hash from docx blocks",
                ),
                flag("--in-place", "inPlace", "bool", "write in place"),
                flag(
                    "--no-validate",
                    "noValidate",
                    "bool",
                    "skip post-write validation",
                ),
                flag("--out", "out", "string", "output file path"),
            ],
            ExecutionSupport::ServeMutation { reason: None },
            None,
        ),
        spec(
            DocxCommandId::BlocksInsertAfter,
            &["docx", "blocks", "insert-after"],
            "insert-after <file>",
            "Insert a paragraph after a hash-guarded DOCX body block.",
            &["paragraph"],
            vec![
                flag("--backup", "backup", "string", "backup path for --in-place"),
                flag(
                    "--block",
                    "block",
                    "int",
                    "1-based body block index from docx blocks; 0 inserts before the first block",
                ),
                flag("--dry-run", "dryRun", "bool", "plan without writing"),
                flag(
                    "--expect-hash",
                    "expectHash",
                    "string",
                    "expected sha256: content hash from docx blocks when --block is greater than 0",
                ),
                flag("--in-place", "inPlace", "bool", "write in place"),
                flag(
                    "--no-validate",
                    "noValidate",
                    "bool",
                    "skip post-write validation",
                ),
                flag("--out", "out", "string", "output file path"),
                flag("--style", "style", "string", "optional paragraph style ID"),
                flag("--text", "text", "string", "paragraph text"),
                flag(
                    "--text-file",
                    "textFile",
                    "string",
                    "path to paragraph text",
                ),
            ],
            ExecutionSupport::ServeMutation { reason: None },
            None,
        ),
        spec(
            DocxCommandId::ParagraphsAppend,
            &["docx", "paragraphs", "append"],
            "append <file>",
            "Append a main document body paragraph, preserving trailing section properties.",
            &["paragraph"],
            vec![
                flag("--text", "text", "string", "paragraph text"),
                flag(
                    "--text-file",
                    "textFile",
                    "string",
                    "path to paragraph text",
                ),
                flag("--style", "style", "string", "optional paragraph style ID"),
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
                    "skip post-write validation",
                ),
            ],
            ExecutionSupport::ServeMutation { reason: None },
            None,
        ),
        spec(
            DocxCommandId::ParagraphsInsert,
            &["docx", "paragraphs", "insert"],
            "insert <file>",
            "Insert a main document body paragraph after a body block index.",
            &["paragraph"],
            vec![
                flag(
                    "--insert-after",
                    "insertAfter",
                    "int",
                    "0 to prepend, or a 1-based body block index",
                ),
                flag("--text", "text", "string", "paragraph text"),
                flag(
                    "--text-file",
                    "textFile",
                    "string",
                    "path to paragraph text",
                ),
                flag("--style", "style", "string", "optional paragraph style ID"),
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
                    "skip post-write validation",
                ),
            ],
            ExecutionSupport::ServeMutation { reason: None },
            None,
        ),
        spec(
            DocxCommandId::ParagraphsSet,
            &["docx", "paragraphs", "set"],
            "set <file>",
            "Replace one main document body paragraph's plain text.",
            &["paragraph"],
            vec![
                flag("--index", "index", "int", "1-based body block index"),
                flag(
                    "--handle",
                    "handle",
                    "string",
                    "stable DOCX paragraph handle",
                ),
                flag("--text", "text", "string", "replacement paragraph text"),
                flag(
                    "--text-file",
                    "textFile",
                    "string",
                    "path to replacement paragraph text",
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
                    "skip post-write validation",
                ),
            ],
            ExecutionSupport::ServeMutation { reason: None },
            None,
        ),
        spec(
            DocxCommandId::ParagraphsClear,
            &["docx", "paragraphs", "clear"],
            "clear <file>",
            "Clear one main document body paragraph's text while retaining paragraph metadata.",
            &["paragraph"],
            vec![
                flag("--index", "index", "int", "1-based body block index"),
                flag(
                    "--handle",
                    "handle",
                    "string",
                    "stable DOCX paragraph handle",
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
                    "skip post-write validation",
                ),
            ],
            ExecutionSupport::ServeMutation { reason: None },
            None,
        ),
        spec(
            DocxCommandId::StylesList,
            &["docx", "styles", "list"],
            "list <file>",
            "List DOCX paragraph, character, table, and numbering styles.",
            &["style"],
            vec![flag(
                "--type",
                "type",
                "string",
                "filter by style type: paragraph, character, table, or numbering",
            )],
            ExecutionSupport::ServeInspect {
                reason: Some(
                    "read-only command; generated style handles can be used by mutation commands",
                ),
            },
            None,
        ),
        spec(
            DocxCommandId::StylesShow,
            &["docx", "styles", "show"],
            "show <file>",
            "Show detailed info for one DOCX style by styleId.",
            &["style"],
            vec![flag("--style", "style", "string", "styleId to show")],
            ExecutionSupport::ServeInspect {
                reason: Some(
                    "read-only command; generated style handles can be used by mutation commands",
                ),
            },
            None,
        ),
        spec(
            DocxCommandId::StylesApply,
            &["docx", "styles", "apply"],
            "apply <file>",
            "Apply a paragraph, run, or table style to DOCX body content.",
            &["style", "paragraph", "table"],
            vec![
                flag(
                    "--index",
                    "index",
                    "int",
                    "1-based body block index for paragraph/run, or 1-based table number for table",
                ),
                flag(
                    "--handle",
                    "handle",
                    "string",
                    "stable DOCX paragraph handle for paragraph/run targets",
                ),
                flag(
                    "--target",
                    "target",
                    "string",
                    "style target: paragraph, run, or table",
                ),
                flag(
                    "--style",
                    "style",
                    "string",
                    "styleId or H:docx/pt:styles/style:n:<styleId> handle",
                ),
                flag(
                    "--expect-hash",
                    "expectHash",
                    "string",
                    "optional sha256 block hash guard from docx blocks",
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
                    "skip style existence/type validation and post-write validation",
                ),
            ],
            ExecutionSupport::ServeMutation { reason: None },
            None,
        ),
        spec(
            DocxCommandId::CommentsList,
            &["docx", "comments", "list"],
            "list <file>",
            "List DOCX comments with stable selectors, hashes, and anchor blocks.",
            &["comment"],
            vec![flag(
                "--comment-id",
                "commentId",
                "int",
                "show only the comment with this numeric w:id",
            )],
            ExecutionSupport::ServeInspect {
                reason: Some(
                    "read-only command; generated comment handles can be used by mutation commands",
                ),
            },
            None,
        ),
        spec(
            DocxCommandId::CommentsAdd,
            &["docx", "comments", "add"],
            "add <file>",
            "Add a DOCX comment anchored to a body paragraph.",
            &["comment"],
            vec![
                flag(
                    "--anchor-block",
                    "anchorBlock",
                    "int",
                    "1-based body block index to anchor to (default: first block)",
                ),
                flag("--author", "author", "string", "comment author name"),
                flag(
                    "--initials",
                    "initials",
                    "string",
                    "optional comment author initials",
                ),
                flag(
                    "--date",
                    "date",
                    "string",
                    "RFC3339 timestamp (default: now)",
                ),
                flag("--text", "text", "string", "comment text"),
                flag("--text-file", "textFile", "string", "path to comment text"),
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
                    "skip post-write validation",
                ),
            ],
            ExecutionSupport::ServeMutation { reason: None },
            None,
        ),
        spec(
            DocxCommandId::CommentsEdit,
            &["docx", "comments", "edit"],
            "edit <file>",
            "Edit an existing DOCX comment by id or stable handle.",
            &["comment"],
            vec![
                flag(
                    "--comment-id",
                    "commentId",
                    "int",
                    "comment id from comments list",
                ),
                flag("--handle", "handle", "string", "stable DOCX comment handle"),
                flag("--text", "text", "string", "new comment text"),
                flag(
                    "--text-file",
                    "textFile",
                    "string",
                    "path to new comment text",
                ),
                flag("--author", "author", "string", "new author"),
                flag("--date", "date", "string", "new RFC3339 timestamp"),
                flag(
                    "--expect-hash",
                    "expectHash",
                    "string",
                    "expected sha256 content hash from comments list",
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
                    "skip post-write validation",
                ),
            ],
            ExecutionSupport::ServeMutation { reason: None },
            None,
        ),
        spec(
            DocxCommandId::CommentsRemove,
            &["docx", "comments", "remove"],
            "remove <file>",
            "Remove an existing DOCX comment and its range/reference markers.",
            &["comment"],
            vec![
                flag(
                    "--comment-id",
                    "commentId",
                    "int",
                    "comment id from comments list",
                ),
                flag("--handle", "handle", "string", "stable DOCX comment handle"),
                flag(
                    "--expect-hash",
                    "expectHash",
                    "string",
                    "expected sha256 content hash from comments list",
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
                    "skip post-write validation",
                ),
            ],
            ExecutionSupport::ServeMutation { reason: None },
            None,
        ),
        spec(
            DocxCommandId::FieldsList,
            &["docx", "fields", "list"],
            "list <file>",
            "List all simple/complex fields in document body + headers/footers.",
            &["field"],
            vec![flag(
                "--type",
                "type",
                "string",
                "show only fields whose leading instruction keyword matches",
            )],
            ExecutionSupport::ServeInspect {
                reason: Some(
                    "read-only command; cached field results are stale until Word recalculates fields",
                ),
            },
            None,
        ),
        spec(
            DocxCommandId::FieldsInsert,
            &["docx", "fields", "insert"],
            "insert <file>",
            "Insert a simple DOCX field into a body, header, or footer paragraph.",
            &["field", "paragraph"],
            vec![
                flag(
                    "--location",
                    "location",
                    "string",
                    "target part:block location, e.g. body:2 or header1:1",
                ),
                flag(
                    "--field-code",
                    "fieldCode",
                    "string",
                    "field instruction, e.g. PAGE",
                ),
                flag("--result", "result", "string", "initial cached result text"),
                flag("--out", "out", "string", "output file path"),
                flag("--backup", "backup", "string", "backup path for --in-place"),
                flag("--dry-run", "dryRun", "bool", "plan without writing"),
                flag("--in-place", "inPlace", "bool", "write in place"),
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
            DocxCommandId::FieldsSetResult,
            &["docx", "fields", "set-result"],
            "set-result <file>",
            "Set the cached result text of a simple or complex DOCX field.",
            &["field", "paragraph"],
            vec![
                flag(
                    "--selector",
                    "selector",
                    "string",
                    "field selector part:block:field, e.g. body:1:0",
                ),
                flag("--result", "result", "string", "new cached result text"),
                flag(
                    "--expect-hash",
                    "expectHash",
                    "string",
                    "expected sha256 of instruction plus cached result",
                ),
                flag("--out", "out", "string", "output file path"),
                flag("--backup", "backup", "string", "backup path for --in-place"),
                flag("--dry-run", "dryRun", "bool", "plan without writing"),
                flag("--in-place", "inPlace", "bool", "write in place"),
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
            DocxCommandId::HeadersList,
            &["docx", "headers", "list"],
            "list <file>",
            "List headers and footers defined per section.",
            &["header", "footer"],
            vec![],
            ExecutionSupport::ServeInspect {
                reason: Some(
                    "read-only command; generated header/footer selectors can be pasted into show or set-text",
                ),
            },
            None,
        ),
        spec(
            DocxCommandId::HeadersShow,
            &["docx", "headers", "show"],
            "show <file>",
            "Show header content by type, section, or relationship id.",
            &["header", "paragraph"],
            vec![
                flag(
                    "--id",
                    "id",
                    "string",
                    "relationship id to resolve directly",
                ),
                flag(
                    "--section",
                    "section",
                    "int",
                    "1-based section index; 0 means the last section",
                ),
                flag(
                    "--selector",
                    "selector",
                    "string",
                    "selector from headers/footers list",
                ),
                flag("--type", "type", "string", "default, first, or even"),
            ],
            ExecutionSupport::ServeInspect {
                reason: Some("read-only command; accepts selectors from docx headers list"),
            },
            None,
        ),
        spec(
            DocxCommandId::HeadersSetText,
            &["docx", "headers", "set-text"],
            "set-text <file>",
            "Set header paragraph text by index.",
            &["header", "paragraph"],
            vec![
                flag(
                    "--id",
                    "id",
                    "string",
                    "relationship id to resolve directly",
                ),
                flag("--type", "type", "string", "default, first, or even"),
                flag(
                    "--section",
                    "section",
                    "int",
                    "1-based section index; 0 means the last section",
                ),
                flag(
                    "--index",
                    "index",
                    "int",
                    "1-based paragraph index within the part",
                ),
                flag(
                    "--selector",
                    "selector",
                    "string",
                    "selector from headers/footers list",
                ),
                flag("--text", "text", "string", "replacement text"),
                flag(
                    "--text-file",
                    "textFile",
                    "string",
                    "path to replacement text",
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
                    "skip post-write validation",
                ),
            ],
            ExecutionSupport::ServeMutation { reason: None },
            None,
        ),
        spec(
            DocxCommandId::FootersList,
            &["docx", "footers", "list"],
            "list <file>",
            "List headers and footers defined per section.",
            &["footer", "header"],
            vec![],
            ExecutionSupport::ServeInspect {
                reason: Some(
                    "read-only command; generated header/footer selectors can be pasted into show or set-text",
                ),
            },
            None,
        ),
        spec(
            DocxCommandId::FootersShow,
            &["docx", "footers", "show"],
            "show <file>",
            "Show footer content by type, section, or relationship id.",
            &["footer", "paragraph"],
            vec![
                flag(
                    "--id",
                    "id",
                    "string",
                    "relationship id to resolve directly",
                ),
                flag(
                    "--section",
                    "section",
                    "int",
                    "1-based section index; 0 means the last section",
                ),
                flag(
                    "--selector",
                    "selector",
                    "string",
                    "selector from headers/footers list",
                ),
                flag("--type", "type", "string", "default, first, or even"),
            ],
            ExecutionSupport::ServeInspect {
                reason: Some("read-only command; accepts selectors from docx footers list"),
            },
            None,
        ),
        spec(
            DocxCommandId::FootersSetText,
            &["docx", "footers", "set-text"],
            "set-text <file>",
            "Set footer paragraph text by index.",
            &["footer", "paragraph"],
            vec![
                flag(
                    "--id",
                    "id",
                    "string",
                    "relationship id to resolve directly",
                ),
                flag("--type", "type", "string", "default, first, or even"),
                flag(
                    "--section",
                    "section",
                    "int",
                    "1-based section index; 0 means the last section",
                ),
                flag(
                    "--index",
                    "index",
                    "int",
                    "1-based paragraph index within the part",
                ),
                flag(
                    "--selector",
                    "selector",
                    "string",
                    "selector from headers/footers list",
                ),
                flag("--text", "text", "string", "replacement text"),
                flag(
                    "--text-file",
                    "textFile",
                    "string",
                    "path to replacement text",
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
                    "skip post-write validation",
                ),
            ],
            ExecutionSupport::ServeMutation { reason: None },
            None,
        ),
        spec(
            DocxCommandId::ImagesList,
            &["docx", "images", "list"],
            "list <file>",
            "List inline images in a DOCX document.",
            &["image", "paragraph"],
            vec![],
            ExecutionSupport::ServeInspect {
                reason: Some(
                    "read-only command; image records include relationship ids, media parts, dimensions, and block anchors",
                ),
            },
            None,
        ),
        spec(
            DocxCommandId::ImagesReplace,
            &["docx", "images", "replace"],
            "replace <file>",
            "Replace one inline DOCX image payload and optionally resize the drawing.",
            &["image"],
            vec![
                flag(
                    "--image",
                    "image",
                    "string",
                    "1-based image index or relationship id from docx images list",
                ),
                flag("--file", "file", "string", "replacement image file path"),
                flag(
                    "--expect-hash",
                    "expectHash",
                    "string",
                    "optional expected sha256: block hash from docx images list or docx blocks",
                ),
                flag(
                    "--width",
                    "width",
                    "int",
                    "replacement width in EMUs; 0 keeps existing width",
                ),
                flag(
                    "--height",
                    "height",
                    "int",
                    "replacement height in EMUs; 0 keeps existing height",
                ),
                flag("--out", "out", "string", "output file path"),
                flag(
                    "--in-place",
                    "inPlace",
                    "bool",
                    "write the input file in place",
                ),
                flag("--backup", "backup", "string", "backup path for --in-place"),
                flag("--dry-run", "dryRun", "bool", "validate without writing"),
                flag(
                    "--no-validate",
                    "noValidate",
                    "bool",
                    "skip post-write validation",
                ),
            ],
            ExecutionSupport::DirectOnly {
                reason: Some(
                    "direct CLI mutation; serve/MCP operation support is not wired for image mutations yet",
                ),
            },
            None,
        ),
        spec(
            DocxCommandId::ImagesInsert,
            &["docx", "images", "insert"],
            "insert <file>",
            "Insert a new inline image paragraph into the DOCX main document body.",
            &["image", "paragraph"],
            vec![
                flag(
                    "--after",
                    "after",
                    "int",
                    "body block index to insert after; 0 inserts before the first block",
                ),
                flag("--file", "file", "string", "image file path to insert"),
                flag(
                    "--expect-hash",
                    "expectHash",
                    "string",
                    "required expected sha256: block hash when --after is greater than 0",
                ),
                flag("--width", "width", "int", "image width in EMUs"),
                flag("--height", "height", "int", "image height in EMUs"),
                flag("--out", "out", "string", "output file path"),
                flag(
                    "--in-place",
                    "inPlace",
                    "bool",
                    "write the input file in place",
                ),
                flag("--backup", "backup", "string", "backup path for --in-place"),
                flag("--dry-run", "dryRun", "bool", "validate without writing"),
                flag(
                    "--no-validate",
                    "noValidate",
                    "bool",
                    "skip post-write validation",
                ),
            ],
            ExecutionSupport::DirectOnly {
                reason: Some(
                    "direct CLI mutation; serve/MCP operation support is not wired for image mutations yet",
                ),
            },
            None,
        ),
        spec(
            DocxCommandId::Replace,
            &["docx", "replace"],
            "replace <file>",
            "Find and replace text across DOCX body text.",
            &["paragraph", "table"],
            vec![
                flag("--backup", "backup", "string", "backup path for --in-place"),
                flag("--dry-run", "dryRun", "bool", "validate without writing"),
                flag(
                    "--expect-count",
                    "expectCount",
                    "int",
                    "expected number of replacements; when set, errors if the actual count differs",
                ),
                flag(
                    "--find",
                    "find",
                    "string",
                    "text or regex pattern to find (required)",
                ),
                flag("--in-place", "inPlace", "bool", "write in place"),
                flag(
                    "--match-case",
                    "matchCase",
                    "bool",
                    "case-sensitive matching",
                ),
                flag(
                    "--no-validate",
                    "noValidate",
                    "bool",
                    "skip post-write validation",
                ),
                flag("--out", "out", "string", "output file path"),
                flag(
                    "--regex",
                    "regex",
                    "bool",
                    "treat --find as a regular expression",
                ),
                flag(
                    "--replace",
                    "replace",
                    "string",
                    "replacement text (inserted literally)",
                ),
                flag(
                    "--whole-word",
                    "wholeWord",
                    "bool",
                    "match whole words only",
                ),
            ],
            ExecutionSupport::ServeMutation { reason: None },
            None,
        ),
        spec(
            DocxCommandId::TablesShow,
            &["docx", "tables", "show"],
            "show <file>",
            "Show DOCX tables by table index, body block index, dimensions, merged-cell flag, and cell text.",
            &[],
            vec![
                flag(
                    "--details",
                    "details",
                    "bool",
                    "include detailed table object in JSON output",
                ),
                flag(
                    "--table",
                    "table",
                    "int",
                    "1-based table number; omitted shows all tables",
                ),
            ],
            ExecutionSupport::ServeInspect {
                reason: Some(
                    "read-only command; call via inspect in serve/MCP; generated table hashes feed hash-guarded DOCX table mutations",
                ),
            },
            None,
        ),
        spec(
            DocxCommandId::TablesCreate,
            &["docx", "tables", "create"],
            "create <file>",
            "Append one rectangular main-document DOCX table before section properties.",
            &["table"],
            vec![
                flag(
                    "--values",
                    "values",
                    "json",
                    "rectangular JSON matrix of strings, numbers, booleans, or nulls",
                ),
                flag(
                    "--values-file",
                    "valuesFile",
                    "string",
                    "path to a rectangular JSON matrix",
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
                    "skip post-write validation",
                ),
            ],
            ExecutionSupport::ServeMutation {
                reason: Some(
                    "append-only first-class authoring; values must be a rectangular JSON matrix",
                ),
            },
            None,
        ),
        spec(
            DocxCommandId::TablesSetCell,
            &["docx", "tables", "set-cell"],
            "set-cell <file>",
            "Set one main-document DOCX table cell's plain text.",
            &["table"],
            vec![
                flag("--table", "table", "int", "1-based table number"),
                flag("--row", "row", "int", "1-based table row"),
                flag("--col", "col", "int", "1-based table column"),
                flag(
                    "--expect-hash",
                    "expectHash",
                    "string",
                    "expected sha256: table block hash from docx tables show or docx blocks",
                ),
                flag("--text", "text", "string", "replacement cell text"),
                flag(
                    "--text-file",
                    "textFile",
                    "string",
                    "path to replacement cell text",
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
                    "skip post-write validation",
                ),
            ],
            ExecutionSupport::ServeMutation { reason: None },
            None,
        ),
        spec(
            DocxCommandId::TablesClearCell,
            &["docx", "tables", "clear-cell"],
            "clear-cell <file>",
            "Clear one main-document DOCX table cell's text.",
            &["table"],
            vec![
                flag("--table", "table", "int", "1-based table number"),
                flag("--row", "row", "int", "1-based table row"),
                flag("--col", "col", "int", "1-based table column"),
                flag(
                    "--expect-hash",
                    "expectHash",
                    "string",
                    "expected sha256: table block hash from docx tables show or docx blocks",
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
                    "skip post-write validation",
                ),
            ],
            ExecutionSupport::ServeMutation { reason: None },
            None,
        ),
        spec(
            DocxCommandId::TablesInsertRow,
            &["docx", "tables", "insert-row"],
            "insert-row <file>",
            "Insert one empty main-document DOCX table row.",
            &["table"],
            vec![
                flag("--table", "table", "int", "1-based table number"),
                flag("--at", "at", "int", "1-based row insertion position"),
                flag(
                    "--expect-hash",
                    "expectHash",
                    "string",
                    "expected sha256: table block hash from docx tables show or docx blocks",
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
                    "skip post-write validation",
                ),
            ],
            ExecutionSupport::ServeMutation { reason: None },
            None,
        ),
        spec(
            DocxCommandId::TablesDeleteRow,
            &["docx", "tables", "delete-row"],
            "delete-row <file>",
            "Delete one main-document DOCX table row.",
            &["table"],
            vec![
                flag("--table", "table", "int", "1-based table number"),
                flag("--row", "row", "int", "1-based table row"),
                flag(
                    "--expect-hash",
                    "expectHash",
                    "string",
                    "expected sha256: table block hash from docx tables show or docx blocks",
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
                    "skip post-write validation",
                ),
            ],
            ExecutionSupport::ServeMutation { reason: None },
            None,
        ),
    ]
}

#[allow(clippy::too_many_arguments)]
fn spec(
    id: DocxCommandId,
    path: &'static [&'static str],
    use_text: &'static str,
    short: &'static str,
    target_object_kinds: &'static [&'static str],
    local_flags: Vec<FlagSpec>,
    execution: ExecutionSupport,
    flag_constraints: Option<Value>,
) -> CommandSpec {
    CommandSpec {
        id: CommandId::Docx(id),
        path,
        use_text,
        short,
        target_object_kinds,
        local_flags,
        execution,
        flag_constraints,
    }
}

fn flag(
    name: &'static str,
    arg_name: &'static str,
    flag_type: &'static str,
    description: &'static str,
) -> FlagSpec {
    FlagSpec {
        name,
        arg_name,
        flag_type,
        description,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::command_manifest::{assert_segment_matches_legacy, capability_value};

    const SERVE_INSPECT_PATHS: &[&str] = &[
        "ooxml docx text",
        "ooxml docx blocks",
        "ooxml docx styles list",
        "ooxml docx styles show",
        "ooxml docx comments list",
        "ooxml docx fields list",
        "ooxml docx headers list",
        "ooxml docx headers show",
        "ooxml docx footers list",
        "ooxml docx footers show",
        "ooxml docx images list",
        "ooxml docx tables show",
    ];

    const SERVE_MUTATION_PATHS: &[&str] = &[
        "ooxml docx blocks replace",
        "ooxml docx blocks delete",
        "ooxml docx blocks insert-after",
        "ooxml docx paragraphs append",
        "ooxml docx paragraphs insert",
        "ooxml docx paragraphs set",
        "ooxml docx paragraphs clear",
        "ooxml docx styles apply",
        "ooxml docx comments add",
        "ooxml docx comments edit",
        "ooxml docx comments remove",
        "ooxml docx fields insert",
        "ooxml docx fields set-result",
        "ooxml docx headers set-text",
        "ooxml docx footers set-text",
        "ooxml docx replace",
        "ooxml docx tables create",
        "ooxml docx tables set-cell",
        "ooxml docx tables clear-cell",
        "ooxml docx tables insert-row",
        "ooxml docx tables delete-row",
    ];

    #[test]
    fn complete_docx_segment_matches_fixed_legacy_slice_and_root_placement() {
        let specs = command_specs();
        let legacy = crate::capabilities::capability_commands();
        let root = crate::command_manifest::command_specs();
        let start = crate::command_manifest::core::command_specs().len()
            + crate::command_manifest::pptx::command_specs().len()
            + crate::command_manifest::xlsx::command_specs().len();
        assert_eq!(start, LEGACY_START);
        assert_eq!(specs.len(), COMMAND_COUNT);
        assert_segment_matches_legacy(&specs, &legacy[LEGACY_START..LEGACY_START + COMMAND_COUNT]);
        assert_eq!(
            root[start..start + COMMAND_COUNT]
                .iter()
                .map(|spec| spec.id)
                .collect::<Vec<_>>(),
            specs.iter().map(|spec| spec.id).collect::<Vec<_>>()
        );
    }

    #[test]
    fn docx_ids_paths_and_repeated_builds_are_unique_stable() {
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
    }

    #[test]
    fn complete_docx_shadow_has_expected_execution_inventory() {
        let specs = command_specs();
        let inventory = specs.iter().fold(
            (0, 0, 0, 0),
            |(groups, direct, inspect, mutation), spec| match &spec.execution {
                ExecutionSupport::GroupOnly { .. } => (groups + 1, direct, inspect, mutation),
                ExecutionSupport::DirectOnly { .. } => (groups, direct + 1, inspect, mutation),
                ExecutionSupport::ServeInspect { .. } => (groups, direct, inspect + 1, mutation),
                ExecutionSupport::ServeMutation { .. } => (groups, direct, inspect, mutation + 1),
            },
        );
        assert_eq!(inventory, (9, 3, 12, 21));
        assert_eq!(
            specs
                .iter()
                .filter(|spec| match &spec.execution {
                    ExecutionSupport::ServeInspect {
                        reason: Some(reason),
                    } => reason.contains("call via inspect in serve/MCP"),
                    _ => false,
                })
                .count(),
            1
        );
    }

    #[test]
    fn docx_serve_inspect_classification_matches_independent_dispatch_oracle() {
        let actual = command_specs()
            .iter()
            .filter(|spec| matches!(&spec.execution, ExecutionSupport::ServeInspect { .. }))
            .filter_map(|spec| capability_value(spec)["path"].as_str().map(str::to_owned))
            .collect::<BTreeSet<_>>();
        let expected = SERVE_INSPECT_PATHS
            .iter()
            .map(|path| (*path).to_owned())
            .collect::<BTreeSet<_>>();
        assert_eq!(actual, expected);
        assert_eq!(actual.len(), 12);
    }

    #[test]
    fn docx_serve_mutations_match_legacy_op_compatible_set_and_advisory() {
        let specs = command_specs();
        let legacy = crate::capabilities::capability_commands();
        let expected = legacy[LEGACY_START..LEGACY_START + COMMAND_COUNT]
            .iter()
            .filter(|command| command["opCompatible"] == true)
            .filter_map(|command| command["path"].as_str().map(str::to_owned))
            .collect::<BTreeSet<_>>();
        let actual = specs
            .iter()
            .filter(|spec| matches!(&spec.execution, ExecutionSupport::ServeMutation { .. }))
            .filter_map(|spec| capability_value(spec)["path"].as_str().map(str::to_owned))
            .collect::<BTreeSet<_>>();
        let dispatch_oracle = SERVE_MUTATION_PATHS
            .iter()
            .map(|path| (*path).to_owned())
            .collect::<BTreeSet<_>>();
        assert_eq!(actual, expected);
        assert_eq!(actual, dispatch_oracle);
        assert_eq!(actual.len(), 21);
        let advisories = specs
            .iter()
            .filter_map(|spec| match &spec.execution {
                ExecutionSupport::ServeMutation {
                    reason: Some(reason),
                } => Some((spec.path, *reason)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            advisories,
            vec![(
                (&["docx", "tables", "create"] as &[&str]),
                "append-only first-class authoring; values must be a rectangular JSON matrix"
            )]
        );
    }
}
