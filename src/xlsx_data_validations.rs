use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::{
    CliError, CliResult, WorkbookSheet, append_xml_text_event, command_arg,
    copy_zip_with_part_override, is_xml_text_event, local_name, normalize_xl_target, relationships,
    render_xml_attrs, replace_xml_span, resolve_sheet, selector_candidates, validate,
    validate_xlsx_mutation_output_flags, workbook_sheets, xlsx_ranges_set_temp_path, xml_attrs_map,
    xml_direct_child_ranges, xml_escape, xml_fragment_bounds, xml_open_tag_from_start,
    xml_tag_prefix, zip_text,
};

#[derive(Clone)]
struct WorksheetRootBounds {
    start: usize,
    open_end: usize,
    close_start: usize,
    end: usize,
    tag_name: String,
    self_closing: bool,
}

#[derive(Clone, Debug)]
struct XlsxDataValidation {
    sqref: String,
    validation_type: String,
    operator: String,
    formula1: String,
    formula2: String,
    allow_blank: bool,
    show_input_message: bool,
    show_error_message: bool,
    prompt_title: String,
    prompt: String,
    error_title: String,
    error: String,
    error_style: String,
    attrs: BTreeMap<String, String>,
}

#[derive(Clone, Copy)]
struct DataValidationOutputOptions<'a> {
    out: Option<&'a str>,
    backup: Option<&'a str>,
    dry_run: bool,
    no_validate: bool,
    in_place: bool,
}

pub(crate) struct XlsxDataValidationFields<'a> {
    pub(crate) validation_type: Option<&'a str>,
    pub(crate) operator: Option<&'a str>,
    pub(crate) formula1: Option<&'a str>,
    pub(crate) formula2: Option<&'a str>,
    pub(crate) list_values: Option<&'a str>,
    pub(crate) list_range: Option<&'a str>,
    pub(crate) allow_blank: bool,
    pub(crate) show_input_message: bool,
    pub(crate) show_error_message: bool,
    pub(crate) prompt_title: Option<&'a str>,
    pub(crate) prompt: Option<&'a str>,
    pub(crate) error_title: Option<&'a str>,
    pub(crate) error: Option<&'a str>,
    pub(crate) error_style: Option<&'a str>,

    pub(crate) set_type: bool,
    pub(crate) set_operator: bool,
    pub(crate) set_formula1: bool,
    pub(crate) set_formula2: bool,
    pub(crate) set_list_values: bool,
    pub(crate) set_list_range: bool,
    pub(crate) set_allow_blank: bool,
    pub(crate) set_show_input_message: bool,
    pub(crate) set_show_error_message: bool,
    pub(crate) set_prompt_title: bool,
    pub(crate) set_prompt: bool,
    pub(crate) set_error_title: bool,
    pub(crate) set_error: bool,
    pub(crate) set_error_style: bool,
}

pub(crate) struct XlsxDataValidationMutationOptions<'a> {
    pub(crate) sheet: Option<&'a str>,
    pub(crate) range: Option<&'a str>,
    pub(crate) fields: XlsxDataValidationFields<'a>,
    pub(crate) expect_type: Option<&'a str>,
    pub(crate) expect_type_present: bool,
    pub(crate) expect_formula1: Option<&'a str>,
    pub(crate) expect_formula1_present: bool,
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
}

struct DataValidationMutation {
    updated_xml: String,
    sqref: String,
    validation: Option<XlsxDataValidation>,
    cells_affected: i64,
}

#[derive(Clone, Copy)]
struct SqrefCell {
    col: u32,
    row: u32,
    abs_col: bool,
    abs_row: bool,
}

pub(crate) fn xlsx_data_validations_list(
    file: &str,
    sheet_selector: Option<&str>,
) -> CliResult<Value> {
    let (sheet, _sheet_part, sheet_xml) = resolve_data_validation_sheet(file, sheet_selector)?;
    let validations = read_data_validations(&sheet_xml)?;
    let data_validations = if validations.is_empty() {
        Value::Null
    } else {
        Value::Array(validations.iter().map(data_validation_json).collect())
    };
    Ok(json!({
        "file": file,
        "sheet": sheet.name,
        "sheetNumber": sheet.position,
        "count": validations.len(),
        "dataValidations": data_validations,
    }))
}

pub(crate) fn xlsx_data_validations_show(
    file: &str,
    sheet_selector: Option<&str>,
    range: &str,
) -> CliResult<Value> {
    let norm_range = normalize_sqref(range)
        .map_err(|err| CliError::invalid_args(format!("invalid --range: {}", err.message)))?;
    let (sheet, _sheet_part, sheet_xml) = resolve_data_validation_sheet(file, sheet_selector)?;
    let validations = read_data_validations(&sheet_xml)?;
    for validation in &validations {
        if normalize_sqref(&validation.sqref).ok().as_deref() == Some(norm_range.as_str()) {
            return Ok(data_validation_json(validation));
        }
    }

    let discovery = format!(
        "ooxml --json xlsx data-validations list <file> --sheet {}",
        command_arg(&data_validation_sheet_selector(&sheet))
    );
    let mut message = format!("data validation not found: {norm_range}");
    if !validations.is_empty() {
        let items = validations
            .iter()
            .map(|validation| {
                (
                    validation.sqref.as_str(),
                    std::slice::from_ref(&validation.sqref),
                )
            })
            .collect::<Vec<_>>();
        let candidates = selector_candidates(&items, &norm_range, 5);
        if !candidates.is_empty() {
            message.push_str(&format!("; did you mean: {}", candidates.join(", ")));
        }
    }
    message.push_str(&format!("; discover with `{discovery}`"));
    Err(CliError::target_not_found(message))
}

