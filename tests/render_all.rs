use serde_json::{Value, json};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

const PPTX: &str = "testdata/pptx/minimal-title/presentation.pptx";
const XLSX: &str = "testdata/xlsx/types-and-formulas/workbook.xlsx";
const DOCX: &str = "testdata/docx/mixed-blocks/document.docx";

#[test]
fn render_mock_has_one_contract_for_pptx_xlsx_and_docx() {
    let root = temp_dir("mock-contract");
    let cases = [
        ("pptx", PPTX, "presentation", "slides", "slide", None),
        ("xlsx", XLSX, "workbook", "pages", "page", Some("Types")),
        ("docx", DOCX, "document", "pages", "page", None),
    ];

    for (family, file, stem, collection, number_key, sheet) in cases {
        let out = root.join(family);
        let out_text = path_text(&out);
        let mut args = vec![
            "--json", "render", file, "--out", &out_text, "--dpi", "96", "--pages", "1",
        ];
        if let Some(sheet) = sheet {
            args.extend(["--sheet", sheet]);
        }
        let output = run_with_env(&args, &[("OOXML_RUST_MOCK_RENDER", "1")]);
        assert_success(&output, family);
        let value = stdout_json(&output);
        assert_eq!(value["schemaVersion"], "1.0");
        assert_eq!(value["type"], family);
        assert_eq!(value["status"], "ok");
        assert_eq!(value["engine"], "mock");
        assert_eq!(value["dpi"], 96);
        assert_eq!(value["imageFormat"], "png");
        assert_eq!(
            value["doctorChecks"],
            serde_json::json!(["render-engine", "fonts"])
        );
        assert_eq!(value["outputDir"], out_text);
        assert!(
            !value["limitations"]
                .as_array()
                .expect("limitations array")
                .is_empty()
        );
        assert!(Path::new(value["pdfPath"].as_str().expect("pdfPath")).is_file());
        let items = value[collection].as_array().expect("rendered items");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0][number_key], 1);
        assert!(Path::new(items[0]["imagePath"].as_str().expect("imagePath")).is_file());
        let mut expected = json!({
            "schemaVersion": "1.0",
            "type": family,
            "status": "ok",
            "sourceFile": file,
            "outputDir": out_text,
            "dpi": 96,
            "imageFormat": "png",
            "doctorChecks": ["render-engine", "fonts"],
            "limitations": render_limitations(family),
            "pdfPath": out.join(format!("{stem}.pdf")).to_string_lossy().into_owned(),
            "engine": "mock",
        });
        let mut expected_item = json!({
            "imagePath": out
                .join(format!("{number_key}-1.png"))
                .to_string_lossy()
                .into_owned(),
        });
        expected_item
            .as_object_mut()
            .expect("render item object")
            .insert(number_key.to_string(), json!(1));
        expected
            .as_object_mut()
            .expect("manifest object")
            .insert(collection.to_string(), json!([expected_item]));
        if family == "xlsx" {
            assert_eq!(value["sheet"]["name"], "Types");
            assert_eq!(value["sheet"]["position"], 1);
            expected.as_object_mut().expect("manifest object").insert(
                "sheet".to_string(),
                json!({"name": "Types", "position": 1, "sheetId": 1}),
            );
        }
        assert!(
            value.get("warnings").is_none(),
            "{family} clean render must preserve the legacy manifest shape"
        );
        assert_eq!(value, expected, "{family} render JSON contract drifted");
    }

    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn render_surfaces_a_font_warning_when_doctor_reports_missing_fonts() {
    use std::os::unix::fs::PermissionsExt;

    let root = temp_dir("font-warning");
    let bin = root.join("bin");
    fs::create_dir_all(&bin).expect("create fake tool directory");
    let fc_list = bin.join("fc-list");
    fs::write(&fc_list, "#!/bin/sh\nexit 0\n").expect("write empty font inventory");
    fs::set_permissions(&fc_list, fs::Permissions::from_mode(0o755))
        .expect("make fake fc-list executable");
    let path = path_text(&bin);

    let doctor = run_with_env(
        &["--json", "doctor", "--only", "fonts"],
        &[("PATH", &path), ("Path", &path)],
    );
    assert_eq!(doctor.status.code(), Some(1));
    let doctor = stdout_json(&doctor);
    assert_eq!(doctor["checks"][0]["id"], "fonts");
    assert_eq!(doctor["checks"][0]["status"], "warn");

    let out = path_text(&root.join("render"));
    let render = run_with_env(
        &["--json", "render", DOCX, "--out", &out],
        &[
            ("PATH", &path),
            ("Path", &path),
            ("OOXML_RUST_MOCK_RENDER", "1"),
        ],
    );
    assert_success(&render, "mock render with missing font inventory");
    assert_eq!(
        stdout_json(&render)["warnings"],
        json!([{
            "code": "OOXML_RENDER_FONTS_UNAVAILABLE",
            "severity": "warning",
            "message": "fc-list returned no installed fonts",
            "remediation": "Install common document fonts for reliable rendering.",
            "doctorCommand": "ooxml --json doctor --only fonts",
        }])
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn render_pages_accepts_ranges_and_preserves_physical_page_numbers() {
    let root = temp_dir("page-ranges");
    let out = root.join("render");
    let out_text = path_text(&out);
    let output = run_with_env(
        &[
            "--json", "render", DOCX, "--out", &out_text, "--pages", "3,1-2,2",
        ],
        &[("OOXML_RUST_MOCK_RENDER", "1")],
    );
    assert_success(&output, "page range render");
    let value = stdout_json(&output);
    let pages = value["pages"].as_array().expect("pages");
    assert_eq!(
        pages
            .iter()
            .map(|page| page["page"].as_u64().expect("page number"))
            .collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn render_without_libreoffice_is_a_clean_skip_with_doctor_remediation() {
    let root = temp_dir("missing-tools");
    let empty_path = root.join("bin");
    let out = root.join("render");
    fs::create_dir_all(&empty_path).expect("empty PATH directory");
    let out_text = path_text(&out);
    let output = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args(["--json", "render", DOCX, "--out", &out_text])
        .env("PATH", &empty_path)
        .env("Path", &empty_path)
        .output()
        .expect("run render without local tools");

    assert_success(&output, "missing LibreOffice render");
    let value = stdout_json(&output);
    assert_eq!(value["status"], "skipped");
    assert_eq!(value["engine"], "libreoffice");
    assert_eq!(value["pdfPath"], Value::Null);
    assert_eq!(value["pages"], serde_json::json!([]));
    assert_eq!(value["missingTools"], serde_json::json!(["soffice"]));
    assert_eq!(
        value["remediation"],
        "Install LibreOffice and ensure soffice is on PATH."
    );
    assert_eq!(
        value["doctorCommand"],
        "ooxml --json doctor --only render-engine,fonts"
    );
    assert!(output.stderr.is_empty());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn real_libreoffice_pipeline_renders_all_three_families_when_available() {
    if !render_tools_available("all-family render", &["soffice|libreoffice", "pdftoppm"]) {
        return;
    }

    let root = temp_dir("real-pipeline");
    for (family, file, collection, expected_count) in [
        (
            "pptx",
            "testdata/pptx/multi-layout/presentation.pptx",
            "slides",
            4,
        ),
        ("xlsx", XLSX, "pages", 1),
        ("docx", DOCX, "pages", 2),
    ] {
        let out = root.join(family);
        let out_text = path_text(&out);
        let output = run(&["--json", "render", file, "--out", &out_text, "--dpi", "24"]);
        assert_success(&output, family);
        let value = stdout_json(&output);
        assert_eq!(value["status"], "ok", "{family}: {value}");
        assert_eq!(value["engine"], "libreoffice");
        assert!(Path::new(value["pdfPath"].as_str().expect("pdfPath")).is_file());
        let items = value[collection].as_array().expect("rendered items");
        assert_eq!(items.len(), expected_count, "{family}: {value}");
        for item in items {
            assert!(Path::new(item["imagePath"].as_str().expect("imagePath")).is_file());
        }
    }

    let two_sheet = path_text(&root.join("two-sheet.xlsx"));
    let add_sheet = run(&[
        "--json",
        "xlsx",
        "sheets",
        "add",
        XLSX,
        "--name",
        "RenderOnly",
        "--out",
        &two_sheet,
    ]);
    assert_success(&add_sheet, "add worksheet for selected-sheet render");
    assert_success(
        &run(&["--json", "validate", &two_sheet, "--strict"]),
        "strict validation of two-sheet render input",
    );
    let populated = path_text(&root.join("populated.xlsx"));
    let set_cell = run(&[
        "--json",
        "xlsx",
        "cells",
        "set",
        &two_sheet,
        "--sheet",
        "RenderOnly",
        "--cell",
        "A1",
        "--value",
        "Selected worksheet",
        "--out",
        &populated,
    ]);
    assert_success(&set_cell, "populate selected worksheet");
    assert_success(
        &run(&["--json", "validate", &populated, "--strict"]),
        "strict validation of populated render input",
    );
    let populated_before = fs::read(&populated).expect("read selected-sheet source before render");
    let sheet_out = path_text(&root.join("selected-sheet"));
    let selected = run(&[
        "--json",
        "render",
        &populated,
        "--out",
        &sheet_out,
        "--sheet",
        "RenderOnly",
        "--dpi",
        "24",
    ]);
    assert_success(&selected, "selected worksheet render");
    let selected = stdout_json(&selected);
    assert_eq!(selected["status"], "ok");
    assert_eq!(selected["sheet"]["name"], "RenderOnly");
    assert_eq!(selected["sheet"]["position"], 2);
    assert_eq!(selected["pages"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        fs::read(&populated).expect("read selected-sheet source after render"),
        populated_before,
        "selected-sheet rendering must not mutate its source workbook"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn real_render_diff_reports_pixel_ratio_and_structural_similarity_when_available() {
    if !render_tools_available(
        "visual render diff",
        &["soffice|libreoffice", "pdftoppm", "compare|magick"],
    ) {
        return;
    }

    let root = temp_dir("real-diff");
    let candidate = root.join("candidate.docx");
    rewrite_zip_text(
        Path::new(DOCX),
        &candidate,
        "word/document.xml",
        "Tail paragraph",
        "Tail sentence",
    );
    let candidate_text = path_text(&candidate);
    let validation = run(&["--json", "validate", &candidate_text, "--strict"]);
    assert_success(&validation, "strict validation of visual-diff candidate");

    let same_out = path_text(&root.join("same"));
    let same = run(&[
        "--json",
        "diff",
        DOCX,
        DOCX,
        "--render",
        "--threshold",
        "0",
        "--out",
        &same_out,
    ]);
    assert_success(&same, "identical real render diff");
    let same = stdout_json(&same);
    let same_pages = same["visual"]["pages"].as_array().expect("same pages");
    assert!(!same_pages.is_empty());
    for page in same_pages {
        assert_eq!(page["pixelDifferenceRatio"], 0.0);
        assert_eq!(page["structuralSimilarity"], 1.0);
        assert_eq!(page["pass"], true);
    }

    let changed_out = path_text(&root.join("changed"));
    let changed = run(&[
        "--json",
        "diff",
        DOCX,
        &candidate_text,
        "--render",
        "--threshold",
        "0",
        "--out",
        &changed_out,
    ]);
    assert_eq!(
        changed.status.code(),
        Some(8),
        "changed real render diff should cross zero threshold; stdout={}; stderr={}",
        String::from_utf8_lossy(&changed.stdout),
        String::from_utf8_lossy(&changed.stderr)
    );
    assert!(changed.stderr.is_empty());
    let changed = stdout_json(&changed);
    let changed_page = changed["visual"]["pages"]
        .as_array()
        .expect("changed pages")
        .iter()
        .find(|page| page["pixelDifferenceRatio"].as_f64().unwrap_or(0.0) > 0.0)
        .expect("at least one changed rendered page");
    let pixel_ratio = changed_page["pixelDifferenceRatio"]
        .as_f64()
        .expect("pixel difference ratio");
    let similarity = changed_page["structuralSimilarity"]
        .as_f64()
        .expect("structural similarity");
    assert!(
        (0.0..=1.0).contains(&pixel_ratio) && pixel_ratio > 0.0,
        "{changed_page}"
    );
    assert!((0.0..1.0).contains(&similarity), "{changed_page}");
    assert_eq!(changed_page["pass"], false);
    assert!(Path::new(changed_page["diffImage"].as_str().expect("diff image")).is_file());

    let _ = fs::remove_dir_all(root);
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args(args)
        .output()
        .expect("run ooxml")
}

fn run_with_env(args: &[&str], vars: &[(&str, &str)]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ooxml"));
    command.args(args);
    for (name, value) in vars {
        command.env(name, value);
    }
    command.output().expect("run ooxml")
}

fn stdout_json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "stdout was not JSON: {error}; stdout={}; stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn assert_success(output: &Output, label: &str) {
    assert!(
        output.status.success(),
        "{label} failed: status={:?}; stdout={}; stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "{label} wrote diagnostics on success: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn command_exists(name: &str) -> bool {
    Command::new(name).arg("--version").output().is_ok()
}

fn render_tools_available(label: &str, requirements: &[&str]) -> bool {
    let missing = requirements
        .iter()
        .filter(|requirement| !requirement.split('|').any(command_exists))
        .copied()
        .collect::<Vec<_>>();
    if missing.is_empty() {
        return true;
    }
    if render_is_required() {
        panic!(
            "OOXML render proof is required but {label} is missing: {}",
            missing.join(", ")
        );
    }
    eprintln!("SKIP {label}: missing {}", missing.join(", "));
    false
}

fn render_is_required() -> bool {
    ["OOXML_REQUIRE_RENDER", "OOXML_REQUIRE_LIBREOFFICE"]
        .into_iter()
        .any(|name| {
            std::env::var(name)
                .is_ok_and(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        })
}

fn render_limitations(family: &str) -> Vec<&'static str> {
    let mut limitations = vec![
        "LibreOffice rendering can substitute unavailable fonts and may differ from Microsoft Office layout.",
    ];
    limitations.push(match family {
        "pptx" => {
            "Static pages do not represent animations, transitions, audio, or video playback."
        }
        "xlsx" => {
            "Pagination follows LibreOffice Calc print areas, scaling, and page-break behavior."
        }
        "docx" => "Pagination can differ from Microsoft Word when fonts or layout engines differ.",
        _ => unreachable!("render family"),
    });
    limitations
}

fn temp_dir(label: &str) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "ooxml-render-all-{label}-{}-{suffix}",
        std::process::id()
    ));
    fs::create_dir_all(&path).expect("create test temp directory");
    path
}

fn path_text(path: &Path) -> String {
    path.to_str().expect("UTF-8 test path").to_string()
}

fn rewrite_zip_text(input: &Path, output: &Path, part: &str, from: &str, to: &str) {
    let source = fs::File::open(input).expect("open source package");
    let mut archive = ZipArchive::new(source).expect("read source package");
    let destination = fs::File::create(output).expect("create candidate package");
    let mut writer = ZipWriter::new(destination);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let mut replaced = false;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).expect("read package entry");
        if entry.is_dir() {
            writer
                .add_directory(entry.name(), options)
                .expect("copy package directory");
        } else {
            let name = entry.name().to_string();
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).expect("read package part");
            if name == part {
                let text = String::from_utf8(bytes).expect("UTF-8 XML part");
                assert!(text.contains(from), "fixture text missing from {part}");
                bytes = text.replacen(from, to, 1).into_bytes();
                replaced = true;
            }
            writer
                .start_file(name, options)
                .expect("start package part");
            writer.write_all(&bytes).expect("write package part");
        }
    }
    writer.finish().expect("finish candidate package");
    assert!(replaced, "expected to replace text in {part}");
}
