use ooxml_cli::build::{
    BuildFamily, BuildSpec, compile_docx_spec, compile_pptx_spec, compile_xlsx_spec, load_spec_str,
    markdown_to_spec, schema_document,
};
use proptest::prelude::*;
use proptest::test_runner::{
    Config, FileFailurePersistence, RngAlgorithm, RngSeed, TestCaseError, TestError, TestRunner,
};
use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Value, json};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

const SOURCE_DATE_EPOCH: &str = "946684800";
// The normal gate stays quick. Scheduled/deep proof runs set
// OOXML_PROPERTY_CASES=1000; every property below honors the same override.
const DEFAULT_CASES: u32 = 4;
// Seed 75 generates the hosted macOS regression (`title: 7`) as the first
// DOCX Markdown case, so every platform replays the discovery deterministically.
const PROPERTY_RNG_SEED: u64 = 75;
static NEXT_CASE: AtomicU64 = AtomicU64::new(1);

fn property_cases() -> u32 {
    std::env::var("OOXML_PROPERTY_CASES")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_CASES)
}

fn runner() -> TestRunner {
    TestRunner::new(Config {
        cases: property_cases(),
        max_shrink_iters: 4_096,
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(
            "tests/build_property.proptest-regressions",
        ))),
        rng_algorithm: RngAlgorithm::ChaCha,
        rng_seed: RngSeed::Fixed(PROPERTY_RNG_SEED),
        ..Config::default()
    })
}

fn text_strategy() -> BoxedStrategy<String> {
    prop::collection::vec(
        prop_oneof![
            Just('a'),
            Just('Z'),
            Just('7'),
            Just(' '),
            Just('&'),
            Just('<'),
            Just('>'),
            Just('é'),
            Just('東'),
            Just('京'),
        ],
        1..24,
    )
    .prop_map(|characters| characters.into_iter().collect())
    .boxed()
}

fn pptx_spec_strategy() -> BoxedStrategy<Value> {
    (
        text_strategy(),
        text_strategy(),
        0usize..4,
        0usize..3,
        1i64..10_000,
        any::<bool>(),
    )
        .prop_map(|(title, body, chart_kind, fit_kind, number, unsupported)| {
            let chart_type = if unsupported {
                "doughnut"
            } else {
                ["bar", "line", "area", "pie"][chart_kind]
            };
            let fit = ["contain", "cover", "fill"][fit_kind];
            json!({
                "schemaVersion": 1,
                "family": "pptx",
                "themeSeed": "2457A6",
                "size": "16:9",
                "slides": [
                    {
                        "id": "cover",
                        "layout": "Title Slide",
                        "title": title,
                        "subtitle": body
                    },
                    {
                        "id": "content",
                        "layout": "Title and Content",
                        "title": "Generated content",
                        "bullets": [
                            {"text": body, "level": 0, "bullet": true, "bold": true},
                            {"text": format!("nested {number}"), "level": 1, "bullet": true, "italic": true}
                        ],
                        "notes": "Generated speaker notes"
                    },
                    {
                        "id": "table",
                        "layout": "Title Only",
                        "title": "Generated table",
                        "tables": [{
                            "id": "table-one",
                            "rows": [["Label", "Value", "Active"], [body, number, true]],
                            "header": true,
                            "bandedRows": true,
                            "style": "Medium2",
                            "slot": "body"
                        }]
                    },
                    {
                        "id": "chart",
                        "layout": "Title Only",
                        "title": "Generated chart",
                        "charts": [{
                            "id": "chart-one",
                            "type": chart_type,
                            "title": "Values",
                            "categories": ["First", "Second"],
                            "series": [{"name": "Series", "values": [number, number + 1]}],
                            "style": "minimal",
                            "slot": "body"
                        }]
                    },
                    {
                        "id": "image",
                        "layout": "Title Only",
                        "title": "Generated image",
                        "images": [{
                            "id": "image-one",
                            "path": "asset.png",
                            "fit": fit,
                            "altText": body,
                            "slot": "body",
                            "keepOriginal": true
                        }]
                    },
                    {
                        "id": "textbox",
                        "layout": "Title Only",
                        "title": "Generated textbox",
                        "textBoxes": [{
                            "id": "textbox-one",
                            "slot": "body",
                            "paragraphs": [{
                                "runs": [
                                    {"text": body, "bold": true},
                                    {"text": " linked", "link": "https://example.test/property"}
                                ],
                                "level": 0,
                                "bullet": false,
                                "align": "center"
                            }]
                        }]
                    }
                ]
            })
        })
        .boxed()
}

