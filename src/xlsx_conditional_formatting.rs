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
    validate_xlsx_mutation_output_flags, workbook_sheets, xlsx_ranges_set_temp_path,
    xml_attr_escape, xml_direct_child_ranges, xml_escape, xml_fragment_bounds, xml_tag_prefix,
    zip_text,
};

mod color_scale;
mod data_bar;
mod icon_set;
mod sqref;
mod xml_support;

use color_scale::{
    ConditionalFormatCfvo, ConditionalFormatColor, ConditionalFormatColorScale, color_scale_json,
    parse_cfvo_spec, parse_color_scale, render_color_scale, validate_color_scale,
};
use data_bar::{
    ConditionalFormatDataBar, data_bar_json, parse_data_bar, render_data_bar, validate_data_bar,
};
use icon_set::{
    ConditionalFormatIconSet, icon_set_json, parse_icon_set, render_icon_set, validate_icon_set,
};
use sqref::{normalize_sqref, sqref_cell_count};
use xml_support::{
    WorksheetRootBounds, attr_is_true, attr_local, element_name, first_element,
    insert_worksheet_child, worksheet_root_bounds,
};

#[derive(Clone, Debug)]
struct ConditionalFormatBlock {
    index: usize,
    sqref: String,
    rules: Vec<ConditionalFormatRule>,
}

#[derive(Clone, Debug)]
struct ConditionalFormatRule {
    index: usize,
    block_index: usize,
    rule_index: usize,
    primary_selector: String,
    selectors: Vec<String>,
    sqref: String,
    rule_type: String,
    operator: String,
    priority: Option<i64>,
    formulas: Vec<String>,
    dxf_id: Option<i64>,
    stop_if_true: bool,
    color_scale: Option<ConditionalFormatColorScale>,
    data_bar: Option<ConditionalFormatDataBar>,
    icon_set: Option<ConditionalFormatIconSet>,
}

#[derive(Clone, Copy)]
struct ConditionalFormatOutputOptions<'a> {
    out: Option<&'a str>,
    backup: Option<&'a str>,
    dry_run: bool,
    no_validate: bool,
    in_place: bool,
}

pub(crate) struct XlsxConditionalFormatMutationOptions<'a> {
    pub(crate) sheet: Option<&'a str>,
    pub(crate) range: Option<&'a str>,
    pub(crate) rule: Option<&'a str>,
    pub(crate) formula: Option<&'a str>,
    pub(crate) rule_type: Option<&'a str>,
    pub(crate) operator: Option<&'a str>,
    pub(crate) formula2: Option<&'a str>,
    pub(crate) has_formula2: bool,
    pub(crate) cfvo: Vec<String>,
    pub(crate) colors: Vec<String>,
    pub(crate) icon_set: Option<&'a str>,
    pub(crate) priority: Option<i64>,
    pub(crate) stop_if_true: bool,
    pub(crate) has_stop_if_true: bool,
    pub(crate) dxf_id: Option<i64>,
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
}

struct ConditionalFormatMutation {
    updated_xml: String,
    sqref: String,
    rule: ConditionalFormatRule,
    cells_affected: i64,
    old_priority: Option<i64>,
    new_priority: Option<i64>,
}

pub(crate) fn xlsx_conditional_formats_list(
    file: &str,
    sheet_selector: Option<&str>,
    range_filter: Option<&str>,
) -> CliResult<Value> {
    let norm_filter = range_filter
        .filter(|value| !value.trim().is_empty())
        .map(normalize_sqref)
        .transpose()
        .map_err(|err| CliError::invalid_args(format!("invalid --range: {}", err.message)))?;
    let (sheet, _sheet_part, sheet_xml) = resolve_conditional_format_sheet(file, sheet_selector)?;
    let blocks = read_conditional_formats(&sheet_xml)?;
    Ok(conditional_formats_list_json(
        file,
        &sheet,
        sheet_selector,
        &blocks,
        norm_filter.as_deref(),
    ))
}

pub(crate) fn xlsx_conditional_formats_show(
    file: &str,
    sheet_selector: Option<&str>,
    selector: &str,
) -> CliResult<Value> {
    if selector.trim().is_empty() {
        return Err(CliError::invalid_args("--rule is required"));
    }
    let (sheet, _sheet_part, sheet_xml) = resolve_conditional_format_sheet(file, sheet_selector)?;
    let blocks = read_conditional_formats(&sheet_xml)?;
    let rule = select_conditional_format_rule(&blocks, selector)
        .map_err(|_| conditional_format_rule_not_found(&blocks, selector, &sheet))?;
    Ok(conditional_format_rule_json(&rule))
}

pub(crate) fn xlsx_conditional_formats_add(
    file: &str,
    options: XlsxConditionalFormatMutationOptions<'_>,
) -> CliResult<Value> {
    let rule_type = normalize_conditional_format_add_type(options.rule_type);
    if options.range.is_none_or(|value| value.trim().is_empty()) {
        return Err(CliError::invalid_args("--range is required"));
    }
    match rule_type.as_str() {
        "expression" => {
            if options.formula.is_none_or(|value| value.trim().is_empty()) {
                return Err(CliError::invalid_args("--formula is required"));
            }
            if options.operator.is_some() {
                return Err(CliError::invalid_args(
                    "--operator is only valid with --type cell-is",
                ));
            }
            if options.has_formula2 {
                return Err(CliError::invalid_args(
                    "--formula2 is only valid with --type cell-is",
                ));
            }
            if !options.cfvo.is_empty() || !options.colors.is_empty() {
                return Err(CliError::invalid_args(
                    "--cfvo and --color are only valid with --type color-scale, data-bar, or icon-set",
                ));
            }
            if options.icon_set.is_some() {
                return Err(CliError::invalid_args(
                    "--icon-set is only valid with --type icon-set",
                ));
            }
        }
        "cellIs" => {
            if options.formula.is_none_or(|value| value.trim().is_empty()) {
                return Err(CliError::invalid_args("--formula is required"));
            }
            if !options.cfvo.is_empty() || !options.colors.is_empty() {
                return Err(CliError::invalid_args(
                    "--cfvo and --color are only valid with --type color-scale, data-bar, or icon-set",
                ));
            }
            if options.icon_set.is_some() {
                return Err(CliError::invalid_args(
                    "--icon-set is only valid with --type icon-set",
                ));
            }
        }
        "colorScale" => {
            if options.operator.is_some() {
                return Err(CliError::invalid_args(
                    "--operator is only valid with --type cell-is",
                ));
            }
            if options.formula.is_some() || options.has_formula2 {
                return Err(CliError::invalid_args(
                    "--formula and --formula2 are not valid with --type color-scale",
                ));
            }
            if options.has_stop_if_true {
                return Err(CliError::invalid_args(
                    "--stop-if-true is not valid with --type color-scale",
                ));
            }
            if options.dxf_id.is_some() {
                return Err(CliError::invalid_args(
                    "--dxf-id is not valid with --type color-scale",
                ));
            }
            if options.icon_set.is_some() {
                return Err(CliError::invalid_args(
                    "--icon-set is only valid with --type icon-set",
                ));
            }
        }
        "dataBar" => {
            if options.operator.is_some() {
                return Err(CliError::invalid_args(
                    "--operator is only valid with --type cell-is",
                ));
            }
            if options.formula.is_some() || options.has_formula2 {
                return Err(CliError::invalid_args(
                    "--formula and --formula2 are not valid with --type data-bar",
                ));
            }
            if options.has_stop_if_true {
                return Err(CliError::invalid_args(
                    "--stop-if-true is not valid with --type data-bar",
                ));
            }
            if options.dxf_id.is_some() {
                return Err(CliError::invalid_args(
                    "--dxf-id is not valid with --type data-bar",
                ));
            }
            if options.icon_set.is_some() {
                return Err(CliError::invalid_args(
                    "--icon-set is only valid with --type icon-set",
                ));
            }
        }
        "iconSet" => {
            if options.operator.is_some() {
                return Err(CliError::invalid_args(
                    "--operator is only valid with --type cell-is",
                ));
            }
            if options.formula.is_some() || options.has_formula2 {
                return Err(CliError::invalid_args(
                    "--formula and --formula2 are not valid with --type icon-set",
                ));
            }
            if options.has_stop_if_true {
                return Err(CliError::invalid_args(
                    "--stop-if-true is not valid with --type icon-set",
                ));
            }
            if options.dxf_id.is_some() {
                return Err(CliError::invalid_args(
                    "--dxf-id is not valid with --type icon-set",
                ));
            }
            if options.colors.iter().any(|value| value.trim() != "[]") {
                return Err(CliError::invalid_args(
                    "--color is not valid with --type icon-set",
                ));
            }
            let icon_set_name = options
                .icon_set
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| {
                    CliError::invalid_args("--icon-set is required with --type icon-set")
                })?;
            let cfvo = parse_conditional_format_cfvo_flags(&options.cfvo)?;
            validate_icon_set(icon_set_name, &cfvo)?;
        }
        _ => {
            return Err(CliError::invalid_args(
                "--type must be expression, cell-is, cellIs, color-scale, colorScale, data-bar, dataBar, icon-set, or iconSet",
            ));
        }
    }
    run_conditional_format_mutation(file, "add", options, |xml, prefix, options| {
        add_conditional_format_xml(xml, prefix, options)
    })
}

