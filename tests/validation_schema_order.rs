use serde_json::Value;
use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

const EXPECTED_INVALID: [(&str, &str); 7] = [
    (
        "testdata/docx/corrupted-missing-document/document.docx",
        "DOCX_MISSING_DOCUMENT",
    ),
    (
        "testdata/docx/scaffold-styles/dangling-style.docx",
        "DOCX_DANGLING_STYLE",
    ),
    (
        "testdata/pptx/animations-stale-media/presentation.pptx",
        "PPTX_MISSING_SLIDE_RELATIONSHIP",
    ),
    (
        "testdata/pptx/corrupted-dangling-layout/presentation.pptx",
        "REL_DANGLING_TARGET",
    ),
    (
        "testdata/pptx/corrupted-missing-media/presentation.pptx",
        "PPTX_MISSING_MEDIA",
    ),
    (
        "testdata/xlsx/corrupted-missing-worksheet/workbook.xlsx",
        "XLSX_MISSING_WORKSHEET",
    ),
    (
        "testdata/xlsx/invalid/pivot-table-parts.xlsx",
        "XML_UNKNOWN_CHILD",
    ),
];

#[derive(Default)]
struct CoveredPartCounts {
    spreadsheet: usize,
    presentation: usize,
    wordprocessing: usize,
    charts: usize,
}

impl CoveredPartCounts {
    fn total(&self) -> usize {
        self.spreadsheet + self.presentation + self.wordprocessing + self.charts
    }

    fn add_to(&self, total: &mut Self) {
        total.spreadsheet += self.spreadsheet;
        total.presentation += self.presentation;
        total.wordprocessing += self.wordprocessing;
        total.charts += self.charts;
    }
}

#[test]
fn committed_package_sweep_has_no_unexpected_strict_failures() {
    let packages = office_packages();
    let mut passed = 0usize;
    let mut expected_failures = 0usize;
    let mut covered_totals = CoveredPartCounts::default();

    for package in &packages {
        let relative = relative_path(package);
        let counts = covered_part_counts(package);
        counts.add_to(&mut covered_totals);
        let (output, report) = run_ooxml(&["--json", "validate", "--strict", path_str(package)]);
        let schema_diagnostics = schema_order_diagnostics(&report);
        println!(
            "fixture={relative} coveredParts={} spreadsheet={} presentation={} wordprocessing={} charts={} exit={} schemaDiagnostics={}",
            counts.total(),
            counts.spreadsheet,
            counts.presentation,
            counts.wordprocessing,
            counts.charts,
            output.status.code().unwrap_or(-1),
            schema_diagnostics.len(),
        );
        for diagnostic in &schema_diagnostics {
            println!("  schemaDiagnostic={diagnostic}");
        }

        if let Some(expected_code) = expected_failure_code(&relative) {
            expected_failures += 1;
            assert_eq!(
                output.status.code(),
                Some(5),
                "expected-invalid fixture unexpectedly passed: {relative}: {report}"
            );
            assert!(
                contains_diagnostic_code(&report, expected_code),
                "{relative} did not report {expected_code}: {report}"
            );
        } else {
            passed += 1;
            assert!(
                output.status.success(),
                "committed calibration fixture failed strict validation: {relative}: {report}"
            );
            assert_eq!(report["status"], "valid", "fixture: {relative}");
        }
    }

    println!(
        "schema-order calibration packages={} passed={} expectedInvalid={} covered spreadsheet={} presentation={} wordprocessing={} charts={} total={}",
        packages.len(),
        passed,
        expected_failures,
        covered_totals.spreadsheet,
        covered_totals.presentation,
        covered_totals.wordprocessing,
        covered_totals.charts,
        covered_totals.total(),
    );
    assert_eq!(
        expected_failures,
        EXPECTED_INVALID.len(),
        "the sweep must exercise every documented invalid fixture"
    );
    assert_eq!(passed + expected_failures, packages.len());
    assert!(covered_totals.spreadsheet > 0);
    assert!(covered_totals.presentation > 0);
    assert!(covered_totals.wordprocessing > 0);
    assert!(covered_totals.charts > 0);
}