pub(crate) fn xlsx_data_validations_create(
    file: &str,
    options: XlsxDataValidationMutationOptions<'_>,
) -> CliResult<Value> {
    let range = options
        .range
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| CliError::invalid_args("--range is required"))?;
    if options
        .fields
        .validation_type
        .is_none_or(|value| value.trim().is_empty())
    {
        return Err(CliError::invalid_args(
            "--type is required (list|whole|decimal|date|text-length)",
        ));
    }
    run_data_validation_mutation(file, "create", options, |xml, prefix, options| {
        create_data_validation_xml(xml, prefix, range, &options.fields)
    })
}

pub(crate) fn xlsx_data_validations_update(
    file: &str,
    options: XlsxDataValidationMutationOptions<'_>,
) -> CliResult<Value> {
    if options.range.is_none_or(|value| value.trim().is_empty()) {
        return Err(CliError::invalid_args("--range is required"));
    }
    run_data_validation_mutation(file, "update", options, |xml, prefix, options| {
        update_data_validation_xml(xml, prefix, options)
    })
}

pub(crate) fn xlsx_data_validations_delete(
    file: &str,
    options: XlsxDataValidationMutationOptions<'_>,
) -> CliResult<Value> {
    if options.range.is_none_or(|value| value.trim().is_empty()) {
        return Err(CliError::invalid_args("--range is required"));
    }
    run_data_validation_mutation(file, "delete", options, |xml, _prefix, options| {
        delete_data_validation_xml(xml, options)
    })
}

fn run_data_validation_mutation<F>(
    file: &str,
    action: &str,
    options: XlsxDataValidationMutationOptions<'_>,
    apply: F,
) -> CliResult<Value>
where
    F: FnOnce(
        &str,
        &str,
        &XlsxDataValidationMutationOptions<'_>,
    ) -> CliResult<DataValidationMutation>,
{
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;
    if !Path::new(file).exists() {
        return Err(CliError::file_not_found(format!("file not found: {file}")));
    }
    let (sheet, sheet_part, sheet_xml) = resolve_data_validation_sheet(file, options.sheet)?;
    let root = worksheet_root_bounds(&sheet_xml)?;
    let prefix = xml_tag_prefix(&root.tag_name);
    let mutation = apply(&sheet_xml, &prefix, &options).map_err(|err| {
        if err.code == "invalid_args" {
            CliError::invalid_args(format!(
                "failed to {action} data validation: {}",
                err.message
            ))
        } else {
            err
        }
    })?;
    let output_path = write_data_validation_mutation(
        file,
        &sheet_part,
        &mutation.updated_xml,
        DataValidationOutputOptions {
            out: options.out,
            backup: options.backup,
            dry_run: options.dry_run,
            no_validate: options.no_validate,
            in_place: options.in_place,
        },
    )?;

    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("sheet".to_string(), json!(sheet.name));
    result.insert("sheetNumber".to_string(), json!(sheet.position));
    result.insert("action".to_string(), json!(action));
    result.insert("range".to_string(), json!(mutation.sqref));
    result.insert("cellsAffected".to_string(), json!(mutation.cells_affected));
    if let Some(validation) = mutation.validation.as_ref() {
        result.insert(
            "dataValidation".to_string(),
            data_validation_json(validation),
        );
    }
    if let Some(output_path) = output_path.as_deref() {
        result.insert("output".to_string(), json!(output_path));
    }
    result.insert("dryRun".to_string(), json!(options.dry_run));
    if let Some(output_path) = output_path.as_deref() {
        let selector = data_validation_sheet_selector(&sheet);
        result.insert(
            "validateCommand".to_string(),
            json!(format!(
                "ooxml validate --strict {}",
                command_arg(output_path)
            )),
        );
        result.insert(
            "dataValidationsListCommand".to_string(),
            json!(format!(
                "ooxml --json xlsx data-validations list {} --sheet {}",
                command_arg(output_path),
                command_arg(&selector)
            )),
        );
        if action != "delete" {
            result.insert(
                "dataValidationsShowCommand".to_string(),
                json!(format!(
                    "ooxml --json xlsx data-validations show {} --sheet {} --range {}",
                    command_arg(output_path),
                    command_arg(&selector),
                    command_arg(&mutation.sqref)
                )),
            );
        }
    }
    Ok(Value::Object(result))
}

