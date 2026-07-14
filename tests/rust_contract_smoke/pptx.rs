// PPTX frozen mutation/render/verify contract tests live here while shared
// baseline and process helpers remain in the parent integration test crate.
use super::*;

include!("pptx/scaffold.rs");

const PPTX_NOTES_REL_TYPE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesSlide";
const PPTX_SLIDE_REL_TYPE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide";

fn rels_part_for_uri(uri: &str) -> String {
    let part = uri.trim_start_matches('/');
    let (dir, name) = part
        .rsplit_once('/')
        .unwrap_or_else(|| panic!("relationship source should be a package part: {uri}"));
    format!("{dir}/_rels/{name}.rels")
}

fn relationship_target_between_parts(source_uri: &str, target_uri: &str) -> String {
    let source = source_uri.trim_start_matches('/');
    let target = target_uri.trim_start_matches('/');
    let source_dirs: Vec<&str> = source
        .rsplit_once('/')
        .map(|(dir, _)| dir.split('/').filter(|part| !part.is_empty()).collect())
        .unwrap_or_default();
    let target_parts: Vec<&str> = target.split('/').filter(|part| !part.is_empty()).collect();
    let common = source_dirs
        .iter()
        .zip(target_parts.iter())
        .take_while(|(left, right)| left == right)
        .count();
    let mut parts = Vec::new();
    for _ in common..source_dirs.len() {
        parts.push("..".to_string());
    }
    for part in target_parts.iter().skip(common) {
        parts.push((*part).to_string());
    }
    if parts.is_empty() {
        target.rsplit('/').next().unwrap_or(target).to_string()
    } else {
        parts.join("/")
    }
}

fn scrub_created_at(value: Value) -> Value {
    match value {
        Value::Object(mut map) => {
            for (key, item) in map.iter_mut() {
                if key == "createdAt" && item.as_str().is_some() {
                    *item = Value::String("[CREATED_AT]".to_string());
                } else {
                    *item = scrub_created_at(item.take());
                }
            }
            Value::Object(map)
        }
        Value::Array(items) => Value::Array(items.into_iter().map(scrub_created_at).collect()),
        other => other,
    }
}

fn scrub_translation_exported_at(value: Value) -> Value {
    match value {
        Value::Object(mut map) => {
            for (key, item) in map.iter_mut() {
                if key == "exportedAt" && item.as_str().is_some() {
                    *item = Value::String("[EXPORTED_AT]".to_string());
                } else {
                    *item = scrub_translation_exported_at(item.take());
                }
            }
            Value::Object(map)
        }
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(scrub_translation_exported_at)
                .collect(),
        ),
        other => other,
    }
}

#[cfg(unix)]
#[test]
fn pptx_render_json_captures_external_tool_chatter() {
    use std::os::unix::fs::PermissionsExt;

    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-render-stdout-contract-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&temp_dir);
    let bin_dir = temp_dir.join("bin");
    let out_dir = temp_dir.join("out");
    std::fs::create_dir_all(&bin_dir).expect("fake tool directory");

    let soffice = bin_dir.join("soffice");
    std::fs::write(
        &soffice,
        r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  echo "fake soffice version chatter"
  exit 0
fi
outdir=""
file=""
while [ "$#" -gt 0 ]; do
  if [ "$1" = "--outdir" ]; then
    shift
    outdir="$1"
  else
    file="$1"
  fi
  shift
done
stem="${file##*/}"
stem="${stem%.*}"
/bin/mkdir -p "$outdir"
: > "$outdir/$stem.pdf"
echo "fake soffice conversion chatter"
"#,
    )
    .expect("write fake soffice");
    std::fs::set_permissions(&soffice, std::fs::Permissions::from_mode(0o755))
        .expect("make fake soffice executable");

    let pdftoppm = bin_dir.join("pdftoppm");
    std::fs::write(
        &pdftoppm,
        r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  echo "fake pdftoppm version chatter"
  exit 0
fi
slide=""
prefix=""
while [ "$#" -gt 0 ]; do
  if [ "$1" = "-f" ]; then
    shift
    slide="$1"
  fi
  prefix="$1"
  shift
done
: > "$prefix-$slide.png"
echo "fake pdftoppm raster chatter"
"#,
    )
    .expect("write fake pdftoppm");
    std::fs::set_permissions(&pdftoppm, std::fs::Permissions::from_mode(0o755))
        .expect("make fake pdftoppm executable");

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args([
            "--json",
            "pptx",
            "render",
            "testdata/pptx/minimal-title/presentation.pptx",
            "--out",
            out_dir.to_str().expect("output path"),
            "--slides",
            "1",
            "--thumbnails",
        ])
        .env("PATH", &bin_dir)
        .output()
        .expect("run render with fake local tools");

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let parsed: Value = serde_json::from_slice(&output.stdout)
        .expect("external tool chatter must not precede render JSON");
    assert_eq!(parsed["slides"].as_array().map(Vec::len), Some(1));
    assert!(out_dir.join("presentation.pdf").exists());
    assert!(out_dir.join("slide-1.png").exists());

    let _ = std::fs::remove_dir_all(&temp_dir);
}

fn run_ooxml_raw(args: &[&str]) -> (i32, String, String) {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args(args)
        .output()
        .expect("run Rust ooxml raw");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8(output.stdout).expect("Rust stdout utf8"),
        String::from_utf8(output.stderr).expect("Rust stderr utf8"),
    )
}

fn run_ooxml_baseline_raw(args: &[&str]) -> (i32, String, String) {
    let output = std::process::Command::new(rust_repeat_or_comparison_binary())
        .args(args)
        .output()
        .expect("run Rust baseline ooxml raw");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8(output.stdout).expect("Rust baseline stdout utf8"),
        String::from_utf8(output.stderr).expect("Rust baseline stderr utf8"),
    )
}

fn parse_raw_json(text: &str) -> Value {
    serde_json::from_str(text.trim()).unwrap_or_else(|err| {
        panic!("invalid raw JSON {err}: {text}");
    })
}

#[test]
fn pptx_translate_export_matches_rust_baseline() {
    for args in [
        vec![
            "--json",
            "pptx",
            "translate",
            "export",
            "testdata/pptx/minimal-title/presentation.pptx",
            "--source-lang",
            "en-US",
            "--target-lang",
            "fr-FR",
        ],
        vec![
            "--json",
            "pptx",
            "translate",
            "export",
            "testdata/pptx/notes-slide/presentation.pptx",
            "--include-notes",
            "--source-lang",
            "en-US",
        ],
        vec![
            "--json",
            "pptx",
            "translate",
            "export",
            "testdata/pptx/minimal-title/presentation.pptx",
            "--slide",
            "99",
        ],
        vec![
            "--json",
            "pptx",
            "translate",
            "export",
            "testdata/pptx/minimal-title/presentation.pptx",
            "--source-lang",
            "xx_BAD",
            "--target-lang",
            "??",
        ],
        vec![
            "--json",
            "pptx",
            "translate",
            "export",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
        ],
    ] {
        let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&args);
        assert_eq!(
            rust_code, baseline_code,
            "translate export exit for {args:?}"
        );
        assert_eq!(
            rust_stderr, baseline_stderr,
            "translate export stderr for {args:?}"
        );
        assert_eq!(
            rust_stdout.map(scrub_translation_exported_at),
            baseline_stdout.map(scrub_translation_exported_at),
            "translate export stdout for {args:?}"
        );
    }
}

#[test]
fn pptx_translate_apply_saved_stale_and_errors_match_rust_baseline() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-translate-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("translate temp dir");

    let baseline_input = temp_dir.join("baseline-input.pptx");
    let rust_input = temp_dir.join("rust-input.pptx");
    let xlsx_input = temp_dir.join("input.xlsx");
    std::fs::copy(
        "testdata/pptx/minimal-title/presentation.pptx",
        &baseline_input,
    )
    .expect("copy Rust baseline translate fixture");
    std::fs::copy("testdata/pptx/minimal-title/presentation.pptx", &rust_input)
        .expect("copy Rust translate fixture");
    std::fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &xlsx_input)
        .expect("copy xlsx translate fixture");

    let manifest_path = temp_dir.join("manifest.json");
    let stale_manifest_path = temp_dir.join("stale.json");
    let invalid_manifest_path = temp_dir.join("invalid-id.json");
    std::fs::write(
        &manifest_path,
        r#"{"metadata":{"version":"1.0.0","exportedAt":"2026-06-20T00:00:00Z","sourceLanguage":"en-US","targetLanguage":"fr-FR","deckName":"presentation.pptx","slideCount":1,"entryCount":1},"entries":[{"id":"slide:0_title_p0_r0","type":"title","sourceText":"Minimal Title Slide","targetText":"Titre minimal","slideId":0,"slideNumber":1,"placeholderKey":"title","shapeId":2,"shapeName":"Title 1","paragraphIndex":0,"runIndex":0,"segmentType":"text"}]}"#,
    )
    .expect("write translate manifest");
    std::fs::write(
        &stale_manifest_path,
        r#"{"metadata":{"version":"1.0.0","exportedAt":"2026-06-20T00:00:00Z","slideCount":1,"entryCount":1},"entries":[{"id":"slide:0_title_p0_r0","type":"title","sourceText":"Old source","targetText":"Titre stale","slideId":0,"slideNumber":1,"placeholderKey":"title","shapeId":2,"shapeName":"Title 1","paragraphIndex":0,"runIndex":0,"segmentType":"text"}]}"#,
    )
    .expect("write stale translate manifest");
    std::fs::write(
        &invalid_manifest_path,
        r#"{"metadata":{"version":"1.0.0","exportedAt":"2026-06-20T00:00:00Z"},"entries":[{"id":"bad","type":"title","sourceText":"Minimal Title Slide","targetText":"Titre","slideId":0,"slideNumber":1,"paragraphIndex":0,"runIndex":0}]}"#,
    )
    .expect("write invalid translate manifest");

    let baseline_out = temp_dir.join("baseline-out.pptx");
    let rust_out = temp_dir.join("rust-out.pptx");
    let baseline_input_str = baseline_input.to_str().expect("baseline input");
    let rust_input_str = rust_input.to_str().expect("rust input");
    let xlsx_input_str = xlsx_input.to_str().expect("xlsx input");
    let manifest_str = manifest_path.to_str().expect("manifest path");
    let stale_manifest_str = stale_manifest_path.to_str().expect("stale manifest path");
    let invalid_manifest_str = invalid_manifest_path
        .to_str()
        .expect("invalid manifest path");
    let baseline_out_str = baseline_out.to_str().expect("baseline output");
    let rust_out_str = rust_out.to_str().expect("rust output");

    let baseline_args = [
        "--json",
        "pptx",
        "translate",
        "apply",
        baseline_input_str,
        manifest_str,
        "--output",
        baseline_out_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "translate",
        "apply",
        rust_input_str,
        manifest_str,
        "--output",
        rust_out_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "translate apply saved exit");
    assert_eq!(rust_stderr, baseline_stderr, "translate apply saved stderr");
    assert_eq!(rust_stdout, baseline_stdout, "translate apply saved stdout");
    assert!(
        baseline_out.exists(),
        "Rust baseline translate output missing"
    );
    assert!(rust_out.exists(), "Rust translate output missing");

    let (baseline_code, baseline_stdout, baseline_stderr) =
        run_ooxml_baseline(&["--json", "pptx", "extract", "text", baseline_out_str]);
    let (rust_code, rust_stdout, rust_stderr) =
        run_ooxml(&["--json", "pptx", "extract", "text", rust_out_str]);
    assert_eq!(rust_code, baseline_code, "translate apply readback exit");
    assert_eq!(
        rust_stderr, baseline_stderr,
        "translate apply readback stderr"
    );
    assert_eq!(
        scrub_paths(
            rust_stdout.expect("rust translate readback"),
            &[(rust_out_str, "[OUT]")]
        ),
        scrub_paths(
            baseline_stdout.expect("baseline translate readback"),
            &[(baseline_out_str, "[OUT]")]
        ),
        "translate apply readback stdout"
    );

    let (validate_code, validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "validate", "--strict", rust_out_str]);
    assert_eq!(validate_code, 0, "translate output strict validate exit");
    assert_eq!(
        validate_stderr, None,
        "translate output strict validate stderr"
    );
    assert_eq!(
        validate_stdout.expect("translate output strict validate")["valid"],
        Value::Bool(true)
    );

    for stale_mode in [None, Some("warn"), Some("error")] {
        let stale_label = stale_mode.unwrap_or("skip");
        let rust_stale_out = temp_dir.join(format!("rust-stale-{stale_label}.pptx"));
        let baseline_stale_out = temp_dir.join(format!("baseline-stale-{stale_label}.pptx"));
        let rust_stale_out_str = rust_stale_out.to_str().expect("rust stale output");
        let baseline_stale_out_str = baseline_stale_out.to_str().expect("baseline stale output");
        let mut baseline_args = vec![
            "--json",
            "pptx",
            "translate",
            "apply",
            baseline_input_str,
            stale_manifest_str,
        ];
        let mut rust_args = vec![
            "--json",
            "pptx",
            "translate",
            "apply",
            rust_input_str,
            stale_manifest_str,
        ];
        if let Some(mode) = stale_mode {
            baseline_args.extend(["--stale", mode]);
            rust_args.extend(["--stale", mode]);
        }
        baseline_args.extend(["--output", baseline_stale_out_str]);
        rust_args.extend(["--output", rust_stale_out_str]);

        let (baseline_code, baseline_stdout, baseline_stderr) =
            run_ooxml_baseline_raw(&baseline_args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml_raw(&rust_args);
        assert_eq!(
            rust_code, baseline_code,
            "translate stale {stale_mode:?} exit"
        );
        assert_eq!(
            rust_stderr, baseline_stderr,
            "translate stale {stale_mode:?} stderr"
        );
        if baseline_stdout.trim().is_empty() || rust_stdout.trim().is_empty() {
            assert_eq!(
                rust_stdout, baseline_stdout,
                "translate stale {stale_mode:?} stdout"
            );
        } else {
            assert_eq!(
                parse_raw_json(&rust_stdout),
                parse_raw_json(&baseline_stdout),
                "translate stale {stale_mode:?} stdout"
            );
        }
    }

    for args in [
        vec![
            "--json",
            "pptx",
            "translate",
            "apply",
            rust_input_str,
            manifest_str,
            "--stale",
            "explode",
            "--output",
            rust_out_str,
        ],
        vec![
            "--json",
            "pptx",
            "translate",
            "apply",
            rust_input_str,
            invalid_manifest_str,
            "--output",
            rust_out_str,
        ],
        vec![
            "--json",
            "pptx",
            "translate",
            "apply",
            xlsx_input_str,
            manifest_str,
            "--output",
            rust_out_str,
        ],
    ] {
        let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&args);
        assert_eq!(
            rust_code, baseline_code,
            "translate apply error exit for {args:?}"
        );
        assert_eq!(
            rust_stderr, baseline_stderr,
            "translate apply error stderr for {args:?}"
        );
        assert_eq!(
            rust_stdout, baseline_stdout,
            "translate apply error stdout for {args:?}"
        );
    }
}

