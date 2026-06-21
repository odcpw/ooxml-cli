// PPTX frozen mutation/render/verify contract tests live here while shared
// baseline and process helpers remain in the parent integration test crate.
use super::*;

const PPTX_NOTES_REL_TYPE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesSlide";
const PPTX_SLIDE_REL_TYPE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide";

fn assert_export_dirs_match(go_dir: &std::path::Path, rust_dir: &std::path::Path) {
    let go_files = sorted_export_files(go_dir);
    let rust_files = sorted_export_files(rust_dir);
    assert_eq!(rust_files, go_files, "exported file set");
    for relative in go_files {
        let go_bytes = std::fs::read(export_path(go_dir, &relative)).unwrap_or_else(|err| {
            panic!("read Go exported artifact {relative}: {err}");
        });
        let rust_bytes = std::fs::read(export_path(rust_dir, &relative)).unwrap_or_else(|err| {
            panic!("read Rust exported artifact {relative}: {err}");
        });
        assert_eq!(
            rust_bytes, go_bytes,
            "exported artifact bytes for {relative}"
        );
    }
}

fn export_path(root: &std::path::Path, relative: &str) -> std::path::PathBuf {
    let mut path = root.to_path_buf();
    for part in relative.split('/') {
        path.push(part);
    }
    path
}

fn sorted_export_files(root: &std::path::Path) -> Vec<String> {
    let mut files = Vec::new();
    collect_export_files(root, root, &mut files);
    files.sort();
    files
}

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

fn run_go_ooxml_raw(args: &[&str]) -> (i32, String, String) {
    let output = std::process::Command::new(go_ooxml_binary())
        .args(args)
        .env("GOCACHE", go_cache_dir())
        .output()
        .expect("run Go ooxml oracle raw");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8(output.stdout).expect("Go stdout utf8"),
        String::from_utf8(output.stderr).expect("Go stderr utf8"),
    )
}

fn parse_raw_json(text: &str) -> Value {
    serde_json::from_str(text.trim()).unwrap_or_else(|err| {
        panic!("invalid raw JSON {err}: {text}");
    })
}

fn collect_export_files(
    root: &std::path::Path,
    current: &std::path::Path,
    files: &mut Vec<String>,
) {
    for entry in std::fs::read_dir(current).unwrap_or_else(|err| {
        panic!("read export dir {}: {err}", current.display());
    }) {
        let path = entry.expect("export dir entry").path();
        if path.is_dir() {
            collect_export_files(root, &path, files);
        } else {
            let relative = path.strip_prefix(root).expect("relative export path");
            files.push(
                relative
                    .components()
                    .map(|part| part.as_os_str().to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("/"),
            );
        }
    }
}

#[test]
fn pptx_translate_export_matches_go_oracle() {
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
        let (go_code, go_stdout, go_stderr) = run_go_ooxml(&args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&args);
        assert_eq!(rust_code, go_code, "translate export exit for {args:?}");
        assert_eq!(
            rust_stderr, go_stderr,
            "translate export stderr for {args:?}"
        );
        assert_eq!(
            rust_stdout.map(scrub_translation_exported_at),
            go_stdout.map(scrub_translation_exported_at),
            "translate export stdout for {args:?}"
        );
    }
}

#[test]
fn pptx_translate_apply_saved_stale_and_errors_match_go_oracle() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-translate-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("translate temp dir");

    let go_input = temp_dir.join("go-input.pptx");
    let rust_input = temp_dir.join("rust-input.pptx");
    let xlsx_input = temp_dir.join("input.xlsx");
    std::fs::copy("testdata/pptx/minimal-title/presentation.pptx", &go_input)
        .expect("copy Go translate fixture");
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

    let go_out = temp_dir.join("go-out.pptx");
    let rust_out = temp_dir.join("rust-out.pptx");
    let go_input_str = go_input.to_str().expect("go input");
    let rust_input_str = rust_input.to_str().expect("rust input");
    let xlsx_input_str = xlsx_input.to_str().expect("xlsx input");
    let manifest_str = manifest_path.to_str().expect("manifest path");
    let stale_manifest_str = stale_manifest_path.to_str().expect("stale manifest path");
    let invalid_manifest_str = invalid_manifest_path
        .to_str()
        .expect("invalid manifest path");
    let go_out_str = go_out.to_str().expect("go output");
    let rust_out_str = rust_out.to_str().expect("rust output");

    let go_args = [
        "--json",
        "pptx",
        "translate",
        "apply",
        go_input_str,
        manifest_str,
        "--output",
        go_out_str,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "translate apply saved exit");
    assert_eq!(rust_stderr, go_stderr, "translate apply saved stderr");
    assert_eq!(rust_stdout, go_stdout, "translate apply saved stdout");
    assert!(go_out.exists(), "Go translate output missing");
    assert!(rust_out.exists(), "Rust translate output missing");

    let (go_code, go_stdout, go_stderr) =
        run_go_ooxml(&["--json", "pptx", "extract", "text", go_out_str]);
    let (rust_code, rust_stdout, rust_stderr) =
        run_ooxml(&["--json", "pptx", "extract", "text", rust_out_str]);
    assert_eq!(rust_code, go_code, "translate apply readback exit");
    assert_eq!(rust_stderr, go_stderr, "translate apply readback stderr");
    assert_eq!(
        scrub_paths(
            rust_stdout.expect("rust translate readback"),
            &[(rust_out_str, "[OUT]")]
        ),
        scrub_paths(
            go_stdout.expect("go translate readback"),
            &[(go_out_str, "[OUT]")]
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
        let go_stale_out = temp_dir.join(format!("go-stale-{stale_label}.pptx"));
        let rust_stale_out_str = rust_stale_out.to_str().expect("rust stale output");
        let go_stale_out_str = go_stale_out.to_str().expect("go stale output");
        let mut go_args = vec![
            "--json",
            "pptx",
            "translate",
            "apply",
            go_input_str,
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
            go_args.extend(["--stale", mode]);
            rust_args.extend(["--stale", mode]);
        }
        go_args.extend(["--output", go_stale_out_str]);
        rust_args.extend(["--output", rust_stale_out_str]);

        let (go_code, go_stdout, go_stderr) = run_go_ooxml_raw(&go_args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml_raw(&rust_args);
        assert_eq!(rust_code, go_code, "translate stale {stale_mode:?} exit");
        assert_eq!(
            rust_stderr, go_stderr,
            "translate stale {stale_mode:?} stderr"
        );
        if go_stdout.trim().is_empty() || rust_stdout.trim().is_empty() {
            assert_eq!(
                rust_stdout, go_stdout,
                "translate stale {stale_mode:?} stdout"
            );
        } else {
            assert_eq!(
                parse_raw_json(&rust_stdout),
                parse_raw_json(&go_stdout),
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
        let (go_code, go_stdout, go_stderr) = run_go_ooxml(&args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&args);
        assert_eq!(
            rust_code, go_code,
            "translate apply error exit for {args:?}"
        );
        assert_eq!(
            rust_stderr, go_stderr,
            "translate apply error stderr for {args:?}"
        );
        assert_eq!(
            rust_stdout, go_stdout,
            "translate apply error stdout for {args:?}"
        );
    }
}

#[test]
fn pptx_add_textbox_saved_readback_dry_run_and_errors_match_go_oracle() {
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, go_code, "add-textbox dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "add-textbox dry-run stderr");
    assert_eq!(
        scrub_created_at(rust_stdout.expect("rust add-textbox dry-run")),
        scrub_created_at(go_stdout.expect("go add-textbox dry-run")),
        "add-textbox dry-run stdout"
    );

    let go_out = temp_dir.join("go-add-textbox.pptx");
    let rust_out = temp_dir.join("rust-add-textbox.pptx");
    let go_out_str = go_out.to_str().expect("go add-textbox output");
    let rust_out_str = rust_out.to_str().expect("rust add-textbox output");
    let go_args = [
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
        go_out_str,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "add-textbox saved exit");
    assert_eq!(rust_stderr, go_stderr, "add-textbox saved stderr");
    let rust_json = rust_stdout.expect("rust add-textbox saved");
    assert_eq!(
        scrub_created_at(scrub_path(rust_json.clone(), rust_out_str, "[OUT]")),
        scrub_created_at(scrub_path(
            go_stdout.expect("go add-textbox saved"),
            go_out_str,
            "[OUT]"
        )),
        "add-textbox saved stdout"
    );
    assert!(go_out.exists(), "Go add-textbox output missing");
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
        assert_go_rust_json_match(&args, "add-textbox representative error");
    }
}

#[test]
fn pptx_place_image_saved_readback_dry_run_and_errors_match_go_oracle() {
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, go_code, "place image dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "place image dry-run stderr");
    assert_eq!(
        rust_stdout.expect("rust place image dry-run"),
        go_stdout.expect("go place image dry-run"),
        "place image dry-run stdout"
    );

    let go_out = temp_dir.join("go-place-image.pptx");
    let rust_out = temp_dir.join("rust-place-image.pptx");
    let go_out_str = go_out.to_str().expect("go place image output");
    let rust_out_str = rust_out.to_str().expect("rust place image output");
    let go_args = [
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
        go_out_str,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "place image saved exit");
    assert_eq!(rust_stderr, go_stderr, "place image saved stderr");
    let rust_json = rust_stdout.expect("rust place image saved");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_out_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go place image saved"),
            go_out_str,
            "[OUT]"
        ),
        "place image saved stdout"
    );
    assert!(go_out.exists(), "Go place image output missing");
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
        assert_go_rust_json_match(&args, "place image representative error");
    }
}