fn create_data_validation_xml(
    xml: &str,
    prefix: &str,
    range: &str,
    fields: &XlsxDataValidationFields<'_>,
) -> CliResult<DataValidationMutation> {
    let norm_sqref = normalize_sqref(range)?;
    let root = worksheet_root_bounds(xml)?;
    let container = data_validations_container_range(xml, &root)?;
    if let Some(container) = container.as_ref()
        && find_data_validation_range(xml, container, &norm_sqref)?.is_some()
    {
        return Err(CliError::invalid_args(format!(
            "a data validation already exists on {norm_sqref} (use update)"
        )));
    }

    let mut validation = XlsxDataValidation::new(norm_sqref.clone());
    apply_data_validation_fields(&mut validation, fields, true)?;
    let validation_xml = render_data_validation(prefix, &validation);
    let updated_xml = if let Some(container) = container {
        let mut children = data_validation_child_fragments(xml, &container)?;
        children.push(validation_xml);
        replace_xml_span(
            xml,
            container.start,
            container.end,
            &render_data_validations_container(xml, &container, prefix, &children)?,
        )
    } else {
        let child = render_new_data_validations_container(prefix, &[validation_xml]);
        insert_worksheet_child(xml, &root, "dataValidations", &child)?
    };
    Ok(DataValidationMutation {
        updated_xml,
        sqref: norm_sqref.clone(),
        validation: Some(validation),
        cells_affected: sqref_cell_count(&norm_sqref),
    })
}

fn update_data_validation_xml(
    xml: &str,
    prefix: &str,
    options: &XlsxDataValidationMutationOptions<'_>,
) -> CliResult<DataValidationMutation> {
    let norm_sqref = normalize_sqref(options.range.unwrap_or_default())?;
    let root = worksheet_root_bounds(xml)?;
    let container = data_validations_container_range(xml, &root)?;
    let Some(container) = container.as_ref() else {
        return Err(CliError::invalid_args(format!(
            "no data validation found on {norm_sqref}"
        )));
    };
    let Some(range) = find_data_validation_range(xml, container, &norm_sqref)? else {
        return Err(CliError::invalid_args(format!(
            "no data validation found on {norm_sqref}"
        )));
    };
    let mut validation = parse_data_validation(&xml[range.start..range.end])?;
    check_data_validation_guards(
        &validation,
        options.expect_type_present,
        options.expect_type.unwrap_or_default(),
        options.expect_formula1_present,
        options.expect_formula1.unwrap_or_default(),
    )?;
    apply_data_validation_fields(&mut validation, &options.fields, false)?;
    let updated_fragment = render_data_validation(prefix, &validation);
    Ok(DataValidationMutation {
        updated_xml: replace_xml_span(xml, range.start, range.end, &updated_fragment),
        sqref: norm_sqref.clone(),
        validation: Some(validation),
        cells_affected: sqref_cell_count(&norm_sqref),
    })
}

fn delete_data_validation_xml(
    xml: &str,
    options: &XlsxDataValidationMutationOptions<'_>,
) -> CliResult<DataValidationMutation> {
    let norm_sqref = normalize_sqref(options.range.unwrap_or_default())?;
    let root = worksheet_root_bounds(xml)?;
    let container = data_validations_container_range(xml, &root)?;
    let Some(container) = container.as_ref() else {
        return Err(CliError::invalid_args(format!(
            "no data validation found on {norm_sqref}"
        )));
    };
    let Some(range) = find_data_validation_range(xml, container, &norm_sqref)? else {
        return Err(CliError::invalid_args(format!(
            "no data validation found on {norm_sqref}"
        )));
    };
    let validation = parse_data_validation(&xml[range.start..range.end])?;
    check_data_validation_guards(
        &validation,
        options.expect_type_present,
        options.expect_type.unwrap_or_default(),
        options.expect_formula1_present,
        options.expect_formula1.unwrap_or_default(),
    )?;
    let mut children = data_validation_child_fragments(xml, container)?;
    let remove_index = children
        .iter()
        .position(|fragment| {
            parse_data_validation(fragment)
                .ok()
                .and_then(|validation| normalize_sqref(&validation.sqref).ok())
                .as_deref()
                == Some(norm_sqref.as_str())
        })
        .ok_or_else(|| CliError::unexpected("matched data validation disappeared"))?;
    children.remove(remove_index);
    let updated_xml = if children.is_empty() {
        replace_xml_span(xml, container.start, container.end, "")
    } else {
        replace_xml_span(
            xml,
            container.start,
            container.end,
            &render_data_validations_container(xml, container, "", &children)?,
        )
    };
    Ok(DataValidationMutation {
        updated_xml,
        sqref: norm_sqref.clone(),
        validation: None,
        cells_affected: sqref_cell_count(&norm_sqref),
    })
}

