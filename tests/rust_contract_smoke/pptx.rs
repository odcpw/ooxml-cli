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

include!("pptx/translate.rs");

include!("pptx/render.rs");

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

include!("pptx/place.rs");

include!("pptx/charts.rs");

include!("pptx/animations.rs");

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

include!("pptx/xlsx_bindings.rs");

include!("pptx/legacy_baseline.rs");

include!("pptx/media.rs");

include!("pptx/replace.rs");

include!("pptx/notes.rs");

include!("pptx/shapes.rs");

include!("pptx/layouts.rs");

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
