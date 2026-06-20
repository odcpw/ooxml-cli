use serde_json::Value;

use super::{capability_command, flag};

pub(super) fn commands() -> Vec<Value> {
    vec![
        capability_command(
            "ooxml version",
            "version",
            "Print the version of ooxml.",
            &[],
            false,
            Some("read-only metadata command"),
            vec![],
        ),
        capability_command(
            "ooxml capabilities",
            "capabilities [--for <filter>]",
            "Emit the Rust-supported machine-readable command and object inventory.",
            &[],
            false,
            Some("read-only self-description command"),
            vec![flag(
                "--for",
                "for",
                "string",
                "filter commands by supported command family or object kind",
            )],
        ),
        capability_command(
            "ooxml apply",
            "apply <file> --ops <ops.json> (--out <file>|--in-place|--dry-run)",
            "Apply a JSON batch of mutation operations to one OOXML package through the same session engine used by serve and MCP.",
            &["package"],
            false,
            Some("batch command owns operation dispatch; do not nest inside serve/MCP op"),
            vec![
                flag(
                    "--ops",
                    "ops",
                    "string",
                    "JSON file containing an array of operations",
                ),
                flag("--out", "out", "string", "output package path"),
                flag(
                    "--backup",
                    "backup",
                    "string",
                    "backup path when used with --in-place",
                ),
                flag(
                    "--dry-run",
                    "dryRun",
                    "bool",
                    "return the operation plan without writing output",
                ),
                flag(
                    "--in-place",
                    "inPlace",
                    "bool",
                    "write the final package back to the input file",
                ),
                flag(
                    "--no-validate",
                    "noValidate",
                    "bool",
                    "skip strict validation before publishing output",
                ),
            ],
        ),
        capability_command(
            "ooxml serve",
            "serve",
            "Run the JSON-RPC 2.0 stdio session server for web and agent workflows.",
            &[],
            false,
            Some("stdio session server; use JSON-RPC methods instead of op argv"),
            vec![],
        ),
        capability_command(
            "ooxml mcp",
            "mcp",
            "Run the MCP stdio server backed by the same Rust session engine.",
            &[],
            false,
            Some("MCP stdio server; use MCP tools/resources instead of op argv"),
            vec![],
        ),
        capability_command(
            "ooxml inspect",
            "inspect <file>",
            "Inspect a supported OOXML package.",
            &["package"],
            false,
            Some("read-only command; use inspect_current_with_ooxml through serve"),
            vec![],
        ),
        capability_command(
            "ooxml validate",
            "validate <file>",
            "Validate an OOXML package.",
            &["package"],
            false,
            Some("read-only validation command"),
            vec![],
        ),
        capability_command(
            "ooxml verify",
            "verify <file>",
            "Validate and compare a package against a baseline where supported.",
            &["package"],
            false,
            Some("read-only verification command"),
            vec![flag(
                "--baseline",
                "baseline",
                "string",
                "baseline file to compare against",
            )],
        ),
    ]
}