#[test]
fn pptx_add_textbox_saved_readback_dry_run_and_errors_match_rust_baseline() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-add-textbox-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("add textbox temp dir");

    let fixture = "testdata/pptx/minimal-title/presentation.pptx";
    let dry_run_args = [
        "--json",
        "pptx",
        "add-textbox",
        fixture,
        "--slide",
        "1",
        "--text",
        "Agent text box",
        "--x",
        "100000",
        "--y",
        "200000",
        "--cx",
        "2000000",
        "--cy",
        "500000",
        "--name",
        "Agent Box",
        "--font-size",
        "20",
        "--font",
        "Arial",
        "--bold",
        "--color",
        "FF0000",
        "--dry-run",
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, baseline_code, "add-textbox dry-run exit");
    assert_eq!(rust_stderr, baseline_stderr, "add-textbox dry-run stderr");
    assert_eq!(
        scrub_created_at(rust_stdout.expect("rust add-textbox dry-run")),
        scrub_created_at(baseline_stdout.expect("baseline add-textbox dry-run")),
        "add-textbox dry-run stdout"
    );

    let baseline_out = temp_dir.join("baseline-add-textbox.pptx");
    let rust_out = temp_dir.join("rust-add-textbox.pptx");
    let baseline_out_str = baseline_out.to_str().expect("baseline add-textbox output");
    let rust_out_str = rust_out.to_str().expect("rust add-textbox output");
    let baseline_args = [
        "--json",
        "pptx",
        "add-textbox",
        fixture,
        "--slide",
        "1",
        "--text",
        "Agent text box",
        "--x",
        "100000",
        "--y",
        "200000",
        "--cx",
        "2000000",
        "--cy",
        "500000",
        "--name",
        "Agent Box",
        "--font-size",
        "20",
        "--font",
        "Arial",
        "--bold",
        "--color",
        "FF0000",
        "--out",
        baseline_out_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "add-textbox",
        fixture,
        "--slide",
        "1",
        "--text",
        "Agent text box",
        "--x",
        "100000",
        "--y",
        "200000",
        "--cx",
        "2000000",
        "--cy",
        "500000",
        "--name",
        "Agent Box",
        "--font-size",
        "20",
        "--font",
        "Arial",
        "--bold",
        "--color",
        "FF0000",
        "--out",
        rust_out_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "add-textbox saved exit");
    assert_eq!(rust_stderr, baseline_stderr, "add-textbox saved stderr");
    let rust_json = rust_stdout.expect("rust add-textbox saved");
    assert_eq!(
        scrub_created_at(scrub_path(rust_json.clone(), rust_out_str, "[OUT]")),
        scrub_created_at(scrub_path(
            baseline_stdout.expect("baseline add-textbox saved"),
            baseline_out_str,
            "[OUT]"
        )),
        "add-textbox saved stdout"
    );
    assert!(
        baseline_out.exists(),
        "Rust baseline add-textbox output missing"
    );
    assert!(rust_out.exists(), "Rust add-textbox output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    for args in [
        vec![
            "--json",
            "pptx",
            "add-textbox",
            fixture,
            "--slide",
            "1",
            "--cx",
            "2000000",
            "--cy",
            "500000",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "add-textbox",
            fixture,
            "--slide",
            "1",
            "--text",
            "Bad dimensions",
            "--cx",
            "0",
            "--cy",
            "500000",
            "--dry-run",
        ],
    ] {
        assert_baseline_rust_json_match(&args, "add-textbox representative error");
    }
}

#[test]
fn pptx_place_image_saved_readback_dry_run_and_errors_match_rust_baseline() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-place-image-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("place image temp dir");

    let fixture = "testdata/pptx/minimal-title/presentation.pptx";
    let image = "testdata/test_image.png";
    let dry_run_args = [
        "--json",
        "pptx",
        "place",
        "image",
        fixture,
        "--slide",
        "1",
        "--image",
        image,
        "--x",
        "100000",
        "--y",
        "200000",
        "--cx",
        "1000000",
        "--cy",
        "700000",
        "--name",
        "Agent Image",
        "--dry-run",
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, baseline_code, "place image dry-run exit");
    assert_eq!(rust_stderr, baseline_stderr, "place image dry-run stderr");
    assert_eq!(
        rust_stdout.expect("rust place image dry-run"),
        baseline_stdout.expect("baseline place image dry-run"),
        "place image dry-run stdout"
    );

    let baseline_out = temp_dir.join("baseline-place-image.pptx");
    let rust_out = temp_dir.join("rust-place-image.pptx");
    let baseline_out_str = baseline_out.to_str().expect("baseline place image output");
    let rust_out_str = rust_out.to_str().expect("rust place image output");
    let baseline_args = [
        "--json",
        "pptx",
        "place",
        "image",
        fixture,
        "--slide",
        "1",
        "--image",
        image,
        "--x",
        "100000",
        "--y",
        "200000",
        "--cx",
        "1000000",
        "--cy",
        "700000",
        "--name",
        "Agent Image",
        "--out",
        baseline_out_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "place",
        "image",
        fixture,
        "--slide",
        "1",
        "--image",
        image,
        "--x",
        "100000",
        "--y",
        "200000",
        "--cx",
        "1000000",
        "--cy",
        "700000",
        "--name",
        "Agent Image",
        "--out",
        rust_out_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "place image saved exit");
    assert_eq!(rust_stderr, baseline_stderr, "place image saved stderr");
    let rust_json = rust_stdout.expect("rust place image saved");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_out_str, "[OUT]"),
        scrub_path(
            baseline_stdout.expect("baseline place image saved"),
            baseline_out_str,
            "[OUT]"
        ),
        "place image saved stdout"
    );
    assert!(
        baseline_out.exists(),
        "Rust baseline place image output missing"
    );
    assert!(rust_out.exists(), "Rust place image output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    for args in [
        vec![
            "--json",
            "pptx",
            "place",
            "image",
            fixture,
            "--slide",
            "1",
            "--image",
            "testdata/missing.png",
            "--x",
            "0",
            "--y",
            "0",
            "--cx",
            "1",
            "--cy",
            "1",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "place",
            "image",
            fixture,
            "--slide",
            "1",
            "--image",
            image,
            "--x",
            "0",
            "--y",
            "0",
            "--cx",
            "1",
            "--cy",
            "1",
            "--fit-mode",
            "stretch",
            "--dry-run",
        ],
    ] {
        assert_baseline_rust_json_match(&args, "place image representative error");
    }
}

#[test]
fn pptx_place_table_saved_dry_run_readback_and_errors_match_rust_baseline() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-place-table-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("place table temp dir");

    let fixture = "testdata/pptx/minimal-title/presentation.pptx";
    let csv = temp_dir.join("data.csv");
    fs::write(&csv, "Region,Amount\nNorth,42").expect("write csv table data");
    let csv_str = csv.to_str().expect("csv data path");
    let json_data = temp_dir.join("data.json");
    fs::write(&json_data, r#"[["Region","Amount"],["South",55]]"#).expect("write json table data");
    let json_data_str = json_data.to_str().expect("json data path");

    let baseline_out = temp_dir.join("baseline-place-table.pptx");
    let rust_out = temp_dir.join("rust-place-table.pptx");
    let baseline_out_str = baseline_out.to_str().expect("baseline place table output");
    let rust_out_str = rust_out.to_str().expect("rust place table output");
    let baseline_args = [
        "--json",
        "pptx",
        "place",
        "table",
        fixture,
        "--slide",
        "1",
        "--data",
        csv_str,
        "--format",
        "csv",
        "--x",
        "0",
        "--y",
        "0",
        "--cx",
        "3000000",
        "--header",
        "--name",
        "Revenue Table",
        "--out",
        baseline_out_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "place",
        "table",
        fixture,
        "--slide",
        "1",
        "--data",
        csv_str,
        "--format",
        "csv",
        "--x",
        "0",
        "--y",
        "0",
        "--cx",
        "3000000",
        "--header",
        "--name",
        "Revenue Table",
        "--out",
        rust_out_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "place table saved exit");
    assert_eq!(rust_stderr, baseline_stderr, "place table saved stderr");
    assert_eq!(
        rust_stdout.expect("rust place table stdout"),
        baseline_stdout.expect("baseline place table stdout"),
        "place table saved stdout"
    );
    assert!(
        baseline_out.exists(),
        "Rust baseline place table output missing"
    );
    assert!(rust_out.exists(), "Rust place table output missing");

    let (baseline_show_code, baseline_show_stdout, baseline_show_stderr) = run_ooxml_baseline(&[
        "--json",
        "pptx",
        "tables",
        "show",
        baseline_out_str,
        "--slide",
        "1",
        "--target",
        "table:1",
    ]);
    let (rust_show_code, rust_show_stdout, rust_show_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "tables",
        "show",
        rust_out_str,
        "--slide",
        "1",
        "--target",
        "table:1",
    ]);
    assert_eq!(
        rust_show_code, baseline_show_code,
        "place table readback exit"
    );
    assert_eq!(
        rust_show_stderr, baseline_show_stderr,
        "place table readback stderr"
    );
    assert_eq!(
        scrub_path(
            rust_show_stdout.expect("rust place table readback"),
            rust_out_str,
            "[OUT]"
        ),
        scrub_path(
            baseline_show_stdout.expect("baseline place table readback"),
            baseline_out_str,
            "[OUT]"
        ),
        "place table readback stdout"
    );

    let (validate_code, _validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "validate", "--strict", rust_out_str]);
    assert_eq!(validate_code, 0, "place table validate exit");
    assert_eq!(validate_stderr, None, "place table validate stderr");

    let dry_run_args = [
        "--json",
        "pptx",
        "place",
        "table",
        fixture,
        "--slide",
        "1",
        "--data",
        json_data_str,
        "--format",
        "json",
        "--x",
        "0",
        "--y",
        "0",
        "--cx",
        "2000000",
        "--dry-run",
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, baseline_code, "place table dry-run exit");
    assert_eq!(rust_stderr, baseline_stderr, "place table dry-run stderr");
    assert_eq!(
        rust_stdout.expect("rust place table dry-run"),
        baseline_stdout.expect("baseline place table dry-run"),
        "place table dry-run stdout"
    );

    for args in [
        vec![
            "--json",
            "pptx",
            "place",
            "table",
            fixture,
            "--slide",
            "1",
            "--data",
            csv_str,
            "--format",
            "tsv",
            "--cx",
            "1000",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "place",
            "table",
            fixture,
            "--slide",
            "1",
            "--data",
            csv_str,
            "--cx",
            "0",
            "--dry-run",
        ],
    ] {
        assert_baseline_rust_json_match(&args, "place table representative error");
    }
}

#[test]
fn pptx_place_table_from_xlsx_saved_dry_run_and_errors_match_rust_baseline() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-place-table-xlsx-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("place table xlsx temp dir");

    let fixture = "testdata/pptx/minimal-title/presentation.pptx";
    let workbook = temp_dir.join("source-range.xlsx");
    write_simple_xlsx_with_sheet_xml(&workbook, pptx_update_source_sheet_xml_4x4());
    let workbook_str = workbook.to_str().expect("source workbook path");
    let table_workbook = temp_dir.join("source-table.xlsx");
    write_pptx_update_table_xlsx(&table_workbook);
    let table_workbook_str = table_workbook.to_str().expect("source table workbook path");

    let baseline_out = temp_dir.join("baseline-place-table-from-xlsx.pptx");
    let rust_out = temp_dir.join("rust-place-table-from-xlsx.pptx");
    let baseline_out_str = baseline_out
        .to_str()
        .expect("baseline place table xlsx output");
    let rust_out_str = rust_out.to_str().expect("rust place table xlsx output");
    let baseline_args = [
        "--json",
        "pptx",
        "place",
        "table-from-xlsx",
        fixture,
        "--workbook",
        workbook_str,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:B2",
        "--formula-mode",
        "formula",
        "--expect-source-range",
        "A1:B2",
        "--slide",
        "1",
        "--x",
        "0",
        "--y",
        "0",
        "--cx",
        "3000000",
        "--header",
        "--name",
        "Revenue Table",
        "--out",
        baseline_out_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "place",
        "table-from-xlsx",
        fixture,
        "--workbook",
        workbook_str,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:B2",
        "--formula-mode",
        "formula",
        "--expect-source-range",
        "A1:B2",
        "--slide",
        "1",
        "--x",
        "0",
        "--y",
        "0",
        "--cx",
        "3000000",
        "--header",
        "--name",
        "Revenue Table",
        "--out",
        rust_out_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "place table-from-xlsx saved exit");
    assert_eq!(
        rust_stderr, baseline_stderr,
        "place table-from-xlsx saved stderr"
    );
    let rust_json = rust_stdout.expect("rust place table-from-xlsx stdout");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_out_str, "[OUT]"),
        scrub_path(
            baseline_stdout.expect("baseline place table-from-xlsx stdout"),
            baseline_out_str,
            "[OUT]"
        ),
        "place table-from-xlsx saved stdout"
    );
    assert_eq!(rust_json["destination"]["cells"][0][0], "=SUM(B1:C1)");
    assert!(
        baseline_out.exists(),
        "Rust baseline place table-from-xlsx output missing"
    );
    assert!(
        rust_out.exists(),
        "Rust place table-from-xlsx output missing"
    );
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    let dry_run_args = [
        "--json",
        "pptx",
        "place",
        "table-from-xlsx",
        fixture,
        "--workbook",
        table_workbook_str,
        "--table",
        "Sales",
        "--expect-source-range",
        "A1:C3",
        "--slide",
        "1",
        "--x",
        "0",
        "--y",
        "0",
        "--cx",
        "3000000",
        "--dry-run",
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(
        rust_code, baseline_code,
        "place table-from-xlsx dry-run exit"
    );
    assert_eq!(
        rust_stderr, baseline_stderr,
        "place table-from-xlsx dry-run stderr"
    );
    assert_eq!(
        rust_stdout.expect("rust place table-from-xlsx dry-run"),
        baseline_stdout.expect("baseline place table-from-xlsx dry-run"),
        "place table-from-xlsx dry-run stdout"
    );

    let bad_out = temp_dir.join("bad.pptx");
    let bad_out_str = bad_out.to_str().expect("bad output path");
    for args in [
        vec![
            "--json",
            "pptx",
            "place",
            "table-from-xlsx",
            fixture,
            "--workbook",
            workbook_str,
            "--range",
            "A1",
            "--slide",
            "1",
            "--cx",
            "1000",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "place",
            "table-from-xlsx",
            fixture,
            "--workbook",
            workbook_str,
            "--sheet",
            "Sheet1",
            "--range",
            "A1:B2",
            "--max-cells",
            "1",
            "--slide",
            "1",
            "--cx",
            "1000",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "place",
            "table-from-xlsx",
            fixture,
            "--workbook",
            workbook_str,
            "--sheet",
            "Sheet1",
            "--range",
            "A1:B2",
            "--expect-source-range",
            "A1:C2",
            "--slide",
            "1",
            "--cx",
            "1000",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "place",
            "table-from-xlsx",
            workbook_str,
            "--workbook",
            workbook_str,
            "--sheet",
            "Sheet1",
            "--range",
            "A1",
            "--slide",
            "1",
            "--cx",
            "1000",
            "--out",
            bad_out_str,
        ],
    ] {
        assert_baseline_rust_json_match(&args, "place table-from-xlsx representative error");
    }
}

include!("pptx/charts.rs");

