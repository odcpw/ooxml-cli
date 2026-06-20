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

use super::capability_command;

const GROUP_COMMAND_REASON: &str = "it is a command group, not a leaf mutation command";

pub(super) fn commands() -> Vec<Value> {
    let mut commands = Vec::new();
    commands.extend(group_commands());
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

fn group_commands() -> Vec<Value> {
    vec![
        capability_command(
            "ooxml docx",
            "docx",
            "Work with DOCX documents",
            &[],
            false,
            Some(GROUP_COMMAND_REASON),
            vec![],
        ),
        capability_command(
            "ooxml docx comments",
            "comments",
            "Inspect and mutate DOCX comments",
            &[],
            false,
            Some(GROUP_COMMAND_REASON),
            vec![],
        ),
        capability_command(
            "ooxml docx fields",
            "fields",
            "Inspect and edit DOCX fields (PAGE, NUMPAGES, DATE, etc.)",
            &[],
            false,
            Some(GROUP_COMMAND_REASON),
            vec![],
        ),
        capability_command(
            "ooxml docx footers",
            "footers",
            "Inspect and edit DOCX footers",
            &[],
            false,
            Some(GROUP_COMMAND_REASON),
            vec![],
        ),
        capability_command(
            "ooxml docx headers",
            "headers",
            "Inspect and edit DOCX headers",
            &[],
            false,
            Some(GROUP_COMMAND_REASON),
            vec![],
        ),
        capability_command(
            "ooxml docx images",
            "images",
            "Inspect and mutate inline images in a DOCX document",
            &[],
            false,
            Some(GROUP_COMMAND_REASON),
            vec![],
        ),
        capability_command(
            "ooxml docx paragraphs",
            "paragraphs",
            "Mutate DOCX body paragraphs",
            &[],
            false,
            Some(GROUP_COMMAND_REASON),
            vec![],
        ),
        capability_command(
            "ooxml docx styles",
            "styles",
            "Inspect DOCX style definitions from word/styles.xml",
            &[],
            false,
            Some(GROUP_COMMAND_REASON),
            vec![],
        ),
        capability_command(
            "ooxml docx tables",
            "tables",
            "Inspect and mutate DOCX tables",
            &[],
            false,
            Some(GROUP_COMMAND_REASON),
            vec![],
        ),
    ]
}