fn apply_data_validation_fields(
    validation: &mut XlsxDataValidation,
    fields: &XlsxDataValidationFields<'_>,
    create: bool,
) -> CliResult<()> {
    let requested_type = normalize_data_validation_type(fields.validation_type.unwrap_or_default());
    if create || fields.set_type {
        validation.validation_type = requested_type;
    }
    if create || fields.set_operator {
        validation.operator = fields.operator.unwrap_or_default().to_string();
    }

    validate_data_validation_type(&validation.validation_type)?;
    validate_data_validation_operator(&validation.operator, &validation.validation_type)?;
    if let Some(error_style) = fields.error_style
        && !error_style.is_empty()
        && !valid_error_style(error_style)
    {
        return Err(CliError::invalid_args(format!(
            "invalid error-style {error_style:?} (want stop, warning, information)"
        )));
    }

    if validation.validation_type == "list"
        && (fields.set_list_values
            || fields.set_list_range
            || (create
                && (fields
                    .list_values
                    .is_some_and(|value| !value.trim().is_empty())
                    || fields
                        .list_range
                        .is_some_and(|value| !value.trim().is_empty()))))
    {
        validation.formula1 = resolve_list_formula1(
            fields.list_values.unwrap_or_default(),
            fields.list_range.unwrap_or_default(),
        )?;
        validation.formula2.clear();
    } else {
        if create || fields.set_formula1 {
            validation.formula1 = fields.formula1.unwrap_or_default().to_string();
        }
        if create || fields.set_formula2 {
            validation.formula2 = fields.formula2.unwrap_or_default().to_string();
        }
    }

    if matches!(validation.operator.as_str(), "between" | "notBetween")
        && validation.formula2.is_empty()
    {
        return Err(CliError::invalid_args(format!(
            "operator {:?} requires formula2",
            validation.operator
        )));
    }
    if !validation.validation_type.is_empty() && validation.formula1.is_empty() {
        if validation.validation_type == "list" {
            return Err(CliError::invalid_args(format!(
                "type {:?} requires --list-values or --list-range",
                validation.validation_type
            )));
        }
        return Err(CliError::invalid_args(format!(
            "type {:?} requires formula1",
            validation.validation_type
        )));
    }

    if create || fields.set_allow_blank {
        validation.allow_blank = fields.allow_blank;
    }
    if create || fields.set_show_input_message {
        validation.show_input_message = fields.show_input_message;
    }
    if create || fields.set_show_error_message {
        validation.show_error_message = fields.show_error_message;
    }
    if create || fields.set_prompt_title {
        validation.prompt_title = fields.prompt_title.unwrap_or_default().to_string();
    }
    if create || fields.set_prompt {
        validation.prompt = fields.prompt.unwrap_or_default().to_string();
    }
    if create || fields.set_error_title {
        validation.error_title = fields.error_title.unwrap_or_default().to_string();
    }
    if create || fields.set_error {
        validation.error = fields.error.unwrap_or_default().to_string();
    }
    if create || fields.set_error_style {
        validation.error_style = fields.error_style.unwrap_or_default().to_string();
    }
    validation.refresh_attrs();
    Ok(())
}

fn check_data_validation_guards(
    current: &XlsxDataValidation,
    has_type: bool,
    expect_type: &str,
    has_formula1: bool,
    expect_formula1: &str,
) -> CliResult<()> {
    if has_type {
        let want = normalize_data_validation_type(expect_type);
        if current.validation_type != want {
            return Err(CliError::invalid_args(format!(
                "expected type {want:?} but found {:?}",
                current.validation_type
            )));
        }
    }
    if has_formula1 && current.formula1 != expect_formula1 {
        return Err(CliError::invalid_args(format!(
            "expected formula1 {expect_formula1:?} but found {:?}",
            current.formula1
        )));
    }
    Ok(())
}

fn read_data_validations(xml: &str) -> CliResult<Vec<XlsxDataValidation>> {
    let root = worksheet_root_bounds(xml)?;
    let Some(container) = data_validations_container_range(xml, &root)? else {
        return Ok(Vec::new());
    };
    data_validation_child_fragments(xml, &container)?
        .into_iter()
        .map(|fragment| parse_data_validation(&fragment))
        .collect()
}

fn parse_data_validation(fragment: &str) -> CliResult<XlsxDataValidation> {
    let (_, attrs, _, _) = first_element(fragment)?;
    let mut validation = XlsxDataValidation {
        sqref: attr_local(&attrs, "sqref").unwrap_or_default(),
        validation_type: attr_local(&attrs, "type").unwrap_or_default(),
        operator: attr_local(&attrs, "operator").unwrap_or_default(),
        formula1: String::new(),
        formula2: String::new(),
        allow_blank: attr_is_true(&attrs, "allowBlank"),
        show_input_message: attr_is_true(&attrs, "showInputMessage"),
        show_error_message: attr_is_true(&attrs, "showErrorMessage"),
        prompt_title: attr_local(&attrs, "promptTitle").unwrap_or_default(),
        prompt: attr_local(&attrs, "prompt").unwrap_or_default(),
        error_title: attr_local(&attrs, "errorTitle").unwrap_or_default(),
        error: attr_local(&attrs, "error").unwrap_or_default(),
        error_style: attr_local(&attrs, "errorStyle").unwrap_or_default(),
        attrs,
    };

    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(false);
    let mut stack = Vec::<String>::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                stack.push(local_name(e.name().as_ref()).to_string());
            }
            Ok(Event::Empty(_)) => {}
            Ok(event) if is_xml_text_event(&event) => {
                if stack.last().map(String::as_str) == Some("formula1") {
                    append_xml_text_event(&mut validation.formula1, &event);
                } else if stack.last().map(String::as_str) == Some("formula2") {
                    append_xml_text_event(&mut validation.formula2, &event);
                }
            }
            Ok(Event::End(_)) => {
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(validation)
}