#[test]
fn pptx_place_table_saved_dry_run_readback_and_errors_match_go_oracle() {
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

    let go_out = temp_dir.join("go-place-table.pptx");
    let rust_out = temp_dir.join("rust-place-table.pptx");
    let go_out_str = go_out.to_str().expect("go place table output");
    let rust_out_str = rust_out.to_str().expect("rust place table output");
    let go_args = [
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
        go_out_str,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "place table saved exit");
    assert_eq!(rust_stderr, go_stderr, "place table saved stderr");
    assert_eq!(
        rust_stdout.expect("rust place table stdout"),
        go_stdout.expect("go place table stdout"),
        "place table saved stdout"
    );
    assert!(go_out.exists(), "Go place table output missing");
    assert!(rust_out.exists(), "Rust place table output missing");

    let (go_show_code, go_show_stdout, go_show_stderr) = run_go_ooxml(&[
        "--json", "pptx", "tables", "show", go_out_str, "--slide", "1", "--target", "table:1",
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
    assert_eq!(rust_show_code, go_show_code, "place table readback exit");
    assert_eq!(
        rust_show_stderr, go_show_stderr,
        "place table readback stderr"
    );
    assert_eq!(
        scrub_path(
            rust_show_stdout.expect("rust place table readback"),
            rust_out_str,
            "[OUT]"
        ),
        scrub_path(
            go_show_stdout.expect("go place table readback"),
            go_out_str,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, go_code, "place table dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "place table dry-run stderr");
    assert_eq!(
        rust_stdout.expect("rust place table dry-run"),
        go_stdout.expect("go place table dry-run"),
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
        assert_go_rust_json_match(&args, "place table representative error");
    }
}

#[test]
fn pptx_place_table_from_xlsx_saved_dry_run_and_errors_match_go_oracle() {
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

    let go_out = temp_dir.join("go-place-table-from-xlsx.pptx");
    let rust_out = temp_dir.join("rust-place-table-from-xlsx.pptx");
    let go_out_str = go_out.to_str().expect("go place table xlsx output");
    let rust_out_str = rust_out.to_str().expect("rust place table xlsx output");
    let go_args = [
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
        go_out_str,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "place table-from-xlsx saved exit");
    assert_eq!(rust_stderr, go_stderr, "place table-from-xlsx saved stderr");
    let rust_json = rust_stdout.expect("rust place table-from-xlsx stdout");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_out_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go place table-from-xlsx stdout"),
            go_out_str,
            "[OUT]"
        ),
        "place table-from-xlsx saved stdout"
    );
    assert_eq!(rust_json["destination"]["cells"][0][0], "=SUM(B1:C1)");
    assert!(go_out.exists(), "Go place table-from-xlsx output missing");
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, go_code, "place table-from-xlsx dry-run exit");
    assert_eq!(
        rust_stderr, go_stderr,
        "place table-from-xlsx dry-run stderr"
    );
    assert_eq!(
        rust_stdout.expect("rust place table-from-xlsx dry-run"),
        go_stdout.expect("go place table-from-xlsx dry-run"),
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
        assert_go_rust_json_match(&args, "place table-from-xlsx representative error");
    }
}

include!("pptx/charts.rs");

#[test]
fn pptx_animations_list_json_and_missing_path_match_go_oracle() {
    assert_go_rust_match(&[
        "--json",
        "pptx",
        "animations",
        "list",
        "testdata/pptx/animations-synthetic/presentation.pptx",
    ]);
    assert_go_rust_match(&[
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
    assert_go_rust_match(&["--json", "pptx", "animations", "list", missing_str]);
}

#[test]
fn pptx_template_inspect_and_validate_layout_match_go_oracle() {
    assert_go_rust_json_match(
        &[
            "--json",
            "pptx",
            "template",
            "inspect",
            "testdata/pptx/template-branded/manifest.json",
        ],
        "pptx template inspect branded manifest",
    );
    assert_go_rust_json_match(
        &[
            "--json",
            "pptx",
            "template",
            "inspect",
            "testdata/pptx/template-branded/missing.json",
        ],
        "pptx template inspect missing manifest",
    );

    for fixture in [
        "testdata/pptx/minimal-title/presentation.pptx",
        "testdata/pptx/title-content/presentation.pptx",
        "testdata/pptx/table-slide/presentation.pptx",
        "testdata/pptx/layout-qa-dense-slide/presentation.pptx",
        "testdata/pptx/layout-qa-shape-collision/presentation.pptx",
        "testdata/pptx/layout-qa-text-overflow/presentation.pptx",
    ] {
        let args = ["--json", "pptx", "validate-layout", fixture];
        assert_go_rust_json_match(&args, fixture);
    }

    assert_go_rust_json_match(
        &[
            "--json",
            "pptx",
            "validate-layout",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
        ],
        "pptx validate-layout rejects xlsx",
    );
    assert_go_rust_json_match(
        &[
            "--json",
            "pptx",
            "validate-layout",
            "testdata/pptx/missing-layout-qa.pptx",
        ],
        "pptx validate-layout missing file",
    );
}

#[test]
fn template_tokens_profiles_and_xlsx_binding_plan_match_go_oracle() {
    assert_go_rust_json_match(
        &[
            "--json",
            "template",
            "tokens",
            "testdata/pptx/theme-custom-colors/presentation.pptx",
        ],
        "template tokens pptx theme",
    );
    assert_go_rust_json_match(
        &[
            "--json",
            "template",
            "tokens",
            "testdata/pptx/chart-simple/presentation.pptx",
        ],
        "template tokens pptx charts",
    );
    assert_go_rust_json_match(
        &[
            "--json",
            "template",
            "tokens",
            "testdata/xlsx/chart-workbook/workbook.xlsx",
        ],
        "template tokens xlsx charts",
    );
    assert_go_rust_json_match(
        &[
            "--json",
            "template",
            "profile",
            "save",
            "testdata/pptx/theme-custom-colors/presentation.pptx",
            "--name",
            "Theme",
            "--description",
            "Demo",
        ],
        "template profile save stdout",
    );

    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-template-bindings-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("template binding temp dir");
    let profile_path = temp_dir.join("profile.json");
    let profile_str = profile_path.to_string_lossy().to_string();
    let (save_code, save_stdout, save_stderr) = run_go_ooxml(&[
        "--json",
        "template",
        "profile",
        "save",
        "testdata/pptx/theme-custom-colors/presentation.pptx",
        "--name",
        "Theme",
        "--description",
        "Demo",
    ]);
    assert_eq!(save_code, 0, "go profile save for inspect fixture");
    assert_eq!(save_stderr, None, "go profile save stderr");
    fs::write(
        &profile_path,
        serde_json::to_vec_pretty(&save_stdout.expect("go profile save stdout")).unwrap(),
    )
    .expect("write profile fixture");
    assert_go_rust_json_match(
        &["--json", "template", "profile", "inspect", &profile_str],
        "template profile inspect",
    );

    let workbook = temp_dir.join("bindings.xlsx");
    write_xlsx_bindings_workbook(&workbook, "subtitle");
    let workbook_str = workbook.to_string_lossy().to_string();
    let plan_args = [
        "--json",
        "pptx",
        "xlsx-bindings",
        "plan",
        "testdata/pptx/title-content/presentation.pptx",
        "--workbook",
        &workbook_str,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:P3",
    ];
    assert_go_rust_json_match(&plan_args, "pptx xlsx-bindings plan");

    let duplicate_workbook = temp_dir.join("duplicate-bindings.xlsx");
    write_xlsx_bindings_workbook(&duplicate_workbook, "title");
    let duplicate_workbook_str = duplicate_workbook.to_string_lossy().to_string();
    let duplicate_args = [
        "--json",
        "pptx",
        "xlsx-bindings",
        "plan",
        "testdata/pptx/title-content/presentation.pptx",
        "--workbook",
        &duplicate_workbook_str,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:P3",
    ];
    assert_go_rust_json_match(&duplicate_args, "pptx xlsx-bindings duplicate target");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn template_parent_groups_are_help_only_like_go_oracle() {
    for args in [
        vec!["--json", "template"],
        vec!["--json", "template", "profile"],
        vec!["--json", "pptx", "template"],
    ] {
        let (go_code, go_stdout, go_stderr) = run_go_ooxml_raw(&args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml_raw(&args);
        assert_eq!(rust_code, go_code, "exit code for {args:?}");
        assert_eq!(go_code, 0, "Go parent help exit for {args:?}");
        assert!(go_stderr.is_empty(), "Go parent help stderr for {args:?}");
        assert!(
            rust_stderr.is_empty(),
            "Rust parent help stderr for {args:?}: {rust_stderr}"
        );
        assert!(go_stdout.contains("Usage:"), "Go help text for {args:?}");
        assert!(
            rust_stdout.contains("Usage:"),
            "Rust help text for {args:?}: {rust_stdout}"
        );
    }
}

#[test]
fn template_apply_theme_tokens_dry_run_saved_ranges_and_errors_match_go_oracle() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-template-apply-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("template apply temp dir");
    let tokens_path = temp_dir.join("tokens.json");
    fs::write(&tokens_path, template_apply_tokens_json()).expect("write template tokens");
    let tokens_str = tokens_path.to_string_lossy().to_string();
    let target = "testdata/pptx/minimal-title/presentation.pptx";

    let dry_args = [
        "--json",
        "template",
        "apply",
        target,
        "--tokens",
        &tokens_str,
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_args);
    assert_eq!(rust_code, go_code, "template apply dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "template apply dry-run stderr");
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust dry-run stdout"),
            &tokens_str,
            "[TOKENS]"
        ),
        scrub_path(
            go_stdout.expect("go dry-run stdout"),
            &tokens_str,
            "[TOKENS]"
        ),
        "template apply dry-run stdout"
    );

    let go_out = temp_dir.join("go-applied.pptx");
    let rust_out = temp_dir.join("rust-applied.pptx");
    let go_out_str = go_out.to_string_lossy().to_string();
    let rust_out_str = rust_out.to_string_lossy().to_string();
    let go_args = [
        "--json",
        "template",
        "apply",
        target,
        "--tokens",
        &tokens_str,
        "--out",
        &go_out_str,
    ];
    let rust_args = [
        "--json",
        "template",
        "apply",
        target,
        "--tokens",
        &tokens_str,
        "--out",
        &rust_out_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "template apply saved exit");
    assert_eq!(rust_stderr, go_stderr, "template apply saved stderr");
    assert_eq!(
        scrub_paths(
            rust_stdout.expect("rust saved stdout"),
            &[(&tokens_str, "[TOKENS]"), (&rust_out_str, "[OUT]")]
        ),
        scrub_paths(
            go_stdout.expect("go saved stdout"),
            &[(&tokens_str, "[TOKENS]"), (&go_out_str, "[OUT]")]
        ),
        "template apply saved stdout"
    );
    assert!(rust_out.exists(), "Rust template apply output exists");
    assert_strict_validate_succeeds(&rust_out_str, "template apply output");
    let (_, applied_tokens, applied_tokens_stderr) =
        run_ooxml(&["--json", "template", "tokens", &rust_out_str]);
    assert_eq!(
        applied_tokens_stderr, None,
        "template tokens stderr for applied output"
    );
    let applied_tokens = applied_tokens.expect("applied output tokens");
    assert_eq!(
        applied_tokens["pptx"]["theme"]["colorScheme"]["accent1"],
        serde_json::json!("123456")
    );
    assert_eq!(
        applied_tokens["pptx"]["theme"]["fontScheme"]["majorFont"],
        serde_json::json!("Aptos Display")
    );

    let go_ranges_out = temp_dir.join("go-ranges.pptx");
    let rust_ranges_out = temp_dir.join("rust-ranges.pptx");
    let go_ranges_out_str = go_ranges_out.to_string_lossy().to_string();
    let rust_ranges_out_str = rust_ranges_out.to_string_lossy().to_string();
    let go_ranges_args = [
        "--json",
        "template",
        "apply",
        target,
        "--tokens",
        &tokens_str,
        "--target-ranges",
        "--out",
        &go_ranges_out_str,
    ];
    let rust_ranges_args = [
        "--json",
        "template",
        "apply",
        target,
        "--tokens",
        &tokens_str,
        "--target-ranges",
        "--out",
        &rust_ranges_out_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_ranges_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_ranges_args);
    assert_eq!(rust_code, go_code, "template apply ranges exit");
    assert_eq!(rust_stderr, go_stderr, "template apply ranges stderr");
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust ranges stdout"),
            &tokens_str,
            "[TOKENS]"
        ),
        scrub_path(
            go_stdout.expect("go ranges stdout"),
            &tokens_str,
            "[TOKENS]"
        ),
        "template apply ranges stdout"
    );
    assert!(
        !rust_ranges_out.exists(),
        "ranges-only apply should not produce an output file"
    );

    assert_go_rust_json_match(
        &["--json", "template", "apply", target],
        "template apply missing output mode",
    );
    assert_go_rust_json_match(
        &[
            "--json",
            "pptx",
            "template",
            "compile",
            "testdata/pptx/template-branded/manifest.json",
            "testdata/pptx/template-branded/spec-simple.yaml",
        ],
        "pptx template compile missing required flags",
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn template_apply_pptx_chart_and_text_style_targets_match_go_oracle() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-template-targets-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("template targets temp dir");

    let tokens_path = temp_dir.join("targets.json");
    fs::write(&tokens_path, template_apply_chart_text_tokens_json())
        .expect("write template target tokens");
    let tokens_str = tokens_path.to_string_lossy().to_string();

    let chart_target = "testdata/pptx/chart-simple/presentation.pptx";
    let go_chart_out = temp_dir.join("go-chart.pptx");
    let rust_chart_out = temp_dir.join("rust-chart.pptx");
    let go_chart_out_str = go_chart_out.to_string_lossy().to_string();
    let rust_chart_out_str = rust_chart_out.to_string_lossy().to_string();
    let go_chart_args = [
        "--json",
        "template",
        "apply",
        chart_target,
        "--tokens",
        &tokens_str,
        "--target-charts",
        "--out",
        &go_chart_out_str,
    ];
    let rust_chart_args = [
        "--json",
        "template",
        "apply",
        chart_target,
        "--tokens",
        &tokens_str,
        "--target-charts",
        "--out",
        &rust_chart_out_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_chart_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_chart_args);
    assert_eq!(rust_code, go_code, "template apply PPTX charts exit");
    assert_eq!(rust_stderr, go_stderr, "template apply PPTX charts stderr");
    assert_eq!(
        scrub_paths(
            rust_stdout.expect("rust PPTX chart apply stdout"),
            &[(&tokens_str, "[TOKENS]"), (&rust_chart_out_str, "[OUT]")]
        ),
        scrub_paths(
            go_stdout.expect("go PPTX chart apply stdout"),
            &[(&tokens_str, "[TOKENS]"), (&go_chart_out_str, "[OUT]")]
        ),
        "template apply PPTX charts stdout"
    );
    assert_strict_validate_succeeds(&rust_chart_out_str, "template apply PPTX chart output");
    assert_conformance_check_runs(&rust_chart_out_str, "template apply PPTX chart output");
    let (_, chart_show, chart_show_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "charts",
        "show",
        &rust_chart_out_str,
        "--chart",
        "chart:1",
    ]);
    assert_eq!(chart_show_stderr, None, "PPTX chart readback stderr");
    let chart_show = chart_show.expect("PPTX chart readback");
    assert_eq!(
        chart_show["charts"][0]["style"]["series"][0]["fillColor"],
        serde_json::json!("FF0000")
    );
    assert_eq!(
        chart_show["charts"][0]["style"]["series"][0]["lineColor"],
        serde_json::json!("00FF00")
    );

    let text_target = "testdata/pptx/minimal-title/presentation.pptx";
    let go_text_out = temp_dir.join("go-text-styles.pptx");
    let rust_text_out = temp_dir.join("rust-text-styles.pptx");
    let go_text_out_str = go_text_out.to_string_lossy().to_string();
    let rust_text_out_str = rust_text_out.to_string_lossy().to_string();
    let go_text_args = [
        "--json",
        "template",
        "apply",
        text_target,
        "--tokens",
        &tokens_str,
        "--target-text-styles",
        "--out",
        &go_text_out_str,
    ];
    let rust_text_args = [
        "--json",
        "template",
        "apply",
        text_target,
        "--tokens",
        &tokens_str,
        "--target-text-styles",
        "--out",
        &rust_text_out_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_text_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_text_args);
    assert_eq!(rust_code, go_code, "template apply text styles exit");
    assert_eq!(rust_stderr, go_stderr, "template apply text styles stderr");
    assert_eq!(
        scrub_paths(
            rust_stdout.expect("rust text styles apply stdout"),
            &[(&tokens_str, "[TOKENS]"), (&rust_text_out_str, "[OUT]")]
        ),
        scrub_paths(
            go_stdout.expect("go text styles apply stdout"),
            &[(&tokens_str, "[TOKENS]"), (&go_text_out_str, "[OUT]")]
        ),
        "template apply text styles stdout"
    );
    assert_strict_validate_succeeds(&rust_text_out_str, "template apply text style output");
    let (_, text_tokens, text_tokens_stderr) =
        run_ooxml(&["--json", "template", "tokens", &rust_text_out_str]);
    assert_eq!(text_tokens_stderr, None, "text style token readback stderr");
    let text_tokens = text_tokens.expect("text style token readback");
    let styles = text_tokens["pptx"]["defaultTextStyles"]
        .as_array()
        .expect("default text styles");
    assert!(
        styles.iter().any(|style| {
            style["role"] == serde_json::json!("title")
                && style["fontRef"] == serde_json::json!("major")
                && style["colorRef"] == serde_json::json!("accent1")
        }),
        "updated title style should read back"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn pptx_template_compile_simple_matches_go_oracle_and_validates() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-template-compile-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("template compile temp dir");
    let go_out = temp_dir.join("go-compiled.pptx");
    let rust_out = temp_dir.join("rust-compiled.pptx");
    let go_out_str = go_out.to_string_lossy().to_string();
    let rust_out_str = rust_out.to_string_lossy().to_string();
    let go_args = [
        "--json",
        "pptx",
        "template",
        "compile",
        "testdata/pptx/template-branded/manifest.json",
        "testdata/pptx/template-branded/spec-simple.yaml",
        "--archetype",
        "testdata/pptx/template-branded/presentation.pptx",
        "--out",
        &go_out_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "template",
        "compile",
        "testdata/pptx/template-branded/manifest.json",
        "testdata/pptx/template-branded/spec-simple.yaml",
        "--archetype",
        "testdata/pptx/template-branded/presentation.pptx",
        "--out",
        &rust_out_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "pptx template compile exit");
    assert_eq!(rust_stderr, go_stderr, "pptx template compile stderr");
    assert_eq!(
        scrub_template_compile_result(rust_stdout.expect("rust compile stdout"), &rust_out_str),
        scrub_template_compile_result(go_stdout.expect("go compile stdout"), &go_out_str),
        "pptx template compile stdout"
    );
    assert!(rust_out.exists(), "Rust compiled PPTX exists");
    assert_strict_validate_succeeds(&rust_out_str, "compiled PPTX");

    let (go_text_code, go_text, go_text_stderr) =
        run_go_ooxml(&["--json", "pptx", "extract", "text", &go_out_str]);
    let (rust_text_code, rust_text, rust_text_stderr) =
        run_ooxml(&["--json", "pptx", "extract", "text", &rust_out_str]);
    assert_eq!(rust_text_code, go_text_code, "compiled text readback exit");
    assert_eq!(
        rust_text_stderr, go_text_stderr,
        "compiled text readback stderr"
    );
    assert_eq!(
        scrub_path(
            rust_text.expect("rust compiled text"),
            &rust_out_str,
            "[OUT]"
        ),
        scrub_path(go_text.expect("go compiled text"), &go_out_str, "[OUT]"),
        "compiled PPTX text readback"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

fn template_apply_tokens_json() -> &'static str {
    r#"{
  "schemaVersion": "1.0",
  "type": "pptx",
  "source": "probe-tokens",
  "pptx": {
    "theme": {
      "colorScheme": {
        "dark1": "101010",
        "light1": "FAFAFA",
        "dark2": "202020",
        "light2": "EEEEEE",
        "accent1": "123456",
        "accent2": "234567",
        "accent3": "345678",
        "accent4": "456789",
        "accent5": "56789A",
        "accent6": "6789AB",
        "hypLink": "789ABC",
        "folLink": "89ABCD"
      },
      "fontScheme": {
        "majorFont": "Aptos Display",
        "minorFont": "Aptos"
      }
    }
  }
}
"#
}

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

fn template_apply_chart_text_tokens_json() -> &'static str {
    r#"{
  "schemaVersion": "1.0",
  "type": "pptx",
  "source": "template-targets",
  "pptx": {
    "theme": null,
    "defaultTextStyles": [
      {
        "masterRef": "/template/master.xml",
        "role": "title",
        "fontRef": "major",
        "sizePt": 31,
        "colorRef": "accent1"
      },
      {
        "masterRef": "/template/master.xml",
        "role": "body",
        "fontRef": "minor",
        "sizePt": 19,
        "color": "FF00AA"
      }
    ],
    "tableStyles": [],
    "chartStyles": [
      {
        "partUri": "/template/chart.xml",
        "seriesFillColor": "FF0000",
        "seriesLineColor": "00FF00"
      }
    ]
  },
  "xlsx": {
    "theme": null,
    "namedCellStyles": [],
    "chartStyles": [
      {
        "partUri": "/template/chart.xml",
        "seriesFillColor": "FF0000",
        "seriesLineColor": "00FF00"
      }
    ]
  }
}
"#
}

