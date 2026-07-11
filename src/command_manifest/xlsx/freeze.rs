use super::{ExecutionSupport, XlsxCommandId, flag, spec};

pub(super) const COMMAND_COUNT: usize = 3;
pub(super) const LEGACY_START: usize = 245;

pub(super) fn command_specs() -> Vec<super::CommandSpec> {
    vec![
        spec(
            XlsxCommandId::FreezeShow,
            &["xlsx", "freeze", "show"],
            "show <file>",
            "Display current worksheet freeze panes state.",
            &["sheet"],
            vec![flag("--sheet", "sheet", "string", "sheet selector")],
            ExecutionSupport::ServeInspect {
                reason: Some("read-only command; call via inspect in serve/MCP"),
            },
            None,
        ),
        spec(
            XlsxCommandId::FreezeSet,
            &["xlsx", "freeze", "set"],
            "set <file>",
            "Set frozen rows and/or columns on a worksheet.",
            &["sheet"],
            vec![
                flag("--sheet", "sheet", "string", "sheet selector"),
                flag("--rows", "rows", "number", "number of top rows to freeze"),
                flag(
                    "--cols",
                    "cols",
                    "number",
                    "number of left columns to freeze",
                ),
                flag(
                    "--expect-state",
                    "expectState",
                    "string",
                    "guard: require current state to be none or frozen",
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
            ExecutionSupport::ServeMutation { reason: None },
            None,
        ),
        spec(
            XlsxCommandId::FreezeClear,
            &["xlsx", "freeze", "clear"],
            "clear <file>",
            "Remove frozen panes from a worksheet.",
            &["sheet"],
            vec![
                flag("--sheet", "sheet", "string", "sheet selector"),
                flag(
                    "--expect-state",
                    "expectState",
                    "string",
                    "guard: require current state to be none or frozen",
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
            ExecutionSupport::ServeMutation { reason: None },
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
    fn freeze_segment_matches_frozen_contract_slice() {
        let specs = command_specs();
        let frozen = frozen_contract_commands();
        assert_eq!(specs.len(), COMMAND_COUNT);
        assert_segment_matches_frozen_contract(
            &specs,
            &frozen[LEGACY_START..LEGACY_START + COMMAND_COUNT],
        );
    }

    #[test]
    fn freeze_ids_paths_builds_and_execution_inventory_are_stable() {
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
        assert_eq!(inventory, (0, 0, 1, 2));
    }
}