#[test]
fn pptx_animations_list_json_and_missing_path_match_rust_baseline() {
    assert_rust_baseline_match(&[
        "--json",
        "pptx",
        "animations",
        "list",
        "testdata/pptx/animations-synthetic/presentation.pptx",
    ]);
    assert_rust_baseline_match(&[
        "--json",
        "pptx",
        "animations",
        "list",
        "testdata/pptx/title-content/presentation.pptx",
    ]);

    let missing = std::env::temp_dir().join(format!(
        "ooxml-rust-missing-animations-{}-{}.pptx",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    let missing_str = missing.to_str().expect("missing path");
    assert_rust_baseline_match(&["--json", "pptx", "animations", "list", missing_str]);
}

include!("pptx/template.rs");

fn assert_strict_validate_succeeds(path: &str, label: &str) {
    let (code, stdout, stderr) = run_ooxml(&["validate", "--strict", path]);
    assert_eq!(code, 0, "{label} strict validate exit");
    assert_eq!(stderr, None, "{label} strict validate stderr");
    assert!(stdout.is_some(), "{label} strict validate stdout");
}

fn assert_conformance_check_runs(path: &str, label: &str) {
    let (_, stdout, stderr) = run_ooxml(&["--json", "conformance", "check", path]);
    assert_eq!(stderr, None, "{label} conformance check stderr");
    assert!(stdout.is_some(), "{label} conformance check stdout");
}

#[test]
fn pptx_xlsx_bindings_apply_saved_dry_run_and_errors_match_rust_baseline() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-xlsx-bindings-apply-{}-{suffix}",
        std::process::id()
    ));
    fs::create_dir_all(&temp_dir).expect("xlsx bindings apply temp dir");

    let fixture = "testdata/pptx/title-content/presentation.pptx";
    let workbook = temp_dir.join("bindings.xlsx");
    write_xlsx_bindings_workbook(&workbook, "subtitle");
    let workbook_str = workbook.to_string_lossy().to_string();
    let baseline_out = temp_dir.join("baseline-bindings.pptx");
    let rust_out = temp_dir.join("rust-bindings.pptx");
    let baseline_out_str = baseline_out.to_str().expect("baseline bindings output");
    let rust_out_str = rust_out.to_str().expect("rust bindings output");

    let baseline_args = [
        "--json",
        "pptx",
        "xlsx-bindings",
        "apply",
        fixture,
        "--workbook",
        &workbook_str,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:P3",
        "--out",
        baseline_out_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "xlsx-bindings",
        "apply",
        fixture,
        "--workbook",
        &workbook_str,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:P3",
        "--out",
        rust_out_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "xlsx-bindings apply saved exit");
    assert_eq!(
        rust_stderr, baseline_stderr,
        "xlsx-bindings apply saved stderr"
    );
    let baseline_json = baseline_stdout.expect("baseline xlsx-bindings apply saved");
    let rust_json = rust_stdout.expect("rust xlsx-bindings apply saved");
    assert_eq!(
        scrub_paths(rust_json.clone(), &[(rust_out_str, "[OUT]")]),
        scrub_paths(baseline_json, &[(baseline_out_str, "[OUT]")]),
        "xlsx-bindings apply saved stdout"
    );
    assert!(
        baseline_out.exists(),
        "Rust baseline xlsx-bindings output missing"
    );
    assert!(rust_out.exists(), "Rust xlsx-bindings output missing");
    let (validate_code, validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "validate", "--strict", rust_out_str]);
    assert_eq!(validate_code, 0, "bindings output strict validate exit");
    assert_eq!(
        validate_stderr, None,
        "bindings output strict validate stderr"
    );
    assert_eq!(
        validate_stdout.expect("bindings strict validate")["valid"],
        Value::Bool(true)
    );

    let (baseline_read_code, baseline_read_stdout, baseline_read_stderr) =
        run_ooxml_baseline(&["--json", "pptx", "extract", "text", baseline_out_str]);
    let (rust_read_code, rust_read_stdout, rust_read_stderr) =
        run_ooxml(&["--json", "pptx", "extract", "text", rust_out_str]);
    assert_eq!(rust_read_code, baseline_read_code, "bindings readback exit");
    assert_eq!(
        rust_read_stderr, baseline_read_stderr,
        "bindings readback stderr"
    );
    assert_eq!(
        scrub_path(
            rust_read_stdout.expect("rust bindings readback"),
            rust_out_str,
            "[OUT]"
        ),
        scrub_path(
            baseline_read_stdout.expect("baseline bindings readback"),
            baseline_out_str,
            "[OUT]"
        ),
        "bindings readback stdout"
    );

    let dry_run_args = [
        "--json",
        "pptx",
        "xlsx-bindings",
        "apply",
        fixture,
        "--workbook",
        &workbook_str,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:P3",
        "--dry-run",
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, baseline_code, "xlsx-bindings apply dry-run exit");
    assert_eq!(
        rust_stderr, baseline_stderr,
        "xlsx-bindings apply dry-run stderr"
    );
    assert_eq!(
        rust_stdout.expect("rust xlsx-bindings dry-run"),
        baseline_stdout.expect("baseline xlsx-bindings dry-run"),
        "xlsx-bindings apply dry-run stdout"
    );

    assert_baseline_rust_json_match(
        &[
            "--json",
            "pptx",
            "xlsx-bindings",
            "apply",
            fixture,
            "--workbook",
            &workbook_str,
            "--sheet",
            "Sheet1",
            "--range",
            "A1:P3",
            "--dry-run",
            "--out",
            rust_out_str,
        ],
        "xlsx-bindings apply dry-run out error",
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

fn write_xlsx_bindings_workbook(path: &Path, second_target: &str) {
    let header = [
        ("A", "id"),
        ("B", "op"),
        ("C", "slide"),
        ("D", "target"),
        ("E", "sourceSheet"),
        ("F", "sourceRange"),
        ("G", "mode"),
        ("H", "rowSep"),
        ("I", "colSep"),
        ("J", "formulaMode"),
        ("K", "x"),
        ("L", "y"),
        ("M", "cx"),
        ("N", "cy"),
        ("O", "name"),
        ("P", "header"),
    ];
    let row1 = header
        .iter()
        .map(|(col, value)| inline_str_cell(&format!("{col}1"), value))
        .collect::<String>();
    let row2 = [
        ("A2", "title"),
        ("B2", "replace-text"),
        ("C2", "1"),
        ("D2", "title"),
        ("E2", "Sheet1"),
        ("F2", "AA1"),
        ("G2", "preserve-format"),
        ("H2", "\\n"),
        ("I2", " | "),
        ("J2", "value"),
    ]
    .iter()
    .map(|(cell, value)| inline_str_cell(cell, value))
    .collect::<String>();
    let row3 = [
        ("A3", "move"),
        ("B3", "set-bounds"),
        ("C3", "1"),
        ("D3", second_target),
        ("K3", "100"),
        ("L3", "200"),
        ("M3", "3000000"),
        ("N3", "1000000"),
    ]
    .iter()
    .map(|(cell, value)| inline_str_cell(cell, value))
    .collect::<String>();
    let sheet_xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:AA3"/>
  <sheetData>
    <row r="1">{row1}<c r="AA1" t="inlineStr"><is><t>Bound Title</t></is></c></row>
    <row r="2">{row2}</row>
    <row r="3">{row3}</row>
  </sheetData>
</worksheet>"#
    );
    write_simple_xlsx_with_sheet_xml(path, &sheet_xml);
}

fn inline_str_cell(cell: &str, value: &str) -> String {
    format!(
        r#"<c r="{cell}" t="inlineStr"><is><t>{}</t></is></c>"#,
        value
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    )
}

#[test]
fn pptx_animations_mutations_match_rust_baseline_and_validate() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-animations-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("animations temp dir");

    let fixture = "testdata/pptx/title-content/presentation.pptx";
    let dry_run_args = [
        "--json",
        "pptx",
        "animations",
        "add",
        fixture,
        "--slide",
        "1",
        "--shape",
        "shape:2",
        "--effect",
        "appear",
        "--dry-run",
    ];
    assert_rust_baseline_match(&dry_run_args);

    let baseline_s1 = temp_dir.join("baseline-s1.pptx");
    let baseline_s2 = temp_dir.join("baseline-s2.pptx");
    let baseline_s3 = temp_dir.join("baseline-s3.pptx");
    let baseline_reordered = temp_dir.join("baseline-reordered.pptx");
    let baseline_removed = temp_dir.join("baseline-removed.pptx");
    let rust_s1 = temp_dir.join("rust-s1.pptx");
    let rust_s2 = temp_dir.join("rust-s2.pptx");
    let rust_s3 = temp_dir.join("rust-s3.pptx");
    let rust_reordered = temp_dir.join("rust-reordered.pptx");
    let rust_removed = temp_dir.join("rust-removed.pptx");

    let mut input_go = fixture.to_string();
    let mut input_rust = fixture.to_string();
    for (effect, baseline_out, rust_out) in [
        ("appear", &baseline_s1, &rust_s1),
        ("wipe", &baseline_s2, &rust_s2),
        ("fade", &baseline_s3, &rust_s3),
    ] {
        let baseline_out_str = baseline_out.to_str().expect("baseline animation output");
        let rust_out_str = rust_out.to_str().expect("rust animation output");
        let mut baseline_args = vec![
            "--json",
            "pptx",
            "animations",
            "add",
            input_go.as_str(),
            "--slide",
            "1",
            "--shape",
            "shape:2",
            "--effect",
            effect,
            "--out",
            baseline_out_str,
        ];
        let mut rust_args = vec![
            "--json",
            "pptx",
            "animations",
            "add",
            input_rust.as_str(),
            "--slide",
            "1",
            "--shape",
            "shape:2",
            "--effect",
            effect,
            "--out",
            rust_out_str,
        ];
        if effect == "wipe" {
            baseline_args.extend(["--direction", "up"]);
            rust_args.extend(["--direction", "up"]);
        }
        let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
        assert_eq!(rust_code, baseline_code, "add {effect} exit");
        assert_eq!(rust_stderr, baseline_stderr, "add {effect} stderr");
        let rust_json = rust_stdout.expect("rust add stdout");
        assert_eq!(
            scrub_paths(
                rust_json.clone(),
                &[(rust_out_str, "[OUT]"), (input_rust.as_str(), "[IN]")]
            ),
            scrub_paths(
                baseline_stdout.expect("baseline add stdout"),
                &[(baseline_out_str, "[OUT]"), (input_go.as_str(), "[IN]")]
            ),
            "add {effect} stdout"
        );
        assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");
        input_go = baseline_out_str.to_string();
        input_rust = rust_out_str.to_string();
    }

    let (baseline_list_code, baseline_list_stdout, baseline_list_stderr) = run_ooxml_baseline(&[
        "--json",
        "pptx",
        "animations",
        "list",
        baseline_s3.to_str().expect("baseline s3"),
    ]);
    let (rust_list_code, rust_list_stdout, rust_list_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "animations",
        "list",
        rust_s3.to_str().expect("rust s3"),
    ]);
    assert_eq!(rust_list_code, baseline_list_code, "list after add exit");
    assert_eq!(
        rust_list_stderr, baseline_list_stderr,
        "list after add stderr"
    );
    assert_eq!(
        rust_list_stdout.clone().expect("rust list after add"),
        baseline_list_stdout.expect("baseline list after add"),
        "list after add stdout"
    );
    let list_json = rust_list_stdout.expect("rust list json");
    let effects = list_json["slides"][0]["effects"]
        .as_array()
        .expect("animation effects");
    let order = [
        effects[2]["clickStepId"].as_i64().expect("third click id"),
        effects[1]["clickStepId"].as_i64().expect("second click id"),
        effects[0]["clickStepId"].as_i64().expect("first click id"),
    ]
    .iter()
    .map(ToString::to_string)
    .collect::<Vec<_>>()
    .join(",");

    let baseline_reordered_str = baseline_reordered
        .to_str()
        .expect("baseline reorder output");
    let rust_reordered_str = rust_reordered.to_str().expect("rust reorder output");
    let baseline_reorder_args = [
        "--json",
        "pptx",
        "animations",
        "reorder",
        baseline_s3.to_str().expect("baseline s3 path"),
        "--slide",
        "1",
        "--order",
        order.as_str(),
        "--out",
        baseline_reordered_str,
    ];
    let rust_reorder_args = [
        "--json",
        "pptx",
        "animations",
        "reorder",
        rust_s3.to_str().expect("rust s3 path"),
        "--slide",
        "1",
        "--order",
        order.as_str(),
        "--out",
        rust_reordered_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) =
        run_ooxml_baseline(&baseline_reorder_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_reorder_args);
    assert_eq!(rust_code, baseline_code, "reorder exit");
    assert_eq!(rust_stderr, baseline_stderr, "reorder stderr");
    let rust_reorder_json = rust_stdout.expect("rust reorder stdout");
    assert_eq!(
        scrub_paths(
            rust_reorder_json.clone(),
            &[
                (rust_reordered_str, "[OUT]"),
                (rust_s3.to_str().expect("rust s3 scrub"), "[IN]"),
            ]
        ),
        scrub_paths(
            baseline_stdout.expect("baseline reorder stdout"),
            &[
                (baseline_reordered_str, "[OUT]"),
                (baseline_s3.to_str().expect("baseline s3 scrub"), "[IN]"),
            ]
        ),
        "reorder stdout"
    );
    assert_rust_emitted_ooxml_command_succeeds(&rust_reorder_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_reorder_json, "validateCommand");

    let rust_reordered_list =
        run_ooxml(&["--json", "pptx", "animations", "list", rust_reordered_str])
            .1
            .expect("rust reordered list");
    let remove_id = rust_reordered_list["slides"][0]["effects"][1]["effectId"]
        .as_i64()
        .expect("middle effect id")
        .to_string();
    let baseline_removed_str = baseline_removed.to_str().expect("baseline removed output");
    let rust_removed_str = rust_removed.to_str().expect("rust removed output");
    let baseline_remove_args = [
        "--json",
        "pptx",
        "animations",
        "remove",
        baseline_reordered_str,
        "--slide",
        "1",
        "--effect-id",
        remove_id.as_str(),
        "--out",
        baseline_removed_str,
    ];
    let rust_remove_args = [
        "--json",
        "pptx",
        "animations",
        "remove",
        rust_reordered_str,
        "--slide",
        "1",
        "--effect-id",
        remove_id.as_str(),
        "--out",
        rust_removed_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) =
        run_ooxml_baseline(&baseline_remove_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_remove_args);
    assert_eq!(rust_code, baseline_code, "remove exit");
    assert_eq!(rust_stderr, baseline_stderr, "remove stderr");
    let rust_remove_json = rust_stdout.expect("rust remove stdout");
    assert_eq!(
        scrub_paths(
            rust_remove_json.clone(),
            &[(rust_removed_str, "[OUT]"), (rust_reordered_str, "[IN]")]
        ),
        scrub_paths(
            baseline_stdout.expect("baseline remove stdout"),
            &[
                (baseline_removed_str, "[OUT]"),
                (baseline_reordered_str, "[IN]")
            ]
        ),
        "remove stdout"
    );
    assert_rust_emitted_ooxml_command_succeeds(&rust_remove_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_remove_json, "validateCommand");

    let missing_args = [
        "--json",
        "pptx",
        "animations",
        "remove",
        "testdata/pptx/animations-synthetic/presentation.pptx",
        "--slide",
        "1",
        "--effect-id",
        "9999",
        "--dry-run",
    ];
    assert_rust_baseline_match(&missing_args);

    let prune_dry_run = [
        "--json",
        "pptx",
        "animations",
        "prune-stale",
        "testdata/pptx/animations-synthetic/presentation.pptx",
        "--dry-run",
    ];
    assert_rust_baseline_match(&prune_dry_run);

    let baseline_pruned = temp_dir.join("baseline-pruned.pptx");
    let rust_pruned = temp_dir.join("rust-pruned.pptx");
    let baseline_pruned_str = baseline_pruned.to_str().expect("baseline pruned output");
    let rust_pruned_str = rust_pruned.to_str().expect("rust pruned output");
    let baseline_prune_args = [
        "--json",
        "pptx",
        "animations",
        "prune-stale",
        "testdata/pptx/animations-synthetic/presentation.pptx",
        "--slide",
        "4",
        "--out",
        baseline_pruned_str,
    ];
    let rust_prune_args = [
        "--json",
        "pptx",
        "animations",
        "prune-stale",
        "testdata/pptx/animations-synthetic/presentation.pptx",
        "--slide",
        "4",
        "--out",
        rust_pruned_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) =
        run_ooxml_baseline(&baseline_prune_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_prune_args);
    assert_eq!(rust_code, baseline_code, "prune saved exit");
    assert_eq!(rust_stderr, baseline_stderr, "prune saved stderr");
    let rust_prune_json = rust_stdout.expect("rust prune stdout");
    assert_eq!(
        scrub_path(rust_prune_json.clone(), rust_pruned_str, "[OUT]"),
        scrub_path(
            baseline_stdout.expect("baseline prune stdout"),
            baseline_pruned_str,
            "[OUT]"
        ),
        "prune saved stdout"
    );
    assert_rust_emitted_ooxml_command_succeeds(&rust_prune_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_prune_json, "validateCommand");

    let (baseline_code, baseline_stdout, baseline_stderr) =
        run_ooxml_baseline(&["--json", "pptx", "animations", "list", baseline_pruned_str]);
    let (rust_code, rust_stdout, rust_stderr) =
        run_ooxml(&["--json", "pptx", "animations", "list", rust_pruned_str]);
    assert_eq!(rust_code, baseline_code, "prune readback exit");
    assert_eq!(rust_stderr, baseline_stderr, "prune readback stderr");
    assert_eq!(
        rust_stdout.expect("rust prune readback"),
        baseline_stdout.expect("baseline prune readback"),
        "prune readback stdout"
    );
}

fn assert_pptx_chart_copy_style_matches_rust_baseline(temp_dir: &Path) {
    let fixture = "testdata/pptx/chart-simple/presentation.pptx";
    let command = "copy-style";
    let baseline_out = temp_dir.join("baseline-copy-style.pptx");
    let rust_out = temp_dir.join("rust-copy-style.pptx");
    let baseline_out_str = baseline_out
        .to_str()
        .expect("baseline copy-style output path");
    let rust_out_str = rust_out.to_str().expect("rust copy-style output path");
    let baseline_args = [
        "--json",
        "pptx",
        "charts",
        command,
        fixture,
        "--chart",
        "chart:2",
        "--from",
        fixture,
        "--from-slide",
        "1",
        "--from-chart",
        "chart:1",
        "--expect-series-count",
        "1",
        "--out",
        baseline_out_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "charts",
        command,
        fixture,
        "--chart",
        "chart:2",
        "--from",
        fixture,
        "--from-slide",
        "1",
        "--from-chart",
        "chart:1",
        "--expect-series-count",
        "1",
        "--out",
        rust_out_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "copy-style exit");
    assert_eq!(rust_stderr, baseline_stderr, "copy-style stderr");
    let rust_json = rust_stdout.expect("rust copy-style stdout");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_out_str, "[OUT]"),
        scrub_path(
            baseline_stdout.expect("baseline copy-style stdout"),
            baseline_out_str,
            "[OUT]"
        ),
        "copy-style stdout"
    );
    assert!(
        baseline_out.exists(),
        "Rust baseline copy-style output missing"
    );
    assert!(rust_out.exists(), "Rust copy-style output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "chartShowCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    let baseline_show_args = [
        "--json",
        "pptx",
        "charts",
        "show",
        baseline_out_str,
        "--slide",
        "2",
        "--chart",
        "part:/ppt/charts/chart2.xml",
    ];
    let rust_show_args = [
        "--json",
        "pptx",
        "charts",
        "show",
        rust_out_str,
        "--slide",
        "2",
        "--chart",
        "part:/ppt/charts/chart2.xml",
    ];
    let (baseline_show_code, baseline_show_stdout, baseline_show_stderr) =
        run_ooxml_baseline(&baseline_show_args);
    let (rust_show_code, rust_show_stdout, rust_show_stderr) = run_ooxml(&rust_show_args);
    assert_eq!(
        rust_show_code, baseline_show_code,
        "copy-style readback exit"
    );
    assert_eq!(
        rust_show_stderr, baseline_show_stderr,
        "copy-style readback stderr"
    );
    assert_eq!(
        scrub_path(
            rust_show_stdout.expect("rust copy-style readback"),
            rust_out_str,
            "[OUT]"
        ),
        scrub_path(
            baseline_show_stdout.expect("baseline copy-style readback"),
            baseline_out_str,
            "[OUT]"
        ),
        "copy-style readback stdout"
    );
}

