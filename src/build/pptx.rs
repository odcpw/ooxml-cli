use serde::Serialize;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use super::{
    Bounds, BuildCompileError, BuildCompiler, BuildFamily, BuildLength, BuildSpec, ChartData,
    CompiledBuildPlan, ImageRef, MarkdownConversion, MarkdownError, TableData, markdown_to_spec,
};

const GENERATED_PREFIX: &str = "@generated/";

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PptxBuildAsset {
    pub path: String,
    pub contents: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompiledPptxBuild {
    pub plan: CompiledBuildPlan,
    #[serde(skip)]
    pub assets: Vec<PptxBuildAsset>,
}

pub fn compile_pptx_spec(spec: &BuildSpec) -> Result<CompiledPptxBuild, BuildCompileError> {
    if spec.family() != BuildFamily::Pptx {
        return Err(error(
            "/family",
            None,
            "BUILD_SPEC_FAMILY_MISMATCH",
            "pptx build requires a pptx build spec",
        ));
    }
    let document = spec
        .document()
        .as_object()
        .expect("validated pptx build spec root");
    reject_unimplemented_top_level(document)?;

    let mut compiler = BuildCompiler::new(BuildFamily::Pptx);
    let mut assets = Vec::new();
    compiler.push_operation(
        "/",
        None,
        "document",
        "pptx scaffold",
        scaffold_args(document)?,
        "destination",
    )?;

    let slides = document["slides"]
        .as_array()
        .expect("validated pptx slides array");
    for (slide_index, slide) in slides.iter().enumerate() {
        compile_slide_shell(
            slide_index,
            slide.as_object().expect("validated pptx slide"),
            &mut compiler,
            &mut assets,
        )?;
    }
    for (slide_index, slide) in slides.iter().enumerate() {
        compile_slide_content(
            slide_index,
            slide.as_object().expect("validated pptx slide"),
            &mut compiler,
            &mut assets,
        )?;
    }
    compile_fields(document, slides, &mut compiler)?;

    Ok(CompiledPptxBuild {
        plan: compiler.finish()?,
        assets,
    })
}

pub(crate) fn pptx_build(args: &[String]) -> crate::CliResult<Value> {
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
        let (spec, base) = load_pptx_build_spec(path)?;
        (spec, base, Vec::new())
    } else {
        let path = markdown_path
            .as_deref()
            .expect("source selection validated above");
        let (spec, base, conversion) = load_pptx_markdown(path)?;
        (spec, base, conversion.warnings)
    };

    let compiled = compile_pptx_spec(&spec).map_err(build_compile_cli_error)?;
    if let Some(path) = emit_spec_path.as_deref() {
        write_emitted_spec(path, spec.document())?;
    }
    let temp = PptxBuildTemp::create()?;
    materialize_assets(&temp.path, &compiled.assets)?;
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
        temp.path.join("new-presentation.pptx")
    };
    let mut apply_args = vec!["--ops".to_string(), ops_path.to_string_lossy().into_owned()];
    if dry_run {
        apply_args.push("--dry-run".to_string());
    } else {
        apply_args.push("--out".to_string());
        apply_args.push(output.clone());
    }
    // Build sources are resolved against the reviewed spec/Markdown directory
    // above, then passed through the same apply path guard as direct batches.
    apply_args.push("--allow-absolute-paths".to_string());
    let mutation_envelope = crate::apply(&virtual_input.to_string_lossy(), &apply_args)?;
    let mutation_envelope = scrub_generated_paths(mutation_envelope, &temp.path);

    let outline = if dry_run {
        Value::Null
    } else {
        crate::outline(
            &output,
            crate::OutlineOptions {
                depth: 2,
                text_preview: 240,
                slide: None,
                sheet: None,
                section: None,
            },
        )?
    };
    let layout_qa = if dry_run {
        Value::Null
    } else {
        crate::pptx_layout_qa::pptx_validate_layout(&output)?
    };
    let check = if run_check {
        crate::check::inspect(&output, &json!({}))?
    } else {
        Value::Null
    };
    let node_map = resolved_node_map(&compiled.plan, &mutation_envelope);
    let mut result = json!({
        "schemaVersion": "ooxml-cli.pptx-build.v1",
        "spec": spec_path,
        "output": if dry_run { Value::Null } else { json!(output) },
        "dryRun": dry_run,
        "validated": mutation_envelope["validated"],
        "mutationEnvelope": mutation_envelope,
        "compiledPlan": compiled.plan,
        "nodeMap": node_map,
        "outline": outline,
        "layoutQa": layout_qa,
        "check": check,
    });
    let result_object = result.as_object_mut().expect("PPTX build result object");
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

pub fn is_generated_asset_path(path: &str) -> bool {
    path.starts_with(GENERATED_PREFIX)
}