fn scrub_template_compile_result(value: Value, output_path: &str) -> Value {
    let mut value = scrub_path(value, output_path, "[OUT]");
    if let Value::Object(map) = &mut value {
        for key in ["startedAt", "completedAt", "duration"] {
            if map.contains_key(key) {
                map.insert(key.to_string(), Value::String(format!("[{key}]")));
            }
        }
    }
    value
}

#[test]
fn pptx_xlsx_bindings_apply_saved_dry_run_and_errors_match_go_oracle() {
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
    let go_out = temp_dir.join("go-bindings.pptx");
    let rust_out = temp_dir.join("rust-bindings.pptx");
    let go_out_str = go_out.to_str().expect("go bindings output");
    let rust_out_str = rust_out.to_str().expect("rust bindings output");

    let go_args = [
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
        go_out_str,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "xlsx-bindings apply saved exit");
    assert_eq!(rust_stderr, go_stderr, "xlsx-bindings apply saved stderr");
    let go_json = go_stdout.expect("go xlsx-bindings apply saved");
    let rust_json = rust_stdout.expect("rust xlsx-bindings apply saved");
    assert_eq!(
        scrub_paths(rust_json.clone(), &[(rust_out_str, "[OUT]")]),
        scrub_paths(go_json, &[(go_out_str, "[OUT]")]),
        "xlsx-bindings apply saved stdout"
    );
    assert!(go_out.exists(), "Go xlsx-bindings output missing");
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

    let (go_read_code, go_read_stdout, go_read_stderr) =
        run_go_ooxml(&["--json", "pptx", "extract", "text", go_out_str]);
    let (rust_read_code, rust_read_stdout, rust_read_stderr) =
        run_ooxml(&["--json", "pptx", "extract", "text", rust_out_str]);
    assert_eq!(rust_read_code, go_read_code, "bindings readback exit");
    assert_eq!(rust_read_stderr, go_read_stderr, "bindings readback stderr");
    assert_eq!(
        scrub_path(
            rust_read_stdout.expect("rust bindings readback"),
            rust_out_str,
            "[OUT]"
        ),
        scrub_path(
            go_read_stdout.expect("go bindings readback"),
            go_out_str,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, go_code, "xlsx-bindings apply dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "xlsx-bindings apply dry-run stderr");
    assert_eq!(
        rust_stdout.expect("rust xlsx-bindings dry-run"),
        go_stdout.expect("go xlsx-bindings dry-run"),
        "xlsx-bindings apply dry-run stdout"
    );

    assert_go_rust_json_match(
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

#[test]
fn pptx_template_capture_produces_manifest_for_readable_deck() {
    let (code, stdout, stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "template",
        "capture",
        "testdata/pptx/title-content/presentation.pptx",
        "--name",
        "Captured",
        "--slides",
        "1",
        "--version",
        "1.2.3",
    ]);
    assert_eq!(code, 0, "capture exit");
    assert_eq!(stderr, None, "capture stderr");
    let manifest = stdout.expect("capture manifest");
    assert_eq!(
        manifest["manifestVersion"],
        Value::String("1.0.0".to_string())
    );
    assert_eq!(manifest["name"], Value::String("Captured".to_string()));
    assert_eq!(manifest["version"]["major"], Value::from(1));
    assert_eq!(manifest["version"]["minor"], Value::from(2));
    assert_eq!(manifest["version"]["patch"], Value::from(3));
    let archetypes = manifest["archetypes"].as_array().expect("archetypes");
    assert_eq!(archetypes.len(), 1);
    assert_eq!(archetypes[0]["sourceSlideNumber"], Value::from(1));
    assert!(
        archetypes[0]["slots"]
            .as_array()
            .expect("slots")
            .iter()
            .any(|slot| slot["id"] == Value::String("title".to_string()))
    );
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
fn pptx_animations_mutations_match_go_oracle_and_validate() {
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
    assert_go_rust_match(&dry_run_args);

    let go_s1 = temp_dir.join("go-s1.pptx");
    let go_s2 = temp_dir.join("go-s2.pptx");
    let go_s3 = temp_dir.join("go-s3.pptx");
    let go_reordered = temp_dir.join("go-reordered.pptx");
    let go_removed = temp_dir.join("go-removed.pptx");
    let rust_s1 = temp_dir.join("rust-s1.pptx");
    let rust_s2 = temp_dir.join("rust-s2.pptx");
    let rust_s3 = temp_dir.join("rust-s3.pptx");
    let rust_reordered = temp_dir.join("rust-reordered.pptx");
    let rust_removed = temp_dir.join("rust-removed.pptx");

    let mut input_go = fixture.to_string();
    let mut input_rust = fixture.to_string();
    for (effect, go_out, rust_out) in [
        ("appear", &go_s1, &rust_s1),
        ("wipe", &go_s2, &rust_s2),
        ("fade", &go_s3, &rust_s3),
    ] {
        let go_out_str = go_out.to_str().expect("go animation output");
        let rust_out_str = rust_out.to_str().expect("rust animation output");
        let mut go_args = vec![
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
            go_out_str,
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
            go_args.extend(["--direction", "up"]);
            rust_args.extend(["--direction", "up"]);
        }
        let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
        assert_eq!(rust_code, go_code, "add {effect} exit");
        assert_eq!(rust_stderr, go_stderr, "add {effect} stderr");
        let rust_json = rust_stdout.expect("rust add stdout");
        assert_eq!(
            scrub_paths(
                rust_json.clone(),
                &[(rust_out_str, "[OUT]"), (input_rust.as_str(), "[IN]")]
            ),
            scrub_paths(
                go_stdout.expect("go add stdout"),
                &[(go_out_str, "[OUT]"), (input_go.as_str(), "[IN]")]
            ),
            "add {effect} stdout"
        );
        assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");
        input_go = go_out_str.to_string();
        input_rust = rust_out_str.to_string();
    }

    let (go_list_code, go_list_stdout, go_list_stderr) = run_go_ooxml(&[
        "--json",
        "pptx",
        "animations",
        "list",
        go_s3.to_str().expect("go s3"),
    ]);
    let (rust_list_code, rust_list_stdout, rust_list_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "animations",
        "list",
        rust_s3.to_str().expect("rust s3"),
    ]);
    assert_eq!(rust_list_code, go_list_code, "list after add exit");
    assert_eq!(rust_list_stderr, go_list_stderr, "list after add stderr");
    assert_eq!(
        rust_list_stdout.clone().expect("rust list after add"),
        go_list_stdout.expect("go list after add"),
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

    let go_reordered_str = go_reordered.to_str().expect("go reorder output");
    let rust_reordered_str = rust_reordered.to_str().expect("rust reorder output");
    let go_reorder_args = [
        "--json",
        "pptx",
        "animations",
        "reorder",
        go_s3.to_str().expect("go s3 path"),
        "--slide",
        "1",
        "--order",
        order.as_str(),
        "--out",
        go_reordered_str,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_reorder_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_reorder_args);
    assert_eq!(rust_code, go_code, "reorder exit");
    assert_eq!(rust_stderr, go_stderr, "reorder stderr");
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
            go_stdout.expect("go reorder stdout"),
            &[
                (go_reordered_str, "[OUT]"),
                (go_s3.to_str().expect("go s3 scrub"), "[IN]"),
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
    let go_removed_str = go_removed.to_str().expect("go removed output");
    let rust_removed_str = rust_removed.to_str().expect("rust removed output");
    let go_remove_args = [
        "--json",
        "pptx",
        "animations",
        "remove",
        go_reordered_str,
        "--slide",
        "1",
        "--effect-id",
        remove_id.as_str(),
        "--out",
        go_removed_str,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_remove_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_remove_args);
    assert_eq!(rust_code, go_code, "remove exit");
    assert_eq!(rust_stderr, go_stderr, "remove stderr");
    let rust_remove_json = rust_stdout.expect("rust remove stdout");
    assert_eq!(
        scrub_paths(
            rust_remove_json.clone(),
            &[(rust_removed_str, "[OUT]"), (rust_reordered_str, "[IN]")]
        ),
        scrub_paths(
            go_stdout.expect("go remove stdout"),
            &[(go_removed_str, "[OUT]"), (go_reordered_str, "[IN]")]
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
    assert_go_rust_match(&missing_args);

    let prune_dry_run = [
        "--json",
        "pptx",
        "animations",
        "prune-stale",
        "testdata/pptx/animations-synthetic/presentation.pptx",
        "--dry-run",
    ];
    assert_go_rust_match(&prune_dry_run);

    let go_pruned = temp_dir.join("go-pruned.pptx");
    let rust_pruned = temp_dir.join("rust-pruned.pptx");
    let go_pruned_str = go_pruned.to_str().expect("go pruned output");
    let rust_pruned_str = rust_pruned.to_str().expect("rust pruned output");
    let go_prune_args = [
        "--json",
        "pptx",
        "animations",
        "prune-stale",
        "testdata/pptx/animations-synthetic/presentation.pptx",
        "--slide",
        "4",
        "--out",
        go_pruned_str,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_prune_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_prune_args);
    assert_eq!(rust_code, go_code, "prune saved exit");
    assert_eq!(rust_stderr, go_stderr, "prune saved stderr");
    let rust_prune_json = rust_stdout.expect("rust prune stdout");
    assert_eq!(
        scrub_path(rust_prune_json.clone(), rust_pruned_str, "[OUT]"),
        scrub_path(go_stdout.expect("go prune stdout"), go_pruned_str, "[OUT]"),
        "prune saved stdout"
    );
    assert_rust_emitted_ooxml_command_succeeds(&rust_prune_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_prune_json, "validateCommand");

    let (go_code, go_stdout, go_stderr) =
        run_go_ooxml(&["--json", "pptx", "animations", "list", go_pruned_str]);
    let (rust_code, rust_stdout, rust_stderr) =
        run_ooxml(&["--json", "pptx", "animations", "list", rust_pruned_str]);
    assert_eq!(rust_code, go_code, "prune readback exit");
    assert_eq!(rust_stderr, go_stderr, "prune readback stderr");
    assert_eq!(
        rust_stdout.expect("rust prune readback"),
        go_stdout.expect("go prune readback"),
        "prune readback stdout"
    );
}

fn assert_pptx_chart_copy_style_matches_go(temp_dir: &Path) {
    let fixture = "testdata/pptx/chart-simple/presentation.pptx";
    let command = "copy-style";
    let go_out = temp_dir.join("go-copy-style.pptx");
    let rust_out = temp_dir.join("rust-copy-style.pptx");
    let go_out_str = go_out.to_str().expect("go copy-style output path");
    let rust_out_str = rust_out.to_str().expect("rust copy-style output path");
    let go_args = [
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
        go_out_str,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "copy-style exit");
    assert_eq!(rust_stderr, go_stderr, "copy-style stderr");
    let rust_json = rust_stdout.expect("rust copy-style stdout");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_out_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go copy-style stdout"),
            go_out_str,
            "[OUT]"
        ),
        "copy-style stdout"
    );
    assert!(go_out.exists(), "Go copy-style output missing");
    assert!(rust_out.exists(), "Rust copy-style output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "chartShowCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    let go_show_args = [
        "--json",
        "pptx",
        "charts",
        "show",
        go_out_str,
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
    let (go_show_code, go_show_stdout, go_show_stderr) = run_go_ooxml(&go_show_args);
    let (rust_show_code, rust_show_stdout, rust_show_stderr) = run_ooxml(&rust_show_args);
    assert_eq!(rust_show_code, go_show_code, "copy-style readback exit");
    assert_eq!(
        rust_show_stderr, go_show_stderr,
        "copy-style readback stderr"
    );
    assert_eq!(
        scrub_path(
            rust_show_stdout.expect("rust copy-style readback"),
            rust_out_str,
            "[OUT]"
        ),
        scrub_path(
            go_show_stdout.expect("go copy-style readback"),
            go_out_str,
            "[OUT]"
        ),
        "copy-style readback stdout"
    );
}

fn assert_pptx_chart_saved_mutation_matches_go(
    temp_dir: &Path,
    command: &str,
    extra_args: &[&str],
) {
    let fixture = "testdata/pptx/chart-simple/presentation.pptx";
    let go_out = temp_dir.join(format!("go-{command}.pptx"));
    let rust_out = temp_dir.join(format!("rust-{command}.pptx"));
    let go_out_str = go_out.to_str().expect("go chart mutation output path");
    let rust_out_str = rust_out.to_str().expect("rust chart mutation output path");

    let mut go_args = vec!["--json", "pptx", "charts", command, fixture];
    go_args.extend_from_slice(extra_args);
    go_args.extend_from_slice(&["--out", go_out_str]);
    let mut rust_args = vec!["--json", "pptx", "charts", command, fixture];
    rust_args.extend_from_slice(extra_args);
    rust_args.extend_from_slice(&["--out", rust_out_str]);

    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "{command} exit");
    assert_eq!(rust_stderr, go_stderr, "{command} stderr");
    let rust_json = rust_stdout.unwrap_or_else(|| panic!("rust {command} stdout"));
    assert_eq!(
        scrub_path(rust_json.clone(), rust_out_str, "[OUT]"),
        scrub_path(
            go_stdout.unwrap_or_else(|| panic!("go {command} stdout")),
            go_out_str,
            "[OUT]"
        ),
        "{command} stdout"
    );
    assert!(go_out.exists(), "Go {command} output missing");
    assert!(rust_out.exists(), "Rust {command} output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "chartShowCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    let go_show_args = [
        "--json",
        "pptx",
        "charts",
        "show",
        go_out_str,
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
    let (go_show_code, go_show_stdout, go_show_stderr) = run_go_ooxml(&go_show_args);
    let (rust_show_code, rust_show_stdout, rust_show_stderr) = run_ooxml(&rust_show_args);
    assert_eq!(rust_show_code, go_show_code, "{command} readback exit");
    assert_eq!(
        rust_show_stderr, go_show_stderr,
        "{command} readback stderr"
    );
    assert_eq!(
        scrub_path(
            rust_show_stdout.unwrap_or_else(|| panic!("rust {command} readback")),
            rust_out_str,
            "[OUT]"
        ),
        scrub_path(
            go_show_stdout.unwrap_or_else(|| panic!("go {command} readback")),
            go_out_str,
            "[OUT]"
        ),
        "{command} readback stdout"
    );
}

#[test]
fn frozen_pptx_mutation_and_validate_match_go_baseline() {
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

#[test]
fn pptx_extract_images_artifacts_null_manifest_and_errors_match_go_oracle() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-extract-images-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("extract images temp dir");

    let fixture = "testdata/pptx/slide-assembly-notes-media/presentation.pptx";
    let go_dir = temp_dir.join("go-images");
    let rust_dir = temp_dir.join("rust-images");
    let go_dir_str = go_dir.to_str().expect("go image dir");
    let rust_dir_str = rust_dir.to_str().expect("rust image dir");
    let go_args = [
        "--json", "pptx", "extract", "images", fixture, "--out", go_dir_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "extract",
        "images",
        fixture,
        "--out",
        rust_dir_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "extract images exit");
    assert_eq!(rust_stderr, go_stderr, "extract images stderr");
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust extract images stdout"),
            rust_dir_str,
            "[OUT]"
        ),
        scrub_path(
            go_stdout.expect("go extract images stdout"),
            go_dir_str,
            "[OUT]"
        ),
        "extract images stdout"
    );
    assert_export_dirs_match(&go_dir, &rust_dir);

    let empty_fixture = "testdata/pptx/minimal-title/presentation.pptx";
    let go_empty_dir = temp_dir.join("go-empty-images");
    let rust_empty_dir = temp_dir.join("rust-empty-images");
    let go_empty_dir_str = go_empty_dir.to_str().expect("go empty image dir");
    let rust_empty_dir_str = rust_empty_dir.to_str().expect("rust empty image dir");
    let go_empty_args = [
        "--json",
        "pptx",
        "extract",
        "images",
        empty_fixture,
        "--include-layout-images",
        "--out",
        go_empty_dir_str,
    ];
    let rust_empty_args = [
        "--json",
        "pptx",
        "extract",
        "images",
        empty_fixture,
        "--include-layout-images",
        "--out",
        rust_empty_dir_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_empty_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_empty_args);
    assert_eq!(rust_code, go_code, "extract images empty exit");
    assert_eq!(rust_stderr, go_stderr, "extract images empty stderr");
    let rust_empty_json = rust_stdout.expect("rust empty images stdout");
    assert_eq!(rust_empty_json["images"], Value::Null);
    assert_eq!(
        scrub_path(rust_empty_json, rust_empty_dir_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go empty images stdout"),
            go_empty_dir_str,
            "[OUT]"
        ),
        "extract images empty stdout"
    );
    assert_export_dirs_match(&go_empty_dir, &rust_empty_dir);

    let out_of_range = [
        "--json",
        "pptx",
        "extract",
        "images",
        "testdata/pptx/minimal-title/presentation.pptx",
        "--slide",
        "99",
        "--out",
        go_empty_dir_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&out_of_range);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&out_of_range);
    assert_eq!(rust_code, go_code, "extract images out-of-range exit");
    assert_eq!(rust_stdout, go_stdout, "extract images out-of-range stdout");
    assert_eq!(rust_stderr, go_stderr, "extract images out-of-range stderr");
}

#[test]
fn pptx_media_list_json_filters_and_missing_args_match_go_oracle() {
    for (label, args) in [
        (
            "media list no media",
            vec![
                "--json",
                "pptx",
                "media",
                "list",
                "testdata/pptx/minimal-title/presentation.pptx",
            ],
        ),
        (
            "media list synthetic",
            vec![
                "--json",
                "pptx",
                "media",
                "list",
                "testdata/pptx/animations-synthetic/presentation.pptx",
            ],
        ),
        (
            "media list stale",
            vec![
                "--json",
                "pptx",
                "media",
                "list",
                "testdata/pptx/animations-stale-media/presentation.pptx",
            ],
        ),
        (
            "media list slide filter",
            vec![
                "--json",
                "pptx",
                "media",
                "list",
                "testdata/pptx/animations-synthetic/presentation.pptx",
                "--slide",
                "5",
            ],
        ),
        (
            "media list missing slide filter",
            vec![
                "--json",
                "pptx",
                "media",
                "list",
                "testdata/pptx/animations-synthetic/presentation.pptx",
                "--slide",
                "99",
            ],
        ),
        (
            "media list missing file",
            vec!["--json", "pptx", "media", "list"],
        ),
        (
            "media add missing file",
            vec!["--json", "pptx", "media", "add"],
        ),
        (
            "media replace missing file",
            vec!["--json", "pptx", "media", "replace"],
        ),
    ] {
        assert_go_rust_json_match(&args, label);
    }
}

#[test]
fn pptx_media_add_replace_saved_readback_and_guards_match_go_oracle() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-media-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("pptx media temp dir");

    let fixture = "testdata/pptx/minimal-title/presentation.pptx";
    let clip = temp_dir.join("clip.mp4");
    let replacement_clip = temp_dir.join("replacement.mp4");
    std::fs::write(&clip, b"opaque-fake-media-bytes").expect("write media clip");
    std::fs::write(&replacement_clip, b"opaque-replacement-media-bytes")
        .expect("write replacement media clip");
    let clip_str = clip.to_str().expect("clip path");
    let replacement_clip_str = replacement_clip.to_str().expect("replacement clip path");

    let go_add_out = temp_dir.join("go-with-media.pptx");
    let rust_add_out = temp_dir.join("rust-with-media.pptx");
    let go_add_out_str = go_add_out.to_str().expect("go media add output");
    let rust_add_out_str = rust_add_out.to_str().expect("rust media add output");
    let go_add_args = [
        "--json",
        "pptx",
        "media",
        "add",
        fixture,
        "--slide",
        "1",
        "--file",
        clip_str,
        "--name",
        "Intro",
        "--out",
        go_add_out_str,
    ];
    let rust_add_args = [
        "--json",
        "pptx",
        "media",
        "add",
        fixture,
        "--slide",
        "1",
        "--file",
        clip_str,
        "--name",
        "Intro",
        "--out",
        rust_add_out_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_add_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_add_args);
    assert_eq!(rust_code, go_code, "media add exit");
    assert_eq!(rust_stderr, go_stderr, "media add stderr");
    let rust_add_json = rust_stdout.expect("rust media add stdout");
    assert_eq!(
        scrub_path(rust_add_json.clone(), rust_add_out_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go media add stdout"),
            go_add_out_str,
            "[OUT]"
        ),
        "media add stdout"
    );
    assert_eq!(rust_add_json["shapeName"], "Intro");
    assert_eq!(rust_add_json["kind"], "video");
    assert_eq!(rust_add_json["posterSynthesized"], Value::Bool(true));
    assert!(go_add_out.exists(), "Go media add output missing");
    assert!(rust_add_out.exists(), "Rust media add output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_add_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_add_json, "validateCommand");

    let (go_list_code, go_list_stdout, go_list_stderr) = run_go_ooxml(&[
        "--json",
        "pptx",
        "media",
        "list",
        go_add_out_str,
        "--slide",
        "1",
    ]);
    let (rust_list_code, rust_list_stdout, rust_list_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "media",
        "list",
        rust_add_out_str,
        "--slide",
        "1",
    ]);
    assert_eq!(rust_list_code, go_list_code, "media add readback exit");
    assert_eq!(
        rust_list_stderr, go_list_stderr,
        "media add readback stderr"
    );
    assert_eq!(
        rust_list_stdout.expect("rust media add readback"),
        go_list_stdout.expect("go media add readback"),
        "media add readback stdout"
    );

    let go_replace_out = temp_dir.join("go-replaced-media.pptx");
    let rust_replace_out = temp_dir.join("rust-replaced-media.pptx");
    let go_replace_out_str = go_replace_out.to_str().expect("go media replace output");
    let rust_replace_out_str = rust_replace_out
        .to_str()
        .expect("rust media replace output");
    let shape_id = rust_add_json["shapeId"]
        .as_i64()
        .expect("media add shape id")
        .to_string();
    let go_replace_args = [
        "--json",
        "pptx",
        "media",
        "replace",
        go_add_out_str,
        "--slide",
        "1",
        "--shape",
        &shape_id,
        "--file",
        replacement_clip_str,
        "--expect-shape-name",
        "Intro",
        "--expect-media-kind",
        "video",
        "--out",
        go_replace_out_str,
    ];
    let rust_replace_args = [
        "--json",
        "pptx",
        "media",
        "replace",
        rust_add_out_str,
        "--slide",
        "1",
        "--shape",
        &shape_id,
        "--file",
        replacement_clip_str,
        "--expect-shape-name",
        "Intro",
        "--expect-media-kind",
        "video",
        "--out",
        rust_replace_out_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_replace_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_replace_args);
    assert_eq!(rust_code, go_code, "media replace exit");
    assert_eq!(rust_stderr, go_stderr, "media replace stderr");
    let rust_replace_json = rust_stdout.expect("rust media replace stdout");
    assert_eq!(
        scrub_paths(
            rust_replace_json.clone(),
            &[(rust_add_out_str, "[IN]"), (rust_replace_out_str, "[OUT]")]
        ),
        scrub_paths(
            go_stdout.expect("go media replace stdout"),
            &[(go_add_out_str, "[IN]"), (go_replace_out_str, "[OUT]")]
        ),
        "media replace stdout"
    );
    assert!(go_replace_out.exists(), "Go media replace output missing");
    assert!(
        rust_replace_out.exists(),
        "Rust media replace output missing"
    );
    assert_rust_emitted_ooxml_command_succeeds(&rust_replace_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_replace_json, "validateCommand");

    let guard_args = [
        "--json",
        "pptx",
        "media",
        "replace",
        rust_add_out_str,
        "--slide",
        "1",
        "--shape",
        &shape_id,
        "--file",
        replacement_clip_str,
        "--expect-shape-name",
        "WrongName",
        "--out",
        rust_replace_out_str,
    ];
    let (code, stdout, stderr) = run_ooxml(&guard_args);
    assert_eq!(code, 2, "media replace guard exit");
    assert_eq!(stdout, None, "media replace guard stdout");
    assert_eq!(
        stderr.expect("media replace guard stderr")["error"]["code"],
        "invalid_args"
    );
}

#[test]
fn pptx_replace_text_occurrences_saved_readback_dry_run_and_errors_match_go_oracle() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-replace-occurrences-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("replace text occurrences temp dir");

    let fixture = "testdata/pptx/minimal-title/presentation.pptx";
    let dry_run_args = [
        "--json",
        "pptx",
        "replace",
        "text-occurrences",
        fixture,
        "--match-text",
        "Minimal",
        "--new-text",
        "Tiny",
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, go_code, "replace text occurrences dry-run exit");
    assert_eq!(
        rust_stderr, go_stderr,
        "replace text occurrences dry-run stderr"
    );
    let rust_dry_json = rust_stdout.expect("rust replace text occurrences dry-run");
    assert_eq!(
        rust_dry_json,
        go_stdout.expect("go replace text occurrences dry-run"),
        "replace text occurrences dry-run stdout"
    );
    let plan_hash = rust_dry_json["staleGuard"]["actualPlanHash"]
        .as_str()
        .expect("dry-run plan hash");

    let go_out = temp_dir.join("go-occurrences.pptx");
    let rust_out = temp_dir.join("rust-occurrences.pptx");
    let go_out_str = go_out.to_str().expect("go occurrences output path");
    let rust_out_str = rust_out.to_str().expect("rust occurrences output path");
    let go_args = [
        "--json",
        "pptx",
        "replace",
        "text-occurrences",
        fixture,
        "--match-text",
        "Minimal",
        "--new-text",
        "Tiny",
        "--expect-count",
        "1",
        "--expect-plan-hash",
        plan_hash,
        "--out",
        go_out_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "replace",
        "text-occurrences",
        fixture,
        "--match-text",
        "Minimal",
        "--new-text",
        "Tiny",
        "--expect-count",
        "1",
        "--expect-plan-hash",
        plan_hash,
        "--out",
        rust_out_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "replace text occurrences saved exit");
    assert_eq!(
        rust_stderr, go_stderr,
        "replace text occurrences saved stderr"
    );
    let rust_json = rust_stdout.expect("rust replace text occurrences saved");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_out_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go replace text occurrences saved"),
            go_out_str,
            "[OUT]"
        ),
        "replace text occurrences saved stdout"
    );
    assert!(
        go_out.exists(),
        "Go replace text occurrences output missing"
    );
    assert!(
        rust_out.exists(),
        "Rust replace text occurrences output missing"
    );
    assert_rust_emitted_ooxml_command_succeeds(&rust_json["matches"][0], "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    let (go_text_code, go_text_stdout, go_text_stderr) =
        run_go_ooxml(&["--json", "pptx", "extract", "text", go_out_str]);
    let (rust_text_code, rust_text_stdout, rust_text_stderr) =
        run_ooxml(&["--json", "pptx", "extract", "text", rust_out_str]);
    assert_eq!(rust_text_code, go_text_code, "text readback exit");
    assert_eq!(rust_text_stderr, go_text_stderr, "text readback stderr");
    assert_eq!(
        scrub_path(
            rust_text_stdout.expect("rust text readback"),
            rust_out_str,
            "[OUT]"
        ),
        scrub_path(
            go_text_stdout.expect("go text readback"),
            go_out_str,
            "[OUT]"
        ),
        "text readback stdout"
    );

    let count_mismatch = [
        "--json",
        "pptx",
        "replace",
        "text-occurrences",
        fixture,
        "--match-text",
        "Minimal",
        "--new-text",
        "Tiny",
        "--expect-count",
        "2",
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&count_mismatch);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&count_mismatch);
    assert_eq!(rust_code, go_code, "replace text occurrences guard exit");
    assert_eq!(
        rust_stdout, go_stdout,
        "replace text occurrences guard stdout"
    );
    assert_eq!(
        rust_stderr, go_stderr,
        "replace text occurrences guard stderr"
    );

    let no_match = temp_dir.join("no-match.pptx");
    let no_match_str = no_match.to_str().expect("no match path");
    let no_match_args = [
        "--json",
        "pptx",
        "replace",
        "text-occurrences",
        fixture,
        "--match-text",
        "Missing",
        "--new-text",
        "Tiny",
        "--out",
        no_match_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&no_match_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&no_match_args);
    assert_eq!(rust_code, go_code, "replace text occurrences no-match exit");
    assert_eq!(
        rust_stdout, go_stdout,
        "replace text occurrences no-match stdout"
    );
    assert_eq!(
        rust_stderr, go_stderr,
        "replace text occurrences no-match stderr"
    );
}

#[test]
fn pptx_replace_images_saved_readback_dry_run_and_errors_match_go_oracle() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-replace-images-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("replace images temp dir");

    let fixture = "testdata/pptx/slide-assembly-notes-media/presentation.pptx";
    let image = "testdata/test_image.png";
    let dry_run_args = [
        "--json",
        "pptx",
        "replace",
        "images",
        fixture,
        "--slide",
        "2",
        "--target",
        "shape:4",
        "--image",
        image,
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, go_code, "replace images dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "replace images dry-run stderr");
    assert_eq!(
        rust_stdout.expect("rust replace images dry-run"),
        go_stdout.expect("go replace images dry-run"),
        "replace images dry-run stdout"
    );

    let go_out = temp_dir.join("go-image.pptx");
    let rust_out = temp_dir.join("rust-image.pptx");
    let go_out_str = go_out.to_str().expect("go image output path");
    let rust_out_str = rust_out.to_str().expect("rust image output path");
    let go_args = [
        "--json", "pptx", "replace", "images", fixture, "--slide", "2", "--target", "shape:4",
        "--image", image, "--out", go_out_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "replace",
        "images",
        fixture,
        "--slide",
        "2",
        "--target",
        "shape:4",
        "--image",
        image,
        "--out",
        rust_out_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "replace images saved exit");
    assert_eq!(rust_stderr, go_stderr, "replace images saved stderr");
    let rust_json = rust_stdout.expect("rust replace images saved");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_out_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go replace images saved"),
            go_out_str,
            "[OUT]"
        ),
        "replace images saved stdout"
    );
    assert!(go_out.exists(), "Go replace images output missing");
    assert!(rust_out.exists(), "Rust replace images output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    let go_dir = temp_dir.join("go-image-extract");
    let rust_dir = temp_dir.join("rust-image-extract");
    let go_dir_str = go_dir.to_str().expect("go image extract dir");
    let rust_dir_str = rust_dir.to_str().expect("rust image extract dir");
    let (go_extract_code, go_extract_stdout, go_extract_stderr) = run_go_ooxml(&[
        "--json", "pptx", "extract", "images", go_out_str, "--out", go_dir_str,
    ]);
    let (rust_extract_code, rust_extract_stdout, rust_extract_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "extract",
        "images",
        rust_out_str,
        "--out",
        rust_dir_str,
    ]);
    assert_eq!(rust_extract_code, go_extract_code, "image readback exit");
    assert_eq!(
        rust_extract_stderr, go_extract_stderr,
        "image readback stderr"
    );
    assert_eq!(
        scrub_paths(
            rust_extract_stdout.expect("rust image readback"),
            &[(rust_out_str, "[PPTX]"), (rust_dir_str, "[OUT]")]
        ),
        scrub_paths(
            go_extract_stdout.expect("go image readback"),
            &[(go_out_str, "[PPTX]"), (go_dir_str, "[OUT]")]
        ),
        "image readback stdout"
    );
    assert_export_dirs_match(&go_dir, &rust_dir);

    let missing_target = [
        "--json",
        "pptx",
        "replace",
        "images",
        fixture,
        "--slide",
        "2",
        "--target",
        "shape:999",
        "--image",
        image,
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&missing_target);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&missing_target);
    assert_eq!(rust_code, go_code, "replace images missing target exit");
    assert_eq!(
        rust_stdout, go_stdout,
        "replace images missing target stdout"
    );
    assert_eq!(
        rust_stderr, go_stderr,
        "replace images missing target stderr"
    );
}

