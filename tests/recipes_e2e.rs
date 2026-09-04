use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const SOURCE_DATE_EPOCH: &str = "946684800";

#[derive(Clone, Copy)]
enum RecipeSource {
    Spec(&'static str),
    Markdown(&'static str),
}

#[derive(Clone, Copy)]
struct Recipe {
    slug: &'static str,
    family: &'static str,
    extension: &'static str,
    source: RecipeSource,
    operation_count: usize,
    outline_count_key: &'static str,
    outline_count: u64,
    render_collection: &'static str,
    render_count_key: &'static str,
    render_count: usize,
}

const Q3_DECK: Recipe = Recipe {
    slug: "q3-review-deck",
    family: "pptx",
    extension: "pptx",
    source: RecipeSource::Spec("testdata/pptx/build-spec/q3-review.json"),
    operation_count: 12,
    outline_count_key: "slides",
    outline_count: 5,
    render_collection: "slides",
    render_count_key: "slide",
    render_count: 5,
};

const SALES_WORKBOOK: Recipe = Recipe {
    slug: "sales-workbook",
    family: "xlsx",
    extension: "xlsx",
    source: RecipeSource::Spec("testdata/xlsx/build-spec/sales.json"),
    operation_count: 36,
    outline_count_key: "sheets",
    outline_count: 2,
    render_collection: "pages",
    render_count_key: "page",
    render_count: 2,
};

const QUARTERLY_REPORT: Recipe = Recipe {
    slug: "quarterly-report-document",
    family: "docx",
    extension: "docx",
    source: RecipeSource::Spec("testdata/docx/build-spec/quarterly-report.json"),
    operation_count: 20,
    outline_count_key: "paragraphs",
    outline_count: 15,
    render_collection: "pages",
    render_count_key: "page",
    render_count: 2,
};

const MARKDOWN_DECK: Recipe = Recipe {
    slug: "markdown-q3-review-deck",
    family: "pptx",
    extension: "pptx",
    source: RecipeSource::Markdown("testdata/markdown/q3-review.md"),
    operation_count: 13,
    outline_count_key: "slides",
    outline_count: 5,
    render_collection: "slides",
    render_count_key: "slide",
    render_count: 5,
};

const MARKDOWN_DOCUMENT: Recipe = Recipe {
    slug: "markdown-quarterly-report-document",
    family: "docx",
    extension: "docx",
    source: RecipeSource::Markdown("testdata/markdown/quarterly-report.md"),
    operation_count: 19,
    outline_count_key: "paragraphs",
    outline_count: 12,
    render_collection: "pages",
    render_count_key: "page",
    render_count: 2,
};

#[test]
fn q3_review_deck_runs_the_full_agent_proof_chain() {
    run_recipe(Q3_DECK);
}

#[test]
fn sales_workbook_runs_the_full_agent_proof_chain() {
    run_recipe(SALES_WORKBOOK);
}

#[test]
fn quarterly_report_document_runs_the_full_agent_proof_chain() {
    run_recipe(QUARTERLY_REPORT);
}

#[test]
fn markdown_q3_review_deck_runs_the_full_agent_proof_chain() {
    run_recipe(MARKDOWN_DECK);
}

#[test]
fn markdown_quarterly_report_document_runs_the_full_agent_proof_chain() {
    run_recipe(MARKDOWN_DOCUMENT);
}

fn run_recipe(recipe: Recipe) {
    let root = artifact_root().join(recipe.slug);
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create recipe proof directory");
    let first = root.join(format!("first.{}", recipe.extension));
    let second = root.join(format!("second.{}", recipe.extension));
    let source = recipe_source_path(recipe.source);
    assert_lf(&source);

    let mut first_args = build_args(recipe, &source, &first);
    first_args.push("--check".to_string());
    if matches!(recipe.source, RecipeSource::Markdown(_)) {
        first_args.push("--emit-spec".to_string());
        first_args.push(path(&root.join("emitted-spec.json")));
    }
    let build = run_json_step(recipe, &root, "01-build", &first_args);
    assert_build_contract(recipe, &build, &first);

    let repeat = run_json_step(
        recipe,
        &root,
        "02-build-repeat",
        &build_args(recipe, &source, &second),
    );
    assert_build_contract(recipe, &repeat, &second);
    let first_bytes = fs::read(&first).expect("read first recipe output");
    let second_bytes = fs::read(&second).expect("read repeated recipe output");
    assert_eq!(
        first_bytes, second_bytes,
        "{} is not byte deterministic under SOURCE_DATE_EPOCH={SOURCE_DATE_EPOCH}",
        recipe.slug
    );

    let outline = run_json_step(
        recipe,
        &root,
        "03-outline",
        &[
            "--json".to_string(),
            "outline".to_string(),
            path(&first),
            "--depth".to_string(),
            "3".to_string(),
            "--text-preview".to_string(),
            "240".to_string(),
        ],
    );
    assert_eq!(outline["type"], recipe.family, "{outline}");
    assert_eq!(
        outline["summary"][recipe.outline_count_key].as_u64(),
        Some(recipe.outline_count),
        "{} outline count drifted: {outline}",
        recipe.slug
    );

    let check = run_json_step(
        recipe,
        &root,
        "04-check",
        &["--json".to_string(), "check".to_string(), path(&first)],
    );
    assert_zero_errors(recipe, "check", &check);

    let design = run_json_step(
        recipe,
        &root,
        "05-design-check",
        &[
            "--json".to_string(),
            "design-check".to_string(),
            path(&first),
        ],
    );
    assert_zero_errors(recipe, "design-check", &design);

    let strict = run_json_step(
        recipe,
        &root,
        "06-strict-validation",
        &[
            "--json".to_string(),
            "validate".to_string(),
            "--strict".to_string(),
            path(&first),
        ],
    );
    assert_eq!(strict["valid"], true, "{}: {strict}", recipe.slug);
    assert_zero_errors(recipe, "strict validation", &strict);

    let conformance = run_json_step(
        recipe,
        &root,
        "07-openxml-sdk",
        &[
            "--json".to_string(),
            "conformance".to_string(),
            "check".to_string(),
            path(&first),
            "--openxml-sdk".to_string(),
        ],
    );
    assert_sdk_contract(recipe, &conformance);

    let layout = if recipe.family == "pptx" {
        let report = run_json_step(
            recipe,
            &root,
            "08-layout",
            &[
                "--json".to_string(),
                "pptx".to_string(),
                "validate-layout".to_string(),
                path(&first),
            ],
        );
        assert_layout_contract(recipe, &report);
        report
    } else {
        Value::Null
    };

    let render = run_json_step(
        recipe,
        &root,
        "09-render",
        &[
            "--json".to_string(),
            "render".to_string(),
            path(&first),
            "--out".to_string(),
            path(&root.join("render")),
        ],
    );
    assert_render_contract(recipe, &render);

    let tool_availability = json!({
        "buildCheckSchema": build["check"]["checks"]["schema"],
        "checkSchema": check["checks"]["schema"],
        "openXmlSdk": conformance["checks"]
            .as_array()
            .and_then(|checks| checks.iter().find(|check| check["name"] == "schema"))
            .map(|check| check["status"].clone())
            .unwrap_or(Value::Null),
        "libreOffice": render["status"],
    });
    write_pretty_json(&root.join("tool-availability.json"), &tool_availability);

    let proof = recipe_proof_summary(
        recipe,
        &root,
        &build,
        &outline,
        &check,
        &design,
        &strict,
        &conformance,
        &layout,
        &first_bytes,
    );
    write_pretty_json(&root.join("proof-summary.json"), &proof);
    assert_golden(recipe, &proof);
}

fn build_args(recipe: Recipe, source: &Path, output: &Path) -> Vec<String> {
    let (source_flag, source_path) = match recipe.source {
        RecipeSource::Spec(_) => ("--spec", path(source)),
        RecipeSource::Markdown(_) => ("--from-markdown", path(source)),
    };
    vec![
        "--json".to_string(),
        recipe.family.to_string(),
        "build".to_string(),
        source_flag.to_string(),
        source_path,
        "--out".to_string(),
        path(output),
    ]
}

fn assert_build_contract(recipe: Recipe, build: &Value, output: &Path) {
    assert_eq!(build["validated"], true, "{}: {build}", recipe.slug);
    assert!(
        output.is_file(),
        "{} did not publish its output",
        recipe.slug
    );
    let envelope = &build["mutationEnvelope"];
    assert_eq!(envelope["validated"], true, "{}: {envelope}", recipe.slug);
    assert_eq!(
        envelope["opsCount"].as_u64(),
        Some(recipe.operation_count as u64),
        "{} mutation count drifted: {envelope}",
        recipe.slug
    );
    let applied = envelope["applied"].as_array().unwrap_or_else(|| {
        panic!(
            "{} mutation envelope lacks applied rows: {envelope}",
            recipe.slug
        )
    });
    assert_eq!(applied.len(), recipe.operation_count, "{envelope}");
    for operation in applied {
        assert!(
            operation["readback"].is_object(),
            "{} operation lacks a real readback: {operation}",
            recipe.slug
        );
        let destination = &operation["mutationEnvelope"]["destination"];
        for key in ["partUri", "primarySelector", "handle", "kind"] {
            assert!(
                destination[key]
                    .as_str()
                    .is_some_and(|value| !value.is_empty()),
                "{} operation destination lacks {key}: {operation}",
                recipe.slug
            );
        }
    }
    if !build["check"].is_null() {
        assert_eq!(build["check"]["summary"]["errors"], 0, "{build}");
    }
    if recipe.family == "pptx" {
        assert_layout_contract(recipe, &build["layoutQa"]);
    }
    if matches!(recipe.source, RecipeSource::Markdown(_)) {
        assert!(
            build["markdown"]
                .as_str()
                .is_some_and(|path| path.ends_with(".md")),
            "{} Markdown source is not reported: {build}",
            recipe.slug
        );
        assert!(
            build["emittedSpec"]
                .as_str()
                .is_none_or(|path| Path::new(path).is_file()),
            "{} emitted spec was not published: {build}",
            recipe.slug
        );
    }
}

fn assert_zero_errors(recipe: Recipe, label: &str, report: &Value) {
    assert_eq!(
        report["summary"]["errors"], 0,
        "{} {label} findings: {report}",
        recipe.slug
    );
}

fn assert_sdk_contract(recipe: Recipe, report: &Value) {
    assert_zero_errors(recipe, "conformance", report);
    assert_eq!(report["summary"]["failed"], 0, "{report}");
    for name in ["package-open", "repo-validation", "repair-invariants"] {
        assert!(
            report["checks"].as_array().is_some_and(|checks| checks
                .iter()
                .any(|check| check["name"] == name && check["status"] == "passed")),
            "{} lacks passed {name} proof: {report}",
            recipe.slug
        );
    }
    let schema = report["checks"]
        .as_array()
        .and_then(|checks| checks.iter().find(|check| check["name"] == "schema"))
        .unwrap_or_else(|| panic!("{} lacks schema proof: {report}", recipe.slug));
    if schema["status"] == "skipped" {
        println!(
            "SKIP Open XML SDK {} recipe proof: {}",
            recipe.slug, schema["diagnostics"]
        );
        assert!(
            !proof_required("OOXML_REQUIRE_OPENXML_SDK"),
            "Open XML SDK proof was required but skipped: {schema}"
        );
    } else {
        assert_eq!(schema["status"], "passed", "{}: {schema}", recipe.slug);
        assert_eq!(schema["schemaCheck"]["checked"], true, "{schema}");
        assert_eq!(schema["schemaCheck"]["valid"], true, "{schema}");
        assert_eq!(schema["schemaCheck"]["errorCount"], 0, "{schema}");
        assert_eq!(
            schema["schemaCheck"]["validator"], "openxml-sdk",
            "{schema}"
        );
    }
}

fn assert_layout_contract(recipe: Recipe, report: &Value) {
    for field in [
        "totalCollisions",
        "totalTextOverflows",
        "totalOffSlide",
        "totalSafeMarginViolations",
    ] {
        assert_eq!(
            report[field], 0,
            "{} layout {field} findings: {report}",
            recipe.slug
        );
    }
    if let Some(total) = report.get("totalSlides") {
        assert_eq!(total, recipe.outline_count, "{report}");
    }
}

fn assert_render_contract(recipe: Recipe, report: &Value) {
    assert_eq!(report["engine"], "libreoffice", "{report}");
    if report["status"] == "skipped" {
        println!(
            "SKIP LibreOffice {} recipe render: {}",
            recipe.slug, report["missingTools"]
        );
        assert!(
            !render_required(),
            "LibreOffice render proof was required but skipped: {report}"
        );
        assert!(
            report["missingTools"]
                .as_array()
                .is_some_and(|tools| !tools.is_empty()),
            "clean render skip must name missing tools: {report}"
        );
        assert!(
            report["remediation"]
                .as_str()
                .is_some_and(|message| !message.is_empty()),
            "clean render skip must include remediation: {report}"
        );
        return;
    }

    assert_eq!(report["status"], "ok", "{}: {report}", recipe.slug);
    let rendered = report[recipe.render_collection]
        .as_array()
        .unwrap_or_else(|| panic!("{} render collection missing: {report}", recipe.slug));
    assert_eq!(rendered.len(), recipe.render_count, "{report}");
    for (index, item) in rendered.iter().enumerate() {
        assert_eq!(
            item[recipe.render_count_key].as_u64(),
            Some((index + 1) as u64),
            "{} render order drifted: {report}",
            recipe.slug
        );
        let image = item["imagePath"].as_str().expect("rendered image path");
        assert!(
            fs::metadata(image).is_ok_and(|metadata| metadata.len() > 1_000),
            "{} render image is missing or empty: {image}",
            recipe.slug
        );
    }
    let pdf = report["pdfPath"].as_str().expect("rendered PDF path");
    assert!(
        fs::metadata(pdf).is_ok_and(|metadata| metadata.len() > 1_000),
        "{} render PDF is missing or empty: {pdf}",
        recipe.slug
    );
}

#[allow(clippy::too_many_arguments)]
fn recipe_proof_summary(
    recipe: Recipe,
    root: &Path,
    build: &Value,
    outline: &Value,
    check: &Value,
    design: &Value,
    strict: &Value,
    conformance: &Value,
    layout: &Value,
    package: &[u8],
) -> Value {
    let source = match recipe.source {
        RecipeSource::Spec(path) => json!({"kind": "spec", "path": path}),
        RecipeSource::Markdown(path) => json!({"kind": "markdown", "path": path}),
    };
    json!({
        "schemaVersion": "ooxml-cli.recipe-e2e.v1",
        "recipe": recipe.slug,
        "family": recipe.family,
        "source": source,
        "build": {
            "schemaVersion": build["schemaVersion"],
            "validated": build["validated"],
            "operations": mutation_readback_contract(build, root),
            "nodeMap": normalize_paths(build["nodeMap"].clone(), root),
            "checkSummary": portable_check_contract(build["check"].clone())["summary"],
            "layoutQa": compact_layout(&build["layoutQa"]),
            "warnings": normalize_paths(build.get("warnings").cloned().unwrap_or(Value::Null), root),
        },
        "outline": normalize_paths(outline.clone(), root),
        "check": portable_check_contract(normalize_paths(check.clone(), root)),
        "designCheck": normalize_paths(design.clone(), root),
        "strictValidation": normalize_paths(strict.clone(), root),
        "conformance": portable_conformance_contract(conformance),
        "layout": if layout.is_null() { Value::Null } else { normalize_paths(layout.clone(), root) },
        "render": {
            "engine": "libreoffice",
            "policy": "real render with exact count, or clean skip when the renderer is unavailable",
            "collection": recipe.render_collection,
            "countKey": recipe.render_count_key,
            "expectedCount": recipe.render_count,
        },
        "determinism": {
            "sourceDateEpoch": SOURCE_DATE_EPOCH,
            "byteLength": package.len(),
            "sha256": sha256_bytes(package),
        },
    })
}

fn mutation_readback_contract(build: &Value, root: &Path) -> Value {
    let operations = build["mutationEnvelope"]["applied"]
        .as_array()
        .expect("applied mutation operations");
    Value::Array(
        operations
            .iter()
            .map(|operation| {
                let readback =
                    normalize_volatiles(normalize_paths(operation["readback"].clone(), root));
                let mut readback_keys = readback
                    .as_object()
                    .expect("operation readback object")
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>();
                readback_keys.sort();
                let destination = &operation["mutationEnvelope"]["destination"];
                json!({
                    "index": operation["index"],
                    "id": operation["id"],
                    "command": operation["command"],
                    "resolvedArgs": normalize_paths(operation["resolvedArgs"].clone(), root),
                    "destination": {
                        "partUri": destination["partUri"],
                        "primarySelector": destination["primarySelector"],
                        "handle": destination["handle"],
                        "kind": destination["kind"],
                    },
                    "readbackKeys": readback_keys,
                    "readbackSha256": sha256_value(&readback),
                })
            })
            .collect(),
    )
}

fn portable_conformance_contract(report: &Value) -> Value {
    let checks = report["checks"].as_array().expect("conformance checks");
    json!({
        "schemaVersion": report["schemaVersion"],
        // Optional schema-tool availability is asserted against the live report and written to
        // tool-availability.json. It must not change the portable proof golden.
        "status": "passed",
        "errors": report["summary"]["errors"],
        "warnings": 0,
        "checks": checks.iter().map(|check| {
            if check["name"] == "schema" {
                json!({
                    "name": "schema",
                    "contract": "Open XML SDK passed, or cleanly skipped when unavailable",
                    "validator": "openxml-sdk",
                })
            } else {
                json!({"name": check["name"], "status": check["status"]})
            }
        }).collect::<Vec<_>>(),
    })
}

fn portable_check_contract(mut report: Value) -> Value {
    let skipped = report["findings"]
        .as_array()
        .map(|findings| {
            findings
                .iter()
                .filter(|finding| finding["code"] == "CHECK_OPENXML_SDK_SKIPPED")
                .count()
        })
        .unwrap_or(0);
    if skipped == 0 {
        return report;
    }

    if let Some(findings) = report["findings"].as_array_mut() {
        findings.retain(|finding| finding["code"] != "CHECK_OPENXML_SDK_SKIPPED");
    }
    for field in ["info", "total"] {
        if let Some(value) = report["summary"][field].as_u64() {
            report["summary"][field] = json!(value.saturating_sub(skipped as u64));
        }
    }
    report["checks"]["schema"] = json!("passed");
    report["proofLevel"]["schema"] = json!("passed");
    report
}

#[test]
fn proof_golden_normalizes_openxml_sdk_availability() {
    let passed = json!({
        "schemaVersion": "ooxml-cli.conformance.v1",
        "status": "passed",
        "summary": {"errors": 0, "warnings": 0},
        "checks": [
            {"name": "package-open", "status": "passed"},
            {"name": "repo-validation", "status": "passed"},
            {"name": "repair-invariants", "status": "passed"},
            {"name": "schema", "status": "passed"}
        ]
    });
    let skipped = json!({
        "schemaVersion": "ooxml-cli.conformance.v1",
        "status": "passed_with_warnings",
        "summary": {"errors": 0, "warnings": 1},
        "checks": [
            {"name": "package-open", "status": "passed"},
            {"name": "repo-validation", "status": "passed"},
            {"name": "repair-invariants", "status": "passed"},
            {"name": "schema", "status": "skipped"}
        ]
    });

    assert_eq!(
        portable_conformance_contract(&passed),
        portable_conformance_contract(&skipped)
    );
}

#[test]
fn proof_golden_removes_only_the_optional_sdk_check_finding() {
    let passed = json!({
        "status": "warning",
        "checks": {"schema": "passed", "strict": "passed"},
        "proofLevel": {"schema": "passed", "strict": "passed"},
        "summary": {"errors": 0, "warnings": 2, "info": 0, "total": 2},
        "findings": [
            {"code": "DESIGN_ONE", "severity": "warning"},
            {"code": "DESIGN_TWO", "severity": "warning"}
        ]
    });
    let skipped = json!({
        "status": "warning",
        "checks": {"schema": "skipped", "strict": "passed"},
        "proofLevel": {"schema": "skipped", "strict": "passed"},
        "summary": {"errors": 0, "warnings": 2, "info": 1, "total": 3},
        "findings": [
            {"code": "DESIGN_ONE", "severity": "warning"},
            {"code": "DESIGN_TWO", "severity": "warning"},
            {"code": "CHECK_OPENXML_SDK_SKIPPED", "severity": "info"}
        ]
    });

    assert_eq!(
        portable_check_contract(passed),
        portable_check_contract(skipped)
    );
}

fn compact_layout(report: &Value) -> Value {
    if report.is_null() {
        return Value::Null;
    }
    json!({
        "totalCollisions": report["totalCollisions"],
        "totalTextOverflows": report["totalTextOverflows"],
        "totalOffSlide": report["totalOffSlide"],
        "totalSafeMarginViolations": report["totalSafeMarginViolations"],
    })
}

fn run_json_step(recipe: Recipe, root: &Path, step: &str, args: &[String]) -> Value {
    let output = run(args);
    let raw_path = root.join(format!("{step}.json"));
    fs::write(&raw_path, &output.stdout).expect("write raw recipe proof JSON");
    let value: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "{} {step} returned invalid JSON ({error})\ncommand: {:?}\nstdout: {}\nstderr: {}",
            recipe.slug,
            args,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    });
    let log = json!({
        "recipe": recipe.slug,
        "step": step,
        "command": normalize_paths(json!(args), root),
        "exitCode": output.status.code(),
        "envelope": value.get("mutationEnvelope").map(|envelope| json!({
            "validated": envelope["validated"],
            "opsCount": envelope["opsCount"],
            "appliedCount": envelope["applied"].as_array().map(Vec::len),
        })),
        "summary": value.get("summary"),
        "findings": value.get("findings"),
    });
    println!(
        "OOXML_RECIPE_STEP {}",
        serde_json::to_string(&log).expect("serialize structured recipe log")
    );
    assert!(
        output.status.success(),
        "{} {step} failed\ncommand: {:?}\nstdout: {}\nstderr: {}",
        recipe.slug,
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "{} {step} wrote diagnostics on success: {}",
        recipe.slug,
        String::from_utf8_lossy(&output.stderr)
    );
    value
}

