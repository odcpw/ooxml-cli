mod charts;
mod comments;
mod conditional_formatting;
mod data_validations;
mod freeze;
mod links_filters;
mod names;
mod pivots_workbook;
mod ranges_cells;
mod structure;
mod tables;

use serde_json::Value;

use super::{capability_command, flag};

const COMMAND_GROUP_REASON: &str = "it is a command group, not a leaf mutation command";

pub(super) fn commands() -> Vec<Value> {
    let mut commands = group_commands();
    commands.extend(scaffold_commands());
    commands.extend(structure::commands());
    commands.extend(charts::commands());
    commands.extend(comments::commands());
    commands.extend(conditional_formatting::commands());
    commands.extend(data_validations::commands());
    commands.extend(links_filters::commands());
    commands.extend(names::commands());
    commands.extend(tables::commands());
    commands.extend(pivots_workbook::commands());
    commands.extend(ranges_cells::commands());
    commands.extend(freeze::commands());
    commands
}

fn scaffold_commands() -> Vec<Value> {
    vec![capability_command(
        "ooxml xlsx scaffold",
        "scaffold <output.xlsx> (or --out <output.xlsx>)",
        "Create a minimal XLSX workbook from scratch and validate it by default.",
        &["package", "sheet"],
        false,
        Some("it creates a package and is not an apply/serve mutation op"),
        vec![
            flag(
                "--out",
                "out",
                "string",
                "output workbook path; accepted as an alternative to positional <output.xlsx>",
            ),
            flag("--sheet", "sheet", "string", "initial worksheet name"),
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
    )]
}

fn group_commands() -> Vec<Value> {
    vec![
        command_group("ooxml xlsx", "xlsx", "Work with XLSX workbooks"),
        command_group(
            "ooxml xlsx cells",
            "cells",
            "Read and mutate worksheet cells",
        ),
        command_group("ooxml xlsx charts", "charts", "Inspect workbook charts"),
        command_group(
            "ooxml xlsx cols",
            "cols",
            "Insert and delete worksheet columns",
        ),
        command_group(
            "ooxml xlsx colwidths",
            "colwidths",
            "Inspect and set worksheet column widths",
        ),
        command_group(
            "ooxml xlsx comments",
            "comments",
            "Inspect and mutate XLSX cell comments (legacy notes)",
        ),
        command_group(
            "ooxml xlsx conditional-formats",
            "conditional-formats",
            "Inspect and mutate worksheet conditional formatting",
        ),
        command_group(
            "ooxml xlsx data-validations",
            "data-validations",
            "Inspect and mutate worksheet data validations",
        ),
        command_group(
            "ooxml xlsx filters-sorts",
            "filters-sorts",
            "Auto-filter and sort for table/range workflows",
        ),
        command_group(
            "ooxml xlsx freeze",
            "freeze",
            "Inspect and set worksheet freeze panes",
        ),
        command_group(
            "ooxml xlsx hyperlinks",
            "hyperlinks",
            "Inspect and mutate worksheet hyperlinks",
        ),
        command_group(
            "ooxml xlsx names",
            "names",
            "Inspect and mutate workbook defined names",
        ),
        command_group(
            "ooxml xlsx pivots",
            "pivots",
            "Inspect workbook PivotTables",
        ),
        command_group(
            "ooxml xlsx ranges",
            "ranges",
            "Export and set rectangular worksheet ranges",
        ),
        command_group(
            "ooxml xlsx rowheights",
            "rowheights",
            "Inspect and set worksheet row heights",
        ),
        command_group(
            "ooxml xlsx rows",
            "rows",
            "Insert and delete worksheet rows",
        ),
        command_group(
            "ooxml xlsx sheets",
            "sheets",
            "Inspect and mutate workbook sheets",
        ),
        command_group(
            "ooxml xlsx tables",
            "tables",
            "Inspect and mutate workbook tables",
        ),
        command_group(
            "ooxml xlsx workbook",
            "workbook",
            "Workbook-level operations",
        ),
        command_group(
            "ooxml xlsx workbook metadata",
            "metadata",
            "Inspect and update workbook metadata and calc settings",
        ),
    ]
}

fn command_group(path: &str, use_text: &str, short: &str) -> Value {
    capability_command(
        path,
        use_text,
        short,
        &[],
        false,
        Some(COMMAND_GROUP_REASON),
        vec![],
    )
}

fn xlsx_chart_fill_flags() -> Vec<Value> {
    vec![
        flag("--sheet", "sheet", "string", "sheet selector"),
        flag("--chart", "chart", "string", "chart selector"),
        flag(
            "--fill-color",
            "fillColor",
            "string",
            "fill color as #RRGGBB, or none",
        ),
        flag(
            "--expect-fill",
            "expectFill",
            "string",
            "guard: current fill color, scheme:<name>, or none",
        ),
        flag("--out", "out", "string", "write edited workbook"),
        flag(
            "--in-place",
            "inPlace",
            "bool",
            "edit the workbook in place",
        ),
        flag("--backup", "backup", "string", "backup path for --in-place"),
        flag("--dry-run", "dryRun", "bool", "validate without writing"),
        flag(
            "--no-validate",
            "noValidate",
            "bool",
            "skip strict validation",
        ),
    ]
}

fn xlsx_data_validation_mutation_flags(include_guards: bool) -> Vec<Value> {
    let mut flags = vec![
        flag("--sheet", "sheet", "string", "sheet selector"),
        flag(
            "--range",
            "range",
            "string",
            "target sqref; space-separated ranges are accepted",
        ),
        flag(
            "--type",
            "type",
            "string",
            "validation type: list, whole, decimal, date, time, textLength, or custom",
        ),
        flag(
            "--list-values",
            "listValues",
            "string",
            "comma-separated inline values for list validations",
        ),
        flag(
            "--list-range",
            "listRange",
            "string",
            "worksheet range source for list validations",
        ),
        flag(
            "--operator",
            "operator",
            "string",
            "operator such as between, equal, greaterThan, or lessThanOrEqual",
        ),
        flag("--formula1", "formula1", "string", "first formula or bound"),
        flag(
            "--formula2",
            "formula2",
            "string",
            "second formula or bound for between/notBetween",
        ),
        flag("--allow-blank", "allowBlank", "bool", "allow blank cells"),
        flag(
            "--show-input-message",
            "showInputMessage",
            "bool",
            "show the input prompt",
        ),
        flag(
            "--input-title",
            "inputTitle",
            "string",
            "input prompt title",
        ),
        flag(
            "--input-message",
            "inputMessage",
            "string",
            "input prompt message",
        ),
        flag(
            "--show-error-message",
            "showErrorMessage",
            "bool",
            "show the error alert",
        ),
        flag("--error-title", "errorTitle", "string", "error alert title"),
        flag(
            "--error-message",
            "errorMessage",
            "string",
            "error alert message",
        ),
        flag(
            "--error-style",
            "errorStyle",
            "string",
            "error alert style: stop, warning, or information",
        ),
    ];
    if include_guards {
        flags.push(flag(
            "--expect-type",
            "expectType",
            "string",
            "guard: require the current validation type to match",
        ));
        flags.push(flag(
            "--expect-formula1",
            "expectFormula1",
            "string",
            "guard: require the current formula1 to match",
        ));
    }
    flags.extend([
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
        flag("--dry-run", "dryRun", "bool", "validate without writing"),
        flag(
            "--no-validate",
            "noValidate",
            "bool",
            "skip post-write validation",
        ),
    ]);
    flags
}