fn xlsx_spec_strategy() -> BoxedStrategy<Value> {
    (
        text_strategy(),
        1i64..10_000,
        0usize..4,
        0usize..2,
        any::<bool>(),
    )
        .prop_map(|(text, number, chart_kind, style_kind, unsupported)| {
            let chart_type = if unsupported {
                "doughnut"
            } else {
                ["bar", "column", "line", "area"][chart_kind]
            };
            let table_style = ["TableStyleMedium2", "TableStyleMedium4"][style_kind];
            json!({
                "schemaVersion": 1,
                "family": "xlsx",
                "themeSeed": "2457A6",
                "metadata": {"title": "Generated workbook", "creator": "property test"},
                "sheets": [{
                    "id": "data",
                    "name": "Data",
                    "tabColor": "2457A6",
                    "freeze": "A2",
                    "headerStyle": "header",
                    "rows": [
                        ["Label", "Amount", "Ratio"],
                        [text, number, 0.25],
                        ["Second", number + 1, 0.5]
                    ],
                    "columns": [
                        {"name": "Label", "type": "text", "width": 18},
                        {"name": "Amount", "type": "currency", "format": "$#,##0.00", "width": 14},
                        {"name": "Ratio", "type": "percent", "format": "0.0%", "width": 12}
                    ],
                    "tables": [{
                        "id": "data-table",
                        "name": "DataTable",
                        "header": true,
                        "style": table_style,
                        "bandedRows": true
                    }],
                    "conditionalFormats": [{
                        "range": "C2:C3",
                        "type": "color-scale",
                        "cfvo": ["min", "percentile:50", "max"],
                        "color": ["F8696B", "FFEB84", "63BE7B"],
                        "priority": 1
                    }],
                    "dataValidations": [{
                        "range": "A2:A3",
                        "type": "list",
                        "listValues": "First,Second",
                        "allowBlank": false
                    }],
                    "names": [{"name": "AmountData", "range": "B2:B3"}],
                    "charts": [{
                        "id": "amount-chart",
                        "type": chart_type,
                        "title": "Amounts",
                        "source": {"path": "self", "sheet": "Data", "range": "A1:B3"},
                        "style": "default",
                        "options": {"anchor": "E2"}
                    }],
                    "hyperlinks": [{
                        "cell": "A5",
                        "url": "https://example.test/property",
                        "display": "Property link"
                    }],
                    "comments": [{"cell": "B1", "author": "Test", "text": text}],
                    "printSetup": {"landscape": true, "fitToWidth": 1, "gridlines": "off"}
                }]
            })
        })
        .boxed()
}

