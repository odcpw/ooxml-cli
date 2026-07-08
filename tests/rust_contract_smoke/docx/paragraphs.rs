#[test]
fn docx_paragraphs_append_matches_rust_baseline() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-docx-paragraphs-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx paragraphs temp dir");
    let baseline_out = temp_dir.join("append-baseline.docx");
    let rust_out = temp_dir.join("append-rust.docx");
    let baseline_out = baseline_out.to_string_lossy().to_string();
    let rust_out = rust_out.to_string_lossy().to_string();

    let baseline_args = [
        "--json",
        "docx",
        "paragraphs",
        "append",
        "testdata/docx/styled-headings/document.docx",
        "--text",
        "Tail Heading",
        "--style",
        "Heading1",
        "--out",
        &baseline_out,
    ];
    let rust_args = [
        "--json",
        "docx",
        "paragraphs",
        "append",
        "testdata/docx/styled-headings/document.docx",
        "--text",
        "Tail Heading",
        "--style",
        "Heading1",
        "--out",
        &rust_out,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "append exit");
    assert_eq!(rust_stderr, baseline_stderr, "append stderr");
    assert_eq!(rust_stdout, baseline_stdout, "append stdout");
    assert!(Path::new(&rust_out).exists(), "Rust append output missing");

    let (validate_code, validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &rust_out]);
    assert_eq!(validate_code, 0, "validate exit");
    assert_eq!(validate_stderr, None, "validate stderr");
    assert!(validate_stdout.is_some(), "validate stdout");

    let (baseline_text_code, baseline_text_stdout, baseline_text_stderr) =
        run_ooxml_baseline(&["--json", "docx", "text", &baseline_out]);
    let (rust_text_code, rust_text_stdout, rust_text_stderr) =
        run_ooxml(&["--json", "docx", "text", &rust_out]);
    assert_eq!(rust_text_code, baseline_text_code, "append readback exit");
    assert_eq!(rust_text_stderr, baseline_text_stderr, "append readback stderr");
    let baseline_text_result = baseline_text_stdout.expect("Rust baseline append readback JSON");
    let rust_text_result = rust_text_stdout.expect("Rust append readback JSON");
    assert_eq!(
        rust_text_result["blocks"], baseline_text_result["blocks"],
        "append readback blocks"
    );
    assert_eq!(rust_text_result["file"], Value::String(rust_out.clone()));

    let blocks = rust_text_result["blocks"].as_array().expect("docx blocks");
    assert_eq!(blocks.len(), 3, "appended block count");
    assert_eq!(blocks[2]["text"], Value::String("Tail Heading".to_string()));
    assert_eq!(blocks[2]["style"], Value::String("Heading1".to_string()));
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn docx_paragraphs_append_dry_run_and_errors_match_rust_baseline() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-docx-paragraphs-dry-run-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx paragraphs temp dir");
    let dry_docx = temp_dir.join("dry-run.docx");
    fs::copy("testdata/docx/minimal/document.docx", &dry_docx).expect("copy dry-run docx");
    let dry_docx = dry_docx.to_string_lossy().to_string();

    assert_rust_baseline_match(&[
        "--json",
        "docx",
        "paragraphs",
        "append",
        &dry_docx,
        "--text",
        "Dry run tail",
        "--dry-run",
    ]);
    let (text_code, text_stdout, text_stderr) = run_ooxml(&["--json", "docx", "text", &dry_docx]);
    assert_eq!(text_code, 0);
    assert_eq!(text_stderr, None);
    let text_result = text_stdout.expect("dry-run readback");
    let blocks = text_result["blocks"].as_array().expect("docx blocks");
    assert_eq!(blocks.len(), 1, "dry-run wrote to document");
    assert_eq!(blocks[0]["text"], Value::String("Hello world".to_string()));

    let text_file = temp_dir.join("text.txt");
    fs::write(&text_file, "x").expect("write text file");
    let text_file = text_file.to_string_lossy().to_string();
    let out = temp_dir.join("bad.docx").to_string_lossy().to_string();
    let missing = temp_dir.join("missing.docx").to_string_lossy().to_string();
    let bad_cases: Vec<Vec<&str>> = vec![
        vec![
            "--json",
            "docx",
            "paragraphs",
            "append",
            &missing,
            "--text",
            "x",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "paragraphs",
            "append",
            "testdata/docx/minimal/document.docx",
            "--text",
            "x",
        ],
        vec![
            "--json",
            "docx",
            "paragraphs",
            "append",
            "testdata/docx/minimal/document.docx",
            "--text",
            "x",
            "--dry-run",
            "--out",
            &out,
        ],
        vec![
            "--json",
            "docx",
            "paragraphs",
            "append",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
            "--text",
            "x",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "paragraphs",
            "append",
            "testdata/docx/minimal/document.docx",
            "--text",
            "x",
            "--text-file",
            &text_file,
            "--dry-run",
        ],
    ];
    for args in bad_cases {
        assert_rust_baseline_match(&args);
    }
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn docx_paragraphs_insert_matches_rust_baseline() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-docx-insert-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx insert temp dir");

    let text_file = temp_dir.join("insert.txt");
    fs::write(&text_file, "Lead\tparagraph\nline 2").expect("write insert text file");
    let text_file = text_file.to_string_lossy().to_string();
    let baseline_out = temp_dir
        .join("insert-baseline.docx")
        .to_string_lossy()
        .to_string();
    let rust_out = temp_dir
        .join("insert-rust.docx")
        .to_string_lossy()
        .to_string();
    let baseline_args = [
        "--json",
        "docx",
        "paragraphs",
        "insert",
        "testdata/docx/styled-headings/document.docx",
        "--insert-after",
        "0",
        "--text-file",
        &text_file,
        "--out",
        &baseline_out,
    ];
    let rust_args = [
        "--json",
        "docx",
        "paragraphs",
        "insert",
        "testdata/docx/styled-headings/document.docx",
        "--insert-after",
        "0",
        "--text-file",
        &text_file,
        "--out",
        &rust_out,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "insert start exit");
    assert_eq!(rust_stderr, baseline_stderr, "insert start stderr");
    assert_eq!(rust_stdout, baseline_stdout, "insert start stdout");

    let (validate_code, _validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &rust_out]);
    assert_eq!(validate_code, 0, "insert start validate exit");
    assert_eq!(validate_stderr, None, "insert start validate stderr");

    let (baseline_text_code, baseline_text_stdout, baseline_text_stderr) =
        run_ooxml_baseline(&["--json", "docx", "text", &baseline_out]);
    let (rust_text_code, rust_text_stdout, rust_text_stderr) =
        run_ooxml(&["--json", "docx", "text", &rust_out]);
    assert_eq!(rust_text_code, baseline_text_code, "insert start readback exit");
    assert_eq!(
        rust_text_stderr, baseline_text_stderr,
        "insert start readback stderr"
    );
    let baseline_text_result = baseline_text_stdout.expect("Rust baseline insert start readback JSON");
    let rust_text_result = rust_text_stdout.expect("Rust insert start readback JSON");
    assert_eq!(
        rust_text_result["blocks"], baseline_text_result["blocks"],
        "insert start readback blocks"
    );
    let blocks = rust_text_result["blocks"].as_array().expect("docx blocks");
    assert_eq!(blocks.len(), 3, "insert start block count");
    assert_eq!(
        blocks[0]["text"],
        Value::String("Lead\tparagraph\nline 2".to_string())
    );
    assert_eq!(blocks[1]["text"], Value::String("Heading Text".to_string()));

    let baseline_table_out = temp_dir
        .join("insert-after-table-baseline.docx")
        .to_string_lossy()
        .to_string();
    let rust_table_out = temp_dir
        .join("insert-after-table-rust.docx")
        .to_string_lossy()
        .to_string();
    let baseline_table_args = [
        "--json",
        "docx",
        "paragraphs",
        "insert",
        "testdata/docx/mixed-blocks/document.docx",
        "--insert-after",
        "1",
        "--text",
        "After table",
        "--out",
        &baseline_table_out,
    ];
    let rust_table_args = [
        "--json",
        "docx",
        "paragraphs",
        "insert",
        "testdata/docx/mixed-blocks/document.docx",
        "--insert-after",
        "1",
        "--text",
        "After table",
        "--out",
        &rust_table_out,
    ];
    let (baseline_table_code, baseline_table_stdout, baseline_table_stderr) = run_ooxml_baseline(&baseline_table_args);
    let (rust_table_code, rust_table_stdout, rust_table_stderr) = run_ooxml(&rust_table_args);
    assert_eq!(rust_table_code, baseline_table_code, "insert table exit");
    assert_eq!(rust_table_stderr, baseline_table_stderr, "insert table stderr");
    assert_eq!(rust_table_stdout, baseline_table_stdout, "insert table stdout");
    let (baseline_table_text_code, baseline_table_text_stdout, baseline_table_text_stderr) =
        run_ooxml_baseline(&["--json", "docx", "text", &baseline_table_out]);
    let (rust_table_text_code, rust_table_text_stdout, rust_table_text_stderr) =
        run_ooxml(&["--json", "docx", "text", &rust_table_out]);
    assert_eq!(
        rust_table_text_code, baseline_table_text_code,
        "insert table readback exit"
    );
    assert_eq!(
        rust_table_text_stderr, baseline_table_text_stderr,
        "insert table readback stderr"
    );
    let baseline_table_text = baseline_table_text_stdout.expect("Rust baseline insert table readback JSON");
    let rust_table_text = rust_table_text_stdout.expect("Rust insert table readback JSON");
    assert_eq!(
        rust_table_text["blocks"], baseline_table_text["blocks"],
        "insert table readback blocks"
    );
    let table_blocks = rust_table_text["blocks"].as_array().expect("docx blocks");
    assert_eq!(table_blocks.len(), 5, "insert table block count");
    assert_eq!(table_blocks[0]["kind"], Value::String("table".to_string()));
    assert_eq!(
        table_blocks[1]["text"],
        Value::String("After table".to_string())
    );
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn docx_paragraphs_insert_dry_run_and_errors_match_rust_baseline() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-docx-insert-dry-run-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx insert temp dir");
    let dry_docx = temp_dir.join("dry-run.docx");
    fs::copy("testdata/docx/minimal/document.docx", &dry_docx).expect("copy dry-run docx");
    let dry_docx = dry_docx.to_string_lossy().to_string();

    assert_rust_baseline_match(&[
        "--json",
        "docx",
        "paragraphs",
        "insert",
        &dry_docx,
        "--insert-after",
        "0",
        "--text",
        "Dry run head",
        "--dry-run",
    ]);
    let (text_code, text_stdout, text_stderr) = run_ooxml(&["--json", "docx", "text", &dry_docx]);
    assert_eq!(text_code, 0);
    assert_eq!(text_stderr, None);
    let text_result = text_stdout.expect("dry-run readback");
    let blocks = text_result["blocks"].as_array().expect("docx blocks");
    assert_eq!(blocks.len(), 1, "insert dry-run wrote to document");
    assert_eq!(blocks[0]["text"], Value::String("Hello world".to_string()));

    let text_file = temp_dir.join("text.txt");
    fs::write(&text_file, "x").expect("write text file");
    let text_file = text_file.to_string_lossy().to_string();
    let out = temp_dir.join("bad.docx").to_string_lossy().to_string();
    let missing = temp_dir.join("missing.docx").to_string_lossy().to_string();
    let bad_cases: Vec<Vec<&str>> = vec![
        vec![
            "--json",
            "docx",
            "paragraphs",
            "insert",
            &missing,
            "--insert-after",
            "-1",
            "--text",
            "x",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "paragraphs",
            "insert",
            "testdata/docx/styled-headings/document.docx",
            "--insert-after",
            "-1",
            "--text",
            "x",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "paragraphs",
            "insert",
            "testdata/docx/styled-headings/document.docx",
            "--insert-after",
            "99",
            "--text",
            "x",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "paragraphs",
            "insert",
            "testdata/docx/styled-headings/document.docx",
            "--insert-after",
            "1",
            "--text",
            "x",
            "--dry-run",
            "--out",
            &out,
        ],
        vec![
            "--json",
            "docx",
            "paragraphs",
            "insert",
            "testdata/docx/styled-headings/document.docx",
            "--insert-after",
            "1",
            "--text",
            "x",
            "--text-file",
            &text_file,
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "paragraphs",
            "insert",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
            "--insert-after",
            "0",
            "--text",
            "x",
            "--dry-run",
        ],
    ];
    for args in bad_cases {
        assert_rust_baseline_match(&args);
    }
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn docx_paragraphs_set_clear_and_handles_match_rust_baseline() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-docx-set-clear-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx set/clear temp dir");

    let baseline_set_out = temp_dir.join("set-baseline.docx").to_string_lossy().to_string();
    let rust_set_out = temp_dir.join("set-rust.docx").to_string_lossy().to_string();
    let baseline_set_args = [
        "--json",
        "docx",
        "paragraphs",
        "set",
        "testdata/docx/styled-headings/document.docx",
        "--index",
        "1",
        "--text",
        "Updated Heading",
        "--out",
        &baseline_set_out,
    ];
    let rust_set_args = [
        "--json",
        "docx",
        "paragraphs",
        "set",
        "testdata/docx/styled-headings/document.docx",
        "--index",
        "1",
        "--text",
        "Updated Heading",
        "--out",
        &rust_set_out,
    ];
    let (baseline_set_code, baseline_set_stdout, baseline_set_stderr) = run_ooxml_baseline(&baseline_set_args);
    let (rust_set_code, rust_set_stdout, rust_set_stderr) = run_ooxml(&rust_set_args);
    assert_eq!(rust_set_code, baseline_set_code, "set exit");
    assert_eq!(rust_set_stderr, baseline_set_stderr, "set stderr");
    assert_eq!(
        scrub_docx_dynamic_handles(rust_set_stdout.expect("Rust set stdout")),
        scrub_docx_dynamic_handles(baseline_set_stdout.expect("Rust baseline set stdout")),
        "set stdout"
    );
    let (validate_code, _validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &rust_set_out]);
    assert_eq!(validate_code, 0, "set validate exit");
    assert_eq!(validate_stderr, None, "set validate stderr");

    let (baseline_text_code, baseline_text_stdout, baseline_text_stderr) =
        run_ooxml_baseline(&["--json", "docx", "text", &baseline_set_out]);
    let (rust_text_code, rust_text_stdout, rust_text_stderr) =
        run_ooxml(&["--json", "docx", "text", &rust_set_out]);
    assert_eq!(rust_text_code, baseline_text_code, "set readback exit");
    assert_eq!(rust_text_stderr, baseline_text_stderr, "set readback stderr");
    let baseline_text = baseline_text_stdout.expect("Rust baseline set readback");
    let rust_text = rust_text_stdout.expect("Rust set readback");
    assert_eq!(
        scrub_docx_dynamic_handles(rust_text["blocks"].clone()),
        scrub_docx_dynamic_handles(baseline_text["blocks"].clone()),
        "set readback blocks"
    );
    let set_blocks = rust_text["blocks"].as_array().expect("docx blocks");
    assert_eq!(
        set_blocks[0]["text"],
        Value::String("Updated Heading".to_string())
    );
    assert_eq!(
        set_blocks[0]["style"],
        Value::String("Heading1".to_string())
    );
    assert_eq!(
        set_blocks[1]["text"],
        Value::String("Body text".to_string())
    );

    let baseline_run_out = temp_dir
        .join("set-run-props-baseline.docx")
        .to_string_lossy()
        .to_string();
    let rust_run_out = temp_dir
        .join("set-run-props-rust.docx")
        .to_string_lossy()
        .to_string();
    let baseline_run_args = [
        "--json",
        "docx",
        "paragraphs",
        "set",
        "testdata/docx/mixed-blocks/document.docx",
        "--index",
        "2",
        "--text",
        "Updated bold heading",
        "--out",
        &baseline_run_out,
    ];
    let rust_run_args = [
        "--json",
        "docx",
        "paragraphs",
        "set",
        "testdata/docx/mixed-blocks/document.docx",
        "--index",
        "2",
        "--text",
        "Updated bold heading",
        "--out",
        &rust_run_out,
    ];
    let (baseline_run_code, baseline_run_stdout, baseline_run_stderr) = run_ooxml_baseline(&baseline_run_args);
    let (rust_run_code, rust_run_stdout, rust_run_stderr) = run_ooxml(&rust_run_args);
    assert_eq!(rust_run_code, baseline_run_code, "run-props set exit");
    assert_eq!(rust_run_stderr, baseline_run_stderr, "run-props set stderr");
    assert_eq!(
        scrub_docx_dynamic_handles(rust_run_stdout.expect("Rust run-props set stdout")),
        scrub_docx_dynamic_handles(baseline_run_stdout.expect("Rust baseline run-props set stdout")),
        "run-props set stdout"
    );
    let (baseline_runs_code, baseline_runs_stdout, baseline_runs_stderr) = run_ooxml_baseline(&[
        "--json",
        "docx",
        "blocks",
        &baseline_run_out,
        "--block",
        "2",
        "--include-runs",
    ]);
    let (rust_runs_code, rust_runs_stdout, rust_runs_stderr) = run_ooxml(&[
        "--json",
        "docx",
        "blocks",
        &rust_run_out,
        "--block",
        "2",
        "--include-runs",
    ]);
    assert_eq!(rust_runs_code, baseline_runs_code, "run-props readback exit");
    assert_eq!(
        rust_runs_stderr, baseline_runs_stderr,
        "run-props readback stderr"
    );
    let baseline_runs = baseline_runs_stdout.expect("Rust baseline run-props readback");
    let rust_runs = rust_runs_stdout.expect("Rust run-props readback");
    assert_eq!(
        scrub_docx_dynamic_handles(rust_runs["blocks"].clone()),
        scrub_docx_dynamic_handles(baseline_runs["blocks"].clone()),
        "run-props readback blocks"
    );
    let run_block = &rust_runs["blocks"].as_array().expect("docx blocks")[0];
    assert_eq!(
        run_block["text"],
        Value::String("Updated bold heading".to_string())
    );
    assert_eq!(run_block["paragraph"]["runs"][0]["bold"], Value::Bool(true));

    let text_file = temp_dir.join("replacement.txt");
    fs::write(&text_file, "line 1\tcol 2\nline 2").expect("write text file");
    let text_file = text_file.to_string_lossy().to_string();
    let baseline_file_out = temp_dir
        .join("set-file-baseline.docx")
        .to_string_lossy()
        .to_string();
    let rust_file_out = temp_dir
        .join("set-file-rust.docx")
        .to_string_lossy()
        .to_string();
    let baseline_file_args = [
        "--json",
        "docx",
        "paragraphs",
        "set",
        "testdata/docx/minimal/document.docx",
        "--index",
        "1",
        "--text-file",
        &text_file,
        "--out",
        &baseline_file_out,
    ];
    let rust_file_args = [
        "--json",
        "docx",
        "paragraphs",
        "set",
        "testdata/docx/minimal/document.docx",
        "--index",
        "1",
        "--text-file",
        &text_file,
        "--out",
        &rust_file_out,
    ];
    let (baseline_file_code, baseline_file_stdout, baseline_file_stderr) = run_ooxml_baseline(&baseline_file_args);
    let (rust_file_code, rust_file_stdout, rust_file_stderr) = run_ooxml(&rust_file_args);
    assert_eq!(rust_file_code, baseline_file_code, "set file exit");
    assert_eq!(rust_file_stderr, baseline_file_stderr, "set file stderr");
    assert_eq!(
        scrub_docx_dynamic_handles(rust_file_stdout.expect("Rust set file stdout")),
        scrub_docx_dynamic_handles(baseline_file_stdout.expect("Rust baseline set file stdout")),
        "set file stdout"
    );
    let (file_text_code, file_text_stdout, file_text_stderr) =
        run_ooxml(&["--json", "docx", "text", &rust_file_out]);
    assert_eq!(file_text_code, 0);
    assert_eq!(file_text_stderr, None);
    let file_blocks = file_text_stdout
        .expect("set file readback")
        .get("blocks")
        .and_then(Value::as_array)
        .cloned()
        .expect("docx blocks");
    assert_eq!(
        file_blocks[0]["text"],
        Value::String("line 1\tcol 2\nline 2".to_string())
    );

    let baseline_clear_out = temp_dir.join("clear-baseline.docx").to_string_lossy().to_string();
    let rust_clear_out = temp_dir
        .join("clear-rust.docx")
        .to_string_lossy()
        .to_string();
    let baseline_clear_args = [
        "--json",
        "docx",
        "paragraphs",
        "clear",
        "testdata/docx/styled-headings/document.docx",
        "--index",
        "1",
        "--out",
        &baseline_clear_out,
    ];
    let rust_clear_args = [
        "--json",
        "docx",
        "paragraphs",
        "clear",
        "testdata/docx/styled-headings/document.docx",
        "--index",
        "1",
        "--out",
        &rust_clear_out,
    ];
    let (baseline_clear_code, baseline_clear_stdout, baseline_clear_stderr) = run_ooxml_baseline(&baseline_clear_args);
    let (rust_clear_code, rust_clear_stdout, rust_clear_stderr) = run_ooxml(&rust_clear_args);
    assert_eq!(rust_clear_code, baseline_clear_code, "clear exit");
    assert_eq!(rust_clear_stderr, baseline_clear_stderr, "clear stderr");
    assert_eq!(
        scrub_docx_dynamic_handles(rust_clear_stdout.expect("Rust clear stdout")),
        scrub_docx_dynamic_handles(baseline_clear_stdout.expect("Rust baseline clear stdout")),
        "clear stdout"
    );
    let (baseline_clear_text_code, baseline_clear_text_stdout, baseline_clear_text_stderr) =
        run_ooxml_baseline(&["--json", "docx", "text", &baseline_clear_out]);
    let (rust_clear_text_code, rust_clear_text_stdout, rust_clear_text_stderr) =
        run_ooxml(&["--json", "docx", "text", &rust_clear_out]);
    assert_eq!(
        rust_clear_text_code, baseline_clear_text_code,
        "clear readback exit"
    );
    assert_eq!(
        rust_clear_text_stderr, baseline_clear_text_stderr,
        "clear readback stderr"
    );
    let baseline_clear_text = baseline_clear_text_stdout.expect("Rust baseline clear readback");
    let rust_clear_text = rust_clear_text_stdout.expect("Rust clear readback");
    assert_eq!(
        scrub_docx_dynamic_handles(rust_clear_text["blocks"].clone()),
        scrub_docx_dynamic_handles(baseline_clear_text["blocks"].clone()),
        "clear readback blocks"
    );
    let clear_blocks = rust_clear_text["blocks"].as_array().expect("docx blocks");
    assert_eq!(clear_blocks[0]["text"], Value::String(String::new()));
    assert_eq!(
        clear_blocks[0]["style"],
        Value::String("Heading1".to_string())
    );

    let baseline_stamped = temp_dir
        .join("handle-stamped-baseline.docx")
        .to_string_lossy()
        .to_string();
    let rust_stamped = temp_dir
        .join("handle-stamped-rust.docx")
        .to_string_lossy()
        .to_string();
    let baseline_stamp_args = [
        "--json",
        "docx",
        "paragraphs",
        "set",
        "testdata/docx/minimal/document.docx",
        "--index",
        "1",
        "--text",
        "Target",
        "--out",
        &baseline_stamped,
    ];
    let rust_stamp_args = [
        "--json",
        "docx",
        "paragraphs",
        "set",
        "testdata/docx/minimal/document.docx",
        "--index",
        "1",
        "--text",
        "Target",
        "--out",
        &rust_stamped,
    ];
    let (_, baseline_stamp_stdout, _) = run_ooxml_baseline(&baseline_stamp_args);
    let (_, rust_stamp_stdout, _) = run_ooxml(&rust_stamp_args);
    let baseline_handle = baseline_stamp_stdout
        .expect("Rust baseline handle stamp")
        .get("handle")
        .and_then(Value::as_str)
        .expect("Rust baseline paragraph handle")
        .to_string();
    let rust_handle = rust_stamp_stdout
        .expect("Rust handle stamp")
        .get("handle")
        .and_then(Value::as_str)
        .expect("Rust paragraph handle")
        .to_string();

    let baseline_prepended = temp_dir
        .join("handle-prepended-baseline.docx")
        .to_string_lossy()
        .to_string();
    let rust_prepended = temp_dir
        .join("handle-prepended-rust.docx")
        .to_string_lossy()
        .to_string();
    let baseline_prepend_args = [
        "--json",
        "docx",
        "paragraphs",
        "insert",
        &baseline_stamped,
        "--insert-after",
        "0",
        "--text",
        "New top",
        "--out",
        &baseline_prepended,
    ];
    let rust_prepend_args = [
        "--json",
        "docx",
        "paragraphs",
        "insert",
        &rust_stamped,
        "--insert-after",
        "0",
        "--text",
        "New top",
        "--out",
        &rust_prepended,
    ];
    let (baseline_prepend_code, baseline_prepend_stdout, baseline_prepend_stderr) = run_ooxml_baseline(&baseline_prepend_args);
    let (rust_prepend_code, rust_prepend_stdout, rust_prepend_stderr) =
        run_ooxml(&rust_prepend_args);
    assert_eq!(rust_prepend_code, baseline_prepend_code, "prepend exit");
    assert_eq!(rust_prepend_stderr, baseline_prepend_stderr, "prepend stderr");
    assert_eq!(
        scrub_file_fields(rust_prepend_stdout.expect("Rust prepend stdout")),
        scrub_file_fields(baseline_prepend_stdout.expect("Rust baseline prepend stdout")),
        "prepend stdout"
    );

    let baseline_resolved = temp_dir
        .join("handle-resolved-baseline.docx")
        .to_string_lossy()
        .to_string();
    let rust_resolved = temp_dir
        .join("handle-resolved-rust.docx")
        .to_string_lossy()
        .to_string();
    let baseline_resolve_args = [
        "--json",
        "docx",
        "paragraphs",
        "set",
        &baseline_prepended,
        "--handle",
        &baseline_handle,
        "--text",
        "Same paragraph",
        "--out",
        &baseline_resolved,
    ];
    let rust_resolve_args = [
        "--json",
        "docx",
        "paragraphs",
        "set",
        &rust_prepended,
        "--handle",
        &rust_handle,
        "--text",
        "Same paragraph",
        "--out",
        &rust_resolved,
    ];
    let (baseline_resolve_code, baseline_resolve_stdout, baseline_resolve_stderr) = run_ooxml_baseline(&baseline_resolve_args);
    let (rust_resolve_code, rust_resolve_stdout, rust_resolve_stderr) =
        run_ooxml(&rust_resolve_args);
    assert_eq!(rust_resolve_code, baseline_resolve_code, "handle resolve exit");
    assert_eq!(
        rust_resolve_stderr, baseline_resolve_stderr,
        "handle resolve stderr"
    );
    let rust_resolve_result = rust_resolve_stdout.expect("Rust handle resolve stdout");
    let baseline_resolve_result = baseline_resolve_stdout.expect("Rust baseline handle resolve stdout");
    assert_eq!(
        scrub_file_fields(scrub_docx_dynamic_handles(rust_resolve_result.clone())),
        scrub_file_fields(scrub_docx_dynamic_handles(baseline_resolve_result)),
        "handle resolve stdout"
    );
    assert_eq!(rust_resolve_result["index"], Value::from(2));
    assert_eq!(
        rust_resolve_result["previousText"],
        Value::String("Target".to_string())
    );
    let (resolved_text_code, resolved_text_stdout, resolved_text_stderr) =
        run_ooxml(&["--json", "docx", "text", &rust_resolved]);
    assert_eq!(resolved_text_code, 0);
    assert_eq!(resolved_text_stderr, None);
    let resolved_blocks = resolved_text_stdout
        .expect("handle resolved readback")
        .get("blocks")
        .and_then(Value::as_array)
        .cloned()
        .expect("docx blocks");
    assert_eq!(
        resolved_blocks[0]["text"],
        Value::String("New top".to_string())
    );
    assert_eq!(
        resolved_blocks[1]["text"],
        Value::String("Same paragraph".to_string())
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn docx_paragraphs_set_clear_errors_match_rust_baseline() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-docx-set-clear-errors-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx set/clear errors temp dir");
    let text_file = temp_dir.join("text.txt");
    fs::write(&text_file, "x").expect("write text file");
    let text_file = text_file.to_string_lossy().to_string();
    let empty_text_file = temp_dir.join("empty.txt");
    fs::write(&empty_text_file, "").expect("write empty text file");
    let empty_text_file = empty_text_file.to_string_lossy().to_string();
    let missing_text_file = temp_dir.join("missing.txt").to_string_lossy().to_string();
    let missing = temp_dir.join("missing.docx").to_string_lossy().to_string();
    let bad_cases: Vec<Vec<&str>> = vec![
        vec![
            "--json",
            "docx",
            "paragraphs",
            "set",
            &missing,
            "--index",
            "1",
            "--text",
            "x",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "paragraphs",
            "set",
            "testdata/docx/minimal/document.docx",
            "--text",
            "x",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "paragraphs",
            "set",
            "testdata/docx/minimal/document.docx",
            "--index",
            "1",
            "--handle",
            "H:docx/pt:doc/para:m:ABCDEF01",
            "--text",
            "x",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "paragraphs",
            "set",
            "testdata/docx/minimal/document.docx",
            "--index",
            "1",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "paragraphs",
            "set",
            "testdata/docx/minimal/document.docx",
            "--index",
            "1",
            "--text",
            "x",
            "--text-file",
            &text_file,
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "paragraphs",
            "set",
            "testdata/docx/minimal/document.docx",
            "--index",
            "1",
            "--text",
            "",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "paragraphs",
            "set",
            "testdata/docx/minimal/document.docx",
            "--index",
            "1",
            "--text-file",
            &empty_text_file,
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "paragraphs",
            "set",
            "testdata/docx/minimal/document.docx",
            "--index",
            "1",
            "--text-file",
            &missing_text_file,
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "paragraphs",
            "set",
            "testdata/docx/mixed-blocks/document.docx",
            "--index",
            "1",
            "--text",
            "x",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "paragraphs",
            "set",
            "testdata/docx/styled-headings/document.docx",
            "--index",
            "99",
            "--text",
            "x",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "paragraphs",
            "set",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
            "--index",
            "1",
            "--text",
            "x",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "paragraphs",
            "set",
            "testdata/docx/minimal/document.docx",
            "--handle",
            "H:pptx/s:256/shape:n:2",
            "--text",
            "x",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "paragraphs",
            "set",
            "testdata/docx/minimal/document.docx",
            "--handle",
            "H:docx/pt:doc/para:m:DOESNOTEXIST",
            "--text",
            "x",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "paragraphs",
            "set",
            "testdata/docx/paraid-dup/document.docx",
            "--handle",
            "H:docx/pt:doc/para:m:DEAD00FF",
            "--text",
            "x",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "paragraphs",
            "clear",
            "testdata/docx/minimal/document.docx",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "paragraphs",
            "clear",
            "testdata/docx/minimal/document.docx",
            "--index",
            "1",
            "--handle",
            "H:docx/pt:doc/para:m:ABCDEF01",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "paragraphs",
            "clear",
            "testdata/docx/mixed-blocks/document.docx",
            "--index",
            "1",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "paragraphs",
            "clear",
            "testdata/docx/styled-headings/document.docx",
            "--index",
            "99",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "paragraphs",
            "clear",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
            "--index",
            "1",
            "--dry-run",
        ],
    ];
    for args in bad_cases {
        assert_rust_baseline_match(&args);
    }
    let _ = fs::remove_dir_all(&temp_dir);
}