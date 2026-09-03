use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const PPTX_CODES: [&str; 10] = [
    "PPTX_TEXT_CONTRAST",
    "PPTX_FONT_TOO_SMALL",
    "PPTX_BULLET_OVERLOAD",
    "PPTX_FONT_OUTSIDE_THEME",
    "PPTX_EMPTY_PLACEHOLDER",
    "PPTX_IMAGE_SCALE",
    "PPTX_MISSING_TITLE",
    "PPTX_MISSING_ALT_TEXT",
    "PPTX_OUTSIDE_SAFE_MARGIN",
    "PPTX_INCONSISTENT_TITLE_POSITION",
];

#[test]
fn rule_catalog_is_reviewable_complete_and_stable() {
    let report = run_ok(&["--json", "design-check", "--rules"]);
    assert_eq!(report["schemaVersion"], "ooxml-cli.design-check.v1");
    let rules = report["rules"].as_array().expect("rules array");
    assert_eq!(rules.len(), 25);
    let codes = rules
        .iter()
        .map(|rule| rule["code"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(codes.len(), rules.len());
    for family in ["pptx", "docx", "xlsx"] {
        assert!(rules.iter().any(|rule| rule["family"] == family));
    }
    assert!(rules.iter().all(|rule| {
        rule["description"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
            && matches!(
                rule["severity"].as_str(),
                Some("error" | "warning" | "info")
            )
    }));
}

#[test]
fn scaffold_recipes_have_zero_design_errors() {
    let temp = temp_dir("recipes");
    let pptx = temp.join("recipe.pptx");
    let docx = temp.join("recipe.docx");
    let xlsx = temp.join("recipe.xlsx");
    run_ok(&[
        "--json",
        "pptx",
        "scaffold",
        path(&pptx),
        "--title",
        "Quarterly review",
        "--subtitle",
        "Prepared for the team",
    ]);
    run_ok(&[
        "--json",
        "docx",
        "scaffold",
        path(&docx),
        "--text",
        "Design review",
    ]);
    run_ok(&[
        "--json",
        "xlsx",
        "scaffold",
        path(&xlsx),
        "--sheet",
        "Summary",
    ]);
    for package in [&pptx, &docx, &xlsx] {
        let report = design_check(package);
        assert_eq!(report["summary"]["errors"], 0, "{report}");
        assert_package_proofs(package);
    }
    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn committed_bad_deck_triggers_every_pptx_rule_exactly_once_with_fixes() {
    let fixture = fixture("testdata/design-check/bad-deck/presentation.pptx");
    let report = design_check(&fixture);
    assert_eq!(
        report,
        design_check(&fixture),
        "design-check output must be deterministic"
    );
    assert_eq!(report["family"], "pptx");
    let mut counts = BTreeMap::<&str, usize>::new();
    for finding in report["findings"].as_array().expect("findings") {
        *counts.entry(finding["code"].as_str().unwrap()).or_default() += 1;
        assert!(finding["location"].is_object(), "{finding}");
        assert!(
            finding["fixCommand"]
                .as_str()
                .is_some_and(|command| command.starts_with("ooxml --json ")),
            "{finding}"
        );
    }
    assert_eq!(
        counts,
        PPTX_CODES.into_iter().map(|code| (code, 1)).collect()
    );
    assert_package_proofs(&fixture);
    assert_libreoffice_renders(&fixture, &temp_dir("bad-deck-render"));
}

#[test]
fn ignore_and_adjacent_config_override_findings_without_changing_rules() {
    let temp = temp_dir("config");
    let deck = temp.join("presentation.pptx");
    fs::copy(
        fixture("testdata/design-check/bad-deck/presentation.pptx"),
        &deck,
    )
    .unwrap();
    fs::write(
        temp.join(".ooxml-design.json"),
        serde_json::to_vec_pretty(&json!({
            "ignore": ["PPTX_MISSING_TITLE"],
            "severity": {"PPTX_TEXT_CONTRAST": "info"},
            "thresholds": {"pptx.minimumFontPoints": 11.0}
        }))
        .unwrap(),
    )
    .unwrap();
    let report = run_ok(&[
        "--json",
        "design-check",
        path(&deck),
        "--ignore=PPTX_EMPTY_PLACEHOLDER",
    ]);
    let findings = report["findings"].as_array().unwrap();
    assert!(!has_code(findings, "PPTX_MISSING_TITLE"));
    assert!(!has_code(findings, "PPTX_EMPTY_PLACEHOLDER"));
    assert!(!has_code(findings, "PPTX_FONT_TOO_SMALL"));
    assert_eq!(
        findings
            .iter()
            .find(|finding| finding["code"] == "PPTX_TEXT_CONTRAST")
            .unwrap()["severity"],
        "info"
    );
    assert!(
        report["config"]
            .as_str()
            .unwrap()
            .ends_with(".ooxml-design.json")
    );
    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn three_pptx_fixes_remove_their_rule_and_render_before_and_after() {
    let scenarios: [(&str, &[&str]); 3] = [
        (
            "PPTX_FONT_TOO_SMALL",
            &[
                "--json", "pptx", "shapes", "delete", "{file}", "--slide", "3", "--target",
                "shape:5", "--out", "{out}",
            ],
        ),
        (
            "PPTX_EMPTY_PLACEHOLDER",
            &[
                "--json", "pptx", "shapes", "delete", "{file}", "--slide", "3", "--target",
                "subtitle", "--out", "{out}",
            ],
        ),
        (
            "PPTX_OUTSIDE_SAFE_MARGIN",
            &[
                "--json",
                "pptx",
                "shapes",
                "set-bounds",
                "{file}",
                "--slide",
                "3",
                "--target",
                "title",
                "--bounds",
                "1097280,1851660,9997440,960120",
                "--out",
                "{out}",
            ],
        ),
    ];
    for (code, command) in scenarios {
        let temp = temp_dir(&code.to_ascii_lowercase());
        let before = temp.join("before.pptx");
        let after = temp.join("after.pptx");
        fs::copy(
            fixture("testdata/design-check/bad-deck/presentation.pptx"),
            &before,
        )
        .unwrap();
        assert!(has_code(
            design_check(&before)["findings"].as_array().unwrap(),
            code
        ));
        assert_libreoffice_renders(&before, &temp.join("before-render"));
        let owned = command
            .iter()
            .map(|arg| match *arg {
                "{file}" => path(&before).to_string(),
                "{out}" => path(&after).to_string(),
                value => value.to_string(),
            })
            .collect::<Vec<_>>();
        run_owned_ok(&owned);
        assert!(!has_code(
            design_check(&after)["findings"].as_array().unwrap(),
            code
        ));
        assert_package_proofs(&after);
        assert_libreoffice_renders(&after, &temp.join("after-render"));
        fs::remove_dir_all(temp).unwrap();
    }
}

#[test]
fn docx_rules_reuse_style_integrity_and_report_image_accessibility() {
    let dangling = design_check(&fixture(
        "testdata/docx/scaffold-styles/dangling-style.docx",
    ));
    assert_eq!(dangling["findings"][0]["code"], "DOCX_DANGLING_STYLE");
    assert!(
        dangling["findings"][0]["fixCommand"]
            .as_str()
            .unwrap()
            .contains("docx styles apply")
    );
    let image = design_check(&fixture("testdata/docx/with-image/document.docx"));
    assert!(has_code(
        image["findings"].as_array().unwrap(),
        "DOCX_MISSING_ALT_TEXT"
    ));
}

#[test]
fn xlsx_rule_set_uses_real_cells_styles_charts_tables_and_freeze_state() {
    let temp = temp_dir("xlsx");
    let workbook = temp.join("bad.xlsx");
    run_ok(&[
        "--json",
        "xlsx",
        "scaffold",
        path(&workbook),
        "--sheet",
        "Data",
        "--sheet",
        "Extra",
    ]);
    for (cell, value, value_type) in [
        ("A1", "Header", "string"),
        ("A2", "123456789", "number"),
        ("A3", "12.34", "number"),
        ("B1", "Series", "string"),
        ("B2", "10", "number"),
        ("B3", "20", "number"),
    ] {
        run_ok(&[
            "--json",
            "xlsx",
            "cells",
            "set",
            path(&workbook),
            "--sheet",
            "Data",
            "--cell",
            cell,
            "--value",
            value,
            "--type",
            value_type,
            "--in-place",
        ]);
    }
    run_ok(&[
        "--json",
        "xlsx",
        "ranges",
        "set-format",
        path(&workbook),
        "--sheet",
        "Data",
        "--range",
        "A2",
        "--preset",
        "currency",
        "--in-place",
    ]);
    run_ok(&[
        "--json",
        "xlsx",
        "ranges",
        "set-format",
        path(&workbook),
        "--sheet",
        "Data",
        "--range",
        "A3",
        "--preset",
        "percent",
        "--in-place",
    ]);
    for (cell, font) in [("A2", "Arial"), ("A3", "Comic Sans MS")] {
        run_ok(&[
            "--json",
            "xlsx",
            "ranges",
            "set-style",
            path(&workbook),
            "--sheet",
            "Data",
            "--range",
            cell,
            "--font-name",
            font,
            "--in-place",
        ]);
    }
    run_ok(&[
        "--json",
        "xlsx",
        "charts",
        "create",
        path(&workbook),
        "--sheet",
        "Data",
        "--type",
        "bar",
        "--range",
        "A1:B3",
        "--anchor",
        "D2",
        "--in-place",
    ]);
    fs::write(
        temp.join(".ooxml-design.json"),
        br#"{"thresholds":{"xlsx.averageCharacterWidth":100,"xlsx.freezeHeaderMinimumRows":0,"xlsx.tableMinimumRows":0,"xlsx.maximumReadableTabs":0}}"#,
    )
    .unwrap();
    let report = design_check(&workbook);
    let codes = report["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|finding| finding["code"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        codes,
        BTreeSet::from([
            "XLSX_NUMBER_CLIPPED",
            "XLSX_HEADER_NOT_FROZEN",
            "XLSX_INCONSISTENT_NUMBER_FORMAT",
            "XLSX_MULTIPLE_FONTS",
            "XLSX_MISSING_TABLE",
            "XLSX_CHART_MISSING_TITLE",
            "XLSX_UNREADABLE_TAB_COUNT",
        ])
    );
    for finding in report["findings"].as_array().unwrap() {
        assert!(
            finding["fixCommand"]
                .as_str()
                .unwrap()
                .starts_with("ooxml --json ")
        );
    }
    assert_package_proofs(&workbook);
    fs::remove_dir_all(temp).unwrap();
}

fn design_check(file: &Path) -> Value {
    run_ok(&["--json", "design-check", path(file)])
}

fn has_code(findings: &[Value], code: &str) -> bool {
    findings.iter().any(|finding| finding["code"] == code)
}

fn run(args: &[&str]) -> (Output, Value) {
    let output = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args(args)
        .output()
        .unwrap();
    let stream = if output.stdout.is_empty() {
        &output.stderr
    } else {
        &output.stdout
    };
    let report = serde_json::from_slice(stream)
        .unwrap_or_else(|error| panic!("{error}: {}", String::from_utf8_lossy(stream)));
    (output, report)
}

fn run_ok(args: &[&str]) -> Value {
    let (output, report) = run(args);
    assert!(output.status.success(), "{args:?}: {report}");
    report
}

fn run_owned_ok(args: &[String]) -> Value {
    let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    run_ok(&refs)
}

fn assert_package_proofs(file: &Path) {
    let report = run_ok(&["--json", "validate", "--strict", path(file)]);
    assert_eq!(report["summary"]["errors"], 0, "{report}");
    assert_eq!(report["summary"]["warnings"], 0, "{report}");
    let conformance = run_ok(&["--json", "conformance", "check", path(file)]);
    assert_eq!(conformance["summary"]["errors"], 0, "{conformance}");
    assert_eq!(conformance["summary"]["warnings"], 0, "{conformance}");

    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        println!("SKIP Open XML SDK design-check proof: HOME is unavailable");
        return;
    };
    let dotnet = home.join("dotnet/dotnet");
    let validator = fixture("tools/openxml-validator/bin/Release/net8.0/openxml-validator.dll");
    if !dotnet.is_file() || !validator.is_file() {
        println!("SKIP Open XML SDK design-check proof: validator unavailable");
        return;
    }
    let output = Command::new(dotnet)
        .arg(validator)
        .arg(file)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "SDK rejected {}: {}{}",
        file.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_libreoffice_renders(file: &Path, output_dir: &Path) {
    if !Path::new("/usr/bin/soffice").is_file() {
        println!("SKIP LibreOffice design-check render: /usr/bin/soffice unavailable");
        return;
    }
    fs::create_dir_all(output_dir).unwrap();
    let profile = output_dir.join("lo-profile");
    let output = Command::new("/usr/bin/soffice")
        .arg(format!("-env:UserInstallation=file://{}", path(&profile)))
        .args(["--headless", "--convert-to", "pdf", "--outdir"])
        .arg(output_dir)
        .arg(file)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "LibreOffice render failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let pdf = output_dir.join(format!(
        "{}.pdf",
        file.file_stem().unwrap().to_string_lossy()
    ));
    assert!(fs::metadata(pdf).unwrap().len() > 1_000);
}

fn temp_dir(label: &str) -> PathBuf {
    let path =
        std::env::temp_dir().join(format!("ooxml-design-check-{label}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

fn fixture(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative)
}

fn path(path: &Path) -> &str {
    path.to_str().unwrap()
}