#[test]
fn pivot_regression_is_precise_and_fixed_writer_output_is_schema_clean() {
    let fixture = repo_path("testdata/xlsx/invalid/pivot-table-parts.xlsx");
    let (invalid_output, invalid_report) =
        run_ooxml(&["--json", "validate", "--strict", path_str(&fixture)]);
    assert_eq!(invalid_output.status.code(), Some(5));
    let diagnostic = schema_order_diagnostics(&invalid_report)
        .into_iter()
        .find(|diagnostic| diagnostic["element"] == "pivotTableParts")
        .expect("pivotTableParts schema diagnostic");
    assert_eq!(diagnostic["code"], "XML_UNKNOWN_CHILD");
    assert_eq!(diagnostic["part"], "/xl/worksheets/sheet1.xml");
    assert_eq!(diagnostic["xpath"], "/x:worksheet[1]/x:pivotTableParts[1]");
    assert_eq!(diagnostic["position"], 3);
    assert_eq!(diagnostic["expectedPosition"], 7);

    let temp_dir = temp_dir("fixed-pivot-writer");
    let seed = temp_dir.join("seed.xlsx");
    let data = temp_dir.join("data.xlsx");
    let table = temp_dir.join("table.xlsx");
    let pivot = temp_dir.join("pivot.xlsx");

    run_ooxml_ok(&["--json", "xlsx", "scaffold", path_str(&seed), "--force"]);
    assert_strict_valid(&seed);
    run_ooxml_ok(&[
        "--json",
        "xlsx",
        "ranges",
        "set",
        path_str(&seed),
        "--sheet",
        "Sheet1",
        "--range",
        "A1:B3",
        "--values",
        r#"[["Region","Amount"],["East",10],["West",20]]"#,
        "--data-format",
        "json",
        "--out",
        path_str(&data),
    ]);
    assert_strict_valid(&data);
    run_ooxml_ok(&[
        "--json",
        "xlsx",
        "tables",
        "create",
        path_str(&data),
        "--sheet",
        "Sheet1",
        "--range",
        "A1:B3",
        "--table",
        "Sales",
        "--out",
        path_str(&table),
    ]);
    assert_strict_valid(&table);
    let worksheet_before = zip_entry_text(&table, "xl/worksheets/sheet1.xml");
    assert!(worksheet_before.contains("<tableParts"));

    run_ooxml_ok(&[
        "--json",
        "xlsx",
        "pivots",
        "create",
        path_str(&table),
        "--table",
        "Sales",
        "--name",
        "SalesPivot",
        "--rows",
        "Region",
        "--values",
        "Amount:sum",
        "--anchor",
        "D1",
        "--out",
        path_str(&pivot),
    ]);
    assert_strict_valid(&pivot);
    let worksheet_after = zip_entry_text(&pivot, "xl/worksheets/sheet1.xml");
    assert_eq!(
        worksheet_after, worksheet_before,
        "relationship-only pivot creation must leave worksheet XML and tableParts untouched"
    );
    assert!(!worksheet_after.contains("pivotTableParts"));

    if let Some((dotnet, validator)) = sdk_validator() {
        let (sdk_output, sdk_report) = run_sdk(&dotnet, &validator, &pivot);
        assert!(
            sdk_output.status.success(),
            "fixed writer output failed SDK validation: {sdk_report}"
        );
        assert_eq!(sdk_report["Valid"], true);
        assert_eq!(sdk_report["ErrorCount"], 0);
    } else {
        println!(
            "SKIP Open XML SDK proof for fixed pivot writer: ~/dotnet/dotnet or validator DLL is unavailable"
        );
    }

    fs::remove_dir_all(&temp_dir).expect("remove fixed-pivot test directory");
}