pub(crate) fn xlsx_conditional_formats_delete(
    file: &str,
    options: XlsxConditionalFormatMutationOptions<'_>,
) -> CliResult<Value> {
    if options.rule.is_none_or(|value| value.trim().is_empty()) {
        return Err(CliError::invalid_args("--rule is required"));
    }
    run_conditional_format_mutation(file, "delete", options, |xml, _prefix, options| {
        delete_conditional_format_xml(xml, options)
    })
}

pub(crate) fn xlsx_conditional_formats_reorder(
    file: &str,
    options: XlsxConditionalFormatMutationOptions<'_>,
) -> CliResult<Value> {
    if options.rule.is_none_or(|value| value.trim().is_empty()) {
        return Err(CliError::invalid_args("--rule is required"));
    }
    let priority = options
        .priority
        .ok_or_else(|| CliError::invalid_args("--priority is required"))?;
    if priority < 1 {
        return Err(CliError::invalid_args(
            "--priority must be greater than zero",
        ));
    }
    run_conditional_format_mutation(file, "reorder", options, |xml, _prefix, options| {
        reorder_conditional_format_xml(xml, options)
    })
}

fn run_conditional_format_mutation<F>(
    file: &str,
    action: &str,
    options: XlsxConditionalFormatMutationOptions<'_>,
    apply: F,
) -> CliResult<Value>
where
    F: FnOnce(
        &str,
        &str,
        &XlsxConditionalFormatMutationOptions<'_>,
    ) -> CliResult<ConditionalFormatMutation>,
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
    let (sheet, sheet_part, sheet_xml) = resolve_conditional_format_sheet(file, options.sheet)?;
    let root = worksheet_root_bounds(&sheet_xml)?;
    let prefix = xml_tag_prefix(&root.tag_name);
    let mutation = apply(&sheet_xml, &prefix, &options).map_err(|err| {
        if err.code == "invalid_args" {
            CliError::invalid_args(format!(
                "failed to {action} conditional format: {}",
                err.message
            ))
        } else {
            err
        }
    })?;
    let output_path = write_conditional_format_mutation(
        file,
        &sheet_part,
        &mutation.updated_xml,
        ConditionalFormatOutputOptions {
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
    result.insert(
        "sheetSelector".to_string(),
        json!(conditional_format_sheet_selector(&sheet)),
    );
    result.insert("action".to_string(), json!(action));
    result.insert("range".to_string(), json!(mutation.sqref));
    result.insert(
        "rule".to_string(),
        conditional_format_rule_json(&mutation.rule),
    );
    result.insert("cellsAffected".to_string(), json!(mutation.cells_affected));
    if let Some(old_priority) = mutation.old_priority
        && old_priority > 0
    {
        result.insert("oldPriority".to_string(), json!(old_priority));
    }
    if let Some(new_priority) = mutation.new_priority
        && new_priority > 0
    {
        result.insert("newPriority".to_string(), json!(new_priority));
    }
    if let Some(output_path) = output_path.as_deref() {
        result.insert("output".to_string(), json!(output_path));
    }
    result.insert("dryRun".to_string(), json!(options.dry_run));
    if let Some(output_path) = output_path.as_deref() {
        let selector = conditional_format_sheet_selector(&sheet);
        result.insert(
            "validateCommand".to_string(),
            json!(format!(
                "ooxml validate --strict {}",
                command_arg(output_path)
            )),
        );
        result.insert(
            "conditionalFormatsListCommand".to_string(),
            json!(format!(
                "ooxml --json xlsx conditional-formats list {} --sheet {}",
                command_arg(output_path),
                command_arg(&selector)
            )),
        );
        if action != "delete" {
            result.insert(
                "conditionalFormatsShowCommand".to_string(),
                json!(format!(
                    "ooxml --json xlsx conditional-formats show {} --sheet {} --rule {}",
                    command_arg(output_path),
                    command_arg(&selector),
                    command_arg(&mutation.rule.primary_selector)
                )),
            );
        }
    }
    Ok(Value::Object(result))
}

fn conditional_formats_list_json(
    file: &str,
    sheet: &WorkbookSheet,
    _sheet_selector: Option<&str>,
    blocks: &[ConditionalFormatBlock],
    range_filter: Option<&str>,
) -> Value {
    let mut json_blocks = Vec::new();
    let mut json_rules = Vec::new();
    for block in blocks {
        if let Some(filter) = range_filter
            && normalize_sqref(&block.sqref).ok().as_deref() != Some(filter)
        {
            continue;
        }
        let rules = block
            .rules
            .iter()
            .map(conditional_format_rule_json)
            .collect::<Vec<_>>();
        json_rules.extend(rules.iter().cloned());
        json_blocks.push(json!({
            "index": block.index,
            "sqref": block.sqref,
            "rules": rules,
        }));
    }
    let mut object = Map::new();
    object.insert("file".to_string(), json!(file));
    object.insert("sheet".to_string(), json!(sheet.name));
    object.insert("sheetNumber".to_string(), json!(sheet.position));
    object.insert(
        "sheetSelector".to_string(),
        json!(conditional_format_sheet_selector(sheet)),
    );
    object.insert("count".to_string(), json!(json_rules.len()));
    object.insert(
        "conditionalFormats".to_string(),
        if json_blocks.is_empty() {
            Value::Null
        } else {
            Value::Array(json_blocks)
        },
    );
    object.insert(
        "rules".to_string(),
        if json_rules.is_empty() {
            Value::Null
        } else {
            Value::Array(json_rules)
        },
    );
    Value::Object(object)
}

fn conditional_format_rule_json(rule: &ConditionalFormatRule) -> Value {
    let mut object = Map::new();
    object.insert("index".to_string(), json!(rule.index));
    object.insert("blockIndex".to_string(), json!(rule.block_index));
    object.insert("ruleIndex".to_string(), json!(rule.rule_index));
    if !rule.primary_selector.is_empty() {
        object.insert("primarySelector".to_string(), json!(rule.primary_selector));
    }
    if !rule.selectors.is_empty() {
        object.insert("selectors".to_string(), json!(rule.selectors));
    }
    object.insert("sqref".to_string(), json!(rule.sqref));
    if !rule.rule_type.is_empty() {
        object.insert("type".to_string(), json!(rule.rule_type));
    }
    if !rule.operator.is_empty() {
        object.insert("operator".to_string(), json!(rule.operator));
    }
    if let Some(priority) = rule.priority {
        object.insert("priority".to_string(), json!(priority));
    }
    if let Some(formula) = rule.formulas.first() {
        object.insert("formula".to_string(), json!(formula));
    }
    if !rule.formulas.is_empty() {
        object.insert("formulas".to_string(), json!(rule.formulas));
    }
    if let Some(dxf_id) = rule.dxf_id {
        object.insert("dxfId".to_string(), json!(dxf_id));
    }
    if rule.stop_if_true {
        object.insert("stopIfTrue".to_string(), json!(true));
    }
    if let Some(color_scale) = rule.color_scale.as_ref() {
        object.insert("colorScale".to_string(), color_scale_json(color_scale));
    }
    if let Some(data_bar) = rule.data_bar.as_ref() {
        object.insert("dataBar".to_string(), data_bar_json(data_bar));
    }
    if let Some(icon_set) = rule.icon_set.as_ref() {
        object.insert("iconSet".to_string(), icon_set_json(icon_set));
    }
    Value::Object(object)
}

fn add_conditional_format_xml(
    xml: &str,
    prefix: &str,
    options: &XlsxConditionalFormatMutationOptions<'_>,
) -> CliResult<ConditionalFormatMutation> {
    let norm_sqref = normalize_sqref(options.range.unwrap_or_default())?;
    let rule_type = normalize_conditional_format_add_type(options.rule_type);
    let formula = options.formula.unwrap_or_default().trim();
    let mut operator = String::new();
    let mut formulas = Vec::<String>::new();
    let mut color_scale = None;
    let mut data_bar = None;
    let mut icon_set = None;
    match rule_type.as_str() {
        "expression" => {
            if formula.is_empty() {
                return Err(CliError::invalid_args("--formula is required"));
            }
            if options.operator.is_some() {
                return Err(CliError::invalid_args(
                    "--operator is only valid with --type cell-is",
                ));
            }
            if options.has_formula2 {
                return Err(CliError::invalid_args(
                    "--formula2 is only valid with --type cell-is",
                ));
            }
            if !options.cfvo.is_empty() || !options.colors.is_empty() {
                return Err(CliError::invalid_args(
                    "--cfvo and --color are only valid with --type color-scale, data-bar, or icon-set",
                ));
            }
            if options.icon_set.is_some() {
                return Err(CliError::invalid_args(
                    "--icon-set is only valid with --type icon-set",
                ));
            }
            formulas.push(formula.to_string());
        }
        "cellIs" => {
            if formula.is_empty() {
                return Err(CliError::invalid_args("--formula is required"));
            }
            if !options.cfvo.is_empty() || !options.colors.is_empty() {
                return Err(CliError::invalid_args(
                    "--cfvo and --color are only valid with --type color-scale, data-bar, or icon-set",
                ));
            }
            if options.icon_set.is_some() {
                return Err(CliError::invalid_args(
                    "--icon-set is only valid with --type icon-set",
                ));
            }
            operator = options.operator.unwrap_or_default().trim().to_string();
            validate_conditional_format_cell_is_operator(&operator)?;
            let formula2 = options.formula2.unwrap_or_default().trim();
            let needs_formula2 = matches!(operator.as_str(), "between" | "notBetween");
            if needs_formula2 && (!options.has_formula2 || formula2.is_empty()) {
                return Err(CliError::invalid_args(format!(
                    "operator {operator:?} requires --formula2"
                )));
            }
            if !needs_formula2 && options.has_formula2 {
                return Err(CliError::invalid_args(
                    "--formula2 is only valid with between or notBetween",
                ));
            }
            formulas.push(formula.to_string());
            if needs_formula2 {
                formulas.push(formula2.to_string());
            }
        }
        "colorScale" => {
            if options.operator.is_some() {
                return Err(CliError::invalid_args(
                    "--operator is only valid with --type cell-is",
                ));
            }
            if options.formula.is_some() || options.has_formula2 {
                return Err(CliError::invalid_args(
                    "--formula and --formula2 are not valid with --type color-scale",
                ));
            }
            if options.has_stop_if_true {
                return Err(CliError::invalid_args(
                    "--stop-if-true is not valid with --type color-scale",
                ));
            }
            if options.dxf_id.is_some() {
                return Err(CliError::invalid_args(
                    "--dxf-id is not valid with --type color-scale",
                ));
            }
            if options.icon_set.is_some() {
                return Err(CliError::invalid_args(
                    "--icon-set is only valid with --type icon-set",
                ));
            }
            let cfvo = parse_conditional_format_cfvo_flags(&options.cfvo)?;
            let colors = parse_conditional_format_color_flags(&options.colors);
            color_scale = Some(validate_color_scale(&cfvo, &colors)?);
        }
        "dataBar" => {
            if options.operator.is_some() {
                return Err(CliError::invalid_args(
                    "--operator is only valid with --type cell-is",
                ));
            }
            if options.formula.is_some() || options.has_formula2 {
                return Err(CliError::invalid_args(
                    "--formula and --formula2 are not valid with --type data-bar",
                ));
            }
            if options.has_stop_if_true {
                return Err(CliError::invalid_args(
                    "--stop-if-true is not valid with --type data-bar",
                ));
            }
            if options.dxf_id.is_some() {
                return Err(CliError::invalid_args(
                    "--dxf-id is not valid with --type data-bar",
                ));
            }
            if options.icon_set.is_some() {
                return Err(CliError::invalid_args(
                    "--icon-set is only valid with --type icon-set",
                ));
            }
            let cfvo = parse_conditional_format_cfvo_flags(&options.cfvo)?;
            let colors = parse_conditional_format_color_flags(&options.colors);
            data_bar = Some(validate_data_bar(&cfvo, &colors)?);
        }
        "iconSet" => {
            if options.operator.is_some() {
                return Err(CliError::invalid_args(
                    "--operator is only valid with --type cell-is",
                ));
            }
            if options.formula.is_some() || options.has_formula2 {
                return Err(CliError::invalid_args(
                    "--formula and --formula2 are not valid with --type icon-set",
                ));
            }
            if options.has_stop_if_true {
                return Err(CliError::invalid_args(
                    "--stop-if-true is not valid with --type icon-set",
                ));
            }
            if options.dxf_id.is_some() {
                return Err(CliError::invalid_args(
                    "--dxf-id is not valid with --type icon-set",
                ));
            }
            if options.colors.iter().any(|value| value.trim() != "[]") {
                return Err(CliError::invalid_args(
                    "--color is not valid with --type icon-set",
                ));
            }
            let cfvo = parse_conditional_format_cfvo_flags(&options.cfvo)?;
            let icon_set_name = options
                .icon_set
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| {
                    CliError::invalid_args("--icon-set is required with --type icon-set")
                })?;
            icon_set = Some(validate_icon_set(icon_set_name, &cfvo)?);
        }
        _ => {
            return Err(CliError::invalid_args(
                "--type must be expression, cell-is, cellIs, color-scale, colorScale, data-bar, dataBar, icon-set, or iconSet",
            ));
        }
    }
    if let Some(priority) = options.priority
        && priority < 1
    {
        return Err(CliError::invalid_args(
            "--priority must be greater than zero",
        ));
    }
    if let Some(dxf_id) = options.dxf_id
        && dxf_id < 0
    {
        return Err(CliError::invalid_args("--dxf-id must be zero or greater"));
    }
    let root = worksheet_root_bounds(xml)?;
    let priority = options
        .priority
        .unwrap_or_else(|| next_conditional_format_priority(xml));
    let rule_xml = if let Some(color_scale) = color_scale.as_ref() {
        render_conditional_format_color_scale_rule(prefix, priority, color_scale)
    } else if let Some(data_bar) = data_bar.as_ref() {
        render_conditional_format_data_bar_rule(prefix, priority, data_bar)
    } else if let Some(icon_set) = icon_set.as_ref() {
        render_conditional_format_icon_set_rule(prefix, priority, icon_set)
    } else {
        render_conditional_format_rule(
            prefix,
            &rule_type,
            &operator,
            &formulas,
            priority,
            options.has_stop_if_true.then_some(options.stop_if_true),
            options.dxf_id,
        )
    };
    let updated_xml =
        if let Some(container) = find_conditional_format_container(xml, &root, &norm_sqref)? {
            insert_rule_into_container(xml, &container, &rule_xml)?
        } else {
            let block_xml = render_new_conditional_format_block(prefix, &norm_sqref, &rule_xml);
            insert_worksheet_child(xml, &root, "conditionalFormatting", &block_xml)?
        };
    let added = read_conditional_formats(&updated_xml)?
        .into_iter()
        .rev()
        .filter(|block| normalize_sqref(&block.sqref).ok().as_deref() == Some(norm_sqref.as_str()))
        .flat_map(|block| block.rules.into_iter().rev())
        .find(|rule| {
            rule.rule_type == rule_type
                && rule.operator == operator
                && rule.priority == Some(priority)
                && rule.formulas == formulas
                && rule.color_scale == color_scale
                && rule.data_bar == data_bar
                && rule.icon_set == icon_set
        })
        .unwrap_or_else(|| ConditionalFormatRule {
            index: 0,
            block_index: 0,
            rule_index: 0,
            primary_selector: String::new(),
            selectors: Vec::new(),
            sqref: norm_sqref.clone(),
            rule_type: rule_type.clone(),
            operator: operator.clone(),
            priority: Some(priority),
            formulas: formulas.clone(),
            dxf_id: options.dxf_id,
            stop_if_true: options.has_stop_if_true && options.stop_if_true,
            color_scale: color_scale.clone(),
            data_bar: data_bar.clone(),
            icon_set: icon_set.clone(),
        });
    Ok(ConditionalFormatMutation {
        updated_xml,
        sqref: norm_sqref.clone(),
        rule: added,
        cells_affected: sqref_cell_count(&norm_sqref),
        old_priority: None,
        new_priority: None,
    })
}