#[test]
fn pptx_replace_images_for_slides_saved_dry_run_and_invalid_cases_match_go_oracle() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-replace-images-for-slides-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("replace images for-slides temp dir");

    let fixture = "testdata/pptx/picture-placeholder/presentation.pptx";
    let image = "testdata/test_image.png";
    let dry_run_args = [
        "--json",
        "pptx",
        "replace",
        "images",
        fixture,
        "--for-slides",
        "1-2",
        "--target",
        "shape:2",
        "--image",
        image,
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, go_code, "replace images for-slides dry-run exit");
    assert_eq!(
        rust_stderr, go_stderr,
        "replace images for-slides dry-run stderr"
    );
    assert_eq!(
        rust_stdout.expect("rust replace images for-slides dry-run"),
        go_stdout.expect("go replace images for-slides dry-run"),
        "replace images for-slides dry-run stdout"
    );

    let go_out = temp_dir.join("go-for-slides.pptx");
    let rust_out = temp_dir.join("rust-for-slides.pptx");
    let go_out_str = go_out.to_str().expect("go for-slides output path");
    let rust_out_str = rust_out.to_str().expect("rust for-slides output path");
    let go_args = [
        "--json",
        "pptx",
        "replace",
        "images",
        fixture,
        "--for-slides",
        "1-2",
        "--target",
        "shape:2",
        "--image",
        image,
        "--out",
        go_out_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "replace",
        "images",
        fixture,
        "--for-slides",
        "1-2",
        "--target",
        "shape:2",
        "--image",
        image,
        "--out",
        rust_out_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "replace images for-slides saved exit");
    assert_eq!(
        rust_stderr, go_stderr,
        "replace images for-slides saved stderr"
    );
    assert_eq!(
        rust_stdout.expect("rust replace images for-slides saved"),
        go_stdout.expect("go replace images for-slides saved"),
        "replace images for-slides saved stdout"
    );
    assert!(go_out.exists(), "Go for-slides output missing");
    assert!(rust_out.exists(), "Rust for-slides output missing");

    let (validate_code, validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "validate", rust_out_str, "--strict"]);
    assert_eq!(validate_code, 0, "strict validate exit");
    assert!(validate_stdout.is_some(), "strict validate stdout");
    assert_eq!(validate_stderr, None, "strict validate stderr");
    let (conformance_code, conformance_stdout, conformance_stderr) =
        run_ooxml(&["--json", "conformance", "check", rust_out_str]);
    assert_eq!(conformance_code, 0, "conformance check exit");
    assert!(conformance_stdout.is_some(), "conformance check stdout");
    assert_eq!(conformance_stderr, None, "conformance check stderr");

    let go_dir = temp_dir.join("go-for-slides-extract");
    let rust_dir = temp_dir.join("rust-for-slides-extract");
    let go_dir_str = go_dir.to_str().expect("go for-slides extract dir");
    let rust_dir_str = rust_dir.to_str().expect("rust for-slides extract dir");
    let (go_extract_code, go_extract_stdout, go_extract_stderr) = run_go_ooxml(&[
        "--json", "pptx", "extract", "images", go_out_str, "--out", go_dir_str,
    ]);
    let (rust_extract_code, rust_extract_stdout, rust_extract_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "extract",
        "images",
        rust_out_str,
        "--out",
        rust_dir_str,
    ]);
    assert_eq!(
        rust_extract_code, go_extract_code,
        "for-slides image readback exit"
    );
    assert_eq!(
        rust_extract_stderr, go_extract_stderr,
        "for-slides image readback stderr"
    );
    assert_eq!(
        scrub_paths(
            rust_extract_stdout.expect("rust for-slides image readback"),
            &[(rust_out_str, "[PPTX]"), (rust_dir_str, "[OUT]")]
        ),
        scrub_paths(
            go_extract_stdout.expect("go for-slides image readback"),
            &[(go_out_str, "[PPTX]"), (go_dir_str, "[OUT]")]
        ),
        "for-slides image readback stdout"
    );
    assert_export_dirs_match(&go_dir, &rust_dir);

    for (name, args) in [
        (
            "combined slide and for-slides",
            vec![
                "--json",
                "pptx",
                "replace",
                "images",
                fixture,
                "--slide",
                "2",
                "--for-slides",
                "1-2",
                "--target",
                "shape:2",
                "--image",
                image,
                "--dry-run",
            ],
        ),
        (
            "invalid for-slides range",
            vec![
                "--json",
                "pptx",
                "replace",
                "images",
                fixture,
                "--for-slides",
                "2-1",
                "--target",
                "shape:2",
                "--image",
                image,
                "--dry-run",
            ],
        ),
        (
            "handle target with for-slides",
            vec![
                "--json",
                "pptx",
                "replace",
                "images",
                fixture,
                "--for-slides",
                "2",
                "--target",
                "H:pptx/s:257/shape:n:2",
                "--image",
                image,
                "--dry-run",
            ],
        ),
        (
            "unsupported selector is per-slide batch error",
            vec![
                "--json",
                "pptx",
                "replace",
                "images",
                fixture,
                "--for-slides",
                "2",
                "--target",
                "body",
                "--image",
                image,
                "--dry-run",
            ],
        ),
    ] {
        let (go_code, go_stdout, go_stderr) = run_go_ooxml(&args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&args);
        assert_eq!(rust_code, go_code, "{name} exit");
        assert_eq!(rust_stdout, go_stdout, "{name} stdout");
        assert_eq!(rust_stderr, go_stderr, "{name} stderr");
    }
}