#[test]
fn sdk_child_order_findings_are_covered_when_validator_is_available() {
    let Some((dotnet, validator)) = sdk_validator() else {
        println!(
            "SKIP SDK child-order calibration: ~/dotnet/dotnet or tools/openxml-validator/bin/Release/net8.0/openxml-validator.dll is unavailable"
        );
        return;
    };

    let packages = office_packages();
    let mut sdk_order_findings = 0usize;
    let mut validator_order_findings = 0usize;
    let mut missing = Vec::new();
    for package in &packages {
        let relative = relative_path(package);
        let (_local_output, local_report) =
            run_ooxml(&["--json", "validate", "--strict", path_str(package)]);
        let local = schema_order_diagnostics(&local_report)
            .into_iter()
            .filter_map(|diagnostic| {
                Some((
                    diagnostic.get("part")?.as_str()?.to_string(),
                    diagnostic.get("element")?.as_str()?.to_string(),
                ))
            })
            .collect::<BTreeSet<_>>();
        validator_order_findings += local.len();

        let (_sdk_output, sdk_report) = run_sdk(&dotnet, &validator, package);
        let sdk = sdk_covered_child_order_findings(&sdk_report);
        sdk_order_findings += sdk.len();
        println!(
            "sdkCalibration fixture={relative} sdkCoveredOrderErrors={} validatorOrderErrors={}",
            sdk.len(),
            local.len(),
        );
        for finding in &sdk {
            println!("  sdkOrderFinding part={} element={}", finding.0, finding.1);
            if !local.contains(finding) {
                missing.push(format!(
                    "fixture={relative} part={} element={} local={local:?} sdk={sdk_report}",
                    finding.0, finding.1
                ));
            }
        }
    }
    println!(
        "sdk child-order calibration packages={} sdkCoveredOrderErrors={} validatorOrderErrors={}",
        packages.len(),
        sdk_order_findings,
        validator_order_findings
    );
    assert!(
        missing.is_empty(),
        "SDK covered child-order findings missing locally:\n{}",
        missing.join("\n")
    );
}

#[test]
fn multiple_schema_diagnostics_are_sorted_and_byte_stable() {
    let temp_dir = temp_dir("diagnostic-determinism");
    let source = repo_path("testdata/xlsx/minimal-workbook/workbook.xlsx");
    let invalid = temp_dir.join("two-schema-errors.xlsx");
    let worksheet = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData/>
  <notInTheSchema/>
  <dimension ref="A1"/>
</worksheet>"#;
    rewrite_zip_entry(&source, &invalid, "xl/worksheets/sheet1.xml", worksheet);

    let (first_output, first) = run_ooxml(&["--json", "validate", "--strict", path_str(&invalid)]);
    let (second_output, second) =
        run_ooxml(&["--json", "validate", "--strict", path_str(&invalid)]);
    assert_eq!(first_output.status.code(), Some(5));
    assert_eq!(second_output.status.code(), Some(5));
    assert_eq!(
        first_output.stdout, second_output.stdout,
        "JSON bytes changed"
    );
    assert_eq!(first["diagnostics"], second["diagnostics"]);

    let diagnostics = schema_order_diagnostics(&first);
    assert_eq!(diagnostics.len(), 2, "report: {first}");
    assert!(contains_diagnostic_code(&first, "XML_CHILD_ORDER"));
    assert!(contains_diagnostic_code(&first, "XML_UNKNOWN_CHILD"));
    let keys = diagnostics
        .iter()
        .map(|diagnostic| {
            (
                diagnostic["part"].as_str().unwrap_or_default(),
                diagnostic["xpath"].as_str().unwrap_or_default(),
                diagnostic["code"].as_str().unwrap_or_default(),
            )
        })
        .collect::<Vec<_>>();
    assert!(keys.windows(2).all(|pair| pair[0] <= pair[1]), "{keys:?}");

    fs::remove_dir_all(&temp_dir).expect("remove determinism test directory");
}

