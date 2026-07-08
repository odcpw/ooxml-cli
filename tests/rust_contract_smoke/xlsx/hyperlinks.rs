#[test]
fn xlsx_hyperlinks_list_show_match_rust_baseline() {
    assert_rust_baseline_match(&[
        "--json",
        "xlsx",
        "hyperlinks",
        "list",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        "--sheet",
        "1",
    ]);

    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-hyperlinks-read-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let workbook = temp_dir.join("hyperlinks.xlsx");
    write_simple_xlsx_with_sheet_xml(
        &workbook,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheetData><row r="1"><c r="A1"><v>1</v></c></row></sheetData>
  <hyperlinks>
    <hyperlink ref="B2:A1" location="Sheet1!C3" display="Jump" tooltip="Tip"/>
    <hyperlink ref="$C$3" r:id="rIdMissing"/>
  </hyperlinks>
</worksheet>"#,
    );
    let workbook = workbook.to_string_lossy().to_string();

    for args in [
        vec![
            "--json",
            "xlsx",
            "hyperlinks",
            "list",
            &workbook,
            "--sheet",
            "Sheet1",
        ],
        vec![
            "--json",
            "xlsx",
            "hyperlinks",
            "list",
            &workbook,
            "--sheet",
            "sheetId:1",
            "--include-broken",
        ],
        vec![
            "--json",
            "xlsx",
            "hyperlinks",
            "show",
            &workbook,
            "--sheet",
            "1",
            "--cell",
            "B2:A1",
        ],
        vec![
            "--json", "xlsx", "links", "show", &workbook, "--sheet", "1", "--cell", "$C$3",
        ],
    ] {
        assert_rust_baseline_match(&args);
    }

    assert_rust_baseline_match(&[
        "--json",
        "xlsx",
        "hyperlinks",
        "show",
        &workbook,
        "--sheet",
        "1",
        "--cell",
        "Z9",
    ]);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_hyperlinks_mutations_match_rust_baseline_and_validate_saved_outputs() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-hyperlinks-mut-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let baseline_in_path = temp_dir.join("baseline-in.xlsx");
    let rust_in_path = temp_dir.join("rust-in.xlsx");
    let baseline_added_path = temp_dir.join("baseline-added.xlsx");
    let rust_added_path = temp_dir.join("rust-added.xlsx");
    let baseline_updated_path = temp_dir.join("baseline-updated.xlsx");
    let rust_updated_path = temp_dir.join("rust-updated.xlsx");
    let baseline_deleted_path = temp_dir.join("baseline-deleted.xlsx");
    let rust_deleted_path = temp_dir.join("rust-deleted.xlsx");
    fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &baseline_in_path).expect("baseline input");
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
    let baseline_deleted = baseline_deleted_path.to_string_lossy().to_string();
    let rust_deleted = rust_deleted_path.to_string_lossy().to_string();

    let add_go = [
        "--json",
        "xlsx",
        "hyperlinks",
        "add",
        &baseline_in,
        "--sheet",
        "Sheet1",
        "--cell",
        "A1",
        "--url",
        "https://example.com/report",
        "--display",
        "Details",
        "--tooltip",
        "Open report",
        "--out",
        &baseline_added,
    ];
    let add_rust = [
        "--json",
        "xlsx",
        "hyperlinks",
        "add",
        &rust_in,
        "--sheet",
        "Sheet1",
        "--cell",
        "A1",
        "--url",
        "https://example.com/report",
        "--display",
        "Details",
        "--tooltip",
        "Open report",
        "--out",
        &rust_added,
    ];
    let rust_add = assert_rust_baseline_match_scrubbed(
        "hyperlinks add",
        &add_go,
        &add_rust,
        &[
            (&baseline_in, "[IN]"),
            (&rust_in, "[IN]"),
            (&baseline_added, "[OUT]"),
            (&rust_added, "[OUT]"),
        ],
    )
    .expect("rust add stdout");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_add, "validateCommand");
    assert_rust_emitted_ooxml_command_succeeds(&rust_add, "hyperlinksListCommand");
    assert_xlsx_strict_valid(&rust_added);
    let added_sheet = read_zip_string(&rust_added_path, "xl/worksheets/sheet1.xml");
    assert!(
        added_sheet.contains("<hyperlinks") && added_sheet.contains(r#"ref="A1""#),
        "saved worksheet missing hyperlink element:\n{added_sheet}"
    );
    let added_rels = read_zip_string(&rust_added_path, "xl/worksheets/_rels/sheet1.xml.rels");
    assert!(
        added_rels.contains("/hyperlink")
            && added_rels.contains(r#"TargetMode="External""#)
            && added_rels.contains("https://example.com/report"),
        "saved worksheet rels missing external hyperlink:\n{added_rels}"
    );
    assert_rust_baseline_match_scrubbed(
        "hyperlinks added readback list",
        &[
            "--json",
            "xlsx",
            "hyperlinks",
            "list",
            &baseline_added,
            "--sheet",
            "1",
        ],
        &[
            "--json",
            "xlsx",
            "hyperlinks",
            "list",
            &rust_added,
            "--sheet",
            "1",
        ],
        &[(&baseline_added, "[OUT]"), (&rust_added, "[OUT]")],
    );
    assert_rust_baseline_match_scrubbed(
        "hyperlinks added readback show",
        &[
            "--json",
            "xlsx",
            "hyperlinks",
            "show",
            &baseline_added,
            "--sheet",
            "Sheet1",
            "--cell",
            "A1",
        ],
        &[
            "--json",
            "xlsx",
            "hyperlinks",
            "show",
            &rust_added,
            "--sheet",
            "Sheet1",
            "--cell",
            "A1",
        ],
        &[(&baseline_added, "[OUT]"), (&rust_added, "[OUT]")],
    );

    let update_go = [
        "--json",
        "xlsx",
        "hyperlinks",
        "update",
        &baseline_added,
        "--sheet",
        "Sheet1",
        "--cell",
        "A1",
        "--url",
        "https://example.net/new",
        "--display",
        "Updated",
        "--expect-url",
        "https://example.com/report",
        "--out",
        &baseline_updated,
    ];
    let update_rust = [
        "--json",
        "xlsx",
        "hyperlinks",
        "update",
        &rust_added,
        "--sheet",
        "Sheet1",
        "--cell",
        "A1",
        "--url",
        "https://example.net/new",
        "--display",
        "Updated",
        "--expect-url",
        "https://example.com/report",
        "--out",
        &rust_updated,
    ];
    let rust_update = assert_rust_baseline_match_scrubbed(
        "hyperlinks update",
        &update_go,
        &update_rust,
        &[
            (&baseline_added, "[IN]"),
            (&rust_added, "[IN]"),
            (&baseline_updated, "[OUT]"),
            (&rust_updated, "[OUT]"),
        ],
    )
    .expect("rust update stdout");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_update, "validateCommand");
    assert_rust_emitted_ooxml_command_succeeds(&rust_update, "hyperlinksListCommand");
    assert_xlsx_strict_valid(&rust_updated);
    let updated_rels = read_zip_string(&rust_updated_path, "xl/worksheets/_rels/sheet1.xml.rels");
    assert!(
        updated_rels.contains("https://example.net/new")
            && !updated_rels.contains("https://example.com/report"),
        "saved worksheet rels did not update target:\n{updated_rels}"
    );
    assert_rust_baseline_match_scrubbed(
        "hyperlinks updated readback list",
        &[
            "--json",
            "xlsx",
            "hyperlinks",
            "list",
            &baseline_updated,
            "--sheet",
            "1",
        ],
        &[
            "--json",
            "xlsx",
            "hyperlinks",
            "list",
            &rust_updated,
            "--sheet",
            "1",
        ],
        &[(&baseline_updated, "[OUT]"), (&rust_updated, "[OUT]")],
    );

    let delete_go = [
        "--json",
        "xlsx",
        "hyperlinks",
        "delete",
        &baseline_updated,
        "--sheet",
        "Sheet1",
        "--cell",
        "A1",
        "--expect-url",
        "https://example.net/new",
        "--out",
        &baseline_deleted,
    ];
    let delete_rust = [
        "--json",
        "xlsx",
        "hyperlinks",
        "delete",
        &rust_updated,
        "--sheet",
        "Sheet1",
        "--cell",
        "A1",
        "--expect-url",
        "https://example.net/new",
        "--out",
        &rust_deleted,
    ];
    let rust_delete = assert_rust_baseline_match_scrubbed(
        "hyperlinks delete",
        &delete_go,
        &delete_rust,
        &[
            (&baseline_updated, "[IN]"),
            (&rust_updated, "[IN]"),
            (&baseline_deleted, "[OUT]"),
            (&rust_deleted, "[OUT]"),
        ],
    )
    .expect("rust delete stdout");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_delete, "validateCommand");
    assert_rust_emitted_ooxml_command_succeeds(&rust_delete, "hyperlinksListCommand");
    assert_xlsx_strict_valid(&rust_deleted);
    assert_rust_baseline_match_scrubbed(
        "hyperlinks deleted readback list",
        &[
            "--json",
            "xlsx",
            "hyperlinks",
            "list",
            &baseline_deleted,
            "--sheet",
            "1",
        ],
        &[
            "--json",
            "xlsx",
            "hyperlinks",
            "list",
            &rust_deleted,
            "--sheet",
            "1",
        ],
        &[(&baseline_deleted, "[OUT]"), (&rust_deleted, "[OUT]")],
    );
    let deleted_sheet = read_zip_string(&rust_deleted_path, "xl/worksheets/sheet1.xml");
    assert!(
        !deleted_sheet.contains("<hyperlink"),
        "delete left hyperlink XML:\n{deleted_sheet}"
    );
    let deleted_rels = read_zip_string(&rust_deleted_path, "xl/worksheets/_rels/sheet1.xml.rels");
    assert!(
        !deleted_rels.contains("/hyperlink") && !deleted_rels.contains("https://example.net/new"),
        "delete left hyperlink rel:\n{deleted_rels}"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_hyperlinks_dry_run_and_errors_match_rust_baseline() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-hyperlinks-dry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let baseline_in_path = temp_dir.join("baseline-in.xlsx");
    let rust_in_path = temp_dir.join("rust-in.xlsx");
    let baseline_added_path = temp_dir.join("baseline-added.xlsx");
    let rust_added_path = temp_dir.join("rust-added.xlsx");
    fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &baseline_in_path).expect("baseline input");
    fs::copy(
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        &rust_in_path,
    )
    .expect("rust input");
    let baseline_in = baseline_in_path.to_string_lossy().to_string();
    let rust_in = rust_in_path.to_string_lossy().to_string();
    let baseline_added = baseline_added_path.to_string_lossy().to_string();
    let rust_added = rust_added_path.to_string_lossy().to_string();

    let before_sheet = read_zip_string(&rust_in_path, "xl/worksheets/sheet1.xml");
    let dry_go = [
        "--json",
        "xlsx",
        "hyperlinks",
        "add",
        &baseline_in,
        "--sheet",
        "Sheet1",
        "--cell",
        "B2",
        "--location",
        "Sheet1!A1",
        "--display",
        "Jump",
        "--dry-run",
    ];
    let dry_rust = [
        "--json",
        "xlsx",
        "hyperlinks",
        "add",
        &rust_in,
        "--sheet",
        "Sheet1",
        "--cell",
        "B2",
        "--location",
        "Sheet1!A1",
        "--display",
        "Jump",
        "--dry-run",
    ];
    let rust_dry = assert_rust_baseline_match_scrubbed(
        "hyperlinks add dry-run",
        &dry_go,
        &dry_rust,
        &[(&baseline_in, "[IN]"), (&rust_in, "[IN]")],
    )
    .expect("rust dry-run stdout");
    assert_eq!(rust_dry["dryRun"], Value::Bool(true));
    assert!(
        rust_dry.get("output").is_none(),
        "dry-run should not emit output"
    );
    assert!(
        rust_dry.get("hyperlinksListCommand").is_none(),
        "dry-run should not emit readback command"
    );
    assert_eq!(
        read_zip_string(&rust_in_path, "xl/worksheets/sheet1.xml"),
        before_sheet,
        "dry-run mutated source worksheet"
    );
    assert!(
        !zip_entry_exists(&rust_in_path, "xl/worksheets/_rels/sheet1.xml.rels"),
        "dry-run created worksheet rels"
    );

    for (label, extra) in [
        (
            "missing target",
            vec![
                "--sheet",
                "Sheet1",
                "--cell",
                "A1",
                "--display",
                "No target",
                "--dry-run",
            ],
        ),
        (
            "two targets",
            vec![
                "--sheet",
                "Sheet1",
                "--cell",
                "A1",
                "--url",
                "https://example.com",
                "--location",
                "Sheet1!A1",
                "--dry-run",
            ],
        ),
    ] {
        let mut baseline_args = vec!["--json", "xlsx", "hyperlinks", "add", &baseline_in];
        baseline_args.extend(extra.iter().copied());
        let mut rust_args = vec!["--json", "xlsx", "hyperlinks", "add", &rust_in];
        rust_args.extend(extra.iter().copied());
        assert_rust_baseline_match_scrubbed(
            &format!("hyperlinks add error {label}"),
            &baseline_args,
            &rust_args,
            &[(&baseline_in, "[IN]"), (&rust_in, "[IN]")],
        );
    }

    let add_go = [
        "--json",
        "xlsx",
        "hyperlinks",
        "add",
        &baseline_in,
        "--sheet",
        "Sheet1",
        "--cell",
        "A1",
        "--url",
        "https://example.com/original",
        "--out",
        &baseline_added,
    ];
    let add_rust = [
        "--json",
        "xlsx",
        "hyperlinks",
        "add",
        &rust_in,
        "--sheet",
        "Sheet1",
        "--cell",
        "A1",
        "--url",
        "https://example.com/original",
        "--out",
        &rust_added,
    ];
    assert_rust_baseline_match_scrubbed(
        "hyperlinks add setup",
        &add_go,
        &add_rust,
        &[
            (&baseline_in, "[IN]"),
            (&rust_in, "[IN]"),
            (&baseline_added, "[OUT]"),
            (&rust_added, "[OUT]"),
        ],
    );

    for (label, command, extra) in [
        (
            "guard mismatch",
            "update",
            vec![
                "--sheet",
                "Sheet1",
                "--cell",
                "A1",
                "--url",
                "https://example.com/next",
                "--expect-url",
                "https://wrong.example",
                "--dry-run",
            ],
        ),
        (
            "update two targets",
            "update",
            vec![
                "--sheet",
                "Sheet1",
                "--cell",
                "A1",
                "--url",
                "https://example.com/next",
                "--location",
                "Sheet1!B2",
                "--dry-run",
            ],
        ),
        (
            "delete guard mismatch",
            "delete",
            vec![
                "--sheet",
                "Sheet1",
                "--cell",
                "A1",
                "--expect-location",
                "Sheet1!B2",
                "--dry-run",
            ],
        ),
    ] {
        let mut baseline_args = vec!["--json", "xlsx", "hyperlinks", command, &baseline_added];
        baseline_args.extend(extra.iter().copied());
        let mut rust_args = vec!["--json", "xlsx", "hyperlinks", command, &rust_added];
        rust_args.extend(extra.iter().copied());
        assert_rust_baseline_match_scrubbed(
            &format!("hyperlinks {label}"),
            &baseline_args,
            &rust_args,
            &[(&baseline_added, "[IN]"), (&rust_added, "[IN]")],
        );
    }

    let _ = fs::remove_dir_all(&temp_dir);
}