#[test]
fn pptx_extract_xml_artifacts_selectors_and_errors_match_go_oracle() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-extract-xml-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("extract xml temp dir");

    let fixture = "testdata/pptx/multi-layout/presentation.pptx";
    let go_dir = temp_dir.join("go-xml");
    let rust_dir = temp_dir.join("rust-xml");
    let go_dir_str = go_dir.to_str().expect("go xml dir");
    let rust_dir_str = rust_dir.to_str().expect("rust xml dir");
    let go_args = [
        "--json", "pptx", "extract", "xml", fixture, "--slide", "1", "--layout", "1", "--master",
        "1", "--out", go_dir_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "extract",
        "xml",
        fixture,
        "--slide",
        "1",
        "--layout",
        "1",
        "--master",
        "1",
        "--out",
        rust_dir_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "extract xml exit");
    assert_eq!(rust_stderr, go_stderr, "extract xml stderr");
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust extract xml stdout"),
            rust_dir_str,
            "[OUT]"
        ),
        scrub_path(
            go_stdout.expect("go extract xml stdout"),
            go_dir_str,
            "[OUT]"
        ),
        "extract xml stdout"
    );
    assert_export_dirs_match(&go_dir, &rust_dir);

    let missing_out = [
        "--json",
        "pptx",
        "extract",
        "xml",
        "testdata/pptx/minimal-title/presentation.pptx",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&missing_out);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&missing_out);
    assert_eq!(rust_code, go_code, "extract xml missing out exit");
    assert_eq!(rust_stdout, go_stdout, "extract xml missing out stdout");
    assert_eq!(rust_stderr, go_stderr, "extract xml missing out stderr");

    let bad_layout_dir = temp_dir.join("bad-layout");
    let bad_layout_dir_str = bad_layout_dir.to_str().expect("bad layout dir");
    let bad_layout = [
        "--json",
        "pptx",
        "extract",
        "xml",
        "testdata/pptx/minimal-title/presentation.pptx",
        "--layout",
        "99",
        "--out",
        bad_layout_dir_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&bad_layout);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&bad_layout);
    assert_eq!(rust_code, go_code, "extract xml bad layout exit");
    assert_eq!(rust_stdout, go_stdout, "extract xml bad layout stdout");
    assert_eq!(rust_stderr, go_stderr, "extract xml bad layout stderr");
}

#[test]
fn pptx_notes_set_clear_dry_run_and_errors_match_go_oracle() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-notes-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("notes temp dir");

    let set_fixture = "testdata/pptx/title-content/presentation.pptx";
    let notes_fixture = "testdata/pptx/notes-slide/presentation.pptx";
    let go_set = temp_dir.join("go-set.pptx");
    let rust_set = temp_dir.join("rust-set.pptx");
    let go_set_str = go_set.to_str().expect("go set path");
    let rust_set_str = rust_set.to_str().expect("rust set path");
    let set_text = "First line\nSecond line";

    let go_set_args = [
        "--json",
        "pptx",
        "notes",
        "set",
        set_fixture,
        "--slide",
        "1",
        "--text",
        set_text,
        "--out",
        go_set_str,
    ];
    let rust_set_args = [
        "--json",
        "pptx",
        "notes",
        "set",
        set_fixture,
        "--slide",
        "1",
        "--text",
        set_text,
        "--out",
        rust_set_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_set_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_set_args);
    assert_eq!(rust_code, go_code, "notes set exit");
    assert_eq!(rust_stderr, go_stderr, "notes set stderr");
    let rust_set_json = rust_stdout.expect("rust notes set stdout");
    assert_eq!(
        scrub_path(rust_set_json.clone(), rust_set_str, "[OUT]"),
        scrub_path(go_stdout.expect("go notes set stdout"), go_set_str, "[OUT]"),
        "notes set stdout"
    );
    assert!(go_set.exists(), "Go notes set output missing");
    assert!(rust_set.exists(), "Rust notes set output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_set_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_set_json, "validateCommand");

    let (go_show_code, go_show_stdout, go_show_stderr) = run_go_ooxml(&[
        "--json", "pptx", "notes", "show", go_set_str, "--slide", "1",
    ]);
    let (rust_show_code, rust_show_stdout, rust_show_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "notes",
        "show",
        rust_set_str,
        "--slide",
        "1",
    ]);
    assert_eq!(rust_show_code, go_show_code, "notes set readback exit");
    assert_eq!(
        rust_show_stderr, go_show_stderr,
        "notes set readback stderr"
    );
    assert_eq!(
        rust_show_stdout.expect("rust set readback"),
        go_show_stdout.expect("go set readback"),
        "notes set readback stdout"
    );

    let go_clear = temp_dir.join("go-clear.pptx");
    let rust_clear = temp_dir.join("rust-clear.pptx");
    let go_clear_str = go_clear.to_str().expect("go clear path");
    let rust_clear_str = rust_clear.to_str().expect("rust clear path");
    let go_clear_args = [
        "--json",
        "pptx",
        "notes",
        "clear",
        notes_fixture,
        "--slide",
        "2",
        "--out",
        go_clear_str,
    ];
    let rust_clear_args = [
        "--json",
        "pptx",
        "notes",
        "clear",
        notes_fixture,
        "--slide",
        "2",
        "--out",
        rust_clear_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_clear_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_clear_args);
    assert_eq!(rust_code, go_code, "notes clear exit");
    assert_eq!(rust_stderr, go_stderr, "notes clear stderr");
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust notes clear stdout"),
            rust_clear_str,
            "[OUT]"
        ),
        scrub_path(
            go_stdout.expect("go notes clear stdout"),
            go_clear_str,
            "[OUT]"
        ),
        "notes clear stdout"
    );

    let (go_clear_show_code, go_clear_show_stdout, go_clear_show_stderr) = run_go_ooxml(&[
        "--json",
        "pptx",
        "notes",
        "show",
        go_clear_str,
        "--slide",
        "2",
    ]);
    let (rust_clear_show_code, rust_clear_show_stdout, rust_clear_show_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "notes",
        "show",
        rust_clear_str,
        "--slide",
        "2",
    ]);
    assert_eq!(
        rust_clear_show_code, go_clear_show_code,
        "notes clear readback exit"
    );
    assert_eq!(
        rust_clear_show_stderr, go_clear_show_stderr,
        "notes clear readback stderr"
    );
    assert_eq!(
        rust_clear_show_stdout.expect("rust clear readback"),
        go_clear_show_stdout.expect("go clear readback"),
        "notes clear readback stdout"
    );

    let dry_run_args = [
        "--json",
        "pptx",
        "notes",
        "set",
        set_fixture,
        "--slide",
        "1",
        "--text",
        "draft notes",
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, go_code, "notes set dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "notes set dry-run stderr");
    assert_eq!(
        rust_stdout.expect("rust notes set dry-run"),
        go_stdout.expect("go notes set dry-run"),
        "notes set dry-run stdout"
    );

    let out_of_range = [
        "--json",
        "pptx",
        "notes",
        "set",
        "testdata/pptx/minimal-title/presentation.pptx",
        "--slide",
        "99",
        "--text",
        "x",
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&out_of_range);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&out_of_range);
    assert_eq!(rust_code, go_code, "notes set out-of-range exit");
    assert_eq!(rust_stdout, go_stdout, "notes set out-of-range stdout");
    assert_eq!(rust_stderr, go_stderr, "notes set out-of-range stderr");
}

