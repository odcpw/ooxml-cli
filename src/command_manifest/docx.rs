use super::CommandSpec;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(super) enum DocxCommandId {}

pub(super) fn command_specs() -> Vec<CommandSpec> {
    Vec::new()
}
