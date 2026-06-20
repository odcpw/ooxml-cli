use super::*;

#[derive(Clone)]
pub(super) struct FilterColumnState {
    pub(super) col_id: i64,
    pub(super) values: Vec<String>,
    pub(super) custom_filter: Option<CustomFilterState>,
}

#[derive(Clone)]
pub(super) struct CustomFilterState {
    pub(super) and: bool,
    pub(super) criteria: Vec<CustomFilterCriterionState>,
}

#[derive(Clone)]
pub(super) struct CustomFilterCriterionState {
    pub(super) operator: String,
    pub(super) val: String,
}

#[derive(Clone)]
pub(super) struct AutoFilterState {
    pub(super) ref_text: String,
    pub(super) columns: Vec<FilterColumnState>,
}

#[derive(Clone)]
pub(super) struct SortConditionState {
    pub(super) ref_text: String,
    pub(super) descending: bool,
}

#[derive(Clone)]
pub(super) struct SortState {
    pub(super) ref_text: String,
    pub(super) conditions: Vec<SortConditionState>,
}

pub(super) fn read_auto_filter_state(
    xml: &str,
    root_kind: &str,
) -> CliResult<Option<AutoFilterState>> {
    let root = xml_root_bounds(xml, root_kind)?;
    let Some(auto_filter) = direct_child_range(xml, &root, "autoFilter")? else {
        return Ok(None);
    };
    parse_auto_filter_fragment(&xml[auto_filter.start..auto_filter.end]).map(Some)
}

pub(super) fn read_sort_state(xml: &str, root_kind: &str) -> CliResult<Option<SortState>> {
    let root = xml_root_bounds(xml, root_kind)?;
    let Some(sort_state) = direct_child_range(xml, &root, "sortState")? else {
        return Ok(None);
    };
    parse_sort_state_fragment(&xml[sort_state.start..sort_state.end]).map(Some)
}

pub(super) fn parse_auto_filter_fragment(fragment: &str) -> CliResult<AutoFilterState> {
    let (_, attrs, _, _) = first_element(fragment)?;
    let (_, _, close_start, self_closing) = xml_fragment_bounds(fragment)?;
    let mut columns = Vec::new();
    if !self_closing {
        for child in
            xml_direct_child_ranges(fragment, fragment.find('>').unwrap_or(0) + 1, close_start)?
        {
            if child.kind == "filterColumn" {
                columns.push(parse_filter_column_fragment(
                    &fragment[child.start..child.end],
                )?);
            }
        }
    }
    columns.sort_by_key(|column| column.col_id);
    Ok(AutoFilterState {
        ref_text: attr_local(&attrs, "ref").unwrap_or_default(),
        columns,
    })
}