fn docx_spec_strategy() -> BoxedStrategy<Value> {
    (
        text_strategy(),
        text_strategy(),
        1u8..=4,
        0usize..3,
        any::<bool>(),
    )
        .prop_map(|(title, text, level, align_kind, unsupported)| {
            let heading_level = if unsupported { 5 } else { level };
            let align = ["left", "center", "right"][align_kind];
            json!({
                "schemaVersion": 1,
                "family": "docx",
                "title": title,
                "metadata": {"creator": "property test"},
                "headers": {"default": "Generated header"},
                "footers": {"default": "Generated footer", "pageNumbers": true},
                "blocks": [
                    {"id": "title", "type": "title", "text": title},
                    {"id": "heading", "type": "heading", "level": heading_level, "text": text},
                    {
                        "id": "paragraph",
                        "type": "paragraph",
                        "runs": [
                            {"text": text, "bold": true, "color": "2457A6"},
                            {"text": " italic", "italic": true},
                            {"text": " link", "link": "https://example.test/property"}
                        ],
                        "style": "Normal"
                    },
                    {"id": "bullet", "type": "bullet", "level": 1, "text": text},
                    {"id": "numbered", "type": "numbered", "level": 0, "text": "Numbered item"},
                    {
                        "id": "table",
                        "type": "table",
                        "table": {
                            "rows": [["Label", "Value"], [text, 42]],
                            "header": true,
                            "style": "TableGrid",
                            "columnWidths": ["2in", "1in"]
                        },
                        "caption": "Generated table"
                    },
                    {
                        "id": "image",
                        "type": "image",
                        "image": {
                            "path": "asset.png",
                            "width": "2in",
                            "height": "1in",
                            "fit": "contain",
                            "altText": text,
                            "align": align,
                            "keepOriginal": true
                        }
                    },
                    {"id": "break", "type": "pageBreak"}
                ]
            })
        })
        .boxed()
}

fn markdown_strategy(family: BuildFamily) -> BoxedStrategy<String> {
    (text_strategy(), text_strategy(), 1i64..10_000)
        .prop_map(move |(title, text, number)| match family {
            BuildFamily::Pptx => format!(
                "---\nthemeSeed: 2457A6\nsize: 16:9\nsplit: rule\n---\n# {title}\n{text}\n\n---\n# Body\n**{text}** and *italic* with [link](https://example.test/property).\n- Item {number}\n  * Nested item\n\n---\n# Table\n| Label | Value |\n| --- | ---: |\n| {text} | {number} |\n\n---\n# Chart\n```chart\n{{\"type\":\"bar\",\"categories\":[\"A\",\"B\"],\"series\":[{{\"name\":\"Value\",\"values\":[{number},{}]}}]}}\n```\n\n---\n# Image\n![Generated image](asset.png)\n",
                number + 1
            ),
            BuildFamily::Docx => format!(
                "---\ntitle: {title}\nauthor: Property Test\n---\n# {title}\n## Section\n{text} with **bold**, *italic*, `code`, and a [link](https://example.test/property).\n- Item {number}\n  * Nested item\n1. Numbered item\n\n| Label | Value |\n| --- | ---: |\n| {text} | {number} |\n\n![Generated image](asset.png) {{width=2in}}\n\n***\n\n```text\nvalue = {number}\n```\n"
            ),
            BuildFamily::Xlsx => unreachable!("Markdown supports PPTX and DOCX only"),
        })
        .boxed()
}

#[test]
fn generated_pptx_specs_never_emit_invalid_xml() {
    run_spec_property(BuildFamily::Pptx, pptx_spec_strategy());
}

#[test]
fn generated_xlsx_specs_never_emit_invalid_xml() {
    run_spec_property(BuildFamily::Xlsx, xlsx_spec_strategy());
}

#[test]
fn generated_docx_specs_never_emit_invalid_xml() {
    run_spec_property(BuildFamily::Docx, docx_spec_strategy());
}

#[test]
fn generated_pptx_markdown_never_emits_invalid_xml() {
    run_markdown_property(BuildFamily::Pptx);
}

#[test]
fn generated_docx_markdown_never_emits_invalid_xml() {
    run_markdown_property(BuildFamily::Docx);
}

fn run_spec_property(family: BuildFamily, strategy: BoxedStrategy<Value>) {
    let required = schema_document(family)["required"]
        .as_array()
        .expect("published schema required fields")
        .clone();
    let mut runner = runner();
    runner
        .run(&strategy, |spec| {
            for field in &required {
                let field = field.as_str().expect("required field name");
                prop_assert!(spec.get(field).is_some(), "generated spec omitted {field}");
            }
            prove_generated_spec(family, &spec).map_err(TestCaseError::fail)
        })
        .unwrap_or_else(|error| panic!("{family} build-spec property failed: {error}"));
}