fn data_validation_json(validation: &XlsxDataValidation) -> Value {
    let mut object = Map::new();
    object.insert("sqref".to_string(), json!(validation.sqref));
    if !validation.sqref.is_empty() {
        object.insert("primarySelector".to_string(), json!(validation.sqref));
        object.insert("selectors".to_string(), json!([validation.sqref]));
    }
    insert_nonempty(&mut object, "type", &validation.validation_type);
    insert_nonempty(&mut object, "operator", &validation.operator);
    insert_nonempty(&mut object, "formula1", &validation.formula1);
    insert_nonempty(&mut object, "formula2", &validation.formula2);
    object.insert("allowBlank".to_string(), json!(validation.allow_blank));
    object.insert(
        "showInputMessage".to_string(),
        json!(validation.show_input_message),
    );
    object.insert(
        "showErrorMessage".to_string(),
        json!(validation.show_error_message),
    );
    insert_nonempty(&mut object, "promptTitle", &validation.prompt_title);
    insert_nonempty(&mut object, "prompt", &validation.prompt);
    insert_nonempty(&mut object, "errorTitle", &validation.error_title);
    insert_nonempty(&mut object, "error", &validation.error);
    insert_nonempty(&mut object, "errorStyle", &validation.error_style);
    Value::Object(object)
}

fn insert_nonempty(object: &mut Map<String, Value>, key: &str, value: &str) {
    if !value.is_empty() {
        object.insert(key.to_string(), json!(value));
    }
}

impl XlsxDataValidation {
    fn new(sqref: String) -> Self {
        let mut attrs = BTreeMap::new();
        attrs.insert("sqref".to_string(), sqref.clone());
        Self {
            sqref,
            validation_type: String::new(),
            operator: String::new(),
            formula1: String::new(),
            formula2: String::new(),
            allow_blank: false,
            show_input_message: false,
            show_error_message: false,
            prompt_title: String::new(),
            prompt: String::new(),
            error_title: String::new(),
            error: String::new(),
            error_style: String::new(),
            attrs,
        }
    }

    fn refresh_attrs(&mut self) {
        set_string_attr(&mut self.attrs, "sqref", &self.sqref);
        set_string_attr(&mut self.attrs, "type", &self.validation_type);
        set_string_attr(&mut self.attrs, "operator", &self.operator);
        set_bool_attr(&mut self.attrs, "allowBlank", self.allow_blank);
        set_bool_attr(&mut self.attrs, "showInputMessage", self.show_input_message);
        set_bool_attr(&mut self.attrs, "showErrorMessage", self.show_error_message);
        set_string_attr(&mut self.attrs, "promptTitle", &self.prompt_title);
        set_string_attr(&mut self.attrs, "prompt", &self.prompt);
        set_string_attr(&mut self.attrs, "errorTitle", &self.error_title);
        set_string_attr(&mut self.attrs, "error", &self.error);
        set_string_attr(&mut self.attrs, "errorStyle", &self.error_style);
    }
}

fn render_data_validation(prefix: &str, validation: &XlsxDataValidation) -> String {
    let tag = element_name(prefix, "dataValidation");
    let mut body = String::new();
    if !validation.formula1.is_empty() {
        body.push_str(&format!(
            "<{}>{}</{}>",
            element_name(prefix, "formula1"),
            xml_escape(&validation.formula1),
            element_name(prefix, "formula1")
        ));
    }
    if !validation.formula2.is_empty() {
        body.push_str(&format!(
            "<{}>{}</{}>",
            element_name(prefix, "formula2"),
            xml_escape(&validation.formula2),
            element_name(prefix, "formula2")
        ));
    }
    if body.is_empty() {
        format!("<{}{} />", tag, render_xml_attrs(&validation.attrs)).replace(" />", "/>")
    } else {
        format!(
            "<{}{}>{}</{}>",
            tag,
            render_xml_attrs(&validation.attrs),
            body,
            tag
        )
    }
}

fn render_new_data_validations_container(prefix: &str, children: &[String]) -> String {
    let tag = element_name(prefix, "dataValidations");
    let mut attrs = BTreeMap::new();
    attrs.insert("count".to_string(), children.len().to_string());
    format!(
        "<{}{}>{}</{}>",
        tag,
        render_xml_attrs(&attrs),
        children.join(""),
        tag
    )
}

fn render_data_validations_container(
    xml: &str,
    container: &crate::XmlNamedRange,
    fallback_prefix: &str,
    children: &[String],
) -> CliResult<String> {
    let (tag_name, mut attrs, _, _) = first_element(&xml[container.start..container.end])?;
    attrs.insert("count".to_string(), children.len().to_string());
    let tag = if tag_name.is_empty() {
        element_name(fallback_prefix, "dataValidations")
    } else {
        tag_name
    };
    Ok(format!(
        "<{}{}>{}</{}>",
        tag,
        render_xml_attrs(&attrs),
        children.join(""),
        tag
    ))
}

fn data_validation_child_fragments(
    xml: &str,
    container: &crate::XmlNamedRange,
) -> CliResult<Vec<String>> {
    let (open_end, close_start, self_closing) = container_inner_bounds(xml, container)?;
    if self_closing {
        return Ok(Vec::new());
    }
    xml_direct_child_ranges(xml, open_end, close_start).map(|children| {
        children
            .into_iter()
            .filter(|child| child.kind == "dataValidation")
            .map(|child| xml[child.start..child.end].to_string())
            .collect()
    })
}

fn find_data_validation_range(
    xml: &str,
    container: &crate::XmlNamedRange,
    norm_sqref: &str,
) -> CliResult<Option<crate::XmlNamedRange>> {
    let (open_end, close_start, self_closing) = container_inner_bounds(xml, container)?;
    if self_closing {
        return Ok(None);
    }
    for child in xml_direct_child_ranges(xml, open_end, close_start)? {
        if child.kind != "dataValidation" {
            continue;
        }
        let validation = parse_data_validation(&xml[child.start..child.end])?;
        if normalize_sqref(&validation.sqref).ok().as_deref() == Some(norm_sqref) {
            return Ok(Some(child));
        }
    }
    Ok(None)
}