fn assert_pptx_chart_saved_mutation_matches_rust_baseline(
    temp_dir: &Path,
    command: &str,
    extra_args: &[&str],
) {
    let fixture = "testdata/pptx/chart-simple/presentation.pptx";
    let baseline_out = temp_dir.join(format!("baseline-{command}.pptx"));
    let rust_out = temp_dir.join(format!("rust-{command}.pptx"));
    let baseline_out_str = baseline_out
        .to_str()
        .expect("baseline chart mutation output path");
    let rust_out_str = rust_out.to_str().expect("rust chart mutation output path");

    let mut baseline_args = vec!["--json", "pptx", "charts", command, fixture];
    baseline_args.extend_from_slice(extra_args);
    baseline_args.extend_from_slice(&["--out", baseline_out_str]);
    let mut rust_args = vec!["--json", "pptx", "charts", command, fixture];
    rust_args.extend_from_slice(extra_args);
    rust_args.extend_from_slice(&["--out", rust_out_str]);

    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "{command} exit");
    assert_eq!(rust_stderr, baseline_stderr, "{command} stderr");
    let rust_json = rust_stdout.unwrap_or_else(|| panic!("rust {command} stdout"));
    assert_eq!(
        scrub_path(rust_json.clone(), rust_out_str, "[OUT]"),
        scrub_path(
            baseline_stdout.unwrap_or_else(|| panic!("baseline {command} stdout")),
            baseline_out_str,
            "[OUT]"
        ),
        "{command} stdout"
    );
    assert!(
        baseline_out.exists(),
        "Rust baseline {command} output missing"
    );
    assert!(rust_out.exists(), "Rust {command} output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "chartShowCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    let baseline_show_args = [
        "--json",
        "pptx",
        "charts",
        "show",
        baseline_out_str,
        "--slide",
        "1",
        "--chart",
        "part:/ppt/charts/chart1.xml",
    ];
    let rust_show_args = [
        "--json",
        "pptx",
        "charts",
        "show",
        rust_out_str,
        "--slide",
        "1",
        "--chart",
        "part:/ppt/charts/chart1.xml",
    ];
    let (baseline_show_code, baseline_show_stdout, baseline_show_stderr) =
        run_ooxml_baseline(&baseline_show_args);
    let (rust_show_code, rust_show_stdout, rust_show_stderr) = run_ooxml(&rust_show_args);
    assert_eq!(
        rust_show_code, baseline_show_code,
        "{command} readback exit"
    );
    assert_eq!(
        rust_show_stderr, baseline_show_stderr,
        "{command} readback stderr"
    );
    assert_eq!(
        scrub_path(
            rust_show_stdout.unwrap_or_else(|| panic!("rust {command} readback")),
            rust_out_str,
            "[OUT]"
        ),
        scrub_path(
            baseline_show_stdout.unwrap_or_else(|| panic!("baseline {command} readback")),
            baseline_out_str,
            "[OUT]"
        ),
        "{command} readback stdout"
    );
}

#[test]
fn frozen_pptx_mutation_and_validate_match_legacy_baseline() {
    let baseline = baseline();
    let temp_dir = std::env::temp_dir().join(format!("ooxml-rust-contract-{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).expect("temp dir");
    let edited = temp_dir.join("edited.pptx");
    let render_dir = temp_dir.join("rendered");
    let edited_str = edited.to_str().expect("temp path");
    let render_dir_str = render_dir.to_str().expect("render path");

    let edit_args = [
        "--json",
        "pptx",
        "replace",
        "text",
        "testdata/pptx/minimal-title/presentation.pptx",
        "--slide",
        "1",
        "--target",
        "title",
        "--text",
        "Rust Port Contract",
        "--out",
        edited_str,
    ];
    let (edit_code, edit_stdout, edit_stderr) = run_ooxml(&edit_args);
    assert_eq!(edit_code, 0);
    assert_eq!(edit_stderr, None);
    let edit_expected = baseline["mutation"]["edit"]["stdoutJson"].clone();
    assert_eq!(
        scrub_path(
            edit_stdout.expect("edit stdout"),
            edited_str,
            "[EDITED_PPTX]"
        ),
        edit_expected
    );
    assert!(edited.exists());

    let validate_args = ["--json", "--strict", "validate", edited_str];
    let (validate_code, validate_stdout, validate_stderr) = run_ooxml(&validate_args);
    assert_eq!(validate_code, 0);
    assert_eq!(validate_stderr, None);
    let validate_expected = baseline["mutation"]["validate"]["stdoutJson"].clone();
    assert_eq!(
        scrub_path(
            validate_stdout.expect("validate stdout"),
            edited_str,
            "[EDITED_PPTX]"
        ),
        validate_expected
    );

    let render_args = [
        "pptx",
        "render",
        edited_str,
        "--out",
        render_dir_str,
        "--slides",
        "1",
        "--format",
        "json",
    ];
    let (render_code, render_stdout, render_stderr) =
        run_ooxml_with_env(&render_args, &[("OOXML_RUST_MOCK_RENDER", "1")]);
    assert_eq!(render_code, 0);
    assert_eq!(render_stderr, None);
    let render_expected = baseline["mutation"]["render"]["stdoutJson"].clone();
    assert_eq!(
        scrub_paths(
            render_stdout.expect("render stdout"),
            &[
                (edited_str, "[EDITED_PPTX]"),
                (render_dir_str, "[RENDER_DIR]")
            ]
        ),
        render_expected
    );

    let verify_args = [
        "--format",
        "json",
        "verify",
        edited_str,
        "--baseline",
        "testdata/pptx/minimal-title/presentation.pptx",
    ];
    let (verify_code, verify_stdout, verify_stderr) = run_ooxml(&verify_args);
    assert_eq!(verify_code, 0);
    assert_eq!(verify_stderr, None);
    let verify_expected = baseline["mutation"]["verify"]["stdoutJson"].clone();
    assert_eq!(
        scrub_path(
            verify_stdout.expect("verify stdout"),
            edited_str,
            "[EDITED_PPTX]"
        ),
        verify_expected
    );
}

include!("pptx/media.rs");

include!("pptx/replace.rs");

include!("pptx/notes.rs");

#[test]
fn pptx_shapes_get_set_bounds_delete_saved_readback_dry_run_and_errors_match_rust_baseline() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-shapes-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("pptx shapes temp dir");

    let fixture = "testdata/pptx/title-content/presentation.pptx";
    let get_args = [
        "--json",
        "pptx",
        "shapes",
        "get",
        fixture,
        "--slide",
        "2",
        "--target",
        "body",
        "--include-text",
        "--include-bounds",
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&get_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&get_args);
    assert_eq!(rust_code, baseline_code, "shapes get exit");
    assert_eq!(rust_stderr, baseline_stderr, "shapes get stderr");
    assert_eq!(
        rust_stdout.expect("rust shapes get stdout"),
        baseline_stdout.expect("baseline shapes get stdout"),
        "shapes get stdout"
    );

    let baseline_bounds_out = temp_dir.join("baseline-set-bounds.pptx");
    let rust_bounds_out = temp_dir.join("rust-set-bounds.pptx");
    let baseline_bounds_out_str = baseline_bounds_out
        .to_str()
        .expect("baseline set-bounds path");
    let rust_bounds_out_str = rust_bounds_out.to_str().expect("rust set-bounds path");
    let baseline_set_args = [
        "--json",
        "pptx",
        "shapes",
        "set-bounds",
        fixture,
        "--slide",
        "2",
        "--target",
        "body",
        "--bounds",
        "111111,222222,333333,444444",
        "--out",
        baseline_bounds_out_str,
    ];
    let rust_set_args = [
        "--json",
        "pptx",
        "shapes",
        "set-bounds",
        fixture,
        "--slide",
        "2",
        "--target",
        "body",
        "--bounds",
        "111111,222222,333333,444444",
        "--out",
        rust_bounds_out_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_set_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_set_args);
    assert_eq!(rust_code, baseline_code, "set-bounds saved exit");
    assert_eq!(rust_stderr, baseline_stderr, "set-bounds saved stderr");
    let rust_set_json = rust_stdout.expect("rust set-bounds stdout");
    assert_eq!(
        scrub_path(rust_set_json.clone(), rust_bounds_out_str, "[OUT]"),
        scrub_path(
            baseline_stdout.expect("baseline set-bounds stdout"),
            baseline_bounds_out_str,
            "[OUT]"
        ),
        "set-bounds saved stdout"
    );
    assert!(
        baseline_bounds_out.exists(),
        "Rust baseline set-bounds output missing"
    );
    assert!(rust_bounds_out.exists(), "Rust set-bounds output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_set_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_set_json, "validateCommand");

    let (baseline_read_code, baseline_read_stdout, baseline_read_stderr) = run_ooxml_baseline(&[
        "--json",
        "pptx",
        "shapes",
        "get",
        baseline_bounds_out_str,
        "--slide",
        "2",
        "--target",
        "body",
        "--include-text",
        "--include-bounds",
    ]);
    let (rust_read_code, rust_read_stdout, rust_read_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "shapes",
        "get",
        rust_bounds_out_str,
        "--slide",
        "2",
        "--target",
        "body",
        "--include-text",
        "--include-bounds",
    ]);
    assert_eq!(
        rust_read_code, baseline_read_code,
        "set-bounds readback exit"
    );
    assert_eq!(
        rust_read_stderr, baseline_read_stderr,
        "set-bounds readback stderr"
    );
    assert_eq!(
        scrub_path(
            rust_read_stdout.expect("rust set-bounds readback"),
            rust_bounds_out_str,
            "[OUT]"
        ),
        scrub_path(
            baseline_read_stdout.expect("baseline set-bounds readback"),
            baseline_bounds_out_str,
            "[OUT]"
        ),
        "set-bounds readback stdout"
    );

    let set_dry_run_args = [
        "--json",
        "pptx",
        "shapes",
        "set-bounds",
        fixture,
        "--slide",
        "2",
        "--target",
        "body",
        "--bounds",
        "555555,666666,777777,888888",
        "--dry-run",
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&set_dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&set_dry_run_args);
    assert_eq!(rust_code, baseline_code, "set-bounds dry-run exit");
    assert_eq!(rust_stderr, baseline_stderr, "set-bounds dry-run stderr");
    assert_eq!(
        rust_stdout.expect("rust set-bounds dry-run stdout"),
        baseline_stdout.expect("baseline set-bounds dry-run stdout"),
        "set-bounds dry-run stdout"
    );

    let baseline_delete_out = temp_dir.join("baseline-delete-shape.pptx");
    let rust_delete_out = temp_dir.join("rust-delete-shape.pptx");
    let baseline_delete_out_str = baseline_delete_out.to_str().expect("baseline delete path");
    let rust_delete_out_str = rust_delete_out.to_str().expect("rust delete path");
    let baseline_delete_args = [
        "--json",
        "pptx",
        "shapes",
        "delete",
        fixture,
        "--slide",
        "2",
        "--target",
        "title",
        "--out",
        baseline_delete_out_str,
    ];
    let rust_delete_args = [
        "--json",
        "pptx",
        "shapes",
        "delete",
        fixture,
        "--slide",
        "2",
        "--target",
        "title",
        "--out",
        rust_delete_out_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) =
        run_ooxml_baseline(&baseline_delete_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_delete_args);
    assert_eq!(rust_code, baseline_code, "delete saved exit");
    assert_eq!(rust_stderr, baseline_stderr, "delete saved stderr");
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust delete stdout"),
            rust_delete_out_str,
            "[OUT]"
        ),
        scrub_path(
            baseline_stdout.expect("baseline delete stdout"),
            baseline_delete_out_str,
            "[OUT]"
        ),
        "delete saved stdout"
    );
    assert!(
        baseline_delete_out.exists(),
        "Rust baseline delete output missing"
    );
    assert!(rust_delete_out.exists(), "Rust delete output missing");
    let (validate_code, validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "validate", "--strict", rust_delete_out_str]);
    assert_eq!(validate_code, 0, "delete strict validate exit");
    assert_eq!(validate_stderr, None, "delete strict validate stderr");
    assert!(validate_stdout.is_some(), "delete strict validate stdout");

    let (baseline_show_code, baseline_show_stdout, baseline_show_stderr) = run_ooxml_baseline(&[
        "--json",
        "pptx",
        "shapes",
        "show",
        baseline_delete_out_str,
        "--slide",
        "2",
        "--include-text",
        "--include-bounds",
    ]);
    let (rust_show_code, rust_show_stdout, rust_show_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "shapes",
        "show",
        rust_delete_out_str,
        "--slide",
        "2",
        "--include-text",
        "--include-bounds",
    ]);
    assert_eq!(
        rust_show_code, baseline_show_code,
        "delete readback show exit"
    );
    assert_eq!(
        rust_show_stderr, baseline_show_stderr,
        "delete readback show stderr"
    );
    assert_eq!(
        scrub_path(
            rust_show_stdout.expect("rust delete readback show"),
            rust_delete_out_str,
            "[OUT]"
        ),
        scrub_path(
            baseline_show_stdout.expect("baseline delete readback show"),
            baseline_delete_out_str,
            "[OUT]"
        ),
        "delete readback show stdout"
    );

    let delete_dry_run_args = [
        "--json",
        "pptx",
        "shapes",
        "delete",
        fixture,
        "--slide",
        "2",
        "--target",
        "title",
        "--dry-run",
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) =
        run_ooxml_baseline(&delete_dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&delete_dry_run_args);
    assert_eq!(rust_code, baseline_code, "delete dry-run exit");
    assert_eq!(rust_stderr, baseline_stderr, "delete dry-run stderr");
    assert_eq!(
        rust_stdout.expect("rust delete dry-run stdout"),
        baseline_stdout.expect("baseline delete dry-run stdout"),
        "delete dry-run stdout"
    );

    let error_cases: Vec<Vec<&str>> = vec![
        vec![
            "--json", "pptx", "shapes", "get", fixture, "--slide", "2", "--target", "missing",
        ],
        vec![
            "--json",
            "pptx",
            "shapes",
            "set-bounds",
            fixture,
            "--slide",
            "2",
            "--target",
            "missing",
            "--bounds",
            "1,2,3,4",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "shapes",
            "set-bounds",
            fixture,
            "--slide",
            "2",
            "--target",
            "body",
            "--bounds",
            "bad",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "shapes",
            "delete",
            fixture,
            "--slide",
            "2",
            "--target",
            "missing",
            "--dry-run",
        ],
    ];
    for args in error_cases {
        let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&args);
        assert_eq!(rust_code, baseline_code, "shape error exit for {args:?}");
        assert_eq!(
            rust_stdout, baseline_stdout,
            "shape error stdout for {args:?}"
        );
        assert_eq!(
            rust_stderr, baseline_stderr,
            "shape error stderr for {args:?}"
        );
    }
}

