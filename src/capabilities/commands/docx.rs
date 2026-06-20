mod blocks;
mod comments;
mod fields;
mod headers_footers;
mod images;
mod paragraphs;
mod replace;
mod styles;
mod tables;

use serde_json::Value;

pub(super) fn commands() -> Vec<Value> {
    let mut commands = Vec::new();
    commands.extend(blocks::commands());
    commands.extend(paragraphs::commands());
    commands.extend(styles::commands());
    commands.extend(comments::commands());
    commands.extend(fields::commands());
    commands.extend(headers_footers::commands());
    commands.extend(images::commands());
    commands.extend(replace::commands());
    commands.extend(tables::commands());
    commands
}
