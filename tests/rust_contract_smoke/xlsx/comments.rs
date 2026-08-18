#[test]
fn xlsx_comments_add_update_remove_matches_rust_baseline_and_saved_output() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-xlsx-comments-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let baseline_in_path = temp_dir.join("baseline-in.xlsx");
    let rust_in_path = temp_dir.join("rust-in.xlsx");
    let baseline_added_path = temp_dir.join("baseline-added.xlsx");
    let rust_added_path = temp_dir.join("rust-added.xlsx");
    let baseline_updated_path = temp_dir.join("baseline-updated.xlsx");
    let rust_updated_path = temp_dir.join("rust-updated.xlsx");
    let baseline_removed_path = temp_dir.join("baseline-removed.xlsx");
    let rust_removed_path = temp_dir.join("rust-removed.xlsx");
    fs::copy(
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        &baseline_in_path,
    )
    .expect("baseline input");
    fs::copy(
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        &rust_in_path,
    )
    .expect("rust input");
    let baseline_in = baseline_in_path.to_string_lossy().to_string();
    let rust_in = rust_in_path.to_string_lossy().to_string();
    let baseline_added = baseline_added_path.to_string_lossy().to_string();
    let rust_added = rust_added_path.to_string_lossy().to_string();
    let baseline_updated = baseline_updated_path.to_string_lossy().to_string();
    let rust_updated = rust_updated_path.to_string_lossy().to_string();
    let baseline_removed = baseline_removed_path.to_string_lossy().to_string();
    let rust_removed = rust_removed_path.to_string_lossy().to_string();

    assert_rust_baseline_match(&[
        "--json",
        "xlsx",
        "comments",
        "list",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        "--sheet",
        "1",
    ]);

    let add_go = [
        "--json",
        "xlsx",
        "comments",
        "add",
        &baseline_in,
        "--sheet",
        "Sheet1",
        "--cell",
        "C3",
        "--author",
        "Ann",
        "--text",
        "before",
        "--out",
        &baseline_added,
    ];
    let add_rust = [
        "--json",
        "xlsx",
        "comments",
        "add",
        &rust_in,
        "--sheet",
        "Sheet1",
        "--cell",
        "C3",
        "--author",
        "Ann",
        "--text",
        "before",
        "--out",
        &rust_added,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&add_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&add_rust);
    assert_eq!(rust_code, baseline_code, "comments add exit");
    assert_eq!(rust_stderr, baseline_stderr, "comments add stderr");
    let rust_add = rust_stdout.expect("rust comments add stdout");
    assert_eq!(
        scrub_paths(
            rust_add.clone(),
            &[(&rust_in, "[IN]"), (&rust_added, "[OUT]")]
        ),
        scrub_paths(
            baseline_stdout.expect("baseline comments add stdout"),
            &[(&baseline_in, "[IN]"), (&baseline_added, "[OUT]")]
        ),
        "comments add stdout"
    );
    assert_eq!(rust_add["handle"], "H:xlsx/ws:1/comment:a:C3");
    assert_eq!(rust_add["createdPart"], Value::Bool(true));
    assert_eq!(rust_add["createdRef"], Value::Bool(true));
    assert_rust_emitted_ooxml_command_exits_zero(&rust_add, "validateCommand");
    assert_rust_emitted_ooxml_command_succeeds(&rust_add, "listCommand");

    assert!(zip_entry_exists(&rust_added_path, "xl/comments1.xml"));
    assert!(zip_entry_exists(
        &rust_added_path,
        "xl/drawings/vmlDrawing1.vml"
    ));
    let content_types = read_zip_string(&rust_added_path, "[Content_Types].xml");
    assert!(
        content_types
            .contains("application/vnd.openxmlformats-officedocument.spreadsheetml.comments+xml"),
        "missing comments content type:\n{content_types}"
    );
    assert!(
        content_types.contains("application/vnd.openxmlformats-officedocument.vmlDrawing"),
        "missing VML content type:\n{content_types}"
    );
    let sheet_rels = read_zip_string(&rust_added_path, "xl/worksheets/_rels/sheet1.xml.rels");
    assert!(
        sheet_rels.contains("/comments") && sheet_rels.contains("/vmlDrawing"),
        "worksheet rels missing comment/VML links:\n{sheet_rels}"
    );
    let sheet_xml = read_zip_string(&rust_added_path, "xl/worksheets/sheet1.xml");
    assert!(
        sheet_xml.contains("<legacyDrawing"),
        "worksheet missing legacyDrawing:\n{sheet_xml}"
    );

    let list_go = [
        "--json",
        "xlsx",
        "comments",
        "list",
        &baseline_added,
        "--sheet",
        "Sheet1",
    ];
    let list_rust = [
        "--json",
        "xlsx",
        "comments",
        "list",
        &rust_added,
        "--sheet",
        "Sheet1",
    ];
    let (baseline_code, baseline_list, baseline_stderr) = run_ooxml_baseline(&list_go);
    let (rust_code, rust_list, rust_stderr) = run_ooxml(&list_rust);
    assert_eq!(rust_code, baseline_code, "saved comments list exit");
    assert_eq!(rust_stderr, baseline_stderr, "saved comments list stderr");
    assert_eq!(
        scrub_path(
            rust_list.expect("rust saved comments list"),
            &rust_added,
            "[OUT]"
        ),
        scrub_path(
            baseline_list.expect("baseline saved comments list"),
            &baseline_added,
            "[OUT]"
        ),
        "saved comments list"
    );

    let dry_go = [
        "--json",
        "xlsx",
        "comments",
        "add",
        &baseline_in,
        "--sheet",
        "Sheet1",
        "--cell",
        "D4",
        "--author",
        "Dry",
        "--text",
        "preview",
        "--dry-run",
    ];
    let dry_rust = [
        "--json",
        "xlsx",
        "comments",
        "add",
        &rust_in,
        "--sheet",
        "Sheet1",
        "--cell",
        "D4",
        "--author",
        "Dry",
        "--text",
        "preview",
        "--dry-run",
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&dry_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_rust);
    assert_eq!(rust_code, baseline_code, "comments add dry-run exit");
    assert_eq!(rust_stderr, baseline_stderr, "comments add dry-run stderr");
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust comments add dry-run"),
            &rust_in,
            "[IN]"
        ),
        scrub_path(
            baseline_stdout.expect("baseline comments add dry-run"),
            &baseline_in,
            "[IN]"
        ),
        "comments add dry-run stdout"
    );
    assert!(
        !zip_entry_exists(&rust_in_path, "xl/comments1.xml"),
        "dry-run wrote comments part into Rust input"
    );

    let expect_hash = rust_add["contentHash"].as_str().expect("add hash");
    let update_go = [
        "--json",
        "xlsx",
        "comments",
        "update",
        &baseline_added,
        "--handle",
        "H:xlsx/ws:1/comment:a:C3",
        "--author",
        "Ben",
        "--text",
        "after",
        "--expect-hash",
        expect_hash,
        "--out",
        &baseline_updated,
    ];
    let update_rust = [
        "--json",
        "xlsx",
        "comments",
        "update",
        &rust_added,
        "--handle",
        "H:xlsx/ws:1/comment:a:C3",
        "--author",
        "Ben",
        "--text",
        "after",
        "--expect-hash",
        expect_hash,
        "--out",
        &rust_updated,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&update_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&update_rust);
    assert_eq!(rust_code, baseline_code, "comments update exit");
    assert_eq!(rust_stderr, baseline_stderr, "comments update stderr");
    let rust_update = rust_stdout.expect("rust comments update stdout");
    assert_eq!(
        scrub_paths(
            rust_update.clone(),
            &[(&rust_added, "[IN]"), (&rust_updated, "[OUT]")]
        ),
        scrub_paths(
            baseline_stdout.expect("baseline comments update stdout"),
            &[(&baseline_added, "[IN]"), (&baseline_updated, "[OUT]")]
        ),
        "comments update stdout"
    );
    assert_eq!(rust_update["previousText"], "before");
    assert_eq!(rust_update["author"], "Ben");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_update, "validateCommand");
    assert_rust_emitted_ooxml_command_succeeds(&rust_update, "listCommand");

    let stale_go = [
        "--json",
        "xlsx",
        "comments",
        "update",
        &baseline_added,
        "--comment-id",
        "0",
        "--text",
        "bad",
        "--expect-hash",
        "sha256:wrong",
        "--dry-run",
    ];
    let stale_rust = [
        "--json",
        "xlsx",
        "comments",
        "update",
        &rust_added,
        "--comment-id",
        "0",
        "--text",
        "bad",
        "--expect-hash",
        "sha256:wrong",
        "--dry-run",
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&stale_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&stale_rust);
    assert_eq!(rust_code, baseline_code, "comments update stale-hash exit");
    assert_eq!(
        rust_stdout, baseline_stdout,
        "comments update stale-hash stdout"
    );
    assert_eq!(
        rust_stderr, baseline_stderr,
        "comments update stale-hash stderr"
    );

    let duplicate_go = [
        "--json",
        "xlsx",
        "comments",
        "add",
        &baseline_added,
        "--sheet",
        "Sheet1",
        "--cell",
        "C3",
        "--author",
        "Ann",
        "--text",
        "duplicate",
        "--dry-run",
    ];
    let duplicate_rust = [
        "--json",
        "xlsx",
        "comments",
        "add",
        &rust_added,
        "--sheet",
        "Sheet1",
        "--cell",
        "C3",
        "--author",
        "Ann",
        "--text",
        "duplicate",
        "--dry-run",
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&duplicate_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&duplicate_rust);
    assert_eq!(rust_code, baseline_code, "comments duplicate add exit");
    assert_eq!(
        rust_stdout, baseline_stdout,
        "comments duplicate add stdout"
    );
    assert_eq!(
        rust_stderr, baseline_stderr,
        "comments duplicate add stderr"
    );

    let text_file_path = temp_dir.join("comment.txt");
    fs::write(&text_file_path, "from file").expect("comment text file");
    let text_file = text_file_path.to_string_lossy().to_string();
    let text_conflict_go = [
        "--json",
        "xlsx",
        "comments",
        "add",
        &baseline_in,
        "--sheet",
        "Sheet1",
        "--cell",
        "E5",
        "--author",
        "Ann",
        "--text",
        "inline",
        "--text-file",
        &text_file,
        "--dry-run",
    ];
    let text_conflict_rust = [
        "--json",
        "xlsx",
        "comments",
        "add",
        &rust_in,
        "--sheet",
        "Sheet1",
        "--cell",
        "E5",
        "--author",
        "Ann",
        "--text",
        "inline",
        "--text-file",
        &text_file,
        "--dry-run",
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&text_conflict_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&text_conflict_rust);
    assert_eq!(rust_code, baseline_code, "comments text conflict exit");
    assert_eq!(
        rust_stdout, baseline_stdout,
        "comments text conflict stdout"
    );
    assert_eq!(
        rust_stderr, baseline_stderr,
        "comments text conflict stderr"
    );

    let missing_go = [
        "--json",
        "xlsx",
        "comments",
        "list",
        &baseline_added,
        "--sheet",
        "Sheet1",
        "--comment-id",
        "9",
    ];
    let missing_rust = [
        "--json",
        "xlsx",
        "comments",
        "list",
        &rust_added,
        "--sheet",
        "Sheet1",
        "--comment-id",
        "9",
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&missing_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&missing_rust);
    assert_eq!(rust_code, baseline_code, "comments missing list exit");
    assert_eq!(rust_stdout, baseline_stdout, "comments missing list stdout");
    assert_eq!(rust_stderr, baseline_stderr, "comments missing list stderr");

    let updated_hash = rust_update["contentHash"].as_str().expect("updated hash");
    let remove_go = [
        "--json",
        "xlsx",
        "comments",
        "remove",
        &baseline_updated,
        "--sheet",
        "Sheet1",
        "--comment-id",
        "0",
        "--expect-hash",
        updated_hash,
        "--out",
        &baseline_removed,
    ];
    let remove_rust = [
        "--json",
        "xlsx",
        "comments",
        "remove",
        &rust_updated,
        "--sheet",
        "Sheet1",
        "--comment-id",
        "0",
        "--expect-hash",
        updated_hash,
        "--out",
        &rust_removed,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&remove_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&remove_rust);
    assert_eq!(rust_code, baseline_code, "comments remove exit");
    assert_eq!(rust_stderr, baseline_stderr, "comments remove stderr");
    let rust_remove = rust_stdout.expect("rust comments remove stdout");
    assert_eq!(
        scrub_paths(
            rust_remove.clone(),
            &[(&rust_updated, "[IN]"), (&rust_removed, "[OUT]")]
        ),
        scrub_paths(
            baseline_stdout.expect("baseline comments remove stdout"),
            &[(&baseline_updated, "[IN]"), (&baseline_removed, "[OUT]")]
        ),
        "comments remove stdout"
    );
    assert_eq!(rust_remove["previousAuthor"], "Ben");
    assert_eq!(rust_remove["previousText"], "after");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_remove, "validateCommand");
    assert_rust_emitted_ooxml_command_succeeds(&rust_remove, "listCommand");

    let (baseline_code, baseline_list, baseline_stderr) =
        run_ooxml_baseline(&["--json", "xlsx", "comments", "list", &baseline_removed]);
    let (rust_code, rust_list, rust_stderr) =
        run_ooxml(&["--json", "xlsx", "comments", "list", &rust_removed]);
    assert_eq!(rust_code, baseline_code, "removed comments list exit");
    assert_eq!(rust_stderr, baseline_stderr, "removed comments list stderr");
    assert_eq!(
        scrub_path(
            rust_list.expect("rust removed comments list"),
            &rust_removed,
            "[OUT]"
        ),
        scrub_path(
            baseline_list.expect("baseline removed comments list"),
            &baseline_removed,
            "[OUT]"
        ),
        "removed comments list"
    );
    assert!(!zip_entry_exists(&rust_removed_path, "xl/comments1.xml"));
    assert!(!zip_entry_exists(
        &rust_removed_path,
        "xl/drawings/vmlDrawing1.vml"
    ));
    let removed_sheet = read_zip_string(&rust_removed_path, "xl/worksheets/sheet1.xml");
    assert!(
        !removed_sheet.contains("<legacyDrawing"),
        "remove-last left legacyDrawing:\n{removed_sheet}"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}
