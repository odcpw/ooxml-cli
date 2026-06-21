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