#[test]
fn pptx_shapes_get_set_bounds_delete_saved_readback_dry_run_and_errors_match_go_oracle() {
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&get_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&get_args);
    assert_eq!(rust_code, go_code, "shapes get exit");
    assert_eq!(rust_stderr, go_stderr, "shapes get stderr");
    assert_eq!(
        rust_stdout.expect("rust shapes get stdout"),
        go_stdout.expect("go shapes get stdout"),
        "shapes get stdout"
    );

    let go_bounds_out = temp_dir.join("go-set-bounds.pptx");
    let rust_bounds_out = temp_dir.join("rust-set-bounds.pptx");
    let go_bounds_out_str = go_bounds_out.to_str().expect("go set-bounds path");
    let rust_bounds_out_str = rust_bounds_out.to_str().expect("rust set-bounds path");
    let go_set_args = [
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
        go_bounds_out_str,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_set_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_set_args);
    assert_eq!(rust_code, go_code, "set-bounds saved exit");
    assert_eq!(rust_stderr, go_stderr, "set-bounds saved stderr");
    let rust_set_json = rust_stdout.expect("rust set-bounds stdout");
    assert_eq!(
        scrub_path(rust_set_json.clone(), rust_bounds_out_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go set-bounds stdout"),
            go_bounds_out_str,
            "[OUT]"
        ),
        "set-bounds saved stdout"
    );
    assert!(go_bounds_out.exists(), "Go set-bounds output missing");
    assert!(rust_bounds_out.exists(), "Rust set-bounds output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_set_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_set_json, "validateCommand");

    let (go_read_code, go_read_stdout, go_read_stderr) = run_go_ooxml(&[
        "--json",
        "pptx",
        "shapes",
        "get",
        go_bounds_out_str,
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
    assert_eq!(rust_read_code, go_read_code, "set-bounds readback exit");
    assert_eq!(
        rust_read_stderr, go_read_stderr,
        "set-bounds readback stderr"
    );
    assert_eq!(
        scrub_path(
            rust_read_stdout.expect("rust set-bounds readback"),
            rust_bounds_out_str,
            "[OUT]"
        ),
        scrub_path(
            go_read_stdout.expect("go set-bounds readback"),
            go_bounds_out_str,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&set_dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&set_dry_run_args);
    assert_eq!(rust_code, go_code, "set-bounds dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "set-bounds dry-run stderr");
    assert_eq!(
        rust_stdout.expect("rust set-bounds dry-run stdout"),
        go_stdout.expect("go set-bounds dry-run stdout"),
        "set-bounds dry-run stdout"
    );

    let go_delete_out = temp_dir.join("go-delete-shape.pptx");
    let rust_delete_out = temp_dir.join("rust-delete-shape.pptx");
    let go_delete_out_str = go_delete_out.to_str().expect("go delete path");
    let rust_delete_out_str = rust_delete_out.to_str().expect("rust delete path");
    let go_delete_args = [
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
        go_delete_out_str,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_delete_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_delete_args);
    assert_eq!(rust_code, go_code, "delete saved exit");
    assert_eq!(rust_stderr, go_stderr, "delete saved stderr");
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust delete stdout"),
            rust_delete_out_str,
            "[OUT]"
        ),
        scrub_path(
            go_stdout.expect("go delete stdout"),
            go_delete_out_str,
            "[OUT]"
        ),
        "delete saved stdout"
    );
    assert!(go_delete_out.exists(), "Go delete output missing");
    assert!(rust_delete_out.exists(), "Rust delete output missing");
    let (validate_code, validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "validate", "--strict", rust_delete_out_str]);
    assert_eq!(validate_code, 0, "delete strict validate exit");
    assert_eq!(validate_stderr, None, "delete strict validate stderr");
    assert!(validate_stdout.is_some(), "delete strict validate stdout");

    let (go_show_code, go_show_stdout, go_show_stderr) = run_go_ooxml(&[
        "--json",
        "pptx",
        "shapes",
        "show",
        go_delete_out_str,
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
    assert_eq!(rust_show_code, go_show_code, "delete readback show exit");
    assert_eq!(
        rust_show_stderr, go_show_stderr,
        "delete readback show stderr"
    );
    assert_eq!(
        scrub_path(
            rust_show_stdout.expect("rust delete readback show"),
            rust_delete_out_str,
            "[OUT]"
        ),
        scrub_path(
            go_show_stdout.expect("go delete readback show"),
            go_delete_out_str,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&delete_dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&delete_dry_run_args);
    assert_eq!(rust_code, go_code, "delete dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "delete dry-run stderr");
    assert_eq!(
        rust_stdout.expect("rust delete dry-run stdout"),
        go_stdout.expect("go delete dry-run stdout"),
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
        let (go_code, go_stdout, go_stderr) = run_go_ooxml(&args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&args);
        assert_eq!(rust_code, go_code, "shape error exit for {args:?}");
        assert_eq!(rust_stdout, go_stdout, "shape error stdout for {args:?}");
        assert_eq!(rust_stderr, go_stderr, "shape error stderr for {args:?}");
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&conformance_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&conformance_args);
    assert_eq!(rust_code, go_code, "nested group delete conformance exit");
    assert_eq!(
        rust_stderr, go_stderr,
        "nested group delete conformance stderr"
    );
    assert_eq!(
        rust_stdout, go_stdout,
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
fn pptx_layouts_mutations_saved_readback_and_dry_run_match_go_oracle() {
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

    let go_rename = temp_dir.join("go-rename.pptx");
    let rust_rename = temp_dir.join("rust-rename.pptx");
    let go_rename_str = go_rename.to_str().expect("go rename path");
    let rust_rename_str = rust_rename.to_str().expect("rust rename path");
    let go_rename_args = [
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
        go_rename_str,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_rename_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_rename_args);
    assert_eq!(rust_code, go_code, "layout rename exit");
    assert_eq!(rust_stderr, go_stderr, "layout rename stderr");
    let rust_rename_json = rust_stdout.expect("rust layout rename stdout");
    assert_eq!(
        scrub_path(rust_rename_json.clone(), rust_rename_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go layout rename stdout"),
            go_rename_str,
            "[OUT]"
        ),
        "layout rename stdout"
    );
    assert!(go_rename.exists(), "Go layout rename output missing");
    assert!(rust_rename.exists(), "Rust layout rename output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_rename_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_rename_json, "validateCommand");

    let (go_show_code, go_show_stdout, go_show_stderr) = run_go_ooxml(&[
        "--json",
        "pptx",
        "layouts",
        "show",
        go_rename_str,
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
    assert_eq!(rust_show_code, go_show_code, "layout rename readback exit");
    assert_eq!(
        rust_show_stderr, go_show_stderr,
        "layout rename readback stderr"
    );
    assert_eq!(
        rust_show_stdout.expect("rust layout rename readback"),
        go_show_stdout.expect("go layout rename readback"),
        "layout rename readback stdout"
    );

    let go_bounds = temp_dir.join("go-bounds.pptx");
    let rust_bounds = temp_dir.join("rust-bounds.pptx");
    let go_bounds_str = go_bounds.to_str().expect("go bounds path");
    let rust_bounds_str = rust_bounds.to_str().expect("rust bounds path");
    let go_bounds_args = [
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
        go_bounds_str,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_bounds_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_bounds_args);
    assert_eq!(rust_code, go_code, "layout set-bounds exit");
    assert_eq!(rust_stderr, go_stderr, "layout set-bounds stderr");
    let rust_bounds_json = rust_stdout.expect("rust layout set-bounds stdout");
    assert_eq!(
        scrub_path(rust_bounds_json.clone(), rust_bounds_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go layout set-bounds stdout"),
            go_bounds_str,
            "[OUT]"
        ),
        "layout set-bounds stdout"
    );
    assert_rust_emitted_ooxml_command_succeeds(&rust_bounds_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_bounds_json, "validateCommand");
    let (go_show_code, go_show_stdout, go_show_stderr) = run_go_ooxml(&[
        "--json",
        "pptx",
        "layouts",
        "show",
        go_bounds_str,
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
        rust_show_code, go_show_code,
        "layout set-bounds readback exit"
    );
    assert_eq!(
        rust_show_stderr, go_show_stderr,
        "layout set-bounds readback stderr"
    );
    assert_eq!(
        rust_show_stdout.expect("rust layout set-bounds readback"),
        go_show_stdout.expect("go layout set-bounds readback"),
        "layout set-bounds readback stdout"
    );

    let go_delete = temp_dir.join("go-delete.pptx");
    let rust_delete = temp_dir.join("rust-delete.pptx");
    let go_delete_str = go_delete.to_str().expect("go delete path");
    let rust_delete_str = rust_delete.to_str().expect("rust delete path");
    let go_delete_args = [
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
        go_delete_str,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_delete_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_delete_args);
    assert_eq!(rust_code, go_code, "layout delete-shape exit");
    assert_eq!(rust_stderr, go_stderr, "layout delete-shape stderr");
    let rust_delete_json = rust_stdout.expect("rust layout delete-shape stdout");
    assert_eq!(
        scrub_path(rust_delete_json.clone(), rust_delete_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go layout delete-shape stdout"),
            go_delete_str,
            "[OUT]"
        ),
        "layout delete-shape stdout"
    );
    assert_rust_emitted_ooxml_command_succeeds(&rust_delete_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_delete_json, "validateCommand");
    let (go_show_code, go_show_stdout, go_show_stderr) = run_go_ooxml(&[
        "--json",
        "pptx",
        "layouts",
        "show",
        go_delete_str,
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
        rust_show_code, go_show_code,
        "layout delete-shape readback exit"
    );
    assert_eq!(
        rust_show_stderr, go_show_stderr,
        "layout delete-shape readback stderr"
    );
    assert_eq!(
        rust_show_stdout.expect("rust layout delete-shape readback"),
        go_show_stdout.expect("go layout delete-shape readback"),
        "layout delete-shape readback stdout"
    );

    let go_add = temp_dir.join("go-add-placeholder.pptx");
    let rust_add = temp_dir.join("rust-add-placeholder.pptx");
    let go_add_str = go_add.to_str().expect("go add-placeholder path");
    let rust_add_str = rust_add.to_str().expect("rust add-placeholder path");
    let go_add_args = [
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
        go_add_str,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_add_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_add_args);
    assert_eq!(rust_code, go_code, "layout add-placeholder exit");
    assert_eq!(rust_stderr, go_stderr, "layout add-placeholder stderr");
    let rust_add_json = rust_stdout.expect("rust layout add-placeholder stdout");
    assert_eq!(
        scrub_path(rust_add_json.clone(), rust_add_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go layout add-placeholder stdout"),
            go_add_str,
            "[OUT]"
        ),
        "layout add-placeholder stdout"
    );
    assert_rust_emitted_ooxml_command_succeeds(&rust_add_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_add_json, "validateCommand");
    let (go_show_code, go_show_stdout, go_show_stderr) = run_go_ooxml(&[
        "--json", "pptx", "layouts", "show", go_add_str, "--layout", "7",
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
        rust_show_code, go_show_code,
        "layout add-placeholder readback exit"
    );
    assert_eq!(
        rust_show_stderr, go_show_stderr,
        "layout add-placeholder readback stderr"
    );
    assert_eq!(
        rust_show_stdout.expect("rust layout add-placeholder readback"),
        go_show_stdout.expect("go layout add-placeholder readback"),
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, go_code, "layout rename dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "layout rename dry-run stderr");
    assert_eq!(
        rust_stdout.expect("rust layout rename dry-run stdout"),
        go_stdout.expect("go layout rename dry-run stdout"),
        "layout rename dry-run stdout"
    );
}

#[test]
fn pptx_layout_slide_authoring_commands_match_go_oracle_and_validate() {
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
        assert_go_rust_match(&args);
    }

    let go_layout = temp_dir.join("go-layout-clone.pptx");
    let rust_layout = temp_dir.join("rust-layout-clone.pptx");
    let go_layout_str = go_layout.to_str().expect("go layout clone path");
    let rust_layout_str = rust_layout.to_str().expect("rust layout clone path");
    let go_args = [
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
        go_layout_str,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "layout clone exit");
    assert_eq!(rust_stderr, go_stderr, "layout clone stderr");
    let rust_json = rust_stdout.expect("rust layout clone stdout");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_layout_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go layout clone stdout"),
            go_layout_str,
            "[OUT]"
        ),
        "layout clone stdout"
    );
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    let (go_show_code, go_show_stdout, go_show_stderr) = run_go_ooxml(&[
        "--json",
        "pptx",
        "layouts",
        "show",
        go_layout_str,
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
    assert_eq!(rust_show_code, go_show_code, "layout clone readback exit");
    assert_eq!(
        rust_show_stderr, go_show_stderr,
        "layout clone readback stderr"
    );
    assert_eq!(
        rust_show_stdout.expect("rust layout clone readback"),
        go_show_stdout.expect("go layout clone readback"),
        "layout clone readback stdout"
    );

    let go_master = temp_dir.join("go-master-placeholder.pptx");
    let rust_master = temp_dir.join("rust-master-placeholder.pptx");
    let go_master_str = go_master.to_str().expect("go master placeholder path");
    let rust_master_str = rust_master.to_str().expect("rust master placeholder path");
    let go_args = [
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
        go_master_str,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "master add-placeholder exit");
    assert_eq!(rust_stderr, go_stderr, "master add-placeholder stderr");
    let rust_json = rust_stdout.expect("rust master add-placeholder stdout");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_master_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go master add-placeholder stdout"),
            go_master_str,
            "[OUT]"
        ),
        "master add-placeholder stdout"
    );
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    let go_clone = temp_dir.join("go-clone-slide.pptx");
    let rust_clone = temp_dir.join("rust-clone-slide.pptx");
    let go_clone_str = go_clone.to_str().expect("go clone-slide path");
    let rust_clone_str = rust_clone.to_str().expect("rust clone-slide path");
    let go_args = [
        "--json",
        "pptx",
        "clone-slide",
        fixture,
        "--slide",
        "1",
        "--out",
        go_clone_str,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "clone-slide exit");
    assert_eq!(rust_stderr, go_stderr, "clone-slide stderr");
    let rust_json = rust_stdout.expect("rust clone-slide stdout");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_clone_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go clone-slide stdout"),
            go_clone_str,
            "[OUT]"
        ),
        "clone-slide stdout"
    );
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    let go_new = temp_dir.join("go-new-slide.pptx");
    let rust_new = temp_dir.join("rust-new-slide.pptx");
    let go_new_str = go_new.to_str().expect("go new slide path");
    let rust_new_str = rust_new.to_str().expect("rust new slide path");
    let go_args = [
        "--json",
        "pptx",
        "new-slide-from-layout",
        fixture,
        "--layout",
        "1",
        "--set-text",
        "title=RustTitle",
        "--out",
        go_new_str,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "new-slide-from-layout exit");
    assert_eq!(rust_stderr, go_stderr, "new-slide-from-layout stderr");
    let rust_json = rust_stdout.expect("rust new-slide-from-layout stdout");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_new_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go new-slide-from-layout stdout"),
            go_new_str,
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
    let go_image_slot = temp_dir.join("go-new-slide-image-slot.pptx");
    let rust_image_slot = temp_dir.join("rust-new-slide-image-slot.pptx");
    let go_image_slot_str = go_image_slot.to_str().expect("go image slot path");
    let rust_image_slot_str = rust_image_slot.to_str().expect("rust image slot path");
    let go_args = [
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
        go_image_slot_str,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "new-slide image-slot exit");
    assert_eq!(rust_stderr, go_stderr, "new-slide image-slot stderr");
    let rust_json = rust_stdout.expect("rust new-slide image-slot stdout");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_image_slot_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go new-slide image-slot stdout"),
            go_image_slot_str,
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
fn pptx_clone_slide_clones_notes_part_and_backlink_like_go() {
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
    let go_out = temp_dir.join("go-clone-notes.pptx");
    let rust_out = temp_dir.join("rust-clone-notes.pptx");
    let go_out_str = go_out.to_str().expect("go clone notes path");
    let rust_out_str = rust_out.to_str().expect("rust clone notes path");
    let go_args = [
        "--json",
        "pptx",
        "clone-slide",
        fixture,
        "--slide",
        "1",
        "--out",
        go_out_str,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "clone notes exit");
    assert_eq!(rust_stderr, go_stderr, "clone notes stderr");
    let rust_json = rust_stdout.expect("rust clone notes stdout");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_out_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go clone notes stdout"),
            go_out_str,
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
fn pptx_import_merge_authoring_commands_match_go_oracle_and_validate() {
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
        assert_go_rust_json_match(&args, label);
    }

    let go_source = temp_dir.join("go-renamed-source.pptx");
    let rust_source = temp_dir.join("rust-renamed-source.pptx");
    let go_source_str = go_source.to_str().expect("go renamed source path");
    let rust_source_str = rust_source.to_str().expect("rust renamed source path");
    let go_rename_args = [
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
        go_source_str,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_rename_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_rename_args);
    assert_eq!(rust_code, go_code, "renamed import source exit");
    assert_eq!(rust_stderr, go_stderr, "renamed import source stderr");
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust renamed source stdout"),
            rust_source_str,
            "[SOURCE]"
        ),
        scrub_path(
            go_stdout.expect("go renamed source stdout"),
            go_source_str,
            "[SOURCE]"
        ),
        "renamed import source stdout"
    );

    let go_import_slide = temp_dir.join("go-import-slide.pptx");
    let rust_import_slide = temp_dir.join("rust-import-slide.pptx");
    let go_import_slide_str = go_import_slide.to_str().expect("go import-slide path");
    let rust_import_slide_str = rust_import_slide.to_str().expect("rust import-slide path");
    let go_args = [
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
        go_import_slide_str,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "slides import-slide saved exit");
    assert_eq!(rust_stderr, go_stderr, "slides import-slide saved stderr");
    assert_eq!(
        rust_stdout.expect("rust import-slide stdout"),
        go_stdout.expect("go import-slide stdout"),
        "slides import-slide saved stdout"
    );
    assert_go_rust_match(&["--json", "validate", "--strict", rust_import_slide_str]);

    let go_merge = temp_dir.join("go-merge.pptx");
    let rust_merge = temp_dir.join("rust-merge.pptx");
    let go_merge_str = go_merge.to_str().expect("go merge path");
    let rust_merge_str = rust_merge.to_str().expect("rust merge path");
    let go_args = [
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
        go_merge_str,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "slides merge saved exit");
    assert_eq!(rust_stderr, go_stderr, "slides merge saved stderr");
    let rust_merge_json = rust_stdout.expect("rust merge stdout");
    assert_eq!(
        scrub_paths(
            rust_merge_json.clone(),
            &[("rust-merge.pptx", "[OUT]"), (rust_merge_str, "[OUT]")]
        ),
        scrub_paths(
            go_stdout.expect("go merge stdout"),
            &[("go-merge.pptx", "[OUT]"), (go_merge_str, "[OUT]")]
        ),
        "slides merge saved stdout"
    );
    assert_go_rust_match(&["--json", "validate", "--strict", rust_merge_str]);

    let go_layout = temp_dir.join("go-layout-import.pptx");
    let rust_layout = temp_dir.join("rust-layout-import.pptx");
    let go_layout_str = go_layout.to_str().expect("go layout import path");
    let rust_layout_str = rust_layout.to_str().expect("rust layout import path");
    let go_args = [
        "--json",
        "pptx",
        "layouts",
        "import",
        target,
        "--source",
        go_source_str,
        "--layout",
        "WorkerOImportedTitle",
        "--theme-policy",
        "import",
        "--out",
        go_layout_str,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "layouts import saved exit");
    assert_eq!(rust_stderr, go_stderr, "layouts import saved stderr");
    let rust_layout_json = rust_stdout.expect("rust layouts import stdout");
    assert_eq!(
        scrub_path(rust_layout_json.clone(), rust_layout_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go layouts import stdout"),
            go_layout_str,
            "[OUT]"
        ),
        "layouts import saved stdout"
    );
    assert_rust_emitted_ooxml_command_succeeds(&rust_layout_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_layout_json, "validateCommand");

    let go_master = temp_dir.join("go-master-import.pptx");
    let rust_master = temp_dir.join("rust-master-import.pptx");
    let go_master_str = go_master.to_str().expect("go master import path");
    let rust_master_str = rust_master.to_str().expect("rust master import path");
    let go_args = [
        "--json",
        "pptx",
        "masters",
        "import",
        target,
        "--source",
        go_source_str,
        "--master",
        "1",
        "--theme-policy",
        "import",
        "--out",
        go_master_str,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "masters import saved exit");
    assert_eq!(rust_stderr, go_stderr, "masters import saved stderr");
    let rust_master_json = rust_stdout.expect("rust masters import stdout");
    assert_eq!(
        scrub_path(rust_master_json.clone(), rust_master_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go masters import stdout"),
            go_master_str,
            "[OUT]"
        ),
        "masters import saved stdout"
    );
    assert_rust_emitted_ooxml_command_succeeds(&rust_master_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_master_json, "validateCommand");
}

#[test]
fn pptx_slides_lifecycle_saved_dry_run_readback_and_errors_match_go_oracle() {
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

    let go_move = temp_dir.join("go-move.pptx");
    let rust_move = temp_dir.join("rust-move.pptx");
    let go_move_str = go_move.to_str().expect("go move path");
    let rust_move_str = rust_move.to_str().expect("rust move path");
    let go_move_args = [
        "--json",
        "pptx",
        "slides",
        "move",
        multi_fixture,
        "1",
        "3",
        "--out",
        go_move_str,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_move_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_move_args);
    assert_eq!(rust_code, go_code, "slides move exit");
    assert_eq!(rust_stderr, go_stderr, "slides move stderr");
    let rust_move_json = rust_stdout.expect("rust slides move stdout");
    assert_eq!(
        scrub_path(rust_move_json.clone(), rust_move_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go slides move stdout"),
            go_move_str,
            "[OUT]"
        ),
        "slides move stdout"
    );
    assert!(go_move.exists(), "Go slides move output missing");
    assert!(rust_move.exists(), "Rust slides move output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_move_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_succeeds(&rust_move_json, "slidesListCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_move_json, "validateCommand");
    assert_go_rust_json_match_with_path_scrub(
        &["--json", "pptx", "slides", "list", go_move_str],
        &["--json", "pptx", "slides", "list", rust_move_str],
        go_move_str,
        rust_move_str,
        "slides move readback list",
    );
    assert_go_rust_json_match_with_path_scrub(
        &["--json", "validate", "--strict", go_move_str],
        &["--json", "validate", "--strict", rust_move_str],
        go_move_str,
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
    assert_go_rust_json_match(&move_dry_run, "slides move dry-run");

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
    assert_go_rust_json_match(&move_no_op_dry_run, "slides move no-op dry-run");

    let go_delete = temp_dir.join("go-delete.pptx");
    let rust_delete = temp_dir.join("rust-delete.pptx");
    let go_delete_str = go_delete.to_str().expect("go delete path");
    let rust_delete_str = rust_delete.to_str().expect("rust delete path");
    let go_delete_args = [
        "--json",
        "pptx",
        "slides",
        "delete",
        notes_fixture,
        "2",
        "--out",
        go_delete_str,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_delete_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_delete_args);
    assert_eq!(rust_code, go_code, "slides delete exit");
    assert_eq!(rust_stderr, go_stderr, "slides delete stderr");
    let rust_delete_json = rust_stdout.expect("rust slides delete stdout");
    assert_eq!(
        scrub_path(rust_delete_json.clone(), rust_delete_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go slides delete stdout"),
            go_delete_str,
            "[OUT]"
        ),
        "slides delete stdout"
    );
    assert!(go_delete.exists(), "Go slides delete output missing");
    assert!(rust_delete.exists(), "Rust slides delete output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_delete_json, "slidesListCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_delete_json, "validateCommand");
    assert_go_rust_json_match_with_path_scrub(
        &["--json", "pptx", "slides", "list", go_delete_str],
        &["--json", "pptx", "slides", "list", rust_delete_str],
        go_delete_str,
        rust_delete_str,
        "slides delete readback list",
    );
    assert_go_rust_json_match_with_path_scrub(
        &["--json", "validate", "--strict", go_delete_str],
        &["--json", "validate", "--strict", rust_delete_str],
        go_delete_str,
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
    assert_go_rust_json_match(&delete_dry_run, "slides delete dry-run");

    let go_reorder = temp_dir.join("go-reorder.pptx");
    let rust_reorder = temp_dir.join("rust-reorder.pptx");
    let go_reorder_str = go_reorder.to_str().expect("go reorder path");
    let rust_reorder_str = rust_reorder.to_str().expect("rust reorder path");
    let go_reorder_args = [
        "--json",
        "pptx",
        "slides",
        "reorder",
        multi_fixture,
        "3,1,2,4,5",
        "--out",
        go_reorder_str,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_reorder_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_reorder_args);
    assert_eq!(rust_code, go_code, "slides reorder exit");
    assert_eq!(rust_stderr, go_stderr, "slides reorder stderr");
    let rust_reorder_json = rust_stdout.expect("rust slides reorder stdout");
    assert_eq!(
        scrub_path(rust_reorder_json.clone(), rust_reorder_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go slides reorder stdout"),
            go_reorder_str,
            "[OUT]"
        ),
        "slides reorder stdout"
    );
    assert!(go_reorder.exists(), "Go slides reorder output missing");
    assert!(rust_reorder.exists(), "Rust slides reorder output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_reorder_json, "slidesListCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_reorder_json, "validateCommand");
    assert_go_rust_json_match_with_path_scrub(
        &["--json", "pptx", "slides", "list", go_reorder_str],
        &["--json", "pptx", "slides", "list", rust_reorder_str],
        go_reorder_str,
        rust_reorder_str,
        "slides reorder readback list",
    );
    assert_go_rust_json_match_with_path_scrub(
        &["--json", "validate", "--strict", go_reorder_str],
        &["--json", "validate", "--strict", rust_reorder_str],
        go_reorder_str,
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
    assert_go_rust_json_match(&reorder_dry_run, "slides reorder dry-run");

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
        assert_go_rust_json_match(&args, label);
    }
}

fn assert_go_rust_json_match(args: &[&str], label: &str) {
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(args);
    assert_eq!(rust_code, go_code, "{label} exit");
    assert_eq!(rust_stdout, go_stdout, "{label} stdout");
    assert_eq!(rust_stderr, go_stderr, "{label} stderr");
}

fn assert_go_rust_json_match_with_path_scrub(
    go_args: &[&str],
    rust_args: &[&str],
    go_path: &str,
    rust_path: &str,
    label: &str,
) {
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(rust_args);
    assert_eq!(rust_code, go_code, "{label} exit");
    assert_eq!(
        scrub_path(rust_stdout.expect("rust stdout"), rust_path, "[OUT]"),
        scrub_path(go_stdout.expect("go stdout"), go_path, "[OUT]"),
        "{label} stdout"
    );
    assert_eq!(rust_stderr, go_stderr, "{label} stderr");
}

#[test]
fn pptx_text_set_saved_readback_dry_run_hyperlink_and_errors_match_go_oracle() {
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
    let go_out = temp_dir.join("go-text-set.pptx");
    let rust_out = temp_dir.join("rust-text-set.pptx");
    let go_out_str = go_out.to_str().expect("go text set path");
    let rust_out_str = rust_out.to_str().expect("rust text set path");

    let go_args = [
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
        go_out_str,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "text set saved exit");
    assert_eq!(rust_stderr, go_stderr, "text set saved stderr");
    let rust_json = rust_stdout.expect("rust text set stdout");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_out_str, "[OUT]"),
        scrub_path(go_stdout.expect("go text set stdout"), go_out_str, "[OUT]"),
        "text set saved stdout"
    );
    assert!(go_out.exists(), "Go text set output missing");
    assert!(rust_out.exists(), "Rust text set output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    let (go_read_code, go_read_stdout, go_read_stderr) = run_go_ooxml(&[
        "--json",
        "pptx",
        "shapes",
        "get",
        go_out_str,
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
    assert_eq!(rust_read_code, go_read_code, "text set readback exit");
    assert_eq!(rust_read_stderr, go_read_stderr, "text set readback stderr");
    assert_eq!(
        scrub_path(
            rust_read_stdout.expect("rust text set readback"),
            rust_out_str,
            "[OUT]"
        ),
        scrub_path(
            go_read_stdout.expect("go text set readback"),
            go_out_str,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, go_code, "text set dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "text set dry-run stderr");
    assert_eq!(
        rust_stdout.expect("rust text set dry-run"),
        go_stdout.expect("go text set dry-run"),
        "text set dry-run stdout"
    );

    let go_hyper = temp_dir.join("go-hyperlink.pptx");
    let rust_hyper = temp_dir.join("rust-hyperlink.pptx");
    let go_hyper_str = go_hyper.to_str().expect("go hyperlink path");
    let rust_hyper_str = rust_hyper.to_str().expect("rust hyperlink path");
    let go_hyper_args = [
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
        go_hyper_str,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_hyper_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_hyper_args);
    assert_eq!(rust_code, go_code, "text set hyperlink exit");
    assert_eq!(rust_stderr, go_stderr, "text set hyperlink stderr");
    let rust_hyper_json = rust_stdout.expect("rust hyperlink stdout");
    assert_eq!(
        scrub_path(rust_hyper_json.clone(), rust_hyper_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go hyperlink stdout"),
            go_hyper_str,
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
        assert_go_rust_json_match(&args, label);
    }
}

#[test]
fn pptx_fields_inspect_set_readback_dry_run_and_errors_match_go_oracle() {
    let header_footer_fixture = "testdata/pptx/header-footer/presentation.pptx";
    let title_content_fixture = "testdata/pptx/title-content/presentation.pptx";

    assert_go_rust_json_match(
        &["--json", "pptx", "fields", "inspect", header_footer_fixture],
        "fields inspect header-footer",
    );
    assert_go_rust_json_match(
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

    let go_out = temp_dir.join("go-fields-set.pptx");
    let rust_out = temp_dir.join("rust-fields-set.pptx");
    let go_out_str = go_out.to_str().expect("go fields set path");
    let rust_out_str = rust_out.to_str().expect("rust fields set path");
    let go_args = [
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
        go_out_str,
    ];
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "fields set saved exit");
    assert_eq!(rust_stderr, go_stderr, "fields set saved stderr");
    let rust_json = rust_stdout.expect("rust fields set stdout");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_out_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go fields set stdout"),
            go_out_str,
            "[OUT]"
        ),
        "fields set saved stdout"
    );
    assert!(go_out.exists(), "Go fields set output missing");
    assert!(rust_out.exists(), "Rust fields set output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    let (go_read_code, go_read_stdout, go_read_stderr) =
        run_go_ooxml(&["--json", "pptx", "fields", "inspect", go_out_str]);
    let (rust_read_code, rust_read_stdout, rust_read_stderr) =
        run_ooxml(&["--json", "pptx", "fields", "inspect", rust_out_str]);
    assert_eq!(rust_read_code, go_read_code, "fields readback exit");
    assert_eq!(rust_read_stderr, go_read_stderr, "fields readback stderr");
    assert_eq!(
        rust_read_stdout.expect("rust fields readback"),
        go_read_stdout.expect("go fields readback"),
        "fields readback stdout"
    );

    for (label, args) in [
        (
            "fields set dry-run",
            vec![
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
            ],
        ),
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
        assert_go_rust_json_match(&args, label);
    }
}

#[test]
fn pptx_theme_update_deck_readback_dry_run_and_errors_match_go_oracle() {
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

    assert_go_rust_json_match(
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

    let go_out = temp_dir.join("go-theme-update.pptx");
    let rust_out = temp_dir.join("rust-theme-update.pptx");
    let go_out_str = go_out.to_str().expect("go theme update path");
    let rust_out_str = rust_out.to_str().expect("rust theme update path");
    let go_args = [
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
        go_out_str,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "theme update saved exit");
    assert_eq!(rust_stderr, go_stderr, "theme update saved stderr");
    assert_eq!(
        rust_stdout.expect("rust theme update stdout"),
        go_stdout.expect("go theme update stdout"),
        "theme update saved stdout"
    );
    assert!(go_out.exists(), "Go theme update output missing");
    assert!(rust_out.exists(), "Rust theme update output missing");

    let (go_read_code, go_read_stdout, go_read_stderr) = run_go_ooxml(&[
        "--json", "pptx", "masters", "show", go_out_str, "--master", "1",
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
    assert_eq!(rust_read_code, go_read_code, "theme readback exit");
    assert_eq!(rust_read_stderr, go_read_stderr, "theme readback stderr");
    assert_eq!(
        rust_read_stdout.expect("rust theme readback"),
        go_read_stdout.expect("go theme readback"),
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
        assert_go_rust_json_match(&args, label);
    }
}

include!("pptx/tables.rs");

include!("pptx/comments.rs");

#[test]
fn pptx_replace_text_from_xlsx_matches_go_oracle_saved_dry_run_and_errors() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-replace-text-xlsx-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("pptx replace text xlsx temp dir");

    let fixture = "testdata/pptx/title-content/presentation.pptx";
    let workbook = temp_dir.join("source-text.xlsx");
    write_simple_xlsx_with_sheet_xml(&workbook, pptx_replace_text_source_sheet_xml());
    let workbook_str = workbook.to_str().expect("source workbook path");

    for args in [
        vec!["--json", "pptx", "replace", "text-from-xlsx"],
        vec!["--json", "pptx", "replace", "text-map-from-xlsx"],
    ] {
        let (go_code, go_stdout, go_stderr) = run_go_ooxml(&args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&args);
        assert_eq!(rust_code, go_code, "missing file exit for {args:?}");
        assert_eq!(rust_stdout, go_stdout, "missing file stdout for {args:?}");
        assert_eq!(rust_stderr, go_stderr, "missing file stderr for {args:?}");
    }

    let go_out = temp_dir.join("go-text-from-xlsx.pptx");
    let rust_out = temp_dir.join("rust-text-from-xlsx.pptx");
    let go_out_str = go_out.to_str().expect("go text-from-xlsx output path");
    let rust_out_str = rust_out.to_str().expect("rust text-from-xlsx output path");
    let go_args = [
        "--json",
        "pptx",
        "replace",
        "text-from-xlsx",
        fixture,
        "--workbook",
        workbook_str,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:B2",
        "--slide",
        "1",
        "--target",
        "title",
        "--out",
        go_out_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "replace",
        "text-from-xlsx",
        fixture,
        "--workbook",
        workbook_str,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:B2",
        "--slide",
        "1",
        "--target",
        "title",
        "--out",
        rust_out_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "text-from-xlsx saved exit");
    assert_eq!(rust_stderr, go_stderr, "text-from-xlsx saved stderr");
    let rust_json = rust_stdout.expect("rust text-from-xlsx stdout");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_out_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go text-from-xlsx stdout"),
            go_out_str,
            "[OUT]"
        ),
        "text-from-xlsx saved stdout"
    );
    assert!(go_out.exists(), "Go text-from-xlsx output missing");
    assert!(rust_out.exists(), "Rust text-from-xlsx output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    let (go_read_code, go_read_stdout, go_read_stderr) = run_go_ooxml(&[
        "--json",
        "pptx",
        "shapes",
        "get",
        go_out_str,
        "--slide",
        "1",
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
        "1",
        "--target",
        "title",
        "--include-text",
    ]);
    assert_eq!(rust_read_code, go_read_code, "text-from readback exit");
    assert_eq!(
        rust_read_stderr, go_read_stderr,
        "text-from readback stderr"
    );
    assert_eq!(
        scrub_path(
            rust_read_stdout.expect("rust text-from readback"),
            rust_out_str,
            "[OUT]"
        ),
        scrub_path(
            go_read_stdout.expect("go text-from readback"),
            go_out_str,
            "[OUT]"
        ),
        "text-from readback stdout"
    );

    let dry_run_args = [
        "--json",
        "pptx",
        "replace",
        "text-from-xlsx",
        fixture,
        "--workbook",
        workbook_str,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:B2",
        "--slide",
        "2",
        "--target",
        "body",
        "--row-sep",
        "\\n",
        "--col-sep",
        " | ",
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, go_code, "text-from-xlsx dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "text-from-xlsx dry-run stderr");
    assert_eq!(
        rust_stdout.expect("rust text-from-xlsx dry-run stdout"),
        go_stdout.expect("go text-from-xlsx dry-run stdout"),
        "text-from-xlsx dry-run stdout"
    );

    let missing_target = [
        "--json",
        "pptx",
        "replace",
        "text-from-xlsx",
        fixture,
        "--workbook",
        workbook_str,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:B2",
        "--slide",
        "1",
        "--target",
        "missing",
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&missing_target);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&missing_target);
    assert_eq!(rust_code, go_code, "text-from missing target exit");
    assert_eq!(rust_stdout, go_stdout, "text-from missing target stdout");
    assert_eq!(rust_stderr, go_stderr, "text-from missing target stderr");
}

#[test]
fn pptx_replace_text_map_from_xlsx_matches_go_oracle_saved_dry_run_and_errors() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-replace-text-map-xlsx-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("pptx replace text map xlsx temp dir");

    let fixture = "testdata/pptx/title-content/presentation.pptx";
    let workbook = temp_dir.join("source-map.xlsx");
    write_simple_xlsx_with_sheet_xml(&workbook, pptx_replace_text_map_source_sheet_xml());
    let workbook_str = workbook.to_str().expect("source map workbook path");
    let table_workbook = temp_dir.join("source-map-table.xlsx");
    write_pptx_text_map_table_xlsx(&table_workbook);
    let table_workbook_str = table_workbook.to_str().expect("source table workbook path");

    let go_out = temp_dir.join("go-text-map-from-xlsx.pptx");
    let rust_out = temp_dir.join("rust-text-map-from-xlsx.pptx");
    let go_out_str = go_out.to_str().expect("go text-map output path");
    let rust_out_str = rust_out.to_str().expect("rust text-map output path");
    let go_args = [
        "--json",
        "pptx",
        "replace",
        "text-map-from-xlsx",
        fixture,
        "--workbook",
        workbook_str,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:C3",
        "--expect-source-range",
        "A1:C3",
        "--out",
        go_out_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "replace",
        "text-map-from-xlsx",
        fixture,
        "--workbook",
        workbook_str,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:C3",
        "--expect-source-range",
        "A1:C3",
        "--out",
        rust_out_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "text-map-from-xlsx saved exit");
    assert_eq!(rust_stderr, go_stderr, "text-map-from-xlsx saved stderr");
    let rust_json = rust_stdout.expect("rust text-map-from-xlsx stdout");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_out_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go text-map-from-xlsx stdout"),
            go_out_str,
            "[OUT]"
        ),
        "text-map-from-xlsx saved stdout"
    );
    assert!(go_out.exists(), "Go text-map-from-xlsx output missing");
    assert!(rust_out.exists(), "Rust text-map-from-xlsx output missing");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    let (go_extract_code, go_extract_stdout, go_extract_stderr) =
        run_go_ooxml(&["--json", "pptx", "extract", "text", go_out_str]);
    let (rust_extract_code, rust_extract_stdout, rust_extract_stderr) =
        run_ooxml(&["--json", "pptx", "extract", "text", rust_out_str]);
    assert_eq!(rust_extract_code, go_extract_code, "text-map extract exit");
    assert_eq!(
        rust_extract_stderr, go_extract_stderr,
        "text-map extract stderr"
    );
    assert_eq!(
        scrub_path(
            rust_extract_stdout.expect("rust text-map extract"),
            rust_out_str,
            "[OUT]"
        ),
        scrub_path(
            go_extract_stdout.expect("go text-map extract"),
            go_out_str,
            "[OUT]"
        ),
        "text-map extract stdout"
    );

    let dry_run_args = [
        "--json",
        "pptx",
        "replace",
        "text-map-from-xlsx",
        fixture,
        "--workbook",
        table_workbook_str,
        "--table",
        "TextMap",
        "--slide-col",
        "1",
        "--target-col",
        "2",
        "--text-col",
        "3",
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, go_code, "text-map-from-xlsx dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "text-map-from-xlsx dry-run stderr");
    assert_eq!(
        rust_stdout.expect("rust text-map dry-run stdout"),
        go_stdout.expect("go text-map dry-run stdout"),
        "text-map-from-xlsx dry-run stdout"
    );

    let error_cases: Vec<Vec<&str>> = vec![
        vec![
            "--json",
            "pptx",
            "replace",
            "text-map-from-xlsx",
            fixture,
            "--workbook",
            workbook_str,
            "--range",
            "A1:C3",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "replace",
            "text-map-from-xlsx",
            fixture,
            "--workbook",
            workbook_str,
            "--sheet",
            "Sheet1",
            "--range",
            "A1:C3",
            "--slide-col",
            "1",
            "--target-col",
            "1",
            "--text-col",
            "3",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "replace",
            "text-map-from-xlsx",
            fixture,
            "--workbook",
            workbook_str,
            "--sheet",
            "Sheet1",
            "--range",
            "A1:C3",
            "--target-col",
            "3",
            "--text-col",
            "2",
            "--dry-run",
        ],
    ];
    for args in error_cases {
        let (go_code, go_stdout, go_stderr) = run_go_ooxml(&args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&args);
        assert_eq!(rust_code, go_code, "text-map error exit for {args:?}");
        assert_eq!(rust_stdout, go_stdout, "text-map error stdout for {args:?}");
        assert_eq!(rust_stderr, go_stderr, "text-map error stderr for {args:?}");
    }
}

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