fn delete_conditional_format_xml(
    xml: &str,
    options: &XlsxConditionalFormatMutationOptions<'_>,
) -> CliResult<ConditionalFormatMutation> {
    let selector = options.rule.unwrap_or_default().trim();
    if selector.is_empty() {
        return Err(CliError::invalid_args("--rule is required"));
    }
    let root = worksheet_root_bounds(xml)?;
    let blocks = read_conditional_formats(xml)?;
    let rule = select_conditional_format_rule(&blocks, selector)?;
    let Some(container) = conditional_format_container_ranges(xml, &root)?
        .into_iter()
        .nth(rule.block_index.saturating_sub(1))
    else {
        return Err(CliError::unexpected(
            "conditional format block disappeared during lookup",
        ));
    };
    let rule_ranges = conditional_format_rule_ranges(xml, &container)?;
    let Some(rule_range) = rule_ranges.get(rule.rule_index.saturating_sub(1)) else {
        return Err(CliError::unexpected(
            "conditional format rule disappeared during lookup",
        ));
    };
    let updated_xml = if rule_ranges.len() == 1 {
        replace_xml_span(xml, container.start, container.end, "")
    } else {
        replace_xml_span(xml, rule_range.start, rule_range.end, "")
    };
    Ok(ConditionalFormatMutation {
        updated_xml,
        sqref: rule.sqref.clone(),
        cells_affected: sqref_cell_count(&rule.sqref),
        rule,
        old_priority: None,
        new_priority: None,
    })
}