fn run(args: &[String]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args(args)
        .env("SOURCE_DATE_EPOCH", SOURCE_DATE_EPOCH)
        .env("NO_COLOR", "1")
        .env("CI", "1")
        .output()
        .expect("run ooxml recipe step")
}

fn normalize_paths(value: Value, root: &Path) -> Value {
    let root = root.to_string_lossy().into_owned();
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .to_string_lossy()
        .into_owned();
    normalize_strings(
        value,
        &[(&root, "<artifact-root>"), (&repository, "<repo>")],
    )
}

fn normalize_strings(value: Value, replacements: &[(&str, &str)]) -> Value {
    let replacements = normalize_replacement_variants(replacements);
    normalize_strings_with_variants(value, &replacements)
}

fn normalize_strings_with_variants(value: Value, replacements: &[(String, String)]) -> Value {
    match value {
        Value::String(text) => {
            let normalized = replacements
                .iter()
                .fold(text, |text, (from, to)| text.replace(from, to));
            Value::String(normalize_placeholder_path_forms(normalized))
        }
        Value::Array(values) => Value::Array(
            values
                .into_iter()
                .map(|value| normalize_strings_with_variants(value, replacements))
                .collect(),
        ),
        Value::Object(values) => Value::Object(
            values
                .into_iter()
                .map(|(key, value)| (key, normalize_strings_with_variants(value, replacements)))
                .collect(),
        ),
        other => other,
    }
}