fn office_packages() -> Vec<PathBuf> {
    fn visit(directory: &Path, packages: &mut Vec<PathBuf>) {
        let mut entries = fs::read_dir(directory)
            .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()))
            .map(|entry| entry.expect("read testdata entry").path())
            .collect::<Vec<_>>();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                visit(&path, packages);
            } else if path
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|extension| {
                    matches!(
                        extension.to_ascii_lowercase().as_str(),
                        "xlsx" | "xlsm" | "pptx" | "pptm" | "docx" | "docm"
                    )
                })
            {
                packages.push(path);
            }
        }
    }

    let mut packages = Vec::new();
    visit(Path::new("testdata"), &mut packages);
    packages.sort();
    packages
}

fn expected_failure_code(relative: &str) -> Option<&'static str> {
    EXPECTED_INVALID
        .iter()
        .find_map(|(path, code)| (*path == relative).then_some(*code))
}

fn covered_part_counts(package: &Path) -> CoveredPartCounts {
    let file = File::open(package).expect("open package for part counts");
    let mut archive = ZipArchive::new(file).expect("open package ZIP for part counts");
    let mut counts = CoveredPartCounts::default();
    for index in 0..archive.len() {
        let name = archive
            .by_index(index)
            .expect("read ZIP entry for part counts")
            .name()
            .to_string();
        if is_chart_part(&name) {
            counts.charts += 1;
        } else if name == "xl/workbook.xml"
            || name.starts_with("xl/worksheets/") && name.ends_with(".xml")
            || name.starts_with("xl/tables/") && name.ends_with(".xml")
            || name.starts_with("xl/pivotTables/") && name.ends_with(".xml")
        {
            counts.spreadsheet += 1;
        } else if is_presentation_part(&name) {
            counts.presentation += 1;
        } else if name == "word/document.xml" {
            counts.wordprocessing += 1;
        }
    }
    counts
}

fn is_chart_part(name: &str) -> bool {
    let in_directory = name.starts_with("xl/charts/")
        || name.starts_with("ppt/charts/")
        || name.starts_with("word/charts/");
    in_directory && numbered_xml_name(name, "chart")
}

fn is_presentation_part(name: &str) -> bool {
    (name.starts_with("ppt/slides/") && numbered_xml_name(name, "slide"))
        || (name.starts_with("ppt/slideLayouts/") && numbered_xml_name(name, "slideLayout"))
        || (name.starts_with("ppt/slideMasters/") && numbered_xml_name(name, "slideMaster"))
}

fn numbered_xml_name(path: &str, prefix: &str) -> bool {
    path.rsplit('/')
        .next()
        .and_then(|name| name.strip_prefix(prefix))
        .and_then(|name| name.strip_suffix(".xml"))
        .is_some_and(|number| {
            !number.is_empty() && number.bytes().all(|byte| byte.is_ascii_digit())
        })
}

fn sdk_covered_child_order_findings(report: &Value) -> BTreeSet<(String, String)> {
    const COVERED_PARENTS: &[&str] = &[
        "worksheet",
        "workbook",
        "table",
        "pivotTableDefinition",
        "sld",
        "sldLayout",
        "sldMaster",
        "cSld",
        "sp",
        "graphicFrame",
        "document",
        "body",
        "p",
        "r",
        "tbl",
        "chartSpace",
    ];
    report["Errors"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|error| error["ErrorType"] == "Schema")
        .filter(|error| {
            error["Node"]
                .as_str()
                .is_some_and(|node| COVERED_PARENTS.contains(&node))
        })
        .filter_map(|error| {
            let description = error["Description"].as_str()?;
            if !description.contains("invalid child element")
                && !description.contains("unexpected child element")
            {
                return None;
            }
            let qualified = description.split('\'').nth(1)?;
            let local = qualified.rsplit(':').next()?.to_string();
            Some((error["Part"].as_str()?.to_string(), local))
        })
        .collect()
}

fn schema_order_diagnostics(report: &Value) -> Vec<&Value> {
    report["diagnostics"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|diagnostic| {
            matches!(
                diagnostic["code"].as_str(),
                Some("XML_CHILD_ORDER" | "XML_UNKNOWN_CHILD")
            )
        })
        .collect()
}