#[test]
fn pptx_shapes_delete_nested_group_child_preserves_siblings_and_conforms() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-nested-group-delete-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("pptx nested group temp dir");

    let fixture = temp_dir.join("grouped-shapes.pptx");
    write_grouped_shapes_pptx(&fixture);
    let fixture_str = fixture.to_str().expect("grouped fixture path");

    let (show_code, show_stdout, show_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "shapes",
        "show",
        fixture_str,
        "--slide",
        "1",
        "--include-text",
        "--include-bounds",
    ]);
    assert_eq!(show_code, 0, "nested group source show exit");
    assert_eq!(show_stderr, None, "nested group source show stderr");
    let show_json = show_stdout.expect("nested group source show stdout");
    assert!(
        pptx_show_contains_shape_id(&show_json, 3),
        "source should publish first nested group child: {show_json}"
    );
    assert!(
        pptx_show_contains_shape_id(&show_json, 4),
        "source should publish deeper nested group child: {show_json}"
    );

    let rust_delete_out = temp_dir.join("rust-delete-nested.pptx");
    let rust_delete_out_str = rust_delete_out.to_str().expect("rust delete nested path");
    let (delete_code, delete_stdout, delete_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "shapes",
        "delete",
        fixture_str,
        "--slide",
        "1",
        "--target",
        "shape:3",
        "--out",
        rust_delete_out_str,
    ]);
    assert_eq!(delete_code, 0, "nested group child delete exit");
    assert_eq!(delete_stderr, None, "nested group child delete stderr");
    let delete_json = delete_stdout.expect("nested group child delete stdout");
    assert_eq!(delete_json["shapeId"], Value::from(3));
    assert_eq!(delete_json["deleted"]["shapeId"], Value::from(3));
    assert!(
        rust_delete_out.exists(),
        "nested group delete output missing"
    );

    let (show_code, show_stdout, show_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "shapes",
        "show",
        rust_delete_out_str,
        "--slide",
        "1",
        "--include-text",
        "--include-bounds",
    ]);
    assert_eq!(show_code, 0, "nested group delete readback exit");
    assert_eq!(show_stderr, None, "nested group delete readback stderr");
    let show_json = show_stdout.expect("nested group delete readback stdout");
    assert!(
        !pptx_show_contains_shape_id(&show_json, 3),
        "deleted nested child should be absent: {show_json}"
    );
    assert!(
        pptx_show_contains_shape_id(&show_json, 4),
        "deeper nested sibling should remain: {show_json}"
    );

    let (validate_code, validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "validate", "--strict", rust_delete_out_str]);
    assert_eq!(validate_code, 0, "nested group delete strict validate exit");
    assert_eq!(
        validate_stderr, None,
        "nested group delete strict validate stderr"
    );
    assert_eq!(
        validate_stdout.expect("nested group delete validate stdout")["valid"],
        Value::Bool(true)
    );

    let conformance_args = ["--json", "conformance", "check", rust_delete_out_str];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&conformance_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&conformance_args);
    assert_eq!(
        rust_code, baseline_code,
        "nested group delete conformance exit"
    );
    assert_eq!(
        rust_stderr, baseline_stderr,
        "nested group delete conformance stderr"
    );
    assert_eq!(
        rust_stdout, baseline_stdout,
        "nested group delete conformance stdout"
    );
}

fn pptx_show_contains_shape_id(show: &Value, shape_id: i64) -> bool {
    show.get("shapes")
        .and_then(Value::as_array)
        .is_some_and(|shapes| {
            shapes
                .iter()
                .any(|shape| shape.get("shapeId").and_then(Value::as_i64) == Some(shape_id))
        })
}

fn write_grouped_shapes_pptx(dest: &Path) {
    rewrite_zip_fixture(
        "testdata/pptx/minimal-title/presentation.pptx",
        dest,
        |name, data| {
            let data = if name == "ppt/slides/slide1.xml" {
                grouped_shapes_slide_xml().as_bytes().to_vec()
            } else {
                data
            };
            Some((name.to_string(), data))
        },
    );
}

fn grouped_shapes_slide_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <p:cSld>
    <p:spTree>
      <p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
      <p:grpSpPr/>
      <p:sp>
        <p:nvSpPr><p:cNvPr id="2" name="Top Text"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr>
        <p:spPr><a:xfrm><a:off x="100000" y="100000"/><a:ext cx="2000000" cy="500000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr>
        <p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t>Top level</a:t></a:r></a:p></p:txBody>
      </p:sp>
      <p:grpSp>
        <p:nvGrpSpPr><p:cNvPr id="10" name="Outer Group"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
        <p:grpSpPr><a:xfrm><a:off x="500000" y="500000"/><a:ext cx="4000000" cy="2000000"/><a:chOff x="0" y="0"/><a:chExt cx="4000000" cy="2000000"/></a:xfrm></p:grpSpPr>
        <p:sp>
          <p:nvSpPr><p:cNvPr id="3" name="Nested Box"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr>
          <p:spPr><a:xfrm><a:off x="600000" y="700000"/><a:ext cx="1200000" cy="500000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr>
          <p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t>Delete me</a:t></a:r></a:p></p:txBody>
        </p:sp>
        <p:grpSp>
          <p:nvGrpSpPr><p:cNvPr id="11" name="Inner Group"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
          <p:grpSpPr><a:xfrm><a:off x="2100000" y="800000"/><a:ext cx="1400000" cy="700000"/><a:chOff x="0" y="0"/><a:chExt cx="1400000" cy="700000"/></a:xfrm></p:grpSpPr>
          <p:sp>
            <p:nvSpPr><p:cNvPr id="4" name="Deep Box"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr>
            <p:spPr><a:xfrm><a:off x="2200000" y="900000"/><a:ext cx="1000000" cy="400000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr>
            <p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t>Keep me</a:t></a:r></a:p></p:txBody>
          </p:sp>
        </p:grpSp>
      </p:grpSp>
    </p:spTree>
  </p:cSld>
  <p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr>
</p:sld>"#
}

#[test]
fn pptx_layouts_mutations_saved_readback_and_dry_run_match_rust_baseline() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-layouts-mutation-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("pptx layouts mutation temp dir");

    let fixture = "testdata/pptx/title-content/presentation.pptx";

    let baseline_rename = temp_dir.join("baseline-rename.pptx");
    let rust_rename = temp_dir.join("rust-rename.pptx");
    let baseline_rename_str = baseline_rename.to_str().expect("baseline rename path");
    let rust_rename_str = rust_rename.to_str().expect("rust rename path");
    let baseline_rename_args = [
        "--json",
        "pptx",
        "layouts",
        "rename",
        fixture,
        "--layout",
        "2",
        "--name",
        "RustLayoutRenamed",
        "--out",
        baseline_rename_str,
    ];
    let rust_rename_args = [
        "--json",
        "pptx",
        "layouts",
        "rename",
        fixture,
        "--layout",
        "2",
        "--name",
        "RustLayoutRenamed",
        "--out",
        rust_rename_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) =
        run_ooxml_baseline(&baseline_rename_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_rename_args);
    assert_eq!(rust_code, baseline_code, "layout rename exit");
    assert_eq!(rust_stderr, baseline_stderr, "layout rename stderr");
    let rust_rename_json = rust_stdout.expect("rust layout rename stdout");
    assert_eq!(
        scrub_path(rust_rename_json.clone(), rust_rename_str, "[OUT]"),
        scrub_path(
            baseline_stdout.expect("baseline layout rename stdout"),
            baseline_rename_str,
            "[OUT]"
        ),
        "layout rename stdout"
    );
    assert!(
        baseline_rename.exists(),
        "Rust baseline layout rename output missing"
    );
    assert!(rust_rename.exists(), "Rust layout rename output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_rename_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_rename_json, "validateCommand");

    let (baseline_show_code, baseline_show_stdout, baseline_show_stderr) = run_ooxml_baseline(&[
        "--json",
        "pptx",
        "layouts",
        "show",
        baseline_rename_str,
        "--layout",
        "RustLayoutRenamed",
    ]);
    let (rust_show_code, rust_show_stdout, rust_show_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "layouts",
        "show",
        rust_rename_str,
        "--layout",
        "RustLayoutRenamed",
    ]);
    assert_eq!(
        rust_show_code, baseline_show_code,
        "layout rename readback exit"
    );
    assert_eq!(
        rust_show_stderr, baseline_show_stderr,
        "layout rename readback stderr"
    );
    assert_eq!(
        rust_show_stdout.expect("rust layout rename readback"),
        baseline_show_stdout.expect("baseline layout rename readback"),
        "layout rename readback stdout"
    );

    let baseline_bounds = temp_dir.join("baseline-bounds.pptx");
    let rust_bounds = temp_dir.join("rust-bounds.pptx");
    let baseline_bounds_str = baseline_bounds.to_str().expect("baseline bounds path");
    let rust_bounds_str = rust_bounds.to_str().expect("rust bounds path");
    let baseline_bounds_args = [
        "--json",
        "pptx",
        "layouts",
        "set-bounds",
        fixture,
        "--layout",
        "2",
        "--target",
        "shape:3",
        "--bounds",
        "111111,222222,333333,444444",
        "--out",
        baseline_bounds_str,
    ];
    let rust_bounds_args = [
        "--json",
        "pptx",
        "layouts",
        "set-bounds",
        fixture,
        "--layout",
        "2",
        "--target",
        "shape:3",
        "--bounds",
        "111111,222222,333333,444444",
        "--out",
        rust_bounds_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) =
        run_ooxml_baseline(&baseline_bounds_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_bounds_args);
    assert_eq!(rust_code, baseline_code, "layout set-bounds exit");
    assert_eq!(rust_stderr, baseline_stderr, "layout set-bounds stderr");
    let rust_bounds_json = rust_stdout.expect("rust layout set-bounds stdout");
    assert_eq!(
        scrub_path(rust_bounds_json.clone(), rust_bounds_str, "[OUT]"),
        scrub_path(
            baseline_stdout.expect("baseline layout set-bounds stdout"),
            baseline_bounds_str,
            "[OUT]"
        ),
        "layout set-bounds stdout"
    );
    assert_rust_emitted_ooxml_command_succeeds(&rust_bounds_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_bounds_json, "validateCommand");
    let (baseline_show_code, baseline_show_stdout, baseline_show_stderr) = run_ooxml_baseline(&[
        "--json",
        "pptx",
        "layouts",
        "show",
        baseline_bounds_str,
        "--layout",
        "2",
    ]);
    let (rust_show_code, rust_show_stdout, rust_show_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "layouts",
        "show",
        rust_bounds_str,
        "--layout",
        "2",
    ]);
    assert_eq!(
        rust_show_code, baseline_show_code,
        "layout set-bounds readback exit"
    );
    assert_eq!(
        rust_show_stderr, baseline_show_stderr,
        "layout set-bounds readback stderr"
    );
    assert_eq!(
        rust_show_stdout.expect("rust layout set-bounds readback"),
        baseline_show_stdout.expect("baseline layout set-bounds readback"),
        "layout set-bounds readback stdout"
    );

    let baseline_delete = temp_dir.join("baseline-delete.pptx");
    let rust_delete = temp_dir.join("rust-delete.pptx");
    let baseline_delete_str = baseline_delete.to_str().expect("baseline delete path");
    let rust_delete_str = rust_delete.to_str().expect("rust delete path");
    let baseline_delete_args = [
        "--json",
        "pptx",
        "layouts",
        "delete-shape",
        fixture,
        "--layout",
        "2",
        "--target",
        "shape:3",
        "--out",
        baseline_delete_str,
    ];
    let rust_delete_args = [
        "--json",
        "pptx",
        "layouts",
        "delete-shape",
        fixture,
        "--layout",
        "2",
        "--target",
        "shape:3",
        "--out",
        rust_delete_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) =
        run_ooxml_baseline(&baseline_delete_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_delete_args);
    assert_eq!(rust_code, baseline_code, "layout delete-shape exit");
    assert_eq!(rust_stderr, baseline_stderr, "layout delete-shape stderr");
    let rust_delete_json = rust_stdout.expect("rust layout delete-shape stdout");
    assert_eq!(
        scrub_path(rust_delete_json.clone(), rust_delete_str, "[OUT]"),
        scrub_path(
            baseline_stdout.expect("baseline layout delete-shape stdout"),
            baseline_delete_str,
            "[OUT]"
        ),
        "layout delete-shape stdout"
    );
    assert_rust_emitted_ooxml_command_succeeds(&rust_delete_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_delete_json, "validateCommand");
    let (baseline_show_code, baseline_show_stdout, baseline_show_stderr) = run_ooxml_baseline(&[
        "--json",
        "pptx",
        "layouts",
        "show",
        baseline_delete_str,
        "--layout",
        "2",
    ]);
    let (rust_show_code, rust_show_stdout, rust_show_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "layouts",
        "show",
        rust_delete_str,
        "--layout",
        "2",
    ]);
    assert_eq!(
        rust_show_code, baseline_show_code,
        "layout delete-shape readback exit"
    );
    assert_eq!(
        rust_show_stderr, baseline_show_stderr,
        "layout delete-shape readback stderr"
    );
    assert_eq!(
        rust_show_stdout.expect("rust layout delete-shape readback"),
        baseline_show_stdout.expect("baseline layout delete-shape readback"),
        "layout delete-shape readback stdout"
    );

    let baseline_add = temp_dir.join("baseline-add-placeholder.pptx");
    let rust_add = temp_dir.join("rust-add-placeholder.pptx");
    let baseline_add_str = baseline_add
        .to_str()
        .expect("baseline add-placeholder path");
    let rust_add_str = rust_add.to_str().expect("rust add-placeholder path");
    let baseline_add_args = [
        "--json",
        "pptx",
        "layouts",
        "add-placeholder",
        fixture,
        "--layout",
        "7",
        "--type",
        "pic",
        "--idx",
        "0",
        "--bounds",
        "1000,2000,3000,4000",
        "--out",
        baseline_add_str,
    ];
    let rust_add_args = [
        "--json",
        "pptx",
        "layouts",
        "add-placeholder",
        fixture,
        "--layout",
        "7",
        "--type",
        "pic",
        "--idx",
        "0",
        "--bounds",
        "1000,2000,3000,4000",
        "--out",
        rust_add_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_add_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_add_args);
    assert_eq!(rust_code, baseline_code, "layout add-placeholder exit");
    assert_eq!(
        rust_stderr, baseline_stderr,
        "layout add-placeholder stderr"
    );
    let rust_add_json = rust_stdout.expect("rust layout add-placeholder stdout");
    assert_eq!(
        scrub_path(rust_add_json.clone(), rust_add_str, "[OUT]"),
        scrub_path(
            baseline_stdout.expect("baseline layout add-placeholder stdout"),
            baseline_add_str,
            "[OUT]"
        ),
        "layout add-placeholder stdout"
    );
    assert_rust_emitted_ooxml_command_succeeds(&rust_add_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_add_json, "validateCommand");
    let (baseline_show_code, baseline_show_stdout, baseline_show_stderr) = run_ooxml_baseline(&[
        "--json",
        "pptx",
        "layouts",
        "show",
        baseline_add_str,
        "--layout",
        "7",
    ]);
    let (rust_show_code, rust_show_stdout, rust_show_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "layouts",
        "show",
        rust_add_str,
        "--layout",
        "7",
    ]);
    assert_eq!(
        rust_show_code, baseline_show_code,
        "layout add-placeholder readback exit"
    );
    assert_eq!(
        rust_show_stderr, baseline_show_stderr,
        "layout add-placeholder readback stderr"
    );
    assert_eq!(
        rust_show_stdout.expect("rust layout add-placeholder readback"),
        baseline_show_stdout.expect("baseline layout add-placeholder readback"),
        "layout add-placeholder readback stdout"
    );

    let dry_run_args = [
        "--json",
        "pptx",
        "layouts",
        "rename",
        fixture,
        "--layout",
        "2",
        "--name",
        "DryRunLayout",
        "--dry-run",
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, baseline_code, "layout rename dry-run exit");
    assert_eq!(rust_stderr, baseline_stderr, "layout rename dry-run stderr");
    assert_eq!(
        rust_stdout.expect("rust layout rename dry-run stdout"),
        baseline_stdout.expect("baseline layout rename dry-run stdout"),
        "layout rename dry-run stdout"
    );
}

