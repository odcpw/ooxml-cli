use super::*;

pub(super) struct AddColumnFilterXmlSpec<'a> {
    pub(super) col_id: i64,
    pub(super) values: &'a [String],
    pub(super) custom_op: Option<&'a str>,
    pub(super) custom_val1: Option<&'a str>,
    pub(super) custom_val2: Option<&'a str>,
    pub(super) custom_present: bool,
    pub(super) expect_filter: Option<&'a str>,
    pub(super) expect_filter_present: bool,
}

pub(super) fn add_column_filter_in_xml(
    xml: &str,
    spec: AddColumnFilterXmlSpec<'_>,
) -> CliResult<(String, AutoFilterState)> {
    if spec.col_id < 0 {
        return Err(CliError::invalid_args("colId must be >= 0"));
    }
    let root = xml_root_bounds(xml, "worksheet")?;
    let prefix = xml_tag_prefix(&root.tag_name);
    let Some(auto_filter_range) = direct_child_range(xml, &root, "autoFilter")? else {
        return Err(CliError::invalid_args(
            "worksheet has no autoFilter; run set-autofilter first",
        ));
    };
    let auto_filter_fragment = &xml[auto_filter_range.start..auto_filter_range.end];
    let auto_filter = parse_auto_filter_fragment(auto_filter_fragment)?;
    let col_count = auto_filter_column_count(&auto_filter)?;
    if spec.col_id as u32 >= col_count {
        return Err(CliError::invalid_args(format!(
            "column ID exceeds range column count: colId {} not in 0-{}",
            spec.col_id,
            col_count.saturating_sub(1)
        )));
    }

    let existing_range = find_filter_column_range(auto_filter_fragment, spec.col_id)?;
    let existing_state = existing_range
        .as_ref()
        .map(|range| parse_filter_column_fragment(&auto_filter_fragment[range.start..range.end]))
        .transpose()?;
    guard_expect_filter(
        existing_state.as_ref(),
        spec.expect_filter_present,
        spec.expect_filter,
    )?;

    let new_column = render_filter_column_fragment(
        &prefix,
        spec.col_id,
        spec.values,
        spec.custom_op,
        spec.custom_val1,
        spec.custom_val2,
        spec.custom_present,
    )?;
    let base_fragment = if let Some(existing_range) = existing_range {
        replace_xml_span(
            auto_filter_fragment,
            existing_range.start,
            existing_range.end,
            "",
        )
    } else {
        auto_filter_fragment.to_string()
    };
    let updated_fragment = insert_filter_column_fragment(&base_fragment, &new_column, spec.col_id)?;
    let updated_state = parse_auto_filter_fragment(&updated_fragment)?;
    Ok((
        replace_xml_span(
            xml,
            auto_filter_range.start,
            auto_filter_range.end,
            &updated_fragment,
        ),
        updated_state,
    ))
}

pub(super) fn clear_column_filter_in_xml(
    xml: &str,
    col_id: i64,
) -> CliResult<(String, AutoFilterState)> {
    let root = xml_root_bounds(xml, "worksheet")?;
    let Some(auto_filter_range) = direct_child_range(xml, &root, "autoFilter")? else {
        return Err(CliError::invalid_args(
            "worksheet has no autoFilter; run set-autofilter first",
        ));
    };
    let auto_filter_fragment = &xml[auto_filter_range.start..auto_filter_range.end];
    let Some(existing_range) = find_filter_column_range(auto_filter_fragment, col_id)? else {
        return Err(CliError::invalid_args(format!(
            "column has no filter: colId {col_id}"
        )));
    };
    let updated_fragment = replace_xml_span(
        auto_filter_fragment,
        existing_range.start,
        existing_range.end,
        "",
    );
    let updated_state = parse_auto_filter_fragment(&updated_fragment)?;
    Ok((
        replace_xml_span(
            xml,
            auto_filter_range.start,
            auto_filter_range.end,
            &updated_fragment,
        ),
        updated_state,
    ))
}