pub(super) fn parse_filter_column_fragment(fragment: &str) -> CliResult<FilterColumnState> {
    let (_, attrs, _, _) = first_element(fragment)?;
    let mut values = Vec::new();
    let mut custom_filter: Option<CustomFilterState> = None;
    let mut in_filters = false;
    let mut in_custom_filters = false;
    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(false);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => match local_name(e.name().as_ref()) {
                "filters" => in_filters = true,
                "customFilters" => {
                    in_custom_filters = true;
                    custom_filter = Some(CustomFilterState {
                        and: attr_local_start(&e, "and").as_deref() == Some("1"),
                        criteria: Vec::new(),
                    });
                }
                "filter" if in_filters => {
                    values.push(attr_local_start(&e, "val").unwrap_or_default());
                }
                "customFilter" if in_custom_filters => {
                    push_custom_filter_criterion(&mut custom_filter, &e);
                }
                _ => {}
            },
            Ok(Event::Empty(e)) => match local_name(e.name().as_ref()) {
                "filter" if in_filters => {
                    values.push(attr_local_start(&e, "val").unwrap_or_default());
                }
                "customFilters" => {
                    custom_filter = Some(CustomFilterState {
                        and: attr_local_start(&e, "and").as_deref() == Some("1"),
                        criteria: Vec::new(),
                    });
                }
                "customFilter" if in_custom_filters => {
                    push_custom_filter_criterion(&mut custom_filter, &e);
                }
                _ => {}
            },
            Ok(Event::End(e)) => match local_name(e.name().as_ref()) {
                "filters" => in_filters = false,
                "customFilters" => in_custom_filters = false,
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(FilterColumnState {
        col_id: attr_local(&attrs, "colId")
            .and_then(|value| value.parse::<i64>().ok())
            .unwrap_or(0),
        values,
        custom_filter,
    })
}

pub(super) fn push_custom_filter_criterion(
    custom_filter: &mut Option<CustomFilterState>,
    element: &BytesStart<'_>,
) {
    let criterion = CustomFilterCriterionState {
        operator: attr_local_start(element, "operator").unwrap_or_else(|| "equal".to_string()),
        val: attr_local_start(element, "val").unwrap_or_default(),
    };
    custom_filter
        .get_or_insert_with(|| CustomFilterState {
            and: false,
            criteria: Vec::new(),
        })
        .criteria
        .push(criterion);
}

pub(super) fn auto_filter_column_count(auto_filter: &AutoFilterState) -> CliResult<u32> {
    let bounds = parse_range(&auto_filter.ref_text).map_err(|err| {
        CliError::invalid_args(format!(
            "invalid autoFilter ref {:?}: {}",
            auto_filter.ref_text, err.message
        ))
    })?;
    Ok(bounds.normalized().col_count())
}

pub(super) fn parse_sort_state_fragment(fragment: &str) -> CliResult<SortState> {
    let (_, attrs, _, _) = first_element(fragment)?;
    let (_, _, close_start, self_closing) = xml_fragment_bounds(fragment)?;
    let mut conditions = Vec::new();
    if !self_closing {
        for child in
            xml_direct_child_ranges(fragment, fragment.find('>').unwrap_or(0) + 1, close_start)?
        {
            if child.kind == "sortCondition" {
                let (_, attrs, _, _) = first_element(&fragment[child.start..child.end])?;
                conditions.push(SortConditionState {
                    ref_text: attr_local(&attrs, "ref").unwrap_or_default(),
                    descending: attr_local(&attrs, "descending").as_deref() == Some("1"),
                });
            }
        }
    }
    Ok(SortState {
        ref_text: attr_local(&attrs, "ref").unwrap_or_default(),
        conditions,
    })
}

pub(super) fn auto_filter_json(state: &AutoFilterState) -> Value {
    let mut object = Map::new();
    object.insert("ref".to_string(), json!(state.ref_text));
    if !state.columns.is_empty() {
        object.insert(
            "columns".to_string(),
            Value::Array(state.columns.iter().map(filter_column_json).collect()),
        );
    }
    Value::Object(object)
}

pub(super) fn filter_column_json(column: &FilterColumnState) -> Value {
    let mut object = Map::new();
    object.insert("colId".to_string(), json!(column.col_id));
    if !column.values.is_empty() {
        object.insert("values".to_string(), json!(column.values));
    }
    if let Some(custom_filter) = column.custom_filter.as_ref() {
        object.insert(
            "customFilter".to_string(),
            json!({
                "and": custom_filter.and,
                "criteria": custom_filter.criteria.iter().map(|criterion| {
                    let mut item = Map::new();
                    if !criterion.operator.is_empty() {
                        item.insert("operator".to_string(), json!(criterion.operator));
                    }
                    item.insert("val".to_string(), json!(criterion.val));
                    Value::Object(item)
                }).collect::<Vec<_>>(),
            }),
        );
    }
    Value::Object(object)
}

pub(super) fn sort_state_json(state: &SortState) -> Value {
    let mut object = Map::new();
    object.insert("ref".to_string(), json!(state.ref_text));
    if !state.conditions.is_empty() {
        object.insert(
            "conditions".to_string(),
            Value::Array(
                state
                    .conditions
                    .iter()
                    .map(|condition| {
                        json!({
                            "ref": condition.ref_text,
                            "descending": condition.descending,
                        })
                    })
                    .collect(),
            ),
        );
    }
    Value::Object(object)
}
