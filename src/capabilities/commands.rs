mod core;
mod docx;
mod pptx;
mod vba;
mod xlsx;

use serde_json::{Map, Value, json};

pub(crate) fn capability_commands() -> Vec<Value> {
    let mut commands = Vec::new();
    commands.extend(core::commands());
    commands.extend(pptx::commands());
    commands.extend(xlsx::commands());
    commands.extend(docx::commands());
    commands.extend(vba::commands());
    commands
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
    let mut object = Map::new();
    object.insert("path".to_string(), json!(path));
    object.insert("use".to_string(), json!(use_text));
    object.insert("short".to_string(), json!(short));
    object.insert("targetObjectKinds".to_string(), json!(target_kinds));
    object.insert("localFlags".to_string(), Value::Array(local_flags));
    object.insert("opCompatible".to_string(), json!(op_compatible));
    if let Some(reason) = op_ineligible_reason {
        object.insert("opIneligibleReason".to_string(), json!(reason));
    }
    Value::Object(object)
}

fn flag(name: &str, arg_name: &str, flag_type: &str, description: &str) -> Value {
    json!({
        "name": name,
        "argName": arg_name,
        "type": flag_type,
        "description": description,
    })
}
