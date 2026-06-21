use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::{
    CliError, CliResult, WorkbookSheet, command_arg, copy_zip_with_part_override, decode_xml_text,
    local_name, normalize_xl_target, relationships, render_xml_attrs, replace_xml_span,
    resolve_sheet, selector_candidates, validate, validate_xlsx_mutation_output_flags,
    workbook_sheets, xlsx_ranges_set_temp_path, xml_attrs_map, xml_direct_child_ranges, xml_escape,
    xml_fragment_bounds, xml_general_ref, xml_open_tag_from_start, xml_tag_prefix, zip_text,
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
    priority: Option<i64>,
    formulas: Vec<String>,
    dxf_id: Option<i64>,
    stop_if_true: bool,
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
}

#[derive(Clone, Copy)]
struct SqrefCell {
    col: u32,
    row: u32,
    abs_col: bool,
    abs_row: bool,
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
    let rule_type = options.rule_type.unwrap_or("expression").trim();
    if !rule_type.is_empty() && rule_type != "expression" {
        return Err(CliError::invalid_args(
            "--type currently supports only expression",
        ));
    }
    if options.range.is_none_or(|value| value.trim().is_empty()) {
        return Err(CliError::invalid_args("--range is required"));
    }
    if options.formula.is_none_or(|value| value.trim().is_empty()) {
        return Err(CliError::invalid_args("--formula is required"));
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
    Value::Object(object)
}

fn add_conditional_format_xml(
    xml: &str,
    prefix: &str,
    options: &XlsxConditionalFormatMutationOptions<'_>,
) -> CliResult<ConditionalFormatMutation> {
    let norm_sqref = normalize_sqref(options.range.unwrap_or_default())?;
    let formula = options.formula.unwrap_or_default().trim();
    if formula.is_empty() {
        return Err(CliError::invalid_args("--formula is required"));
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
    let rule_xml = render_conditional_format_rule(
        prefix,
        formula,
        priority,
        options.has_stop_if_true.then_some(options.stop_if_true),
        options.dxf_id,
    );
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
            rule.priority == Some(priority)
                && rule.formulas.len() == 1
                && rule.formulas[0] == formula
        })
        .unwrap_or_else(|| ConditionalFormatRule {
            index: 0,
            block_index: 0,
            rule_index: 0,
            primary_selector: String::new(),
            selectors: Vec::new(),
            sqref: norm_sqref.clone(),
            rule_type: "expression".to_string(),
            priority: Some(priority),
            formulas: vec![formula.to_string()],
            dxf_id: options.dxf_id,
            stop_if_true: options.has_stop_if_true && options.stop_if_true,
        });
    Ok(ConditionalFormatMutation {
        updated_xml,
        sqref: norm_sqref.clone(),
        rule: added,
        cells_affected: sqref_cell_count(&norm_sqref),
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
        priority,
        formulas: Vec::new(),
        dxf_id,
        stop_if_true: attr_is_true(&attrs, "stopIfTrue"),
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
            Ok(Event::Text(e)) => {
                if stack.last().map(String::as_str) == Some("formula") {
                    if let Some(formula) = rule.formulas.last_mut() {
                        formula.push_str(&decode_xml_text(e.as_ref()));
                    }
                }
            }
            Ok(Event::CData(e)) => {
                if stack.last().map(String::as_str) == Some("formula") {
                    if let Some(formula) = rule.formulas.last_mut() {
                        formula.push_str(&decode_xml_text(e.as_ref()));
                    }
                }
            }
            Ok(Event::GeneralRef(e)) => {
                if stack.last().map(String::as_str) == Some("formula")
                    && let Some(formula) = rule.formulas.last_mut()
                {
                    formula.push_str(&xml_general_ref(e.as_ref()));
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

fn render_conditional_format_rule(
    prefix: &str,
    formula: &str,
    priority: i64,
    stop_if_true: Option<bool>,
    dxf_id: Option<i64>,
) -> String {
    let tag = element_name(prefix, "cfRule");
    let mut attrs = BTreeMap::new();
    attrs.insert("priority".to_string(), priority.to_string());
    attrs.insert("type".to_string(), "expression".to_string());
    if let Some(stop_if_true) = stop_if_true
        && stop_if_true
    {
        attrs.insert("stopIfTrue".to_string(), "1".to_string());
    }
    if let Some(dxf_id) = dxf_id {
        attrs.insert("dxfId".to_string(), dxf_id.to_string());
    }
    format!(
        "<{}{}><{}>{}</{}></{}>",
        tag,
        render_xml_attrs(&attrs),
        element_name(prefix, "formula"),
        xml_escape(formula),
        element_name(prefix, "formula"),
        tag
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

fn element_name(prefix: &str, local: &str) -> String {
    if prefix.is_empty() {
        local.to_string()
    } else {
        format!("{prefix}:{local}")
    }
}

fn conditional_format_sheet_selector(sheet: &WorkbookSheet) -> String {
    format!("sheetId:{}", sheet.sheet_id)
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
