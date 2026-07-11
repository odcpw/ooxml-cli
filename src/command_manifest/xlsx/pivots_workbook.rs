use super::{ExecutionSupport, XlsxCommandId, flag, spec};

pub(super) const COMMAND_COUNT: usize = 5;
pub(super) const LEGACY_START: usize = 232;

pub(super) fn command_specs() -> Vec<super::CommandSpec> {
    vec![
        spec(
            XlsxCommandId::PivotsList,
            &["xlsx", "pivots", "list"],
            "list <file>",
            "List workbook PivotTables, cache sources, selectors, and generated readback commands.",
            &["pivot", "sheet", "range", "table"],
            vec![flag("--sheet", "sheet", "string", "sheet selector")],
            ExecutionSupport::DirectOnly {
                reason: Some("read-only command; call via inspect/list in direct CLI"),
            },
            None,
        ),
        spec(
            XlsxCommandId::PivotsShow,
            &["xlsx", "pivots", "show"],
            "show <file>",
            "Show one PivotTable with cache fields, source range, axis fields, and selectors.",
            &["pivot", "sheet", "range", "table"],
            vec![
                flag("--sheet", "sheet", "string", "sheet selector"),
                flag(
                    "--pivot",
                    "pivot",
                    "string",
                    "pivot selector from pivots list",
                ),
            ],
            ExecutionSupport::DirectOnly {
                reason: Some("read-only command; call via inspect/list in direct CLI"),
            },
            None,
        ),
        spec(
            XlsxCommandId::PivotsCreate,
            &["xlsx", "pivots", "create"],
            "create <file>",
            "Author a new PivotTable from a source table or worksheet range.",
            &["pivot", "sheet", "range", "table"],
            vec![
                flag(
                    "--sheet",
                    "sheet",
                    "string",
                    "source sheet selector for --range or table disambiguation",
                ),
                flag(
                    "--range",
                    "range",
                    "string",
                    "source A1 range with a header row",
                ),
                flag("--table", "table", "string", "source table selector"),
                flag(
                    "--target-sheet",
                    "targetSheet",
                    "string",
                    "sheet to place the pivot on; defaults to the source sheet",
                ),
                flag(
                    "--anchor",
                    "anchor",
                    "string",
                    "top-left pivot anchor cell; defaults right of the source range",
                ),
                flag("--name", "name", "string", "pivot table name"),
                flag(
                    "--rows",
                    "rows",
                    "string",
                    "comma-separated row field names",
                ),
                flag(
                    "--cols",
                    "cols",
                    "string",
                    "comma-separated column field names",
                ),
                flag(
                    "--filters",
                    "filters",
                    "string",
                    "comma-separated filter/page field names",
                ),
                flag(
                    "--values",
                    "values",
                    "string",
                    "comma-separated value field specs such as Amount or Amount:sum",
                ),
                flag(
                    "--expect-source-range",
                    "expectSourceRange",
                    "string",
                    "guard: require the resolved source range to match",
                ),
                flag(
                    "--max-cells",
                    "maxCells",
                    "number",
                    "maximum source cells to read",
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
            ExecutionSupport::DirectOnly {
                reason: Some(
                    "direct CLI mutation; serve/MCP operation dispatch is not wired for this command",
                ),
            },
            None,
        ),
        spec(
            XlsxCommandId::WorkbookMetadataInspect,
            &["xlsx", "workbook", "metadata", "inspect"],
            "inspect <file>",
            "Inspect workbook core/app properties and calc settings.",
            &["package"],
            vec![],
            ExecutionSupport::ServeInspect {
                reason: Some(
                    "it does not accept the mutation output flags injected by the op engine",
                ),
            },
            None,
        ),
        spec(
            XlsxCommandId::WorkbookMetadataUpdate,
            &["xlsx", "workbook", "metadata", "update"],
            "update <file>",
            "Update workbook metadata fields and calc settings.",
            &["package"],
            vec![
                flag(
                    "--backup",
                    "backup",
                    "string",
                    "backup file path for --in-place",
                ),
                flag(
                    "--calc-mode",
                    "calcMode",
                    "string",
                    "set workbook calcMode (auto|manual|autoNoTable)",
                ),
                flag(
                    "--category",
                    "category",
                    "string",
                    "set core property category",
                ),
                flag("--company", "company", "string", "set app property Company"),
                flag(
                    "--creator",
                    "creator",
                    "string",
                    "set core property dc:creator",
                ),
                flag(
                    "--description",
                    "description",
                    "string",
                    "set core property dc:description",
                ),
                flag(
                    "--dry-run",
                    "dryRun",
                    "bool",
                    "validate mutation without writing",
                ),
                flag(
                    "--expect-category",
                    "expectCategory",
                    "string",
                    "guard: require current category to equal this value",
                ),
                flag(
                    "--expect-company",
                    "expectCompany",
                    "string",
                    "guard: require current company to equal this value",
                ),
                flag(
                    "--expect-creator",
                    "expectCreator",
                    "string",
                    "guard: require current creator to equal this value",
                ),
                flag(
                    "--expect-description",
                    "expectDescription",
                    "string",
                    "guard: require current description to equal this value",
                ),
                flag(
                    "--expect-keywords",
                    "expectKeywords",
                    "string",
                    "guard: require current keywords to equal this value",
                ),
                flag(
                    "--expect-last-modified-by",
                    "expectLastModifiedBy",
                    "string",
                    "guard: require current lastModifiedBy to equal this value",
                ),
                flag(
                    "--expect-manager",
                    "expectManager",
                    "string",
                    "guard: require current manager to equal this value",
                ),
                flag(
                    "--expect-subject",
                    "expectSubject",
                    "string",
                    "guard: require current subject to equal this value",
                ),
                flag(
                    "--expect-title",
                    "expectTitle",
                    "string",
                    "guard: require current title to equal this value",
                ),
                flag(
                    "--full-calc-on-load",
                    "fullCalcOnLoad",
                    "bool",
                    "set fullCalcOnLoad and forceFullCalc so Excel recalculates on open",
                ),
                flag(
                    "--in-place",
                    "inPlace",
                    "bool",
                    "write the input file in place",
                ),
                flag(
                    "--keywords",
                    "keywords",
                    "string",
                    "set core property keywords",
                ),
                flag(
                    "--last-modified-by",
                    "lastModifiedBy",
                    "string",
                    "set core property lastModifiedBy",
                ),
                flag("--manager", "manager", "string", "set app property Manager"),
                flag(
                    "--no-validate",
                    "noValidate",
                    "bool",
                    "skip validation after mutation",
                ),
                flag("--out", "out", "string", "output file path"),
                flag(
                    "--subject",
                    "subject",
                    "string",
                    "set core property dc:subject",
                ),
                flag("--title", "title", "string", "set core property dc:title"),
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
    fn pivots_workbook_segment_matches_fixed_legacy_slice() {
        let specs = command_specs();
        let legacy = crate::capabilities::legacy_capability_commands();
        assert_eq!(specs.len(), COMMAND_COUNT);
        assert_segment_matches_legacy(&specs, &legacy[LEGACY_START..LEGACY_START + COMMAND_COUNT]);
    }

    #[test]
    fn pivots_workbook_ids_paths_builds_and_execution_inventory_are_stable() {
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
        assert_eq!(inventory, (0, 3, 1, 1));
    }
}
