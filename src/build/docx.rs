use serde::Serialize;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use super::{
    BuildCompileError, BuildCompiler, BuildFamily, BuildLength, BuildOperation, BuildSpec,
    CompiledBuildPlan, ImageRef, MarkdownConversion, MarkdownError, TableData, markdown_to_spec,
};

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompiledDocxBuild {
    pub plan: CompiledBuildPlan,
}

pub fn compile_docx_spec(spec: &BuildSpec) -> Result<CompiledDocxBuild, BuildCompileError> {
    if spec.family() != BuildFamily::Docx {
        return Err(error(
            "/family",
            "BUILD_SPEC_FAMILY_MISMATCH",
            "docx build requires a docx build spec",
        ));
    }
    let document = spec
        .document()
        .as_object()
        .expect("validated docx build spec root");
    let mut compiler = BuildCompiler::new(BuildFamily::Docx);
    compiler.push_operation(
        "/",
        None,
        "document",
        "docx scaffold",
        scaffold_args(document)?,
        "destination",
    )?;
    // The scaffold owns one empty seed paragraph. Keep it until all requested
    // body content exists (DOCX refuses deleting the last body block), then
    // remove it using the hash returned by the scaffold operation.
    let mut body_block_count = 1usize;
    if let Some(title) = document.get("title").and_then(Value::as_str) {
        compile_paragraph(
            ParagraphOperation {
                path: "/title",
                spec_id: None,
                op_id: "title",
                text: title,
                runs: None,
                style: "Title",
                list: None,
                level: None,
            },
            &mut compiler,
        )?;
        body_block_count += 1;
    }
    if let Some(subtitle) = document.get("subtitle").and_then(Value::as_str) {
        compile_paragraph(
            ParagraphOperation {
                path: "/subtitle",
                spec_id: None,
                op_id: "subtitle",
                text: subtitle,
                runs: None,
                style: "Subtitle",
                list: None,
                level: None,
            },
            &mut compiler,
        )?;
        body_block_count += 1;
    }

    let sections = document
        .get("sections")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let blocks = document["blocks"]
        .as_array()
        .expect("validated docx blocks array");
    for (block_index, block) in blocks.iter().enumerate() {
        for (section_index, section) in sections.iter().enumerate().skip(1) {
            let section = section.as_object().expect("validated DOCX section");
            if section.get("startBlock").and_then(Value::as_u64) == Some(block_index as u64) {
                compiler.push_internal_operation(
                    format!("section_break_{section_index}"),
                    "docx breaks insert",
                    Map::from_iter([("section".to_string(), json!(true))]),
                )?;
                body_block_count += 1;
            }
        }
        compile_block(
            block_index,
            block.as_object().expect("validated DOCX block"),
            body_block_count,
            &mut compiler,
        )?;
        body_block_count += produced_block_count(block);
    }
    compiler.push_internal_operation(
        "remove_seed",
        "docx blocks delete",
        Map::from_iter([
            ("block".to_string(), json!(1)),
            (
                "expectHash".to_string(),
                super::operation_reference("document", "readback.blockHashes.0.contentHash")
                    .map_err(|message| invalid("/", message))?,
            ),
        ]),
    )?;
    compile_section_setup(&sections, &mut compiler)?;
    compile_headers_footers(document, &mut compiler)?;

    Ok(CompiledDocxBuild {
        plan: compiler.finish()?,
    })
}