fn reorder_conditional_format_xml(
    xml: &str,
    options: &XlsxConditionalFormatMutationOptions<'_>,
) -> CliResult<ConditionalFormatMutation> {
    let selector = options.rule.unwrap_or_default().trim();
    if selector.is_empty() {
        return Err(CliError::invalid_args("--rule is required"));
    }
    let target_priority = options
        .priority
        .ok_or_else(|| CliError::invalid_args("--priority is required"))?;
    if target_priority < 1 {
        return Err(CliError::invalid_args(
            "--priority must be greater than zero",
        ));
    }

    let root = worksheet_root_bounds(xml)?;
    let blocks = read_conditional_formats(xml)?;
    let selected = select_conditional_format_rule(&blocks, selector)?;
    let rules = blocks
        .iter()
        .flat_map(|block| block.rules.iter().cloned())
        .collect::<Vec<_>>();
    if target_priority > rules.len() as i64 {
        return Err(CliError::invalid_args(format!(
            "--priority must be between 1 and {}",
            rules.len()
        )));
    }

    let rule_ranges = conditional_format_rule_ranges_by_document_order(xml, &root)?;
    if rule_ranges.len() != rules.len() {
        return Err(CliError::unexpected(
            "conditional format rule count changed during lookup",
        ));
    }

    let selected_doc_index = selected.index.checked_sub(1).ok_or_else(|| {
        CliError::unexpected("conditional format rule has invalid document-order index")
    })?;
    let mut priority_order = (0..rules.len()).collect::<Vec<_>>();
    priority_order.sort_by(|left, right| {
        conditional_format_priority_sort_key(&rules[*left])
            .cmp(&conditional_format_priority_sort_key(&rules[*right]))
    });
    let selected_order_index = priority_order
        .iter()
        .position(|index| *index == selected_doc_index)
        .ok_or_else(|| {
            CliError::unexpected("selected conditional format rule disappeared during ordering")
        })?;
    let selected_entry = priority_order.remove(selected_order_index);
    priority_order.insert(target_priority as usize - 1, selected_entry);

    let mut new_priorities = vec![0i64; rules.len()];
    for (position, doc_index) in priority_order.into_iter().enumerate() {
        new_priorities[doc_index] = position as i64 + 1;
    }
    let updated_xml =
        rewrite_conditional_format_rule_priorities(xml, &rule_ranges, &new_priorities)?;
    let updated_rule = read_conditional_formats(&updated_xml)?
        .into_iter()
        .flat_map(|block| block.rules.into_iter())
        .find(|rule| rule.index == selected.index)
        .ok_or_else(|| {
            CliError::unexpected("selected conditional format rule disappeared after reorder")
        })?;

    Ok(ConditionalFormatMutation {
        updated_xml,
        sqref: updated_rule.sqref.clone(),
        cells_affected: sqref_cell_count(&updated_rule.sqref),
        rule: updated_rule,
        old_priority: selected.priority,
        new_priority: Some(target_priority),
    })
}

