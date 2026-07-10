use super::{PptxCommandId, direct, flag, spec};

pub(super) const COMMAND_COUNT: usize = 1;

pub(super) fn command_specs() -> Vec<super::CommandSpec> {
    vec![spec(
        PptxCommandId::Diff,
        &["pptx", "diff"],
        "diff <baseline> <candidate>",
        "Compare two PPTX presentations",
        &[],
        vec![
            flag(
                "--render",
                "render",
                "bool",
                "enable visual diff via rendered slide images",
            ),
            flag("--threshold", "threshold", "float", "visual diff threshold"),
            flag(
                "--out",
                "out",
                "string",
                "output directory for visual diff artifacts",
            ),
        ],
        direct("read-only package comparison command; not a serve/MCP mutation op"),
        None,
    )]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command_manifest::ExecutionSupport;

    #[test]
    fn owner_contract() {
        let specs = command_specs();
        assert_eq!(specs.len(), COMMAND_COUNT);
        assert!(matches!(
            &specs[0].execution,
            ExecutionSupport::DirectOnly {
                reason: Some("read-only package comparison command; not a serve/MCP mutation op")
            }
        ));
    }
}