pub(crate) fn docx_build(args: &[String]) -> crate::CliResult<Value> {
    crate::reject_unknown_flags(
        args,
        &["--spec", "--from-markdown", "--emit-spec", "--out"],
        &["--check", "--dry-run", "--force"],
    )?;
    let spec_path = crate::parse_string_flag(args, "--spec")?;
    let markdown_path = crate::parse_string_flag(args, "--from-markdown")?;
    let emit_spec_path = crate::parse_string_flag(args, "--emit-spec")?;
    match (spec_path.as_deref(), markdown_path.as_deref()) {
        (Some(_), Some(_)) => {
            return Err(crate::CliError::invalid_args(
                "--spec and --from-markdown are mutually exclusive",
            ));
        }
        (None, None) => {
            return Err(crate::CliError::invalid_args(
                "exactly one of --spec or --from-markdown is required",
            ));
        }
        _ => {}
    }
    if emit_spec_path.is_some() && markdown_path.is_none() {
        return Err(crate::CliError::invalid_args(
            "--emit-spec requires --from-markdown",
        ));
    }
    let output = crate::parse_string_flag(args, "--out")?
        .ok_or_else(|| crate::CliError::invalid_args("--out is required"))?;
    let dry_run = crate::has_flag(args, "--dry-run");
    let run_check = crate::has_flag(args, "--check");
    let force = crate::has_flag(args, "--force");
    if run_check && dry_run {
        return Err(crate::CliError::invalid_args(
            "--check requires a published build; omit --dry-run",
        ));
    }
    validate_output_path(&output, force)?;
    if let Some(path) = emit_spec_path.as_deref() {
        validate_emitted_spec_path(path, &output, force)?;
    }

    let (spec, spec_base, warnings) = if let Some(path) = spec_path.as_deref() {
        let (spec, base) = load_docx_build_spec(path)?;
        (spec, base, Vec::new())
    } else {
        let path = markdown_path
            .as_deref()
            .expect("source selection validated above");
        let (spec, base, conversion) = load_docx_markdown(path)?;
        (spec, base, conversion.warnings)
    };

    let compiled = compile_docx_spec(&spec).map_err(build_compile_cli_error)?;
    if let Some(path) = emit_spec_path.as_deref() {
        write_emitted_spec(path, spec.document())?;
    }
    let temp = DocxBuildTemp::create()?;
    let operations = materialize_operations(&compiled.plan.operations, &temp.path, &spec_base)?;
    let ops_path = temp.path.join("operations.json");
    let mut ops_bytes = serde_json::to_vec_pretty(&operations).map_err(|cause| {
        crate::CliError::unexpected(format!("failed to encode build plan: {cause}"))
    })?;
    ops_bytes.push(b'\n');
    fs::write(&ops_path, ops_bytes).map_err(|cause| {
        crate::CliError::unexpected(format!("failed to write build plan: {cause}"))
    })?;

    let virtual_input = if dry_run {
        PathBuf::from(&output)
    } else {
        temp.path.join("new-document.docx")
    };
    let mut apply_args = vec!["--ops".to_string(), ops_path.to_string_lossy().into_owned()];
    if dry_run {
        apply_args.push("--dry-run".to_string());
    } else {
        apply_args.push("--out".to_string());
        apply_args.push(output.clone());
    }
    let mutation_envelope = crate::apply(&virtual_input.to_string_lossy(), &apply_args)
        .map_err(|error| super::compiler::execution_error_with_spec_path(&compiled.plan, error))?;
    let mutation_envelope = scrub_paths(mutation_envelope, &temp.path, &spec_base);
    let outline = if dry_run {
        Value::Null
    } else {
        crate::outline(
            &output,
            crate::OutlineOptions {
                depth: 3,
                text_preview: 240,
                slide: None,
                sheet: None,
                section: None,
            },
        )?
    };
    let check = if run_check {
        crate::check::inspect(&output, &json!({}))?
    } else {
        Value::Null
    };
    let node_map = resolved_node_map(&compiled.plan, &mutation_envelope);
    let mut result = json!({
        "schemaVersion": "ooxml-cli.docx-build.v1",
        "spec": spec_path,
        "output": if dry_run { Value::Null } else { json!(output) },
        "dryRun": dry_run,
        "validated": mutation_envelope["validated"],
        "mutationEnvelope": mutation_envelope,
        "compiledPlan": compiled.plan,
        "nodeMap": node_map,
        "outline": outline,
        "check": check,
    });
    let result_object = result.as_object_mut().expect("DOCX build result object");
    if let Some(path) = markdown_path {
        result_object.insert("markdown".to_string(), json!(path));
    }
    if let Some(path) = emit_spec_path {
        result_object.insert("emittedSpec".to_string(), json!(path));
    }
    if !warnings.is_empty() {
        result_object.insert(
            "warnings".to_string(),
            serde_json::to_value(warnings).expect("Markdown warnings serialize"),
        );
    }
    Ok(result)
}