fn container_inner_bounds(
    xml: &str,
    container: &crate::XmlNamedRange,
) -> CliResult<(usize, usize, bool)> {
    let fragment = &xml[container.start..container.end];
    let (open_end, _, close_start, self_closing) = xml_fragment_bounds(fragment)?;
    Ok((
        container.start + open_end + 1,
        container.start + close_start,
        self_closing,
    ))
}

fn data_validations_container_range(
    xml: &str,
    root: &WorksheetRootBounds,
) -> CliResult<Option<crate::XmlNamedRange>> {
    Ok(
        xml_direct_child_ranges(xml, root.open_end, root.close_start)?
            .into_iter()
            .find(|child| child.kind == "dataValidations"),
    )
}

fn normalize_data_validation_type(value: &str) -> String {
    match value.trim() {
        "text-length" | "textLength" | "textlength" => "textLength".to_string(),
        other => other.to_string(),
    }
}

fn validate_data_validation_type(value: &str) -> CliResult<()> {
    if value.is_empty() || valid_data_validation_type(value) {
        return Ok(());
    }
    Err(CliError::invalid_args(format!(
        "invalid data validation type {value:?} (want list, whole, decimal, date, time, textLength, custom)"
    )))
}

fn validate_data_validation_operator(operator: &str, validation_type: &str) -> CliResult<()> {
    if operator.is_empty() {
        return Ok(());
    }
    if !valid_data_validation_operator(operator) {
        return Err(CliError::invalid_args(format!(
            "invalid operator {operator:?}"
        )));
    }
    if matches!(validation_type, "list" | "custom") {
        return Err(CliError::invalid_args(format!(
            "operator is not valid for type {validation_type:?}"
        )));
    }
    Ok(())
}

fn valid_data_validation_type(value: &str) -> bool {
    matches!(
        value,
        "list" | "whole" | "decimal" | "date" | "time" | "textLength" | "custom"
    )
}

fn valid_data_validation_operator(value: &str) -> bool {
    matches!(
        value,
        "between"
            | "notBetween"
            | "equal"
            | "notEqual"
            | "greaterThan"
            | "lessThan"
            | "greaterThanOrEqual"
            | "lessThanOrEqual"
    )
}

fn valid_error_style(value: &str) -> bool {
    matches!(value, "stop" | "warning" | "information")
}

fn resolve_list_formula1(values: &str, list_range: &str) -> CliResult<String> {
    let values = values.trim();
    let list_range = list_range.trim();
    if values.is_empty() == list_range.is_empty() {
        return Err(CliError::invalid_args(
            "list type requires exactly one of list-values or list-range",
        ));
    }
    if !list_range.is_empty() {
        return Ok(list_range.to_string());
    }
    Ok(format!("\"{}\"", values.replace('"', "\"\"")))
}

fn normalize_sqref(value: &str) -> CliResult<String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(CliError::invalid_args("range cannot be empty"));
    }
    value
        .split_whitespace()
        .map(normalize_sqref_part)
        .collect::<CliResult<Vec<_>>>()
        .map(|parts| parts.join(" "))
}

fn normalize_sqref_part(value: &str) -> CliResult<String> {
    if value.contains(':') {
        let range = parse_sqref_range(value)?;
        if range.0.render() == range.1.render() {
            Ok(range.0.render())
        } else {
            Ok(format!("{}:{}", range.0.render(), range.1.render()))
        }
    } else {
        parse_sqref_cell(value).map(|cell| cell.render())
    }
}

fn parse_sqref_range(value: &str) -> CliResult<(SqrefCell, SqrefCell)> {
    let value = value.trim();
    if value.is_empty() {
        return Err(CliError::invalid_args("range reference cannot be empty"));
    }
    let parts = value.split(':').collect::<Vec<_>>();
    if parts.len() > 2 {
        return Err(CliError::invalid_args(format!(
            "invalid range reference {value:?}"
        )));
    }
    let start = parse_sqref_cell(parts[0])
        .map_err(|err| CliError::invalid_args(format!("invalid range start: {}", err.message)))?;
    let end = if let Some(end) = parts.get(1) {
        if end.trim().is_empty() {
            return Err(CliError::invalid_args("range end cannot be empty"));
        }
        parse_sqref_cell(end)
            .map_err(|err| CliError::invalid_args(format!("invalid range end: {}", err.message)))?
    } else {
        start
    };
    Ok((start, end))
}