pub(super) fn find_filter_column_range(
    auto_filter_fragment: &str,
    col_id: i64,
) -> CliResult<Option<crate::XmlNamedRange>> {
    let (open_end, _, close_start, self_closing) = xml_fragment_bounds(auto_filter_fragment)?;
    if self_closing {
        return Ok(None);
    }
    for child in xml_direct_child_ranges(auto_filter_fragment, open_end + 1, close_start)? {
        if child.kind != "filterColumn" {
            continue;
        }
        let (_, attrs, _, _) = first_element(&auto_filter_fragment[child.start..child.end])?;
        if attr_local(&attrs, "colId").and_then(|value| value.parse::<i64>().ok()) == Some(col_id) {
            return Ok(Some(child));
        }
    }
    Ok(None)
}

pub(super) fn insert_filter_column_fragment(
    auto_filter_fragment: &str,
    column_fragment: &str,
    col_id: i64,
) -> CliResult<String> {
    let (open_end, tag_name, close_start, self_closing) =
        xml_fragment_bounds(auto_filter_fragment)?;
    if self_closing {
        let start_tag = xml_open_tag_from_start(&auto_filter_fragment[..=open_end]);
        let mut updated = String::new();
        updated.push_str(&start_tag);
        updated.push_str(column_fragment);
        updated.push_str(&format!("</{tag_name}>"));
        return Ok(updated);
    }

    let insert_at = xml_direct_child_ranges(auto_filter_fragment, open_end + 1, close_start)?
        .into_iter()
        .find(|child| {
            if child.kind == "filterColumn" {
                let Ok((_, attrs, _, _)) =
                    first_element(&auto_filter_fragment[child.start..child.end])
                else {
                    return false;
                };
                return attr_local(&attrs, "colId")
                    .and_then(|value| value.parse::<i64>().ok())
                    .is_some_and(|existing_col_id| existing_col_id > col_id);
            }
            child.kind == "sortState"
        })
        .map(|child| child.start)
        .unwrap_or(close_start);
    Ok(replace_xml_span(
        auto_filter_fragment,
        insert_at,
        insert_at,
        column_fragment,
    ))
}

pub(super) fn render_filter_column_fragment(
    prefix: &str,
    col_id: i64,
    values: &[String],
    custom_op: Option<&str>,
    custom_val1: Option<&str>,
    custom_val2: Option<&str>,
    custom_present: bool,
) -> CliResult<String> {
    let mut out = format!(
        "<{} colId=\"{}\">",
        element_name(prefix, "filterColumn"),
        col_id
    );
    if !values.is_empty() {
        out.push_str(&format!("<{}>", element_name(prefix, "filters")));
        for value in values {
            out.push_str(&format!(
                "<{} val=\"{}\"/>",
                element_name(prefix, "filter"),
                xml_attr_escape(value)
            ));
        }
        out.push_str(&format!("</{}>", element_name(prefix, "filters")));
    }
    if custom_present {
        out.push_str(&render_custom_filters_fragment(
            prefix,
            custom_op.unwrap_or_default(),
            custom_val1.unwrap_or_default(),
            custom_val2.unwrap_or_default(),
        )?);
    }
    out.push_str(&format!("</{}>", element_name(prefix, "filterColumn")));
    Ok(out)
}

pub(super) fn render_custom_filters_fragment(
    prefix: &str,
    op: &str,
    val1: &str,
    val2: &str,
) -> CliResult<String> {
    let normalized = normalize_custom_operator(op)?;
    if val1.trim().is_empty() {
        return Err(CliError::invalid_args(
            "--custom-val1 is required for a custom filter",
        ));
    }
    let name = element_name(prefix, "customFilters");
    let mut out = if normalized == "between" {
        format!("<{name} and=\"1\">")
    } else {
        format!("<{name}>")
    };
    match normalized.as_str() {
        "between" => {
            if val2.trim().is_empty() {
                return Err(CliError::invalid_args(format!(
                    "--custom-val2 is required for {op}"
                )));
            }
            push_custom_filter_xml(prefix, &mut out, "greaterThanOrEqual", val1);
            push_custom_filter_xml(prefix, &mut out, "lessThanOrEqual", val2);
        }
        "notBetween" => {
            if val2.trim().is_empty() {
                return Err(CliError::invalid_args(format!(
                    "--custom-val2 is required for {op}"
                )));
            }
            push_custom_filter_xml(prefix, &mut out, "lessThan", val1);
            push_custom_filter_xml(prefix, &mut out, "greaterThan", val2);
        }
        operator => {
            if !val2.trim().is_empty() {
                return Err(CliError::invalid_args(
                    "--custom-val2 is only valid with the between or notBetween operator",
                ));
            }
            push_custom_filter_xml(prefix, &mut out, operator, val1);
        }
    }
    out.push_str(&format!("</{}>", element_name(prefix, "customFilters")));
    Ok(out)
}