fn scaffold_args(document: &Map<String, Value>) -> Result<Map<String, Value>, BuildCompileError> {
    let mut args = Map::new();
    for field in ["theme", "themeSeed", "template"] {
        copy_value(document, field, field, &mut args);
    }
    if let Some(brand) = document.get("brand") {
        let path = brand
            .as_str()
            .or_else(|| brand.get("path").and_then(Value::as_str));
        let Some(path) = path else {
            return Err(unsupported(
                "/brand/name",
                "named brand lookup is not available; provide brand.path",
            ));
        };
        args.insert("brand".to_string(), json!(path));
    }
    if let Some(metadata) = document.get("metadata").and_then(Value::as_object) {
        for field in ["title", "subject", "creator", "keywords", "description"] {
            copy_value(metadata, field, field, &mut args);
        }
    }
    Ok(args)
}

fn compile_block(
    block_index: usize,
    block: &Map<String, Value>,
    body_block_count: usize,
    compiler: &mut BuildCompiler,
) -> Result<(), BuildCompileError> {
    let path = format!("/blocks/{block_index}");
    let spec_id = block.get("id").and_then(Value::as_str);
    let kind = block["type"].as_str().expect("validated DOCX block type");
    match kind {
        "title" | "subtitle" | "heading" | "paragraph" | "bullet" | "numbered" => {
            let text = block
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let style = paragraph_style(block, kind, &path)?;
            let list = match kind {
                "bullet" => Some("bullet"),
                "numbered" => Some("number"),
                _ => None,
            };
            let op_id = format!("block_{:03}", block_index + 1);
            compile_paragraph(
                ParagraphOperation {
                    path: &path,
                    spec_id,
                    op_id: &op_id,
                    text,
                    runs: block.get("runs"),
                    style: &style,
                    list,
                    level: block.get("level"),
                },
                compiler,
            )?;
        }
        "table" => compile_table(block_index, block, &path, spec_id, compiler)?,
        "image" => compile_image(
            block_index,
            block,
            &path,
            spec_id,
            body_block_count,
            compiler,
        )?,
        "pageBreak" => compiler.push_operation(
            &path,
            spec_id,
            format!("block_{:03}", block_index + 1),
            "docx breaks insert",
            Map::from_iter([("page".to_string(), json!(true))]),
            "destination.primarySelector",
        )?,
        "toc" => compiler.push_operation(
            &path,
            spec_id,
            format!("block_{:03}", block_index + 1),
            "docx fields insert",
            Map::from_iter([
                ("toc".to_string(), json!(true)),
                ("levels".to_string(), json!("1-4")),
            ]),
            "destination.primarySelector",
        )?,
        _ => return Err(invalid(&format!("{path}/type"), "unknown DOCX block type")),
    }
    Ok(())
}

struct ParagraphOperation<'a> {
    path: &'a str,
    spec_id: Option<&'a str>,
    op_id: &'a str,
    text: &'a str,
    runs: Option<&'a Value>,
    style: &'a str,
    list: Option<&'a str>,
    level: Option<&'a Value>,
}

fn compile_paragraph(
    operation: ParagraphOperation<'_>,
    compiler: &mut BuildCompiler,
) -> Result<(), BuildCompileError> {
    let mut args = Map::new();
    if let Some(runs) = operation
        .runs
        .filter(|runs| !runs.as_array().is_none_or(Vec::is_empty))
    {
        args.insert("runs".to_string(), runs.clone());
    } else {
        args.insert("text".to_string(), json!(operation.text));
    }
    if !operation.style.is_empty() {
        args.insert("style".to_string(), json!(operation.style));
    }
    if let Some(list) = operation.list {
        args.insert("list".to_string(), json!(list));
        if let Some(level) = operation.level {
            args.insert("level".to_string(), level.clone());
        }
    }
    compiler.push_operation(
        operation.path,
        operation.spec_id,
        operation.op_id,
        "docx paragraphs append",
        args,
        "destination.primarySelector",
    )
}

fn paragraph_style(
    block: &Map<String, Value>,
    kind: &str,
    path: &str,
) -> Result<String, BuildCompileError> {
    if let Some(style) = block.get("style").and_then(Value::as_str) {
        return Ok(style.to_string());
    }
    Ok(match kind {
        "title" => "Title".to_string(),
        "subtitle" => "Subtitle".to_string(),
        "heading" => {
            let level = block.get("level").and_then(Value::as_u64).unwrap_or(1);
            if !(1..=4).contains(&level) {
                return Err(invalid(
                    &format!("{path}/level"),
                    "heading level must be from 1 through 4",
                ));
            }
            format!("Heading{level}")
        }
        "bullet" => "ListBullet".to_string(),
        "numbered" => "ListNumber".to_string(),
        _ => "Normal".to_string(),
    })
}

