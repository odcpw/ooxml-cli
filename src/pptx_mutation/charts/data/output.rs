use super::*;

pub(in crate::pptx_mutation::charts) fn unique_sorted_warnings(
    warnings: Vec<String>,
) -> Vec<String> {
    warnings
        .into_iter()
        .map(|warning| warning.trim().to_string())
        .filter(|warning| !warning.is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(in crate::pptx_mutation::charts) struct ChartCreateResultInput<'a> {
    pub(in crate::pptx_mutation::charts) file: &'a str,
    pub(in crate::pptx_mutation::charts) output_path: Option<&'a str>,
    pub(in crate::pptx_mutation::charts) dry_run: bool,
    pub(in crate::pptx_mutation::charts) slide: i64,
    pub(in crate::pptx_mutation::charts) create: &'a CreateSlideChartResult,
    pub(in crate::pptx_mutation::charts) source: &'a ChartCreateSource,
    pub(in crate::pptx_mutation::charts) geometry: &'a ChartGeometry,
    pub(in crate::pptx_mutation::charts) chart: Value,
}

pub(in crate::pptx_mutation::charts) fn chart_create_result_json(
    input: ChartCreateResultInput<'_>,
) -> Value {
    let ChartCreateResultInput {
        file,
        output_path,
        dry_run,
        slide,
        create,
        source,
        geometry,
        chart,
    } = input;
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    if !dry_run && let Some(output_path) = output_path {
        result.insert("output".to_string(), json!(output_path));
    }
    result.insert("dryRun".to_string(), json!(dry_run));
    result.insert("action".to_string(), json!("pptx.chart.create"));
    result.insert("slide".to_string(), json!(slide));
    result.insert("chartType".to_string(), json!(create.chart_type));
    if !create.title.is_empty() {
        result.insert("title".to_string(), json!(create.title));
    }
    result.insert("chartPartUri".to_string(), json!(create.chart_uri));
    result.insert(
        "chartRelationshipId".to_string(),
        json!(create.chart_relationship_id),
    );
    result.insert("shapeId".to_string(), json!(create.shape_id));
    result.insert("shapeName".to_string(), json!(create.shape_name));
    result.insert("seriesCount".to_string(), json!(create.series_count));
    result.insert("categories".to_string(), json!(create.categories));
    result.insert("x".to_string(), json!(geometry.x));
    result.insert("y".to_string(), json!(geometry.y));
    result.insert("cx".to_string(), json!(geometry.cx));
    result.insert("cy".to_string(), json!(geometry.cy));
    result.insert("sourceMode".to_string(), json!(source.mode));
    if source.mode == "external" {
        result.insert("sourceFile".to_string(), json!(source.source_file));
    }
    if !source.sheet.is_empty() {
        result.insert("sourceSheet".to_string(), json!(source.sheet));
    }
    if !source.range.is_empty() {
        result.insert("sourceRange".to_string(), json!(source.range));
    }
    if !create.embedded_workbook_part_uri.is_empty() {
        result.insert(
            "embeddedWorkbookPartUri".to_string(),
            json!(create.embedded_workbook_part_uri),
        );
    }
    result.insert("chart".to_string(), chart);
    let warnings = unique_sorted_warnings(create.warnings.clone());
    if !warnings.is_empty() {
        result.insert("warnings".to_string(), json!(warnings));
    }
    add_pptx_chart_create_commands(&mut result, output_path, dry_run, slide, &create.chart_uri);
    Value::Object(result)
}

pub(in crate::pptx_mutation::charts) struct ChartUpdateDataResultInput<'a> {
    pub(in crate::pptx_mutation::charts) file: &'a str,
    pub(in crate::pptx_mutation::charts) output_path: Option<&'a str>,
    pub(in crate::pptx_mutation::charts) dry_run: bool,
    pub(in crate::pptx_mutation::charts) slide: i64,
    pub(in crate::pptx_mutation::charts) series: i64,
    pub(in crate::pptx_mutation::charts) chart: Value,
    pub(in crate::pptx_mutation::charts) selected: &'a SelectedChart,
    pub(in crate::pptx_mutation::charts) updated_roles: Vec<UpdatedRoleResult>,
    pub(in crate::pptx_mutation::charts) embedded_updated: bool,
    pub(in crate::pptx_mutation::charts) current_values_hash: &'a str,
    pub(in crate::pptx_mutation::charts) expect_values_hash: &'a str,
    pub(in crate::pptx_mutation::charts) warnings: Vec<String>,
}

