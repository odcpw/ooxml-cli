use serde_json::Value;

use super::{capability_command, flag};

pub(super) fn commands() -> Vec<Value> {
    vec![
        capability_command(
            "ooxml vba inspect",
            "inspect <file>",
            "Inspect opaque VBA package state for XLSM/PPTM package wiring.",
            &["package", "module"],
            false,
            Some("read-only command; use vba attach/remove for package mutation"),
            vec![],
        ),
        capability_command(
            "ooxml vba extract-bin",
            "extract-bin <file>",
            "Extract opaque vbaProject.bin bytes.",
            &["package", "module"],
            false,
            Some("read-only binary extraction command"),
            vec![flag("--out", "out", "string", "output vbaProject.bin path")],
        ),
        capability_command(
            "ooxml vba attach",
            "attach <file>",
            "Attach or replace opaque vbaProject.bin and macro package wiring.",
            &["package", "module"],
            true,
            None,
            vec![
                flag(
                    "--allow-host-family-risk",
                    "allowHostFamilyRisk",
                    "bool",
                    "accepted for Go CLI compatibility; opaque Rust attach does not parse source-project host risk yet",
                ),
                flag(
                    "--backup",
                    "backup",
                    "string",
                    "backup file path for --in-place",
                ),
                flag("--bin", "bin", "string", "vbaProject.bin to attach"),
                flag(
                    "--dry-run",
                    "dryRun",
                    "bool",
                    "validate mutation without writing an output file",
                ),
                flag(
                    "--in-place",
                    "inPlace",
                    "bool",
                    "modify the input file in place",
                ),
                flag(
                    "--no-validate",
                    "noValidate",
                    "bool",
                    "skip validation after mutation",
                ),
                flag("--out", "out", "string", "output file path"),
            ],
        ),
        capability_command(
            "ooxml vba remove",
            "remove <file>",
            "Remove opaque VBA package wiring and restore non-macro main content type.",
            &["package", "module"],
            true,
            None,
            vec![
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
                    "validate mutation without writing an output file",
                ),
                flag(
                    "--in-place",
                    "inPlace",
                    "bool",
                    "modify the input file in place",
                ),
                flag(
                    "--no-validate",
                    "noValidate",
                    "bool",
                    "skip validation after mutation",
                ),
                flag("--out", "out", "string", "output file path"),
            ],
        ),
    ]
}