pub(super) fn push_custom_filter_xml(prefix: &str, out: &mut String, operator: &str, val: &str) {
    out.push_str(&format!("<{}", element_name(prefix, "customFilter")));
    if operator != "equal" {
        out.push_str(&format!(" operator=\"{}\"", xml_attr_escape(operator)));
    }
    out.push_str(&format!(" val=\"{}\"/>", xml_attr_escape(val)));
}

pub(super) fn normalize_custom_operator(op: &str) -> CliResult<String> {
    let trimmed = op.trim();
    if trimmed.is_empty() {
        return Err(CliError::invalid_args("custom operator cannot be empty"));
    }
    let normalized = match trimmed.to_ascii_lowercase().as_str() {
        "eq" | "equals" | "==" | "=" => "equal",
        "ne" | "!=" | "<>" => "notEqual",
        "lt" | "<" | "less-than" => "lessThan",
        "le" | "lte" | "<=" | "less-than-or-equal" => "lessThanOrEqual",
        "gt" | ">" | "greater-than" => "greaterThan",
        "ge" | "gte" | ">=" | "greater-than-or-equal" => "greaterThanOrEqual",
        "between" => "between",
        "not-between" | "notbetween" => "notBetween",
        _ => match trimmed {
            "equal" | "notEqual" | "lessThan" | "lessThanOrEqual" | "greaterThan"
            | "greaterThanOrEqual" => trimmed,
            _ => {
                return Err(CliError::invalid_args(format!(
                    "invalid custom operator {op:?} (use one of equal,notEqual,lessThan,lessThanOrEqual,greaterThan,greaterThanOrEqual,between,notBetween)"
                )));
            }
        },
    };
    Ok(normalized.to_string())
}

pub(super) fn parse_filter_values(values: Option<&str>) -> Vec<String> {
    let Some(values) = values.filter(|value| !value.trim().is_empty()) else {
        return Vec::new();
    };
    let mut seen = BTreeMap::new();
    let mut deduped = Vec::new();
    for value in values
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if seen.insert(value.to_string(), true).is_none() {
            deduped.push(value.to_string());
        }
    }
    deduped
}

pub(super) fn guard_expect_filter(
    column: Option<&FilterColumnState>,
    has_expect: bool,
    expect: Option<&str>,
) -> CliResult<()> {
    if !has_expect {
        return Ok(());
    }
    let current = column
        .map(summarize_filter_column)
        .unwrap_or_else(|| "none".to_string());
    let want = expect.unwrap_or_default().trim();
    if current != want {
        return Err(CliError::invalid_args(format!(
            "filter mismatch: expected {want:?}, found {current:?}"
        )));
    }
    Ok(())
}

pub(super) fn summarize_filter_column(column: &FilterColumnState) -> String {
    if !column.values.is_empty() {
        return format!("values:{}", column.values.join(","));
    }
    if let Some(custom_filter) = column.custom_filter.as_ref() {
        let parts = custom_filter
            .criteria
            .iter()
            .map(|criterion| {
                let operator = if criterion.operator.is_empty() {
                    "equal"
                } else {
                    &criterion.operator
                };
                format!("{}={}", operator, criterion.val)
            })
            .collect::<Vec<_>>();
        return format!("custom:{}", parts.join(","));
    }
    "none".to_string()
}
