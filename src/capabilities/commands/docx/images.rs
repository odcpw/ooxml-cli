use serde_json::Value;

use super::super::{capability_command, flag};

pub(super) fn commands() -> Vec<Value> {
    vec![
        capability_command(
            "ooxml docx images list",
            "list <file>",
            "List inline images in a DOCX document.",
            &["image", "paragraph"],
            false,
            Some(
                "read-only command; image records include relationship ids, media parts, dimensions, and block anchors",
            ),
            vec![],
        ),
        capability_command(
            "ooxml docx images replace",
            "replace <file>",
            "Replace one inline DOCX image payload and optionally resize the drawing.",
            &["image"],
            false,
            Some(
                "direct CLI mutation; serve/MCP operation support is not wired for image mutations yet",
            ),
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
        ),
        capability_command(
            "ooxml docx images insert",
            "insert <file>",
            "Insert a new inline image paragraph into the DOCX main document body.",
            &["image", "paragraph"],
            false,
            Some(
                "direct CLI mutation; serve/MCP operation support is not wired for image mutations yet",
            ),
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
        ),
    ]
}
