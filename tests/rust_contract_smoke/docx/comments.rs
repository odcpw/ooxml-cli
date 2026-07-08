#[test]
fn docx_comments_list_matches_rust_baseline() {
    let cases: Vec<Vec<&str>> = vec![
        vec![
            "--json",
            "docx",
            "comments",
            "list",
            "testdata/docx/with-comments/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "comments",
            "list",
            "testdata/docx/with-comments/document.docx",
            "--comment-id",
            "0",
        ],
        vec![
            "--json",
            "docx",
            "comments",
            "list",
            "testdata/docx/minimal/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "comments",
            "list",
            "testdata/docx/with-comments/document.docx",
            "--comment-id",
            "99",
        ],
    ];

    for args in cases {
        assert_rust_baseline_match(&args);
    }
}

#[test]
fn docx_comments_add_matches_rust_baseline() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-docx-comments-add-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx comments temp dir");
    let baseline_out = temp_dir.join("comments-baseline.docx");
    let rust_out = temp_dir.join("comments-rust.docx");
    let baseline_out = baseline_out.to_string_lossy().to_string();
    let rust_out = rust_out.to_string_lossy().to_string();

    let baseline_args = [
        "--json",
        "docx",
        "comments",
        "add",
        "testdata/docx/minimal/document.docx",
        "--anchor-block",
        "1",
        "--author",
        "Bob",
        "--initials",
        "BB",
        "--text",
        "Brand new",
        "--date",
        "2025-06-06T10:30:00Z",
        "--out",
        &baseline_out,
    ];
    let rust_args = [
        "--json",
        "docx",
        "comments",
        "add",
        "testdata/docx/minimal/document.docx",
        "--anchor-block",
        "1",
        "--author",
        "Bob",
        "--initials",
        "BB",
        "--text",
        "Brand new",
        "--date",
        "2025-06-06T10:30:00Z",
        "--out",
        &rust_out,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "comments add exit");
    assert_eq!(rust_stderr, baseline_stderr, "comments add stderr");
    assert_eq!(rust_stdout, baseline_stdout, "comments add stdout");
    assert!(
        Path::new(&rust_out).exists(),
        "Rust comments output missing"
    );

    let (validate_code, validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &rust_out]);
    assert_eq!(validate_code, 0, "validate exit");
    assert_eq!(validate_stderr, None, "validate stderr");
    assert!(validate_stdout.is_some(), "validate stdout");

    let (baseline_list_code, baseline_list_stdout, baseline_list_stderr) =
        run_ooxml_baseline(&["--json", "docx", "comments", "list", &baseline_out]);
    let (rust_list_code, rust_list_stdout, rust_list_stderr) =
        run_ooxml(&["--json", "docx", "comments", "list", &rust_out]);
    assert_eq!(rust_list_code, baseline_list_code, "comments list readback exit");
    assert_eq!(
        rust_list_stderr, baseline_list_stderr,
        "comments list readback stderr"
    );
    let baseline_list = baseline_list_stdout.expect("Rust baseline comments list JSON");
    let rust_list = rust_list_stdout.expect("Rust comments list JSON");
    assert_eq!(
        rust_list["comments"], baseline_list["comments"],
        "comments list readback"
    );
    assert_eq!(
        rust_list["comments"][0]["text"],
        Value::String("Brand new".to_string())
    );
    assert_eq!(
        rust_list["comments"][0]["author"],
        Value::String("Bob".to_string())
    );

    let dry_run = [
        "--json",
        "docx",
        "comments",
        "add",
        "testdata/docx/minimal/document.docx",
        "--author",
        "Bob",
        "--text",
        "Dry run",
        "--date",
        "2025-06-06T10:30:00Z",
        "--dry-run",
    ];
    assert_rust_baseline_match(&dry_run);

    let missing_author = [
        "--json",
        "docx",
        "comments",
        "add",
        "testdata/docx/minimal/document.docx",
        "--text",
        "No author",
        "--dry-run",
    ];
    assert_rust_baseline_match(&missing_author);

    let unsupported_type = [
        "--json",
        "docx",
        "comments",
        "add",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        "--author",
        "Bob",
        "--text",
        "Wrong package",
        "--date",
        "2025-06-06T10:30:00Z",
        "--dry-run",
    ];
    assert_rust_baseline_match(&unsupported_type);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn docx_comments_edit_matches_rust_baseline() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-docx-comments-edit-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx comments temp dir");
    let baseline_out = temp_dir.join("comments-edit-baseline.docx");
    let rust_out = temp_dir.join("comments-edit-rust.docx");
    let baseline_out = baseline_out.to_string_lossy().to_string();
    let rust_out = rust_out.to_string_lossy().to_string();

    let (hash_code, hash_stdout, hash_stderr) = run_ooxml_baseline(&[
        "--json",
        "docx",
        "comments",
        "list",
        "testdata/docx/with-comments/document.docx",
        "--comment-id",
        "0",
    ]);
    assert_eq!(hash_code, 0, "hash list exit");
    assert_eq!(hash_stderr, None, "hash list stderr");
    let hash_json = hash_stdout.expect("hash list JSON");
    let hash = hash_json["comments"][0]["contentHash"]
        .as_str()
        .expect("comment content hash");

    let baseline_args = [
        "--json",
        "docx",
        "comments",
        "edit",
        "testdata/docx/with-comments/document.docx",
        "--comment-id",
        "0",
        "--text",
        "Updated comment",
        "--author",
        "Carol",
        "--date",
        "2030-01-02T03:04:05Z",
        "--expect-hash",
        hash,
        "--out",
        &baseline_out,
    ];
    let rust_args = [
        "--json",
        "docx",
        "comments",
        "edit",
        "testdata/docx/with-comments/document.docx",
        "--comment-id",
        "0",
        "--text",
        "Updated comment",
        "--author",
        "Carol",
        "--date",
        "2030-01-02T03:04:05Z",
        "--expect-hash",
        hash,
        "--out",
        &rust_out,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "comments edit exit");
    assert_eq!(rust_stderr, baseline_stderr, "comments edit stderr");
    assert_eq!(rust_stdout, baseline_stdout, "comments edit stdout");
    assert!(Path::new(&rust_out).exists(), "Rust edit output missing");

    let (validate_code, validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &rust_out]);
    assert_eq!(validate_code, 0, "validate exit");
    assert_eq!(validate_stderr, None, "validate stderr");
    assert!(validate_stdout.is_some(), "validate stdout");

    let (baseline_list_code, baseline_list_stdout, baseline_list_stderr) = run_ooxml_baseline(&[
        "--json",
        "docx",
        "comments",
        "list",
        &baseline_out,
        "--comment-id",
        "0",
    ]);
    let (rust_list_code, rust_list_stdout, rust_list_stderr) = run_ooxml(&[
        "--json",
        "docx",
        "comments",
        "list",
        &rust_out,
        "--comment-id",
        "0",
    ]);
    assert_eq!(rust_list_code, baseline_list_code, "comments edit readback exit");
    assert_eq!(
        rust_list_stderr, baseline_list_stderr,
        "comments edit readback stderr"
    );
    let baseline_list = baseline_list_stdout.expect("Rust baseline comments edit readback JSON");
    let rust_list = rust_list_stdout.expect("Rust comments edit readback JSON");
    assert_eq!(
        rust_list["comments"], baseline_list["comments"],
        "comments edit readback"
    );

    let wrong_hash = [
        "--json",
        "docx",
        "comments",
        "edit",
        "testdata/docx/with-comments/document.docx",
        "--comment-id",
        "0",
        "--text",
        "x",
        "--expect-hash",
        "sha256:bogus",
        "--dry-run",
    ];
    assert_rust_baseline_match(&wrong_hash);

    let by_handle = [
        "--json",
        "docx",
        "comments",
        "edit",
        "testdata/docx/with-comments/document.docx",
        "--handle",
        "H:docx/pt:doc/comment:n:0",
        "--text",
        "Edited by handle",
        "--date",
        "2031-02-03T04:05:06Z",
        "--dry-run",
    ];
    assert_rust_baseline_match(&by_handle);

    let stale_handle = [
        "--json",
        "docx",
        "comments",
        "edit",
        "testdata/docx/with-comments/document.docx",
        "--handle",
        "H:docx/pt:doc/comment:n:9999",
        "--text",
        "x",
        "--dry-run",
    ];
    assert_rust_baseline_match(&stale_handle);

    let unsupported_type = [
        "--json",
        "docx",
        "comments",
        "edit",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        "--comment-id",
        "0",
        "--text",
        "Wrong package",
        "--dry-run",
    ];
    assert_rust_baseline_match(&unsupported_type);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn docx_comments_remove_matches_rust_baseline() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-docx-comments-remove-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx comments temp dir");
    let baseline_out = temp_dir.join("comments-remove-baseline.docx");
    let rust_out = temp_dir.join("comments-remove-rust.docx");
    let baseline_out = baseline_out.to_string_lossy().to_string();
    let rust_out = rust_out.to_string_lossy().to_string();

    let (hash_code, hash_stdout, hash_stderr) = run_ooxml_baseline(&[
        "--json",
        "docx",
        "comments",
        "list",
        "testdata/docx/with-comments/document.docx",
        "--comment-id",
        "0",
    ]);
    assert_eq!(hash_code, 0, "hash list exit");
    assert_eq!(hash_stderr, None, "hash list stderr");
    let hash_json = hash_stdout.expect("hash list JSON");
    let hash = hash_json["comments"][0]["contentHash"]
        .as_str()
        .expect("comment content hash");

    let baseline_args = [
        "--json",
        "docx",
        "comments",
        "remove",
        "testdata/docx/with-comments/document.docx",
        "--comment-id",
        "0",
        "--expect-hash",
        hash,
        "--out",
        &baseline_out,
    ];
    let rust_args = [
        "--json",
        "docx",
        "comments",
        "remove",
        "testdata/docx/with-comments/document.docx",
        "--comment-id",
        "0",
        "--expect-hash",
        hash,
        "--out",
        &rust_out,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "comments remove exit");
    assert_eq!(rust_stderr, baseline_stderr, "comments remove stderr");
    assert_eq!(rust_stdout, baseline_stdout, "comments remove stdout");
    assert!(Path::new(&rust_out).exists(), "Rust remove output missing");

    let remove_json = rust_stdout.expect("Rust remove JSON");
    assert_eq!(
        remove_json["operation"],
        Value::String("removed".to_string())
    );
    assert_eq!(remove_json["rangeMarkersRemoved"], Value::Bool(true));

    let (validate_code, validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &rust_out]);
    assert_eq!(validate_code, 0, "validate exit");
    assert_eq!(validate_stderr, None, "validate stderr");
    assert!(validate_stdout.is_some(), "validate stdout");

    let (baseline_list_code, baseline_list_stdout, baseline_list_stderr) =
        run_ooxml_baseline(&["--json", "docx", "comments", "list", &baseline_out]);
    let (rust_list_code, rust_list_stdout, rust_list_stderr) =
        run_ooxml(&["--json", "docx", "comments", "list", &rust_out]);
    assert_eq!(
        rust_list_code, baseline_list_code,
        "comments remove readback exit"
    );
    assert_eq!(
        rust_list_stderr, baseline_list_stderr,
        "comments remove readback stderr"
    );
    let baseline_list = baseline_list_stdout.expect("Rust baseline comments remove readback JSON");
    let rust_list = rust_list_stdout.expect("Rust comments remove readback JSON");
    assert_eq!(
        rust_list["comments"], baseline_list["comments"],
        "comments remove readback"
    );
    assert_eq!(rust_list["comments"], Value::Array(Vec::new()));

    let rust_document_xml = read_zip_string(Path::new(&rust_out), "word/document.xml");
    assert!(
        !rust_document_xml.contains("commentRangeStart")
            && !rust_document_xml.contains("commentRangeEnd")
            && !rust_document_xml.contains("commentReference"),
        "comment markers/reference survived removal:\n{rust_document_xml}"
    );

    let wrong_hash = [
        "--json",
        "docx",
        "comments",
        "remove",
        "testdata/docx/with-comments/document.docx",
        "--comment-id",
        "0",
        "--expect-hash",
        "sha256:bogus",
        "--dry-run",
    ];
    assert_rust_baseline_match(&wrong_hash);

    let by_handle = [
        "--json",
        "docx",
        "comments",
        "remove",
        "testdata/docx/with-comments/document.docx",
        "--handle",
        "H:docx/pt:doc/comment:n:0",
        "--dry-run",
    ];
    assert_rust_baseline_match(&by_handle);

    let stale_handle = [
        "--json",
        "docx",
        "comments",
        "remove",
        "testdata/docx/with-comments/document.docx",
        "--handle",
        "H:docx/pt:doc/comment:n:9999",
        "--dry-run",
    ];
    assert_rust_baseline_match(&stale_handle);

    let no_comments = [
        "--json",
        "docx",
        "comments",
        "remove",
        "testdata/docx/minimal/document.docx",
        "--comment-id",
        "0",
        "--dry-run",
    ];
    assert_rust_baseline_match(&no_comments);

    let missing_id = [
        "--json",
        "docx",
        "comments",
        "remove",
        "testdata/docx/with-comments/document.docx",
        "--dry-run",
    ];
    assert_rust_baseline_match(&missing_id);

    let unsupported_type = [
        "--json",
        "docx",
        "comments",
        "remove",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        "--comment-id",
        "0",
        "--dry-run",
    ];
    assert_rust_baseline_match(&unsupported_type);

    let _ = fs::remove_dir_all(&temp_dir);
}