fn normalize_replacement_variants(replacements: &[(&str, &str)]) -> Vec<(String, String)> {
    let mut variants = Vec::new();
    for &(from, to) in replacements {
        if from.is_empty() {
            continue;
        }

        let native = from.replace('/', "\\");
        let slashed = from.replace('\\', "/");
        let mut path_variants = vec![from.to_string(), native.clone(), slashed.clone()];
        if native.as_bytes().get(1) == Some(&b':') {
            path_variants.push(format!(r"\\?\{native}"));
            path_variants.push(format!("//?/{slashed}"));
        }
        path_variants.sort();
        path_variants.dedup();

        for path in path_variants {
            let escaped = path.replace('\\', r"\\");
            for variant in [&path, &escaped] {
                variants.push((variant.to_string(), to.to_string()));
                variants.push((format!("'{variant}'"), to.to_string()));
                variants.push((format!("\"{variant}\""), to.to_string()));
            }
        }
    }
    variants.sort_by_key(|(from, _)| std::cmp::Reverse(from.len()));
    variants.dedup();
    variants
}

fn normalize_placeholder_path_forms(mut text: String) -> String {
    if !["<artifact-root>", "<repo>", "<build-stage>"]
        .into_iter()
        .any(|placeholder| text.contains(placeholder))
    {
        return text;
    }

    while text.contains(r"\\") {
        text = text.replace(r"\\", "/");
    }
    strip_shell_quotes_around_placeholders(&text.replace('\\', "/"))
}