fn contains_diagnostic_code(value: &Value, code: &str) -> bool {
    value["diagnostics"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|diagnostic| diagnostic["code"] == code)
}

fn run_ooxml(args: &[&str]) -> (Output, Value) {
    let output = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args(args)
        .output()
        .expect("run ooxml");
    assert!(
        output.stderr.is_empty(),
        "ooxml stderr for {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report = serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "parse ooxml JSON for {args:?}: {error}: {}",
            String::from_utf8_lossy(&output.stdout)
        )
    });
    (output, report)
}

fn run_ooxml_ok(args: &[&str]) -> Value {
    let (output, report) = run_ooxml(args);
    assert!(
        output.status.success(),
        "ooxml failed for {args:?}: {report}"
    );
    report
}

fn assert_strict_valid(package: &Path) {
    let (output, report) = run_ooxml(&["--json", "validate", "--strict", path_str(package)]);
    assert!(
        output.status.success(),
        "generated package failed strict validation: {}: {report}",
        package.display()
    );
    assert_eq!(report["status"], "valid");
}

fn sdk_validator() -> Option<(PathBuf, PathBuf)> {
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    let dotnet = home.join("dotnet/dotnet");
    let validator = repo_path("tools/openxml-validator/bin/Release/net8.0/openxml-validator.dll");
    (dotnet.is_file() && validator.is_file()).then_some((dotnet, validator))
}

fn run_sdk(dotnet: &Path, validator: &Path, package: &Path) -> (Output, Value) {
    let output = Command::new(dotnet)
        .args([
            validator.as_os_str(),
            "--json".as_ref(),
            package.as_os_str(),
        ])
        .output()
        .expect("run Open XML SDK validator");
    assert!(
        output.stderr.is_empty(),
        "Open XML SDK stderr for {}: {}",
        package.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    let report = serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "parse Open XML SDK JSON for {}: {error}: {}",
            package.display(),
            String::from_utf8_lossy(&output.stdout)
        )
    });
    (output, report)
}

fn zip_entry_text(package: &Path, entry: &str) -> String {
    let file = File::open(package).expect("open ZIP package");
    let mut archive = ZipArchive::new(file).expect("read ZIP package");
    let mut text = String::new();
    archive
        .by_name(entry)
        .unwrap_or_else(|error| panic!("read {entry}: {error}"))
        .read_to_string(&mut text)
        .unwrap_or_else(|error| panic!("decode {entry}: {error}"));
    text
}

fn rewrite_zip_entry(source: &Path, destination: &Path, target: &str, replacement: &[u8]) {
    let input = File::open(source).expect("open source ZIP");
    let mut archive = ZipArchive::new(input).expect("read source ZIP");
    let output = File::create(destination).expect("create rewritten ZIP");
    let mut writer = ZipWriter::new(output);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let mut replaced = false;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).expect("read source ZIP entry");
        let name = entry.name().to_string();
        let mut data = Vec::new();
        entry.read_to_end(&mut data).expect("read source ZIP bytes");
        writer.start_file(&name, options).expect("start ZIP entry");
        if name == target {
            writer.write_all(replacement).expect("write replacement");
            replaced = true;
        } else {
            writer.write_all(&data).expect("write original entry");
        }
    }
    writer.finish().expect("finish rewritten ZIP");
    assert!(replaced, "target ZIP entry not found: {target}");
}

fn temp_dir(label: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "ooxml-validation-schema-order-{label}-{}",
        std::process::id()
    ));
    if directory.exists() {
        fs::remove_dir_all(&directory).expect("remove stale test directory");
    }
    fs::create_dir_all(&directory).expect("create test directory");
    directory
}

fn repo_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
}

fn relative_path(path: &Path) -> String {
    path.strip_prefix(env!("CARGO_MANIFEST_DIR"))
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn path_str(path: &Path) -> &str {
    path.to_str().expect("UTF-8 test path")
}