fn run_markdown_property(family: BuildFamily) {
    let strategy = markdown_strategy(family);
    let mut runner = runner();
    runner
        .run(&strategy, |source| {
            let conversion =
                markdown_to_spec(family, &source, "generated.md").map_err(|error| {
                    TestCaseError::fail(format!(
                        "{family} Markdown conversion failed: {error}\nsource:\n{source}"
                    ))
                })?;
            prove_generated_spec(family, &conversion.spec).map_err(|error| {
                TestCaseError::fail(format!(
                    "{error}\nMarkdown source:\n{source}\nintermediate spec:\n{}",
                    serde_json::to_string_pretty(&conversion.spec).unwrap()
                ))
            })
        })
        .unwrap_or_else(|error| panic!("{family} Markdown property failed: {error}"));
}

fn prove_generated_spec(family: BuildFamily, document: &Value) -> Result<(), String> {
    let serialized = serde_json::to_string_pretty(document).unwrap();
    let spec = load_spec_str(family, &serialized).map_err(|error| {
        format!("published schema rejected generated spec: {error}\n{serialized}")
    })?;
    if let Err(error) = compile(family, &spec) {
        if error.code.starts_with("BUILD_SPEC_")
            && !error.path.is_empty()
            && !error.message.is_empty()
        {
            return Ok(());
        }
        return Err(format!(
            "compiler refusal is not a teaching error: {error}\n{serialized}"
        ));
    }

    let temp = case_dir(family);
    let result = prove_built_package(family, document, &serialized, &temp);
    let _ = fs::remove_dir_all(&temp);
    result
}

fn compile(
    family: BuildFamily,
    spec: &BuildSpec,
) -> Result<(), ooxml_cli::build::BuildCompileError> {
    match family {
        BuildFamily::Pptx => compile_pptx_spec(spec).map(|_| ()),
        BuildFamily::Xlsx => compile_xlsx_spec(spec).map(|_| ()),
        BuildFamily::Docx => compile_docx_spec(spec).map(|_| ()),
    }
}

fn case_dir(family: BuildFamily) -> PathBuf {
    let nonce = NEXT_CASE.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "ooxml-build-property-{family}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&path).expect("create property case directory");
    path
}

fn prove_built_package(
    family: BuildFamily,
    document: &Value,
    serialized: &str,
    temp: &Path,
) -> Result<(), String> {
    let spec_path = temp.join("spec.json");
    let output_path = temp.join(format!("output.{}", family.as_str()));
    fs::write(&spec_path, serde_json::to_vec_pretty(document).unwrap())
        .map_err(|error| error.to_string())?;
    fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata/test_image.png"),
        temp.join("asset.png"),
    )
    .map_err(|error| format!("copy image asset: {error}"))?;

    let build = run(&[
        "--json".to_string(),
        family.as_str().to_string(),
        "build".to_string(),
        "--spec".to_string(),
        path(&spec_path),
        "--out".to_string(),
        path(&output_path),
    ]);
    if !build.status.success() {
        return Err(command_failure("build", &build, serialized));
    }
    let report: Value = serde_json::from_slice(&build.stdout)
        .map_err(|error| format!("build returned invalid JSON: {error}\n{serialized}"))?;
    if report["validated"] != true {
        return Err(format!(
            "build did not validate its staged output: {report}\n{serialized}"
        ));
    }

    assert_every_xml_part_well_formed(&output_path)
        .map_err(|error| format!("{error}\n{serialized}"))?;
    assert_strict_valid(&output_path).map_err(|error| format!("{error}\n{serialized}"))?;
    assert_sdk_valid_if_available(&output_path)
        .map_err(|error| format!("{error}\n{serialized}"))?;
    Ok(())
}