fn compile_table(
    block_index: usize,
    block: &Map<String, Value>,
    path: &str,
    spec_id: Option<&str>,
    compiler: &mut BuildCompiler,
) -> Result<(), BuildCompileError> {
    let table_value = block.get("table").cloned().unwrap_or_else(|| {
        let mut value = Map::new();
        for field in ["rows", "style", "header", "columnWidths"] {
            copy_value(block, field, field, &mut value);
        }
        Value::Object(value)
    });
    let table: TableData = serde_json::from_value(table_value)
        .map_err(|cause| invalid(path, format!("invalid table block: {cause}")))?;
    let mut args = Map::new();
    if !table.rows.is_empty() {
        args.insert(
            "values".to_string(),
            json!(serde_json::to_string(&table.rows).map_err(|cause| {
                invalid(path, format!("failed to encode table rows: {cause}"))
            })?),
        );
    } else if let Some(csv) = table.csv {
        args.insert("csvSource".to_string(), json!(csv));
    } else if let Some(json_path) = table.json {
        args.insert("valuesFile".to_string(), json!(json_path));
    } else if let Some(source) = table.xlsx {
        args.insert(
            "xlsxSource".to_string(),
            json!({"path": source.path, "sheet": source.sheet, "range": source.range}),
        );
    } else {
        return Err(invalid(
            path,
            "table requires rows, csv, json, or xlsx data",
        ));
    }
    args.insert(
        "style".to_string(),
        json!(table.style.as_deref().unwrap_or("TableGrid")),
    );
    if table.header.unwrap_or(true) {
        args.insert("headerRow".to_string(), json!(true));
    }
    if !table.column_widths.is_empty() {
        args.insert(
            "widths".to_string(),
            json!(
                table
                    .column_widths
                    .iter()
                    .map(BuildLength::cli_value)
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        );
    }
    if let Some(caption) = block.get("caption").and_then(Value::as_str) {
        args.insert("caption".to_string(), json!(caption));
    }
    compiler.push_operation(
        path,
        spec_id,
        format!("block_{:03}", block_index + 1),
        "docx tables create",
        args,
        "destination.primarySelector",
    )
}

fn compile_image(
    block_index: usize,
    block: &Map<String, Value>,
    path: &str,
    spec_id: Option<&str>,
    body_block_count: usize,
    compiler: &mut BuildCompiler,
) -> Result<(), BuildCompileError> {
    let image: ImageRef = serde_json::from_value(
        block
            .get("image")
            .cloned()
            .ok_or_else(|| invalid(path, "image block requires image"))?,
    )
    .map_err(|cause| invalid(path, format!("invalid image block: {cause}")))?;
    let width = image
        .width
        .as_ref()
        .map(length_emu)
        .transpose()?
        .unwrap_or(3_657_600);
    let height = image
        .height
        .as_ref()
        .map(length_emu)
        .transpose()?
        .unwrap_or(2_194_560);
    let mut args = Map::from_iter([
        ("after".to_string(), json!(body_block_count)),
        ("image".to_string(), json!(image.path)),
        ("width".to_string(), json!(width)),
        ("height".to_string(), json!(height)),
    ]);
    if let Some(value) = image.fit {
        args.insert("fit".to_string(), json!(value));
    }
    if let Some(value) = image.alt_text {
        args.insert("alt".to_string(), json!(value));
    }
    if let Some(value) = image.align {
        args.insert("align".to_string(), json!(value));
    }
    if let Some(value) = image.caption.or_else(|| {
        block
            .get("caption")
            .and_then(Value::as_str)
            .map(str::to_string)
    }) {
        args.insert("caption".to_string(), json!(value));
    }
    if let Some(value) = image.max_dpi {
        args.insert("maxDpi".to_string(), json!(value));
    }
    if image.keep_original == Some(true) {
        args.insert("keepOriginal".to_string(), json!(true));
    }
    compiler.push_operation(
        path,
        spec_id,
        format!("block_{:03}", block_index + 1),
        "docx images insert",
        args,
        "destination.primarySelector",
    )
}

fn produced_block_count(block: &Value) -> usize {
    if block.get("type").and_then(Value::as_str) == Some("image")
        && block
            .pointer("/image/caption")
            .or_else(|| block.get("caption"))
            .and_then(Value::as_str)
            .is_some_and(|value| !value.trim().is_empty())
    {
        2
    } else {
        1
    }
}

fn compile_section_setup(
    sections: &[Value],
    compiler: &mut BuildCompiler,
) -> Result<(), BuildCompileError> {
    for (index, section) in sections.iter().enumerate() {
        let section = section.as_object().expect("validated DOCX section");
        let mut args = Map::from_iter([
            ("section".to_string(), json!(index + 1)),
            (
                "orientation".to_string(),
                section
                    .get("orientation")
                    .cloned()
                    .unwrap_or_else(|| json!("portrait")),
            ),
            (
                "size".to_string(),
                section
                    .get("size")
                    .cloned()
                    .unwrap_or_else(|| json!("Letter")),
            ),
        ]);
        let margins = section.get("margins").and_then(Value::as_object);
        let margins = ["top", "right", "bottom", "left"]
            .iter()
            .map(|side| {
                margins
                    .and_then(|values| values.get(*side))
                    .map(length_value)
                    .unwrap_or_else(|| "1in".to_string())
            })
            .collect::<Vec<_>>()
            .join(",");
        args.insert("margins".to_string(), json!(margins));
        compiler.push_operation(
            format!("/sections/{index}"),
            section.get("id").and_then(Value::as_str),
            format!("section_setup_{:03}", index + 1),
            "docx sections set",
            args,
            "destination",
        )?;
    }
    Ok(())
}

fn compile_headers_footers(
    document: &Map<String, Value>,
    compiler: &mut BuildCompiler,
) -> Result<(), BuildCompileError> {
    if let Some(headers) = document.get("headers").and_then(Value::as_object) {
        for kind in ["default", "first", "even"] {
            if let Some(text) = headers.get(kind).and_then(Value::as_str) {
                compiler.push_internal_operation(
                    format!("header_{kind}"),
                    "docx headers set-text",
                    Map::from_iter([
                        ("section".to_string(), json!(1)),
                        ("type".to_string(), json!(kind)),
                        ("text".to_string(), json!(text)),
                    ]),
                )?;
            }
        }
    }
    if let Some(footers) = document.get("footers").and_then(Value::as_object) {
        for kind in ["default", "first", "even"] {
            if let Some(text) = footers.get(kind).and_then(Value::as_str) {
                compiler.push_internal_operation(
                    format!("footer_{kind}"),
                    "docx footers set-text",
                    Map::from_iter([
                        ("section".to_string(), json!(1)),
                        ("type".to_string(), json!(kind)),
                        ("text".to_string(), json!(text)),
                    ]),
                )?;
            }
        }
        if footers.get("pageNumbers").and_then(Value::as_bool) == Some(true) {
            compiler.push_internal_operation(
                "footer_page_numbers",
                "docx footers set-text",
                Map::from_iter([
                    ("section".to_string(), json!(1)),
                    ("type".to_string(), json!("default")),
                    ("pageNumbers".to_string(), json!(true)),
                ]),
            )?;
        }
    }
    Ok(())
}

fn materialize_operations(
    operations: &[BuildOperation],
    temp: &Path,
    spec_base: &Path,
) -> crate::CliResult<Vec<BuildOperation>> {
    operations
        .iter()
        .enumerate()
        .map(|(operation_index, operation)| {
            let mut operation = operation.clone();
            materialize_table_source(&mut operation, spec_base)?;
            for key in ["brand", "template", "image", "valuesFile"] {
                if let Some(Value::String(value)) = operation.args.get_mut(key) {
                    *value = stage_build_source(value, spec_base, temp, operation_index, key)?;
                }
            }
            Ok(operation)
        })
        .collect()
}

fn stage_build_source(
    value: &str,
    spec_base: &Path,
    temp: &Path,
    operation_index: usize,
    key: &str,
) -> crate::CliResult<String> {
    let source = resolve_source_path(value, spec_base);
    let extension = source
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| format!(".{value}"))
        .unwrap_or_default();
    let relative =
        PathBuf::from("external").join(format!("op-{}-{key}{extension}", operation_index + 1));
    let destination = temp.join(&relative);
    fs::create_dir_all(destination.parent().expect("staged source parent")).map_err(|cause| {
        crate::CliError::unexpected(format!("failed to create build source stage: {cause}"))
    })?;
    fs::copy(&source, &destination).map_err(|cause| {
        crate::CliError::invalid_args(format!(
            "failed to stage DOCX build source {}: {cause}",
            source.display()
        ))
    })?;
    Ok(relative.to_string_lossy().into_owned())
}

