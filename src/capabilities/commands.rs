mod core;
mod docx;
mod pptx;
mod vba;
mod xlsx;

use serde::Serialize;
use serde_json::{Value, json};

pub(crate) fn capability_commands() -> Vec<Value> {
    let mut commands = Vec::new();
    commands.extend(core::commands());
    commands.extend(pptx::commands());
    commands.extend(xlsx::commands());
    commands.extend(docx::commands());
    commands.extend(vba::commands());
    commands
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CapabilityCommand<'a> {
    path: &'a str,
    #[serde(rename = "use")]
    use_text: &'a str,
    short: &'a str,
    target_object_kinds: &'a [&'a str],
    local_flags: Vec<Value>,
    op_compatible: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    op_ineligible_reason: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    flag_constraints: Option<Value>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CapabilityFlag<'a> {
    name: &'a str,
    arg_name: &'a str,
    #[serde(rename = "type")]
    flag_type: &'a str,
    description: &'a str,
}

fn capability_command(
    path: &str,
    use_text: &str,
    short: &str,
    target_kinds: &[&str],
    op_compatible: bool,
    op_ineligible_reason: Option<&str>,
    local_flags: Vec<Value>,
) -> Value {
    capability_command_with_flag_constraints(
        path,
        use_text,
        short,
        target_kinds,
        op_compatible,
        op_ineligible_reason,
        local_flags,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn capability_command_with_flag_constraints(
    path: &str,
    use_text: &str,
    short: &str,
    target_kinds: &[&str],
    op_compatible: bool,
    op_ineligible_reason: Option<&str>,
    local_flags: Vec<Value>,
    flag_constraints: Option<Value>,
) -> Value {
    json!(CapabilityCommand {
        path,
        use_text,
        short,
        target_object_kinds: target_kinds,
        local_flags,
        op_compatible,
        op_ineligible_reason,
        flag_constraints,
    })
}

fn flag(name: &str, arg_name: &str, flag_type: &str, description: &str) -> Value {
    json!(CapabilityFlag {
        name,
        arg_name,
        flag_type,
        description,
    })
}