fn run(args: &[String]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args(args)
        .env("SOURCE_DATE_EPOCH", SOURCE_DATE_EPOCH)
        .output()
        .expect("run ooxml")
}

fn path(path: &Path) -> String {
    path.to_str().expect("UTF-8 test path").to_string()
}

fn command_failure(action: &str, output: &Output, specimen: &str) -> String {
    format!(
        "{action} failed\nstdout: {}\nstderr: {}\nspecimen:\n{specimen}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn assert_every_xml_part_well_formed(package: &Path) -> Result<(), String> {
    let file = fs::File::open(package).map_err(|error| error.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|error| error.to_string())?;
    let mut xml_parts = 0usize;
    for index in 0..archive.len() {
        let mut part = archive.by_index(index).map_err(|error| error.to_string())?;
        let name = part.name().to_string();
        if !(name.ends_with(".xml") || name.ends_with(".rels")) {
            continue;
        }
        xml_parts += 1;
        let mut bytes = Vec::new();
        part.read_to_end(&mut bytes)
            .map_err(|error| format!("read {name}: {error}"))?;
        let mut reader = Reader::from_reader(bytes.as_slice());
        let mut buffer = Vec::new();
        loop {
            match reader.read_event_into(&mut buffer) {
                Ok(Event::Eof) => break,
                Ok(_) => buffer.clear(),
                Err(error) => {
                    return Err(format!(
                        "{} contains malformed XML at byte {}: {error}",
                        name,
                        reader.buffer_position()
                    ));
                }
            }
        }
    }
    if xml_parts == 0 {
        return Err(format!("{} contains no XML parts", package.display()));
    }
    Ok(())
}

fn assert_strict_valid(package: &Path) -> Result<(), String> {
    let output = run(&[
        "--json".to_string(),
        "validate".to_string(),
        "--strict".to_string(),
        path(package),
    ]);
    if output.status.success() {
        Ok(())
    } else {
        Err(command_failure("strict validation", &output, ""))
    }
}

fn assert_sdk_valid_if_available(package: &Path) -> Result<(), String> {
    let required = std::env::var("OOXML_REQUIRE_OPENXML_SDK").as_deref() == Ok("1");
    let dotnet = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default()
        .join("dotnet/dotnet");
    let relative = "tools/openxml-validator/bin/Release/net8.0/openxml-validator.dll";
    let local = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    let shared = Path::new("/home/oliver/Projects/odcpw/ooxml-cli").join(relative);
    let validator = [local, shared]
        .into_iter()
        .find(|candidate| candidate.is_file());
    let Some(validator) = validator else {
        return if required {
            Err("Open XML SDK validator is required but unavailable".to_string())
        } else {
            Ok(())
        };
    };
    if !dotnet.is_file() {
        return if required {
            Err(format!(
                "Open XML SDK runtime {} is unavailable",
                dotnet.display()
            ))
        } else {
            Ok(())
        };
    }
    let output = Command::new(dotnet)
        .arg(validator)
        .arg(package)
        .output()
        .map_err(|error| format!("run Open XML SDK validator: {error}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if output.status.success() && stdout.contains("0 errors") {
        Ok(())
    } else {
        Err(format!(
            "Open XML SDK rejected {}\nstdout: {stdout}\nstderr: {}",
            package.display(),
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

#[test]
fn injected_failure_shrinks_to_the_smallest_counterexample() {
    let mut runner = TestRunner::new(Config {
        cases: 8,
        failure_persistence: None,
        ..Config::default()
    });
    let error = runner
        .run(&(4u8..64), |value| {
            prop_assert!(value < 4, "injected writer bug at {value}");
            Ok(())
        })
        .expect_err("injected bug must fail");
    match error {
        TestError::Fail(_, smallest) => assert_eq!(smallest, 4),
        other => panic!("unexpected shrink result: {other}"),
    }
}