fn materialize_table_source(
    operation: &mut BuildOperation,
    spec_base: &Path,
) -> crate::CliResult<()> {
    if operation.command != "docx tables create" {
        return Ok(());
    }
    if let Some(Value::String(source)) = operation.args.remove("csvSource") {
        let source = resolve_source_path(&source, spec_base);
        let text = fs::read_to_string(&source).map_err(|cause| {
            crate::CliError::invalid_args(format!(
                "failed to read DOCX table CSV {}: {cause}",
                source.display()
            ))
        })?;
        let rows = parse_csv(&text)?;
        operation.args.insert(
            "values".to_string(),
            json!(serde_json::to_string(&rows).expect("CSV rows serialize")),
        );
    }
    if let Some(source) = operation.args.remove("xlsxSource") {
        let path = source["path"]
            .as_str()
            .ok_or_else(|| crate::CliError::invalid_args("xlsx table source requires path"))?;
        let path = resolve_source_path(path, spec_base);
        let sheet = source["sheet"]
            .as_str()
            .ok_or_else(|| crate::CliError::invalid_args("xlsx table source requires sheet"))?;
        let range = source["range"]
            .as_str()
            .ok_or_else(|| crate::CliError::invalid_args("xlsx table source requires range"))?;
        let exported = crate::xlsx_range_export_with_options(
            &path.to_string_lossy(),
            sheet,
            range,
            crate::XlsxRangeExportOptions {
                include_types: false,
                include_formulas: false,
                include_formats: false,
                data_out: None,
                max_cells: 1_000_000,
            },
        )?;
        operation.args.insert(
            "values".to_string(),
            json!(serde_json::to_string(&exported["values"]).expect("range values serialize")),
        );
    }
    Ok(())
}

