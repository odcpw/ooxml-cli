mod animations;
mod authoring;
mod charts;
mod diff;
mod extract_media_notes_comments;
mod masters_layouts;
mod render;
mod replace;
mod slides;
mod tables;
mod template;

use serde_json::Value;

use super::capability_command;

const COMMAND_GROUP_REASON: &str = "it is a command group, not a leaf mutation command";

pub(super) fn commands() -> Vec<Value> {
    let mut commands = group_commands();
    commands.extend(diff::commands());
    commands.extend(slides::commands());
    commands.extend(template::commands());
    commands.extend(authoring::commands());
    commands.extend(animations::commands());
    commands.extend(masters_layouts::commands());
    commands.extend(charts::commands());
    commands.extend(tables::commands());
    commands.extend(extract_media_notes_comments::commands());
    commands.extend(replace::commands());
    commands.extend(render::commands());
    commands
}

fn group_commands() -> Vec<Value> {
    vec![
        command_group("ooxml pptx", "pptx", "Work with PPTX presentations"),
        command_group(
            "ooxml pptx animations",
            "animations",
            "Inspect per-slide animations and embedded media",
        ),
        command_group(
            "ooxml pptx charts",
            "charts",
            "Inspect and mutate slide charts",
        ),
        command_group(
            "ooxml pptx comments",
            "comments",
            "Inspect and mutate PPTX slide comments",
        ),
        command_group(
            "ooxml pptx extract",
            "extract",
            "Extract resources from presentations",
        ),
        command_group(
            "ooxml pptx fields",
            "fields",
            "Inspect and set header/footer/slide-number/date fields",
        ),
        command_group("ooxml pptx layouts", "layouts", "Inspect slide layouts"),
        command_group("ooxml pptx masters", "masters", "Inspect slide masters"),
        command_group(
            "ooxml pptx media",
            "media",
            "Embed, replace, and inspect slide audio/video media",
        ),
        command_group(
            "ooxml pptx notes",
            "notes",
            "Set, clear, and show slide speaker notes",
        ),
        command_group(
            "ooxml pptx place",
            "place",
            "Place content on presentations",
        ),
        command_group(
            "ooxml pptx replace",
            "replace",
            "Replace content in presentations",
        ),
        command_group(
            "ooxml pptx shapes",
            "shapes",
            "Inspect and mutate slide shapes",
        ),
        command_group("ooxml pptx slides", "slides", "Inspect slides"),
        command_group(
            "ooxml pptx tables",
            "tables",
            "Inspect and mutate PPTX tables",
        ),
        command_group(
            "ooxml pptx template",
            "template",
            "Work with template manifests and compilation",
        ),
        command_group(
            "ooxml pptx text",
            "text",
            "Set slide text run/paragraph styling",
        ),
        command_group(
            "ooxml pptx theme",
            "theme",
            "Inspect and modify presentation themes",
        ),
        command_group(
            "ooxml pptx translate",
            "translate",
            "Export and manage translations",
        ),
        command_group(
            "ooxml pptx xlsx-bindings",
            "xlsx-bindings",
            "Plan and apply workbook-driven PPTX updates",
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
