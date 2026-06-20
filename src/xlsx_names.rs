mod model;
mod mutation;
mod output;
mod package;
mod workbook_xml;

use serde_json::{Value, json};

pub(crate) use model::XlsxNameMutationOptions;

use crate::{CliResult, command_arg};
use mutation::xlsx_names_mutate;
use output::{xlsx_defined_name_item_json, xlsx_defined_name_items_json};
use package::{
    filter_xlsx_defined_names_by_scope_sheet, select_xlsx_defined_name, xlsx_defined_names,
};

pub(crate) fn xlsx_names_list(file: &str, scope_sheet: Option<&str>) -> CliResult<Value> {
    let (sheets, names) = xlsx_defined_names(file)?;
    let names = filter_xlsx_defined_names_by_scope_sheet(&sheets, names, scope_sheet)?;
    Ok(json!({
        "file": file,
        "validateCommand": format!("ooxml validate --strict {}", command_arg(file)),
        "names": xlsx_defined_name_items_json(file, &names),
    }))
}

pub(crate) fn xlsx_names_add(file: &str, options: XlsxNameMutationOptions<'_>) -> CliResult<Value> {
    xlsx_names_mutate(file, "add", options)
}

pub(crate) fn xlsx_names_update(
    file: &str,
    options: XlsxNameMutationOptions<'_>,
) -> CliResult<Value> {
    xlsx_names_mutate(file, "update", options)
}

pub(crate) fn xlsx_names_rename(
    file: &str,
    options: XlsxNameMutationOptions<'_>,
) -> CliResult<Value> {
    xlsx_names_mutate(file, "rename", options)
}

pub(crate) fn xlsx_names_delete(
    file: &str,
    options: XlsxNameMutationOptions<'_>,
) -> CliResult<Value> {
    xlsx_names_mutate(file, "delete", options)
}

pub(crate) fn xlsx_names_show(
    file: &str,
    selector: &str,
    scope_sheet: Option<&str>,
) -> CliResult<Value> {
    let (sheets, names) = xlsx_defined_names(file)?;
    let name = select_xlsx_defined_name(&sheets, &names, selector, scope_sheet)?;
    Ok(json!({
        "file": file,
        "validateCommand": format!("ooxml validate --strict {}", command_arg(file)),
        "name": xlsx_defined_name_item_json(file, &name, None),
    }))
}