fn parse_csv(source: &str) -> crate::CliResult<Vec<Vec<Value>>> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut chars = source.chars().peekable();
    let mut quoted = false;
    while let Some(ch) = chars.next() {
        match ch {
            '"' if quoted && chars.peek() == Some(&'"') => {
                chars.next();
                field.push('"');
            }
            '"' => quoted = !quoted,
            ',' if !quoted => row.push(json!(std::mem::take(&mut field))),
            '\n' if !quoted => {
                if field.ends_with('\r') {
                    field.pop();
                }
                row.push(json!(std::mem::take(&mut field)));
                rows.push(std::mem::take(&mut row));
            }
            _ => field.push(ch),
        }
    }
    if quoted {
        return Err(crate::CliError::invalid_args(
            "DOCX table CSV has an unterminated quoted field",
        ));
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(json!(field));
        rows.push(row);
    }
    if rows.is_empty() || rows.iter().any(Vec::is_empty) {
        return Err(crate::CliError::invalid_args(
            "DOCX table CSV must contain at least one row and column",
        ));
    }
    let width = rows[0].len();
    if rows.iter().any(|row| row.len() != width) {
        return Err(crate::CliError::invalid_args(
            "DOCX table CSV rows must have equal column counts",
        ));
    }
    Ok(rows)
}

fn validate_output_path(output: &str, force: bool) -> crate::CliResult<()> {
    let path = Path::new(output);
    if path.extension().and_then(|value| value.to_str()) != Some("docx") {
        return Err(crate::CliError::invalid_args(
            "--out must use the .docx extension",
        ));
    }
    if path.exists() && !force {
        return Err(crate::CliError::invalid_args(
            "output file already exists; pass --force to replace it",
        ));
    }
    Ok(())
}

