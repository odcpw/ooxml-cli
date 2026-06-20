use super::*;

pub(super) fn normalize_filters_sorts_range(range: &str) -> CliResult<String> {
    let bounds = parse_range(range)
        .map_err(|err| CliError::invalid_args(format!("invalid range: {}", err.message)))?;
    Ok(range_bounds_ref(bounds.normalized()))
}

pub(super) fn guard_expect_range(
    state: Option<&AutoFilterState>,
    has_expect: bool,
    expect: Option<&str>,
) -> CliResult<()> {
    if !has_expect {
        return Ok(());
    }
    let want = parse_range(expect.unwrap_or_default())
        .map_err(|err| CliError::invalid_args(format!("invalid --expect-range: {}", err.message)))
        .map(|bounds| range_bounds_ref(bounds.normalized()))?;
    let current = state
        .map(|state| state.ref_text.as_str())
        .unwrap_or_default();
    let got = if current.is_empty() {
        String::new()
    } else {
        parse_range(current)
            .map(|bounds| range_bounds_ref(bounds.normalized()))
            .unwrap_or_else(|_| current.to_string())
    };
    if got != want {
        return Err(CliError::invalid_args(format!(
            "range mismatch: expected {want}, found {current:?}"
        )));
    }
    Ok(())
}

pub(super) fn set_autofilter_in_xml(
    xml: &str,
    root_kind: &str,
    range: &str,
) -> CliResult<(String, AutoFilterState)> {
    let root = xml_root_bounds(xml, root_kind)?;
    let prefix = xml_tag_prefix(&root.tag_name);
    if let Some(existing) = direct_child_range(xml, &root, "autoFilter")? {
        let fragment = &xml[existing.start..existing.end];
        let updated_fragment = replace_element_ref_attr(fragment, range)?;
        let state = parse_auto_filter_fragment(&updated_fragment)?;
        return Ok((
            replace_xml_span(xml, existing.start, existing.end, &updated_fragment),
            state,
        ));
    }

    let child_xml = format!(
        "<{} ref=\"{}\"/>",
        element_name(&prefix, "autoFilter"),
        xml_attr_escape(range)
    );
    let updated = if root_kind == "table" {
        insert_first_child(xml, &root, &child_xml)?
    } else {
        insert_ordered_child(xml, &root, "autoFilter", &child_xml)?
    };
    Ok((
        updated,
        AutoFilterState {
            ref_text: range.to_string(),
            columns: Vec::new(),
        },
    ))
}

pub(super) fn clear_autofilter_in_xml(xml: &str, root_kind: &str) -> CliResult<String> {
    let root = xml_root_bounds(xml, root_kind)?;
    let Some(existing) = direct_child_range(xml, &root, "autoFilter")? else {
        return Err(CliError::invalid_args(
            "worksheet has no autoFilter; run set-autofilter first",
        ));
    };
    Ok(replace_xml_span(xml, existing.start, existing.end, ""))
}
