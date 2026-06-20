use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

use crate::{CliError, CliResult, command_arg, xlsx_source_command};

use super::model::XlsxDefinedName;
use super::package::xlsx_defined_names;

pub(super) fn xlsx_defined_name_items_json(file: &str, names: &[XlsxDefinedName]) -> Vec<Value> {
    let counts = workbook_scoped_defined_name_counts(names);
    names
        .iter()
        .map(|name| xlsx_defined_name_item_json(file, name, Some(&counts)))
        .collect()
}

pub(super) fn xlsx_defined_name_item_json(
    file: &str,
    name: &XlsxDefinedName,
    counts: Option<&BTreeMap<String, usize>>,
) -> Value {
    let mut object = Map::new();
    object.insert("number".to_string(), json!(name.number));
    object.insert("name".to_string(), json!(name.name));
    object.insert("scope".to_string(), json!(name.scope));
    if let Some(local_sheet_id) = name.local_sheet_id {
        object.insert("localSheetId".to_string(), json!(local_sheet_id));
    }
    if name.sheet_number > 0 {
        object.insert("sheetNumber".to_string(), json!(name.sheet_number));
    }
    if !name.sheet_name.is_empty() {
        object.insert("sheetName".to_string(), json!(name.sheet_name));
    }
    object.insert("ref".to_string(), json!(name.ref_text));
    if name.hidden {
        object.insert("hidden".to_string(), json!(true));
    }
    if !name.comment.is_empty() {
        object.insert("comment".to_string(), json!(name.comment));
    }
    if !name.description.is_empty() {
        object.insert("description".to_string(), json!(name.description));
    }
    let unique =
        counts.is_none_or(|counts| counts.get(&name.name).copied().unwrap_or_default() == 1);
    if unique && name.scope == "workbook" && !name.name.trim().is_empty() {
        object.insert(
            "handle".to_string(),
            json!(format!("H:xlsx/wb/name:n:{}", name.name)),
        );
    }
    if !name.primary_selector.is_empty() {
        object.insert("primarySelector".to_string(), json!(name.primary_selector));
    }
    if !name.selectors.is_empty() {
        object.insert("selectors".to_string(), json!(name.selectors));
    }
    if !file.is_empty() {
        object.insert(
            "showCommand".to_string(),
            json!(xlsx_name_show_command(file, name)),
        );
    }
    Value::Object(object)
}

fn workbook_scoped_defined_name_counts(names: &[XlsxDefinedName]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for name in names {
        if name.scope == "workbook" && !name.name.trim().is_empty() {
            *counts.entry(name.name.clone()).or_insert(0) += 1;
        }
    }
    counts
}

fn xlsx_name_show_command(file: &str, name: &XlsxDefinedName) -> String {
    let mut command = xlsx_source_command(
        vec!["ooxml", "--json", "xlsx", "names", "show", file],
        &[("--name", &name.name)],
    );
    if name.scope == "sheet" && name.sheet_number > 0 {
        command.push_str(&format!(
            " --scope-sheet {}",
            command_arg(&format!("sheet:{}", name.sheet_number))
        ));
    }
    command
}

pub(super) fn xlsx_name_mutation_readback_commands(
    file: &str,
    name: Option<&XlsxDefinedName>,
) -> Map<String, Value> {
    let mut object = Map::new();
    if file.trim().is_empty() {
        let placeholder = "<out.xlsx>";
        object.insert(
            "validateCommandTemplate".to_string(),
            json!(format!(
                "ooxml validate --strict {}",
                command_arg(placeholder)
            )),
        );
        object.insert(
            "namesListCommandTemplate".to_string(),
            json!(xlsx_names_list_command(placeholder)),
        );
        if let Some(name) = name {
            object.insert(
                "nameShowCommandTemplate".to_string(),
                json!(xlsx_name_show_command(placeholder, name)),
            );
        }
    } else {
        object.insert(
            "validateCommand".to_string(),
            json!(format!("ooxml validate --strict {}", command_arg(file))),
        );
        object.insert(
            "namesListCommand".to_string(),
            json!(xlsx_names_list_command(file)),
        );
        if let Some(name) = name {
            object.insert(
                "nameShowCommand".to_string(),
                json!(xlsx_name_show_command(file, name)),
            );
        }
    }
    object
}

fn xlsx_names_list_command(file: &str) -> String {
    format!("ooxml --json xlsx names list {}", command_arg(file))
}

pub(super) fn readback_xlsx_defined_name(
    file: &str,
    name: &str,
    local_sheet_id: Option<i64>,
) -> CliResult<XlsxDefinedName> {
    let (_, names) = xlsx_defined_names(file)?;
    names
        .into_iter()
        .find(|candidate| {
            candidate.name.eq_ignore_ascii_case(name) && candidate.local_sheet_id == local_sheet_id
        })
        .ok_or_else(|| {
            CliError::unexpected(format!("changed defined name {name:?} did not read back"))
        })
}