fn validate_emitted_spec_path(path: &str, output: &str, force: bool) -> crate::CliResult<()> {
    if path == "-" {
        return Err(crate::CliError::invalid_args(
            "--emit-spec requires a file path because stdout is reserved for the build result",
        ));
    }
    if Path::new(path) == Path::new(output) {
        return Err(crate::CliError::invalid_args(
            "--emit-spec and --out must name different files",
        ));
    }
    if Path::new(path).exists() && !force {
        return Err(crate::CliError::invalid_args(
            "emitted spec already exists; pass --force to replace it",
        ));
    }
    Ok(())
}

fn write_emitted_spec(path: &str, document: &Value) -> crate::CliResult<()> {
    let mut encoded = serde_json::to_vec_pretty(document).map_err(|cause| {
        crate::CliError::unexpected(format!("failed to encode emitted DOCX build spec: {cause}"))
    })?;
    encoded.push(b'\n');
    fs::write(path, encoded).map_err(|cause| {
        crate::CliError::unexpected(format!(
            "failed to write emitted DOCX build spec {path}: {cause}"
        ))
    })
}

fn load_docx_build_spec(path: &str) -> crate::CliResult<(BuildSpec, PathBuf)> {
    if path == "-" {
        let mut source = Vec::new();
        std::io::stdin().read_to_end(&mut source).map_err(|cause| {
            crate::CliError::unexpected(format!("failed to read spec stdin: {cause}"))
        })?;
        let spec =
            super::load_spec_bytes(BuildFamily::Docx, &source).map_err(build_spec_cli_error)?;
        return Ok((
            spec,
            std::env::current_dir().map_err(|cause| {
                crate::CliError::unexpected(format!("failed to resolve current directory: {cause}"))
            })?,
        ));
    }
    let spec = super::load_spec_file(BuildFamily::Docx, path).map_err(build_spec_cli_error)?;
    let base = Path::new(path)
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .canonicalize()
        .map_err(|cause| {
            crate::CliError::unexpected(format!(
                "failed to resolve spec directory for {path}: {cause}"
            ))
        })?;
    Ok((spec, base))
}

fn load_docx_markdown(path: &str) -> crate::CliResult<(BuildSpec, PathBuf, MarkdownConversion)> {
    let (source, base, source_name) = if path == "-" {
        let mut source = String::new();
        std::io::stdin()
            .read_to_string(&mut source)
            .map_err(|cause| {
                crate::CliError::unexpected(format!("failed to read Markdown stdin: {cause}"))
            })?;
        let base = std::env::current_dir().map_err(|cause| {
            crate::CliError::unexpected(format!("failed to resolve current directory: {cause}"))
        })?;
        (source, base, "<stdin>".to_string())
    } else {
        let source = fs::read_to_string(path).map_err(|cause| {
            crate::CliError::file_not_found(format!("cannot read Markdown input {path}: {cause}"))
        })?;
        let base = Path::new(path)
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .canonicalize()
            .map_err(|cause| {
                crate::CliError::unexpected(format!(
                    "failed to resolve Markdown directory for {path}: {cause}"
                ))
            })?;
        (source, base, path.to_string())
    };
    let conversion =
        markdown_to_spec(BuildFamily::Docx, &source, &source_name).map_err(markdown_cli_error)?;
    let encoded = serde_json::to_vec(&conversion.spec).map_err(|cause| {
        crate::CliError::unexpected(format!(
            "failed to encode generated DOCX build spec: {cause}"
        ))
    })?;
    let spec = super::load_spec_bytes(BuildFamily::Docx, &encoded).map_err(build_spec_cli_error)?;
    Ok((spec, base, conversion))
}

struct DocxBuildTemp {
    path: PathBuf,
}

impl DocxBuildTemp {
    fn create() -> crate::CliResult<Self> {
        let path = std::env::temp_dir().join(format!("ooxml-docx-build-{}", std::process::id()));
        if path.exists() {
            fs::remove_dir_all(&path).map_err(|cause| {
                crate::CliError::unexpected(format!(
                    "failed to clear stale DOCX build staging directory: {cause}"
                ))
            })?;
        }
        fs::create_dir_all(&path).map_err(|cause| {
            crate::CliError::unexpected(format!(
                "failed to create DOCX build staging directory: {cause}"
            ))
        })?;
        Ok(Self { path })
    }
}

