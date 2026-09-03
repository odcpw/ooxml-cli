use ooxml_cli::build::{BuildFamily, markdown_to_spec};
use serde_json::{Value, json};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const MARKDOWN_FIXTURE: &str = "testdata/markdown/q3-review.md";
const DOCX_MARKDOWN_FIXTURE: &str = "testdata/markdown/quarterly-report.md";
const SPEC_GOLDEN: &str = "testdata/golden/build-spec/markdown/q3-review-pptx.json";
const OUTLINE_GOLDEN: &str = "testdata/golden/build-spec/markdown/q3-review-outline.json";
const DOCX_SPEC_GOLDEN: &str = "testdata/golden/build-spec/markdown/quarterly-report-docx.json";
const DOCX_OUTLINE_GOLDEN: &str =
    "testdata/golden/build-spec/markdown/quarterly-report-outline.json";

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args(args)
        .output()
        .expect("run ooxml")
}

fn json_stdout(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "JSON stdout ({error}): {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn json_error(output: &Output) -> Value {
    let bytes = if output.stdout.is_empty() {
        &output.stderr
    } else {
        &output.stdout
    };
    serde_json::from_slice(bytes).unwrap_or_else(|error| {
        panic!(
            "JSON error ({error}): {}\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(bytes),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn temp_dir(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "ooxml-markdown-build-{label}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("create Markdown build temp directory");
    path
}

#[test]
fn family_neutral_converter_maps_rich_pptx_markdown_and_reports_source_lines() {
    let source = fs::read_to_string(MARKDOWN_FIXTURE).expect("read committed Markdown fixture");
    assert!(!source.as_bytes().contains(&b'\r'), "fixture must use LF");
    let conversion = markdown_to_spec(BuildFamily::Pptx, &source, MARKDOWN_FIXTURE)
        .expect("convert committed PPTX Markdown");
    assert_eq!(conversion.spec["family"], "pptx");
    assert_eq!(conversion.spec["theme"], "corporate");
    assert_eq!(conversion.spec["size"], "16:9");
    let slides = conversion.spec["slides"].as_array().expect("slides");
    assert_eq!(slides.len(), 5);
    assert_eq!(slides[0]["layout"], "Title Slide");
    assert_eq!(slides[1]["bullets"][0]["runs"][0]["bold"], true);
    assert_eq!(
        slides[1]["bullets"][1]["runs"][3]["link"],
        "https://example.test/q3"
    );
    assert_eq!(slides[1]["bullets"][1]["runs"][1]["inlineCode"], true);
    assert_eq!(slides[2]["tables"][0]["rows"][3][1], "45");
    assert_eq!(slides[3]["charts"][0]["type"], "column");
    assert_eq!(slides[4]["images"][0]["altText"], "Product detail");
    assert_eq!(
        slides[1]["notes"],
        "Explain the Q3 variance before discussing the forecast."
    );
    assert!(conversion.warnings.iter().any(|warning| {
        warning.line == 13 && warning.code == "MARKDOWN_NUMBERING_RENDERED_AS_BULLET"
    }));

    let unsupported = markdown_to_spec(
        BuildFamily::Pptx,
        "# Quote\n\n> retained words\n",
        "quote.md",
    )
    .expect("unsupported syntax is preserved");
    assert_eq!(
        unsupported.spec["slides"][0]["subtitle"],
        "> retained words"
    );
    assert!(
        unsupported.warnings.iter().any(|warning| {
            warning.line == 3 && warning.code == "MARKDOWN_BLOCKQUOTE_UNSUPPORTED"
        })
    );
}

#[test]
fn pptx_markdown_build_emits_spec_and_builds_equivalent_strict_sdk_clean_decks() {
    let temp = temp_dir("pptx");
    let source_dir = temp.join("source");
    fs::create_dir_all(&source_dir).expect("create portable Markdown source directory");
    let markdown_input = source_dir.join("q3-review.md");
    fs::copy(MARKDOWN_FIXTURE, &markdown_input).expect("copy Markdown fixture");
    fs::copy("testdata/test_image.png", temp.join("test_image.png"))
        .expect("copy Markdown image asset");
    let markdown_output = temp.join("markdown.pptx");
    let json_output = temp.join("json.pptx");
    let emitted_spec = source_dir.join("emitted.json");
    let markdown_input_text = markdown_input.to_str().expect("Markdown input path");
    let markdown_output_text = markdown_output.to_str().expect("Markdown output path");
    let json_output_text = json_output.to_str().expect("JSON output path");
    let emitted_spec_text = emitted_spec.to_str().expect("emitted spec path");

    let built = run(&[
        "--json",
        "pptx",
        "build",
        "--from-markdown",
        markdown_input_text,
        "--emit-spec",
        emitted_spec_text,
        "--out",
        markdown_output_text,
        "--check",
    ]);
    assert!(
        built.status.success(),
        "Markdown build stderr: {}",
        String::from_utf8_lossy(&built.stderr)
    );
    let result = json_stdout(&built);
    assert_eq!(result["validated"], true);
    assert_eq!(result["check"]["summary"]["errors"], 0);
    assert_eq!(result["layoutQa"]["totalCollisions"], 0);
    assert_eq!(result["layoutQa"]["totalTextOverflows"], 0);
    assert_eq!(result["layoutQa"]["totalOffSlide"], 0);
    assert_eq!(result["layoutQa"]["totalSafeMarginViolations"], 0);
    assert_eq!(result["markdown"], markdown_input_text);
    assert_eq!(result["emittedSpec"], emitted_spec_text);
    assert!(result["warnings"].as_array().is_some_and(|warnings| {
        warnings.iter().any(|warning| {
            warning["line"] == 13 && warning["code"] == "MARKDOWN_NUMBERING_RENDERED_AS_BULLET"
        })
    }));
    assert!(
        result["compiledPlan"]["operations"]
            .as_array()
            .expect("compiled operations")
            .iter()
            .filter(|operation| operation["command"] == "pptx text set")
            .count()
            >= 2
    );

    compare_or_update(
        SPEC_GOLDEN,
        &fs::read(&emitted_spec).expect("read emitted spec"),
    );
    let outline = semantic_outline(&result["outline"]);
    let mut outline_bytes = serde_json::to_vec_pretty(&outline).expect("serialize outline golden");
    outline_bytes.push(b'\n');
    compare_or_update(OUTLINE_GOLDEN, &outline_bytes);

    let json_built = run(&[
        "--json",
        "pptx",
        "build",
        "--spec",
        emitted_spec_text,
        "--out",
        json_output_text,
    ]);
    assert!(
        json_built.status.success(),
        "JSON twin build stderr: {}",
        String::from_utf8_lossy(&json_built.stderr)
    );
    assert_eq!(
        fs::read(&markdown_output).expect("read Markdown deck"),
        fs::read(&json_output).expect("read JSON twin deck"),
        "Markdown and emitted JSON twins must produce byte-identical decks"
    );
    assert!(
        package_text_contains(&markdown_output, "https://example.test/q3"),
        "Markdown link must be preserved as an external package relationship"
    );
    assert!(
        package_text_contains(&markdown_output, "Aptos Mono"),
        "inline code must be preserved with the deterministic monospace typeface"
    );

    let strict = run(&["--json", "validate", "--strict", markdown_output_text]);
    assert!(
        strict.status.success(),
        "strict validation stderr: {}",
        String::from_utf8_lossy(&strict.stderr)
    );
    validate_with_openxml_sdk_if_available(&markdown_output);
    let _ = fs::remove_dir_all(temp);
}

#[test]
fn pptx_markdown_flags_reject_ambiguous_or_stdout_clobbering_inputs() {
    let temp = temp_dir("flags");
    let output = temp.join("out.pptx");
    let output_text = output.to_str().expect("output path");
    let ambiguous = run(&[
        "--json",
        "pptx",
        "build",
        "--spec",
        "deck.json",
        "--from-markdown",
        MARKDOWN_FIXTURE,
        "--out",
        output_text,
    ]);
    assert_eq!(ambiguous.status.code(), Some(2));
    assert!(
        json_error(&ambiguous)["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("mutually exclusive"))
    );

    let stdout_spec = run(&[
        "--json",
        "pptx",
        "build",
        "--from-markdown",
        MARKDOWN_FIXTURE,
        "--emit-spec",
        "-",
        "--out",
        output_text,
    ]);
    assert_eq!(stdout_spec.status.code(), Some(2));
    assert!(
        json_error(&stdout_spec)["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("stdout is reserved"))
    );
    assert!(!output.exists());
    let _ = fs::remove_dir_all(temp);
}

#[test]
fn docx_markdown_build_emits_spec_and_builds_equivalent_strict_sdk_clean_documents() {
    let temp = temp_dir("docx");
    let source_dir = temp.join("source");
    fs::create_dir_all(&source_dir).expect("create portable DOCX Markdown source directory");
    let markdown_input = source_dir.join("quarterly-report.md");
    fs::copy(DOCX_MARKDOWN_FIXTURE, &markdown_input).expect("copy DOCX Markdown fixture");
    fs::copy("testdata/test_image.png", temp.join("test_image.png"))
        .expect("copy DOCX Markdown image asset");
    let markdown_output = temp.join("markdown.docx");
    let json_output = temp.join("json.docx");
    let emitted_spec = source_dir.join("emitted.json");
    let markdown_input_text = markdown_input.to_str().expect("DOCX Markdown input path");
    let markdown_output_text = markdown_output.to_str().expect("DOCX output path");
    let json_output_text = json_output.to_str().expect("DOCX JSON output path");
    let emitted_spec_text = emitted_spec.to_str().expect("DOCX emitted spec path");

    let built = run(&[
        "--json",
        "docx",
        "build",
        "--from-markdown",
        markdown_input_text,
        "--emit-spec",
        emitted_spec_text,
        "--out",
        markdown_output_text,
        "--check",
    ]);
    assert!(
        built.status.success(),
        "DOCX Markdown build stderr: {}",
        String::from_utf8_lossy(&built.stderr)
    );
    let result = json_stdout(&built);
    assert_eq!(result["validated"], true);
    assert_eq!(result["check"]["summary"]["errors"], 0);
    assert_eq!(result["markdown"], markdown_input_text);
    assert_eq!(result["emittedSpec"], emitted_spec_text);
    assert!(
        result.get("warnings").is_none(),
        "fully supported DOCX Markdown should not emit warnings: {result}"
    );
    compare_or_update(
        DOCX_SPEC_GOLDEN,
        &fs::read(&emitted_spec).expect("read emitted DOCX spec"),
    );
    let outline = semantic_docx_outline(&result["outline"]);
    let mut outline_bytes = serde_json::to_vec_pretty(&outline).expect("serialize DOCX outline");
    outline_bytes.push(b'\n');
    compare_or_update(DOCX_OUTLINE_GOLDEN, &outline_bytes);

    let json_built = run(&[
        "--json",
        "docx",
        "build",
        "--spec",
        emitted_spec_text,
        "--out",
        json_output_text,
    ]);
    assert!(
        json_built.status.success(),
        "DOCX JSON twin build stderr: {}",
        String::from_utf8_lossy(&json_built.stderr)
    );
    assert_eq!(
        fs::read(&markdown_output).expect("read Markdown DOCX"),
        fs::read(&json_output).expect("read JSON twin DOCX"),
        "Markdown and emitted JSON twins must produce byte-identical documents"
    );
    assert!(package_text_contains(
        &markdown_output,
        "https://example.test/q3-report"
    ));
    assert!(package_text_contains(&markdown_output, "Consolas"));
    let strict = run(&["--json", "validate", "--strict", markdown_output_text]);
    assert!(
        strict.status.success(),
        "strict DOCX validation stderr: {}",
        String::from_utf8_lossy(&strict.stderr)
    );
    validate_with_openxml_sdk_if_available(&markdown_output);
    let _ = fs::remove_dir_all(temp);
}

fn semantic_outline(outline: &Value) -> Value {
    let slides = outline["slides"]
        .as_array()
        .expect("outline slides")
        .iter()
        .map(|slide| {
            json!({
                "number": slide["number"],
                "layout": slide["layout"],
                "title": slide["title"],
                "textPreview": slide["textPreview"],
                "shapeCount": slide["shapeCount"],
                "tableCount": slide["tableCount"],
                "imageCount": slide["imageCount"],
                "notes": slide["notes"],
            })
        })
        .collect::<Vec<_>>();
    json!({
        "schemaVersion": outline["schemaVersion"],
        "summary": outline["summary"],
        "slides": slides,
    })
}

fn semantic_docx_outline(outline: &Value) -> Value {
    let blocks = outline["blocks"]
        .as_array()
        .expect("DOCX outline blocks")
        .iter()
        .map(|block| {
            json!({
                "index": block["index"],
                "kind": block["kind"],
                "styleId": block["styleId"],
                "textPreview": block["textPreview"],
            })
        })
        .collect::<Vec<_>>();
    json!({
        "schemaVersion": outline["schemaVersion"],
        "summary": outline["summary"],
        "coreProperties": outline["coreProperties"],
        "blocks": blocks,
    })
}

fn compare_or_update(path: &str, actual: &[u8]) {
    if std::env::var_os("UPDATE_GOLDENS").is_some() {
        if let Some(parent) = Path::new(path).parent() {
            fs::create_dir_all(parent).expect("create golden directory");
        }
        fs::write(path, actual).expect("write reviewed Markdown build golden");
    }
    let expected = fs::read(path).unwrap_or_else(|error| {
        panic!("missing {path}: {error}; rerun this target with UPDATE_GOLDENS=1 and review")
    });
    assert_eq!(actual, expected, "Markdown build golden drift: {path}");
}

fn validate_with_openxml_sdk_if_available(path: &Path) {
    let dotnet = Path::new("/home/oliver/dotnet/dotnet");
    let validator = Path::new(
        "/home/oliver/Projects/odcpw/ooxml-cli/tools/openxml-validator/bin/Release/net8.0/openxml-validator.dll",
    );
    if !dotnet.is_file() || !validator.is_file() {
        return;
    }
    let output = Command::new(dotnet)
        .arg(validator)
        .arg(path)
        .output()
        .expect("run Open XML SDK validator");
    assert!(
        output.status.success(),
        "SDK stdout: {}\nSDK stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("0 errors"));
}

fn package_text_contains(path: &Path, needle: &str) -> bool {
    let file = fs::File::open(path).expect("open built OOXML package");
    let mut archive = zip::ZipArchive::new(file).expect("open package as zip");
    for index in 0..archive.len() {
        let mut part = archive.by_index(index).expect("read package part");
        if !(part.name().ends_with(".xml") || part.name().ends_with(".rels")) {
            continue;
        }
        let mut text = String::new();
        if part.read_to_string(&mut text).is_ok() && text.contains(needle) {
            return true;
        }
    }
    false
}