#[test]
fn pptx_layout_slide_authoring_commands_match_rust_baseline_and_validate() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-layout-slide-authoring-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("pptx layout slide authoring temp dir");

    let fixture = "testdata/pptx/title-content/presentation.pptx";

    for args in [
        vec![
            "--json",
            "pptx",
            "layouts",
            "clone",
            fixture,
            "--layout",
            "1",
            "--name",
            "RustClonedLayout",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "masters",
            "add-placeholder",
            fixture,
            "--master",
            "1",
            "--type",
            "text",
            "--bounds",
            "100000,100000,1000000,500000",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "clone-slide",
            fixture,
            "--slide",
            "1",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "new-slide-from-layout",
            fixture,
            "--layout",
            "1",
            "--set-text",
            "title=RustTitle",
            "--dry-run",
        ],
    ] {
        assert_rust_baseline_match(&args);
    }

    let baseline_layout = temp_dir.join("baseline-layout-clone.pptx");
    let rust_layout = temp_dir.join("rust-layout-clone.pptx");
    let baseline_layout_str = baseline_layout
        .to_str()
        .expect("baseline layout clone path");
    let rust_layout_str = rust_layout.to_str().expect("rust layout clone path");
    let baseline_args = [
        "--json",
        "pptx",
        "layouts",
        "clone",
        fixture,
        "--layout",
        "1",
        "--name",
        "RustClonedLayout",
        "--out",
        baseline_layout_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "layouts",
        "clone",
        fixture,
        "--layout",
        "1",
        "--name",
        "RustClonedLayout",
        "--out",
        rust_layout_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "layout clone exit");
    assert_eq!(rust_stderr, baseline_stderr, "layout clone stderr");
    let rust_json = rust_stdout.expect("rust layout clone stdout");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_layout_str, "[OUT]"),
        scrub_path(
            baseline_stdout.expect("baseline layout clone stdout"),
            baseline_layout_str,
            "[OUT]"
        ),
        "layout clone stdout"
    );
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    let (baseline_show_code, baseline_show_stdout, baseline_show_stderr) = run_ooxml_baseline(&[
        "--json",
        "pptx",
        "layouts",
        "show",
        baseline_layout_str,
        "--layout",
        "RustClonedLayout",
    ]);
    let (rust_show_code, rust_show_stdout, rust_show_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "layouts",
        "show",
        rust_layout_str,
        "--layout",
        "RustClonedLayout",
    ]);
    assert_eq!(
        rust_show_code, baseline_show_code,
        "layout clone readback exit"
    );
    assert_eq!(
        rust_show_stderr, baseline_show_stderr,
        "layout clone readback stderr"
    );
    assert_eq!(
        rust_show_stdout.expect("rust layout clone readback"),
        baseline_show_stdout.expect("baseline layout clone readback"),
        "layout clone readback stdout"
    );

    let baseline_master = temp_dir.join("baseline-master-placeholder.pptx");
    let rust_master = temp_dir.join("rust-master-placeholder.pptx");
    let baseline_master_str = baseline_master
        .to_str()
        .expect("baseline master placeholder path");
    let rust_master_str = rust_master.to_str().expect("rust master placeholder path");
    let baseline_args = [
        "--json",
        "pptx",
        "masters",
        "add-placeholder",
        fixture,
        "--master",
        "1",
        "--type",
        "text",
        "--bounds",
        "100000,100000,1000000,500000",
        "--out",
        baseline_master_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "masters",
        "add-placeholder",
        fixture,
        "--master",
        "1",
        "--type",
        "text",
        "--bounds",
        "100000,100000,1000000,500000",
        "--out",
        rust_master_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "master add-placeholder exit");
    assert_eq!(
        rust_stderr, baseline_stderr,
        "master add-placeholder stderr"
    );
    let rust_json = rust_stdout.expect("rust master add-placeholder stdout");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_master_str, "[OUT]"),
        scrub_path(
            baseline_stdout.expect("baseline master add-placeholder stdout"),
            baseline_master_str,
            "[OUT]"
        ),
        "master add-placeholder stdout"
    );
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    let baseline_clone = temp_dir.join("baseline-clone-slide.pptx");
    let rust_clone = temp_dir.join("rust-clone-slide.pptx");
    let baseline_clone_str = baseline_clone.to_str().expect("baseline clone-slide path");
    let rust_clone_str = rust_clone.to_str().expect("rust clone-slide path");
    let baseline_args = [
        "--json",
        "pptx",
        "clone-slide",
        fixture,
        "--slide",
        "1",
        "--out",
        baseline_clone_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "clone-slide",
        fixture,
        "--slide",
        "1",
        "--out",
        rust_clone_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "clone-slide exit");
    assert_eq!(rust_stderr, baseline_stderr, "clone-slide stderr");
    let rust_json = rust_stdout.expect("rust clone-slide stdout");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_clone_str, "[OUT]"),
        scrub_path(
            baseline_stdout.expect("baseline clone-slide stdout"),
            baseline_clone_str,
            "[OUT]"
        ),
        "clone-slide stdout"
    );
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    let baseline_new = temp_dir.join("baseline-new-slide.pptx");
    let rust_new = temp_dir.join("rust-new-slide.pptx");
    let baseline_new_str = baseline_new.to_str().expect("baseline new slide path");
    let rust_new_str = rust_new.to_str().expect("rust new slide path");
    let baseline_args = [
        "--json",
        "pptx",
        "new-slide-from-layout",
        fixture,
        "--layout",
        "1",
        "--set-text",
        "title=RustTitle",
        "--out",
        baseline_new_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "new-slide-from-layout",
        fixture,
        "--layout",
        "1",
        "--set-text",
        "title=RustTitle",
        "--out",
        rust_new_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "new-slide-from-layout exit");
    assert_eq!(rust_stderr, baseline_stderr, "new-slide-from-layout stderr");
    let rust_json = rust_stdout.expect("rust new-slide-from-layout stdout");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_new_str, "[OUT]"),
        scrub_path(
            baseline_stdout.expect("baseline new-slide-from-layout stdout"),
            baseline_new_str,
            "[OUT]"
        ),
        "new-slide-from-layout stdout"
    );
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    let new_slide = rust_json["newSlideNumber"]
        .as_i64()
        .expect("new slide number");
    let new_slide_arg = new_slide.to_string();
    let rust_readback = run_ooxml(&[
        "--json",
        "pptx",
        "shapes",
        "get",
        rust_new_str,
        "--slide",
        &new_slide_arg,
        "--target",
        "title",
        "--include-text",
    ])
    .1
    .expect("rust new slide title readback");
    assert_eq!(
        rust_readback["shapes"][0]["textPreview"], "RustTitle",
        "new slide title text readback"
    );

    let image_slot_fixture = "testdata/pptx/picture-placeholder/presentation.pptx";
    let baseline_image_slot = temp_dir.join("baseline-new-slide-image-slot.pptx");
    let rust_image_slot = temp_dir.join("rust-new-slide-image-slot.pptx");
    let baseline_image_slot_str = baseline_image_slot
        .to_str()
        .expect("baseline image slot path");
    let rust_image_slot_str = rust_image_slot.to_str().expect("rust image slot path");
    let baseline_args = [
        "--json",
        "pptx",
        "new-slide-from-layout",
        image_slot_fixture,
        "--layout",
        "9",
        "--set-image-slot",
        "pic:1=testdata/pptx/template-branded/test-image.png",
        "--image-fit",
        "cover",
        "--out",
        baseline_image_slot_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "new-slide-from-layout",
        image_slot_fixture,
        "--layout",
        "9",
        "--set-image-slot",
        "pic:1=testdata/pptx/template-branded/test-image.png",
        "--image-fit",
        "cover",
        "--out",
        rust_image_slot_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "new-slide image-slot exit");
    assert_eq!(rust_stderr, baseline_stderr, "new-slide image-slot stderr");
    let rust_json = rust_stdout.expect("rust new-slide image-slot stdout");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_image_slot_str, "[OUT]"),
        scrub_path(
            baseline_stdout.expect("baseline new-slide image-slot stdout"),
            baseline_image_slot_str,
            "[OUT]"
        ),
        "new-slide image-slot stdout"
    );
    assert_eq!(
        rust_json["destination"]["images"], 1,
        "new slide image-slot readback image count"
    );
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");
}

#[test]
fn pptx_clone_slide_clones_notes_part_and_backlink_like_rust_baseline() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-clone-notes-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("pptx clone notes temp dir");

    let fixture = "testdata/pptx/slide-assembly-notes-media/presentation.pptx";
    let baseline_out = temp_dir.join("baseline-clone-notes.pptx");
    let rust_out = temp_dir.join("rust-clone-notes.pptx");
    let baseline_out_str = baseline_out.to_str().expect("baseline clone notes path");
    let rust_out_str = rust_out.to_str().expect("rust clone notes path");
    let baseline_args = [
        "--json",
        "pptx",
        "clone-slide",
        fixture,
        "--slide",
        "1",
        "--out",
        baseline_out_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "clone-slide",
        fixture,
        "--slide",
        "1",
        "--out",
        rust_out_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "clone notes exit");
    assert_eq!(rust_stderr, baseline_stderr, "clone notes stderr");
    let rust_json = rust_stdout.expect("rust clone notes stdout");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_out_str, "[OUT]"),
        scrub_path(
            baseline_stdout.expect("baseline clone notes stdout"),
            baseline_out_str,
            "[OUT]"
        ),
        "clone notes stdout"
    );

    let new_slide_uri = rust_json["newSlideUri"].as_str().expect("new slide URI");
    let notes_uri = rust_json["notesUri"].as_str().expect("cloned notes URI");
    assert_eq!(
        rust_json["destination"]["notesPartUri"],
        Value::String(notes_uri.to_string()),
        "destination readback should report cloned notes"
    );
    assert_eq!(rust_json["destination"]["notes"], true);

    let slide_rels = read_zip_string(&rust_out, &rels_part_for_uri(new_slide_uri));
    assert!(
        slide_rels.contains(PPTX_NOTES_REL_TYPE),
        "cloned slide notes rel"
    );
    assert!(
        slide_rels.contains(&relationship_target_between_parts(new_slide_uri, notes_uri)),
        "cloned slide should point at cloned notes part: {slide_rels}"
    );
    assert!(
        zip_entry_exists(&rust_out, notes_uri.trim_start_matches('/')),
        "cloned notes part should exist"
    );
    let notes_rels = read_zip_string(&rust_out, &rels_part_for_uri(notes_uri));
    assert!(
        notes_rels.contains(PPTX_SLIDE_REL_TYPE),
        "cloned notes backlink rel"
    );
    assert!(
        notes_rels.contains(&relationship_target_between_parts(notes_uri, new_slide_uri)),
        "cloned notes should link back to cloned slide: {notes_rels}"
    );

    assert_strict_validate_succeeds(rust_out_str, "clone notes output");
    assert_conformance_check_runs(rust_out_str, "clone notes output");
}

#[test]
fn pptx_import_merge_authoring_commands_match_rust_baseline_and_validate() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-import-merge-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("pptx import/merge temp dir");

    let target = "testdata/pptx/minimal-title/presentation.pptx";
    let notes_source = "testdata/pptx/slide-assembly-notes-media/presentation.pptx";
    let multi_source = "testdata/pptx/slide-assembly-multi/presentation.pptx";

    for (label, args) in [
        (
            "slides import-slide dry-run",
            vec![
                "--json",
                "pptx",
                "slides",
                "import-slide",
                target,
                "--source",
                target,
                "--slide",
                "1",
                "--dry-run",
            ],
        ),
        (
            "slides merge dry-run",
            vec![
                "--json",
                "pptx",
                "slides",
                "merge",
                target,
                target,
                "--dry-run",
            ],
        ),
        (
            "layouts import dry-run",
            vec![
                "--json",
                "pptx",
                "layouts",
                "import",
                target,
                "--source",
                target,
                "--layout",
                "1",
                "--dry-run",
            ],
        ),
        (
            "masters import dry-run",
            vec![
                "--json",
                "pptx",
                "masters",
                "import",
                target,
                "--source",
                target,
                "--master",
                "1",
                "--dry-run",
            ],
        ),
        (
            "slides import-slide missing source slide",
            vec![
                "--json",
                "pptx",
                "slides",
                "import-slide",
                target,
                "--source",
                target,
                "--slide",
                "99",
                "--dry-run",
            ],
        ),
        (
            "layouts import missing layout",
            vec![
                "--json",
                "pptx",
                "layouts",
                "import",
                target,
                "--source",
                target,
                "--layout",
                "99",
                "--dry-run",
            ],
        ),
    ] {
        assert_baseline_rust_json_match(&args, label);
    }

    let baseline_source = temp_dir.join("baseline-renamed-source.pptx");
    let rust_source = temp_dir.join("rust-renamed-source.pptx");
    let baseline_source_str = baseline_source
        .to_str()
        .expect("baseline renamed source path");
    let rust_source_str = rust_source.to_str().expect("rust renamed source path");
    let baseline_rename_args = [
        "--json",
        "pptx",
        "layouts",
        "rename",
        target,
        "--layout",
        "1",
        "--name",
        "WorkerOImportedTitle",
        "--out",
        baseline_source_str,
    ];
    let rust_rename_args = [
        "--json",
        "pptx",
        "layouts",
        "rename",
        target,
        "--layout",
        "1",
        "--name",
        "WorkerOImportedTitle",
        "--out",
        rust_source_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) =
        run_ooxml_baseline(&baseline_rename_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_rename_args);
    assert_eq!(rust_code, baseline_code, "renamed import source exit");
    assert_eq!(rust_stderr, baseline_stderr, "renamed import source stderr");
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust renamed source stdout"),
            rust_source_str,
            "[SOURCE]"
        ),
        scrub_path(
            baseline_stdout.expect("baseline renamed source stdout"),
            baseline_source_str,
            "[SOURCE]"
        ),
        "renamed import source stdout"
    );

    let baseline_import_slide = temp_dir.join("baseline-import-slide.pptx");
    let rust_import_slide = temp_dir.join("rust-import-slide.pptx");
    let baseline_import_slide_str = baseline_import_slide
        .to_str()
        .expect("baseline import-slide path");
    let rust_import_slide_str = rust_import_slide.to_str().expect("rust import-slide path");
    let baseline_args = [
        "--json",
        "pptx",
        "slides",
        "import-slide",
        target,
        "--source",
        notes_source,
        "--slide",
        "1",
        "--layout-policy",
        "import",
        "--theme-policy",
        "import",
        "--notes-policy",
        "clone",
        "--out",
        baseline_import_slide_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "slides",
        "import-slide",
        target,
        "--source",
        notes_source,
        "--slide",
        "1",
        "--layout-policy",
        "import",
        "--theme-policy",
        "import",
        "--notes-policy",
        "clone",
        "--out",
        rust_import_slide_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "slides import-slide saved exit");
    assert_eq!(
        rust_stderr, baseline_stderr,
        "slides import-slide saved stderr"
    );
    assert_eq!(
        rust_stdout.expect("rust import-slide stdout"),
        baseline_stdout.expect("baseline import-slide stdout"),
        "slides import-slide saved stdout"
    );
    assert_rust_baseline_match(&["--json", "validate", "--strict", rust_import_slide_str]);

    let baseline_merge = temp_dir.join("baseline-merge.pptx");
    let rust_merge = temp_dir.join("rust-merge.pptx");
    let baseline_merge_str = baseline_merge.to_str().expect("baseline merge path");
    let rust_merge_str = rust_merge.to_str().expect("rust merge path");
    let baseline_args = [
        "--json",
        "pptx",
        "slides",
        "merge",
        target,
        multi_source,
        "--layout-policy",
        "import",
        "--theme-policy",
        "import",
        "--out",
        baseline_merge_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "slides",
        "merge",
        target,
        multi_source,
        "--layout-policy",
        "import",
        "--theme-policy",
        "import",
        "--out",
        rust_merge_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "slides merge saved exit");
    assert_eq!(rust_stderr, baseline_stderr, "slides merge saved stderr");
    let rust_merge_json = rust_stdout.expect("rust merge stdout");
    assert_eq!(
        scrub_paths(
            rust_merge_json.clone(),
            &[("rust-merge.pptx", "[OUT]"), (rust_merge_str, "[OUT]")]
        ),
        scrub_paths(
            baseline_stdout.expect("baseline merge stdout"),
            &[
                ("baseline-merge.pptx", "[OUT]"),
                (baseline_merge_str, "[OUT]")
            ]
        ),
        "slides merge saved stdout"
    );
    assert_rust_baseline_match(&["--json", "validate", "--strict", rust_merge_str]);

    let baseline_layout = temp_dir.join("baseline-layout-import.pptx");
    let rust_layout = temp_dir.join("rust-layout-import.pptx");
    let baseline_layout_str = baseline_layout
        .to_str()
        .expect("baseline layout import path");
    let rust_layout_str = rust_layout.to_str().expect("rust layout import path");
    let baseline_args = [
        "--json",
        "pptx",
        "layouts",
        "import",
        target,
        "--source",
        baseline_source_str,
        "--layout",
        "WorkerOImportedTitle",
        "--theme-policy",
        "import",
        "--out",
        baseline_layout_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "layouts",
        "import",
        target,
        "--source",
        rust_source_str,
        "--layout",
        "WorkerOImportedTitle",
        "--theme-policy",
        "import",
        "--out",
        rust_layout_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "layouts import saved exit");
    assert_eq!(rust_stderr, baseline_stderr, "layouts import saved stderr");
    let rust_layout_json = rust_stdout.expect("rust layouts import stdout");
    assert_eq!(
        scrub_path(rust_layout_json.clone(), rust_layout_str, "[OUT]"),
        scrub_path(
            baseline_stdout.expect("baseline layouts import stdout"),
            baseline_layout_str,
            "[OUT]"
        ),
        "layouts import saved stdout"
    );
    assert_rust_emitted_ooxml_command_succeeds(&rust_layout_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_layout_json, "validateCommand");

    let baseline_master = temp_dir.join("baseline-master-import.pptx");
    let rust_master = temp_dir.join("rust-master-import.pptx");
    let baseline_master_str = baseline_master
        .to_str()
        .expect("baseline master import path");
    let rust_master_str = rust_master.to_str().expect("rust master import path");
    let baseline_args = [
        "--json",
        "pptx",
        "masters",
        "import",
        target,
        "--source",
        baseline_source_str,
        "--master",
        "1",
        "--theme-policy",
        "import",
        "--out",
        baseline_master_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "masters",
        "import",
        target,
        "--source",
        rust_source_str,
        "--master",
        "1",
        "--theme-policy",
        "import",
        "--out",
        rust_master_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "masters import saved exit");
    assert_eq!(rust_stderr, baseline_stderr, "masters import saved stderr");
    let rust_master_json = rust_stdout.expect("rust masters import stdout");
    assert_eq!(
        scrub_path(rust_master_json.clone(), rust_master_str, "[OUT]"),
        scrub_path(
            baseline_stdout.expect("baseline masters import stdout"),
            baseline_master_str,
            "[OUT]"
        ),
        "masters import saved stdout"
    );
    assert_rust_emitted_ooxml_command_succeeds(&rust_master_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_master_json, "validateCommand");
}