impl Drop for DocxBuildTemp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn resolved_node_map(plan: &CompiledBuildPlan, envelope: &Value) -> Value {
    let applied = envelope["applied"].as_array();
    let map = plan
        .node_map
        .iter()
        .map(|(path, node)| {
            let selector = applied
                .and_then(|items| {
                    items
                        .iter()
                        .find(|item| item["id"].as_str() == Some(node.op_id.as_str()))
                })
                .and_then(|item| item.pointer("/mutationEnvelope/destination/primarySelector"))
                .cloned()
                .unwrap_or(Value::Null);
            let selector = final_docx_selector(path, selector);
            (
                path.clone(),
                json!({"opId": node.op_id, "specId": node.spec_id, "selector": selector}),
            )
        })
        .collect::<BTreeMap<_, _>>();
    serde_json::to_value(map).expect("resolved DOCX build node map is serializable")
}

fn final_docx_selector(path: &str, selector: Value) -> Value {
    if !path.starts_with("/blocks/") {
        return selector;
    }
    let Some(index) = selector
        .as_str()
        .and_then(|value| value.strip_prefix("block:"))
        .and_then(|value| value.parse::<usize>().ok())
    else {
        return selector;
    };
    Value::String(format!("block:{}", index.saturating_sub(1)))
}

fn scrub_paths(value: Value, temp: &Path, spec_base: &Path) -> Value {
    let temp = temp.to_string_lossy();
    let spec_base = spec_base.to_string_lossy();
    scrub_path_prefixes(value, temp.as_ref(), spec_base.as_ref())
}

fn scrub_path_prefixes(value: Value, temp: &str, spec_base: &str) -> Value {
    match value {
        Value::String(text) => {
            let text = super::path_scrub::scrub_path_string(&text, temp, "<build-stage>");
            Value::String(super::path_scrub::scrub_path_string(
                &text,
                spec_base,
                "<spec-dir>",
            ))
        }
        Value::Array(values) => Value::Array(
            values
                .into_iter()
                .map(|value| scrub_path_prefixes(value, temp, spec_base))
                .collect(),
        ),
        Value::Object(values) => Value::Object(
            values
                .into_iter()
                .map(|(key, value)| (key, scrub_path_prefixes(value, temp, spec_base)))
                .collect(),
        ),
        scalar => scalar,
    }
}

fn resolve_source_path(path: &str, base: &Path) -> PathBuf {
    if Path::new(path).is_absolute() {
        PathBuf::from(path)
    } else {
        base.join(path)
    }
}

fn length_emu(length: &BuildLength) -> Result<i64, BuildCompileError> {
    match length {
        BuildLength::Emu(value) => Ok(*value),
        BuildLength::Human(value) => crate::cli_dispatch::units::parse_length(value, None)
            .map_err(|cause| invalid("/blocks/image", cause.message)),
    }
}

fn length_value(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_string)
        .or_else(|| value.as_i64().map(|value| value.to_string()))
        .unwrap_or_else(|| "1in".to_string())
}

fn copy_value(
    source: &Map<String, Value>,
    source_name: &str,
    target_name: &str,
    target: &mut Map<String, Value>,
) {
    if let Some(value) = source.get(source_name) {
        target.insert(target_name.to_string(), value.clone());
    }
}

fn build_spec_cli_error(error: super::BuildSpecError) -> crate::CliError {
    crate::CliError::invalid_args(
        serde_json::to_string(&error).unwrap_or_else(|_| error.to_string()),
    )
}

fn build_compile_cli_error(error: BuildCompileError) -> crate::CliError {
    crate::CliError::invalid_args(
        serde_json::to_string(&error).unwrap_or_else(|_| error.to_string()),
    )
}

fn markdown_cli_error(error: MarkdownError) -> crate::CliError {
    crate::CliError::invalid_args(
        serde_json::to_string(&error).unwrap_or_else(|_| error.to_string()),
    )
}

fn invalid(path: &str, message: impl Into<String>) -> BuildCompileError {
    error(path, "BUILD_SPEC_VALUE_INVALID", message)
}

fn unsupported(path: &str, message: impl Into<String>) -> BuildCompileError {
    error(path, "BUILD_SPEC_OPERATION_UNAVAILABLE", message)
}

fn error(path: &str, code: &str, message: impl Into<String>) -> BuildCompileError {
    BuildCompileError {
        code: code.to_string(),
        path: path.to_string(),
        op_id: None,
        message: message.into(),
    }
}