fn parse_sqref_cell(value: &str) -> CliResult<SqrefCell> {
    let mut rest = value.trim();
    if rest.is_empty() {
        return Err(CliError::invalid_args("cell reference cannot be empty"));
    }
    let abs_col = rest.starts_with('$');
    if abs_col {
        rest = &rest[1..];
        if rest.is_empty() {
            return Err(CliError::invalid_args("missing column in cell reference"));
        }
    }
    let col_len = rest
        .bytes()
        .take_while(|byte| byte.is_ascii_alphabetic())
        .count();
    if col_len == 0 {
        return Err(CliError::invalid_args("missing column in cell reference"));
    }
    let letters = &rest[..col_len];
    let mut col = 0u32;
    for ch in letters.chars() {
        col = col * 26 + (ch.to_ascii_uppercase() as u32 - 'A' as u32 + 1);
        if col > 16_384 {
            return Err(CliError::invalid_args(format!(
                "column {letters:?} out of XLSX bounds A-XFD"
            )));
        }
    }
    rest = &rest[col_len..];
    if rest.is_empty() {
        return Err(CliError::invalid_args("missing row in cell reference"));
    }
    let abs_row = rest.starts_with('$');
    if abs_row {
        rest = &rest[1..];
        if rest.is_empty() {
            return Err(CliError::invalid_args("missing row in cell reference"));
        }
    }
    if rest.contains('$') {
        return Err(CliError::invalid_args(
            "invalid absolute marker in row reference",
        ));
    }
    if !rest.chars().all(|ch| ch.is_ascii_digit()) {
        return Err(CliError::invalid_args(format!(
            "invalid row {rest:?} in cell reference"
        )));
    }
    let row = rest
        .parse::<u32>()
        .map_err(|err| CliError::invalid_args(format!("invalid row {rest:?}: {err}")))?;
    if row == 0 || row > 1_048_576 {
        return Err(CliError::invalid_args(format!(
            "row {row} out of XLSX bounds 1-1048576"
        )));
    }
    Ok(SqrefCell {
        col,
        row,
        abs_col,
        abs_row,
    })
}

impl SqrefCell {
    fn render(self) -> String {
        let mut out = String::new();
        if self.abs_col {
            out.push('$');
        }
        out.push_str(&sqref_col_name(self.col));
        if self.abs_row {
            out.push('$');
        }
        out.push_str(&self.row.to_string());
        out
    }
}

fn sqref_col_name(mut col: u32) -> String {
    let mut chars = Vec::new();
    while col > 0 {
        col -= 1;
        chars.push((b'A' + (col % 26) as u8) as char);
        col /= 26;
    }
    chars.iter().rev().collect()
}

fn sqref_cell_count(sqref: &str) -> i64 {
    let mut total = 0i64;
    for part in sqref.split_whitespace() {
        if part.contains(':')
            && let Ok((start, end)) = parse_sqref_range(part)
        {
            let cols = end.col as i64 - start.col as i64 + 1;
            let rows = end.row as i64 - start.row as i64 + 1;
            if cols > 0 && rows > 0 {
                total += cols * rows;
            }
            continue;
        }
        total += 1;
    }
    total
}

fn resolve_data_validation_sheet(
    file: &str,
    sheet_selector: Option<&str>,
) -> CliResult<(WorkbookSheet, String, String)> {
    let workbook = zip_text(file, "xl/workbook.xml")?;
    let sheets = workbook_sheets(&workbook)?;
    if sheets.is_empty() {
        return Err(CliError::invalid_args("workbook has no sheets"));
    }
    let selector = sheet_selector.unwrap_or("").trim();
    let sheet = if selector.is_empty() {
        sheets[0].clone()
    } else {
        resolve_sheet(&sheets, selector)?
    };
    let rels = relationships(file, "xl/_rels/workbook.xml.rels")?;
    let target = rels
        .get(&sheet.rel_id)
        .ok_or_else(|| CliError::unexpected(format!("missing relationship {}", sheet.rel_id)))?;
    let sheet_part = normalize_xl_target(target);
    if !sheet_part.starts_with("xl/worksheets/") {
        return Err(CliError::invalid_args(format!(
            "sheet {:?} is not a worksheet",
            sheet.name
        )));
    }
    let sheet_xml = zip_text(file, &sheet_part)?;
    Ok((sheet, sheet_part, sheet_xml))
}

fn write_data_validation_mutation(
    file: &str,
    sheet_part: &str,
    updated_xml: &str,
    options: DataValidationOutputOptions<'_>,
) -> CliResult<Option<String>> {
    let output_path = options.out.filter(|value| !value.trim().is_empty());
    let commit_path = if options.in_place {
        Some(file)
    } else {
        output_path
    };
    let readback_path = if options.dry_run || options.in_place || output_path == Some(file) {
        xlsx_ranges_set_temp_path(file)
    } else {
        output_path
            .ok_or_else(|| {
                CliError::invalid_args(
                    "must specify exactly one of --out, --in-place, or --dry-run",
                )
            })?
            .to_string()
    };

    copy_zip_with_part_override(file, &readback_path, sheet_part, updated_xml)?;
    if !options.no_validate {
        validate(&readback_path, true)?;
    }
    if options.dry_run {
        let _ = fs::remove_file(&readback_path);
    } else if options.in_place || output_path == Some(file) {
        if let Some(backup_path) = options.backup.filter(|value| !value.trim().is_empty()) {
            fs::copy(file, backup_path)
                .map_err(|err| CliError::unexpected(format!("failed to create backup: {err}")))?;
        }
        fs::rename(&readback_path, file)
            .or_else(|_| {
                fs::copy(&readback_path, file)?;
                fs::remove_file(&readback_path)
            })
            .map_err(|err| CliError::unexpected(format!("failed to write output file: {err}")))?;
    }
    Ok(commit_path.map(ToOwned::to_owned))
}