pub(in crate::pptx_mutation::charts) fn chart_update_data_result_json(
    input: ChartUpdateDataResultInput<'_>,
) -> Value {
    let ChartUpdateDataResultInput {
        file,
        output_path,
        dry_run,
        slide,
        series,
        chart,
        selected,
        updated_roles,
        embedded_updated,
        current_values_hash,
        expect_values_hash,
        warnings,
    } = input;
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    if !dry_run && let Some(output_path) = output_path {
        result.insert("output".to_string(), json!(output_path));
    }
    result.insert("dryRun".to_string(), json!(dry_run));
    result.insert("action".to_string(), json!("pptx.chart.update-data"));
    result.insert("chart".to_string(), chart);
    result.insert("series".to_string(), json!(series));
    result.insert(
        "updatedRoles".to_string(),
        Value::Array(
            updated_roles
                .iter()
                .map(updated_role_result_json)
                .collect::<Vec<_>>(),
        ),
    );
    if !selected.embedded_workbook_part_uri.is_empty() {
        result.insert(
            "embeddedWorkbookPartUri".to_string(),
            json!(selected.embedded_workbook_part_uri),
        );
    }
    result.insert(
        "embeddedWorkbookUpdated".to_string(),
        json!(embedded_updated),
    );
    result.insert("cacheVerified".to_string(), json!(false));
    if !warnings.is_empty() {
        result.insert("warnings".to_string(), json!(warnings));
    }
    result.insert(
        "storedCacheContract".to_string(),
        json!("stored chart cache values and embedded workbook cells are updated, but chart rendering is not recalculated by PowerPoint until validation/render/open"),
    );
    add_pptx_chart_update_commands(
        &mut result,
        output_path,
        dry_run,
        slide,
        &selected.part_selector(),
    );
    if !current_values_hash.is_empty() {
        result.insert("currentValuesHash".to_string(), json!(current_values_hash));
    }
    if !expect_values_hash.trim().is_empty() {
        result.insert(
            "expectedValuesHashAccepted".to_string(),
            json!(expect_values_hash.trim()),
        );
    }
    Value::Object(result)
}

fn updated_role_result_json(role: &UpdatedRoleResult) -> Value {
    let mut item = Map::new();
    item.insert("role".to_string(), json!(role.role));
    item.insert("formula".to_string(), json!(role.snapshot.formula));
    if !role.snapshot.sheet.is_empty() {
        item.insert("sheet".to_string(), json!(role.snapshot.sheet));
    }
    if !role.snapshot.range.is_empty() {
        item.insert("range".to_string(), json!(role.snapshot.range));
    }
    item.insert("refKind".to_string(), json!(role.snapshot.ref_kind));
    if !role.snapshot.cache_type.is_empty() {
        item.insert(
            "previousCacheType".to_string(),
            json!(role.snapshot.cache_type),
        );
    }
    item.insert(
        "previousCachePointCount".to_string(),
        json!(role.snapshot.point_count),
    );
    let previous_preview = preview_strings(&role.snapshot.values, 5);
    if !previous_preview.is_empty() {
        item.insert("previousCachePreview".to_string(), json!(previous_preview));
    }
    if !role.previous_values_hash.is_empty() {
        item.insert(
            "previousValuesHash".to_string(),
            json!(role.previous_values_hash),
        );
    }
    if !role.mutation.cache_type.is_empty() {
        item.insert("cacheType".to_string(), json!(role.mutation.cache_type));
    }
    item.insert(
        "cachePointCount".to_string(),
        json!(role.mutation.cache_point_count),
    );
    if !role.mutation.cache_preview.is_empty() {
        item.insert(
            "cachePreview".to_string(),
            json!(role.mutation.cache_preview),
        );
    }
    item.insert(
        "embeddedWorkbookRangeUpdated".to_string(),
        json!(role.embedded_workbook_range_updated),
    );
    Value::Object(item)
}

fn add_pptx_chart_create_commands(
    result: &mut Map<String, Value>,
    output_path: Option<&str>,
    dry_run: bool,
    slide: i64,
    chart_part_uri: &str,
) {
    let target = output_path.unwrap_or("<out.pptx>");
    let suffix = if dry_run { "Template" } else { "" };
    let selector = format!("part:{chart_part_uri}");
    result.insert(
        format!("chartShowCommand{suffix}"),
        json!(pptx_chart_show_command(target, slide, &selector)),
    );
    result.insert(
        format!("chartsListCommand{suffix}"),
        json!(pptx_charts_list_command(target, slide)),
    );
    result.insert(
        format!("validateCommand{suffix}"),
        json!(pptx_validate_command(target)),
    );
    result.insert(
        format!("renderCommand{suffix}"),
        json!(pptx_render_command(target)),
    );
}

fn add_pptx_chart_update_commands(
    result: &mut Map<String, Value>,
    output_path: Option<&str>,
    dry_run: bool,
    slide: i64,
    selector: &str,
) {
    let target = output_path.unwrap_or("<out.pptx>");
    let suffix = if dry_run { "Template" } else { "" };
    result.insert(
        format!("validateCommand{suffix}"),
        json!(pptx_validate_command(target)),
    );
    result.insert(
        format!("chartShowCommand{suffix}"),
        json!(pptx_chart_show_command(target, slide, selector)),
    );
    result.insert(
        format!("renderCommand{suffix}"),
        json!(pptx_render_command(target)),
    );
}

fn pptx_chart_show_command(file: &str, slide: i64, selector: &str) -> String {
    let mut command = format!("ooxml --json pptx charts show {}", command_arg(file));
    if slide > 0 {
        command.push_str(&format!(" --slide {slide}"));
    }
    if !selector.trim().is_empty() {
        command.push_str(&format!(" --chart {}", command_arg(selector)));
    }
    command
}

fn pptx_charts_list_command(file: &str, slide: i64) -> String {
    let mut command = format!("ooxml --json pptx charts list {}", command_arg(file));
    if slide > 0 {
        command.push_str(&format!(" --slide {slide}"));
    }
    command
}

fn pptx_validate_command(file: &str) -> String {
    format!("ooxml validate --strict {}", command_arg(file))
}

fn pptx_render_command(file: &str) -> String {
    format!("ooxml pptx render {} --out render-check", command_arg(file))
}
