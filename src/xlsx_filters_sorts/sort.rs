use super::*;

pub(super) struct SetSortXmlSpec<'a> {
    pub(super) ref_range: &'a str,
    pub(super) column: &'a str,
    pub(super) descending: bool,
    pub(super) expect_sort: Option<&'a str>,
    pub(super) expect_sort_present: bool,
}

pub(super) fn set_sort_in_xml(
    xml: &str,
    spec: SetSortXmlSpec<'_>,
) -> CliResult<(String, SortState)> {
    let sort_bounds = parse_range(spec.ref_range)
        .map_err(|err| CliError::invalid_args(format!("invalid --ref: {}", err.message)))?;
    let sort_ref = range_bounds_ref(sort_bounds);
    let condition_ref = sort_condition_ref(sort_bounds, spec.column)?;
    let root = xml_root_bounds(xml, "worksheet")?;
    let prefix = xml_tag_prefix(&root.tag_name);
    let existing_range = direct_child_range(xml, &root, "sortState")?;
    let existing_state = existing_range
        .as_ref()
        .map(|range| parse_sort_state_fragment(&xml[range.start..range.end]))
        .transpose()?;
    guard_expect_sort(
        existing_state.as_ref(),
        spec.expect_sort_present,
        spec.expect_sort,
    )?;

    let condition = render_sort_condition_fragment(&prefix, &condition_ref, spec.descending);
    let sort_state_fragment = if let Some(existing_range) = existing_range.as_ref() {
        let existing_fragment = &xml[existing_range.start..existing_range.end];
        let updated_ref = replace_element_ref_attr(existing_fragment, &sort_ref)?;
        let without_existing = remove_sort_condition_fragment(&updated_ref, &condition_ref)?;
        append_sort_condition_fragment(&without_existing, &condition)?
    } else {
        render_sort_state_fragment(&prefix, &sort_ref, &condition)
    };
    let sort_state = parse_sort_state_fragment(&sort_state_fragment)?;
    let updated_xml = if let Some(existing_range) = existing_range {
        replace_xml_span(
            xml,
            existing_range.start,
            existing_range.end,
            &sort_state_fragment,
        )
    } else {
        insert_ordered_child(xml, &root, "sortState", &sort_state_fragment)?
    };
    Ok((updated_xml, sort_state))
}

pub(super) fn clear_sort_in_xml(xml: &str) -> CliResult<String> {
    let root = xml_root_bounds(xml, "worksheet")?;
    let Some(sort_state_range) = direct_child_range(xml, &root, "sortState")? else {
        return Err(CliError::invalid_args("worksheet has no sortState"));
    };
    Ok(replace_xml_span(
        xml,
        sort_state_range.start,
        sort_state_range.end,
        "",
    ))
}

pub(super) fn sort_condition_ref(sort_bounds: RangeBounds, column: &str) -> CliResult<String> {
    let col_idx = parse_sort_column_index(column)?;
    let normalized = sort_bounds.normalized();
    if col_idx < normalized.min_col() || col_idx > normalized.max_col() {
        return Err(CliError::invalid_args(format!(
            "column {} is outside sort ref {}",
            column.to_ascii_uppercase(),
            range_bounds_ref(sort_bounds)
        )));
    }
    Ok(range_bounds_ref(RangeBounds {
        start_col: col_idx,
        start_row: normalized.min_row(),
        end_col: col_idx,
        end_row: normalized.max_row(),
    }))
}

pub(super) fn parse_sort_column_index(column: &str) -> CliResult<u32> {
    let letters = column.trim();
    if letters.is_empty() {
        return Err(CliError::invalid_args(
            "invalid --column: column letters cannot be empty",
        ));
    }
    let mut index = 0u32;
    for ch in letters.chars() {
        let upper = ch.to_ascii_uppercase();
        if !upper.is_ascii_uppercase() {
            return Err(CliError::invalid_args(format!(
                "invalid --column: invalid column letter {ch:?}"
            )));
        }
        index = index * 26 + (upper as u32 - 'A' as u32 + 1);
        if index > 16_384 {
            return Err(CliError::invalid_args(format!(
                "invalid --column: column {letters:?} out of XLSX bounds A-XFD"
            )));
        }
    }
    Ok(index)
}

pub(super) fn guard_expect_sort(
    state: Option<&SortState>,
    has_expect: bool,
    expect: Option<&str>,
) -> CliResult<()> {
    if !has_expect {
        return Ok(());
    }
    let want = parse_range(expect.unwrap_or_default())
        .map_err(|err| CliError::invalid_args(format!("invalid --expect-sort: {}", err.message)))
        .map(range_bounds_ref)?;
    let current = state
        .map(|state| state.ref_text.as_str())
        .unwrap_or_default();
    let got = if current.is_empty() {
        String::new()
    } else {
        parse_range(current)
            .map(range_bounds_ref)
            .unwrap_or_else(|_| current.to_string())
    };
    if got != want {
        return Err(CliError::invalid_args(format!(
            "sort ref mismatch: expected {want}, found {current:?}"
        )));
    }
    Ok(())
}

pub(super) fn render_sort_state_fragment(prefix: &str, sort_ref: &str, condition: &str) -> String {
    format!(
        "<{} ref=\"{}\">{}</{}>",
        element_name(prefix, "sortState"),
        xml_attr_escape(sort_ref),
        condition,
        element_name(prefix, "sortState")
    )
}

pub(super) fn render_sort_condition_fragment(
    prefix: &str,
    condition_ref: &str,
    descending: bool,
) -> String {
    let name = element_name(prefix, "sortCondition");
    if descending {
        format!(
            "<{name} descending=\"1\" ref=\"{}\"/>",
            xml_attr_escape(condition_ref)
        )
    } else {
        format!("<{name} ref=\"{}\"/>", xml_attr_escape(condition_ref))
    }
}

pub(super) fn remove_sort_condition_fragment(
    sort_state_fragment: &str,
    condition_ref: &str,
) -> CliResult<String> {
    let (open_end, _, close_start, self_closing) = xml_fragment_bounds(sort_state_fragment)?;
    if self_closing {
        return Ok(sort_state_fragment.to_string());
    }
    for child in xml_direct_child_ranges(sort_state_fragment, open_end + 1, close_start)? {
        if child.kind != "sortCondition" {
            continue;
        }
        let (_, attrs, _, _) = first_element(&sort_state_fragment[child.start..child.end])?;
        if attr_local(&attrs, "ref").as_deref() == Some(condition_ref) {
            return Ok(replace_xml_span(
                sort_state_fragment,
                child.start,
                child.end,
                "",
            ));
        }
    }
    Ok(sort_state_fragment.to_string())
}

pub(super) fn append_sort_condition_fragment(
    sort_state_fragment: &str,
    condition: &str,
) -> CliResult<String> {
    let (open_end, tag_name, close_start, self_closing) = xml_fragment_bounds(sort_state_fragment)?;
    if self_closing {
        let start_tag = xml_open_tag_from_start(&sort_state_fragment[..=open_end]);
        let mut updated = String::new();
        updated.push_str(&start_tag);
        updated.push_str(condition);
        updated.push_str(&format!("</{tag_name}>"));
        return Ok(updated);
    }
    Ok(replace_xml_span(
        sort_state_fragment,
        close_start,
        close_start,
        condition,
    ))
}
