use super::{PptxCommandId, direct, flag, spec};

pub(super) const COMMAND_COUNT: usize = 1;

pub(super) fn command_specs() -> Vec<super::CommandSpec> {
    vec![spec(
        PptxCommandId::Render,
        &["pptx", "render"],
        "render <file>",
        "Render a PPTX to PDF/thumbnails when local tools are installed.",
        &["slide"],
        vec![
            flag("--out", "out", "string", "render output directory"),
            flag("--slides", "slides", "string", "comma-separated slide list"),
            flag("--format", "format", "string", "json"),
        ],
        direct("render command is not a mutation op"),
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
                reason: Some("render command is not a mutation op")
            }
        ));
    }
}