fn validate_output_path(output: &str, force: bool) -> crate::CliResult<()> {
    let path = Path::new(output);
    if path.extension().and_then(|value| value.to_str()) != Some("pptx") {
        return Err(crate::CliError::invalid_args(
            "--out must use the .pptx extension",
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
        crate::CliError::unexpected(format!("failed to encode emitted PPTX build spec: {cause}"))
    })?;
    encoded.push(b'\n');
    fs::write(path, encoded).map_err(|cause| {
        crate::CliError::unexpected(format!(
            "failed to write emitted PPTX build spec {path}: {cause}"
        ))
    })
}

fn load_pptx_build_spec(path: &str) -> crate::CliResult<(BuildSpec, PathBuf)> {
    if path == "-" {
        let mut source = Vec::new();
        std::io::stdin().read_to_end(&mut source).map_err(|cause| {
            crate::CliError::unexpected(format!("failed to read spec stdin: {cause}"))
        })?;
        let spec =
            super::load_spec_bytes(BuildFamily::Pptx, &source).map_err(build_spec_cli_error)?;
        return Ok((
            spec,
            std::env::current_dir().map_err(|cause| {
                crate::CliError::unexpected(format!("failed to resolve current directory: {cause}"))
            })?,
        ));
    }
    let spec = super::load_spec_file(BuildFamily::Pptx, path).map_err(build_spec_cli_error)?;
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

fn load_pptx_markdown(path: &str) -> crate::CliResult<(BuildSpec, PathBuf, MarkdownConversion)> {
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
        markdown_to_spec(BuildFamily::Pptx, &source, &source_name).map_err(markdown_cli_error)?;
    let encoded = serde_json::to_vec(&conversion.spec).map_err(|cause| {
        crate::CliError::unexpected(format!(
            "failed to encode generated PPTX build spec: {cause}"
        ))
    })?;
    let spec = super::load_spec_bytes(BuildFamily::Pptx, &encoded).map_err(build_spec_cli_error)?;
    Ok((spec, base, conversion))
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

struct PptxBuildTemp {
    path: PathBuf,
}

impl PptxBuildTemp {
    fn create() -> crate::CliResult<Self> {
        let path = std::env::temp_dir().join(format!("ooxml-pptx-build-{}", std::process::id()));
        if path.exists() {
            fs::remove_dir_all(&path).map_err(|cause| {
                crate::CliError::unexpected(format!(
                    "failed to clear stale PPTX build staging directory: {cause}"
                ))
            })?;
        }
        fs::create_dir_all(&path).map_err(|cause| {
            crate::CliError::unexpected(format!(
                "failed to create PPTX build staging directory: {cause}"
            ))
        })?;
        Ok(Self { path })
    }
}

impl Drop for PptxBuildTemp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn materialize_assets(root: &Path, assets: &[PptxBuildAsset]) -> crate::CliResult<()> {
    for asset in assets {
        let relative = asset
            .path
            .strip_prefix(GENERATED_PREFIX)
            .ok_or_else(|| crate::CliError::unexpected("invalid generated build asset path"))?;
        let destination = root.join("generated").join(relative);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|cause| {
                crate::CliError::unexpected(format!(
                    "failed to create build asset directory: {cause}"
                ))
            })?;
        }
        fs::write(&destination, &asset.contents).map_err(|cause| {
            crate::CliError::unexpected(format!("failed to write build asset: {cause}"))
        })?;
    }
    Ok(())
}

fn materialize_operations(
    operations: &[super::BuildOperation],
    temp: &Path,
    spec_base: &Path,
) -> crate::CliResult<Vec<super::BuildOperation>> {
    operations
        .iter()
        .map(|operation| {
            let mut operation = operation.clone();
            for (key, value) in &mut operation.args {
                if is_path_arg(&operation.command, key) {
                    *value = materialize_path_value(value, key, temp, spec_base)?;
                }
            }
            Ok(operation)
        })
        .collect()
}

fn is_path_arg(command: &str, key: &str) -> bool {
    matches!(
        key,
        "brand" | "template" | "image" | "data" | "workbook" | "sourceFile" | "paragraphsFile"
    ) && !(command == "pptx place image" && key == "data")
}

fn materialize_path_value(
    value: &Value,
    key: &str,
    temp: &Path,
    spec_base: &Path,
) -> crate::CliResult<Value> {
    match value {
        Value::String(path) => {
            materialize_path_string(path, key, temp, spec_base).map(Value::String)
        }
        Value::Array(values) => values
            .iter()
            .map(|value| materialize_path_value(value, key, temp, spec_base))
            .collect::<crate::CliResult<Vec<_>>>()
            .map(Value::Array),
        _ => Err(crate::CliError::invalid_args(format!(
            "build path field {key:?} must be a string or string array"
        ))),
    }
}

fn materialize_path_string(
    value: &str,
    key: &str,
    _temp: &Path,
    spec_base: &Path,
) -> crate::CliResult<String> {
    let (assignment, path) = if key == "paragraphsFile" {
        value
            .split_once('=')
            .map(|(target, path)| (Some(target), path))
            .unwrap_or((None, value))
    } else {
        (None, value)
    };
    let generated = path.strip_prefix(GENERATED_PREFIX);
    let resolved = if let Some(relative) = generated {
        PathBuf::from("generated").join(relative)
    } else if Path::new(path).is_absolute() {
        PathBuf::from(path)
    } else {
        spec_base.join(path)
    };
    let rendered = resolved.to_string_lossy();
    Ok(match assignment {
        Some(target) => format!("{target}={rendered}"),
        None => rendered.into_owned(),
    })
}

fn scrub_generated_paths(value: Value, temp: &Path) -> Value {
    let prefix = temp.to_string_lossy();
    match value {
        Value::String(text) => Value::String(scrub_build_stage_string(&text, prefix.as_ref())),
        Value::Array(values) => Value::Array(
            values
                .into_iter()
                .map(|value| scrub_generated_paths(value, temp))
                .collect(),
        ),
        Value::Object(values) => Value::Object(
            values
                .into_iter()
                .map(|(key, value)| (key, scrub_generated_paths(value, temp)))
                .collect(),
        ),
        scalar => scalar,
    }
}

fn scrub_build_stage_string(text: &str, prefix: &str) -> String {
    let native = prefix.replace('/', "\\");
    let slashed = prefix.replace('\\', "/");
    let mut path_variants = vec![prefix.to_string(), native.clone(), slashed.clone()];
    if native.as_bytes().get(1) == Some(&b':') {
        path_variants.push(format!(r"\\?\{native}"));
        path_variants.push(format!("//?/{slashed}"));
    }
    path_variants.sort();
    path_variants.dedup();

    let mut replacements = Vec::new();
    for variant in path_variants {
        let escaped = variant.replace('\\', r"\\");
        replacements.push(format!("'{escaped}'"));
        replacements.push(format!("\"{escaped}\""));
        replacements.push(escaped);
        replacements.push(crate::command_arg(&variant));
        replacements.push(format!("'{variant}'"));
        replacements.push(format!("\"{variant}\""));
        replacements.push(variant);
    }
    replacements.sort_by_key(|value| std::cmp::Reverse(value.len()));
    replacements.dedup();

    replacements
        .into_iter()
        .fold(text.to_string(), |text, from| {
            text.replace(&from, "<build-stage>")
        })
}

fn resolved_node_map(plan: &CompiledBuildPlan, envelope: &Value) -> Value {
    let applied = envelope["applied"].as_array();
    let map = plan
        .node_map
        .iter()
        .map(|(path, node)| {
            let operation = applied.and_then(|items| {
                items
                    .iter()
                    .find(|item| item["id"].as_str() == Some(node.op_id.as_str()))
            });
            let selector = operation
                .and_then(|item| item.pointer("/mutationEnvelope/destination/primarySelector"))
                .cloned()
                .unwrap_or(Value::Null);
            let slide = path
                .strip_prefix("/slides/")
                .and_then(|remainder| remainder.split('/').next())
                .and_then(|index| index.parse::<usize>().ok())
                .map(|index| index + 1);
            (
                path.clone(),
                json!({
                    "opId": node.op_id,
                    "specId": node.spec_id,
                    "selector": selector,
                    "slide": slide,
                }),
            )
        })
        .collect::<BTreeMap<_, _>>();
    serde_json::to_value(map).expect("resolved build node map is serializable")
}

fn reject_unimplemented_top_level(document: &Map<String, Value>) -> Result<(), BuildCompileError> {
    if document.contains_key("metadata") {
        return Err(unsupported(
            "/metadata",
            "PPTX core metadata has no mutation operation yet",
        ));
    }
    if document.contains_key("sections") {
        return Err(unsupported(
            "/sections",
            "native PowerPoint section grouping has no mutation operation yet",
        ));
    }
    Ok(())
}

fn scaffold_args(document: &Map<String, Value>) -> Result<Map<String, Value>, BuildCompileError> {
    let mut args = Map::new();
    copy_value(document, "theme", "theme", &mut args);
    copy_value(document, "themeSeed", "themeSeed", &mut args);
    copy_value(document, "template", "template", &mut args);
    copy_value(document, "size", "size", &mut args);
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
    Ok(args)
}

fn compile_slide_shell(
    slide_index: usize,
    slide: &Map<String, Value>,
    compiler: &mut BuildCompiler,
    assets: &mut Vec<PptxBuildAsset>,
) -> Result<(), BuildCompileError> {
    let slide_number = slide_index + 1;
    let slide_path = format!("/slides/{slide_index}");
    if slide.get("hidden").and_then(Value::as_bool) == Some(true) {
        return Err(unsupported(
            &format!("{slide_path}/hidden"),
            "hidden slides require a dedicated slide-properties mutation operation",
        ));
    }
    if slide.contains_key("section") {
        return Err(unsupported(
            &format!("{slide_path}/section"),
            "native PowerPoint section grouping has no mutation operation yet",
        ));
    }

    let slide_op = format!("slide_{}", slide_number);
    let mut args = Map::new();
    args.insert("layout".to_string(), slide["layout"].clone());
    let mut set_text = Vec::new();
    if let Some(title) = slide.get("title").and_then(Value::as_str) {
        set_text.push(json!(format!("title={title}")));
    }
    if let Some(subtitle) = slide.get("subtitle").and_then(Value::as_str) {
        let target = if slide.get("layout").and_then(Value::as_str) == Some("Section Header") {
            "body"
        } else {
            "subtitle"
        };
        set_text.push(json!(format!("{target}={subtitle}")));
    }
    if !set_text.is_empty() {
        args.insert("setText".to_string(), Value::Array(set_text));
    }
    if let Some(bullets) = slide.get("bullets").and_then(Value::as_array)
        && !bullets.is_empty()
    {
        let asset_path = generated_path(slide_number, "bullets", 0);
        let paragraphs = paragraph_values(bullets, true, &format!("{slide_path}/bullets"))?;
        push_json_asset(assets, &asset_path, &paragraphs)?;
        let body_target = match slide.get("layout").and_then(Value::as_str) {
            Some("Two Content" | "Comparison") => "body:1",
            _ => "body",
        };
        args.insert(
            "paragraphsFile".to_string(),
            json!(format!("{body_target}={asset_path}")),
        );
    }
    let spec_id = slide.get("id").and_then(Value::as_str);
    compiler.push_operation(
        &slide_path,
        spec_id,
        &slide_op,
        "pptx new-slide-from-layout",
        args,
        "destination",
    )?;
    for field in ["title", "subtitle", "bullets"] {
        if slide.contains_key(field) {
            compiler.map_node(
                format!("{slide_path}/{field}"),
                None,
                &slide_op,
                "destination.primarySelector",
            )?;
        }
    }
    if slide_index == 0 {
        compiler.push_internal_operation(
            "remove_seed_slide",
            "pptx slides delete",
            Map::from_iter([("slide-number".to_string(), json!(1))]),
        )?;
    }

    Ok(())
}

fn compile_slide_content(
    slide_index: usize,
    slide: &Map<String, Value>,
    compiler: &mut BuildCompiler,
    assets: &mut Vec<PptxBuildAsset>,
) -> Result<(), BuildCompileError> {
    let slide_number = slide_index + 1;
    let slide_path = format!("/slides/{slide_index}");
    compile_bullet_run_enrichments(slide_number, &slide_path, slide, compiler)?;
    compile_textboxes(slide_number, &slide_path, slide, compiler, assets)?;
    compile_images(slide_number, &slide_path, slide, compiler)?;
    compile_tables(slide_number, &slide_path, slide, compiler, assets)?;
    compile_charts(slide_number, &slide_path, slide, compiler)?;
    if let Some(notes) = slide.get("notes").and_then(Value::as_str) {
        compiler.push_operation(
            format!("{slide_path}/notes"),
            None,
            format!("slide_{}_notes", slide_number),
            "pptx notes set",
            Map::from_iter([
                ("slide".to_string(), json!(slide_number)),
                ("text".to_string(), json!(notes)),
            ]),
            "destination",
        )?;
    }
    Ok(())
}

fn compile_bullet_run_enrichments(
    slide_number: usize,
    slide_path: &str,
    slide: &Map<String, Value>,
    compiler: &mut BuildCompiler,
) -> Result<(), BuildCompileError> {
    let Some(paragraphs) = slide.get("bullets").and_then(Value::as_array) else {
        return Ok(());
    };
    let target = match slide.get("layout").and_then(Value::as_str) {
        Some("Two Content" | "Comparison") => "body:1",
        _ => "body",
    };
    for (paragraph_index, paragraph) in paragraphs.iter().enumerate() {
        let Some(runs) = paragraph.get("runs").and_then(Value::as_array) else {
            continue;
        };
        for (run_index, run) in runs.iter().enumerate() {
            let link = run.get("link").and_then(Value::as_str);
            let inline_code = run.get("inlineCode").and_then(Value::as_bool) == Some(true);
            if link.is_none() && !inline_code {
                continue;
            }
            let path = format!("{slide_path}/bullets/{paragraph_index}/runs/{run_index}");
            let mut args = Map::from_iter([
                ("slide".to_string(), json!(slide_number)),
                ("target".to_string(), json!(target)),
                ("paragraph".to_string(), json!(paragraph_index)),
                ("runIndex".to_string(), json!(run_index)),
            ]);
            if let Some(link) = link {
                args.insert("hyperlink".to_string(), json!(link));
            }
            if inline_code {
                args.insert("fontFamily".to_string(), json!("Aptos Mono"));
            }
            compiler.push_operation(
                &path,
                None,
                format!(
                    "slide_{}_paragraph_{}_run_{}_style",
                    slide_number,
                    paragraph_index + 1,
                    run_index + 1
                ),
                "pptx text set",
                args,
                "destination",
            )?;
        }
    }
    Ok(())
}

fn compile_textboxes(
    slide_number: usize,
    slide_path: &str,
    slide: &Map<String, Value>,
    compiler: &mut BuildCompiler,
    assets: &mut Vec<PptxBuildAsset>,
) -> Result<(), BuildCompileError> {
    let Some(textboxes) = slide.get("textBoxes").and_then(Value::as_array) else {
        return Ok(());
    };
    for (index, textbox) in textboxes.iter().enumerate() {
        let path = format!("{slide_path}/textBoxes/{index}");
        let textbox = textbox.as_object().expect("validated text box");
        let mut args = Map::from_iter([("slide".to_string(), json!(slide_number))]);
        geometry_args(textbox, &path, &mut args)?;
        if let Some(id) = textbox.get("id").and_then(Value::as_str) {
            args.insert("name".to_string(), json!(id));
        }
        match (
            textbox.get("text").and_then(Value::as_str),
            textbox.get("paragraphs").and_then(Value::as_array),
        ) {
            (Some(text), None) => {
                args.insert("text".to_string(), json!(text));
            }
            (None, Some(paragraphs)) if !paragraphs.is_empty() => {
                let asset_path = generated_path(slide_number, "textbox", index + 1);
                let paragraphs =
                    paragraph_values(paragraphs, false, &format!("{path}/paragraphs"))?;
                push_json_asset(assets, &asset_path, &paragraphs)?;
                args.insert("paragraphsFile".to_string(), json!(asset_path));
            }
            (Some(_), Some(_)) => {
                return Err(invalid(&path, "text and paragraphs are mutually exclusive"));
            }
            _ => return Err(invalid(&path, "a text box requires text or paragraphs")),
        }
        compiler.push_operation(
            &path,
            textbox.get("id").and_then(Value::as_str),
            format!("slide_{}_textbox_{}", slide_number, index + 1),
            "pptx add-textbox",
            args,
            "destination.primarySelector",
        )?;
    }
    Ok(())
}

fn compile_images(
    slide_number: usize,
    slide_path: &str,
    slide: &Map<String, Value>,
    compiler: &mut BuildCompiler,
) -> Result<(), BuildCompileError> {
    let Some(images) = slide.get("images").and_then(Value::as_array) else {
        return Ok(());
    };
    for (index, image) in images.iter().enumerate() {
        let path = format!("{slide_path}/images/{index}");
        let image: ImageRef = serde_json::from_value(image.clone())
            .map_err(|cause| invalid(&path, format!("invalid image: {cause}")))?;
        if image.width.is_some() || image.height.is_some() || image.align.is_some() {
            return Err(unsupported(
                &path,
                "PPTX images use slot or bounds; width, height, and align are document-flow fields",
            ));
        }
        let mut args = Map::from_iter([
            ("slide".to_string(), json!(slide_number)),
            ("image".to_string(), json!(image.path)),
        ]);
        geometry_typed_args(
            image.slot.as_deref(),
            image.bounds.as_ref(),
            &path,
            &mut args,
        )?;
        if let Some(id) = image.id.as_deref() {
            args.insert("name".to_string(), json!(id));
        }
        if let Some(fit) = image.fit.as_deref() {
            args.insert("fit".to_string(), json!(fit));
        }
        if let Some(alt) = image.alt_text.as_deref() {
            args.insert("alt".to_string(), json!(alt));
        }
        if let Some(max_dpi) = image.max_dpi {
            args.insert("maxDpi".to_string(), json!(max_dpi));
        }
        if image.keep_original == Some(true) {
            args.insert("keepOriginal".to_string(), json!(true));
        }
        let op_id = format!("slide_{}_image_{}", slide_number, index + 1);
        compiler.push_operation(
            &path,
            image.id.as_deref(),
            &op_id,
            "pptx place image",
            args,
            "destination.primarySelector",
        )?;
        if let Some(caption) = image.caption.as_deref() {
            let caption_path = format!("{path}/caption");
            compiler.push_operation(
                &caption_path,
                None,
                format!("{op_id}_caption"),
                "pptx add-textbox",
                Map::from_iter([
                    ("slide".to_string(), json!(slide_number)),
                    ("text".to_string(), json!(caption)),
                    ("slot".to_string(), json!("caption")),
                    ("fontSize".to_string(), json!(14)),
                    ("align".to_string(), json!("center")),
                ]),
                "destination.primarySelector",
            )?;
        }
    }
    Ok(())
}

fn compile_tables(
    slide_number: usize,
    slide_path: &str,
    slide: &Map<String, Value>,
    compiler: &mut BuildCompiler,
    assets: &mut Vec<PptxBuildAsset>,
) -> Result<(), BuildCompileError> {
    let Some(tables) = slide.get("tables").and_then(Value::as_array) else {
        return Ok(());
    };
    for (index, table) in tables.iter().enumerate() {
        let path = format!("{slide_path}/tables/{index}");
        let table: TableData = serde_json::from_value(table.clone())
            .map_err(|cause| invalid(&path, format!("invalid table: {cause}")))?;
        if table.total_row.is_some() || !table.column_widths.is_empty() {
            return Err(unsupported(
                &path,
                "total rows and per-column widths have no PPTX table placement mutation yet",
            ));
        }
        let mut args = Map::from_iter([("slide".to_string(), json!(slide_number))]);
        geometry_typed_args(
            table.slot.as_deref(),
            table.bounds.as_ref(),
            &path,
            &mut args,
        )?;
        if let Some(name) = table.name.as_deref().or(table.id.as_deref()) {
            args.insert("name".to_string(), json!(name));
        }
        if table.header == Some(true) {
            args.insert("header".to_string(), json!(true));
        }
        if table.banded_rows == Some(true) {
            args.insert("bandedRows".to_string(), json!(true));
        }
        apply_table_style(table.style.as_deref(), &path, &mut args)?;

        let command = if let Some(source) = table.xlsx.as_ref() {
            args.insert("workbook".to_string(), json!(source.path));
            args.insert("sheet".to_string(), json!(source.sheet));
            args.insert("range".to_string(), json!(source.range));
            "pptx place table-from-xlsx"
        } else {
            let sources = usize::from(!table.rows.is_empty())
                + usize::from(table.csv.is_some())
                + usize::from(table.json.is_some());
            if sources != 1 {
                return Err(invalid(
                    &path,
                    "a table requires exactly one of rows, csv, json, or xlsx",
                ));
            }
            if !table.rows.is_empty() {
                let asset_path = generated_path(slide_number, "table", index + 1);
                push_json_asset(assets, &asset_path, &table.rows)?;
                args.insert("data".to_string(), json!(asset_path));
                args.insert("format".to_string(), json!("json"));
            } else if let Some(csv) = table.csv.as_deref() {
                args.insert("data".to_string(), json!(csv));
                args.insert("format".to_string(), json!("csv"));
            } else if let Some(json_path) = table.json.as_deref() {
                args.insert("data".to_string(), json!(json_path));
                args.insert("format".to_string(), json!("json"));
            }
            "pptx place table"
        };
        compiler.push_operation(
            &path,
            table.id.as_deref(),
            format!("slide_{}_table_{}", slide_number, index + 1),
            command,
            args,
            "destination.primarySelector",
        )?;
    }
    Ok(())
}

fn compile_charts(
    slide_number: usize,
    slide_path: &str,
    slide: &Map<String, Value>,
    compiler: &mut BuildCompiler,
) -> Result<(), BuildCompileError> {
    let Some(charts) = slide.get("charts").and_then(Value::as_array) else {
        return Ok(());
    };
    for (index, chart) in charts.iter().enumerate() {
        let path = format!("{slide_path}/charts/{index}");
        let chart: ChartData = serde_json::from_value(chart.clone())
            .map_err(|cause| invalid(&path, format!("invalid chart: {cause}")))?;
        if chart.chart_type == "doughnut" {
            return Err(unsupported(
                &format!("{path}/type"),
                "doughnut chart creation is not implemented by pptx charts create",
            ));
        }
        if chart.series.iter().any(|series| series.color.is_some()) {
            return Err(unsupported(
                &format!("{path}/series"),
                "per-series colors require a follow-up chart styling operation",
            ));
        }
        let mutation_chart_type = if chart.chart_type == "column" {
            "bar"
        } else {
            chart.chart_type.as_str()
        };
        let mut args = Map::from_iter([
            ("slide".to_string(), json!(slide_number)),
            ("type".to_string(), json!(mutation_chart_type)),
        ]);
        geometry_typed_args(
            chart.slot.as_deref(),
            chart.bounds.as_ref(),
            &path,
            &mut args,
        )?;
        if let Some(title) = chart.title.as_deref() {
            args.insert("title".to_string(), json!(title));
        }
        if let Some(style) = chart.style.as_ref() {
            let Some(style) = style.as_str() else {
                return Err(unsupported(
                    &format!("{path}/style"),
                    "numeric Office chart style ids are not accepted by the house-style mutation",
                ));
            };
            args.insert("style".to_string(), json!(style));
        }
        chart_options(&chart.options, &path, &mut args)?;
        match (chart.source.as_ref(), chart.series.is_empty()) {
            (Some(source), true) => {
                args.insert("sourceFile".to_string(), json!(source.path));
                args.insert("sourceSheet".to_string(), json!(source.sheet));
                args.insert("sourceRange".to_string(), json!(source.range));
            }
            (None, false) => {
                args.insert(
                    "valuesJson".to_string(),
                    json!(inline_chart_matrix(&chart, &path)?),
                );
            }
            _ => {
                return Err(invalid(
                    &path,
                    "a chart requires exactly one of inline series or source",
                ));
            }
        }
        compiler.push_operation(
            &path,
            chart.id.as_deref(),
            format!("slide_{}_chart_{}", slide_number, index + 1),
            "pptx charts create",
            args,
            "destination.primarySelector",
        )?;
    }
    Ok(())
}

fn compile_fields(
    document: &Map<String, Value>,
    slides: &[Value],
    compiler: &mut BuildCompiler,
) -> Result<(), BuildCompileError> {
    let top_footer = document.get("footer").and_then(Value::as_str);
    let top_numbers = document.get("slideNumbers").and_then(Value::as_bool);
    for (index, slide) in slides.iter().enumerate() {
        let slide = slide.as_object().expect("validated slide");
        if let Some(footer) = slide.get("footer").and_then(Value::as_str)
            && Some(footer) != top_footer
        {
            return Err(unsupported(
                &format!("/slides/{index}/footer"),
                "per-slide footer overrides are not supported by pptx fields set",
            ));
        }
        if let Some(numbers) = slide.get("slideNumber").and_then(Value::as_bool)
            && Some(numbers) != top_numbers
        {
            return Err(unsupported(
                &format!("/slides/{index}/slideNumber"),
                "per-slide number overrides are not supported by pptx fields set",
            ));
        }
    }
    if top_footer.is_none() && top_numbers.is_none() {
        return Ok(());
    }
    let mut args = Map::new();
    if let Some(footer) = top_footer {
        args.insert("footer".to_string(), json!(footer));
        args.insert("showFooter".to_string(), json!(true));
    }
    if let Some(show) = top_numbers {
        args.insert("showSlideNumber".to_string(), json!(show));
    }
    compiler.push_operation(
        "/fields",
        None,
        "presentation_fields",
        "pptx fields set",
        args,
        "destination",
    )
}

fn geometry_args(
    object: &Map<String, Value>,
    path: &str,
    args: &mut Map<String, Value>,
) -> Result<(), BuildCompileError> {
    let slot = object.get("slot").and_then(Value::as_str);
    let bounds = object.get("bounds");
    match (slot, bounds) {
        (Some(slot), None) => {
            args.insert("slot".to_string(), json!(normalize_slot(slot)));
            Ok(())
        }
        (None, Some(bounds)) => {
            let bounds: Bounds = serde_json::from_value(bounds.clone())
                .map_err(|cause| invalid(path, format!("invalid bounds: {cause}")))?;
            insert_bounds(&bounds, args);
            Ok(())
        }
        (Some(_), Some(_)) => Err(invalid(path, "slot and bounds are mutually exclusive")),
        (None, None) => Err(invalid(path, "one of slot or bounds is required")),
    }
}

fn geometry_typed_args(
    slot: Option<&str>,
    bounds: Option<&Bounds>,
    path: &str,
    args: &mut Map<String, Value>,
) -> Result<(), BuildCompileError> {
    match (slot, bounds) {
        (Some(slot), None) => {
            args.insert("slot".to_string(), json!(normalize_slot(slot)));
            Ok(())
        }
        (None, Some(bounds)) => {
            insert_bounds(bounds, args);
            Ok(())
        }
        (Some(_), Some(_)) => Err(invalid(path, "slot and bounds are mutually exclusive")),
        (None, None) => Err(invalid(path, "one of slot or bounds is required")),
    }
}

fn normalize_slot(slot: &str) -> &str {
    match slot {
        "left" => "left-half",
        "right" => "right-half",
        "top" => "top-half",
        "bottom" => "bottom-half",
        other => other,
    }
}

fn insert_bounds(bounds: &Bounds, args: &mut Map<String, Value>) {
    for (name, value) in [
        ("x", &bounds.x),
        ("y", &bounds.y),
        ("cx", &bounds.cx),
        ("cy", &bounds.cy),
    ] {
        args.insert(name.to_string(), json!(value.cli_value()));
    }
}

fn paragraph_values(
    paragraphs: &[Value],
    default_bullet: bool,
    path: &str,
) -> Result<Vec<Value>, BuildCompileError> {
    paragraphs
        .iter()
        .enumerate()
        .map(|(index, paragraph)| {
            let paragraph_path = format!("{path}/{index}");
            let source = paragraph.as_object().expect("validated paragraph");
            for unsupported_field in ["numbered", "style"] {
                if source.get(unsupported_field).is_some_and(|value| value != &Value::Null) {
                    return Err(unsupported(
                        &format!("{paragraph_path}/{unsupported_field}"),
                        format!("paragraph {unsupported_field} is not implemented by the PPTX paragraph writer"),
                    ));
                }
            }
            let mut out = Map::new();
            for field in ["text", "level", "bullet", "bold", "italic", "color", "align"] {
                copy_value(source, field, field, &mut out);
            }
            if default_bullet && !source.contains_key("bullet") {
                out.insert("bullet".to_string(), json!(true));
            }
            if let Some(size) = source.get("size") {
                out.insert("size".to_string(), json!(font_size_points(size, &format!("{paragraph_path}/size"))?));
            }
            if let Some(runs) = source.get("runs").and_then(Value::as_array) {
                let mut out_runs = Vec::new();
                for (run_index, run) in runs.iter().enumerate() {
                    let run_path = format!("{paragraph_path}/runs/{run_index}");
                    let source = run.as_object().expect("validated paragraph run");
                    for unsupported_field in ["underline"] {
                        if source.get(unsupported_field).is_some_and(|value| value != &Value::Null) {
                            return Err(unsupported(
                                &format!("{run_path}/{unsupported_field}"),
                                format!("run {unsupported_field} is not implemented by the PPTX paragraph writer"),
                            ));
                        }
                    }
                    let mut out_run = Map::new();
                    for field in ["text", "bold", "italic", "color"] {
                        copy_value(source, field, field, &mut out_run);
                    }
                    if let Some(size) = source.get("size") {
                        out_run.insert("size".to_string(), json!(font_size_points(size, &format!("{run_path}/size"))?));
                    }
                    out_runs.push(Value::Object(out_run));
                }
                out.insert("runs".to_string(), Value::Array(out_runs));
            }
            Ok(Value::Object(out))
        })
        .collect()
}

fn font_size_points(value: &Value, path: &str) -> Result<f64, BuildCompileError> {
    let length: BuildLength = serde_json::from_value(value.clone())
        .map_err(|cause| invalid(path, format!("invalid font size: {cause}")))?;
    match length {
        BuildLength::Emu(emu) if emu > 0 => Ok(emu as f64 / 12_700.0),
        BuildLength::Human(text) if text.ends_with('%') => {
            Err(invalid(path, "percentage font sizes are not supported"))
        }
        BuildLength::Human(text) => crate::cli_dispatch::units::parse_length(&text, None)
            .map(|emu| emu as f64 / 12_700.0)
            .map_err(|cause| invalid(path, cause.message)),
        BuildLength::Emu(_) => Err(invalid(path, "font size must be positive")),
    }
}

fn apply_table_style(
    style: Option<&str>,
    path: &str,
    args: &mut Map<String, Value>,
) -> Result<(), BuildCompileError> {
    let Some(style) = style else {
        return Ok(());
    };
    match style.trim().to_ascii_lowercase().as_str() {
        "medium2" | "medium-2" => {
            args.entry("header".to_string()).or_insert(json!(true));
            args.entry("bandedRows".to_string()).or_insert(json!(true));
            args.insert("headerColor".to_string(), json!("4472C4"));
            args.insert("band1Color".to_string(), json!("D9E1F2"));
            Ok(())
        }
        "light1" | "light-1" => {
            args.entry("header".to_string()).or_insert(json!(true));
            args.insert("headerColor".to_string(), json!("D9E1F2"));
            Ok(())
        }
        _ => Err(invalid(
            &format!("{path}/style"),
            "unsupported PPTX table style; use Light1 or Medium2",
        )),
    }
}

fn chart_options(
    options: &Map<String, Value>,
    path: &str,
    args: &mut Map<String, Value>,
) -> Result<(), BuildCompileError> {
    for (key, value) in options {
        match key.as_str() {
            "dataLabels" if value.is_boolean() => {
                args.insert("dataLabels".to_string(), value.clone());
            }
            "numberFormat" if value.is_string() => {
                args.insert("numberFormat".to_string(), value.clone());
            }
            _ => {
                return Err(invalid(
                    &format!("{path}/options/{key}"),
                    "unsupported chart option (accepted: dataLabels, numberFormat)",
                ));
            }
        }
    }
    Ok(())
}

fn inline_chart_matrix(chart: &ChartData, path: &str) -> Result<String, BuildCompileError> {
    let value_count = chart
        .series
        .first()
        .map(|series| series.values.len())
        .unwrap_or(0);
    if value_count == 0
        || chart
            .series
            .iter()
            .any(|series| series.values.len() != value_count)
    {
        return Err(invalid(
            &format!("{path}/series"),
            "chart series must be non-empty and have equal value counts",
        ));
    }
    if !chart.categories.is_empty() && chart.categories.len() != value_count {
        return Err(invalid(
            &format!("{path}/categories"),
            "chart categories must match the series value count",
        ));
    }
    let mut rows = Vec::with_capacity(value_count + 1);
    let mut header = vec![json!("")];
    header.extend(chart.series.iter().map(|series| json!(series.name)));
    rows.push(Value::Array(header));
    for row_index in 0..value_count {
        let category = chart
            .categories
            .get(row_index)
            .cloned()
            .unwrap_or_else(|| json!(row_index + 1));
        let mut row = vec![category];
        row.extend(
            chart
                .series
                .iter()
                .map(|series| json!(series.values[row_index])),
        );
        rows.push(Value::Array(row));
    }
    serde_json::to_string(&rows)
        .map_err(|cause| invalid(path, format!("failed to encode chart values: {cause}")))
}

fn generated_path(slide_number: usize, kind: &str, index: usize) -> String {
    format!("{GENERATED_PREFIX}slide-{slide_number:03}-{kind}-{index:03}.json")
}

fn push_json_asset(
    assets: &mut Vec<PptxBuildAsset>,
    path: &str,
    value: &impl Serialize,
) -> Result<(), BuildCompileError> {
    let mut contents = serde_json::to_vec_pretty(value).map_err(|cause| {
        invalid(
            "/",
            format!("failed to encode generated build asset: {cause}"),
        )
    })?;
    contents.push(b'\n');
    assets.push(PptxBuildAsset {
        path: path.to_string(),
        contents,
    });
    Ok(())
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

fn invalid(path: &str, message: impl Into<String>) -> BuildCompileError {
    error(path, None, "BUILD_SPEC_VALUE_INVALID", message)
}

fn unsupported(path: &str, message: impl Into<String>) -> BuildCompileError {
    error(path, None, "BUILD_SPEC_OPERATION_UNAVAILABLE", message)
}

fn error(
    path: &str,
    op_id: Option<&str>,
    code: &str,
    message: impl Into<String>,
) -> BuildCompileError {
    BuildCompileError {
        code: code.to_string(),
        path: path.to_string(),
        op_id: op_id.map(str::to_string),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_stage_scrubber_handles_windows_native_quoted_and_verbatim_paths() {
        let prefix = r"C:\Users\RUNNER~1\AppData\Local\Temp\ooxml-pptx-build-123";
        let value = json!({
            "nativeCommand": format!("ooxml validate --strict {prefix}\\new-presentation.pptx"),
            "quoted": format!("\"{prefix}\""),
            "verbatim": format!(r"\\?\{prefix}\operations.json"),
            "forwardSlash": format!("{}/new-presentation.pptx", prefix.replace('\\', "/")),
            "nestedJson": format!(r#"{{"file":"{}\\generated\\bullets.json"}}"#, prefix.replace('\\', r"\\")),
        });

        let scrubbed = scrub_generated_paths(value, Path::new(prefix));
        let serialized = serde_json::to_string(&scrubbed).unwrap();
        assert!(!serialized.contains("RUNNER~1"));
        assert_eq!(serialized.matches("<build-stage>").count(), 5);
        assert_eq!(
            scrubbed["nativeCommand"],
            "ooxml validate --strict <build-stage>\\new-presentation.pptx"
        );
        assert_eq!(scrubbed["quoted"], "<build-stage>");
        assert_eq!(scrubbed["verbatim"], "<build-stage>\\operations.json");
        assert_eq!(
            scrubbed["forwardSlash"],
            "<build-stage>/new-presentation.pptx"
        );
    }
}