fn read_conditional_formats(xml: &str) -> CliResult<Vec<ConditionalFormatBlock>> {
    let root = worksheet_root_bounds(xml)?;
    let mut blocks = Vec::new();
    let mut global_rule_index = 0usize;
    for container in conditional_format_container_ranges(xml, &root)? {
        let (_, attrs, _, _) = first_element(&xml[container.start..container.end])?;
        let sqref = attr_local(&attrs, "sqref").unwrap_or_default();
        let mut block = ConditionalFormatBlock {
            index: blocks.len() + 1,
            sqref,
            rules: Vec::new(),
        };
        for rule_range in conditional_format_rule_ranges(xml, &container)? {
            global_rule_index += 1;
            let mut rule = parse_conditional_format_rule(
                &xml[rule_range.start..rule_range.end],
                block.index,
                block.rules.len() + 1,
                global_rule_index,
                &block.sqref,
            )?;
            rule.selectors = conditional_format_rule_selectors(&rule);
            block.rules.push(rule);
        }
        blocks.push(block);
    }
    Ok(blocks)
}

fn parse_conditional_format_rule(
    fragment: &str,
    block_index: usize,
    rule_index: usize,
    global_index: usize,
    sqref: &str,
) -> CliResult<ConditionalFormatRule> {
    let (_, attrs, _, _) = first_element(fragment)?;
    let priority = attr_local(&attrs, "priority").and_then(|value| value.parse::<i64>().ok());
    let dxf_id = attr_local(&attrs, "dxfId").and_then(|value| value.parse::<i64>().ok());
    let mut rule = ConditionalFormatRule {
        index: global_index,
        block_index,
        rule_index,
        primary_selector: format!("cfRule:{global_index}"),
        selectors: Vec::new(),
        sqref: sqref.to_string(),
        rule_type: attr_local(&attrs, "type").unwrap_or_default(),
        operator: attr_local(&attrs, "operator").unwrap_or_default(),
        priority,
        formulas: Vec::new(),
        dxf_id,
        stop_if_true: attr_is_true(&attrs, "stopIfTrue"),
        color_scale: parse_color_scale(fragment)?,
        data_bar: parse_data_bar(fragment)?,
        icon_set: parse_icon_set(fragment)?,
    };
    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(false);
    let mut stack = Vec::<String>::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if name == "formula" {
                    rule.formulas.push(String::new());
                }
                stack.push(name);
            }
            Ok(event) if is_xml_text_event(&event) => {
                if stack.last().map(String::as_str) == Some("formula")
                    && let Some(formula) = rule.formulas.last_mut()
                {
                    append_xml_text_event(formula, &event);
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
    Ok(rule)
}

fn conditional_format_rule_selectors(rule: &ConditionalFormatRule) -> Vec<String> {
    let mut selectors = vec![
        format!("cfRule:{}", rule.index),
        format!("rule:{}", rule.index),
        format!("block:{}/rule:{}", rule.block_index, rule.rule_index),
    ];
    if let Some(priority) = rule.priority
        && priority > 0
    {
        selectors.push(format!("priority:{priority}"));
    }
    if !rule.sqref.is_empty() {
        selectors.push(format!("sqref:{}", rule.sqref));
    }
    selectors
}

fn select_conditional_format_rule(
    blocks: &[ConditionalFormatBlock],
    selector: &str,
) -> CliResult<ConditionalFormatRule> {
    let matches = blocks
        .iter()
        .flat_map(|block| block.rules.iter())
        .filter(|rule| conditional_format_rule_matches(rule, selector))
        .cloned()
        .collect::<Vec<_>>();
    match matches.len() {
        0 => Err(CliError::invalid_args(format!(
            "no conditional format rule found for {selector:?}"
        ))),
        1 => Ok(matches[0].clone()),
        _ => Err(CliError::invalid_args(format!(
            "conditional format rule selector {selector:?} is ambiguous"
        ))),
    }
}

fn conditional_format_rule_matches(rule: &ConditionalFormatRule, selector: &str) -> bool {
    let selector = selector.trim();
    if selector.is_empty() {
        return false;
    }
    if let Ok(index) = selector.parse::<usize>() {
        return rule.index == index;
    }
    rule.selectors.iter().any(|candidate| candidate == selector)
}

fn conditional_format_priority_sort_key(rule: &ConditionalFormatRule) -> (u8, i64, usize) {
    match rule.priority {
        Some(priority) if priority > 0 => (0, priority, rule.index),
        _ => (1, 0, rule.index),
    }
}

fn conditional_format_rule_not_found(
    blocks: &[ConditionalFormatBlock],
    selector: &str,
    sheet: &WorkbookSheet,
) -> CliError {
    let candidates = blocks
        .iter()
        .flat_map(|block| block.rules.iter())
        .map(|rule| (rule.primary_selector.as_str(), rule.selectors.as_slice()))
        .collect::<Vec<_>>();
    let mut message = format!("conditional format rule not found: {selector}");
    if !candidates.is_empty() {
        let suggestions = selector_candidates(&candidates, selector, 5);
        if !suggestions.is_empty() {
            message.push_str(&format!("; did you mean: {}", suggestions.join(", ")));
        }
    }
    message.push_str(&format!(
        "; discover with `ooxml --json xlsx conditional-formats list <file> --sheet {}`",
        command_arg(&conditional_format_sheet_selector(sheet))
    ));
    CliError::target_not_found(message)
}

fn conditional_format_container_ranges(
    xml: &str,
    root: &WorksheetRootBounds,
) -> CliResult<Vec<crate::XmlNamedRange>> {
    Ok(
        xml_direct_child_ranges(xml, root.open_end, root.close_start)?
            .into_iter()
            .filter(|child| child.kind == "conditionalFormatting")
            .collect(),
    )
}

fn find_conditional_format_container(
    xml: &str,
    root: &WorksheetRootBounds,
    norm_sqref: &str,
) -> CliResult<Option<crate::XmlNamedRange>> {
    for container in conditional_format_container_ranges(xml, root)? {
        let (_, attrs, _, _) = first_element(&xml[container.start..container.end])?;
        if attr_local(&attrs, "sqref")
            .and_then(|sqref| normalize_sqref(&sqref).ok())
            .as_deref()
            == Some(norm_sqref)
        {
            return Ok(Some(container));
        }
    }
    Ok(None)
}

fn conditional_format_rule_ranges(
    xml: &str,
    container: &crate::XmlNamedRange,
) -> CliResult<Vec<crate::XmlNamedRange>> {
    let (open_end, close_start, self_closing) = container_inner_bounds(xml, container)?;
    if self_closing {
        return Ok(Vec::new());
    }
    Ok(xml_direct_child_ranges(xml, open_end, close_start)?
        .into_iter()
        .filter(|child| child.kind == "cfRule")
        .collect())
}

fn conditional_format_rule_ranges_by_document_order(
    xml: &str,
    root: &WorksheetRootBounds,
) -> CliResult<Vec<crate::XmlNamedRange>> {
    let mut ranges = Vec::new();
    for container in conditional_format_container_ranges(xml, root)? {
        ranges.extend(conditional_format_rule_ranges(xml, &container)?);
    }
    Ok(ranges)
}

fn rewrite_conditional_format_rule_priorities(
    xml: &str,
    rule_ranges: &[crate::XmlNamedRange],
    priorities: &[i64],
) -> CliResult<String> {
    if rule_ranges.len() != priorities.len() {
        return Err(CliError::unexpected(
            "conditional format priority rewrite count mismatch",
        ));
    }
    let mut updated = xml.to_string();
    for (rule_range, priority) in rule_ranges.iter().zip(priorities.iter()).rev() {
        let rewritten = set_conditional_format_rule_priority(
            &updated[rule_range.start..rule_range.end],
            *priority,
        )?;
        updated = replace_xml_span(&updated, rule_range.start, rule_range.end, &rewritten);
    }
    Ok(updated)
}

fn set_conditional_format_rule_priority(fragment: &str, priority: i64) -> CliResult<String> {
    let open_end = fragment
        .find('>')
        .ok_or_else(|| CliError::unexpected("invalid conditional format rule XML"))?;
    let updated_open_tag = replace_or_insert_start_tag_attr(
        &fragment[..=open_end],
        "priority",
        &priority.to_string(),
    )?;
    Ok(replace_xml_span(
        fragment,
        0,
        open_end + 1,
        &updated_open_tag,
    ))
}

fn replace_or_insert_start_tag_attr(
    tag: &str,
    attr_local_name: &str,
    value: &str,
) -> CliResult<String> {
    let tag_end = tag
        .rfind('>')
        .ok_or_else(|| CliError::unexpected("invalid conditional format rule XML"))?;
    if !tag.starts_with('<') {
        return Err(CliError::unexpected("invalid conditional format rule XML"));
    }

    let bytes = tag.as_bytes();
    let mut cursor = 1usize;
    while cursor < tag_end && !bytes[cursor].is_ascii_whitespace() && bytes[cursor] != b'/' {
        cursor += 1;
    }
    while cursor < tag_end {
        while cursor < tag_end && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor >= tag_end || bytes[cursor] == b'/' {
            break;
        }
        let name_start = cursor;
        while cursor < tag_end
            && !bytes[cursor].is_ascii_whitespace()
            && bytes[cursor] != b'='
            && bytes[cursor] != b'/'
        {
            cursor += 1;
        }
        let name_end = cursor;
        while cursor < tag_end && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor >= tag_end || bytes[cursor] != b'=' {
            return Err(CliError::unexpected(
                "invalid conditional format rule attribute",
            ));
        }
        cursor += 1;
        while cursor < tag_end && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor >= tag_end || !matches!(bytes[cursor], b'"' | b'\'') {
            return Err(CliError::unexpected(
                "invalid conditional format rule attribute",
            ));
        }
        let quote = bytes[cursor];
        let value_start = cursor + 1;
        cursor = value_start;
        while cursor < tag_end && bytes[cursor] != quote {
            cursor += 1;
        }
        if cursor >= tag_end {
            return Err(CliError::unexpected(
                "invalid conditional format rule attribute",
            ));
        }
        let value_end = cursor;
        if local_name(&tag.as_bytes()[name_start..name_end]) == attr_local_name {
            let mut out = String::with_capacity(tag.len() + value.len());
            out.push_str(&tag[..value_start]);
            out.push_str(&xml_attr_escape(value));
            out.push_str(&tag[value_end..]);
            return Ok(out);
        }
        cursor += 1;
    }

    let insert_at = if tag[..=tag_end].trim_end().ends_with("/>") {
        tag[..tag_end].rfind('/').unwrap_or(tag_end)
    } else {
        tag_end
    };
    let mut out = String::with_capacity(tag.len() + attr_local_name.len() + value.len() + 4);
    out.push_str(&tag[..insert_at]);
    out.push(' ');
    out.push_str(attr_local_name);
    out.push_str("=\"");
    out.push_str(&xml_attr_escape(value));
    out.push('"');
    out.push_str(&tag[insert_at..]);
    Ok(out)
}

fn insert_rule_into_container(
    xml: &str,
    container: &crate::XmlNamedRange,
    rule_xml: &str,
) -> CliResult<String> {
    let (_open_end, close_start, self_closing) = container_inner_bounds(xml, container)?;
    if !self_closing {
        return Ok(replace_xml_span(xml, close_start, close_start, rule_xml));
    }
    let (tag_name, attrs, _, _) = first_element(&xml[container.start..container.end])?;
    let tag = if tag_name.is_empty() {
        "conditionalFormatting".to_string()
    } else {
        tag_name
    };
    let replacement = format!(
        "<{}{}>{}</{}>",
        tag,
        render_xml_attrs(&attrs),
        rule_xml,
        tag
    );
    Ok(replace_xml_span(
        xml,
        container.start,
        container.end,
        &replacement,
    ))
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

fn normalize_conditional_format_add_type(rule_type: Option<&str>) -> String {
    match rule_type.unwrap_or_default().trim() {
        "" | "expression" => "expression".to_string(),
        "cell-is" | "cellIs" => "cellIs".to_string(),
        "color-scale" | "colorScale" => "colorScale".to_string(),
        "data-bar" | "dataBar" => "dataBar".to_string(),
        "icon-set" | "iconSet" => "iconSet".to_string(),
        other => other.to_string(),
    }
}

fn validate_conditional_format_cell_is_operator(operator: &str) -> CliResult<()> {
    if operator.is_empty() {
        return Err(CliError::invalid_args(
            "--operator is required for cellIs conditional formats",
        ));
    }
    if !matches!(
        operator,
        "between"
            | "notBetween"
            | "equal"
            | "notEqual"
            | "greaterThan"
            | "lessThan"
            | "greaterThanOrEqual"
            | "lessThanOrEqual"
    ) {
        return Err(CliError::invalid_args(format!(
            "invalid operator {operator:?} (use one of between, notBetween, equal, notEqual, greaterThan, lessThan, greaterThanOrEqual, lessThanOrEqual)"
        )));
    }
    Ok(())
}

fn parse_conditional_format_cfvo_flags(values: &[String]) -> CliResult<Vec<ConditionalFormatCfvo>> {
    values
        .iter()
        .filter(|value| value.trim() != "[]")
        .map(|value| parse_cfvo_spec(value))
        .collect()
}

fn parse_conditional_format_color_flags(values: &[String]) -> Vec<ConditionalFormatColor> {
    values
        .iter()
        .filter(|value| value.trim() != "[]")
        .map(|value| ConditionalFormatColor { rgb: value.clone() })
        .collect()
}

fn render_conditional_format_rule(
    prefix: &str,
    rule_type: &str,
    operator: &str,
    formulas: &[String],
    priority: i64,
    stop_if_true: Option<bool>,
    dxf_id: Option<i64>,
) -> String {
    let tag = element_name(prefix, "cfRule");
    let mut attrs = BTreeMap::new();
    attrs.insert("priority".to_string(), priority.to_string());
    attrs.insert("type".to_string(), rule_type.to_string());
    if !operator.is_empty() {
        attrs.insert("operator".to_string(), operator.to_string());
    }
    if let Some(stop_if_true) = stop_if_true
        && stop_if_true
    {
        attrs.insert("stopIfTrue".to_string(), "1".to_string());
    }
    if let Some(dxf_id) = dxf_id {
        attrs.insert("dxfId".to_string(), dxf_id.to_string());
    }
    let formula_tag = element_name(prefix, "formula");
    let formula_xml = formulas
        .iter()
        .map(|formula| format!("<{}>{}</{}>", formula_tag, xml_escape(formula), formula_tag))
        .collect::<String>();
    format!(
        "<{}{}>{}</{}>",
        tag,
        render_xml_attrs(&attrs),
        formula_xml,
        tag
    )
}

fn render_conditional_format_color_scale_rule(
    prefix: &str,
    priority: i64,
    color_scale: &ConditionalFormatColorScale,
) -> String {
    let rule_tag = element_name(prefix, "cfRule");
    let mut attrs = BTreeMap::new();
    attrs.insert("priority".to_string(), priority.to_string());
    attrs.insert("type".to_string(), "colorScale".to_string());
    let color_scale_xml = render_color_scale(prefix, color_scale);
    format!(
        "<{}{}>{}</{}>",
        rule_tag,
        render_xml_attrs(&attrs),
        color_scale_xml,
        rule_tag
    )
}

fn render_conditional_format_data_bar_rule(
    prefix: &str,
    priority: i64,
    data_bar: &ConditionalFormatDataBar,
) -> String {
    let rule_tag = element_name(prefix, "cfRule");
    let mut attrs = BTreeMap::new();
    attrs.insert("priority".to_string(), priority.to_string());
    attrs.insert("type".to_string(), "dataBar".to_string());
    let data_bar_xml = render_data_bar(prefix, data_bar);
    format!(
        "<{}{}>{}</{}>",
        rule_tag,
        render_xml_attrs(&attrs),
        data_bar_xml,
        rule_tag
    )
}

fn render_conditional_format_icon_set_rule(
    prefix: &str,
    priority: i64,
    icon_set: &ConditionalFormatIconSet,
) -> String {
    let rule_tag = element_name(prefix, "cfRule");
    let mut attrs = BTreeMap::new();
    attrs.insert("priority".to_string(), priority.to_string());
    attrs.insert("type".to_string(), "iconSet".to_string());
    let icon_set_xml = render_icon_set(prefix, icon_set);
    format!(
        "<{}{}>{}</{}>",
        rule_tag,
        render_xml_attrs(&attrs),
        icon_set_xml,
        rule_tag
    )
}

fn render_new_conditional_format_block(prefix: &str, sqref: &str, rule_xml: &str) -> String {
    let tag = element_name(prefix, "conditionalFormatting");
    let mut attrs = BTreeMap::new();
    attrs.insert("sqref".to_string(), sqref.to_string());
    format!(
        "<{}{}>{}</{}>",
        tag,
        render_xml_attrs(&attrs),
        rule_xml,
        tag
    )
}

fn next_conditional_format_priority(xml: &str) -> i64 {
    let mut max_priority = 0i64;
    let mut rule_count = 0i64;
    if let Ok(blocks) = read_conditional_formats(xml) {
        for rule in blocks.into_iter().flat_map(|block| block.rules.into_iter()) {
            rule_count += 1;
            if let Some(priority) = rule.priority
                && priority > max_priority
            {
                max_priority = priority;
            }
        }
    }
    if max_priority > 0 {
        max_priority + 1
    } else {
        rule_count + 1
    }
}

fn write_conditional_format_mutation(
    file: &str,
    sheet_part: &str,
    updated_xml: &str,
    options: ConditionalFormatOutputOptions<'_>,
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

fn resolve_conditional_format_sheet(
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

fn conditional_format_sheet_selector(sheet: &WorkbookSheet) -> String {
    format!("sheetId:{}", sheet.sheet_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reorder_options<'a>(
        rule: &'a str,
        priority: Option<i64>,
    ) -> XlsxConditionalFormatMutationOptions<'a> {
        XlsxConditionalFormatMutationOptions {
            sheet: None,
            range: None,
            rule: Some(rule),
            formula: None,
            rule_type: None,
            operator: None,
            formula2: None,
            has_formula2: false,
            cfvo: Vec::new(),
            colors: Vec::new(),
            icon_set: None,
            priority,
            stop_if_true: false,
            has_stop_if_true: false,
            dxf_id: None,
            out: None,
            backup: None,
            dry_run: true,
            no_validate: true,
            in_place: false,
        }
    }

    #[test]
    fn reorders_priorities_and_preserves_rule_payloads() {
        let xml = r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:x14="http://schemas.microsoft.com/office/spreadsheetml/2009/9/main"><sheetData/><conditionalFormatting sqref="A1:A5"><cfRule type="expression" priority="3" dxfId="2"><formula>A1&gt;10</formula><extLst><ext uri="{ABC}"><x14:id>keep-me</x14:id></ext></extLst></cfRule><cfRule type="dataBar" priority="1"><dataBar><cfvo type="min"/><cfvo type="max"/><color rgb="FF638EC6"/></dataBar></cfRule></conditionalFormatting><conditionalFormatting sqref="B1:B5"><cfRule type="iconSet" priority="2"><iconSet iconSet="3TrafficLights1"><cfvo type="percent" val="0"/><cfvo type="percent" val="33"/><cfvo type="percent" val="67"/></iconSet></cfRule></conditionalFormatting></worksheet>"#;

        let mutation = reorder_conditional_format_xml(xml, &reorder_options("cfRule:1", Some(1)))
            .expect("reorder conditional format priorities");
        let blocks = read_conditional_formats(&mutation.updated_xml).expect("read updated rules");
        let priorities = blocks
            .iter()
            .flat_map(|block| block.rules.iter().map(|rule| rule.priority))
            .collect::<Vec<_>>();

        assert_eq!(mutation.rule.rule_type, "expression");
        assert_eq!(mutation.rule.priority, Some(1));
        assert_eq!(mutation.rule.formulas, vec!["A1>10".to_string()]);
        assert!(mutation.rule.selectors.contains(&"priority:1".to_string()));
        assert_eq!(priorities, vec![Some(1), Some(2), Some(3)]);
        assert!(mutation.updated_xml.contains(
            r#"<formula>A1&gt;10</formula><extLst><ext uri="{ABC}"><x14:id>keep-me</x14:id></ext></extLst>"#
        ));
        assert!(mutation.updated_xml.contains(
            r#"<dataBar><cfvo type="min"/><cfvo type="max"/><color rgb="FF638EC6"/></dataBar>"#
        ));
        assert!(mutation.updated_xml.contains(
            r#"<iconSet iconSet="3TrafficLights1"><cfvo type="percent" val="0"/><cfvo type="percent" val="33"/><cfvo type="percent" val="67"/></iconSet>"#
        ));
        let expression_pos = mutation
            .updated_xml
            .find(r#"<cfRule type="expression" priority="1""#)
            .expect("expression rule position");
        let data_bar_pos = mutation
            .updated_xml
            .find(r#"<cfRule type="dataBar" priority="2""#)
            .expect("data bar rule position");
        let icon_set_pos = mutation
            .updated_xml
            .find(r#"<cfRule type="iconSet" priority="3""#)
            .expect("icon set rule position");
        assert!(expression_pos < data_bar_pos);
        assert!(data_bar_pos < icon_set_pos);
    }

    #[test]
    fn reorder_uses_priority_order_with_document_order_ties() {
        let xml = r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData/><conditionalFormatting sqref="A1:A3"><cfRule type="expression" priority="20"><formula>A1</formula></cfRule><cfRule type="expression" priority="10"><formula>A2</formula></cfRule><cfRule type="expression" priority="10"><formula>A3</formula></cfRule></conditionalFormatting></worksheet>"#;

        let mutation = reorder_conditional_format_xml(xml, &reorder_options("cfRule:1", Some(2)))
            .expect("reorder conditional format priorities");
        let priorities = read_conditional_formats(&mutation.updated_xml)
            .expect("read updated rules")
            .into_iter()
            .flat_map(|block| block.rules.into_iter().map(|rule| rule.priority))
            .collect::<Vec<_>>();

        assert_eq!(mutation.rule.priority, Some(2));
        assert_eq!(priorities, vec![Some(2), Some(1), Some(3)]);
    }

    #[test]
    fn reorder_adds_missing_priority_attributes() {
        let xml = r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData/><conditionalFormatting sqref="A1:A2"><cfRule type="expression"><formula>TRUE</formula></cfRule><cfRule type="expression" priority="1"><formula>FALSE</formula></cfRule></conditionalFormatting></worksheet>"#;

        let mutation = reorder_conditional_format_xml(xml, &reorder_options("cfRule:1", Some(1)))
            .expect("reorder conditional format priorities");
        let priorities = read_conditional_formats(&mutation.updated_xml)
            .expect("read updated rules")
            .into_iter()
            .flat_map(|block| block.rules.into_iter().map(|rule| rule.priority))
            .collect::<Vec<_>>();

        assert_eq!(mutation.rule.priority, Some(1));
        assert_eq!(priorities, vec![Some(1), Some(2)]);
        assert!(mutation.updated_xml.contains(
            r#"<cfRule type="expression" priority="1"><formula>TRUE</formula></cfRule>"#
        ));
    }

    #[test]
    fn reorder_rejects_missing_zero_and_out_of_range_priority() {
        let xml = r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData/><conditionalFormatting sqref="A1:A2"><cfRule type="expression" priority="1"><formula>TRUE</formula></cfRule><cfRule type="expression" priority="2"><formula>FALSE</formula></cfRule></conditionalFormatting></worksheet>"#;

        let missing = reorder_conditional_format_xml(xml, &reorder_options("cfRule:1", None))
            .err()
            .expect("missing priority should fail");
        assert_eq!(missing.code, "invalid_args");
        assert!(missing.message.contains("--priority is required"));

        let zero = reorder_conditional_format_xml(xml, &reorder_options("cfRule:1", Some(0)))
            .err()
            .expect("zero priority should fail");
        assert_eq!(zero.code, "invalid_args");
        assert!(
            zero.message
                .contains("--priority must be greater than zero")
        );

        let out_of_range =
            reorder_conditional_format_xml(xml, &reorder_options("cfRule:1", Some(3)))
                .err()
                .expect("out-of-range priority should fail");
        assert_eq!(out_of_range.code, "invalid_args");
        assert!(
            out_of_range
                .message
                .contains("--priority must be between 1 and 2")
        );
    }

    #[test]
    fn parses_data_bar_rule_json() {
        let xml = r#"<cfRule type="dataBar" priority="4"><dataBar><cfvo type="min"/><cfvo type="max"/><color rgb="FF638EC6"/></dataBar></cfRule>"#;
        let rule =
            parse_conditional_format_rule(xml, 1, 1, 1, "A1:A5").expect("parse data bar rule");
        let readback = conditional_format_rule_json(&rule);

        assert_eq!(readback["type"], "dataBar");
        assert_eq!(readback["priority"], 4);
        assert_eq!(
            readback["dataBar"]["cfvo"],
            serde_json::json!([
                { "type": "min" },
                { "type": "max" },
            ])
        );
        assert_eq!(
            readback["dataBar"]["color"],
            serde_json::json!({ "rgb": "FF638EC6" })
        );
    }

    #[test]
    fn parses_icon_set_rule_json() {
        let xml = r#"<cfRule type="iconSet" priority="4"><iconSet iconSet="3TrafficLights1"><cfvo type="percent" val="0"/><cfvo type="percent" val="33"/><cfvo type="percent" val="67"/></iconSet></cfRule>"#;
        let rule =
            parse_conditional_format_rule(xml, 1, 1, 1, "A1:A5").expect("parse icon set rule");
        let readback = conditional_format_rule_json(&rule);

        assert_eq!(readback["type"], "iconSet");
        assert_eq!(readback["priority"], 4);
        assert_eq!(readback["iconSet"]["iconSet"], "3TrafficLights1");
        assert_eq!(
            readback["iconSet"]["cfvo"],
            serde_json::json!([
                { "type": "percent", "value": "0" },
                { "type": "percent", "value": "33" },
                { "type": "percent", "value": "67" },
            ])
        );
    }

    #[test]
    fn adds_data_bar_rule_xml() {
        let xml = r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData/></worksheet>"#;
        let options = XlsxConditionalFormatMutationOptions {
            sheet: None,
            range: Some("A1:A5"),
            rule: None,
            formula: None,
            rule_type: Some("data-bar"),
            operator: None,
            formula2: None,
            has_formula2: false,
            cfvo: vec!["min".to_string(), "max".to_string()],
            colors: vec!["638EC6".to_string()],
            icon_set: None,
            priority: Some(7),
            stop_if_true: false,
            has_stop_if_true: false,
            dxf_id: None,
            out: None,
            backup: None,
            dry_run: true,
            no_validate: true,
            in_place: false,
        };

        let mutation =
            add_conditional_format_xml(xml, "", &options).expect("add data bar rule XML");

        assert_eq!(mutation.rule.rule_type, "dataBar");
        assert_eq!(mutation.rule.priority, Some(7));
        assert_eq!(
            mutation
                .rule
                .data_bar
                .as_ref()
                .expect("data bar")
                .cfvo
                .len(),
            2
        );
        assert!(mutation.updated_xml.contains(
            r#"<cfRule priority="7" type="dataBar"><dataBar><cfvo type="min"/><cfvo type="max"/><color rgb="FF638EC6"/></dataBar></cfRule>"#
        ));
    }

    #[test]
    fn adds_icon_set_rule_xml() {
        let xml = r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData/></worksheet>"#;
        let options = XlsxConditionalFormatMutationOptions {
            sheet: None,
            range: Some("A1:A5"),
            rule: None,
            formula: None,
            rule_type: Some("icon-set"),
            operator: None,
            formula2: None,
            has_formula2: false,
            cfvo: vec![
                "percent:0".to_string(),
                "percent:33".to_string(),
                "percent:67".to_string(),
            ],
            colors: Vec::new(),
            icon_set: Some("3TrafficLights1"),
            priority: Some(8),
            stop_if_true: false,
            has_stop_if_true: false,
            dxf_id: None,
            out: None,
            backup: None,
            dry_run: true,
            no_validate: true,
            in_place: false,
        };

        let mutation =
            add_conditional_format_xml(xml, "", &options).expect("add icon set rule XML");

        assert_eq!(mutation.rule.rule_type, "iconSet");
        assert_eq!(mutation.rule.priority, Some(8));
        assert_eq!(
            mutation.rule.icon_set.as_ref().expect("icon set").icon_set,
            "3TrafficLights1"
        );
        assert!(mutation.updated_xml.contains(
            r#"<cfRule priority="8" type="iconSet"><iconSet iconSet="3TrafficLights1"><cfvo type="percent" val="0"/><cfvo type="percent" val="33"/><cfvo type="percent" val="67"/></iconSet></cfRule>"#
        ));
    }
}