#[test]
fn pptx_slides_lifecycle_saved_dry_run_readback_and_errors_match_rust_baseline() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-slides-lifecycle-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("pptx slides lifecycle temp dir");

    let multi_fixture = "testdata/pptx/slide-assembly-multi/presentation.pptx";
    let notes_fixture = "testdata/pptx/notes-slide/presentation.pptx";

    let baseline_move = temp_dir.join("baseline-move.pptx");
    let rust_move = temp_dir.join("rust-move.pptx");
    let baseline_move_str = baseline_move.to_str().expect("baseline move path");
    let rust_move_str = rust_move.to_str().expect("rust move path");
    let baseline_move_args = [
        "--json",
        "pptx",
        "slides",
        "move",
        multi_fixture,
        "1",
        "3",
        "--out",
        baseline_move_str,
    ];
    let rust_move_args = [
        "--json",
        "pptx",
        "slides",
        "move",
        multi_fixture,
        "1",
        "3",
        "--out",
        rust_move_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_move_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_move_args);
    assert_eq!(rust_code, baseline_code, "slides move exit");
    assert_eq!(rust_stderr, baseline_stderr, "slides move stderr");
    let rust_move_json = rust_stdout.expect("rust slides move stdout");
    assert_eq!(
        scrub_path(rust_move_json.clone(), rust_move_str, "[OUT]"),
        scrub_path(
            baseline_stdout.expect("baseline slides move stdout"),
            baseline_move_str,
            "[OUT]"
        ),
        "slides move stdout"
    );
    assert!(
        baseline_move.exists(),
        "Rust baseline slides move output missing"
    );
    assert!(rust_move.exists(), "Rust slides move output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_move_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_succeeds(&rust_move_json, "slidesListCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_move_json, "validateCommand");
    assert_baseline_rust_json_match_with_path_scrub(
        &["--json", "pptx", "slides", "list", baseline_move_str],
        &["--json", "pptx", "slides", "list", rust_move_str],
        baseline_move_str,
        rust_move_str,
        "slides move readback list",
    );
    assert_baseline_rust_json_match_with_path_scrub(
        &["--json", "validate", "--strict", baseline_move_str],
        &["--json", "validate", "--strict", rust_move_str],
        baseline_move_str,
        rust_move_str,
        "slides move strict validate",
    );

    let move_dry_run = [
        "--json",
        "pptx",
        "slides",
        "move",
        multi_fixture,
        "1",
        "3",
        "--dry-run",
    ];
    assert_baseline_rust_json_match(&move_dry_run, "slides move dry-run");

    let move_no_op_dry_run = [
        "--json",
        "pptx",
        "slides",
        "move",
        multi_fixture,
        "2",
        "2",
        "--dry-run",
    ];
    assert_baseline_rust_json_match(&move_no_op_dry_run, "slides move no-op dry-run");

    let baseline_delete = temp_dir.join("baseline-delete.pptx");
    let rust_delete = temp_dir.join("rust-delete.pptx");
    let baseline_delete_str = baseline_delete.to_str().expect("baseline delete path");
    let rust_delete_str = rust_delete.to_str().expect("rust delete path");
    let baseline_delete_args = [
        "--json",
        "pptx",
        "slides",
        "delete",
        notes_fixture,
        "2",
        "--out",
        baseline_delete_str,
    ];
    let rust_delete_args = [
        "--json",
        "pptx",
        "slides",
        "delete",
        notes_fixture,
        "2",
        "--out",
        rust_delete_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) =
        run_ooxml_baseline(&baseline_delete_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_delete_args);
    assert_eq!(rust_code, baseline_code, "slides delete exit");
    assert_eq!(rust_stderr, baseline_stderr, "slides delete stderr");
    let rust_delete_json = rust_stdout.expect("rust slides delete stdout");
    assert_eq!(
        scrub_path(rust_delete_json.clone(), rust_delete_str, "[OUT]"),
        scrub_path(
            baseline_stdout.expect("baseline slides delete stdout"),
            baseline_delete_str,
            "[OUT]"
        ),
        "slides delete stdout"
    );
    assert!(
        baseline_delete.exists(),
        "Rust baseline slides delete output missing"
    );
    assert!(rust_delete.exists(), "Rust slides delete output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_delete_json, "slidesListCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_delete_json, "validateCommand");
    assert_baseline_rust_json_match_with_path_scrub(
        &["--json", "pptx", "slides", "list", baseline_delete_str],
        &["--json", "pptx", "slides", "list", rust_delete_str],
        baseline_delete_str,
        rust_delete_str,
        "slides delete readback list",
    );
    assert_baseline_rust_json_match_with_path_scrub(
        &["--json", "validate", "--strict", baseline_delete_str],
        &["--json", "validate", "--strict", rust_delete_str],
        baseline_delete_str,
        rust_delete_str,
        "slides delete strict validate",
    );

    let delete_dry_run = [
        "--json",
        "pptx",
        "slides",
        "delete",
        notes_fixture,
        "2",
        "--dry-run",
    ];
    assert_baseline_rust_json_match(&delete_dry_run, "slides delete dry-run");

    let baseline_reorder = temp_dir.join("baseline-reorder.pptx");
    let rust_reorder = temp_dir.join("rust-reorder.pptx");
    let baseline_reorder_str = baseline_reorder.to_str().expect("baseline reorder path");
    let rust_reorder_str = rust_reorder.to_str().expect("rust reorder path");
    let baseline_reorder_args = [
        "--json",
        "pptx",
        "slides",
        "reorder",
        multi_fixture,
        "3,1,2,4,5",
        "--out",
        baseline_reorder_str,
    ];
    let rust_reorder_args = [
        "--json",
        "pptx",
        "slides",
        "reorder",
        multi_fixture,
        "3,1,2,4,5",
        "--out",
        rust_reorder_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) =
        run_ooxml_baseline(&baseline_reorder_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_reorder_args);
    assert_eq!(rust_code, baseline_code, "slides reorder exit");
    assert_eq!(rust_stderr, baseline_stderr, "slides reorder stderr");
    let rust_reorder_json = rust_stdout.expect("rust slides reorder stdout");
    assert_eq!(
        scrub_path(rust_reorder_json.clone(), rust_reorder_str, "[OUT]"),
        scrub_path(
            baseline_stdout.expect("baseline slides reorder stdout"),
            baseline_reorder_str,
            "[OUT]"
        ),
        "slides reorder stdout"
    );
    assert!(
        baseline_reorder.exists(),
        "Rust baseline slides reorder output missing"
    );
    assert!(rust_reorder.exists(), "Rust slides reorder output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_reorder_json, "slidesListCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_reorder_json, "validateCommand");
    assert_baseline_rust_json_match_with_path_scrub(
        &["--json", "pptx", "slides", "list", baseline_reorder_str],
        &["--json", "pptx", "slides", "list", rust_reorder_str],
        baseline_reorder_str,
        rust_reorder_str,
        "slides reorder readback list",
    );
    assert_baseline_rust_json_match_with_path_scrub(
        &["--json", "validate", "--strict", baseline_reorder_str],
        &["--json", "validate", "--strict", rust_reorder_str],
        baseline_reorder_str,
        rust_reorder_str,
        "slides reorder strict validate",
    );

    let reorder_dry_run = [
        "--json",
        "pptx",
        "slides",
        "reorder",
        multi_fixture,
        "3,1,2,4,5",
        "--dry-run",
    ];
    assert_baseline_rust_json_match(&reorder_dry_run, "slides reorder dry-run");

    for (label, args) in [
        (
            "slides move from out-of-range",
            vec![
                "--json",
                "pptx",
                "slides",
                "move",
                multi_fixture,
                "9",
                "1",
                "--dry-run",
            ],
        ),
        (
            "slides move to out-of-range",
            vec![
                "--json",
                "pptx",
                "slides",
                "move",
                multi_fixture,
                "1",
                "9",
                "--dry-run",
            ],
        ),
        (
            "slides delete out-of-range",
            vec![
                "--json",
                "pptx",
                "slides",
                "delete",
                notes_fixture,
                "9",
                "--dry-run",
            ],
        ),
        (
            "slides reorder wrong length",
            vec![
                "--json",
                "pptx",
                "slides",
                "reorder",
                multi_fixture,
                "3,1,2",
                "--dry-run",
            ],
        ),
        (
            "slides reorder duplicate",
            vec![
                "--json",
                "pptx",
                "slides",
                "reorder",
                multi_fixture,
                "1,1,2,3,4",
                "--dry-run",
            ],
        ),
        (
            "slides reorder out-of-range",
            vec![
                "--json",
                "pptx",
                "slides",
                "reorder",
                multi_fixture,
                "9,1,2,3,4",
                "--dry-run",
            ],
        ),
    ] {
        assert_baseline_rust_json_match(&args, label);
    }
}

fn assert_baseline_rust_json_match(args: &[&str], label: &str) {
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(args);
    assert_eq!(rust_code, baseline_code, "{label} exit");
    assert_eq!(rust_stdout, baseline_stdout, "{label} stdout");
    assert_eq!(rust_stderr, baseline_stderr, "{label} stderr");
}

fn assert_baseline_rust_json_match_with_path_scrub(
    baseline_args: &[&str],
    rust_args: &[&str],
    baseline_path: &str,
    rust_path: &str,
    label: &str,
) {
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(rust_args);
    assert_eq!(rust_code, baseline_code, "{label} exit");
    assert_eq!(
        scrub_path(rust_stdout.expect("rust stdout"), rust_path, "[OUT]"),
        scrub_path(
            baseline_stdout.expect("baseline stdout"),
            baseline_path,
            "[OUT]"
        ),
        "{label} stdout"
    );
    assert_eq!(rust_stderr, baseline_stderr, "{label} stderr");
}

#[test]
fn pptx_text_set_saved_readback_dry_run_hyperlink_and_errors_match_rust_baseline() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-text-set-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("pptx text set temp dir");

    let fixture = "testdata/pptx/title-content/presentation.pptx";
    let baseline_out = temp_dir.join("baseline-text-set.pptx");
    let rust_out = temp_dir.join("rust-text-set.pptx");
    let baseline_out_str = baseline_out.to_str().expect("baseline text set path");
    let rust_out_str = rust_out.to_str().expect("rust text set path");

    let baseline_args = [
        "--json",
        "pptx",
        "text",
        "set",
        fixture,
        "--slide",
        "2",
        "--target",
        "title",
        "--paragraph",
        "0",
        "--run-index",
        "0",
        "--bold",
        "--italic",
        "--font-size",
        "28",
        "--color",
        "ff0000",
        "--font-family",
        "Arial",
        "--out",
        baseline_out_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "text",
        "set",
        fixture,
        "--slide",
        "2",
        "--target",
        "title",
        "--paragraph",
        "0",
        "--run-index",
        "0",
        "--bold",
        "--italic",
        "--font-size",
        "28",
        "--color",
        "ff0000",
        "--font-family",
        "Arial",
        "--out",
        rust_out_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "text set saved exit");
    assert_eq!(rust_stderr, baseline_stderr, "text set saved stderr");
    let rust_json = rust_stdout.expect("rust text set stdout");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_out_str, "[OUT]"),
        scrub_path(
            baseline_stdout.expect("baseline text set stdout"),
            baseline_out_str,
            "[OUT]"
        ),
        "text set saved stdout"
    );
    assert!(
        baseline_out.exists(),
        "Rust baseline text set output missing"
    );
    assert!(rust_out.exists(), "Rust text set output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    let (baseline_read_code, baseline_read_stdout, baseline_read_stderr) = run_ooxml_baseline(&[
        "--json",
        "pptx",
        "shapes",
        "get",
        baseline_out_str,
        "--slide",
        "2",
        "--target",
        "title",
        "--include-text",
    ]);
    let (rust_read_code, rust_read_stdout, rust_read_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "shapes",
        "get",
        rust_out_str,
        "--slide",
        "2",
        "--target",
        "title",
        "--include-text",
    ]);
    assert_eq!(rust_read_code, baseline_read_code, "text set readback exit");
    assert_eq!(
        rust_read_stderr, baseline_read_stderr,
        "text set readback stderr"
    );
    assert_eq!(
        scrub_path(
            rust_read_stdout.expect("rust text set readback"),
            rust_out_str,
            "[OUT]"
        ),
        scrub_path(
            baseline_read_stdout.expect("baseline text set readback"),
            baseline_out_str,
            "[OUT]"
        ),
        "text set readback stdout"
    );

    let dry_run_args = [
        "--json",
        "pptx",
        "text",
        "set",
        fixture,
        "--slide",
        "2",
        "--target",
        "title",
        "--paragraph",
        "0",
        "--run-index",
        "0",
        "--underline",
        "single",
        "--dry-run",
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, baseline_code, "text set dry-run exit");
    assert_eq!(rust_stderr, baseline_stderr, "text set dry-run stderr");
    assert_eq!(
        rust_stdout.expect("rust text set dry-run"),
        baseline_stdout.expect("baseline text set dry-run"),
        "text set dry-run stdout"
    );

    let baseline_hyper = temp_dir.join("baseline-hyperlink.pptx");
    let rust_hyper = temp_dir.join("rust-hyperlink.pptx");
    let baseline_hyper_str = baseline_hyper.to_str().expect("baseline hyperlink path");
    let rust_hyper_str = rust_hyper.to_str().expect("rust hyperlink path");
    let baseline_hyper_args = [
        "--json",
        "pptx",
        "text",
        "set",
        fixture,
        "--slide",
        "2",
        "--target",
        "title",
        "--paragraph",
        "0",
        "--run-index",
        "0",
        "--hyperlink",
        "https://example.com",
        "--out",
        baseline_hyper_str,
    ];
    let rust_hyper_args = [
        "--json",
        "pptx",
        "text",
        "set",
        fixture,
        "--slide",
        "2",
        "--target",
        "title",
        "--paragraph",
        "0",
        "--run-index",
        "0",
        "--hyperlink",
        "https://example.com",
        "--out",
        rust_hyper_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) =
        run_ooxml_baseline(&baseline_hyper_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_hyper_args);
    assert_eq!(rust_code, baseline_code, "text set hyperlink exit");
    assert_eq!(rust_stderr, baseline_stderr, "text set hyperlink stderr");
    let rust_hyper_json = rust_stdout.expect("rust hyperlink stdout");
    assert_eq!(
        scrub_path(rust_hyper_json.clone(), rust_hyper_str, "[OUT]"),
        scrub_path(
            baseline_stdout.expect("baseline hyperlink stdout"),
            baseline_hyper_str,
            "[OUT]"
        ),
        "text set hyperlink stdout"
    );
    assert_rust_emitted_ooxml_command_exits_zero(&rust_hyper_json, "validateCommand");

    for (label, args) in [
        (
            "text set paragraph out of range",
            vec![
                "--json",
                "pptx",
                "text",
                "set",
                fixture,
                "--slide",
                "2",
                "--target",
                "title",
                "--paragraph",
                "99",
                "--bold",
                "--dry-run",
            ],
        ),
        (
            "text set run index out of range",
            vec![
                "--json",
                "pptx",
                "text",
                "set",
                fixture,
                "--slide",
                "2",
                "--target",
                "title",
                "--paragraph",
                "0",
                "--run-index",
                "99",
                "--bold",
                "--dry-run",
            ],
        ),
        (
            "text set invalid color",
            vec![
                "--json",
                "pptx",
                "text",
                "set",
                fixture,
                "--slide",
                "2",
                "--target",
                "title",
                "--paragraph",
                "0",
                "--color",
                "ZZZZZZ",
                "--dry-run",
            ],
        ),
        (
            "text set mutually exclusive flags",
            vec![
                "--json",
                "pptx",
                "text",
                "set",
                fixture,
                "--slide",
                "2",
                "--target",
                "title",
                "--paragraph",
                "0",
                "--bold",
                "--remove-bold",
                "--dry-run",
            ],
        ),
        (
            "text set no styling flags",
            vec![
                "--json",
                "pptx",
                "text",
                "set",
                fixture,
                "--slide",
                "2",
                "--target",
                "title",
                "--paragraph",
                "0",
                "--dry-run",
            ],
        ),
        (
            "text set unknown target",
            vec![
                "--json",
                "pptx",
                "text",
                "set",
                fixture,
                "--slide",
                "2",
                "--target",
                "nonexistent",
                "--paragraph",
                "0",
                "--bold",
                "--dry-run",
            ],
        ),
    ] {
        assert_baseline_rust_json_match(&args, label);
    }
}