fn worksheet_root_bounds(xml: &str) -> CliResult<WorksheetRootBounds> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "worksheet" => {
                let open_end = reader.buffer_position() as usize;
                let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let close_tag = format!("</{tag_name}>");
                let close_start = xml
                    .rfind(&close_tag)
                    .ok_or_else(|| CliError::unexpected("worksheet root has no closing tag"))?;
                return Ok(WorksheetRootBounds {
                    start: before,
                    open_end,
                    close_start,
                    end: close_start + close_tag.len(),
                    tag_name,
                    self_closing: false,
                });
            }
            Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == "worksheet" => {
                let end = reader.buffer_position() as usize;
                return Ok(WorksheetRootBounds {
                    start: before,
                    open_end: end,
                    close_start: end,
                    end,
                    tag_name: String::from_utf8_lossy(e.name().as_ref()).to_string(),
                    self_closing: true,
                });
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                return Err(CliError::unexpected(format!(
                    "worksheet root is {:?}",
                    local_name(e.name().as_ref())
                )));
            }
            Ok(Event::Eof) => return Err(CliError::unexpected("worksheet root not found")),
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
}

fn insert_worksheet_child(
    xml: &str,
    root: &WorksheetRootBounds,
    local_name: &str,
    child_xml: &str,
) -> CliResult<String> {
    if root.self_closing {
        let start_tag = xml_open_tag_from_start(&xml[root.start..root.open_end]);
        let mut updated = String::new();
        updated.push_str(&xml[..root.start]);
        updated.push_str(&start_tag);
        updated.push_str(child_xml);
        updated.push_str(&format!("</{}>", root.tag_name));
        updated.push_str(&xml[root.end..]);
        return Ok(updated);
    }
    let target_order = worksheet_child_order(local_name);
    let insert_at = xml_direct_child_ranges(xml, root.open_end, root.close_start)?
        .into_iter()
        .find(|child| worksheet_child_order(&child.kind) > target_order)
        .map(|child| child.start)
        .unwrap_or(root.close_start);
    Ok(replace_xml_span(xml, insert_at, insert_at, child_xml))
}

fn first_element(fragment: &str) -> CliResult<(String, BTreeMap<String, String>, bool, usize)> {
    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(false);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let end = reader.buffer_position() as usize;
                return Ok((
                    String::from_utf8_lossy(e.name().as_ref()).to_string(),
                    xml_attrs_map(&e),
                    false,
                    end,
                ));
            }
            Ok(Event::Empty(e)) => {
                let end = reader.buffer_position() as usize;
                return Ok((
                    String::from_utf8_lossy(e.name().as_ref()).to_string(),
                    xml_attrs_map(&e),
                    true,
                    end,
                ));
            }
            Ok(Event::Eof) => return Err(CliError::unexpected("XML element not found")),
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
}

fn attr_local(attrs: &BTreeMap<String, String>, wanted: &str) -> Option<String> {
    attrs
        .iter()
        .find(|(key, _)| local_name(key.as_bytes()) == wanted)
        .map(|(_, value)| value.clone())
}

fn attr_is_true(attrs: &BTreeMap<String, String>, key: &str) -> bool {
    attr_local(attrs, key)
        .map(|value| {
            let value = value.trim();
            value == "1" || value == "true"
        })
        .unwrap_or(false)
}

fn set_bool_attr(attrs: &mut BTreeMap<String, String>, key: &str, value: bool) {
    if value {
        attrs.insert(key.to_string(), "1".to_string());
    } else {
        attrs.remove(key);
    }
}

fn set_string_attr(attrs: &mut BTreeMap<String, String>, key: &str, value: &str) {
    if value.is_empty() {
        attrs.remove(key);
    } else {
        attrs.insert(key.to_string(), value.to_string());
    }
}

fn element_name(prefix: &str, local: &str) -> String {
    if prefix.is_empty() {
        local.to_string()
    } else {
        format!("{prefix}:{local}")
    }
}

fn data_validation_sheet_selector(sheet: &WorkbookSheet) -> String {
    format!("sheetId:{}", sheet.sheet_id)
}

fn worksheet_child_order(local_name: &str) -> i32 {
    match local_name {
        "sheetPr" => 10,
        "dimension" => 20,
        "sheetViews" => 30,
        "sheetFormatPr" => 40,
        "cols" => 50,
        "sheetData" => 60,
        "sheetCalcPr" => 70,
        "sheetProtection" => 80,
        "protectedRanges" => 90,
        "scenarios" => 100,
        "autoFilter" => 110,
        "sortState" => 120,
        "dataConsolidate" => 130,
        "customSheetViews" => 140,
        "mergeCells" => 150,
        "phoneticPr" => 160,
        "conditionalFormatting" => 170,
        "dataValidations" => 180,
        "hyperlinks" => 190,
        "printOptions" => 200,
        "pageMargins" => 210,
        "pageSetup" => 220,
        "headerFooter" => 230,
        "rowBreaks" => 240,
        "colBreaks" => 250,
        "customProperties" => 260,
        "cellWatches" => 270,
        "ignoredErrors" => 280,
        "smartTags" => 290,
        "drawing" => 300,
        "legacyDrawing" => 310,
        "legacyDrawingHF" => 320,
        "picture" => 330,
        "oleObjects" => 340,
        "controls" => 350,
        "webPublishItems" => 360,
        "tableParts" => 370,
        "extLst" => 380,
        _ => 1000,
    }
}