fn strip_shell_quotes_around_placeholders(text: &str) -> String {
    let chars = text.chars().collect::<Vec<_>>();
    let mut normalized = String::with_capacity(text.len());
    let mut index = 0;

    while index < chars.len() {
        let quote = chars[index];
        if matches!(quote, '\'' | '"')
            && shell_quote_precedes_path(&chars, index)
            && let Some(end) = quoted_placeholder_end(&chars, index, quote)
        {
            normalized.extend(&chars[index + 1..end]);
            index = end + 1;
        } else {
            normalized.push(quote);
            index += 1;
        }
    }
    normalized
}

fn shell_quote_precedes_path(chars: &[char], index: usize) -> bool {
    index == 0 || chars[index - 1].is_whitespace() || chars[index - 1] == '='
}

fn quoted_placeholder_end(chars: &[char], start: usize, quote: char) -> Option<usize> {
    let mut end = start + 1;
    while end < chars.len() && chars[end] != quote {
        end += 1;
    }
    if end == chars.len() {
        return None;
    }

    let segment = chars[start + 1..end].iter().collect::<String>();
    ["<artifact-root>", "<repo>", "<build-stage>"]
        .into_iter()
        .any(|placeholder| segment.contains(placeholder))
        .then_some(end)
}

#[test]
fn normalize_paths_scrubs_windows_temp_path_forms() {
    let root = Path::new(r"C:\Users\RUNNER~1\AppData\Local\Temp\ooxml-recipe-e2e");
    let native = root.to_string_lossy();
    let slashed = native.replace('\\', "/");
    let escaped = native.replace('\\', r"\\");
    let verbatim = format!(r"\\?\{native}");
    let mut value = json!([
        format!("ooxml --json check '{native}\\first.docx'"),
        format!("ooxml --json check \"{slashed}/first.docx\""),
        format!(r#"{{"file":"{escaped}\\first.docx"}}"#),
        format!(r"ooxml --json check {verbatim}\first.docx"),
        r"<build-stage>\external\image.png".to_string(),
        r#"{"file":"<build-stage>\\external\\image.png"}"#.to_string(),
    ]);

    value = normalize_paths(value, root);

    assert_eq!(
        value,
        json!([
            "ooxml --json check <artifact-root>/first.docx",
            "ooxml --json check <artifact-root>/first.docx",
            r#"{"file":"<artifact-root>/first.docx"}"#,
            "ooxml --json check <artifact-root>/first.docx",
            "<build-stage>/external/image.png",
            r#"{"file":"<build-stage>/external/image.png"}"#,
        ])
    );
}

fn normalize_volatiles(value: Value) -> Value {
    match value {
        Value::Array(values) => Value::Array(values.into_iter().map(normalize_volatiles).collect()),
        Value::Object(values) => Value::Object(
            values
                .into_iter()
                .map(|(key, value)| {
                    if key == "createdAt" {
                        (key, Value::String("<timestamp>".to_string()))
                    } else {
                        (key, normalize_volatiles(value))
                    }
                })
                .collect(),
        ),
        other => other,
    }
}

fn assert_golden(recipe: Recipe, value: &Value) {
    let golden = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("testdata/golden/recipes-e2e")
        .join(format!("{}.json", recipe.slug));
    let mut rendered = serde_json::to_vec_pretty(value).expect("serialize recipe proof golden");
    rendered.push(b'\n');
    assert!(
        !rendered.contains(&b'\r'),
        "{} golden must use LF line endings",
        recipe.slug
    );
    if std::env::var("UPDATE_GOLDENS").as_deref() == Ok("1") {
        fs::create_dir_all(golden.parent().expect("golden parent"))
            .expect("create recipe golden directory");
        fs::write(&golden, &rendered).expect("write reviewed recipe proof golden");
    }
    let expected = fs::read(&golden).unwrap_or_else(|error| {
        panic!(
            "missing {}: {error}; rerun with UPDATE_GOLDENS=1 and review the diff",
            golden.display()
        )
    });
    assert!(
        !expected.contains(&b'\r'),
        "{} contains CRLF despite its text contract",
        golden.display()
    );
    assert_eq!(
        rendered,
        expected,
        "{} proof contract drifted",
        golden.display()
    );
}

fn write_pretty_json(path: &Path, value: &Value) {
    let mut bytes = serde_json::to_vec_pretty(value).expect("serialize recipe proof artifact");
    bytes.push(b'\n');
    fs::write(path, bytes).expect("write recipe proof summary");
}

fn recipe_source_path(source: RecipeSource) -> PathBuf {
    let relative = match source {
        RecipeSource::Spec(path) | RecipeSource::Markdown(path) => path,
    };
    Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
}

fn artifact_root() -> PathBuf {
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("target"));
    target.join("ooxml-recipe-e2e")
}

fn assert_lf(path: &Path) {
    let bytes = fs::read(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    assert!(
        !bytes.contains(&b'\r'),
        "{} must use LF line endings",
        path.display()
    );
}

fn render_required() -> bool {
    ["OOXML_REQUIRE_RENDER", "OOXML_REQUIRE_LIBREOFFICE"]
        .into_iter()
        .any(proof_required)
}

fn proof_required(name: &str) -> bool {
    std::env::var(name).is_ok_and(|value| value == "1" || value.eq_ignore_ascii_case("true"))
}

fn sha256_value(value: &Value) -> String {
    sha256_bytes(&serde_json::to_vec(value).expect("serialize readback for hashing"))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn path(path: &Path) -> String {
    path.to_str().expect("UTF-8 test path").to_string()
}
