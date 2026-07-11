use super::{ExecutionSupport, XlsxCommandId, flag, spec};

pub(super) const COMMAND_COUNT: usize = 12;
pub(super) const LEGACY_START: usize = 206;

pub(super) fn command_specs() -> Vec<super::CommandSpec> {
    vec![
        spec(
            XlsxCommandId::HyperlinksList,
            &["xlsx", "hyperlinks", "list"],
            "list <file> [--sheet <sheet>]",
            "List worksheet hyperlinks with stable cell or range selectors.",
            &["hyperlink", "cell", "range", "sheet"],
            vec![
                flag("--sheet", "sheet", "string", "sheet selector"),
                flag(
                    "--include-broken",
                    "includeBroken",
                    "bool",
                    "only return broken external hyperlinks",
                ),
            ],
            ExecutionSupport::ServeInspect {
                reason: Some("read-only command; call via inspect in serve/MCP"),
            },
            None,
        ),
        spec(
            XlsxCommandId::HyperlinksShow,
            &["xlsx", "hyperlinks", "show"],
            "show <file> --sheet <sheet> --cell <A1>",
            "Show the hyperlink attached to a worksheet cell or range ref.",
            &["hyperlink", "cell", "range", "sheet"],
            vec![
                flag("--sheet", "sheet", "string", "sheet selector"),
                flag("--cell", "cell", "string", "cell or range ref such as B2"),
            ],
            ExecutionSupport::ServeInspect {
                reason: Some("read-only command; call via inspect in serve/MCP"),
            },
            None,
        ),
        spec(
            XlsxCommandId::HyperlinksAdd,
            &["xlsx", "hyperlinks", "add"],
            "add <file> --sheet <sheet> --cell <A1> (--url <url>|--location <ref>)",
            "Add an internal or external worksheet hyperlink.",
            &["hyperlink", "cell", "range", "sheet"],
            vec![
                flag("--sheet", "sheet", "string", "sheet selector"),
                flag("--cell", "cell", "string", "cell or range ref such as B2"),
                flag("--url", "url", "string", "external hyperlink target"),
                flag(
                    "--location",
                    "location",
                    "string",
                    "internal workbook hyperlink target",
                ),
                flag("--display", "display", "string", "optional display text"),
                flag("--tooltip", "tooltip", "string", "optional tooltip text"),
                flag(
                    "--replace",
                    "replace",
                    "bool",
                    "replace an existing hyperlink on the ref",
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
            XlsxCommandId::HyperlinksUpdate,
            &["xlsx", "hyperlinks", "update"],
            "update <file> --sheet <sheet> --cell <A1>",
            "Update an existing worksheet hyperlink with optional stale-target guards.",
            &["hyperlink", "cell", "range", "sheet"],
            vec![
                flag("--sheet", "sheet", "string", "sheet selector"),
                flag("--cell", "cell", "string", "cell or range ref such as B2"),
                flag("--url", "url", "string", "new external hyperlink target"),
                flag(
                    "--location",
                    "location",
                    "string",
                    "new internal workbook hyperlink target",
                ),
                flag("--display", "display", "string", "replacement display text"),
                flag("--tooltip", "tooltip", "string", "replacement tooltip text"),
                flag(
                    "--expect-url",
                    "expectUrl",
                    "string",
                    "guard: expected current external target",
                ),
                flag(
                    "--expect-location",
                    "expectLocation",
                    "string",
                    "guard: expected current internal target",
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
            XlsxCommandId::HyperlinksDelete,
            &["xlsx", "hyperlinks", "delete"],
            "delete <file> --sheet <sheet> --cell <A1>",
            "Delete an existing worksheet hyperlink with optional stale-target guards.",
            &["hyperlink", "cell", "range", "sheet"],
            vec![
                flag("--sheet", "sheet", "string", "sheet selector"),
                flag("--cell", "cell", "string", "cell or range ref such as B2"),
                flag(
                    "--expect-url",
                    "expectUrl",
                    "string",
                    "guard: expected current external target",
                ),
                flag(
                    "--expect-location",
                    "expectLocation",
                    "string",
                    "guard: expected current internal target",
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
            XlsxCommandId::FiltersSortsShow,
            &["xlsx", "filters-sorts", "show"],
            "show <file> [--sheet <sheet>] [--table <table>]",
            "Display worksheet or table autoFilter state and worksheet sortState.",
            &["sheet", "range", "table"],
            vec![
                flag("--sheet", "sheet", "string", "sheet selector"),
                flag(
                    "--range",
                    "range",
                    "string",
                    "informational range hint; state is read from worksheet/table XML",
                ),
                flag("--table", "table", "string", "table selector"),
            ],
            ExecutionSupport::ServeInspect {
                reason: Some("read-only command; call via inspect in serve/MCP"),
            },
            None,
        ),
        spec(
            XlsxCommandId::FiltersSortsSetAutofilter,
            &["xlsx", "filters-sorts", "set-autofilter"],
            "set-autofilter <file> (--range <A1:D10>|--table <table>)",
            "Add or replace a worksheet or table autoFilter ref.",
            &["sheet", "range", "table"],
            vec![
                flag("--sheet", "sheet", "string", "sheet selector"),
                flag("--range", "range", "string", "worksheet autoFilter range"),
                flag("--table", "table", "string", "table selector"),
                flag(
                    "--expect-range",
                    "expectRange",
                    "string",
                    "guard: require the current autoFilter ref to match",
                ),
                flag("--out", "out", "string", "output file path"),
                flag("--in-place", "inPlace", "bool", "write in place"),
                flag("--backup", "backup", "string", "backup path for --in-place"),
                flag("--dry-run", "dryRun", "bool", "plan without writing"),
                flag(
                    "--no-validate",
                    "noValidate",
                    "bool",
                    "skip post-write validation",
                ),
            ],
            ExecutionSupport::DirectOnly {
                reason: Some("direct CLI parity only in the current Rust slice"),
            },
            None,
        ),
        spec(
            XlsxCommandId::FiltersSortsClearAutofilter,
            &["xlsx", "filters-sorts", "clear-autofilter"],
            "clear-autofilter <file> [--sheet <sheet>] [--table <table>]",
            "Remove a worksheet or table autoFilter.",
            &["sheet", "range", "table"],
            vec![
                flag("--sheet", "sheet", "string", "sheet selector"),
                flag(
                    "--range",
                    "range",
                    "string",
                    "accepted for CLI compatibility; current state is read from the worksheet/table",
                ),
                flag("--table", "table", "string", "table selector"),
                flag(
                    "--expect-range",
                    "expectRange",
                    "string",
                    "guard: require the current autoFilter ref to match",
                ),
                flag("--out", "out", "string", "output file path"),
                flag("--in-place", "inPlace", "bool", "write in place"),
                flag("--backup", "backup", "string", "backup path for --in-place"),
                flag("--dry-run", "dryRun", "bool", "plan without writing"),
                flag(
                    "--no-validate",
                    "noValidate",
                    "bool",
                    "skip post-write validation",
                ),
            ],
            ExecutionSupport::DirectOnly {
                reason: Some("direct CLI parity only in the current Rust slice"),
            },
            None,
        ),
        spec(
            XlsxCommandId::FiltersSortsAddColumnFilter,
            &["xlsx", "filters-sorts", "add-column-filter"],
            "add-column-filter <file> --column <colId> (--values <a,b>|--custom-op <op>)",
            "Add values and/or custom criteria to one worksheet autoFilter column.",
            &["sheet", "range"],
            vec![
                flag("--sheet", "sheet", "string", "sheet selector"),
                flag(
                    "--column",
                    "column",
                    "number",
                    "0-based column offset within the autoFilter ref",
                ),
                flag(
                    "--values",
                    "values",
                    "string",
                    "comma-separated filter values",
                ),
                flag(
                    "--custom-op",
                    "customOp",
                    "string",
                    "custom operator: equal|notEqual|lessThan|lessThanOrEqual|greaterThan|greaterThanOrEqual|between|notBetween",
                ),
                flag(
                    "--custom-val1",
                    "customVal1",
                    "string",
                    "first custom criterion value",
                ),
                flag(
                    "--custom-val2",
                    "customVal2",
                    "string",
                    "second custom criterion value for between/notBetween",
                ),
                flag(
                    "--expect-filter",
                    "expectFilter",
                    "string",
                    "guard: require current column filter summary to match",
                ),
                flag("--out", "out", "string", "output file path"),
                flag("--in-place", "inPlace", "bool", "write in place"),
                flag("--backup", "backup", "string", "backup path for --in-place"),
                flag("--dry-run", "dryRun", "bool", "plan without writing"),
                flag(
                    "--no-validate",
                    "noValidate",
                    "bool",
                    "skip post-write validation",
                ),
            ],
            ExecutionSupport::DirectOnly {
                reason: Some("direct CLI parity only in the current Rust slice"),
            },
            None,
        ),
        spec(
            XlsxCommandId::FiltersSortsClearColumnFilter,
            &["xlsx", "filters-sorts", "clear-column-filter"],
            "clear-column-filter <file> --column <colId>",
            "Remove one worksheet autoFilter column criterion.",
            &["sheet", "range"],
            vec![
                flag("--sheet", "sheet", "string", "sheet selector"),
                flag(
                    "--column",
                    "column",
                    "int",
                    "0-based column offset within the autoFilter ref",
                ),
                flag("--out", "out", "string", "output file path"),
                flag("--in-place", "inPlace", "bool", "write in place"),
                flag("--backup", "backup", "string", "backup path for --in-place"),
                flag("--dry-run", "dryRun", "bool", "plan without writing"),
                flag(
                    "--no-validate",
                    "noValidate",
                    "bool",
                    "skip post-write validation",
                ),
            ],
            ExecutionSupport::DirectOnly {
                reason: Some("direct CLI parity only in the current Rust slice"),
            },
            None,
        ),
        spec(
            XlsxCommandId::FiltersSortsSetSort,
            &["xlsx", "filters-sorts", "set-sort"],
            "set-sort <file> --ref <A1:D10> --column <A>",
            "Add or replace one worksheet sortState condition.",
            &["sheet", "range"],
            vec![
                flag("--sheet", "sheet", "string", "sheet selector"),
                flag("--ref", "ref", "string", "sortState range such as A1:D10"),
                flag("--column", "column", "string", "column letter to sort by"),
                flag("--descending", "descending", "bool", "sort descending"),
                flag(
                    "--expect-sort",
                    "expectSort",
                    "string",
                    "guard: require current sortState ref to match",
                ),
                flag("--out", "out", "string", "output file path"),
                flag("--in-place", "inPlace", "bool", "write in place"),
                flag("--backup", "backup", "string", "backup path for --in-place"),
                flag("--dry-run", "dryRun", "bool", "plan without writing"),
                flag(
                    "--no-validate",
                    "noValidate",
                    "bool",
                    "skip post-write validation",
                ),
            ],
            ExecutionSupport::DirectOnly {
                reason: Some("direct CLI parity only in the current Rust slice"),
            },
            None,
        ),
        spec(
            XlsxCommandId::FiltersSortsClearSort,
            &["xlsx", "filters-sorts", "clear-sort"],
            "clear-sort <file> [--sheet <sheet>]",
            "Remove the worksheet sortState.",
            &["sheet", "range"],
            vec![
                flag("--sheet", "sheet", "string", "sheet selector"),
                flag("--out", "out", "string", "output file path"),
                flag("--in-place", "inPlace", "bool", "write in place"),
                flag("--backup", "backup", "string", "backup path for --in-place"),
                flag("--dry-run", "dryRun", "bool", "plan without writing"),
                flag(
                    "--no-validate",
                    "noValidate",
                    "bool",
                    "skip post-write validation",
                ),
            ],
            ExecutionSupport::DirectOnly {
                reason: Some("direct CLI parity only in the current Rust slice"),
            },
            None,
        ),
    ]
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::command_manifest::{
        assert_segment_matches_frozen_contract, capability_value, frozen_contract_commands,
    };

    #[test]
    fn links_filters_segment_matches_frozen_contract_slice() {
        let specs = command_specs();
        let frozen = frozen_contract_commands();
        assert_eq!(specs.len(), COMMAND_COUNT);
        assert_segment_matches_frozen_contract(
            &specs,
            &frozen[LEGACY_START..LEGACY_START + COMMAND_COUNT],
        );
    }

    #[test]
    fn links_filters_ids_paths_builds_and_execution_inventory_are_stable() {
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
        assert_eq!(inventory, (0, 6, 3, 3));
    }
}