#[test]
fn pptx_fields_inspect_set_readback_dry_run_and_errors_match_rust_baseline() {
    let header_footer_fixture = "testdata/pptx/header-footer/presentation.pptx";
    let title_content_fixture = "testdata/pptx/title-content/presentation.pptx";

    assert_baseline_rust_json_match(
        &["--json", "pptx", "fields", "inspect", header_footer_fixture],
        "fields inspect header-footer",
    );
    assert_baseline_rust_json_match(
        &["--json", "pptx", "fields", "inspect", title_content_fixture],
        "fields inspect no-header-footer",
    );

    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-fields-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("pptx fields temp dir");

    let rust_out = temp_dir.join("rust-fields-set.pptx");
    let rust_out_str = rust_out.to_str().expect("rust fields set path");
    let rust_args = [
        "--json",
        "pptx",
        "fields",
        "set",
        header_footer_fixture,
        "--footer",
        "Confidential",
        "--show-slide-number=false",
        "--date-format",
        "date-only",
        "--out",
        rust_out_str,
    ];
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, 0, "fields set saved exit");
    assert_eq!(rust_stderr, None, "fields set saved stderr");
    let rust_json = rust_stdout.expect("rust fields set stdout");
    let scrubbed = scrub_path(rust_json.clone(), rust_out_str, "[OUT]");
    assert_eq!(scrubbed["output"], Value::String("[OUT]".to_string()));
    assert_eq!(
        scrubbed["footerText"],
        Value::String("Confidential".to_string())
    );
    assert_eq!(scrubbed["footerPlaceholdersUpdated"], Value::from(2));
    assert_eq!(scrubbed["footerPlaceholdersCreated"], Value::from(1));
    assert_eq!(scrubbed.get("slidesWithoutFooterPlaceholder"), None);
    assert!(rust_out.exists(), "Rust fields set output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    let (rust_read_code, rust_read_stdout, rust_read_stderr) =
        run_ooxml(&["--json", "pptx", "fields", "inspect", rust_out_str]);
    assert_eq!(rust_read_code, 0, "fields readback exit");
    assert_eq!(rust_read_stderr, None, "fields readback stderr");
    let readback = rust_read_stdout.expect("rust fields readback");
    let slides = readback["slides"]
        .as_array()
        .expect("fields readback slides");
    assert_eq!(slides.len(), 2, "header-footer fixture slide count");
    for slide in slides {
        assert_eq!(
            slide["footerPlaceholder"]["text"],
            Value::String("Confidential".to_string()),
            "fields readback footer text: {slide:?}"
        );
    }

    let (dry_code, dry_stdout, dry_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "fields",
        "set",
        header_footer_fixture,
        "--footer",
        "Confidential",
        "--show-slide-number=false",
        "--date-format",
        "date-only",
        "--dry-run",
    ]);
    assert_eq!(dry_code, 0, "fields set dry-run exit");
    assert_eq!(dry_stderr, None, "fields set dry-run stderr");
    let dry_result = dry_stdout.expect("fields set dry-run stdout");
    assert_eq!(dry_result["footerPlaceholdersUpdated"], Value::from(2));
    assert_eq!(dry_result["footerPlaceholdersCreated"], Value::from(1));
    assert_eq!(dry_result.get("slidesWithoutFooterPlaceholder"), None);

    for (label, args) in [
        (
            "fields set creates master hf dry-run",
            vec![
                "--json",
                "pptx",
                "fields",
                "set",
                title_content_fixture,
                "--show-footer=false",
                "--dry-run",
            ],
        ),
        (
            "fields set no flags",
            vec![
                "--json",
                "pptx",
                "fields",
                "set",
                header_footer_fixture,
                "--dry-run",
            ],
        ),
        (
            "fields set invalid date format",
            vec![
                "--json",
                "pptx",
                "fields",
                "set",
                header_footer_fixture,
                "--date-format",
                "bogus",
                "--dry-run",
            ],
        ),
        (
            "fields inspect unsupported xlsx",
            vec![
                "--json",
                "pptx",
                "fields",
                "inspect",
                "testdata/xlsx/minimal-workbook/workbook.xlsx",
            ],
        ),
        (
            "fields set unsupported xlsx",
            vec![
                "--json",
                "pptx",
                "fields",
                "set",
                "testdata/xlsx/minimal-workbook/workbook.xlsx",
                "--footer",
                "Confidential",
                "--dry-run",
            ],
        ),
    ] {
        assert_baseline_rust_json_match(&args, label);
    }
}

#[test]
fn pptx_fields_set_synthesizes_missing_footer_placeholders() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-footer-synthesis-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("pptx footer synthesis temp dir");
    let out = temp_dir.join("footer-visible.pptx");
    let out_str = out.to_str().expect("footer output path");

    let (code, stdout, stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "fields",
        "set",
        "testdata/pptx/title-content/presentation.pptx",
        "--footer",
        "Confidential",
        "--show-footer=true",
        "--out",
        out_str,
    ]);
    assert_eq!(code, 0, "fields set footer synthesis exit");
    assert_eq!(stderr, None, "fields set footer synthesis stderr");
    let result = stdout.expect("fields set footer synthesis stdout");
    assert_eq!(result["footerPlaceholdersCreated"], Value::from(2));
    assert_eq!(result.get("slidesWithoutFooterPlaceholder"), None);
    assert_rust_emitted_ooxml_command_exits_zero(&result, "validateCommand");

    let slide_xml = read_zip_string(&out, "ppt/slides/slide1.xml");
    assert!(
        slide_xml.contains(r#"type="ftr""#),
        "synthesized slide XML should contain a footer placeholder: {slide_xml}"
    );
    assert!(
        slide_xml.contains("Confidential"),
        "synthesized slide XML should contain footer text: {slide_xml}"
    );

    let (inspect_code, inspect_stdout, inspect_stderr) =
        run_ooxml(&["--json", "pptx", "fields", "inspect", out_str]);
    assert_eq!(inspect_code, 0, "footer synthesis inspect exit");
    assert_eq!(inspect_stderr, None, "footer synthesis inspect stderr");
    let inspect = inspect_stdout.expect("footer synthesis inspect stdout");
    let slides = inspect["slides"].as_array().expect("inspect slides");
    assert_eq!(slides.len(), 2, "title-content fixture slide count");
    for slide in slides {
        assert_eq!(
            slide["footerPlaceholder"]["text"],
            Value::String("Confidential".to_string()),
            "slide footer placeholder should be inspectable: {slide:?}"
        );
    }
}

#[test]
fn pptx_theme_update_deck_readback_dry_run_and_errors_match_rust_baseline() {
    let fixture = "testdata/pptx/multi-layout/presentation.pptx";
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-theme-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("pptx theme temp dir");

    assert_baseline_rust_json_match(
        &[
            "--json",
            "pptx",
            "theme",
            "update",
            fixture,
            "--color",
            "accent1=FF0000",
            "--major-font",
            "Georgia",
            "--minor-font",
            "Verdana",
            "--dry-run",
        ],
        "theme update dry-run",
    );

    let baseline_out = temp_dir.join("baseline-theme-update.pptx");
    let rust_out = temp_dir.join("rust-theme-update.pptx");
    let baseline_out_str = baseline_out.to_str().expect("baseline theme update path");
    let rust_out_str = rust_out.to_str().expect("rust theme update path");
    let baseline_args = [
        "--json",
        "pptx",
        "theme",
        "update",
        fixture,
        "--color",
        "accent1=FF0000",
        "--major-font",
        "Georgia",
        "--minor-font",
        "Verdana",
        "--out",
        baseline_out_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "theme",
        "update",
        fixture,
        "--color",
        "accent1=FF0000",
        "--major-font",
        "Georgia",
        "--minor-font",
        "Verdana",
        "--out",
        rust_out_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "theme update saved exit");
    assert_eq!(rust_stderr, baseline_stderr, "theme update saved stderr");
    assert_eq!(
        rust_stdout.expect("rust theme update stdout"),
        baseline_stdout.expect("baseline theme update stdout"),
        "theme update saved stdout"
    );
    assert!(
        baseline_out.exists(),
        "Rust baseline theme update output missing"
    );
    assert!(rust_out.exists(), "Rust theme update output missing");

    let (baseline_read_code, baseline_read_stdout, baseline_read_stderr) = run_ooxml_baseline(&[
        "--json",
        "pptx",
        "masters",
        "show",
        baseline_out_str,
        "--master",
        "1",
    ]);
    let (rust_read_code, rust_read_stdout, rust_read_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "masters",
        "show",
        rust_out_str,
        "--master",
        "1",
    ]);
    assert_eq!(rust_read_code, baseline_read_code, "theme readback exit");
    assert_eq!(
        rust_read_stderr, baseline_read_stderr,
        "theme readback stderr"
    );
    assert_eq!(
        rust_read_stdout.expect("rust theme readback"),
        baseline_read_stdout.expect("baseline theme readback"),
        "theme readback stdout"
    );

    for (label, args) in [
        (
            "theme update no updates",
            vec!["--json", "pptx", "theme", "update", fixture, "--dry-run"],
        ),
        (
            "theme update invalid color format",
            vec![
                "--json",
                "pptx",
                "theme",
                "update",
                fixture,
                "--color",
                "accent1",
                "--dry-run",
            ],
        ),
        (
            "theme update invalid color name",
            vec![
                "--json",
                "pptx",
                "theme",
                "update",
                fixture,
                "--color",
                "bad=FF0000",
                "--dry-run",
            ],
        ),
        (
            "theme update invalid hex",
            vec![
                "--json",
                "pptx",
                "theme",
                "update",
                fixture,
                "--color",
                "accent1=ZZZZZZ",
                "--dry-run",
            ],
        ),
        (
            "theme update slide color oracle error",
            vec![
                "--json",
                "pptx",
                "theme",
                "update",
                fixture,
                "--mode",
                "slide",
                "--slide",
                "1",
                "--color",
                "accent1=FF0000",
                "--dry-run",
            ],
        ),
        (
            "theme update slide font unsupported",
            vec![
                "--json",
                "pptx",
                "theme",
                "update",
                fixture,
                "--mode",
                "slide",
                "--slide",
                "1",
                "--major-font",
                "Georgia",
                "--dry-run",
            ],
        ),
        (
            "theme update unsupported xlsx",
            vec![
                "--json",
                "pptx",
                "theme",
                "update",
                "testdata/xlsx/minimal-workbook/workbook.xlsx",
                "--color",
                "accent1=FF0000",
                "--dry-run",
            ],
        ),
    ] {
        assert_baseline_rust_json_match(&args, label);
    }
}

include!("pptx/tables.rs");

include!("pptx/comments.rs");

fn pptx_replace_text_source_sheet_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:B2"/>
  <sheetData>
    <row r="1"><c r="A1" t="inlineStr"><is><t>Region</t></is></c><c r="B1" t="inlineStr"><is><t>Amount</t></is></c></row>
    <row r="2"><c r="A2" t="inlineStr"><is><t>North</t></is></c><c r="B2"><v>42</v></c></row>
  </sheetData>
</worksheet>"#
}

fn pptx_replace_text_map_source_sheet_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:C3"/>
  <sheetData>
    <row r="1"><c r="A1" t="inlineStr"><is><t>slide</t></is></c><c r="B1" t="inlineStr"><is><t>target</t></is></c><c r="C1" t="inlineStr"><is><t>text</t></is></c></row>
    <row r="2"><c r="A2"><v>1</v></c><c r="B2" t="inlineStr"><is><t>title</t></is></c><c r="C2" t="inlineStr"><is><t>Range Title</t></is></c></row>
    <row r="3"><c r="A3"><v>2</v></c><c r="B3" t="inlineStr"><is><t>body</t></is></c><c r="C3" t="inlineStr"><is><t>Range Body</t></is></c></row>
  </sheetData>
</worksheet>"#
}

fn pptx_update_source_sheet_xml_4x4() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:D4"/>
  <sheetData>
    <row r="1"><c r="A1"><f>SUM(B1:C1)</f><v>7</v></c><c r="B1" t="inlineStr"><is><t>Header B</t></is></c><c r="C1" t="inlineStr"><is><t>Header C</t></is></c><c r="D1" t="inlineStr"><is><t>D</t></is></c></row>
    <row r="2"><c r="A2" t="inlineStr"><is><t>North</t></is></c><c r="B2"><v>42</v></c><c r="C2" t="inlineStr"><is><t>ok</t></is></c><c r="D2" t="inlineStr"><is><t>H</t></is></c></row>
    <row r="3"><c r="A3" t="inlineStr"><is><t>South</t></is></c><c r="B3"><v>55</v></c><c r="C3" t="inlineStr"><is><t>done</t></is></c><c r="D3" t="inlineStr"><is><t>L</t></is></c></row>
    <row r="4"><c r="A4" t="inlineStr"><is><t>M</t></is></c><c r="B4" t="inlineStr"><is><t>N</t></is></c><c r="C4" t="inlineStr"><is><t>O</t></is></c><c r="D4" t="inlineStr"><is><t>P</t></is></c></row>
  </sheetData>
</worksheet>"#
}

fn write_pptx_text_map_table_xlsx(dest: &Path) {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).expect("fixture parent");
    }
    let output = File::create(dest).expect("create pptx text map table xlsx");
    let mut writer = ZipWriter::new(output);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    write_zip_string(
        &mut writer,
        options,
        "[Content_Types].xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
  <Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
  <Override PartName="/xl/tables/table1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.table+xml"/>
</Types>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "_rels/.rels",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "xl/workbook.xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets>
    <sheet name="Data" sheetId="1" r:id="rId1"/>
  </sheets>
</workbook>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "xl/_rels/workbook.xml.rels",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
</Relationships>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "xl/worksheets/sheet1.xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <dimension ref="A1:C2"/>
  <sheetData>
    <row r="1"><c r="A1" t="inlineStr"><is><t>slide</t></is></c><c r="B1" t="inlineStr"><is><t>target</t></is></c><c r="C1" t="inlineStr"><is><t>text</t></is></c></row>
    <row r="2"><c r="A2"><v>1</v></c><c r="B2" t="inlineStr"><is><t>title</t></is></c><c r="C2" t="inlineStr"><is><t>Table Title</t></is></c></row>
  </sheetData>
  <tableParts count="1"><tablePart r:id="rId1"/></tableParts>
</worksheet>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "xl/worksheets/_rels/sheet1.xml.rels",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/table" Target="../tables/table1.xml"/>
</Relationships>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "xl/tables/table1.xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<table xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" id="1" name="TextMap" displayName="TextMap" ref="A1:C2" headerRowCount="1" totalsRowShown="0">
  <autoFilter ref="A1:C2"/>
  <tableColumns count="3">
    <tableColumn id="1" name="slide"/>
    <tableColumn id="2" name="target"/>
    <tableColumn id="3" name="text"/>
  </tableColumns>
  <tableStyleInfo name="TableStyleMedium2" showFirstColumn="0" showLastColumn="0" showRowStripes="1" showColumnStripes="0"/>
</table>"#,
    );
    writer.finish().expect("finish pptx text map table xlsx");
}
